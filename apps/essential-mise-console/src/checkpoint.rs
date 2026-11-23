//! One checkpoint line from an apply log, and the check that follows it.
//!
//! A config hook prints
//! `essential-mise-checkpoint: {"check":"google-accounts","accounts":["user@example.com"]}`
//! and exits 0. After a successful apply the console checks those accounts
//! in-process. Nothing is passed to a shell.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, TryRecvError, channel};

/// Text that starts a checkpoint line, including the trailing space.
pub(crate) const PREFIX: &str = "essential-mise-checkpoint: ";

/// What `observe` has seen so far.
///
/// The first well-formed line stays. A later well-formed line does not
/// replace it. Any malformed checkpoint line becomes [`Record::Error`] and
/// stays there, even when a good line was seen earlier.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) enum Record {
    /// No checkpoint line yet.
    #[default]
    None,
    /// The first accounts list that parsed.
    Ready(Sentinel),
    /// A checkpoint line that did not parse. Still blocks.
    Error(String),
}

/// Accounts from a well-formed checkpoint line. Already ASCII-lowercased.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Sentinel {
    /// One to sixteen addresses, in line order, with no duplicates.
    pub(crate) accounts: Vec<String>,
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
    if map.len() != 2 || !map.contains_key("check") || !map.contains_key("accounts") {
        return Err("checkpoint line must contain only check and accounts".to_string());
    }
    let Some(check) = map.get("check").and_then(serde_json::Value::as_str) else {
        return Err("checkpoint check is not a string".to_string());
    };
    if check != "google-accounts" {
        return Err("checkpoint check is not google-accounts".to_string());
    }
    let Some(list) = map.get("accounts").and_then(serde_json::Value::as_array) else {
        return Err("checkpoint accounts is not an array".to_string());
    };
    if list.is_empty() || list.len() > 16 {
        return Err("checkpoint accounts must list 1 to 16 addresses".to_string());
    }
    let mut accounts = Vec::with_capacity(list.len());
    let mut seen = HashSet::with_capacity(list.len());
    for item in list {
        let Some(raw) = item.as_str() else {
            return Err("checkpoint account is not a string".to_string());
        };
        let Some(address) = crate::google_accounts::normalize_address(raw) else {
            return Err(format!("checkpoint account is not an email address: {raw}"));
        };
        if !seen.insert(address.clone()) {
            return Err(format!("checkpoint account is duplicated: {address}"));
        }
        accounts.push(address);
    }
    Ok(Sentinel { accounts })
}

/// Sentence used when a parsed report is not satisfied and `lines` is empty.
pub(crate) const EMPTY_REPORT: &str = "The accounts check did not say what is wrong.";

#[cfg(test)]
const STATUSES: &[&str] = &["ok", "gaps", "schema-mismatch", "unreadable"];

/// The checker's report, reduced to what the console shows.
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

    /// The database was scored and at least one account is not ok.
    pub(crate) fn gaps(&self) -> bool {
        self.status == "gaps"
    }
}

impl From<&crate::google_accounts::Report> for Report {
    fn from(report: &crate::google_accounts::Report) -> Self {
        Self {
            ok: report.ok,
            status: status_name(report.status).to_string(),
            title: report.title.clone(),
            lines: report.lines.clone(),
        }
    }
}

fn status_name(status: crate::google_accounts::ReportStatus) -> &'static str {
    match status {
        crate::google_accounts::ReportStatus::Ok => "ok",
        crate::google_accounts::ReportStatus::Gaps => "gaps",
        crate::google_accounts::ReportStatus::SchemaMismatch => "schema-mismatch",
        crate::google_accounts::ReportStatus::Unreadable => "unreadable",
    }
}

/// What to do with a recorded checkpoint before any process starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Plan {
    /// No sentinel. Exit 0.
    Clear,
    /// A malformed line. Exit 3 with this text, and do not open the database.
    Blocked(String),
    /// Check these accounts.
    Run(Sentinel),
}

/// Classify a record. A malformed line does not open the database.
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
    /// Whether the accounts check is part of reaching this decision.
    pub(crate) spawn: bool,
    /// Lines to print. Empty on success.
    pub(crate) lines: Vec<String>,
}

/// The pure exit decision. `check` is the result of the accounts check, if it ran.
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

/// Parse a checker report reduced to the fields the screen shows.
#[cfg(test)]
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

/// Which fixed action to run off the UI thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum JobKind {
    /// Score the accounts database.
    Check,
    /// Open the Internet Accounts pane.
    Open,
}

