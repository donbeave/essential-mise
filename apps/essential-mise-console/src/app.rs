//! Console app: screen routing, selection memory, and the apply event pump.
//!
//! The app owns one screen at a time (S1→S2→S3→S4, with `Back` restoring
//! the previous selection state) plus the apply event receiver. The pinned
//! shell owns focus, hit registration, pointer routing, cursor placement, and
//! clipboard output; each screen owner retains its stateful widgets.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use crate::backend::{ApplyRequest, Backend, BackendError, ResId};
use crate::screens::apply::{ApplyAction, ApplyOwner, ApplyScreen};
use crate::screens::modules::{ModuleAction, ModuleInfo, ModulesOwner, ModulesState};
use crate::screens::packages::{PackageAction, PackagesOwner, PackagesState, package_rows};
use crate::screens::result::{ResultAction, ResultOwner, ResultScreen};
use junie_tui::clock::{Moment, WallClock};
use junie_tui::core::event::{External, Input, Key, Outcome};
use junie_tui::core::focus::FocusChange;
use junie_tui::core::id::WidgetId;
use junie_tui::core::pointer::{PointerEvent, PointerHost, PointerParts, PointerRouter};
use junie_tui::core::response::Response;
use junie_tui::runtime::ExternalReceiver;
use junie_tui::shell::{App as ShellApp, PageKey, Shell};
use junie_tui::theme::{ColorLevel, Theme};
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::widgets::too_small::TooSmall;
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};

pub(crate) enum Screen {
    Modules(ModulesOwner),
    Packages(PackagesOwner),
    Apply(Box<ApplyOwner>),
    Result(Box<ResultOwner>),
}

impl Screen {
    fn primary_focus(&self) -> Option<WidgetId> {
        match self {
            Self::Modules(owner) => Some(owner.primary_focus()),
            Self::Packages(owner) => Some(owner.primary_focus()),
            Self::Apply(owner) => Some(owner.primary_focus()),
            Self::Result(owner) => Some(owner.primary_focus()),
        }
    }

    fn page_key(&self) -> PageKey {
        match self {
            Self::Modules(_) => PageKey::of("emc.modules"),
            Self::Packages(_) => PageKey::of("emc.packages"),
            Self::Apply(_) => PageKey::of("emc.apply"),
            Self::Result(_) => PageKey::of("emc.result"),
        }
    }
}

/// Console app: routing, selection memory, and the apply event pump.
pub struct App {
    backend: Arc<dyn Backend>,
    config_dir: std::path::PathBuf,
    screen: Screen,
    envs: Vec<ModuleInfo>,
    selected: Vec<String>,
    packages_snapshot: Option<PackagesOwner>,
    shell: Shell<()>,
    clock: WallClock,
    tick: u64,
    exit_code: Option<u8>,
    last_error: Option<String>,
    theme: Theme,
}

impl App {
    /// Build from a backend and config dir. Reads the `essential-mise.toml`
    /// module catalogue and `miserc.local.toml` pre-ticks (both read-only).
    pub fn new(backend: Arc<dyn Backend>, config_dir: &Path) -> Result<Self, BackendError> {
        let manifest = crate::manifest::Manifest::load(config_dir)
            .map_err(|e| BackendError::Parse(e.to_string()))?;
        let envs: Vec<ModuleInfo> = manifest
            .modules
            .into_iter()
            .map(|m| ModuleInfo {
                name: m.name,
                description: m.description,
            })
            .collect();
        let preticks =
            crate::screens::modules::read_preticks(&config_dir.join("miserc.local.toml"));
        let shell = Shell::new();
        shell.clipboard.install();
        let mut app = Self {
            backend,
            config_dir: config_dir.to_path_buf(),
            screen: Screen::Modules(ModulesOwner::new(ModulesState::new(
                envs.clone(),
                &preticks,
            ))),
            envs,
            selected: preticks,
            packages_snapshot: None,
            shell,
            clock: WallClock::new(),
            tick: 0,
            exit_code: None,
            last_error: None,
            theme: theme_for_env(),
        };
        app.refresh_badges();
        let focus = app.screen.primary_focus();
        app.shell.focus.set(focus);
        Ok(app)
    }

