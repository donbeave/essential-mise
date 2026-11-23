//! One checkpoint line from an apply log, and the check that follows it.
//!
//! A config hook prints `essential-mise-checkpoint: {"script":"bin/<name>.ts"}`
//! and exits 0. The console keeps that script, confined to the config
//! directory. After a successful apply it runs `check --json` itself. The
//! script is never passed to a shell, and `guide` is never invoked.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, TryRecvError, channel};

/// Text that starts a checkpoint line, including the trailing space.
pub(crate) const PREFIX: &str = "essential-mise-checkpoint: ";

/// What `push_line` has seen so far.
///
/// The first well-formed script stays. A later well-formed line does not
/// replace it. Any malformed checkpoint line becomes [`Record::Error`] and
/// stays there, so a bad line cannot be skipped.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) enum Record {
    /// No checkpoint line yet.
    #[default]
    None,
    /// The first script that parsed.
    Ready(Sentinel),
    /// A checkpoint line that did not parse. Still blocks.
    Error(String),
}

/// A script path that matched the checkpoint rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Sentinel {
    /// Relative path, `bin/<name>.ts`.
    pub(crate) script: String,
}

/// Fold one apply-log line into `record`.
pub(crate) fn observe(record: &mut Record, line: &str) {
    let Some(parsed) = parse_line(line) else {
        return;
    };
    match parsed {
        Err(message) => {
            if !matches!(record, Record::Error(_)) {
                *record = Record::Error(message);
            }
        }
        Ok(sentinel) => {
            if matches!(record, Record::None) {
                *record = Record::Ready(sentinel);
            }
        }
    }
}

/// Parse one line. `None` when it is not a checkpoint line.
pub(crate) fn parse_line(line: &str) -> Option<Result<Sentinel, String>> {
    let body = line.strip_prefix(PREFIX)?;
    Some(parse_body(body))
}

fn parse_body(body: &str) -> Result<Sentinel, String> {
    let value: serde_json::Value = serde_json::from_str(body)
        .map_err(|_| "checkpoint line is not a JSON object".to_string())?;
    let serde_json::Value::Object(map) = value else {
        return Err("checkpoint line is not a JSON object".to_string());
    };
    if map.len() != 1 || !map.contains_key("script") {
        return Err("checkpoint line must contain only script".to_string());
    }
    let Some(script) = map.get("script").and_then(serde_json::Value::as_str) else {
        return Err("checkpoint script is not a string".to_string());
    };
    if !script_name_ok(script) {
        return Err(format!("checkpoint script is not under bin/: {script}"));
    }
    Ok(Sentinel {
        script: script.to_string(),
    })
}

/// `^bin/[A-Za-z0-9._-]+\.ts$`
fn script_name_ok(script: &str) -> bool {
    let Some(name) = script
        .strip_prefix("bin/")
        .and_then(|rest| rest.strip_suffix(".ts"))
    else {
        return false;
    };
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// Resolve `script` under `config_dir`.
///
/// The path is canonicalized. A path that leaves the config directory,
/// including through a symlink, is refused. A missing script is an error.
/// Nothing is executed.
pub(crate) fn resolve_script(config_dir: &Path, script: &str) -> Result<PathBuf, String> {
    if !script_name_ok(script) {
        return Err(format!("checkpoint script is not under bin/: {script}"));
    }
    let config = config_dir
        .canonicalize()
        .map_err(|err| format!("config directory: {err}"))?;
    let candidate = config.join(script);
    let resolved = candidate
        .canonicalize()
        .map_err(|err| format!("script: {err}"))?;
    if resolved == config || !resolved.starts_with(&config) {
        return Err("script escapes the config directory".to_string());
    }
    if !resolved.is_file() {
        return Err("script is missing".to_string());
    }
    Ok(resolved)
}

/// Sentence used when a parsed report is not satisfied and `lines` is empty.
pub(crate) const EMPTY_REPORT: &str = "The accounts check did not say what is wrong.";

/// Bun the console runs. The script's arguments are only `check` or `open`.
pub(crate) const BUN: &str = "/opt/homebrew/bin/bun";

const STATUSES: &[&str] = &["ok", "gaps", "schema-mismatch", "unreadable"];

/// The checker's stdout, reduced to what the console shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Report {
    /// The checker's `ok` flag. Satisfied only together with `status == "ok"`.
    pub(crate) ok: bool,
    /// `ok`, `gaps`, `schema-mismatch`, or `unreadable`.
    pub(crate) status: String,
    /// Shown above the lines.
    pub(crate) title: String,
    /// The sentences the person reads.
    pub(crate) lines: Vec<String>,
}

