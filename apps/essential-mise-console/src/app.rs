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
use crate::check::{self, CheckHandle};
use crate::screens::apply::{ApplyAction, ApplyOwner, ApplyScreen};
use crate::screens::modules::{ModuleAction, ModuleInfo, ModulesOwner, ModulesState};
use crate::screens::packages::{
    DEFAULT_ARM_DELAY, PackageAction, PackagesOwner, PackagesState, package_rows,
};
use crate::screens::result::{ResultAction, ResultOwner, ResultScreen};
use junie_tui::clock::{Moment, WallClock};
use junie_tui::core::event::{External, Input, Key, Outcome};
use junie_tui::core::focus::FocusChange;
use junie_tui::core::id::WidgetId;
use junie_tui::core::pointer::{PointerEvent, PointerHost, PointerParts, PointerRouter};
use junie_tui::core::response::Response;
use junie_tui::runtime::ExternalReceiver;
use junie_tui::shell::{App as ShellApp, PageKey, Shell};
use junie_tui::theme::{Charset, ColorLevel, Theme};
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::widgets::too_small::TooSmall;
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};

pub(crate) enum Screen {
    Modules(Box<ModulesOwner>),
    Packages(PackagesOwner),
    Apply(Box<ApplyOwner>),
    Result(Box<ResultOwner>),
}

impl Screen {
    /// Whether a text box has the keyboard.
    fn typing(&self) -> bool {
        matches!(self, Self::Modules(owner) if owner.typing())
    }

    /// The keys that work on this screen right now, for the help overlay.
    fn help_keys(&self) -> Vec<(&'static str, String)> {
        match self {
            Self::Modules(owner) => owner.hints(),
            Self::Packages(owner) => owner.hints(),
            Self::Apply(owner) => owner.hints(),
            Self::Result(owner) => owner.state.hints(),
        }
    }

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
    skipped: Vec<String>,
    check: Option<CheckHandle>,
    reapplying: bool,
    help_open: bool,
    arm_delay: Duration,
    shell: Shell<()>,
    clock: WallClock,
    tick: u64,
    exit_code: Option<u8>,
    last_error: Option<String>,
    theme: Theme,
}

impl App {
    /// Build from a backend and config dir. Reads the module catalogue (`#:` headers
    /// of the config files) and `miserc.local.toml` pre-ticks (both read-only).
    pub fn new(backend: Arc<dyn Backend>, config_dir: &Path) -> Result<Self, BackendError> {
        let manifest = crate::metadata::Manifest::load(config_dir)
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
            screen: Screen::Modules(Box::new(ModulesOwner::new(ModulesState::new(
                envs.clone(),
                &preticks,
            )))),
            envs,
            selected: preticks,
            skipped: Vec::new(),
            check: None,
            reapplying: false,
            help_open: false,
            arm_delay: std::env::var("EMC_REVIEW_ARM_MS")
                .ok()
                .and_then(|ms| ms.parse().ok())
                .map_or(DEFAULT_ARM_DELAY, Duration::from_millis),
            shell,
            clock: WallClock::new(),
            tick: 0,
            exit_code: None,
            last_error: None,
            theme: theme_for_env(),
        };
        app.refresh_module_facts();
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

    /// Lines for the normal terminal after the console closes: what happened
    /// and the commands to run next. Empty when nothing has run.
    pub fn exit_summary(&self) -> Vec<String> {
        match &self.screen {
            Screen::Result(owner) => owner.state.exit_lines(),
            Screen::Apply(owner) => {
                let mut lines: Vec<String> = owner.state.banner().into_iter().collect();
                if let Some(command) = owner.state.rerun() {
                    lines.push("To try again:".to_string());
                    lines.push(format!("  $ {command}"));
                }
                lines
            }
            _ => Vec::new(),
        }
    }

