//! App-level tests: flows, keys, exit codes and what each screen renders.

use super::*;
use crate::backend::mock::MockBackend;
use junie_tui::core::event::Key;
use junie_tui::runtime::Application;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};

fn key(code: KeyCode) -> Key {
    Key {
        code,
        mods: KeyModifiers::empty(),
    }
}

fn char_key(c: char) -> Key {
    Key {
        code: KeyCode::Char(c),
        mods: KeyModifiers::empty(),
    }
}

fn mock_app() -> App {
    mock_app_with(
        MockBackend::new()
            .with_origin("brew", "jq", &["alpha"])
            .with_origin("brew", "rg", &["beta"]),
    )
}

fn mock_app_with(backend: MockBackend) -> App {
    let dir = std::env::temp_dir().join(format!("emc-test-app-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    write_manifest(
        &dir,
        &[("alpha", "First module"), ("beta", "Second module")],
    );
    std::fs::write(dir.join("miserc.local.toml"), "env = [\"alpha\"]\n").expect("write local");
    let backend: Arc<dyn Backend> = Arc::new(backend);
    let mut app = App::new(backend, &dir).expect("app builds");
    app.arm_delay = Duration::ZERO;
    app
}

fn write_manifest(dir: &Path, modules: &[(&str, &str)]) {
    std::fs::write(
        dir.join("config.toml"),
        "#: schema = 1\n#: name = \"test\"\n",
    )
    .expect("write config.toml");
    for (index, (name, description)) in modules.iter().enumerate() {
        let text = format!(
            "#: description = \"{description}\"\n#: order = {}\n",
            index + 1
        );
        std::fs::write(dir.join(format!("config.{name}.toml")), text).expect("write module");
    }
}

fn mock_app_many_groups() -> App {
    let dir = std::env::temp_dir().join(format!("emc-test-groups-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    write_manifest(
        &dir,
        &[
            ("e0", "Zero"),
            ("e1", "One"),
            ("e2", "Two"),
            ("e3", "Three"),
            ("e4", "Four"),
            ("e5", "Five"),
        ],
    );
    std::fs::write(dir.join("miserc.local.toml"), "").expect("write local");
    let mut backend = MockBackend::new();
    for i in 0..6 {
        backend = backend.with_origin("brew", &format!("pkg{i}"), &[&format!("e{i}")]);
    }
    App::new(Arc::new(backend), &dir).expect("app builds")
}

fn draw(app: &mut App, terminal: &mut ratatui::Terminal<ratatui::backend::TestBackend>) {
    terminal.draw(|frame| app.render(frame)).expect("draw app");
}

fn buffer_text(terminal: &ratatui::Terminal<ratatui::backend::TestBackend>) -> String {
    let buffer = terminal.backend().buffer();
    (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn click_id(app: &mut App, id: WidgetId) {
    let area = app
        .shell
        .hits
        .regions()
        .iter()
        .find(|region| region.id == id)
        .map(|region| region.area)
        .unwrap_or_else(|| panic!("missing hit region for {id:?}"));
    let pos = ratatui::layout::Position::new(area.x + area.width / 2, area.y + area.height / 2);
    app.handle(Input::Mouse(junie_tui::core::event::Mouse::new(
        junie_tui::core::event::MouseKind::Down,
        pos,
    )));
    app.handle(Input::Mouse(junie_tui::core::event::Mouse::new(
        junie_tui::core::event::MouseKind::Up,
        pos,
    )));
}

fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, b)| n | u32::from(*b) << (16 - 8 * i));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

// ---- drivers -------------------------------------------------------------

fn wait_until(app: &mut App, what: &str, done: impl Fn(&App) -> bool) {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !done(app) {
        app.pump();
        assert!(std::time::Instant::now() < deadline, "timed out: {what}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Pump until the Review screen has finished its check.
fn wait_review(app: &mut App) {
    wait_until(
        app,
        "review check",
        |a| !matches!(&a.screen, Screen::Packages(o) if o.state.is_checking()),
    );
}

/// Pump until the Result screen has finished its check.
fn wait_check(app: &mut App) {
    wait_until(
        app,
        "result check",
        |a| !matches!(&a.screen, Screen::Result(o) if o.state.is_checking()),
    );
}

/// Pump until the apply stopped, or moved on to the Result screen.
fn finish_apply(app: &mut App) {
    wait_until(
        app,
        "apply outcome",
        |a| !matches!(&a.screen, Screen::Apply(o) if o.state.outcome().is_none()),
    );
}

fn to_review(app: &mut App) {
    app.handle_key(&key(KeyCode::Enter));
    assert_eq!(app.screen_name(), "packages");
    wait_review(app);
}

fn start_apply(app: &mut App) {
    to_review(app);
    app.handle_key(&key(KeyCode::Enter));
    assert_eq!(app.screen_name(), "apply");
}

fn to_result(app: &mut App) {
    start_apply(app);
    finish_apply(app);
    assert_eq!(app.screen_name(), "result", "success moves on by itself");
    wait_check(app);
}

fn fixture_report(name: &str) -> crate::status::StatusReport {
    let text = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name),
    )
    .expect("fixture readable");
    crate::status::StatusReport::parse(&text, crate::backend::mock::MOCK_CHECKED_AT)
        .expect("fixture parses")
}

fn app_with_status(name: &str) -> App {
    mock_app_with(
        MockBackend::new()
            .with_origin("brew", "jq", &["alpha"])
            .with_status_sequence(vec![fixture_report(name)]),
    )
}

/// An app whose `status` calls return these reports in order. The Review
/// screen's check consumes the first, so it is listed twice: the Result
/// screen's first check sees it too.
fn app_with_sequence(reports: &[&str]) -> (App, MockBackend) {
    let mut sequence = vec![fixture_report(reports[0])];
    sequence.extend(reports.iter().map(|n| fixture_report(n)));
    let mock = MockBackend::new()
        .with_origin("brew", "jq", &["alpha"])
        .with_status_sequence(sequence);
    let probe = mock.clone();
    (mock_app_with(mock), probe)
}

fn render(app: &mut App, w: u16, h: u16) -> String {
    let mut terminal =
        ratatui::Terminal::new(ratatui::backend::TestBackend::new(w, h)).expect("terminal");
    draw(app, &mut terminal);
    buffer_text(&terminal)
}

// ---- step 1 --------------------------------------------------------------

#[test]
fn step1_reads_as_choose_modules_with_one_primary_line() {
    let mut app = mock_app();
    let screen = render(&mut app, 100, 30);
    assert!(screen.contains("Step 1 of 4 · Choose modules"), "{screen}");
    assert!(
        screen.contains("Nothing changes until you confirm"),
        "{screen}"
    );
    assert!(screen.contains("Enter  Review 1 module"), "{screen}");
    assert!(screen.contains("1 of 2 selected"), "{screen}");
    assert!(screen.contains("Search"), "{screen}");
}

#[test]
fn step1_empty_selection_says_what_to_do_and_enter_waits() {
    let mut app = mock_app();
    app.handle_key(&char_key('n'));
    let screen = render(&mut app, 100, 30);
    assert!(
        screen.contains("Select at least one module (Space)"),
        "{screen}"
    );
    app.handle_key(&key(KeyCode::Enter));
    assert_eq!(app.screen_name(), "modules");
}

#[test]
fn step1_counts_selected_modules_hidden_by_search_and_explains_no_match() {
    let mut app = mock_app();
    app.handle_key(&char_key('/'));
    for c in "beta".chars() {
        app.handle_key(&char_key(c));
    }
    let screen = render(&mut app, 100, 30);
    assert!(screen.contains("1 selected hidden by search"), "{screen}");
    for _ in 0..4 {
        app.handle_key(&key(KeyCode::Backspace));
    }
    for c in "zzz".chars() {
        app.handle_key(&char_key(c));
    }
    let screen = render(&mut app, 100, 30);
    assert!(
        screen.contains("No modules match 'zzz'. Esc clears the search."),
        "{screen}"
    );
}

#[test]
fn step1_narrow_terminal_moves_the_focused_module_details_under_the_list() {
    let mut app = mock_app();
    let screen = render(&mut app, 80, 24);
    assert!(screen.contains("about"), "{screen}");
    assert!(screen.contains("First module"), "{screen}");
    assert!(screen.contains("1 package"), "{screen}");
    assert!(!screen.contains('…'), "nothing is cut: {screen}");
}

#[test]
fn step1_quit_before_any_work_exits_zero() {
    for quit in [key(KeyCode::Esc), char_key('q')] {
        let mut app = mock_app();
        app.handle_key(&quit);
        assert!(app.quit());
        assert_eq!(app.exit_code(), Some(0));
    }
}

#[test]
fn step1_enter_opens_review_with_the_preticked_module() {
    let mut app = mock_app();
    app.handle_key(&char_key(' '));
    app.handle_key(&key(KeyCode::Down));
    app.handle_key(&char_key(' '));
    to_review(&mut app);
    assert_eq!(app.selected, vec!["beta".to_string()]);
}

#[test]
fn step1_the_review_button_in_the_detail_pane_still_works_with_the_pointer() {
    let mut app = mock_app();
    let mut terminal =
        ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 30)).expect("terminal");
    draw(&mut app, &mut terminal);
    click_id(&mut app, WidgetId::of("emc.s1.detail").sub("continue"));
    assert_eq!(app.screen_name(), "packages");
}

#[test]
fn step1_wheel_reveals_later_modules() {
    let mut app = mock_app_many_groups();
    let mut terminal =
        ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 22)).expect("terminal");
    draw(&mut app, &mut terminal);
    let master = WidgetId::of("emc.s1.master");
    let area = app
        .shell
        .hits
        .regions()
        .iter()
        .find(|region| region.id == master)
        .map(|region| region.area)
        .expect("module master hit region");
    let pos = ratatui::layout::Position::new(area.x + 2, area.y + 1);
    for _ in 0..5 {
        app.handle(Input::Mouse(junie_tui::core::event::Mouse::new(
            junie_tui::core::event::MouseKind::WheelDown,
            pos,
        )));
        draw(&mut app, &mut terminal);
    }
    assert!(
        buffer_text(&terminal).contains("e5"),
        "the final module is reachable"
    );
}

