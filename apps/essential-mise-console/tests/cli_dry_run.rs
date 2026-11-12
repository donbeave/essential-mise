//! T3.2 dry-run golden tests: `apply --dry-run` and `plan --json` against
//! fixtures via [`MockBackend`], asserting exit codes and that `--dry-run`
//! never spawns a child (asserted on the mock call log, plus a live-backend
//! dry run against a nonexistent `mise` binary).
//!
//! Tier 1 (always run): mock + fixture + hermetic live-backend tests — no
//! host config is touched. Tier 2 (ignored; operators run with
//! `cargo nextest run --run-ignored all -- tier2`): our `--help` flag names
//! diffed against `mise help`, catching upstream flag drift.

use clap::Parser as _;
use essential_mise_console::backend::mock::{MockBackend, MockCall};
use essential_mise_console::backend::{ApplyPhase, ApplyRequest, Backend, MiseBackend};
use essential_mise_console::cli::{self, Cli, EXIT_ERROR, EXIT_OK, EXIT_PARTIAL};

fn parse(args: &[&str]) -> Cli {
    Cli::try_parse_from(args).expect("args parse")
}

fn run_with(backend: std::sync::Arc<dyn Backend>, args: &[&str]) -> (u8, String) {
    let cli = parse(args);
    let mut out = Vec::new();
    let code = cli::run(backend, &cli, &mut out);
    (code, String::from_utf8(out).expect("utf8 output"))
}

#[test]
fn apply_dry_run_exits_zero_and_spawns_nothing() {
    let backend = std::sync::Arc::new(MockBackend::new());
    let (code, out) = run_with(
        backend.clone(),
        &["emc", "-E", "terminal", "apply", "--dry-run", "brew:jq"],
    );
    assert_eq!(code, EXIT_OK, "dry-run apply exits 0, got output:\n{out}");
    assert!(
        out.contains("mock: apply preview"),
        "dry-run prints the preview, got:\n{out}"
    );
    let spawns: Vec<_> = backend
        .calls()
        .into_iter()
        .filter(|c| matches!(c, MockCall::SpawnApply { .. }))
        .collect();
    assert_eq!(spawns.len(), 1, "exactly one spawn_apply call");
    assert!(
        spawns
            .iter()
            .all(|c| matches!(c, MockCall::SpawnApply { dry_run: true })),
        "every spawn_apply call is a dry run: {spawns:?}"
    );
}