    /// The exit code that matches the current screen when the person leaves
    /// without choosing one: nothing has run on steps 1 and 2.
    fn leave_code(&self) -> u8 {
        match &self.screen {
            Screen::Modules(_) | Screen::Packages(_) => 0,
            Screen::Apply(_) => 1,
            Screen::Result(owner) => owner.state.exit_code(),
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
                self.exit_code = Some(self.leave_code());
                self.shell.quit = true;
                return Outcome::Changed;
            }
            return Outcome::Consumed;
        }
        if self.help_open {
            if key.is(ratatui::crossterm::event::KeyCode::Esc)
                || key.is(ratatui::crossterm::event::KeyCode::Enter)
                || key.is_char('?')
                || key.is_char('q')
            {
                self.help_open = false;
            }
            return Outcome::Changed;
        }
        if key.is_char('?') && !self.screen.typing() && !ShellApp::layer_open(self) {
            self.help_open = true;
            return Outcome::Changed;
        }
        let clipboard = self.shell.clipboard.clone();
        let out = match &mut self.screen {
            Screen::Modules(owner) => {
                let response = owner.key(key, &mut self.shell.focus, &self.shell.ring);
                self.modules_response(response)
            }
            Screen::Packages(owner) => {
                let response = owner.key(key);
                self.packages_response(response)
            }
            Screen::Apply(owner) => {
                let response = owner.key(key, &mut self.shell.focus, &self.shell.ring, &clipboard);
                self.apply_response(response)
            }
            Screen::Result(owner) => {
                let response = owner.key(key, &clipboard);
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
            Screen::Packages(_) => Outcome::Ignored,
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
                // Nothing has run yet: leaving here is not a failure.
                self.exit_code = Some(0);
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
            Some(PackageAction::Quit) => {
                // Nothing has run yet: leaving here is not a failure.
                self.exit_code = Some(0);
                self.shell.quit = true;
                Outcome::Changed
            }
            Some(PackageAction::None) | None => out,
        }
    }

    fn apply_response(&mut self, response: Response<ApplyAction>) -> Outcome {
        let (out, action) = response.into_parts();
        match action {
            Some(ApplyAction::Retry) => {
                self.retry_apply();
                Outcome::Changed
            }
            Some(ApplyAction::Quit) => {
                self.exit_code = Some(1);
                self.shell.quit = true;
                Outcome::Changed
            }
            Some(ApplyAction::None) | None => out,
        }
    }

