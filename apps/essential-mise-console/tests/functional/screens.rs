//! Screen baselines for the four steps and the PTY-driven journeys, including
//! exit codes. Screens render with `MockBackend` (`EMC_MOCK=1`);
//! `EMC_MOCK_FAIL=packages` scripts a failed apply, `EMC_MOCK_STATUS=differs`
//! makes the machine need one change, `EMC_MOCK_FOLLOW_UPS=1` adds the log
//! lines behind the follow-ups.

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
fn step1_choose_modules_matches_baseline() {
    let app = app("s1-modules", &[]);
    app.wait_text("terminal");
    app.visual("emc/s1-modules");
    assert_eq!(
        app.finish_with_key("escape"),
        Some(0),
        "quitting before any work is not a failure"
    );
}

#[test]
fn step2_review_matches_baseline_and_esc_goes_back() {
    let app = app("s2-review", &[]);
    app.wait_text("terminal");
    app.key("a");
    app.key("enter");
    app.wait_text("Apply now");
    app.visual("emc/s2-packages");
    app.key("escape");
    app.wait_text("Choose modules");
    assert_eq!(app.finish_with_key("q"), Some(0));
}

#[test]
fn s1_search_accepts_typed_query_and_paste() {
    let app = app("s1-filter-typing", &[]);
    app.wait_text("terminal");
    app.key("/");
    for key in ["g", "h", "o", "s", "t", "t", "y"] {
        app.key(key);
    }
    app.wait_absent("neovim");
    app.key("escape");
    app.key("/");
    app.expect_cursor_visible();
    app.paste("x");
    app.key("escape");
    assert_eq!(app.finish_with_key("escape"), Some(0));
}

#[test]
fn step2_space_skips_a_package_this_time() {
    let app = app("s2-skip", &[]);
    app.wait_text("terminal");
    app.key("enter");
    app.wait_text("starship");
    app.key("down");
    app.key("space");
    let screen = app.stable_screen();
    let point = super::support::find(&screen, "starship");
    let row = super::support::row_text(&screen, point.y.into());
    assert!(
        row.contains("skipped this time"),
        "Down then Space must skip starship, not the first row: {row}"
    );
    app.key("q");
    app.finish_wait();
}

#[test]
fn step3_success_moves_on_to_a_clean_result_and_enter_exits_zero() {
    let app = app("s3-s4", &[]);
    app.wait_text("terminal");
    app.key("enter");
    app.wait_text("starship");
    app.key("enter");
    app.wait_text("Step 4 of 4 · Done");
    app.visual("emc/s4-result");
    assert_eq!(app.finish_with_key("enter"), Some(0));
}

#[test]
fn step4_result_without_colour_still_matches_a_baseline() {
    let app = app("s4-no-color", &[("NO_COLOR", "1")]);
    app.wait_text("terminal");
    app.key("enter");
    app.wait_text("starship");
    app.key("enter");
    app.wait_text("Step 4 of 4 · Done");
    app.visual("emc/s4-result-no-color");
    assert_eq!(app.finish_with_key("enter"), Some(0));
}

#[test]
fn step4_needs_attention_names_the_change_and_quit_exits_three() {
    let app = app("s4-pending", &[("EMC_MOCK_STATUS", "differs")]);
    app.wait_text("terminal");
    app.key("enter");
    app.wait_text("starship");
    app.key("enter");
    app.wait_text("Step 4 of 4 · Needs attention");
    app.wait_text("1 of 67 values differs");
    app.visual("emc/s4-needs-attention");
    app.key("q");
    app.wait_text("Quit with 1 area");
    assert_eq!(app.finish_with_key("y"), Some(3));
    // The summary and the command survive in the normal terminal.
    let after = super::support::text(&app.screen());
    assert!(after.contains("Needs changes: macOS settings"), "{after}");
    assert!(after.contains("$ mise -C"), "{after}");
    assert!(after.contains("bootstrap --only macos-defaults"), "{after}");
}