impl Report {
    /// Exactly satisfied: both fields, not either one alone.
    pub(crate) fn satisfied(&self) -> bool {
        self.ok && self.status == "ok"
    }

    pub(crate) fn gaps(&self) -> bool {
        self.status == "gaps"
    }
}

/// What to do with a recorded checkpoint before any process starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Plan {
    /// No sentinel. Exit 0.
    Clear,
    /// A malformed line. Exit 3 with this text, and do not spawn.
    Blocked(String),
    /// Run `check --json` for this script.
    Run(Sentinel),
}

/// Classify a record. A malformed line does not spawn, even if a script was seen earlier.
pub(crate) fn plan(record: &Record) -> Plan {
    match record {
        Record::None => Plan::Clear,
        Record::Error(text) => Plan::Blocked(text.clone()),
        Record::Ready(sentinel) => Plan::Run(sentinel.clone()),
    }
}

/// Exit after a successful apply.
///
/// `check` is `None` when no check ran. `spawn` is true only when a check was
/// required; a malformed record keeps it false.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExitDecision {
    /// Process exit code.
    pub(crate) code: u8,
    /// Whether `check --json` is part of reaching this decision.
    pub(crate) spawn: bool,
    /// Lines to print. Empty on success.
    pub(crate) lines: Vec<String>,
}

/// The pure exit decision. `check` is the result of `check --json`, if it ran.
pub(crate) fn decide(record: &Record, check: Option<Result<Report, String>>) -> ExitDecision {
    match record {
        Record::None => ExitDecision {
            code: 0,
            spawn: false,
            lines: Vec::new(),
        },
        Record::Error(text) => ExitDecision {
            code: 3,
            spawn: false,
            lines: vec![text.clone()],
        },
        Record::Ready(_) => match check {
            Some(Ok(report)) if report.satisfied() => ExitDecision {
                code: 0,
                spawn: true,
                lines: Vec::new(),
            },
            Some(Ok(report)) => ExitDecision {
                code: 3,
                spawn: true,
                lines: shown_lines(&report),
            },
            Some(Err(text)) => ExitDecision {
                code: 3,
                spawn: true,
                lines: vec![text],
            },
            None => ExitDecision {
                code: 3,
                spawn: true,
                lines: vec!["the accounts check did not run".to_string()],
            },
        },
    }
}

/// Lines the person reads. An empty unsatisfied report gets one fixed sentence.
pub(crate) fn shown_lines(report: &Report) -> Vec<String> {
    if report.lines.is_empty() && !report.satisfied() {
        vec![EMPTY_REPORT.to_string()]
    } else {
        report.lines.clone()
    }
}

/// Parse the checker's stdout. Unknown statuses and a non-object are errors.
pub(crate) fn parse_report(stdout: &str) -> Result<Report, String> {
    let value: serde_json::Value = serde_json::from_str(stdout.trim())
        .map_err(|_| "the accounts check did not return a report".to_string())?;
    let Some(map) = value.as_object() else {
        return Err("the accounts check did not return a report".to_string());
    };
    let Some(ok) = map.get("ok").and_then(serde_json::Value::as_bool) else {
        return Err("the accounts check did not return a report".to_string());
    };
    let Some(status) = map.get("status").and_then(serde_json::Value::as_str) else {
        return Err("the accounts check did not return a report".to_string());
    };
    if !STATUSES.contains(&status) {
        return Err(format!("unknown status: {status}"));
    }
    let Some(title) = map.get("title").and_then(serde_json::Value::as_str) else {
        return Err("the accounts check did not return a report".to_string());
    };
    let Some(raw_lines) = map.get("lines").and_then(serde_json::Value::as_array) else {
        return Err("the accounts check did not return a report".to_string());
    };
    let mut lines = Vec::with_capacity(raw_lines.len());
    for line in raw_lines {
        let Some(text) = line.as_str() else {
            return Err("the accounts check did not return a report".to_string());
        };
        lines.push(text.to_string());
    }
    Ok(Report {
        ok,
        status: status.to_string(),
        title: title.to_string(),
        lines,
    })
}

