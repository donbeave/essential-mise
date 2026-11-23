//! One checkpoint line from an apply log.
//!
//! A config hook prints `essential-mise-checkpoint: {"script":"bin/<name>.ts"}`
//! and exits 0. The console keeps that script, confined to the config
//! directory, and decides later whether the apply is finished. This module
//! does not spawn a process and does not pass the script to a shell.

use std::path::{Path, PathBuf};

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
#[allow(dead_code)] // the checkpoint screen calls this; this commit only records
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
}