    /// Current screen discriminant (tests + paint dispatch).
    pub fn screen_name(&self) -> &'static str {
        match &self.screen {
            Screen::Modules(_) => "modules",
            Screen::Packages(_) => "packages",
            Screen::Apply(_) => "apply",
            Screen::Result(_) => "result",
        }
    }

    /// Whether the app requested quit.
    pub fn quit(&self) -> bool {
        self.shell.quit
    }

    /// Exit code after quit (`Some` once `Done` fired).
    pub fn exit_code(&self) -> Option<u8> {
        self.exit_code
    }

    /// Last backend error (shown in the chrome status line).
    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    /// Route a key to the active screen, performing transitions.
    pub fn handle_key(&mut self, key: &Key) {
        let _ = self.route_key(key);
    }

    fn route_key(&mut self, key: &Key) -> Outcome {
        self.shell.pointer.suppress_hover();
        if self.shell.too_small {
            if key.is_char('q') || key.ctrl_char('q') {
                self.shell.quit = true;
                return Outcome::Changed;
            }
            return Outcome::Consumed;
        }
        let clipboard = self.shell.clipboard.clone();
        let backend = self.backend.clone();
        let out = match &mut self.screen {
            Screen::Modules(owner) => {
                let response = owner.key(key, &mut self.shell.focus, &self.shell.ring);
                self.modules_response(response)
            }
            Screen::Packages(owner) => {
                let response = owner.key(key, &mut self.shell.focus, &self.shell.ring);
                self.packages_response(response)
            }
            Screen::Apply(owner) => {
                let response = owner.key(key, &mut self.shell.focus, &self.shell.ring, &clipboard);
                self.apply_response(response)
            }
            Screen::Result(owner) => {
                let response = owner.key(
                    key,
                    &mut self.shell.focus,
                    &self.shell.ring,
                    &*backend,
                    &clipboard,
                );
                self.result_response(response)
            }
        };
        if out == Outcome::Ignored && !ShellApp::layer_open(self) {
            use ratatui::crossterm::event::KeyCode;
            match key.code {
                KeyCode::Tab => {
                    self.shell.focus_next();
                    return Outcome::Changed;
                }
                KeyCode::BackTab => {
                    self.shell.focus_prev();
                    return Outcome::Changed;
                }
                _ => {}
            }
        }
        out
    }

    fn route_paste(&mut self, text: &str) -> Outcome {
        if self.shell.too_small {
            return Outcome::Consumed;
        }
        match &mut self.screen {
            Screen::Modules(owner) => owner.paste(text, &self.shell.focus),
            Screen::Packages(owner) => owner.paste(text, &self.shell.focus),
            Screen::Apply(owner) => owner.paste(text),
            Screen::Result(_) => Outcome::Ignored,
        }
    }

    fn modules_response(&mut self, response: Response<ModuleAction>) -> Outcome {
        let (out, action) = response.into_parts();
        match action {
            Some(ModuleAction::Confirm) => {
                self.enter_packages();
                Outcome::Changed
            }
            Some(ModuleAction::Quit) => {
                self.shell.quit = true;
                Outcome::Changed
            }
            Some(ModuleAction::None) | None => out,
        }
    }

    fn packages_response(&mut self, response: Response<PackageAction>) -> Outcome {
        let (out, action) = response.into_parts();
        match action {
            Some(PackageAction::Apply) => {
                self.enter_apply();
                Outcome::Changed
            }
            Some(PackageAction::Back) => {
                self.back_to_modules();
                Outcome::Changed
            }
            Some(PackageAction::None) | None => out,
        }
    }

    fn apply_response(&mut self, response: Response<ApplyAction>) -> Outcome {
        let (out, action) = response.into_parts();
        match action {
            Some(ApplyAction::Done) => {
                self.enter_result();
                Outcome::Changed
            }
            Some(ApplyAction::None) | None => out,
        }
    }

    fn result_response(&mut self, response: Response<ResultAction>) -> Outcome {
        let (out, action) = response.into_parts();
        match action {
            Some(ResultAction::Back) => {
                self.back_to_packages();
                Outcome::Changed
            }
            Some(ResultAction::Done) => {
                self.exit_code = Some(0);
                self.shell.quit = true;
                Outcome::Changed
            }
            Some(ResultAction::None) | None => out,
        }
    }

    /// Drain the S3 worker channel into the screen (no-op elsewhere).
    pub fn pump(&mut self) {
        let mut events = Vec::new();
        if let Screen::Apply(owner) = &self.screen {
            events.extend(owner.state.drain());
        }
        for event in &events {
            self.feed_external(event);
        }
        let _ = self.poll_apply();
    }

    fn feed_external(&mut self, external: &External) -> bool {
        let consumed = match &mut self.screen {
            Screen::Apply(owner) => owner.on_external(external),
            _ => false,
        };
        if consumed {
            let _ = self.poll_apply();
        }
        consumed
    }

    fn poll_apply(&mut self) -> Outcome {
        let outcome = match &mut self.screen {
            Screen::Apply(owner) => owner.state.try_join(),
            _ => None,
        };
        match outcome {
            Some(crate::backend::ApplyOutcome::Failed { .. }) => match &mut self.screen {
                Screen::Apply(owner) => owner.sync_modal_from_domain(&mut self.shell.focus),
                _ => Outcome::Ignored,
            },
            Some(_) => match &mut self.screen {
                Screen::Apply(owner) => {
                    owner.sync_completion_focus(&mut self.shell.focus);
                    Outcome::Changed
                }
                _ => Outcome::Ignored,
            },
            None => Outcome::Ignored,
        }
    }

    /// Refresh S1 badges from live plan/status (all known envs).
    /// Backend failures keep the pending defaults; never panics.
    pub fn refresh_badges(&mut self) {
        let all: Vec<String> = match &self.screen {
            Screen::Modules(owner) => owner.state.envs.iter().map(|e| e.name.clone()).collect(),
            _ => return,
        };
        let empty = HashMap::new();
        let origins = self.backend.origin_map(&self.config_dir).unwrap_or(empty);
        let plan = self.backend.plan(&all, None).unwrap_or_default();
        let parts_ok = self
            .backend
            .status(&all)
            .map(|s| s.parts.iter().all(|p| p.state == "ok"))
            .unwrap_or(true);
        if let Screen::Modules(owner) = &mut self.screen {
            owner.state.refresh_badges(&plan, &origins, parts_ok);
        }
    }

    fn enter_packages(&mut self) {
        let selected = match &self.screen {
            Screen::Modules(owner) => owner.state.selected_envs(),
            _ => return,
        };
        self.selected = selected.clone();
        match self.backend.origin_map(&self.config_dir) {
            Ok(origins) => {
                let rows = package_rows(&selected, &Self::filter_origins(&origins, &selected));
                self.set_screen(Screen::Packages(PackagesOwner::new(PackagesState::new(
                    rows,
                ))));
            }
            Err(e) => {
                self.last_error = Some(format!("origin_map failed: {e}"));
            }
        }
    }

    fn filter_origins(
        origins: &HashMap<ResId, Vec<String>>,
        selected: &[String],
    ) -> HashMap<ResId, Vec<String>> {
        origins
            .iter()
            .filter(|(_, decl)| decl.iter().any(|e| selected.contains(e)))
            .map(|(id, decl)| {
                (
                    ResId {
                        kind: id.kind.clone(),
                        name: id.name.clone(),
                    },
                    decl.iter()
                        .filter(|e| selected.contains(e))
                        .cloned()
                        .collect(),
                )
            })
            .collect()
    }

    fn enter_apply(&mut self) {
        let (keep, envs) = match &self.screen {
            Screen::Packages(owner) => (owner.state.kept_specs(), self.selected.clone()),
            _ => return,
        };
        self.packages_snapshot = match &self.screen {
            Screen::Packages(owner) => Some(owner.clone()),
            _ => None,
        };
        let req = ApplyRequest {
            envs,
            keep,
            manager: None,
            skip_parts: Vec::new(),
            only_parts: Vec::new(),
            update: true,
            upgrade: true,
            dry_run: false,
        };
        let mut state = ApplyScreen::new(self.backend.clone(), req);
        state.start();
        self.set_screen(Screen::Apply(Box::new(ApplyOwner::new(state))));
    }

    /// Jump directly to a screen (`tui [screen]`; S3 starts immediately).
    pub fn jump_to(&mut self, screen: crate::cli::TuiScreen) {
        use crate::cli::TuiScreen;
        match screen {
            TuiScreen::Modules => self.back_to_modules(),
            TuiScreen::Packages => self.enter_packages(),
            TuiScreen::Apply => {
                if !matches!(self.screen, Screen::Packages(_)) {
                    self.enter_packages();
                }
                self.enter_apply();
            }
            TuiScreen::Result => self.enter_result(),
        }
    }

    fn back_to_packages(&mut self) {
        if let Some(owner) = self.packages_snapshot.clone() {
            self.set_screen(Screen::Packages(owner));
        }
    }

    fn back_to_modules(&mut self) {
        self.set_screen(Screen::Modules(ModulesOwner::new(ModulesState::new(
            self.envs.clone(),
            &self.selected,
        ))));
        self.refresh_badges();
    }

    fn enter_result(&mut self) {
        let backend = self.backend.clone();
        let envs = self.selected.clone();
        let keep = match &self.screen {
            Screen::Packages(owner) => owner.state.kept_specs(),
            Screen::Apply(_) => self
                .packages_snapshot
                .as_ref()
                .map(|s| s.state.kept_specs())
                .unwrap_or_default(),
            _ => Vec::new(),
        };
        let mut screen = ResultScreen::new(backend.clone(), envs, keep);
        screen.refresh(&*backend);
        self.set_screen(Screen::Result(Box::new(ResultOwner::new(screen))));
    }

    fn set_screen(&mut self, screen: Screen) {
        self.screen = screen;
        let focus = self.screen.primary_focus();
        self.shell.focus.set(focus);
    }

    fn tick_duration(&self) -> Duration {
        if self.shell.pointer.autoscrolling() {
            return Duration::from_millis(self.shell.pointer.config.autoscroll_ms);
        }
        if self.shell.pointer.track_repeating() {
            return Duration::from_millis(self.shell.pointer.config.track_repeat_ms);
        }
        Duration::from_millis(100)
    }

    fn on_tick(&mut self) -> Outcome {
        let mut out = PointerRouter::tick(self).or(self.poll_apply());
        let now = self.clock.now_ms();
        if self.shell.pointer.expire_flash(now) {
            out = Outcome::Changed;
        }
        if self.shell.status.expire(Moment::from_millis(now)) {
            out = Outcome::Changed;
        }
        let animating = self.shell.pointer.has_flash()
            || matches!(&self.screen, Screen::Apply(owner) if owner.state.started() && owner.state.outcome().is_none());
        if animating {
            self.tick = self.tick.wrapping_add(1);
            out = Outcome::Changed;
        }
        out
    }
}