/// Which fixed argv to run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum JobKind {
    /// `bun <script> check --json`
    Check,
    /// `bun <script> open`
    Open,
}

/// What the worker thread delivers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum JobOutput {
    /// A `check --json` result. `Err` is a worker failure, not a schema report.
    Report(Result<Report, String>),
    /// An `open` result. Failure stays on the screen.
    Opened(Result<(), String>),
}

/// A check or open running off the UI thread.
pub(crate) struct CheckpointHandle {
    rx: Receiver<JobOutput>,
}

impl CheckpointHandle {
    /// The result, once the worker has finished. Never blocks.
    pub(crate) fn poll(&self) -> Option<JobOutput> {
        match self.rx.try_recv() {
            Ok(result) => Some(result),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(JobOutput::Report(Err(
                "the accounts check stopped unexpectedly".to_string(),
            ))),
        }
    }
}

/// Start `check` or `open` on a worker thread.
pub(crate) fn spawn_job(config_dir: PathBuf, script: String, kind: JobKind) -> CheckpointHandle {
    let (tx, rx) = channel();
    let spawned = std::thread::Builder::new()
        .name("emc-checkpoint".to_string())
        .spawn({
            let tx = tx.clone();
            move || {
                let _ = tx.send(run_job(&config_dir, &script, kind));
            }
        });
    if let Err(err) = spawned {
        let message = format!("could not start the accounts check: {err}");
        let output = match kind {
            JobKind::Check => JobOutput::Report(Err(message)),
            JobKind::Open => JobOutput::Opened(Err(message)),
        };
        let _ = tx.send(output);
    }
    CheckpointHandle { rx }
}

/// Run `check --json` on this thread. Headless apply uses this. It does not
/// open a screen, read a terminal, or invoke `guide`.
pub(crate) fn run_check(config_dir: &Path, script: &str) -> Result<Report, String> {
    match run_job(config_dir, script, JobKind::Check) {
        JobOutput::Report(result) => result,
        JobOutput::Opened(_) => Err("the accounts check did not return a report".to_string()),
    }
}

fn run_job(config_dir: &Path, script: &str, kind: JobKind) -> JobOutput {
    if let Some(mocked) = mocked_report() {
        return match kind {
            JobKind::Check => JobOutput::Report(mocked),
            JobKind::Open => JobOutput::Opened(Ok(())),
        };
    }
    if std::env::var("EMC_MOCK").ok().as_deref() == Some("1") {
        let message = "EMC_MOCK=1 did not set EMC_MOCK_CHECKPOINT".to_string();
        return match kind {
            JobKind::Check => JobOutput::Report(Err(message)),
            JobKind::Open => JobOutput::Opened(Err(message)),
        };
    }
    let path = match resolve_script(config_dir, script) {
        Ok(path) => path,
        Err(err) => {
            return match kind {
                JobKind::Check => JobOutput::Report(Err(err)),
                JobKind::Open => JobOutput::Opened(Err(err)),
            };
        }
    };
    let args: &[&str] = match kind {
        JobKind::Check => &["check", "--json"],
        JobKind::Open => &["open"],
    };
    match run_bun(&path, args) {
        Ok(output) => interpret(kind, &output),
        Err(err) => match kind {
            JobKind::Check => JobOutput::Report(Err(err)),
            JobKind::Open => JobOutput::Opened(Err(err)),
        },
    }
}

/// `EMC_MOCK_CHECKPOINT=ok|gaps|schema`, without starting bun.
fn mocked_report() -> Option<Result<Report, String>> {
    let value = std::env::var("EMC_MOCK_CHECKPOINT").ok()?;
    Some(match value.as_str() {
        "ok" => Ok(example_report(true, "ok", Vec::new())),
        "gaps" => Ok(example_report(
            false,
            "gaps",
            vec![
                "user@example.com is not an Internet Account on this Mac. Add it and choose Google. Turn Mail and Calendar on."
                    .to_string(),
            ],
        )),
        "schema" => Ok(example_report(
            false,
            "schema-mismatch",
            vec![
                "The Accounts database is not in the shape this check understands (example). Nothing was treated as missing. System Settings was not opened."
                    .to_string(),
            ],
        )),
        other => Err(format!(
            "EMC_MOCK_CHECKPOINT={other} is not ok, gaps, or schema"
        )),
    })
}

