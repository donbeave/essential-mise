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
fn s1_filter_accepts_typed_query() {
    let app = app("s1-filter-typing", &[]);
    app.wait_text("terminal");
    app.key("/");
    for key in ["g", "h", "o", "s", "t", "t", "y"] {
        app.key(key);
    }
    app.wait_text("showing 1");
    app.wait_absent("neovim");
    app.key("escape");
    app.finish_with_key("escape");
}

#[test]
fn s2_arrow_navigation_toggles_the_cursor_row() {
    let app = app("s2-arrow-navigation", &[]);
    app.wait_text("terminal");
    app.key("enter");
    app.wait_text("starship");
    app.key("down");
    app.key("space");
    let screen = app.stable_screen();
    let point = super::support::find(&screen, "starship");
    let row = super::support::row_text(&screen, point.y.into());
    assert!(
        row.contains("omitted"),
        "Down then Space must omit starship, not the first row: {row}"
    );
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
    app.key("enter");
    app.wait_text("Apply complete");
    app.visual("emc/s3-apply");
    app.key("enter");
    app.wait_text("4/4 Result");
    app.visual("emc/s4-result");
    app.key("enter");
    app.finish_wait();
}

#[test]
fn s3_failure_modal_matches_baseline() {
    let app = app("s3-failed", &[("EMC_MOCK_FAIL", "packages")]);
    app.wait_text("terminal");
    app.key("enter");
    app.wait_text("starship");
    app.key("enter");
    app.wait_text("Apply failed");
    app.visual("emc/s3-failed");
    app.key("enter");
    app.wait_text("4/4 Result");
    app.key("enter");
    app.finish_wait();
}

#[test]
fn s1_filter_paste_focus_and_continue_click() {
    let app = app("s1-filter-paste-focus", &[]);
    app.wait_text("terminal");
    app.key("/");
    app.expect_cursor_visible();
    app.paste("ghostty");
    app.wait_text("showing 1");
    app.wait_absent("neovim");
    app.key("escape");
    app.click_text("Continue");
    app.wait_text("starship");
    app.key("backspace");
    app.wait_text("terminal");
    app.finish_with_key("escape");
}

#[test]
fn s2_marker_click_toggles_the_clicked_row() {
    let app = app("s2-marker-click", &[]);
    app.wait_text("terminal");
    app.key("enter");
    let point = app.point("starship");
    app.click(super::support::Point {
        x: point.x.saturating_sub(3),
        y: point.y,
    });
    let screen = app.stable_screen();
    let point = super::support::find(&screen, "starship");
    let row = super::support::row_text(&screen, point.y.into());
    assert!(row.contains("omitted"), "clicked row must toggle: {row}");
    app.key("backspace");
    app.wait_text("terminal");
    app.finish_with_key("escape");
}

#[test]
fn s3_cancel_dialog_dismisses_and_confirms() {
    let app = app("s3-cancel-dialog", &[("EMC_MOCK_DELAY_MS", "5000")]);
    app.wait_text("terminal");
    app.key("enter");
    app.key("enter");
    app.wait_text("Applying base configuration");
    app.key("escape");
    app.wait_text("Cancel apply?");
    app.key("enter");
    app.wait_absent("Cancel apply?");
    app.key("escape");
    app.key("right");
    app.key("enter");
    app.wait_text("Apply cancelled");
    app.key("enter");
    app.wait_text("4/4 Result");
    app.key("enter");
    app.finish_wait();
}

#[test]
fn s3_running_shows_activity_instead_of_empty_zero_percent() {
    let app = app("s3-running", &[("EMC_MOCK_DELAY_MS", "5000")]);
    app.wait_text("terminal");
    app.key("enter");
    app.key("enter");
    app.wait_text("Applying base configuration");
    app.wait_text("phase 1: base");
    app.wait_absent("0%");
    app.wait_absent("Enter done");
    app.key("escape");
    app.key("right");
    app.key("enter");
    app.wait_text("Apply cancelled");
    app.key("enter");
    app.wait_text("4/4 Result");
    app.key("enter");
    app.finish_wait();
}

#[test]
fn s3_failure_copy_reaches_terminal_clipboard() {
    let app = app("s3-failure-copy", &[("EMC_MOCK_FAIL", "packages")]);
    app.wait_text("terminal");
    app.key("enter");
    app.key("enter");
    app.wait_text("Apply failed");
    app.key("y");
    app.wait_clipboard("mise");
    app.key("enter");
    app.wait_text("4/4 Result");
    app.key("enter");
    app.finish_wait();
}

#[test]
fn s4_copy_reaches_terminal_clipboard() {
    let app = app("s4-copy", &[]);
    app.wait_text("terminal");
    app.key("enter");
    app.key("enter");
    app.wait_text("Apply complete");
    app.key("enter");
    app.wait_text("4/4 Result");
    app.key("c");
    app.wait_clipboard("parts ok");
    app.key("enter");
    app.finish_wait();
}

#[test]
fn static_screen_resize_redraws_and_preserves_filtering() {
    let app = app("s1-resize", &[]);
    app.wait_text("terminal");
    app.resize(80, 24);
    let screen = app.wait_text("S1 · search keeps selection");
    assert_eq!(screen.rows(), 24);
    assert!(super::support::row_text(&screen, 23).contains("search keeps selection"));
    app.key("/");
    app.paste("ghostty");
    app.wait_text("showing 1");
    app.wait_absent("neovim");
    app.key("escape");
    app.finish_with_key("escape");
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