fn theme_for_env() -> Theme {
    if std::env::var("COLORTERM").as_deref() == Ok("truecolor") {
        Theme::for_level(ColorLevel::TrueColor)
    } else {
        Theme::for_level(ColorLevel::Ansi256)
    }
}

impl ShellApp for App {
    type Status = ();

    fn shell(&self) -> &Shell<()> {
        &self.shell
    }

    fn shell_mut(&mut self) -> &mut Shell<()> {
        &mut self.shell
    }

    fn theme(&self) -> Theme {
        self.theme
    }

    fn too_small(&self) -> TooSmall<'_> {
        TooSmall::new(crate::screens::chrome::TITLE)
    }

    fn now_ms(&self) -> u64 {
        self.clock.now_ms()
    }

    fn tick(&self) -> u64 {
        self.tick
    }

    fn tick_interval(&self) -> Duration {
        self.tick_duration()
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        match &mut self.screen {
            Screen::Modules(owner) => owner.draw(area, buf, ctx),
            Screen::Packages(owner) => owner.draw(area, buf, ctx),
            Screen::Apply(owner) => owner.draw(area, buf, ctx),
            Screen::Result(owner) => owner.draw(area, buf, ctx),
        }
    }

    fn on_input(&mut self, input: Input) -> Outcome {
        match input {
            Input::Resize(_, _) => Outcome::Changed,
            Input::Key(key) => self.route_key(&key),
            Input::Mouse(mouse) => PointerRouter::route(self, mouse),
            Input::Paste(text) => self.route_paste(&text),
            Input::External(external) if self.feed_external(&external) => Outcome::Changed,
            Input::External(_) => Outcome::Ignored,
            Input::Tick => self.on_tick(),
        }
    }

    fn on_blur(&mut self, change: FocusChange) -> Outcome {
        let clipboard = self.shell.clipboard.clone();
        match &mut self.screen {
            Screen::Modules(owner) => owner.blur(change),
            Screen::Packages(owner) => owner.blur(change),
            Screen::Apply(_) => Outcome::Ignored,
            Screen::Result(owner) => owner.blur(change, &clipboard),
        }
    }

    fn layer_open(&self) -> bool {
        matches!(&self.screen, Screen::Apply(owner) if owner.layer_open())
    }

    fn page_under_layer(&self) -> bool {
        ShellApp::layer_open(self)
    }

    fn page_key(&self) -> PageKey {
        self.screen.page_key()
    }

    fn primary_focus(&self) -> Option<WidgetId> {
        self.screen.primary_focus()
    }

    fn external(&self) -> Option<&ExternalReceiver> {
        match &self.screen {
            Screen::Apply(owner) => owner.state.receiver(),
            _ => None,
        }
    }
}