// ---- step 2 --------------------------------------------------------------

#[test]
fn step2_summarises_the_run_and_lists_only_what_changes() {
    let mut app = mock_app();
    app.handle_key(&char_key('a'));
    to_review(&mut app);
    let screen = render(&mut app, 100, 30);
    assert!(
        screen.contains("Step 2 of 4 · Review and confirm"),
        "{screen}"
    );
    assert!(
        screen.contains("2 modules · 2 packages to install"),
        "{screen}"
    );
    assert!(screen.contains("administrator password once"), "{screen}");
    assert!(screen.contains("Enter  Apply now"), "{screen}");
    assert!(screen.contains("jq"), "{screen}");
    assert!(
        !screen.contains("packages apply"),
        "no CLI syntax leaks: {screen}"
    );
}

#[test]
fn step2_enter_applies_only_after_the_check_and_never_on_a_held_key() {
    let mut app = mock_app_with(
        MockBackend::new()
            .with_origin("brew", "jq", &["alpha"])
            .with_status_delay(Duration::from_millis(200)),
    );
    app.arm_delay = DEFAULT_ARM_DELAY;
    app.handle_key(&key(KeyCode::Enter));
    assert_eq!(app.screen_name(), "packages");
    assert!(render(&mut app, 100, 30).contains("Checking"));
    app.handle_key(&key(KeyCode::Enter));
    assert_eq!(
        app.screen_name(),
        "packages",
        "Enter while checking does nothing"
    );
    wait_review(&mut app);
    app.handle_key(&key(KeyCode::Enter));
    assert_eq!(
        app.screen_name(),
        "packages",
        "a key held down from the previous screen must not start a real apply"
    );
    std::thread::sleep(DEFAULT_ARM_DELAY + Duration::from_millis(50));
    app.handle_key(&key(KeyCode::Enter));
    assert_eq!(app.screen_name(), "apply");
}

