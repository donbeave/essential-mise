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

    let backend = MiseBackend::new(dir.clone(), vec!["alpha".to_string(), "beta".to_string()]);
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
fn status_fixture_parses_without_panic() {
    let raw = fixture_text("status.json");
    let snapshot = snapshot_from_raw(&raw);
    assert_eq!(
        snapshot.raw_note, raw,
        "raw status text preserved for follow-up display"
    );
    // `{}` (missing everything) must also parse without panic.
    let empty = snapshot_from_raw("{}");
    assert!(empty.parts.is_empty());
    assert_eq!(empty.raw_note, "{}");
}

#[test]
fn spawn_apply_dry_run_spawns_nothing() {
    let dir = test_dir("dry-run");
    let backend = MiseBackend::new(dir.clone(), vec!["terminal".to_string()]);
    let (tx, rx) = junie_tui::runtime::external_channel();
    let req = ApplyRequest {
        envs: vec!["terminal".to_string()],
        keep: vec!["brew:jq".to_string()],
        manager: None,
        skip_parts: Vec::new(),
        only_parts: Vec::new(),
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
    let lines: Vec<ApplyLine> = rx
        .drain()
        .iter()
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

#[test]
fn status_parts_derive_ok_and_drift() {
    // Fixture host state is fully clean: every part ok.
    let fixture = super::snapshot_from_raw(&fixture_text("status.json"));
    assert!(!fixture.parts.is_empty(), "real schema yields parts");
    assert!(
        fixture.parts.iter().all(|p| p.state == "ok"),
        "clean host has no drift: {:?}",
        fixture
            .parts
            .iter()
            .filter(|p| p.state != "ok")
            .collect::<Vec<_>>()
    );
    // Drifted packages inventory.
    let drifted = super::snapshot_from_raw(
        r#"{"packages": {"brew": {"available": true, "packages": [
            {"package": "jq", "requested_version": "latest", "desired_state": "present", "state": "absent", "managed_by": "mise"}
        ]}}}"#,
    );
    let packages = drifted
        .parts
        .iter()
        .find(|p| p.part == "packages")
        .expect("packages part");
    assert_eq!(packages.state, "drift");
    assert!(packages.drift.iter().any(|d| d.contains("brew:jq")));
    // Pending-work list with entries.
    let pending = super::snapshot_from_raw(r#"{"files": [{"x": 1}]}"#);
    assert_eq!(pending.parts[0].state, "drift");
    // Uninstalled tool.
    let tools = super::snapshot_from_raw(
        r#"{"tools": [{"tool": "rust", "requested_version": "latest", "resolved_version": "1.99.1", "state": "missing", "installed": false}]}"#,
    );
    assert_eq!(tools.parts[0].state, "drift");
    // Legacy parts array still parses.
    let legacy =
        super::snapshot_from_raw(r#"{"parts": [{"part": "x", "state": "ok", "drift": []}]}"#);
    assert_eq!(legacy.parts.len(), 1);
}

/// Host config dir for tier-2 live tests: `$EMC_TEST_CONFIG_DIR`, else the
/// repo root.
fn host_config_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("EMC_TEST_CONFIG_DIR") {
        return PathBuf::from(dir);
    }
    let out = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .expect("git rev-parse for host config dir");
    assert!(out.status.success(), "git rev-parse must succeed");
    PathBuf::from(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

#[test]
#[ignore]
fn tier2_live_plan_readonly_matches_fixture_shape() {
    let dir = host_config_dir();
    let backend = MiseBackend::new(dir.clone(), vec!["terminal".to_string()]).with_mise_bin("mise");
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
    let backend = MiseBackend::new(dir, vec!["terminal".to_string()]).with_mise_bin("mise");
    let snapshot = backend
        .status(&["terminal".to_string()])
        .expect("live status --json parses");
    assert!(!snapshot.raw_note.is_empty(), "live status keeps raw text");
}