impl PointerHost for App {
    fn pointer_parts(&mut self) -> PointerParts<'_> {
        self.shell.pointer_parts()
    }

    fn layer_open(&self) -> bool {
        ShellApp::layer_open(self)
    }

    fn now_ms(&self) -> u64 {
        self.clock.now_ms()
    }

    fn deliver(&mut self, ev: &PointerEvent) -> Response<()> {
        let clipboard = self.shell.clipboard.clone();
        match &mut self.screen {
            Screen::Modules(owner) => {
                let response = owner.pointer(ev, &mut self.shell.focus);
                self.modules_response(response).into()
            }
            Screen::Packages(owner) => {
                let response = owner.pointer(ev, &mut self.shell.focus);
                self.packages_response(response).into()
            }
            Screen::Apply(owner) => {
                let response = owner.pointer(ev, &mut self.shell.focus, &clipboard);
                self.apply_response(response).into()
            }
            Screen::Result(owner) => {
                let response =
                    owner.pointer(ev, &mut self.shell.focus, &self.shell.ring, &clipboard);
                self.result_response(response).into()
            }
        }
    }

    fn outside(&mut self, _pos: Position) -> Response<()> {
        let clipboard = self.shell.clipboard.clone();
        match &mut self.screen {
            Screen::Apply(owner) if owner.layer_open() => {
                let response = owner.outside(&mut self.shell.focus, &clipboard);
                self.apply_response(response).into()
            }
            _ => Response::changed(),
        }
    }
}