#[test]
fn step2_space_skips_a_package_this_time_and_the_apply_leaves_it_out() {
    let probe_backend = MockBackend::new()
        .with_origin("brew", "jq", &["alpha"])
        .with_origin("brew", "rg", &["alpha"]);
    let probe = probe_backend.clone();
    let mut app = mock_app_with(probe_backend);
    to_review(&mut app);
    app.handle_key(&char_key(' '));
    let screen = render(&mut app, 100, 30);
    assert!(screen.contains("skipped this time (not saved)"), "{screen}");
    app.handle_key(&key(KeyCode::Enter));
    assert_eq!(probe.requests()[0].keep, vec!["brew:rg"]);
    assert_eq!(app.skipped, vec!["brew:jq"]);
}

#[test]
fn step2_esc_goes_back_keeping_the_selection_and_q_exits_zero() {
    let mut app = mock_app();
    app.handle_key(&char_key('a'));
    to_review(&mut app);
    app.handle_key(&key(KeyCode::Esc));
    assert_eq!(app.screen_name(), "modules");
    assert!(render(&mut app, 100, 30).contains("2 of 2 selected"));
    to_review(&mut app);
    app.handle_key(&char_key('q'));
    assert!(app.quit());
    assert_eq!(app.exit_code(), Some(0));
}

// ---- step 3 --------------------------------------------------------------

#[test]
fn step3_success_moves_on_to_the_result_without_another_enter() {
    let mut app = mock_app();
    to_result(&mut app);
}

#[test]
fn step3_running_shows_the_plain_word_rail_and_how_to_stop() {
    let mut app = mock_app_with(
        MockBackend::new()
            .with_origin("brew", "jq", &["alpha"])
            .with_apply_delay(Duration::from_secs(5)),
    );
    start_apply(&mut app);
    let screen = render(&mut app, 100, 30);
    assert!(screen.contains("Step 3 of 4 · Applying"), "{screen}");
    assert!(
        screen.contains("Setting up your Mac. Keep this window open."),
        "{screen}"
    );
    for label in [
        "Administrator access",
        "Modules and settings",
        "Packages",
        "Check result",
    ] {
        assert!(screen.contains(label), "{label}: {screen}");
    }
    assert!(screen.contains("Esc  Stop (asks first)"), "{screen}");
    app.handle_key(&key(KeyCode::Esc));
    let screen = render(&mut app, 100, 30);
    assert!(screen.contains("Stop applying?"), "{screen}");
}