#[test]
fn apply_dry_run_empty_keep_skips_phase_two() {
    // Live backend against a nonexistent binary: any spawn attempt would
    // fail, so success proves the dry-run path spawns nothing while emitting
    // the real narration.
    let dir = std::env::temp_dir().join(format!("emc-test-cli-empty-keep-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let backend = std::sync::Arc::new(
        MiseBackend::new(dir.clone()).with_mise_bin("/nonexistent/emc-test-mise"),
    );
    let (code, out) = run_with(
        backend.clone(),
        &["emc", "-E", "terminal", "apply", "--dry-run"],
    );
    assert_eq!(code, EXIT_OK, "empty keep-list dry-run exits 0");
    assert!(
        out.contains("keep-list empty"),
        "empty keep-list skips phase 2, got:\n{out}"
    );
    assert!(
        out.contains("dry-run:"),
        "dry-run narrates the preview, got:\n{out}"
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn plan_json_envelope_matches_fixture_shape() {
    let backend = std::sync::Arc::new(MockBackend::new());
    let (code, out) = run_with(
        backend.clone(),
        &["emc", "-E", "terminal", "plan", "--json"],
    );
    assert_eq!(code, EXIT_OK, "plan --json exits 0, got:\n{out}");
    let doc: serde_json::Value =
        serde_json::from_str(&out).expect("stdout is the console envelope");
    assert_eq!(
        doc.get("envs").and_then(|e| e.as_array()).map(|e| e.len()),
        Some(1),
        "envelope carries the requested envs"
    );
    let resources = doc
        .get("resources")
        .and_then(|r| r.as_array())
        .expect("envelope has resources");
    assert!(!resources.is_empty(), "fixture has entries");
    for entry in resources {
        let mut keys: Vec<String> = entry.as_object().expect("object").keys().cloned().collect();
        keys.sort();
        assert_eq!(keys, vec!["action", "current", "desired", "kind", "name"]);
    }
    let summary = doc.get("summary").expect("envelope has summary");
    assert_eq!(
        summary.get("unknown").and_then(|u| u.as_u64()),
        Some(0),
        "fixture has no unknown actions"
    );
}

#[test]
fn plan_detailed_exitcode_clean_fixture_is_zero() {
    let backend = std::sync::Arc::new(MockBackend::new());
    let (code, _) = run_with(
        backend.clone(),
        &[
            "emc",
            "-E",
            "terminal",
            "plan",
            "--json",
            "--detailed-exitcode",
        ],
    );
    assert_eq!(code, EXIT_OK, "all-noop fixture is clean");
}

#[test]
fn apply_failure_exit_codes_zero_one_three() {
    for (phase, expected) in [
        (ApplyPhase::Base, EXIT_ERROR),
        (ApplyPhase::Packages, EXIT_PARTIAL),
    ] {
        let backend = std::sync::Arc::new(MockBackend::new().with_failure(phase));
        let (code, _) = run_with(
            backend.clone(),
            &["emc", "-E", "terminal", "-y", "apply", "brew:jq"],
        );
        assert_eq!(
            code, expected,
            "failure in {phase:?} exits {expected}, dry-run flag off"
        );
    }
}

#[test]
fn live_backend_dry_run_spawns_nothing() {
    // A nonexistent mise binary proves no spawn is attempted: any spawn
    // (or sudo preflight) would fail, but dry-run succeeds.
    let dir = std::env::temp_dir().join(format!("emc-test-cli-dry-run-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let backend = MiseBackend::new(dir.clone()).with_mise_bin("/nonexistent/emc-test-mise");
    let (tx, _rx) = junie_tui::runtime::external_channel();
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
    let outcome = backend.spawn_apply(&req, tx).expect("spawn dry-run").join();
    assert_eq!(
        outcome,
        essential_mise_console::backend::ApplyOutcome::Success
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn setup_dry_run_prints_staged_plan_with_zero_subprocesses() {
    let backend = std::sync::Arc::new(MockBackend::new());
    let (code, out) = run_with(
        backend.clone(),
        &["emc", "setup", "--dry-run", "--config", "OWNER/REPO"],
    );
    assert_eq!(code, EXIT_OK, "setup --dry-run exits 0, got:\n{out}");
    for stage in 1..=6u8 {
        assert!(
            out.contains(&format!("stage {stage}/6")),
            "staged plan lists stage {stage}/6, got:\n{out}"
        );
    }
    for url in [
        "https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh",
        "https://mise.run",
        "GIT_TERMINAL_PROMPT=0 /usr/bin/git ls-remote https://github.com/OWNER/REPO.git",
        "bootstrap --adopt https://github.com/OWNER/REPO.git",
    ] {
        assert!(out.contains(url), "staged plan shows {url}, got:\n{out}");
    }
    assert!(
        backend.calls().is_empty(),
        "dry-run spawns nothing and touches no backend: {:?}",
        backend.calls()
    );
}

/// Passthrough flags and the mise subcommand whose `--help` must list them.
/// Console-side flags (`--manager` on `packages`, `--json` envelopes) are
/// excluded: they are filtered or added by the console, never passed through.
const PASSTHROUGH_FLAGS: &[(&str, &[&str])] = &[
    ("--json", &["plan", "status"]),
    ("--detailed-exitcode", &["plan"]),
    ("--jobs", &["plan"]),
    ("--skip", &["bootstrap"]),
    ("--only", &["bootstrap"]),
    ("--update", &["bootstrap", "apply"]),
    ("--upgrade", &["bootstrap"]),
    ("--manager", &["apply"]),
    ("--dry-run", &["bootstrap", "apply"]),
    ("--missing", &["status"]),
];

fn mise_help(sub: &str) -> String {
    let out = std::process::Command::new("mise")
        .args(["bootstrap"])
        .args(sub.split_whitespace())
        .arg("--help")
        .output()
        .expect("mise --help runs");
    assert!(out.status.success(), "mise bootstrap {sub} --help succeeds");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
#[ignore]
fn tier2_help_matches_mise_flags() {
    let bootstrap = mise_help("");
    let plan = mise_help("plan");
    let apply = mise_help("packages apply");
    let status = mise_help("packages status");
    let help_of = |sub: &str| match sub {
        "bootstrap" => &bootstrap,
        "plan" => &plan,
        "apply" => &apply,
        "status" => &status,
        _ => unreachable!(),
    };
    for (flag, subs) in PASSTHROUGH_FLAGS {
        for sub in *subs {
            assert!(
                help_of(sub).contains(flag),
                "mise bootstrap {sub} --help still lists {flag}"
            );
        }
    }
    // Console-side fact that must keep holding: `packages status` has no
    // `-m`, so the console keeps filtering via `origin_map`.
    assert!(
        !status.contains("--manager"),
        "packages status keeps having no --manager (console filters)"
    );
}

#[test]
fn lint_passes_on_the_starter_and_fails_on_an_empty_dir() {
    let starter = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../templates/config-starter"
    );
    let backend = std::sync::Arc::new(MockBackend::new());
    let (code, out) = run_with(backend.clone(), &["emc", "lint", "--config-dir", starter]);
    assert_eq!(code, EXIT_OK, "starter passes lint, got:\n{out}");

    let empty = std::env::temp_dir().join(format!("emc-test-lint-empty-{}", std::process::id()));
    std::fs::create_dir_all(&empty).expect("create temp dir");
    let (code, out) = run_with(
        backend,
        &["emc", "lint", "--config-dir", &empty.display().to_string()],
    );
    assert_eq!(code, EXIT_ERROR, "empty dir fails lint, got:\n{out}");
    assert!(out.contains("config.toml: not found"), "got:\n{out}");
    std::fs::remove_dir_all(&empty).ok();
}
