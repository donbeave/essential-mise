//! Console app: screen routing and the external-event pump.
//!
//! The app owns one screen at a time (S1→S2→S3→S4, with `Back` restoring
//! the previous selection state) plus the apply event receiver. Key routing
//! mirrors `app.rs::handle_input`: `Input::External` drains through
//! [`App::pump`](crate::app::App::pump) exactly like the `external.rs` test pump (drain, feed each
//! event, collect outcomes). Painting lands below in `render_*` (T4.6
//! harness); the state machine here is fully headless-testable.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use crate::backend::{ApplyRequest, Backend, BackendError, ResId};
use crate::screens::apply::{ApplyAction, ApplyScreen};
use crate::screens::modules::{ModuleAction, ModuleInfo, ModulesState};
use crate::screens::packages::{PackageAction, PackagesState, package_rows};
use crate::screens::result::{ResultAction, ResultScreen};
use junie_tui::core::event::{Input, Outcome};
use junie_tui::core::focus::FocusRing;
use junie_tui::core::hit::HitRegistry;
use junie_tui::shell::clipboard::Clipboard;
use junie_tui::theme::{ButtonKind, ColorLevel, Theme, Tone};
use junie_tui::ui::ctx::{Interaction, RenderCtx, fill};
use junie_tui::ui::layout::Split;
use junie_tui::widgets::button::Button;
use junie_tui::widgets::grid::{GridRows, RowTotal};
use junie_tui::widgets::hintbar::HintLayer;
use junie_tui::widgets::info_dialog::InfoDialog;
use junie_tui::widgets::input::TextInput;
use junie_tui::widgets::keyhint::hint;
use junie_tui::widgets::list::{ListBox, ListItem, SelectMode};
use junie_tui::widgets::list_detail::{
    Action as DetailAction, Frame as DetailFrame, Line as DetailLine, ListDetail,
};
use junie_tui::widgets::page_header::PageHeader;
use junie_tui::widgets::panel::Panel;
use junie_tui::widgets::progress::ProgressBar;
use junie_tui::widgets::props;
use junie_tui::widgets::statusbar::{StatusBar, StatusItem};
use junie_tui::widgets::steps::{Step, StepRail};
use junie_tui::widgets::viewport::{Span as ViewportSpan, TextViewport};
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

/// Console screens in journey order.
pub enum Screen {
    /// Module multi-select.
    Modules(ModulesState),
    /// Per-package checklist.
    Packages(PackagesState),
    /// Live two-phase apply.
    Apply(ApplyScreen),
    /// Verify screen.
    Result(ResultScreen),
}

/// Console app: routing, selection memory, and the apply event pump.
pub struct App {
    backend: Arc<dyn Backend>,
    config_dir: std::path::PathBuf,
    screen: Screen,
    envs: Vec<ModuleInfo>,
    selected: Vec<String>,
    packages_snapshot: Option<PackagesState>,
    quit: bool,
    exit_code: Option<u8>,
    last_error: Option<String>,
    theme: junie_tui::theme::Theme,
}