fn example_report(ok: bool, status: &str, lines: Vec<String>) -> Report {
    Report {
        ok,
        status: status.to_string(),
        title: "Google accounts".to_string(),
        lines,
    }
}

fn run_bun(script: &Path, args: &[&str]) -> Result<std::process::Output, String> {
    let bun = Path::new(BUN);
    if !bun.is_file() {
        return Err(format!(
            "{BUN} is missing, so the accounts were not checked"
        ));
    }
    Command::new(bun)
        .arg(script)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|err| format!("could not run the accounts check: {err}"))
}

fn interpret(kind: JobKind, output: &std::process::Output) -> JobOutput {
    if !output.status.success() {
        let code = output
            .status
            .code()
            .map(|code| code.to_string())
            .unwrap_or_else(|| "no status".to_string());
        let stderr = stderr_text(&output.stderr);
        let text = if stderr.is_empty() {
            format!("the accounts check failed ({code})")
        } else {
            stderr
        };
        return match kind {
            JobKind::Check => JobOutput::Report(Err(text)),
            JobKind::Open => JobOutput::Opened(Err(text)),
        };
    }
    match kind {
        JobKind::Check => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            JobOutput::Report(parse_report(&stdout))
        }
        JobKind::Open => JobOutput::Opened(Ok(())),
    }
}