#[test]
fn step3_stop_confirm_cancels_and_offers_try_again_copy_and_quit() {
    let mut app = mock_app_with(
        MockBackend::new()
            .with_origin("brew", "jq", &["alpha"])
            .with_apply_delay(Duration::from_secs(5)),
    );
    start_apply(&mut app);
    app.handle_key(&key(KeyCode::Esc));
    app.handle_key(&key(KeyCode::Esc));
    assert!(
        matches!(&app.screen, Screen::Apply(o) if !o.state.cancel_armed()),
        "Esc keeps going"
    );
    app.handle_key(&char_key('q'));
    app.handle_key(&key(KeyCode::Right));
    app.handle_key(&key(KeyCode::Enter));
    finish_apply(&mut app);
    assert_eq!(
        app.screen_name(),
        "apply",
        "a stop stays on the Applying screen"
    );
    let screen = render(&mut app, 100, 30);
    assert!(
        screen.contains("Stopped. Some changes may already have been made."),
        "{screen}"
    );
    assert!(screen.contains("Enter  Try again"), "{screen}");
    assert!(screen.contains("c copy command"), "{screen}");
    let _ = app.take_output();
    app.handle_key(&char_key('c'));
    assert!(
        !app.take_output().is_empty(),
        "the command reaches the clipboard"
    );
    app.handle_key(&char_key('q'));
    assert!(app.quit());
    assert_eq!(app.exit_code(), Some(1));
}

#[test]
fn step3_failure_names_where_it_stopped_and_try_again_runs_the_request_again() {
    let backend = MockBackend::new()
        .with_origin("brew", "jq", &["alpha"])
        .with_failure(crate::backend::ApplyPhase::Packages);
    let probe = backend.clone();
    let mut app = mock_app_with(backend);
    start_apply(&mut app);
    finish_apply(&mut app);
    let screen = render(&mut app, 100, 30);
    assert!(
        screen.contains("Stopped at: Packages (exit code 1)"),
        "{screen}"
    );
    assert!(screen.contains("Enter  Try again"), "{screen}");
    app.handle_key(&key(KeyCode::Enter));
    assert_eq!(app.screen_name(), "apply");
    assert_eq!(probe.requests().len(), 2);
    assert_eq!(probe.requests()[0], probe.requests()[1], "the same request");
}

#[test]
fn step3_viewport_scrolling_anchors_during_new_output() {
    let mut app = mock_app_with(
        MockBackend::new()
            .with_origin("brew", "jq", &["alpha"])
            .with_apply_delay(Duration::from_secs(5)),
    );
    start_apply(&mut app);
    for seq in 1..=40 {
        app.feed_external(&External::new(crate::backend::ApplyLine {
            seq,
            stream: crate::backend::Stream::Stdout,
            text: format!("line {seq}"),
        }));
    }
    let mut terminal =
        ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 30)).expect("terminal");
    draw(&mut app, &mut terminal);
    app.handle_key(&key(KeyCode::Up));
    let Screen::Apply(owner) = &mut app.screen else {
        panic!("expected apply")
    };
    assert!(!owner.viewport.follow);
    let offset = owner.viewport.scroll.offset;
    app.feed_external(&External::new(crate::backend::ApplyLine {
        seq: 41,
        stream: crate::backend::Stream::Stdout,
        text: "line 41".to_string(),
    }));
    draw(&mut app, &mut terminal);
    let Screen::Apply(owner) = &mut app.screen else {
        panic!("expected apply")
    };
    assert_eq!(owner.viewport.scroll.offset, offset);
    app.handle_key(&key(KeyCode::End));
    let Screen::Apply(owner) = &app.screen else {
        panic!("expected apply")
    };
    assert!(owner.viewport.follow);
}

// ---- step 4 --------------------------------------------------------------

#[test]
fn result_shows_checking_at_once_then_the_checked_time() {
    let mut app = mock_app_with(
        MockBackend::new()
            .with_origin("brew", "jq", &["alpha"])
            .with_status_delay(Duration::from_millis(300)),
    );
    start_apply(&mut app);
    finish_apply(&mut app);
    // The Review check also takes 300 ms; the Result check starts now.
    let started = std::time::Instant::now();
    let screen = render(&mut app, 100, 30);
    assert!(started.elapsed() < Duration::from_millis(100));
    assert!(screen.contains("Step 4 of 4 · Checking"), "{screen}");
    assert!(screen.contains("Checking your Mac"), "{screen}");
    assert!(!screen.contains("Checked "), "{screen}");
    wait_check(&mut app);
    let done = render(&mut app, 100, 30);
    assert!(done.contains("Checked 12:04:31"), "{done}");
    assert!(!done.contains("Checking your Mac"), "{done}");
}

#[test]
fn recheck_shows_checking_again_with_the_check_number() {
    let mut app = app_with_status("status-one-differs.json");
    to_result(&mut app);
    app.handle_key(&char_key('R'));
    assert!(render(&mut app, 100, 30).contains("Checking your Mac"));
    wait_check(&mut app);
    assert!(render(&mut app, 100, 30).contains("(check #2)"));
}

#[test]
fn result_shows_up_to_date_for_sixty_six_already_set_defaults() {
    let mut app = app_with_status("status-applied.json");
    to_result(&mut app);
    let screen = render(&mut app, 100, 30);
    assert!(screen.contains("Step 4 of 4 · Done"), "{screen}");
    assert!(screen.contains("Your Mac is set up"), "{screen}");
    assert!(screen.contains("Enter  Finish and exit"), "{screen}");
    assert!(!screen.contains("Needs your attention"), "{screen}");
    assert!(!screen.contains("pending"), "{screen}");
    app.handle_key(&key(KeyCode::Enter));
    assert!(app.quit());
    assert_eq!(app.exit_code(), Some(0));
}