impl App {
    /// Build from a backend and config dir. Reads `bootstrap.sh` envs and
    /// `miserc.local.toml` pre-ticks (both read-only).
    pub fn new(backend: Arc<dyn Backend>, config_dir: &Path) -> Result<Self, BackendError> {
        let text = std::fs::read_to_string(config_dir.join("bootstrap.sh"))
            .map_err(BackendError::Spawn)?;
        let envs = crate::screens::modules::parse_environments(&text);
        let preticks =
            crate::screens::modules::read_preticks(&config_dir.join("miserc.local.toml"));
        let pretick_names: Vec<String> = preticks;
        let mut app = Self {
            backend,
            config_dir: config_dir.to_path_buf(),
            screen: Screen::Modules(ModulesState::new(envs.clone(), &pretick_names)),
            envs,
            selected: pretick_names,
            packages_snapshot: None,
            quit: false,
            exit_code: None,
            last_error: None,
            theme: theme_for_env(),
        };
        // Best-effort badges; failures keep the pending defaults.
        app.refresh_badges();
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
        self.quit
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
    pub fn handle_key(&mut self, key: &junie_tui::core::event::Key) {
        let clipboard = Clipboard::new();
        match &mut self.screen {
            Screen::Modules(state) => {
                if let Some(mapped) = state.map_key(key) {
                    match state.apply(mapped) {
                        ModuleAction::None => {}
                        ModuleAction::Confirm => self.enter_packages(),
                        ModuleAction::Quit => {
                            self.quit = true;
                        }
                    }
                }
            }
            Screen::Packages(state) => {
                if let Some(mapped) = state.map_key(key) {
                    match state.apply(mapped) {
                        PackageAction::None => {}
                        PackageAction::Apply => self.enter_apply(),
                        PackageAction::Back => self.back_to_modules(),
                    }
                }
            }
            Screen::Apply(state) => {
                if let Some(mapped) = state.map_key(key) {
                    match state.apply(mapped, &clipboard) {
                        ApplyAction::None => {}
                        ApplyAction::Cancelled => self.enter_result(),
                        ApplyAction::Done => self.enter_result(),
                    }
                }
            }
            Screen::Result(state) => {
                if let Some(mapped) = state.map_key(key) {
                    match state.apply(mapped, &*self.backend) {
                        ResultAction::None => {}
                        ResultAction::Back => self.back_to_packages(),
                        ResultAction::Done => {
                            self.exit_code = Some(0);
                            self.quit = true;
                        }
                    }
                }
            }
        }
    }

    /// Drain the S3 worker channel into the screen (no-op elsewhere).
    /// Mirrors the `external.rs` pump: drain, feed each, collect.
    pub fn pump(&mut self) {
        let mut events = Vec::new();
        if let Screen::Apply(state) = &self.screen {
            events.extend(state.drain());
        }
        for m in &events {
            self.feed_external(m);
        }
        // Poll completion even when no new lines arrived. Failed workers
        // stay on S3 with the modal; success/cancel auto-advance to S4.
        if let Screen::Apply(state) = &mut self.screen {
            match state.try_join() {
                Some(crate::backend::ApplyOutcome::Failed { .. }) | None => {}
                Some(_) => self.enter_result(),
            }
        }
    }

    /// Refresh S1 badges from live plan/status (all known envs).
    /// Backend failures keep the pending defaults; never panics.
    pub fn refresh_badges(&mut self) {
        let all: Vec<String> = match &self.screen {
            Screen::Modules(state) => state.envs.iter().map(|e| e.name.clone()).collect(),
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
        if let Screen::Modules(state) = &mut self.screen {
            state.refresh_badges(&plan, &origins, parts_ok);
        }
    }

    /// S1 → S2: build the checklist for the confirmed selection.
    fn enter_packages(&mut self) {
        let selected = match &self.screen {
            Screen::Modules(state) => state.selected_envs(),
            _ => return,
        };
        self.selected = selected.clone();
        match self.backend.origin_map(&self.config_dir) {
            Ok(origins) => {
                let rows = package_rows(&selected, &Self::filter_origins(&origins, &selected));
                self.screen = Screen::Packages(PackagesState::new(rows));
            }
            Err(e) => {
                self.last_error = Some(format!("origin_map failed: {e}"));
            }
        }
    }

    /// Keep only declarations for the selected envs.
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

    /// S2 → S3: snapshot the checklist; the worker starts on `Enter`.
    fn enter_apply(&mut self) {
        let (keep, envs) = match &self.screen {
            Screen::Packages(state) => (state.kept_specs(), self.selected.clone()),
            _ => return,
        };
        self.packages_snapshot = match &self.screen {
            Screen::Packages(state) => Some(state.clone()),
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
        self.screen = Screen::Apply(ApplyScreen::new(self.backend.clone(), req));
    }

    /// Jump directly to a screen (`tui [screen]`; S3 opens idle).
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

    /// S3/S4 → S2: restore the snapshotted checklist (selection preserved).
    fn back_to_packages(&mut self) {
        if let Some(state) = self.packages_snapshot.clone() {
            self.screen = Screen::Packages(state);
        }
    }

    /// S2 → S1: rebuild modules with the confirmed selection.
    fn back_to_modules(&mut self) {
        self.screen = Screen::Modules(ModulesState::new(self.envs.clone(), &self.selected));
        self.refresh_badges();
    }

    /// S3/S4 entry: refresh the verify screen (auto `status` on entry).
    fn enter_result(&mut self) {
        let backend = self.backend.clone();
        let envs = self.selected.clone();
        let keep = match &self.screen {
            Screen::Packages(state) => state.kept_specs(),
            Screen::Apply(_) => self
                .packages_snapshot
                .as_ref()
                .map(|s| s.kept_specs())
                .unwrap_or_default(),
            _ => Vec::new(),
        };
        let mut screen = ResultScreen::new(backend.clone(), envs, keep);
        screen.refresh(&*backend);
        self.screen = Screen::Result(screen);
    }
}

// ---------------------------------------------------------------------------
// Rendering (T4.6 harness): immediate retained-mode paint with a fresh
// context per frame. Widgets are constructed from screen state, rendered,
// and dropped; widget-internal cursor/scroll persistence is P2 polish.

/// Theme from the environment: truecolor when the terminal says so.
fn theme_for_env() -> Theme {
    if std::env::var("COLORTERM").as_deref() == Ok("truecolor") {
        Theme::for_level(ColorLevel::TrueColor)
    } else {
        Theme::for_level(ColorLevel::Ansi256)
    }
}

/// Feed one external event; true when a screen consumed it. Shared by the
/// runtime loop (`Application::handle`) and the in-process test pump.
impl App {
    fn feed_external(&mut self, external: &junie_tui::core::event::External) -> bool {
        use crate::backend::ApplyOutcome;
        if let Screen::Apply(state) = &mut self.screen
            && state.on_external(external)
        {
            // Auto-advance on terminal success/cancel; a FAILED worker stays
            // on S3 with the modal open until the user continues.
            if let Some(outcome) = state.try_join() {
                match outcome {
                    ApplyOutcome::Failed { .. } => {}
                    _ => self.enter_result(),
                }
            }
            return true;
        }
        false
    }
}

impl junie_tui::runtime::Application for App {
    fn handle(&mut self, input: Input) -> Outcome {
        use crate::backend::ApplyOutcome;
        match input {
            Input::Key(key) => {
                self.handle_key(&key);
                Outcome::Changed
            }
            Input::External(external) => {
                if self.feed_external(&external) {
                    Outcome::Changed
                } else {
                    Outcome::Ignored
                }
            }
            Input::Tick => {
                // Poll worker completion even with no new lines, so the
                // failure modal opens and finished applies advance.
                if let Screen::Apply(state) = &mut self.screen {
                    match state.try_join() {
                        Some(ApplyOutcome::Failed { .. }) => Outcome::Changed,
                        Some(_) => {
                            self.enter_result();
                            Outcome::Changed
                        }
                        None => Outcome::Ignored,
                    }
                } else {
                    Outcome::Ignored
                }
            }
            _ => Outcome::Ignored,
        }
    }

    fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();
        let buf = frame.buffer_mut();
        let mut hits = HitRegistry::default();
        let mut ring = FocusRing::default();
        let interaction = Interaction {
            hover_suppressed: true,
            ..Default::default()
        };
        let mut ctx = RenderCtx::new(&self.theme, interaction, &mut hits, &mut ring);
        fill(buf, area, self.theme.base());
        let theme = self.theme;
        match &mut self.screen {
            Screen::Modules(state) => render_modules(state, area, buf, &mut ctx, &theme),
            Screen::Packages(state) => render_packages(state, area, buf, &mut ctx, &theme),
            Screen::Apply(state) => render_apply(state, area, buf, &mut ctx, &theme),
            Screen::Result(state) => render_result(state, area, buf, &mut ctx, &theme),
        }
    }

    fn should_quit(&self) -> bool {
        self.quit
    }

    fn tick_interval(&self) -> std::time::Duration {
        std::time::Duration::from_millis(100)
    }

    fn external(&self) -> Option<&junie_tui::runtime::ExternalReceiver> {
        match &self.screen {
            Screen::Apply(state) => state.receiver(),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Painting: immediate retained-mode render with a fresh context per frame.

fn paint_header(buf: &mut Buffer, theme: &Theme, area: Rect, step: &str) {
    let header = PageHeader::new(crate::screens::chrome::TITLE).right(step);
    header.render(area, buf, theme);
}

fn paint_hints(
    buf: &mut Buffer,
    theme: &Theme,
    area: Rect,
    hints: &[(&'static str, &'static str)],
) {
    use junie_tui::widgets::hintbar::HintBar;
    let layer = HintLayer::new(hints.iter().map(|(k, a)| hint(k, a)).collect());
    HintBar::render_at(area, buf, theme, &layer);
}

fn paint_status(buf: &mut Buffer, ctx: &mut RenderCtx, area: Rect, left: &str) {
    let mut bar = StatusBar::new();
    bar.left = vec![StatusItem::new(left, Tone::Muted)];
    bar.render(area, buf, ctx);
}

fn render_modules(
    state: &crate::screens::modules::ModulesState,
    area: Rect,
    buf: &mut Buffer,
    ctx: &mut RenderCtx,
    theme: &Theme,
) {
    let bg: Color = theme.canvas;
    paint_header(
        buf,
        theme,
        Rect::new(area.x, area.y, area.width, 1),
        "1/4 Modules",
    );
    let body = Rect::new(
        area.x,
        area.y + 1,
        area.width,
        area.height.saturating_sub(3),
    );
    let mut input = TextInput::new(junie_tui::core::id::WidgetId::of("emc.s1.filter"), "Filter");
    input.field.set_text(state.query());
    let input_area = Rect::new(body.x, body.y, body.width, 3.min(body.height));
    input.render(input_area, buf, ctx, bg);
    let list_area = Rect::new(
        body.x,
        body.y + 3.min(body.height),
        body.width,
        body.height.saturating_sub(3),
    );
    let rows = state.master_rows();
    let (visible, total, selected) = state.counts();
    let mut list = ListDetail::new(
        junie_tui::core::id::WidgetId::of("emc.s1.master"),
        junie_tui::core::id::WidgetId::of("emc.s1.detail"),
        Split::new(45, 20, 20),
    );
    list.select(state.cursor());
    list.clamp(rows.len());
    let cursor_name = state
        .cursor_env()
        .map(|e| e.name.clone())
        .unwrap_or_default();
    let lines: Vec<DetailLine> = state
        .detail_props()
        .into_iter()
        .map(|p| DetailLine::Prop(p.label, p.value, p.tone))
        .collect();
    let frame = DetailFrame {
        title: "environments",
        meta: format!("showing {visible} of {total} · {selected} selected"),
        head: vec![],
        rows: &rows,
        under: None,
        detail_title: cursor_name,
        detail_meta: state
            .cursor_env()
            .map(|e| state.badge(&e.name).label().to_string())
            .unwrap_or_default(),
        lines,
        actions: vec![
            DetailAction::new("continue", "Continue →", ButtonKind::Primary)
                .disabled(!state.can_continue()),
        ],
    };
    list.render(list_area, buf, ctx, &frame);
    let hints_area = Rect::new(area.x, area.bottom().saturating_sub(2), area.width, 1);
    paint_hints(
        buf,
        theme,
        hints_area,
        &[
            ("↑↓", "move"),
            ("Space", "toggle"),
            ("/", "filter"),
            ("a", "all"),
            ("n", "none"),
            ("Enter", "continue"),
            ("Esc", "quit"),
        ],
    );
    let status_area = Rect::new(area.x, area.bottom().saturating_sub(1), area.width, 1);
    paint_status(buf, ctx, status_area, "S1 · search keeps selection");
}

fn render_packages(
    state: &crate::screens::packages::PackagesState,
    area: Rect,
    buf: &mut Buffer,
    ctx: &mut RenderCtx,
    theme: &Theme,
) {
    let bg: Color = theme.canvas;
    paint_header(
        buf,
        theme,
        Rect::new(area.x, area.y, area.width, 1),
        "2/4 Packages",
    );
    let body = Rect::new(
        area.x,
        area.y + 1,
        area.width,
        area.height.saturating_sub(3),
    );
    let mut input = TextInput::new(junie_tui::core::id::WidgetId::of("emc.s2.filter"), "Filter");
    input.field.set_text(state.query());
    let input_area = Rect::new(body.x, body.y, body.width, 3.min(body.height));
    input.render(input_area, buf, ctx, bg);
    let mut rest = Rect::new(
        body.x,
        body.y + 3.min(body.height),
        body.width,
        body.height.saturating_sub(3),
    );
    let groups = state.group_order();
    let (kept, total) = state.counts();
    for (gi, (group, indices)) in groups.iter().enumerate() {
        if rest.height == 0 {
            break;
        }
        let want = (indices.len() as u16 + 3).min(rest.height);
        let group_area = Rect::new(rest.x, rest.y, rest.width, want);
        let panel = Panel::card(Some(group.as_str()));
        let inner = panel.render(group_area, buf, theme);
        let items: Vec<ListItem> = indices.iter().map(|&i| state.list_item(i)).collect();
        let mut list = ListBox::new(
            junie_tui::core::id::WidgetId::of("emc.s2.list").child(gi),
            items,
            SelectMode::Multi,
        );
        for (li, &row_idx) in indices.iter().enumerate() {
            let row = &state.rows[row_idx];
            list.set_checked(li, state.is_kept(&row.manager, &row.name));
        }
        list.render(inner, buf, ctx, bg);
        rest.y += want;
        rest.height = rest.height.saturating_sub(want);
    }
    let _ = kept;
    let _ = total;
    let hints_area = Rect::new(area.x, area.bottom().saturating_sub(2), area.width, 1);
    paint_hints(
        buf,
        theme,
        hints_area,
        &[
            ("Space", "toggle"),
            ("A", "keep-all"),
            ("N", "keep-none"),
            ("/", "filter"),
            ("Enter", "apply"),
            ("Bksp", "back"),
        ],
    );
    let status_area = Rect::new(area.x, area.bottom().saturating_sub(1), area.width, 1);
    paint_status(
        buf,
        ctx,
        status_area,
        &format!("S2 · keeping {kept} of {total}"),
    );
}

fn render_apply(
    state: &mut crate::screens::apply::ApplyScreen,
    area: Rect,
    buf: &mut Buffer,
    ctx: &mut RenderCtx,
    theme: &Theme,
) {
    let bg: Color = theme.canvas;
    paint_header(
        buf,
        theme,
        Rect::new(area.x, area.y, area.width, 1),
        "3/4 Apply",
    );
    let body = Rect::new(
        area.x,
        area.y + 1,
        area.width,
        area.height.saturating_sub(3),
    );
    let step_states = state.step_states();
    let mut rail = StepRail::new(
        junie_tui::core::id::WidgetId::of("emc.s3.rail"),
        step_states
            .iter()
            .map(|(label, _)| Step::new(label))
            .collect(),
    );
    for (i, (_, step_state)) in step_states.into_iter().enumerate() {
        rail.set_state(i, step_state);
    }
    let rail_h = state.step_states().len() as u16;
    let rail_area = Rect::new(body.x, body.y, body.width, rail_h.min(body.height));
    rail.render(rail_area, buf, ctx, bg);
    let (done, total) = state.progress();
    let progress = ProgressBar::new("apply", done as f64 / total.max(1) as f64);
    let progress_area = Rect::new(
        body.x,
        body.y + rail_h.min(body.height.saturating_sub(1)),
        body.width,
        1.min(body.height.saturating_sub(rail_h + 1)),
    );
    progress.render(progress_area, buf, ctx, bg);
    let log_top = body.y + (rail_h + 1).min(body.height);
    let log_area = Rect::new(
        body.x,
        log_top,
        body.width,
        body.height.saturating_sub(log_top - body.y + 1),
    );
    let mut viewport = TextViewport::new(junie_tui::core::id::WidgetId::of("emc.s3.log"))
        .wrap(true)
        .max_lines(5000);
    viewport.set_lines(
        state
            .lines()
            .map(|l| {
                vec![ViewportSpan::new(
                    l.text.clone(),
                    match l.stream {
                        crate::backend::Stream::Stdout => Tone::Normal,
                        crate::backend::Stream::Stderr => Tone::Error,
                        crate::backend::Stream::System => Tone::Secondary,
                    },
                )]
            })
            .collect(),
    );
    viewport.render(log_area, buf, ctx, bg);
    if state.outcome().is_none() {
        let mut cancel =
            Button::danger(junie_tui::core::id::WidgetId::of("emc.s3.cancel"), "Cancel");
        let w = cancel.width().min(body.width);
        cancel.render(
            Rect::new(
                body.x + body.width.saturating_sub(w),
                log_area.bottom().saturating_sub(1).max(body.y),
                w,
                1,
            ),
            buf,
            ctx,
            bg,
        );
    }
    if state.modal_open()
        && let Some(rerun) = state.rerun()
    {
        let mut prop = junie_tui::widgets::props::Prop::new("rerun", rerun);
        prop.copyable = true;
        let modal = InfoDialog::new(
            junie_tui::core::id::WidgetId::of("emc.s3.modal"),
            "Apply failed",
            vec![prop],
        )
        .intro(vec![(
            "Phase failed — copy the rerun command.".to_string(),
            Tone::Error,
        )])
        .detail(vec![rerun.to_string()]);
        let mut modal = modal;
        modal.render(area, buf, ctx);
    }
    let hints_area = Rect::new(area.x, area.bottom().saturating_sub(2), area.width, 1);
    paint_hints(
        buf,
        theme,
        hints_area,
        &[
            ("Esc", "cancel"),
            ("↑↓", "scroll"),
            ("c", "copy"),
            ("Enter", "done"),
        ],
    );
    let status_area = Rect::new(area.x, area.bottom().saturating_sub(1), area.width, 1);
    paint_status(buf, ctx, status_area, "S3 · worker streams to External");
}

fn render_result(
    state: &crate::screens::result::ResultScreen,
    area: Rect,
    buf: &mut Buffer,
    ctx: &mut RenderCtx,
    theme: &Theme,
) {
    let bg: Color = theme.canvas;
    paint_header(
        buf,
        theme,
        Rect::new(area.x, area.y, area.width, 1),
        "4/4 Result",
    );
    let body = Rect::new(
        area.x,
        area.y + 1,
        area.width,
        area.height.saturating_sub(3),
    );
    let table_h = (state.parts().len() as u16 + 3).min(body.height.saturating_sub(4).max(4));
    let table_area = Rect::new(body.x, body.y, body.width, table_h);
    let mut grid = junie_tui::widgets::grid::DataGrid::new(
        junie_tui::core::id::WidgetId::of("emc.s4.grid"),
        crate::screens::result::ResultScreen::grid_columns(),
    );
    grid.set_rows(GridRows {
        rows: state.grid_rows(),
        total: RowTotal::Exact(state.parts().len()),
        more: false,
    });
    grid.render(table_area, buf, ctx, bg);
    let follow_area = Rect::new(
        body.x,
        table_area.bottom(),
        body.width,
        body.height.saturating_sub(table_area.bottom() - body.y + 1),
    );
    props::render(follow_area, buf, theme, &state.follow_up_props(), bg);
    let hints_area = Rect::new(area.x, area.bottom().saturating_sub(2), area.width, 1);
    paint_hints(
        buf,
        theme,
        hints_area,
        &[
            ("R", "re-check"),
            ("c", "copy"),
            ("B", "back"),
            ("Enter", "done"),
        ],
    );
    let status_area = Rect::new(area.x, area.bottom().saturating_sub(1), area.width, 1);
    paint_status(buf, ctx, status_area, state.summary());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::mock::MockBackend;
    use junie_tui::core::event::Key;
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
        let dir = std::env::temp_dir().join(format!("emc-test-app-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        std::fs::write(
            dir.join("bootstrap.sh"),
            "environment_names=(\n  alpha beta\n)\nenvironment_descriptions=(\n  'First env'\n  'Second env'\n)\n",
        )
        .expect("write bootstrap.sh");
        std::fs::write(dir.join("miserc.local.toml"), "env = [\"alpha\"]\n").expect("write local");
        // Canned origins for the fixture envs.
        let backend: Arc<dyn Backend> = Arc::new(
            MockBackend::new()
                .with_origin("brew", "jq", &["alpha"])
                .with_origin("brew", "rg", &["beta"]),
        );
        App::new(backend, &dir).expect("app builds")
    }

    /// Pump until the screen leaves `apply` (bounded; the mock worker
    /// finishes in milliseconds).
    fn settle(app: &mut App) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while app.screen_name() == "apply" && std::time::Instant::now() < deadline {
            app.pump();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    #[test]
    fn s1_enter_moves_to_s2_with_pretrained_selection() {
        let mut app = mock_app();
        assert_eq!(app.screen_name(), "modules");
        // alpha pre-ticked from miserc.local.toml; toggle beta, confirm.
        app.handle_key(&char_key(' '));
        app.handle_key(&key(KeyCode::Down));
        app.handle_key(&char_key(' '));
        app.handle_key(&key(KeyCode::Enter));
        assert_eq!(app.screen_name(), "packages");
    }

    #[test]
    fn s2_enter_starts_apply_and_pump_reaches_result() {
        let mut app = mock_app();
        app.handle_key(&key(KeyCode::Enter));
        assert_eq!(app.screen_name(), "packages");
        app.handle_key(&key(KeyCode::Enter));
        assert_eq!(app.screen_name(), "apply");
        // S3 opens idle; Enter starts the worker, the pump moves S3 → S4.
        app.handle_key(&key(KeyCode::Enter));
        settle(&mut app);
        assert_eq!(app.screen_name(), "result");
    }

    #[test]
    fn s4_back_restores_packages_and_done_quits() {
        let mut app = mock_app();
        app.handle_key(&key(KeyCode::Enter));
        app.handle_key(&key(KeyCode::Enter));
        app.handle_key(&key(KeyCode::Enter));
        settle(&mut app);
        assert_eq!(app.screen_name(), "result");
        app.handle_key(&char_key('B'));
        assert_eq!(app.screen_name(), "packages");
        // Reaches S3 idle, starts the worker, finishes to S4.
        app.handle_key(&key(KeyCode::Enter));
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
}