#[cfg(test)]
mod tests {
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

    fn write_manifest(dir: &Path, modules: &[(&str, &str)]) {
        let mut text = String::from("schema = 1\nname = \"test\"\n\n[setup]\nbase_modules = []\n");
        for (name, description) in modules {
            text.push_str(&format!(
                "\n[[modules]]\nname = \"{name}\"\ndescription = \"{description}\"\n"
            ));
        }
        std::fs::write(dir.join("essential-mise.toml"), text).expect("write manifest");
    }

    fn mock_app_with(backend: MockBackend) -> App {
        let dir = std::env::temp_dir().join(format!("emc-test-app-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        write_manifest(&dir, &[("alpha", "First env"), ("beta", "Second env")]);
        std::fs::write(dir.join("miserc.local.toml"), "env = [\"alpha\"]\n").expect("write local");
        let backend: Arc<dyn Backend> = Arc::new(backend);
        App::new(backend, &dir).expect("app builds")
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

    fn settle(app: &mut App) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while app.screen_name() == "apply" && std::time::Instant::now() < deadline {
            app.pump();
            if matches!(&app.screen, Screen::Apply(owner) if owner.state.outcome().is_some()) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(
            matches!(&app.screen, Screen::Apply(owner) if owner.state.outcome().is_some()),
            "apply must finish before verification"
        );
        app.handle_key(&key(KeyCode::Enter));
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

    #[test]
    fn s1_enter_moves_to_s2_with_pretrained_selection() {
        let mut app = mock_app();
        assert_eq!(app.screen_name(), "modules");
        app.handle_key(&char_key(' '));
        app.handle_key(&key(KeyCode::Down));
        app.handle_key(&char_key(' '));
        app.handle_key(&key(KeyCode::Enter));
        assert_eq!(app.screen_name(), "packages");
    }

    #[test]
    fn s2_enter_starts_apply_and_success_waits_for_results() {
        let mut app = mock_app();
        app.handle_key(&key(KeyCode::Enter));
        assert_eq!(app.screen_name(), "packages");
        app.handle_key(&key(KeyCode::Enter));
        assert_eq!(app.screen_name(), "apply");
        assert!(
            matches!(&app.screen, Screen::Apply(owner) if owner.state.started()),
            "confirming packages must start the worker without a second Enter"
        );
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while std::time::Instant::now() < deadline {
            app.pump();
            if matches!(&app.screen, Screen::Apply(owner) if owner.state.outcome().is_some()) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(app.screen_name(), "apply");
        assert!(
            matches!(&app.screen, Screen::Apply(owner) if owner.state.outcome() == Some(&crate::backend::ApplyOutcome::Success)),
            "successful output must remain visible until the user continues"
        );
        app.handle_key(&key(KeyCode::Enter));
        assert_eq!(app.screen_name(), "result");
    }

    #[test]
    fn s4_back_restores_packages_and_done_quits() {
        let mut app = mock_app();
        app.handle_key(&key(KeyCode::Enter));
        app.handle_key(&key(KeyCode::Enter));
        settle(&mut app);
        assert_eq!(app.screen_name(), "result");
        app.handle_key(&char_key('B'));
        assert_eq!(app.screen_name(), "packages");
        app.handle_key(&key(KeyCode::Enter));
        app.handle_key(&key(KeyCode::Enter));
        settle(&mut app);
        app.handle_key(&key(KeyCode::Enter));
        assert!(app.quit());
        assert_eq!(app.exit_code(), Some(0));
    }

    #[test]
    fn s1_escape_quits() {
        let mut app = mock_app();
        app.handle_key(&key(KeyCode::Esc));
        assert!(app.quit());
    }

    #[test]
    fn interaction_regressions_filter_accepts_typing_and_paste() {
        let mut app = mock_app();
        app.handle_key(&char_key('/'));
        app.handle_key(&char_key('b'));
        assert_eq!(
            app.handle(Input::Paste("eta".to_string())),
            Outcome::Changed
        );
        let Screen::Modules(owner) = &app.screen else {
            panic!("expected modules")
        };
        assert_eq!(owner.state.query(), "beta");
        assert_eq!(owner.state.counts().0, 1);
        assert!(owner.state.selected_envs().contains(&"alpha".to_string()));
    }

    #[test]
    fn interaction_regressions_second_package_row_toggles() {
        let mut app = mock_app();
        app.handle_key(&key(KeyCode::Down));
        app.handle_key(&char_key(' '));
        app.handle_key(&key(KeyCode::Enter));
        app.handle_key(&key(KeyCode::Down));
        app.handle_key(&char_key(' '));
        let Screen::Packages(owner) = &app.screen else {
            panic!("expected packages")
        };
        assert!(owner.state.is_kept("brew", "jq"));
        assert!(!owner.state.is_kept("brew", "rg"));
    }

    #[test]
    fn interaction_regressions_cancel_dialog_dismisses_without_cancelling() {
        let mut app = mock_app_with(
            MockBackend::new()
                .with_origin("brew", "jq", &["alpha"])
                .with_origin("brew", "rg", &["beta"])
                .with_apply_delay(std::time::Duration::from_secs(5)),
        );
        app.handle_key(&key(KeyCode::Enter));
        app.handle_key(&key(KeyCode::Enter));
        app.handle_key(&key(KeyCode::Esc));
        app.handle_key(&key(KeyCode::Enter));
        let Screen::Apply(owner) = &app.screen else {
            panic!("expected apply")
        };
        assert!(owner.state.started());
        assert!(!owner.state.cancel_armed());
        assert!(!owner.state.cancelling());
        assert!(owner.state.outcome().is_none());
    }

    #[test]
    fn interaction_regressions_resize_requires_redraw() {
        let mut app = mock_app();
        assert_eq!(app.handle(Input::Resize(80, 24)), Outcome::Changed);
    }

    #[test]
    fn interaction_regressions_result_copy_emits_osc52() {
        let mut app = mock_app();
        app.handle_key(&key(KeyCode::Enter));
        app.handle_key(&key(KeyCode::Enter));
        settle(&mut app);
        assert_eq!(app.screen_name(), "result");
        let _ = app.take_output();
        app.handle_key(&char_key('c'));
        let output = app.take_output();
        assert!(!output.is_empty(), "copy must reach terminal output");
        assert!(output[0].starts_with("\x1b]52;c;"));
    }

    #[test]
    fn shell_pointer_continue_enters_packages() {
        let mut app = mock_app();
        let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 30))
            .expect("test terminal");
        draw(&mut app, &mut terminal);
        click_id(&mut app, WidgetId::of("emc.s1.detail").sub("continue"));
        assert_eq!(app.screen_name(), "packages");
    }

    #[test]
    fn outer_group_scroll_reveals_focused_package() {
        let mut app = mock_app_many_groups();
        app.handle_key(&char_key('a'));
        app.handle_key(&key(KeyCode::Enter));
        let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 22))
            .expect("test terminal");
        draw(&mut app, &mut terminal);
        for _ in 0..5 {
            app.handle_key(&key(KeyCode::Down));
        }
        draw(&mut app, &mut terminal);
        assert!(
            buffer_text(&terminal).contains("pkg5"),
            "focused final group must be revealed by the outer ScrollView"
        );
    }

    #[test]
    fn module_wheel_reveals_later_environments() {
        let mut app = mock_app_many_groups();
        let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 22))
            .expect("test terminal");
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
            "wheel input must reveal the final environment"
        );
    }

    #[test]
    fn viewport_scrolling_anchors_during_new_output() {
        let mut app = mock_app();
        app.handle_key(&key(KeyCode::Enter));
        app.handle_key(&key(KeyCode::Enter));
        for seq in 1..=40 {
            app.feed_external(&External::new(crate::backend::ApplyLine {
                seq,
                stream: crate::backend::Stream::Stdout,
                text: format!("line {seq}"),
            }));
        }
        let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 30))
            .expect("test terminal");
        draw(&mut app, &mut terminal);
        app.handle_key(&key(KeyCode::Up));
        let Screen::Apply(owner) = &mut app.screen else {
            panic!("expected apply")
        };
        assert!(!owner.viewport.follow);
        let offset = owner.viewport.scroll.offset;
        let depth = owner.viewport.scrollback_depth();
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
        assert_eq!(owner.viewport.scrollback_depth(), depth + 1);
        app.handle_key(&key(KeyCode::End));
        let Screen::Apply(owner) = &app.screen else {
            panic!("expected apply")
        };
        assert!(owner.viewport.follow);
    }

    #[test]
    fn cancel_dialog_confirm_joins_worker_before_results() {
        let mut app = mock_app_with(
            MockBackend::new()
                .with_origin("brew", "jq", &["alpha"])
                .with_origin("brew", "rg", &["beta"])
                .with_apply_delay(std::time::Duration::from_secs(5)),
        );
        app.handle_key(&key(KeyCode::Enter));
        app.handle_key(&key(KeyCode::Enter));
        app.handle_key(&key(KeyCode::Esc));
        app.handle_key(&key(KeyCode::Right));
        app.handle_key(&key(KeyCode::Enter));
        assert!(matches!(&app.screen, Screen::Apply(owner) if owner.state.cancelling()));
        settle(&mut app);
        assert_eq!(app.screen_name(), "result");
    }

    #[test]
    fn failure_dialog_copies_rerun_and_close_reaches_result() {
        let mut app = mock_app_with(
            MockBackend::new()
                .with_origin("brew", "jq", &["alpha"])
                .with_origin("brew", "rg", &["beta"])
                .with_failure(crate::backend::ApplyPhase::Packages),
        );
        app.handle_key(&key(KeyCode::Enter));
        app.handle_key(&key(KeyCode::Enter));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while std::time::Instant::now() < deadline {
            app.pump();
            if matches!(&app.screen, Screen::Apply(owner) if owner.layer_open()) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(matches!(&app.screen, Screen::Apply(owner) if owner.layer_open()));
        let _ = app.take_output();
        app.handle_key(&char_key('y'));
        assert!(!app.take_output().is_empty());
        assert_eq!(app.screen_name(), "apply");
        app.handle_key(&key(KeyCode::Enter));
        assert_eq!(app.screen_name(), "result");
    }

    #[test]
    fn resize_redraws_static_chrome_at_new_bottom() {
        let mut app = mock_app();
        let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 30))
            .expect("test terminal");
        draw(&mut app, &mut terminal);
        terminal.backend_mut().resize(80, 24);
        assert_eq!(app.handle(Input::Resize(80, 24)), Outcome::Changed);
        draw(&mut app, &mut terminal);
        let text = buffer_text(&terminal);
        let last = text.lines().nth(23).expect("new bottom row");
        assert!(last.contains("S1 · search keeps selection"), "{last}");
    }
}