#[test]
fn result_names_the_single_differing_value_and_quit_exits_three() {
    let mut app = app_with_status("status-one-differs.json");
    to_result(&mut app);
    let screen = render(&mut app, 100, 30);
    assert!(screen.contains("Step 4 of 4 · Needs attention"), "{screen}");
    assert!(screen.contains("1 of 67 values differs"), "{screen}");
    assert!(screen.contains("com.apple.dock tilesize"), "{screen}");
    assert!(screen.contains("(now 48, wanted 36)"), "{screen}");
    assert!(
        screen.contains("Enter  Apply the 1 pending area again"),
        "{screen}"
    );
    app.handle_key(&char_key('q'));
    assert!(!app.quit(), "q asks first while changes are pending");
    assert!(render(&mut app, 100, 30).contains("Quit with 1 area"));
    app.handle_key(&char_key('y'));
    assert!(app.quit());
    assert_eq!(app.exit_code(), Some(3));
}

#[test]
fn result_with_a_failed_check_says_why_and_exits_one() {
    let mut app = mock_app_with(
        MockBackend::new()
            .with_origin("brew", "jq", &["alpha"])
            .with_status_error("mise exited 1"),
    );
    to_result(&mut app);
    let screen = render(&mut app, 100, 30);
    assert!(screen.contains("Could not check: "), "{screen}");
    assert!(screen.contains("Enter  Check again"), "{screen}");
    app.handle_key(&char_key('q'));
    assert_eq!(app.exit_code(), Some(1));
}

#[test]
fn enter_on_pending_applies_again_for_exactly_those_areas_then_checks() {
    let (mut app, probe) = app_with_sequence(&["status-one-differs.json", "status-applied.json"]);
    to_result(&mut app);
    app.handle_key(&key(KeyCode::Enter));
    assert_eq!(app.screen_name(), "apply", "Enter starts the re-apply");
    let requests = probe.requests();
    assert_eq!(requests.len(), 2, "first apply, then the re-apply");
    let again = &requests[1];
    assert_eq!(again.only_parts, vec!["macos-defaults"]);
    assert!(again.keep.is_empty() && again.run_base && !again.update && !again.upgrade);
    finish_apply(&mut app);
    wait_check(&mut app);
    let done = render(&mut app, 100, 30);
    assert!(done.contains("Step 4 of 4 · Done"), "{done}");
    assert!(!done.contains("Still different"), "{done}");
}

#[test]
fn details_view_shows_the_exact_command_and_enter_runs_it() {
    let (mut app, probe) = app_with_sequence(&["status-one-differs.json", "status-applied.json"]);
    to_result(&mut app);
    app.handle_key(&char_key('d'));
    let screen = render(&mut app, 100, 30);
    assert!(screen.contains("com.apple.dock tilesize"), "{screen}");
    assert!(screen.contains("setting"), "{screen}");
    assert!(
        screen.contains("bootstrap --only macos-defaults"),
        "the command Enter runs is shown: {screen}"
    );
    let _ = app.take_output();
    app.handle_key(&char_key('c'));
    let copied = app.take_output().concat();
    let shown = crate::reapply::commands(
        &*app.backend,
        &crate::reapply::plan(&fixture_report("status-one-differs.json"), &app.selected)
            .expect("pending")
            .request,
    );
    assert!(
        copied.contains(&base64(shown[0].as_bytes())),
        "the copied command is the shown command: {copied:?}"
    );
    app.handle_key(&key(KeyCode::Enter));
    assert_eq!(app.screen_name(), "apply");
    assert_eq!(probe.requests()[1].only_parts, vec!["macos-defaults"]);
}

#[test]
fn still_different_after_applying_stops_the_loop() {
    let (mut app, probe) =
        app_with_sequence(&["status-one-differs.json", "status-one-differs.json"]);
    to_result(&mut app);
    app.handle_key(&key(KeyCode::Enter));
    finish_apply(&mut app);
    wait_check(&mut app);
    let screen = render(&mut app, 100, 30);
    assert!(
        screen.contains("Still different after applying"),
        "{screen}"
    );
    assert!(screen.contains("com.apple.dock tilesize"), "{screen}");
    assert!(screen.contains("Enter  Finish anyway"), "{screen}");
    assert!(!screen.contains("Apply the"), "no second loop: {screen}");
    app.handle_key(&key(KeyCode::Enter));
    assert!(app.quit());
    assert_eq!(app.exit_code(), Some(3));
    assert_eq!(probe.requests().len(), 2, "no third apply");
}

#[test]
fn esc_never_traps_on_a_pending_result() {
    let mut app = app_with_status("status-one-differs.json");
    to_result(&mut app);
    app.handle_key(&key(KeyCode::Esc));
    app.handle_key(&key(KeyCode::Esc));
    assert!(!app.quit(), "Esc dismisses the question");
    app.handle_key(&char_key('q'));
    app.handle_key(&char_key('y'));
    assert!(app.quit());
    assert_eq!(app.exit_code(), Some(3));
}

