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
use crate::screens::checkpoint::{CheckpointAction, CheckpointOwner, CheckpointScreen};
use crate::screens::modules::{ModuleAction, ModuleInfo, ModulesOwner, ModulesState};
use crate::screens::packages::{
    DEFAULT_ARM_DELAY, PackageAction, PackagesOwner, PackagesState, package_rows,
};
use crate::screens::result::{ResultAction, ResultOwner, ResultScreen};
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use termrock::clock::{Moment, WallClock};
use termrock::core::event::{External, Input, Key, Outcome};
use termrock::core::focus::FocusChange;
use termrock::core::id::WidgetId;
use termrock::core::pointer::{PointerEvent, PointerHost, PointerParts, PointerRouter};
use termrock::core::response::Response;
use termrock::runtime::ExternalReceiver;
use termrock::shell::{App as ShellApp, PageKey, Shell};
use termrock::theme::{Charset, ColorLevel, Theme};
use termrock::ui::ctx::RenderCtx;
use termrock::widgets::too_small::TooSmall;

pub(crate) enum Screen {
    Modules(Box<ModulesOwner>),
    Packages(PackagesOwner),
    Apply(Box<ApplyOwner>),
    Checkpoint(Box<CheckpointOwner>),
    Result(Box<ResultOwner>),
}

struct PackageRecovery {
    request: ApplyRequest,
    exit_summary: Vec<String>,
}

impl Screen {
    /// Whether a text box has the keyboard.
    fn typing(&self) -> bool {
        match self {
            Self::Modules(owner) => owner.typing(),
            Self::Packages(_) | Self::Apply(_) | Self::Checkpoint(_) | Self::Result(_) => false,
        }
    }

    /// The keys that work on this screen right now, for the help overlay.
    fn help_keys(&self) -> Vec<(&'static str, String)> {
        match self {
            Self::Modules(owner) => owner.hints(),
            Self::Packages(owner) => owner.hints(),
            Self::Apply(owner) => owner.hints(),
            Self::Checkpoint(owner) => owner.hints(),
            Self::Result(owner) => owner.state.hints(),
        }
    }

    fn primary_focus(&self) -> Option<WidgetId> {
        match self {
            Self::Modules(owner) => Some(owner.primary_focus()),
            Self::Packages(owner) => Some(owner.primary_focus()),
            Self::Apply(owner) => Some(owner.primary_focus()),
            Self::Checkpoint(owner) => Some(owner.primary_focus()),
            Self::Result(owner) => Some(owner.primary_focus()),
        }
    }

