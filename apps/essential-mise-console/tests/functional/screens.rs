//! Screen baselines (one per screen, S1–S4, plus the S3 failure modal)
//! and the PTY-driven journey. Screens render with `MockBackend`
//! (`EMC_MOCK=1`); the failure modal adds `EMC_MOCK_FAIL=packages`.

use std::path::PathBuf;

use essential_mise_console::backend::Backend;

use super::support::App;

const BIN: &str = env!("CARGO_BIN_EXE_essential-mise-console");

fn fixture_config() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/functional/fixtures/config")
}

fn tui_args() -> Vec<String> {
    vec![
        "--config-dir".to_string(),
        fixture_config().display().to_string(),
        "tui".to_string(),
    ]
}

fn app(name: &str, extra_env: &[(&str, &str)]) -> App {
    let args = tui_args();
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    App::new(name, BIN, &arg_refs, extra_env, 100, 30)
}

#[test]
fn s1_modules_matches_baseline() {
    let app = app("s1-modules", &[]);
    app.wait_text("terminal");
    app.visual("emc/s1-modules");
    app.finish_with_key("escape");
}

#[test]
fn s2_packages_matches_baseline() {
    let app = app("s2-packages", &[]);
    app.wait_text("terminal");
    // Select everything, then continue to the checklist.
    app.key("a");
    app.key("enter");
    app.wait_text("ripgrep");
    app.visual("emc/s2-packages");
    // Back out and quit: S2 → S1 → Esc.
    app.key("backspace");
    app.wait_text("terminal");
    app.finish_with_key("escape");
}

#[test]
fn s3_apply_and_s4_result_match_baselines() {
    let app = app("s3-s4", &[]);
    app.wait_text("terminal");
    app.key("enter");
    app.wait_text("starship");
    // S3 idle: the worker starts on Enter.
    app.key("enter");
    app.wait_text("3/4 Apply");
    app.visual("emc/s3-apply");
    app.key("enter");
    app.wait_text("4/4 Result");
    app.visual("emc/s4-result");
    app.key("enter");
    app.finish_with_key("enter");
}

#[test]
fn s3_failure_modal_matches_baseline() {
    let app = app("s3-failed", &[("EMC_MOCK_FAIL", "packages")]);
    app.wait_text("terminal");
    app.key("enter");
    app.wait_text("starship");
    app.key("enter");
    app.wait_text("3/4 Apply");
    // Starting fails instantly; the modal carries the rerun command.
    app.key("enter");
    app.wait_text("Apply failed");
    app.visual("emc/s3-failed");
    app.key("enter");
    app.wait_text("4/4 Result");
    app.key("enter");
    app.finish_with_key("enter");
}

#[test]
fn canned_mock_origins_cover_fixture_envs() {
    use essential_mise_console::cli::mock_backend;
    use essential_mise_console::screens::modules::parse_environments;
    let text = std::fs::read_to_string(fixture_config().join("bootstrap.sh"))
        .expect("fixture bootstrap.sh");
    let envs: Vec<String> = parse_environments(&text)
        .into_iter()
        .map(|e| e.name)
        .collect();
    assert!(!envs.is_empty(), "fixture declares envs");
    let origins = mock_backend()
        .origin_map(&fixture_config())
        .expect("mock origins");
    for decl in origins.values().flatten() {
        assert!(
            envs.contains(decl),
            "canned origin env {decl} must exist in the fixture"
        );
    }
}