#[test]
fn follow_ups_from_the_apply_log_are_typed_and_copyable() {
    let mut app = mock_app_with(
        MockBackend::new()
            .with_origin("brew", "jq", &["alpha"])
            .with_follow_up_log(),
    );
    to_result(&mut app);
    let screen = render(&mut app, 100, 30);
    assert!(
        screen.contains("Optional, for your information"),
        "{screen}"
    );
    assert!(
        screen.contains("Restart Dock, Finder and SystemUIServer to show new settings"),
        "{screen}"
    );
    assert!(
        screen.contains("$ killall Dock Finder SystemUIServer"),
        "{screen}"
    );
    assert!(
        screen.contains("Plugin hook skipped: example-plugin/Tool"),
        "{screen}"
    );
    assert!(screen.contains("c copy command"), "{screen}");
    let _ = app.take_output();
    app.handle_key(&char_key('c'));
    let output = app.take_output().concat();
    let payload = base64(b"killall Dock Finder SystemUIServer");
    assert!(
        output.contains(&format!("\x1b]52;c;{payload}")),
        "copy must reach the terminal as OSC 52: {output:?}"
    );
    assert!(render(&mut app, 100, 30).contains("Copied to the clipboard"));
}

// ---- exit -----------------------------------------------------------------

#[test]
fn exit_summary_for_pending_changes_prints_the_change_and_the_exact_command() {
    let mut app = app_with_status("status-one-differs.json");
    to_result(&mut app);
    let lines = app.exit_summary();
    let text = lines.join("\n");
    assert!(text.contains("5 of 6 areas up to date"), "{text}");
    assert!(text.contains("Needs changes: macOS settings"), "{text}");
    assert!(
        text.contains("com.apple.dock tilesize (now 48, wanted 36)"),
        "{text}"
    );
    let shown = crate::reapply::commands(
        &*app.backend,
        &crate::reapply::plan(&fixture_report("status-one-differs.json"), &app.selected)
            .expect("pending")
            .request,
    );
    assert!(
        text.contains(&format!("  $ {}", shown[0])),
        "the printed command is the shown and executed one: {text}"
    );
    let mut out = Vec::new();
    crate::cli::write_exit_summary(&mut out, &lines);
    let printed = String::from_utf8(out).expect("utf8");
    assert!(
        printed.starts_with('\n') && printed.contains("Needs changes"),
        "{printed}"
    );
}

#[test]
fn exit_summary_for_a_clean_machine_lists_follow_up_commands() {
    let mut app = mock_app_with(
        MockBackend::new()
            .with_origin("brew", "jq", &["alpha"])
            .with_follow_up_log(),
    );
    to_result(&mut app);
    let text = app.exit_summary().join("\n");
    assert!(text.contains("Your Mac is set up."), "{text}");
    assert!(
        text.contains("  $ killall Dock Finder SystemUIServer"),
        "{text}"
    );
    assert!(!text.contains("To apply the remaining changes"), "{text}");
}

#[test]
fn exit_summary_after_a_stopped_apply_names_where_and_how_to_try_again() {
    let mut app = mock_app_with(
        MockBackend::new()
            .with_origin("brew", "jq", &["alpha"])
            .with_failure(crate::backend::ApplyPhase::Packages),
    );
    start_apply(&mut app);
    finish_apply(&mut app);
    let text = app.exit_summary().join("\n");
    assert!(
        text.contains("Stopped at: Packages (exit code 1)"),
        "{text}"
    );
    assert!(text.contains("To try again:"), "{text}");
    assert!(
        text.contains("  $ mise -C /tmp/emc-mock-config -E alpha bootstrap"),
        "{text}"
    );
}

#[test]
fn leaving_before_any_work_prints_nothing_and_a_tiny_window_still_quits_cleanly() {
    let mut app = mock_app();
    assert!(app.exit_summary().is_empty());
    app.shell.too_small = true;
    app.handle_key(&char_key('q'));
    assert!(app.quit());
    assert_eq!(app.exit_code(), Some(0));
}

// ---- keys, help, theme ---------------------------------------------------

#[test]
fn question_mark_opens_help_on_every_screen_and_esc_closes_it() {
    let mut app = mock_app();
    app.handle_key(&char_key('?'));
    let screen = render(&mut app, 100, 30);
    assert!(screen.contains("Keys on this screen"), "{screen}");
    assert!(screen.contains("Words used here"), "{screen}");
    assert!(
        screen.contains("module: a bundle of apps and settings"),
        "{screen}"
    );
    app.handle_key(&key(KeyCode::Esc));
    assert!(!render(&mut app, 100, 30).contains("Keys on this screen"));
    to_result(&mut app);
    app.handle_key(&char_key('?'));
    assert!(render(&mut app, 100, 30).contains("Up to date: already matches"));
    app.handle_key(&char_key('?'));
    assert!(!app.quit(), "help swallows q instead of quitting");
}