#[test]
fn step4_details_and_help_overlay_open_and_close() {
    let app = app("s4-details", &[("EMC_MOCK_STATUS", "differs")]);
    app.wait_text("terminal");
    app.key("enter");
    app.wait_text("starship");
    app.key("enter");
    app.wait_text("Step 4 of 4 · Needs attention");
    app.key("d");
    app.wait_text("com.apple.dock tilesize");
    app.key("escape");
    app.wait_absent("Command that Enter runs");
    app.key("?");
    app.wait_text("Words used here");
    app.key("escape");
    app.wait_absent("Words used here");
    app.key("q");
    assert_eq!(app.finish_with_key("y"), Some(3));
}

#[test]
fn step3_failure_shows_the_stopped_banner_and_matches_baseline() {
    let app = app("s3-failed", &[("EMC_MOCK_FAIL", "packages")]);
    app.wait_text("terminal");
    app.key("enter");
    app.wait_text("starship");
    app.key("enter");
    app.wait_text("Stopped at: Packages");
    app.visual("emc/s3-failed");
    assert_eq!(
        app.finish_with_key("q"),
        Some(1),
        "a stopped apply is an error exit"
    );
}

#[test]
fn step3_stop_asks_first_then_offers_try_again() {
    let app = app("s3-cancel-dialog", &[("EMC_MOCK_DELAY_MS", "5000")]);
    app.wait_text("terminal");
    app.key("enter");
    app.wait_text("starship");
    app.key("enter");
    app.wait_text("Setting up your Mac");
    app.key("escape");
    app.wait_text("Stop applying?");
    app.key("escape");
    app.wait_absent("Stop applying?");
    app.key("escape");
    app.key("right");
    app.key("enter");
    app.wait_text("Stopped. Some changes may already have been made.");
    app.wait_text("Try again");
    assert_eq!(app.finish_with_key("q"), Some(1));
}

#[test]
fn step3_failure_copy_reaches_terminal_clipboard() {
    let app = app("s3-failure-copy", &[("EMC_MOCK_FAIL", "packages")]);
    app.wait_text("terminal");
    app.key("enter");
    app.wait_text("starship");
    app.key("enter");
    app.wait_text("Stopped at: Packages");
    app.key("c");
    app.wait_clipboard("mise");
    assert_eq!(app.finish_with_key("q"), Some(1));
}

#[test]
fn step4_follow_up_command_reaches_terminal_clipboard() {
    let app = app("s4-copy", &[("EMC_MOCK_FOLLOW_UPS", "1")]);
    app.wait_text("terminal");
    app.key("enter");
    app.wait_text("starship");
    app.key("enter");
    app.wait_text("Step 4 of 4 · Done");
    app.wait_text("Plugin hook skipped");
    app.key("c");
    app.wait_clipboard("killall Dock Finder SystemUIServer");
    assert_eq!(app.finish_with_key("enter"), Some(0));
}

#[test]
fn step4_reapply_fixes_the_pending_area_and_finishes_clean() {
    let app = app("s4-reapply", &[("EMC_MOCK_STATUS", "differs-then-clean")]);
    app.wait_text("terminal");
    app.key("enter");
    app.wait_text("starship");
    app.key("enter");
    app.wait_text("Apply the 1 pending area again");
    app.key("enter");
    app.wait_text("Step 4 of 4 · Done");
    assert_eq!(app.finish_with_key("enter"), Some(0));
}

#[test]
fn static_screen_resize_redraws_and_keeps_the_footer_in_view() {
    let app = app("s1-resize", &[]);
    app.wait_text("terminal");
    app.resize(80, 24);
    let screen = app.wait_text("q quit");
    assert_eq!(screen.rows(), 24);
    assert!(super::support::row_text(&screen, 23).contains("q quit"));
    assert!(super::support::row_text(&screen, 22).contains("Enter"));
    app.key("/");
    app.paste("ghostty");
    app.wait_absent("neovim");
    app.key("escape");
    assert_eq!(app.finish_with_key("escape"), Some(0));
}

#[test]
fn canned_mock_origins_cover_fixture_envs() {
    use essential_mise_console::cli::mock_backend;
    use essential_mise_console::metadata::Manifest;
    let envs: Vec<String> = Manifest::load(&fixture_config())
        .expect("fixture headers")
        .modules
        .into_iter()
        .map(|m| m.name)
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