    fn result_response(&mut self, response: Response<ResultAction>) -> Outcome {
        let (out, action) = response.into_parts();
        match action {
            Some(ResultAction::Quit(code)) => {
                self.exit_code = Some(code);
                self.shell.quit = true;
                Outcome::Changed
            }
            Some(ResultAction::Recheck) => {
                self.recheck();
                Outcome::Changed
            }
            Some(ResultAction::Reapply) => {
                self.start_reapply();
                Outcome::Changed
            }
            Some(ResultAction::Copy(_) | ResultAction::None) | None => out,
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
        let _ = self.poll_check();
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

    /// Success moves on to the Result screen by itself; a failure or a stop
    /// stays on the Applying screen with its banner.
    fn poll_apply(&mut self) -> Outcome {
        let outcome = match &mut self.screen {
            Screen::Apply(owner) => owner.state.try_join(),
            _ => None,
        };
        match outcome {
            Some(crate::backend::ApplyOutcome::Success) => {
                self.enter_result();
                Outcome::Changed
            }
            Some(_) => Outcome::Changed,
            None => Outcome::Ignored,
        }
    }

    /// Run the same request again (Try again after a failure or a stop).
    fn retry_apply(&mut self) {
        let Screen::Apply(owner) = &self.screen else {
            return;
        };
        let request = owner.state.request().clone();
        let mut state = ApplyScreen::new(self.backend.clone(), request);
        state.start();
        self.set_screen(Screen::Apply(Box::new(ApplyOwner::new(state))));
    }

    /// Read how many packages each module declares into the S1 detail pane.
    /// Reads config files only; never calls `mise`.
    fn refresh_module_facts(&mut self) {
        let origins = self
            .backend
            .origin_map(&self.config_dir)
            .unwrap_or_default();
        if let Screen::Modules(owner) = &mut self.screen {
            owner.state.set_package_counts(&origins);
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
                let state = PackagesState::new(rows, selected.len()).with_arm_delay(self.arm_delay);
                self.set_screen(Screen::Packages(PackagesOwner::new(state)));
                self.start_check(Vec::new());
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
        let (keep, skipped) = match &self.screen {
            Screen::Packages(owner) => (owner.state.install_specs(), owner.state.skipped_specs()),
            _ => return,
        };
        self.skipped = skipped;
        self.check = None;
        let req = ApplyRequest {
            envs: self.selected.clone(),
            keep,
            manager: None,
            skip_parts: Vec::new(),
            only_parts: Vec::new(),
            run_base: true,
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

    fn back_to_modules(&mut self) {
        self.check = None;
        self.set_screen(Screen::Modules(Box::new(ModulesOwner::new(
            ModulesState::new(self.envs.clone(), &self.selected),
        ))));
        self.refresh_module_facts();
    }

    fn enter_result(&mut self) {
        let mut screen = ResultScreen::new();
        if std::mem::take(&mut self.reapplying) {
            screen.mark_reapplied();
        }
        if let Screen::Apply(owner) = &self.screen {
            screen.set_log_follow_ups(crate::followup::from_log(
                owner.state.lines().map(|line| line.text.as_str()),
            ));
        }
        self.set_screen(Screen::Result(Box::new(ResultOwner::new(screen))));
        self.start_check(self.skipped.clone());
    }

    /// Apply again for exactly the areas that differ. The request is built
    /// by [`crate::reapply::plan`], the same one whose command the Result
    /// screen shows and copies.
    fn start_reapply(&mut self) {
        let Screen::Result(owner) = &self.screen else {
            return;
        };
        let Some(plan) = owner
            .state
            .report()
            .and_then(|report| crate::reapply::plan(report, &self.selected))
        else {
            return;
        };
        self.reapplying = true;
        let mut state = ApplyScreen::new(self.backend.clone(), plan.request);
        state.start();
        self.set_screen(Screen::Apply(Box::new(ApplyOwner::new(state))));
    }

    fn recheck(&mut self) {
        if let Screen::Result(owner) = &mut self.screen {
            owner.state.begin_check();
        }
        self.start_check(self.skipped.clone());
    }

    fn start_check(&mut self, skipped: Vec<String>) {
        self.check = Some(check::spawn(
            self.backend.clone(),
            self.selected.clone(),
            skipped,
        ));
    }

    /// Deliver a finished check to the Result screen.
    fn poll_check(&mut self) -> Outcome {
        let Some(result) = self.check.as_ref().and_then(CheckHandle::poll) else {
            return Outcome::Ignored;
        };
        self.check = None;
        let commands = match &result {
            Ok(report) => crate::reapply::plan(report, &self.selected)
                .map(|plan| crate::reapply::commands(&*self.backend, &plan.request))
                .unwrap_or_default(),
            Err(_) => Vec::new(),
        };
        match &mut self.screen {
            Screen::Result(owner) => {
                owner.state.finish_check(result);
                owner.state.set_reapply_commands(commands);
                Outcome::Changed
            }
            Screen::Packages(owner) => {
                owner.state.apply_check(result);
                Outcome::Changed
            }
            _ => Outcome::Ignored,
        }
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
        let mut out = PointerRouter::tick(self)
            .or(self.poll_apply())
            .or(self.poll_check());
        let now = self.clock.now_ms();
        if self.shell.pointer.expire_flash(now) {
            out = Outcome::Changed;
        }
        if self.shell.status.expire(Moment::from_millis(now)) {
            out = Outcome::Changed;
        }
        let animating = self.shell.pointer.has_flash()
            || matches!(&self.screen, Screen::Apply(owner) if owner.state.started() && owner.state.outcome().is_none())
            || matches!(&self.screen, Screen::Result(owner) if owner.state.is_checking())
            || matches!(&self.screen, Screen::Packages(owner) if owner.state.is_checking());
        if animating {
            self.tick = self.tick.wrapping_add(1);
            out = Outcome::Changed;
        }
        out
    }
}

/// Theme from the environment: `NO_COLOR` and `TERM=dumb` select the mono
/// level, and `TERM=dumb` (or `JUNIE_TUI_GLYPHS=ascii`) the ASCII glyph set,
/// so every state is still told apart by glyph and word.
fn theme_for_env() -> Theme {
    theme_from(|key| std::env::var(key).ok())
}

pub(crate) fn theme_from(get: impl Fn(&str) -> Option<String>) -> Theme {
    let set = |key: &str| get(key).is_some_and(|v| !v.is_empty());
    let dumb = get("TERM").is_some_and(|t| t == "dumb");
    let level = if set("NO_COLOR") || dumb {
        ColorLevel::Mono
    } else if matches!(get("COLORTERM").as_deref(), Some("truecolor" | "24bit")) {
        ColorLevel::TrueColor
    } else {
        ColorLevel::Ansi256
    };
    Charset::from_env(&get, cfg!(windows)).apply(Theme::for_level(level))
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
        if self.help_open {
            let theme = *ctx.theme;
            crate::screens::help::render(area, buf, &theme, theme.canvas, &self.screen.help_keys());
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
        match &mut self.screen {
            Screen::Modules(owner) => owner.blur(change),
            Screen::Packages(_) => Outcome::Ignored,
            Screen::Apply(_) => Outcome::Ignored,
            Screen::Result(_) => Outcome::Ignored,
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
            Screen::Packages(_) => Response::ignored(),
            Screen::Apply(owner) => {
                let response = owner.pointer(ev, &mut self.shell.focus, &clipboard);
                self.apply_response(response).into()
            }
            Screen::Result(_) => {
                let _ = ev;
                Response::ignored()
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
mod tests;