#[test]
fn question_mark_is_typed_text_while_searching() {
    let mut app = mock_app();
    app.handle_key(&char_key('/'));
    app.handle_key(&char_key('?'));
    assert!(!render(&mut app, 100, 30).contains("Keys on this screen"));
}

#[test]
fn removed_keys_do_nothing() {
    let mut app = app_with_status("status-one-differs.json");
    to_result(&mut app);
    for removed in ['B', 'D', 'b'] {
        app.handle_key(&char_key(removed));
        assert_eq!(app.screen_name(), "result");
        assert!(!app.quit());
    }
    let mut app = mock_app();
    to_review(&mut app);
    app.handle_key(&key(KeyCode::Backspace));
    assert_eq!(app.screen_name(), "packages", "Backspace is no longer back");
}

#[test]
fn theme_honours_no_color_and_dumb_terminals() {
    use junie_tui::theme::{ColorLevel, Glyphs};
    let env = |pairs: &'static [(&'static str, &'static str)]| {
        move |k: &str| {
            pairs
                .iter()
                .find(|(key, _)| *key == k)
                .map(|(_, v)| (*v).to_string())
        }
    };
    let plain = theme_from(env(&[
        ("TERM", "xterm-256color"),
        ("COLORTERM", "truecolor"),
    ]));
    assert_eq!(plain.level, ColorLevel::TrueColor);
    let no_color = theme_from(env(&[("TERM", "xterm-256color"), ("NO_COLOR", "1")]));
    assert_eq!(no_color.level, ColorLevel::Mono);
    let dumb = theme_from(env(&[("TERM", "dumb")]));
    assert_eq!(dumb.level, ColorLevel::Mono);
    assert!(dumb.glyphs == Glyphs::ASCII);
}

#[test]
fn ascii_and_mono_rendering_still_tells_states_apart_by_glyph_and_word() {
    let mut app = app_with_status("status-one-differs.json");
    app.theme = theme_from(|k| (k == "TERM").then(|| "dumb".to_string()));
    to_result(&mut app);
    app.handle_key(&char_key('e'));
    let screen = render(&mut app, 100, 30);
    assert!(screen.contains("[ok] 5 of 6 areas up to date"), "{screen}");
    assert!(screen.contains("[!] 1 needs changes"), "{screen}");
    assert!(screen.contains("[!] macOS settings"), "{screen}");
    assert!(screen.contains("[ok] Dotfiles"), "{screen}");
    assert!(
        screen.is_ascii() || !screen.contains('✓'),
        "no Unicode state glyphs: {screen}"
    );
}

// ---- layout and vocabulary over every screen -----------------------------