    fn page_key(&self) -> PageKey {
        match self {
            Self::Modules(_) => PageKey::of("emc.modules"),
            Self::Packages(_) => PageKey::of("emc.packages"),
            Self::Apply(_) => PageKey::of("emc.apply"),
            Self::Checkpoint(_) => PageKey::of("emc.checkpoint"),
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
    package_recovery: Option<PackageRecovery>,
    apply_follow_ups: Vec<crate::followup::FollowUp>,
    check: Option<CheckHandle>,
    reapplying: bool,
    authentication_pending: bool,
    help: termrock::ui::modal::ModalStack<termrock::widgets::help::HelpOverlay>,
    arm_delay: Duration,
    shell: Shell<()>,
    _clipboard_hook: termrock::shell::ClipboardHook,
    clock: WallClock,
    tick: u64,
    exit_code: Option<u8>,
    last_error: Option<String>,
    theme: Theme,
}

impl App {
    /// Start a new run with every module in the config catalogue selected.
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
        let selected: Vec<_> = envs.iter().map(|module| module.name.clone()).collect();
        let shell = Shell::new();
        let clipboard_hook = shell.clipboard().install();
        let mut app = Self {
            backend,
            config_dir: config_dir.to_path_buf(),
            screen: Screen::Modules(Box::new(ModulesOwner::new(ModulesState::new(
                envs.clone(),
                &selected,
            )))),
            envs,
            selected,
            skipped: Vec::new(),
            package_recovery: None,
            apply_follow_ups: Vec::new(),
            check: None,
            reapplying: false,
            authentication_pending: false,
            help: termrock::ui::modal::ModalStack::new(),
            arm_delay: std::env::var("EMC_REVIEW_ARM_MS")
                .ok()
                .and_then(|ms| ms.parse().ok())
                .map_or(DEFAULT_ARM_DELAY, Duration::from_millis),
            shell,
            _clipboard_hook: clipboard_hook,
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
            Screen::Checkpoint(_) => "checkpoint",
            Screen::Result(_) => "result",
        }
    }

    /// Lines for the normal terminal after the console closes: what happened
    /// and the commands to run next. Empty when nothing has run.
    pub fn exit_summary(&self) -> Vec<String> {
        match &self.screen {
            Screen::Result(owner) => owner.state.exit_lines(),
            Screen::Apply(owner) => owner.state.exit_summary(),
            Screen::Checkpoint(owner) => owner.state.exit_lines(),
            Screen::Packages(_) => self
                .package_recovery
                .as_ref()
                .map(|recovery| recovery.exit_summary.clone())
                .unwrap_or_default(),
            Screen::Modules(_) => Vec::new(),
        }
    }

    /// The exit code that matches the current screen when the person leaves
    /// without choosing one: nothing has run on steps 1 and 2.
    fn leave_code(&self) -> u8 {
        match &self.screen {
            Screen::Packages(_) if self.package_recovery.is_some() => 1,
            Screen::Modules(_) | Screen::Packages(_) => 0,
            Screen::Apply(_) => 1,
            Screen::Checkpoint(_) => crate::cli::EXIT_PARTIAL,
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
        if let Some(help) = self.help.top_mut() {
            let out = help.on_key(key);
            if help.closed {
                self.help.pop(&mut self.shell.focus);
            }
            return out;
        }
        if key.is_char('?') && !self.screen.typing() && !ShellApp::layer_open(self) {
            let help = crate::screens::help::overlay(&self.screen.help_keys());
            let initial = help.done.id;
            self.help.push(help, &mut self.shell.focus, Some(initial));
            return Outcome::Changed;
        }
        let clipboard = self.shell.clipboard().clone();
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
                let response = owner.key(key, &mut self.shell.focus, &clipboard);
                self.apply_response(response)
            }
            Screen::Checkpoint(owner) => {
                let response = owner.key(key, &mut self.shell.focus);
                self.checkpoint_response(response)
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
            Screen::Checkpoint(_) | Screen::Result(_) => Outcome::Ignored,
        }
    }

    fn modules_response(&mut self, response: Response<ModuleAction>) -> Outcome {
        let (out, actions) = response.into_parts();
        actions.into_iter().fold(out, |out, action| match action {
            ModuleAction::Confirm => {
                self.enter_packages();
                Outcome::Changed
            }
            ModuleAction::Quit => {
                // Nothing has run yet: leaving here is not a failure.
                self.exit_code = Some(0);
                self.shell.quit = true;
                Outcome::Changed
            }
            ModuleAction::None => out,
        })
    }

    fn packages_response(&mut self, response: Response<PackageAction>) -> Outcome {
        let (out, actions) = response.into_parts();
        actions.into_iter().fold(out, |out, action| match action {
            PackageAction::Apply => {
                self.enter_apply();
                Outcome::Changed
            }
            PackageAction::Back => {
                self.back_to_modules();
                Outcome::Changed
            }
            PackageAction::Quit => {
                self.exit_code = Some(self.leave_code());
                self.shell.quit = true;
                Outcome::Changed
            }
            PackageAction::None => out,
        })
    }

    fn apply_response(&mut self, response: Response<ApplyAction>) -> Outcome {
        let (out, actions) = response.into_parts();
        actions.into_iter().fold(out, |out, action| match action {
            ApplyAction::Retry => {
                self.retry_apply();
                Outcome::Changed
            }
            ApplyAction::ReviewPackages => {
                self.review_failed_packages();
                Outcome::Changed
            }
            ApplyAction::Quit => {
                self.exit_code = Some(1);
                self.shell.quit = true;
                Outcome::Changed
            }
            ApplyAction::None => out,
        })
    }

    fn result_response(&mut self, response: Response<ResultAction>) -> Outcome {
        let (out, actions) = response.into_parts();
        actions.into_iter().fold(out, |out, action| match action {
            ResultAction::Quit(code) => {
                self.exit_code = Some(code);
                self.shell.quit = true;
                Outcome::Changed
            }
            ResultAction::Recheck => {
                self.recheck();
                Outcome::Changed
            }
            ResultAction::Reapply => {
                self.start_reapply();
                Outcome::Changed
            }
            ResultAction::Copy(_) | ResultAction::None => out,
        })
    }

    fn checkpoint_response(&mut self, response: Response<CheckpointAction>) -> Outcome {
        let (out, actions) = response.into_parts();
        actions.into_iter().fold(out, |_out, action| match action {
            CheckpointAction::Quit => {
                self.exit_code = Some(crate::cli::EXIT_PARTIAL);
                self.shell.quit = true;
                Outcome::Changed
            }
        })
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
        let _ = self.poll_checkpoint();
    }

    fn feed_external(&mut self, external: &External) -> bool {
        match &mut self.screen {
            Screen::Apply(owner) => owner.on_external(external),
            Screen::Modules(_)
            | Screen::Packages(_)
            | Screen::Checkpoint(_)
            | Screen::Result(_) => false,
        }
    }

    /// Success moves on by itself. A recorded checkpoint is checked before
    /// Result. A failure or a stop stays on the Applying screen.
    fn poll_apply(&mut self) -> Outcome {
        let (update, record) = {
            let Screen::Apply(owner) = &mut self.screen else {
                return Outcome::Ignored;
            };
            let (update, outcome) = owner.poll_worker();
            let mut record = None;
            if let Some(outcome) = outcome {
                for item in owner.state.take_follow_ups() {
                    if !self.apply_follow_ups.contains(&item) {
                        self.apply_follow_ups.push(item);
                    }
                }
                if outcome == crate::backend::ApplyOutcome::Success {
                    record = Some(owner.state.take_checkpoint());
                }
            }
            (update, record)
        };
        if let Some(record) = record {
            if matches!(
                crate::checkpoint::plan(&record),
                crate::checkpoint::Plan::Clear
            ) {
                self.enter_result();
            } else {
                self.enter_checkpoint(record);
            }
            return Outcome::Changed;
        }
        update
    }

    fn enter_checkpoint(&mut self, record: crate::checkpoint::Record) {
        let mut state = CheckpointScreen::new(crate::google_accounts::default_db_path(), record);
        state.start();
        self.check = None;
        self.set_screen(Screen::Checkpoint(Box::new(CheckpointOwner::new(state))));
    }

    fn poll_checkpoint(&mut self) -> Outcome {
        let satisfied = {
            let Screen::Checkpoint(owner) = &mut self.screen else {
                return Outcome::Ignored;
            };
            let Some(output) = owner.state.poll() else {
                return Outcome::Ignored;
            };
            owner.state.finish_job(output)
        };
        if satisfied {
            self.enter_result();
        }
        Outcome::Changed
    }

    fn retry_apply(&mut self) {
        let Screen::Apply(owner) = &self.screen else {
            return;
        };
        let request = owner.state.retry_request();
        self.prepare_apply(request);
    }

    fn review_failed_packages(&mut self) {
        if let Screen::Apply(owner) = &self.screen
            && owner.state.can_review_packages()
        {
            self.open_package_review(Some(PackageRecovery {
                request: owner.state.retry_request(),
                exit_summary: owner.state.exit_summary(),
            }));
        }
    }

    fn prepare_apply(&mut self, request: ApplyRequest) {
        let mut state = ApplyScreen::new(self.backend.clone(), request);
        self.authentication_pending = self.backend.requires_authentication();
        if self.authentication_pending {
            self.shell.quit = true;
        } else {
            state.start();
        }
        self.set_screen(Screen::Apply(Box::new(ApplyOwner::new(state))));
    }

    /// The runtime must restore the terminal before asking sudo to authenticate.
    pub fn authentication_pending(&self) -> bool {
        self.authentication_pending
    }

    /// Resume the existing selection after the terminal-owned sudo prompt.
    pub fn finish_authentication(&mut self, result: Result<(), BackendError>) {
        self.authentication_pending = false;
        self.shell.quit = false;
        if let Screen::Apply(owner) = &mut self.screen {
            match result {
                Ok(()) => owner.state.start(),
                Err(error) => owner.state.authentication_failed(error.to_string()),
            }
        }
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
        self.selected = selected;
        self.open_package_review(None);
    }

    fn open_package_review(&mut self, recovery: Option<PackageRecovery>) {
        let selected = &self.selected;
        match self.backend.origin_map(&self.config_dir) {
            Ok(origins) => {
                let mut rows = package_rows(selected, &Self::filter_origins(&origins, selected));
                if let Some(recovery) = &recovery {
                    rows.retain(|row| {
                        let spec = row.spec();
                        recovery.request.keep.contains(&spec)
                            || (!self.reapplying && self.skipped.contains(&spec))
                    });
                }
                let mut state =
                    PackagesState::new(rows, selected.len()).with_arm_delay(self.arm_delay);
                if recovery.is_some() {
                    state.resume_packages(&self.skipped);
                }
                self.package_recovery = recovery;
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
        if self.package_recovery.is_some() && self.reapplying {
            self.skipped.extend(skipped);
        } else {
            self.skipped = skipped;
        }
        self.check = None;
        if let Some(recovery) = self.package_recovery.take() {
            if keep.is_empty() {
                self.enter_result();
                return;
            }
            self.prepare_apply(ApplyRequest {
                keep,
                ..recovery.request
            });
            return;
        }
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
        self.prepare_apply(req);
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
        self.package_recovery = None;
        self.apply_follow_ups.clear();
        self.reapplying = false;
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
        screen.set_log_follow_ups(self.apply_follow_ups.clone());
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
        self.prepare_apply(plan.request);
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
            .or(self.poll_check())
            .or(self.poll_checkpoint());
        let now = self.clock.now_ms();
        if self.shell.pointer.expire_flash(now) {
            out = Outcome::Changed;
        }
        if self.shell.status.expire(Moment::from_millis(now)) {
            out = Outcome::Changed;
        }
        let animating = self.shell.pointer.has_flash()
            || matches!(&self.screen, Screen::Apply(owner) if owner.state.started() && owner.state.outcome().is_none())
            || matches!(&self.screen, Screen::Checkpoint(owner) if owner.state.checking())
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
/// level, and `TERM=dumb` (or `TERMROCK_GLYPHS=ascii`) the ASCII glyph set,
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
    let charset_env = |key: &str| {
        get(key).or_else(|| {
            (key == Charset::OVERRIDE_ENV)
                .then(|| get("JUNIE_TUI_GLYPHS"))
                .flatten()
        })
    };
    Charset::from_env(charset_env, cfg!(windows)).apply(Theme::for_level(level))
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
            Screen::Checkpoint(owner) => owner.draw(area, buf, ctx),
            Screen::Result(owner) => owner.draw(area, buf, ctx),
        }
        if let Some(help) = self.help.top_mut() {
            help.render(area, buf, ctx);
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
            Screen::Apply(_) | Screen::Checkpoint(_) | Screen::Result(_) => Outcome::Ignored,
        }
    }

    fn layer_open(&self) -> bool {
        !self.help.is_empty()
            || matches!(&self.screen, Screen::Apply(owner) if owner.layer_open())
            || matches!(&self.screen, Screen::Checkpoint(owner) if owner.layer_open())
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
            Screen::Modules(_)
            | Screen::Packages(_)
            | Screen::Checkpoint(_)
            | Screen::Result(_) => None,
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
        if let Some(help) = self.help.top_mut() {
            use termrock::core::pointer::MouseTarget;
            let response = help.route(ev).unwrap_or_else(Response::consumed);
            if help.closed {
                self.help.pop(&mut self.shell.focus);
            }
            return response;
        }
        if ev.on(crate::screens::paint::PRIMARY) && ev.is(termrock::core::intent::Phase::Click) {
            if let Screen::Modules(owner) = &mut self.screen {
                let action = owner
                    .state
                    .apply(crate::screens::modules::ModuleKey::Confirm);
                return self.modules_response(Response::emit(action)).into();
            }
            let code = if matches!(self.screen, Screen::Apply(ref owner) if owner.state.outcome().is_none())
            {
                ratatui::crossterm::event::KeyCode::Esc
            } else {
                ratatui::crossterm::event::KeyCode::Enter
            };
            return self
                .route_key(&Key {
                    code,
                    mods: ratatui::crossterm::event::KeyModifiers::NONE,
                })
                .into();
        }
        let clipboard = self.shell.clipboard().clone();
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
            Screen::Checkpoint(owner) => {
                let response = owner.pointer(ev, &mut self.shell.focus);
                self.checkpoint_response(response).into()
            }
            Screen::Result(_) => {
                let _ = ev;
                Response::ignored()
            }
        }
    }

    fn outside(&mut self, _pos: Position) -> Response<()> {
        if !self.help.is_empty() {
            self.help.pop(&mut self.shell.focus);
            return Response::changed();
        }
        let clipboard = self.shell.clipboard().clone();
        match &mut self.screen {
            Screen::Apply(owner) if owner.layer_open() => {
                let response = owner.outside(&mut self.shell.focus, &clipboard);
                self.apply_response(response).into()
            }
            Screen::Checkpoint(owner) if owner.layer_open() => {
                let response = owner.outside(&mut self.shell.focus);
                self.checkpoint_response(response).into()
            }
            Screen::Modules(_)
            | Screen::Packages(_)
            | Screen::Apply(_)
            | Screen::Checkpoint(_)
            | Screen::Result(_) => Response::changed(),
        }
    }
}

#[cfg(test)]
mod tests;