fn stderr_text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(script: &str) -> String {
        format!("{PREFIX}{{\"script\":\"{script}\"}}")
    }

    #[test]
    fn parses_one_script_object() {
        let sentinel = parse_line(&line("bin/example.ts"))
            .expect("checkpoint")
            .expect("ok");
        assert_eq!(sentinel.script, "bin/example.ts");
    }

    #[test]
    fn ignores_a_line_without_the_prefix() {
        assert!(parse_line("phase 1: base").is_none());
        assert!(parse_line("essential-mise-checkpoint:{\"script\":\"bin/example.ts\"}").is_none());
    }

    #[test]
    fn rejects_extra_keys_bad_json_and_a_script_outside_bin() {
        let cases = [
            format!("{PREFIX}{{\"script\":\"bin/example.ts\",\"ok\":true}}"),
            format!("{PREFIX}not-json"),
            format!("{PREFIX}[]"),
            format!("{PREFIX}{{\"script\":1}}"),
            line("bin/../example.ts"),
            line("/tmp/example.ts"),
            line("bin/example.js"),
            line("bin/.ts"),
            format!("{PREFIX}{{\"script\":\"bin/example.ts\"}} trailing"),
        ];
        for case in cases {
            let err = parse_line(&case).expect("prefix").expect_err(&case);
            assert!(!err.is_empty(), "{case}");
        }
    }

    #[test]
    fn first_script_wins_and_a_bad_line_still_blocks() {
        let mut record = Record::None;
        observe(&mut record, &line("bin/example.ts"));
        observe(&mut record, &line("bin/other.ts"));
        assert_eq!(
            record,
            Record::Ready(Sentinel {
                script: "bin/example.ts".into(),
            })
        );
        observe(&mut record, &format!("{PREFIX}{{}}"));
        assert!(matches!(record, Record::Error(_)));
        let error = record.clone();
        observe(&mut record, &line("bin/example.ts"));
        assert_eq!(record, error);

        let mut only_bad = Record::None;
        observe(&mut only_bad, &format!("{PREFIX}{{}}"));
        observe(&mut only_bad, &line("bin/example.ts"));
        assert!(matches!(only_bad, Record::Error(_)));
    }

    #[test]
    fn resolve_stays_inside_the_config_directory() {
        let root = tempfile::tempdir().expect("temp");
        let config = root.path().join("config");
        std::fs::create_dir_all(config.join("bin")).expect("bin");
        std::fs::write(config.join("bin/example.ts"), "// example\n").expect("script");
        let resolved = resolve_script(&config, "bin/example.ts").expect("inside");
        let config = config.canonicalize().expect("config");
        assert!(resolved.starts_with(&config));
        assert!(resolved.ends_with("bin/example.ts"));
        assert!(resolved.is_file());
    }

    #[test]
    fn resolve_allows_a_symlink_that_stays_inside() {
        let root = tempfile::tempdir().expect("temp");
        let config = root.path().join("config");
        std::fs::create_dir_all(config.join("bin")).expect("bin");
        std::fs::write(config.join("bin/real.ts"), "// example\n").expect("script");
        std::os::unix::fs::symlink("real.ts", config.join("bin/example.ts")).expect("symlink");
        let resolved = resolve_script(&config, "bin/example.ts").expect("inside symlink");
        assert!(resolved.ends_with("bin/real.ts"));
        assert!(resolved.starts_with(config.canonicalize().expect("config")));
    }

    #[test]
    fn resolve_refuses_a_symlink_that_leaves_the_config_directory() {
        let root = tempfile::tempdir().expect("temp");
        let config = root.path().join("config");
        let outside = root.path().join("outside.ts");
        std::fs::create_dir_all(config.join("bin")).expect("bin");
        std::fs::write(&outside, "// outside\n").expect("outside");
        std::os::unix::fs::symlink(&outside, config.join("bin/example.ts")).expect("symlink");
        let err = resolve_script(&config, "bin/example.ts").expect_err("escape");
        assert!(err.contains("escapes"), "{err}");
    }

    #[test]
    fn resolve_refuses_a_missing_script_and_a_bad_name() {
        let root = tempfile::tempdir().expect("temp");
        let config = root.path().join("config");
        std::fs::create_dir_all(config.join("bin")).expect("bin");
        let missing = resolve_script(&config, "bin/example.ts").expect_err("missing");
        assert!(missing.contains("script"), "{missing}");
        let bad = resolve_script(&config, "bin/../example.ts").expect_err("regex");
        assert!(bad.contains("bin/"), "{bad}");
    }

    fn ready() -> Record {
        Record::Ready(Sentinel {
            script: "bin/example.ts".into(),
        })
    }

    fn report(ok: bool, status: &str, lines: &[&str]) -> Report {
        Report {
            ok,
            status: status.to_string(),
            title: "Google accounts".to_string(),
            lines: lines.iter().map(|line| (*line).to_string()).collect(),
        }
    }

    #[test]
    fn exit_decision_spawns_only_for_a_well_formed_sentinel() {
        let clear = decide(&Record::None, None);
        assert_eq!(clear.code, 0);
        assert!(!clear.spawn);

        let blocked = decide(&Record::Error("bad checkpoint".into()), None);
        assert_eq!(blocked.code, 3);
        assert!(!blocked.spawn);
        assert_eq!(blocked.lines, vec!["bad checkpoint".to_string()]);
        assert!(matches!(
            plan(&Record::Error("bad checkpoint".into())),
            Plan::Blocked(_)
        ));

        let ok = decide(&ready(), Some(Ok(report(true, "ok", &[]))));
        assert_eq!(ok.code, 0);
        assert!(ok.spawn);
        assert!(ok.lines.is_empty());

        let gaps = decide(
            &ready(),
            Some(Ok(report(false, "gaps", &["user@example.com is missing"]))),
        );
        assert_eq!(gaps.code, 3);
        assert_eq!(gaps.lines, vec!["user@example.com is missing".to_string()]);

        let schema = decide(
            &ready(),
            Some(Ok(report(false, "schema-mismatch", &["shape"]))),
        );
        assert_eq!(schema.code, 3);
        assert_eq!(schema.lines, vec!["shape".to_string()]);

        let failed = decide(&ready(), Some(Err("bun is missing".into())));
        assert_eq!(failed.code, 3);
        assert_eq!(failed.lines, vec!["bun is missing".to_string()]);

        let status_without_ok = decide(&ready(), Some(Ok(report(false, "ok", &[]))));
        assert_eq!(status_without_ok.code, 3);
        assert_eq!(status_without_ok.lines, vec![EMPTY_REPORT.to_string()]);

        let ok_with_gaps = decide(
            &ready(),
            Some(Ok(report(true, "gaps", &["user@example.com is missing"]))),
        );
        assert_eq!(ok_with_gaps.code, 3);
        assert_eq!(
            ok_with_gaps.lines,
            vec!["user@example.com is missing".to_string()]
        );
    }

    #[test]
    fn unknown_status_is_not_a_report() {
        let raw = r#"{"ok":false,"status":"later","title":"Google accounts","lines":[]}"#;
        let err = parse_report(raw).expect_err("unknown");
        assert!(err.contains("unknown status"), "{err}");
    }
}