/// Every screen and state, rendered at one size.
fn all_screens(w: u16, h: u16) -> Vec<(&'static str, String)> {
    let mut out: Vec<(&'static str, String)> = Vec::new();
    let mut shot = |name: &'static str, app: &mut App| out.push((name, render(app, w, h)));

    let mut app = mock_app();
    shot("modules", &mut app);
    app.handle_key(&char_key('n'));
    shot("modules, nothing selected", &mut app);
    app.handle_key(&char_key('/'));
    for c in "zzz".chars() {
        app.handle_key(&char_key(c));
    }
    shot("modules, no match", &mut app);
    app.handle_key(&key(KeyCode::Esc));
    app.handle_key(&char_key('?'));
    shot("modules, help", &mut app);

    let mut app = mock_app_with(
        MockBackend::new()
            .with_origin("brew", "jq", &["alpha"])
            .with_status_delay(Duration::from_millis(150)),
    );
    app.handle_key(&key(KeyCode::Enter));
    shot("review, checking", &mut app);
    wait_review(&mut app);
    shot("review", &mut app);
    app.handle_key(&char_key('h'));
    shot("review, installed shown", &mut app);

    let mut app = mock_app_with(
        MockBackend::new()
            .with_origin("brew", "jq", &["alpha"])
            .with_apply_delay(Duration::from_secs(30)),
    );
    start_apply(&mut app);
    shot("applying", &mut app);
    app.handle_key(&key(KeyCode::Esc));
    shot("applying, stop question", &mut app);
    app.handle_key(&key(KeyCode::Esc));
    app.handle_key(&char_key('?'));
    shot("applying, help", &mut app);

    let mut app = mock_app_with(
        MockBackend::new()
            .with_origin("brew", "jq", &["alpha"])
            .with_failure(crate::backend::ApplyPhase::Packages),
    );
    start_apply(&mut app);
    finish_apply(&mut app);
    shot("stopped at a phase", &mut app);

    let mut app = mock_app_with(
        MockBackend::new()
            .with_origin("brew", "jq", &["alpha"])
            .with_status_delay(Duration::from_millis(200)),
    );
    start_apply(&mut app);
    finish_apply(&mut app);
    shot("result, checking", &mut app);

    let mut app = app_with_status("status-applied.json");
    to_result(&mut app);
    shot("result, done", &mut app);

    let mut app = mock_app_with(
        MockBackend::new()
            .with_origin("brew", "jq", &["alpha"])
            .with_follow_up_log(),
    );
    to_result(&mut app);
    shot("result, done with follow-ups", &mut app);
    app.handle_key(&char_key('e'));
    shot("result, done, all good expanded", &mut app);

    let mut app = app_with_status("status-one-differs.json");
    to_result(&mut app);
    shot("result, needs attention", &mut app);
    app.handle_key(&char_key('d'));
    shot("result, details", &mut app);
    app.handle_key(&key(KeyCode::Esc));
    app.handle_key(&char_key('q'));
    shot("result, quit question", &mut app);
    app.handle_key(&char_key('n'));
    app.handle_key(&char_key('?'));
    shot("result, help", &mut app);

    let mut app = mock_app_with(
        MockBackend::new()
            .with_origin("brew", "jq", &["alpha"])
            .with_status_error("mise exited 1"),
    );
    to_result(&mut app);
    shot("result, check failed", &mut app);

    let (mut app, _) = app_with_sequence(&["status-one-differs.json", "status-one-differs.json"]);
    to_result(&mut app);
    app.handle_key(&key(KeyCode::Enter));
    finish_apply(&mut app);
    wait_check(&mut app);
    shot("result, still different", &mut app);
    out
}

/// Words that must never appear in the rendered interface (matched as whole
/// words, case-insensitive). Command lines (`$ ...`) are the one place CLI
/// syntax may appear, because the command is the thing being shown or copied.
const BANNED: &[&str] = &[
    "drift",
    "drifted",
    "kept",
    "omitted",
    "one-shot",
    "part",
    "parts",
    "environment",
    "environments",
    "s1",
    "s2",
    "s3",
    "s4",
    "-e",
    "macos_defaults",
    "macos_login_items",
    "mise_shell_activate",
    "login_shell",
    "user_services",
    "plugin_deps",
    "re-check",
];

fn banned_words(screen: &str) -> Vec<String> {
    let mut found = Vec::new();
    for line in screen.lines() {
        let trimmed = line.trim_start().trim_start_matches('│').trim_start();
        if trimmed.starts_with("$ ") {
            continue;
        }
        let tokens: Vec<String> = line
            .split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '-'))
            .filter(|t| !t.is_empty())
            .map(str::to_lowercase)
            .collect();
        for (i, token) in tokens.iter().enumerate() {
            if BANNED.contains(&token.as_str()) {
                found.push(token.clone());
            }
            // "pending" is fine only in "the N pending areas again".
            if token == "pending"
                && !matches!(
                    tokens.get(i + 1).map(String::as_str),
                    Some("area" | "areas")
                )
            {
                found.push(token.clone());
            }
        }
    }
    found
}

#[test]
fn no_screen_uses_banned_vocabulary_at_100x30_or_80x24() {
    for (w, h) in [(100, 30), (80, 24)] {
        for (name, screen) in all_screens(w, h) {
            let found = banned_words(&screen);
            assert!(
                found.is_empty(),
                "{name} at {w}x{h} uses {found:?}:\n{screen}"
            );
        }
    }
}

#[test]
fn every_screen_shows_its_footer_and_nothing_is_cut_without_a_keyed_way_to_read_it() {
    for (w, h) in [(100, 30), (80, 24)] {
        for (name, screen) in all_screens(w, h) {
            let lines: Vec<&str> = screen.lines().collect();
            assert_eq!(lines.len(), h as usize, "{name}");
            let footer = lines[h as usize - 1];
            assert!(
                !footer.contains('…'),
                "{name} at {w}x{h} cuts its footer: {footer:?}\n{screen}"
            );
            assert!(
                footer.split_whitespace().count() >= 2,
                "{name} at {w}x{h} has no footer: {footer:?}\n{screen}"
            );
            if !name.ends_with("help")
                && !name.contains("stop question")
                && name != "result, checking"
            {
                let primary = lines[h as usize - 2];
                assert!(
                    primary.contains("Enter")
                        || primary.contains("Esc")
                        || primary.contains("Select at least")
                        || primary.contains("Checking")
                        || primary.contains("Quit with"),
                    "{name} at {w}x{h} has no primary line: {primary:?}\n{screen}"
                );
            }
            if (name.starts_with("result") || name == "review")
                && !name.contains("help")
                && !name.contains("details")
                && !name.contains("checking")
            {
                assert!(
                    !screen.contains('…'),
                    "{name} at {w}x{h} truncates text:\n{screen}"
                );
            }
        }
    }
}

#[test]
fn the_tiny_terminal_floor_says_what_size_is_needed() {
    let mut app = mock_app();
    let screen = render(&mut app, 60, 18);
    assert!(
        screen.contains("72") || screen.contains("small"),
        "{screen}"
    );
}

/// Developer aid: `cargo nextest run --run-ignored only dump_all_screens
/// --no-capture` prints every screen at both sizes.
#[test]
#[ignore]
fn dump_all_screens() {
    for (w, h) in [(100, 30), (80, 24)] {
        for (name, screen) in all_screens(w, h) {
            println!("===== {name} {w}x{h}\n{screen}");
        }
    }
}
