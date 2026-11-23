//! Backend unit tests (T2.1): origin parsing over synthetic TOML.

use super::*;
use std::io::Write as _;
use std::sync::atomic::{AtomicU64, Ordering};

static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

/// Fresh temp dir for one test; removed afterwards.
fn test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "emc-test-{name}-{}-{}",
        std::process::id(),
        TEST_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).expect("create test dir");
    dir
}

fn write_env(dir: &Path, env: &str, body: &str) {
    let mut file = std::fs::File::create(dir.join(format!("config.{env}.toml")))
        .expect("create synthetic env toml");
    file.write_all(body.as_bytes())
        .expect("write synthetic env toml");
}

#[test]
fn stream_child_sets_emc_apply_only_on_the_child() {
    let dir = test_dir("emc-apply-env");
    let log_path = dir.join("apply.log");
    let log_file = std::fs::File::create(&log_path).expect("log");
    let mut log = std::io::BufWriter::new(log_file);
    let current = Arc::new(Mutex::new(None));
    let mut lines = Vec::new();
    let before = std::env::var("EMC_APPLY").ok();
    let mut emit = |_stream: Stream, text: String, _log: &mut std::io::BufWriter<std::fs::File>| {
        lines.push(text);
    };
    let status = stream_child(
        Path::new("/usr/bin/printenv"),
        &["EMC_APPLY".to_string()],
        &dir,
        &mut emit,
        &mut log,
        &current,
        true,
    )
    .expect("printenv");
    assert_eq!(status, None, "printenv exits 0 when EMC_APPLY is set");
    assert_eq!(lines, vec!["1".to_string()]);
    assert_eq!(std::env::var("EMC_APPLY").ok(), before);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stream_child_leaves_emc_apply_unset_for_setup() {
    let dir = test_dir("emc-apply-unset");
    let log_path = dir.join("apply.log");
    let log_file = std::fs::File::create(&log_path).expect("log");
    let mut log = std::io::BufWriter::new(log_file);
    let current = Arc::new(Mutex::new(None));
    let mut lines = Vec::new();
    let before = std::env::var("EMC_APPLY").ok();
    let mut emit = |_stream: Stream, text: String, _log: &mut std::io::BufWriter<std::fs::File>| {
        lines.push(text);
    };
    let status = stream_child(
        Path::new("/usr/bin/printenv"),
        &["EMC_APPLY".to_string()],
        &dir,
        &mut emit,
        &mut log,
        &current,
        false,
    )
    .expect("printenv");
    assert_eq!(std::env::var("EMC_APPLY").ok(), before);
    match before {
        Some(value) => {
            assert_eq!(status, None);
            assert_eq!(lines, vec![value]);
        }
        None => {
            assert_eq!(status, Some(1));
            assert!(lines.is_empty());
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn query_captures_large_outputs_before_its_deadline() {
    let mut command = Command::new("/bin/sh");
    command.args([
        "-c",
        "head -c 1048576 /dev/zero; head -c 1048576 /dev/zero >&2",
    ]);
    let output = run_with_timeout(&mut command, Duration::from_secs(5)).expect("query completes");
    assert!(output.status.success());
    assert_eq!(output.stdout.len(), 1_048_576);
    assert_eq!(output.stderr.len(), 1_048_576);
}

#[test]
fn default_mise_bin_uses_the_setup_install_when_present() {
    let home = test_dir("mise-home");
    let local_bin = home.join(".local/bin");
    std::fs::create_dir_all(&local_bin).expect("create local bin");
    let installed = local_bin.join("mise");
    std::fs::write(&installed, "synthetic test binary").expect("write mise fixture");
    assert_eq!(default_mise_bin(Some(&home)), installed);
    assert_eq!(default_mise_bin(None), PathBuf::from("mise"));
    std::fs::remove_dir_all(&home).ok();
}

#[test]
fn failed_spawn_is_streamed_to_the_apply_log() {
    let dir = test_dir("spawn-failure");
    let log_file = std::fs::File::create(dir.join("apply.log")).expect("create apply log");
    let mut log = std::io::BufWriter::new(log_file);
    let current = Arc::new(Mutex::new(None));
    let lines = std::cell::RefCell::new(Vec::new());
    let mut seq = 0;
    let mut emit = |stream, text, _log: &mut std::io::BufWriter<std::fs::File>| {
        seq += 1;
        lines.borrow_mut().push(ApplyLine { seq, stream, text });
    };
    let outcome = run_phases(
        Path::new("/nonexistent/emc-mise"),
        &dir,
        vec![(ApplyPhase::Base, "phase 1: base", Vec::new())],
        &mut emit,
        &mut log,
        &|| false,
        &current,
    );
    assert!(matches!(outcome, ApplyOutcome::Failed { .. }));
    assert!(
        lines
            .borrow()
            .iter()
            .any(|line| line.text.contains("failed to spawn mise")),
        "spawn error must reach the apply log: {:?}",
        lines.borrow()
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn package_failure_reports_the_error_before_mise_footer_lines() {
    let dir = tempfile::tempdir().expect("test working directory");
    let mut log = std::io::BufWriter::new(tempfile::tempfile().expect("test log"));
    let current = Arc::new(Mutex::new(None));
    let mut emitted = Vec::new();
    let outcome = run_phases(
        Path::new("/bin/sh"),
        dir.path(),
        vec![
            (
                ApplyPhase::Base,
                "base",
                vec!["-c".into(), "echo 'mise ERROR earlier warning' >&2".into()],
            ),
            (
                ApplyPhase::Packages,
                "packages",
                vec![
                    "-c".into(),
                    "echo 'mise ERROR store declined the requested download' >&2
                     echo 'mise ERROR Version: mock' >&2
                     echo 'mise ERROR Run with --verbose or MISE_VERBOSE=1 for more information' >&2
                     exit 1"
                        .into(),
                ],
            ),
        ],
        &mut |_, text, _| emitted.push(text),
        &mut log,
        &|| false,
        &current,
    );
    assert_eq!(
        outcome,
        ApplyOutcome::Failed {
            code: Some(1),
            detail: "store declined the requested download".into(),
            phase: ApplyPhase::Packages,
        }
    );
    assert!(
        emitted
            .iter()
            .any(|line| line.starts_with("mise ERROR Run with --verbose"))
    );
}

#[test]
fn origin_map_reads_declaring_module() {
    let dir = test_dir("origin");
    write_env(
        &dir,
        "alpha",
        "[bootstrap.packages]\n\"brew:shared\" = \"latest\"\n\"brew:only-alpha\" = \"latest\"\n",
    );
    write_env(
        &dir,
        "beta",
        "[bootstrap.packages]\n\"brew:shared\" = \"latest\"\n\"npm:only-beta\" = \"1.2.3\"\n",
    );

    let backend = MiseBackend::new(dir.clone());
    let map = backend.origin_map(&dir).expect("origin_map parses");

    let shared = ResId {
        kind: "brew".to_string(),
        name: "shared".to_string(),
    };
    let envs = map.get(&shared).expect("shared package present");
    assert_eq!(envs.len(), 2, "multi-declare origin lists both envs");
    assert!(envs.contains(&"alpha".to_string()));
    assert!(envs.contains(&"beta".to_string()));

    let alpha_only = ResId {
        kind: "brew".to_string(),
        name: "only-alpha".to_string(),
    };
    assert_eq!(
        map.get(&alpha_only),
        Some(&vec!["alpha".to_string()]),
        "single-declare origin lists one env"
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn origin_map_does_not_depend_on_cli_env_selection() {
    let dir = test_dir("origin-without-cli-envs");
    write_env(
        &dir,
        "terminal",
        "[bootstrap.packages]\n\"brew:ripgrep\" = \"latest\"\n",
    );

    let backend = MiseBackend::new(dir.clone());
    let map = backend.origin_map(&dir).expect("origin_map parses");

    assert_eq!(
        map.get(&ResId {
            kind: "brew".to_string(),
            name: "ripgrep".to_string(),
        }),
        Some(&vec!["terminal".to_string()]),
        "interactive module selection must work without initial CLI environments"
    );

    std::fs::remove_dir_all(&dir).ok();
}

/// Read a captured fixture (`tests/fixtures/<name>`) as text.
fn fixture_text(name: &str) -> String {
    std::fs::read_to_string(format!("tests/fixtures/{name}")).expect("read fixture")
}

fn sorted_keys(value: &serde_json::Value) -> Vec<String> {
    let mut keys: Vec<String> = value.as_object().expect("object").keys().cloned().collect();
    keys.sort();
    keys
}

fn assert_no_origin(value: &serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            assert!(
                !map.contains_key("origin"),
                "plan JSON must carry no origin field"
            );
            for v in map.values() {
                assert_no_origin(v);
            }
        }
        serde_json::Value::Array(items) => {
            for v in items {
                assert_no_origin(v);
            }
        }
        _ => {}
    }
}

#[test]
fn plan_fixture_entry_keys_exact_and_no_origin() {
    let raw = fixture_text("plan.json");
    let doc: serde_json::Value = serde_json::from_str(&raw).expect("fixture parses");
    let resources = doc
        .get("resources")
        .and_then(|r| r.as_array())
        .expect("resources array present");
    assert!(!resources.is_empty(), "fixture has at least one entry");
    for entry in resources {
        assert_eq!(
            sorted_keys(entry),
            vec!["action", "current", "desired", "id"],
            "entry keys exactly {{action,current,desired,id}}"
        );
        assert_eq!(
            sorted_keys(&entry["id"]),
            vec!["kind", "name"],
            "id keys exactly {{kind,name}}"
        );
    }
    assert_no_origin(&doc);
}

#[test]
fn plan_unknown_action_and_missing_summary_parse() {
    // No `summary`, one unrecognized action: must parse without panic.
    let raw = r#"{"resources": [
        {"id": {"kind": "packages", "name": "brew:jq"}, "current": "absent", "desired": "present", "action": "create"},
        {"id": {"kind": "packages", "name": "brew:warp"}, "current": "present", "desired": "present", "action": "teleport"}
    ]}"#;
    let doc: PlanDoc = serde_json::from_str(raw).expect("tolerant parse");
    assert_eq!(doc.resources.len(), 2);
    let actions: Vec<Action> = doc
        .resources
        .iter()
        .map(|r| Action::parse(&r.action))
        .collect();
    assert_eq!(actions[0], Action::Create);
    assert_eq!(
        actions[1],
        Action::Unknown,
        "unrecognized action maps to Unknown"
    );
}

#[test]
fn status_fixtures_parse_into_typed_reports() {
    for name in [
        "status.json",
        "status-applied.json",
        "status-one-differs.json",
    ] {
        let report = StatusReport::parse(&fixture_text(name), "12:00:00")
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(!report.areas.is_empty(), "{name} lists configured areas");
    }
    assert!(
        StatusReport::parse("{}", "t")
            .expect("empty object")
            .areas
            .is_empty()
    );
}

#[test]
fn spawn_apply_dry_run_spawns_nothing() {
    let dir = test_dir("dry-run");
    let backend = MiseBackend::new(dir.clone());
    let (tx, rx) = termrock::runtime::external_channel();
    let req = ApplyRequest {
        envs: vec!["terminal".to_string()],
        keep: vec!["brew:jq".to_string()],
        manager: None,
        skip_parts: Vec::new(),
        only_parts: Vec::new(),
        run_base: true,
        update: true,
        upgrade: true,
        dry_run: true,
    };
    let handle = backend.spawn_apply(&req, tx).expect("spawn dry-run");
    let log_path = handle.log_path().to_path_buf();
    let outcome = handle.join();
    assert_eq!(
        outcome,
        ApplyOutcome::Success,
        "dry-run succeeds without spawning"
    );
    assert!(log_path.exists(), "full log is teed even for dry-run");
    let lines: Vec<ApplyLine> = std::iter::from_fn(|| rx.try_recv())
        .filter_map(|m| m.downcast_ref::<ApplyLine>().cloned())
        .collect();
    assert!(
        lines.iter().any(|l| l.text.contains("dry-run")),
        "dry-run emits preview lines, got {lines:?}"
    );
    assert!(
        lines.iter().all(|l| matches!(l.stream, Stream::System)),
        "dry-run emits system lines only"
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn apply_argv_answers_yes_because_stderr_is_piped() {
    let dir = test_dir("yes-argv");
    let backend = MiseBackend::new(dir.clone());
    let mut req = ApplyRequest {
        envs: vec!["ai-cli".to_string()],
        keep: vec!["brew:jq".to_string()],
        manager: None,
        skip_parts: Vec::new(),
        only_parts: Vec::new(),
        run_base: true,
        update: true,
        upgrade: true,
        dry_run: false,
    };
    let phase1 = backend.phase1_argv(&req);
    assert_eq!(
        phase1.last().map(String::as_str),
        Some("--yes"),
        "phase 1 must pass --yes so mise can overwrite copy-mode dotfiles: {phase1:?}"
    );
    assert!(
        phase1
            .windows(3)
            .any(|w| w == ["--skip", "packages", "--update"]),
        "package skip stays ahead of --yes: {phase1:?}"
    );
    let phase2 = backend.phase2_argv(&req).expect("keep-list runs phase 2");
    assert_eq!(
        phase2.last().map(String::as_str),
        Some("--yes"),
        "phase 2 must pass --yes too: {phase2:?}"
    );
    assert!(
        phase2
            .windows(4)
            .any(|w| w == ["packages", "apply", "brew:jq", "--yes"]),
        "kept package stays a positional before --yes: {phase2:?}"
    );

    req.only_parts = vec!["dotfiles".to_string()];
    let only = backend.phase1_argv(&req);
    assert!(
        only.windows(2).any(|w| w == ["--only", "dotfiles"]),
        "--only stays a single flag: {only:?}"
    );
    assert!(
        !only.iter().any(|arg| arg == "--skip"),
        "--only is not combined with --skip: {only:?}"
    );
    assert_eq!(only.last().map(String::as_str), Some("--yes"));

    req.only_parts.clear();
    req.manager = Some("brew".to_string());
    let managed = backend.phase2_argv(&req).expect("manager runs phase 2");
    assert!(
        managed
            .windows(5)
            .any(|w| { w == ["packages", "apply", "--manager", "brew", "brew:jq"] }),
        "manager flag stays before the keep-list: {managed:?}"
    );
    assert_eq!(managed.last().map(String::as_str), Some("--yes"));

    req.keep.clear();
    req.manager = None;
    assert!(
        backend.phase2_argv(&req).is_none(),
        "an empty keep-list still skips phase 2"
    );

    req.dry_run = true;
    req.keep = vec!["brew:jq".to_string()];
    let preview = backend.phase1_argv(&req);
    assert!(
        preview.windows(2).any(|w| w == ["--dry-run", "--yes"]),
        "preview keeps --dry-run and still shows --yes: {preview:?}"
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn mock_loads_plan_fixture() {
    let backend = super::mock::MockBackend::new();
    let entries = backend
        .plan(&["terminal".to_string()], None)
        .expect("mock plan parses");
    assert!(
        !entries.is_empty(),
        "mock loads at least one plan entry from the fixture"
    );
}

/// Config dir for tier-2 live tests: `$EMC_TEST_CONFIG_DIR`, else the public
/// starter template (exercised with `-E terminal`).
fn host_config_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("EMC_TEST_CONFIG_DIR") {
        return PathBuf::from(dir);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../templates/config-starter")
        .canonicalize()
        .expect("templates/config-starter exists")
}

#[test]
#[ignore]
fn tier2_live_plan_readonly_matches_fixture_shape() {
    let dir = host_config_dir();
    let backend = MiseBackend::new(dir.clone()).with_mise_bin("mise");
    let entries = backend
        .plan(&["terminal".to_string()], None)
        .expect("live plan --json parses");
    assert!(!entries.is_empty(), "live plan has entries");
    for e in &entries {
        assert!(!e.kind.is_empty() && !e.name.is_empty());
    }
    let _ = backend.origin_map(&dir).expect("live origin_map parses");
}

#[test]
#[ignore]
fn tier2_live_status_readonly_parses() {
    let dir = host_config_dir();
    let backend = MiseBackend::new(dir).with_mise_bin("mise");
    let report = backend
        .status(&["terminal".to_string()])
        .expect("live status --json parses");
    assert!(!report.checked_at.is_empty(), "live status is stamped");
}