/// What the worker thread delivers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum JobOutput {
    /// A check result. `Err` is a worker failure, not a schema report.
    Report(Result<Report, String>),
    /// An open result. Failure stays on the screen.
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
pub(crate) fn spawn_job(db: PathBuf, accounts: Vec<String>, kind: JobKind) -> CheckpointHandle {
    let (tx, rx) = channel();
    let spawned = std::thread::Builder::new()
        .name("emc-checkpoint".to_string())
        .spawn({
            let tx = tx.clone();
            move || {
                let _ = tx.send(run_job(&db, &accounts, kind));
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

/// Run the accounts check on this thread. Headless apply uses this. It does
/// not open a screen or read a terminal.
pub(crate) fn run_check(db: &Path, accounts: &[String]) -> Result<Report, String> {
    match run_job(db, accounts, JobKind::Check) {
        JobOutput::Report(result) => result,
        JobOutput::Opened(_) => Err("the accounts check did not return a report".to_string()),
    }
}

fn run_job(db: &Path, accounts: &[String], kind: JobKind) -> JobOutput {
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
    match kind {
        JobKind::Check => JobOutput::Report(Ok(Report::from(&crate::google_accounts::check(
            db, accounts,
        )))),
        JobKind::Open => JobOutput::Opened(crate::google_accounts::open_accounts_pane()),
    }
}

/// `EMC_MOCK_CHECKPOINT=ok|gaps|schema`, without opening sqlite.
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

#[cfg(test)]
mod tests {
    use super::*;

    fn line(accounts: &str) -> String {
        format!(r#"{PREFIX}{{"check":"google-accounts","accounts":[{accounts}]}}"#)
    }

    fn one(address: &str) -> String {
        line(&format!("\"{address}\""))
    }

    #[test]
    fn parses_accounts_and_ignores_key_order() {
        let sentinel = parse_line(&one("User@Example.com"))
            .expect("checkpoint")
            .expect("ok");
        assert_eq!(sentinel.accounts, vec!["user@example.com".to_string()]);

        let swapped = format!(
            r#"{PREFIX}{{"accounts":["a@example.com","b@example.com"],"check":"google-accounts"}}"#
        );
        let sentinel = parse_line(&swapped).expect("checkpoint").expect("ok");
        assert_eq!(
            sentinel.accounts,
            vec!["a@example.com".to_string(), "b@example.com".to_string()]
        );
    }

    #[test]
    fn ignores_a_line_without_the_prefix() {
        assert!(parse_line("phase 1: base").is_none());
        assert!(
            parse_line(
                "essential-mise-checkpoint:{\"check\":\"google-accounts\",\"accounts\":[\"user@example.com\"]}"
            )
            .is_none()
        );
    }

    #[test]
    fn rejects_a_malformed_checkpoint_line() {
        let mut many = Vec::new();
        for index in 0..17 {
            many.push(format!("\"user{index}@example.com\""));
        }
        let cases = [
            one("not-an-email"),
            line("\"user@example.com\",\"user@example.com\""),
            line("\"User@Example.com\",\"user@example.com\""),
            format!("{PREFIX}{{\"check\":\"google-accounts\",\"accounts\":[]}}"),
            format!(
                "{PREFIX}{{\"check\":\"google-accounts\",\"accounts\":[{}]}}",
                many.join(",")
            ),
            format!("{PREFIX}{{\"check\":\"google-accounts\",\"accounts\":\"user@example.com\"}}"),
            format!(
                "{PREFIX}{{\"check\":\"google-accounts\",\"accounts\":[\"user@example.com\"],\"script\":\"bin/example.ts\"}}"
            ),
            format!(r#"{PREFIX}{{"script":"bin/example.ts"}}"#),
            format!(r#"{PREFIX}{{"check":"other","accounts":["user@example.com"]}}"#),
            format!(r#"{PREFIX}{{"accounts":["user@example.com"]}}"#),
            format!("{PREFIX}not-json"),
            format!("{PREFIX}[]"),
            format!("{PREFIX}{{\"check\":\"google-accounts\",\"accounts\":[1]}}"),
            format!(
                "{PREFIX}{{\"check\":\"google-accounts\",\"accounts\":[\"user@example.com\"]}} trailing"
            ),
        ];
        for case in cases {
            let err = parse_line(&case).expect("prefix").expect_err(&case);
            assert!(!err.is_empty(), "{case}");
        }
    }

    #[test]
    fn first_line_wins_and_a_bad_line_still_blocks() {
        let mut record = Record::None;
        observe(&mut record, &one("user@example.com"));
        observe(&mut record, &one("other@example.com"));
        assert_eq!(
            record,
            Record::Ready(Sentinel {
                accounts: vec!["user@example.com".into()],
            })
        );
        observe(&mut record, &format!("{PREFIX}{{}}"));
        assert!(matches!(record, Record::Error(_)));
        let error = record.clone();
        observe(&mut record, &one("user@example.com"));
        assert_eq!(record, error);

        let mut only_bad = Record::None;
        observe(&mut only_bad, &format!("{PREFIX}{{}}"));
        observe(&mut only_bad, &one("user@example.com"));
        assert!(matches!(only_bad, Record::Error(_)));
    }

    fn ready() -> Record {
        Record::Ready(Sentinel {
            accounts: vec!["user@example.com".into()],
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
    fn exit_decision_checks_only_a_well_formed_sentinel() {
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

        let failed = decide(&ready(), Some(Err("the accounts check stopped".into())));
        assert_eq!(failed.code, 3);
        assert_eq!(failed.lines, vec!["the accounts check stopped".to_string()]);

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
