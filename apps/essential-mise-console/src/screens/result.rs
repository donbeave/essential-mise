//! S4 Result: verify screen with drift table, Done gate, and re-check.
//!
//! Entry auto-runs [`Backend::status`](crate::backend::Backend::status); the
//! drift verdict joins plan entries with status parts. The screen owner keeps
//! one read-only `DataGrid`, preserving navigation and scrolling across
//! refreshes. Follow-ups stay read-only and are never auto-executed. `Done`
//! exits 0 only when nothing drifts; `R` re-checks, `B` restores S2, and `c`
//! copies the summary through the shell clipboard.

use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::focus::{Focus, FocusChange, FocusRing};
use junie_tui::core::id::WidgetId;
use junie_tui::core::intent::Phase;
use junie_tui::core::pointer::{MouseTarget, PointerEvent};
use junie_tui::core::response::Response;
use junie_tui::shell::clipboard::Clipboard;
use junie_tui::theme::{Theme, Tone};
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::widgets::grid::{CellValue, ColumnSpec, DataGrid, GridAction, GridRows, RowTotal};
use junie_tui::widgets::props::{self, Prop};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

use super::chrome::{render_header, render_hints, render_status};
use super::rerun_fmt::rerun_command;
use crate::backend::Backend;
use crate::status::{Health, Note, StatusReport};

/// Per-part drift state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartState {
    /// Clean (status clean and no plan drift).
    Ok,
    /// Drifted (plan non-`noop` or status-derived drift).
    Drift,
}

/// One drift-table row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartRow {
    /// Part name (e.g. `packages`).
    pub part: String,
    /// Drift state.
    pub state: PartState,
    /// Human note (counts, pending specs, context).
    pub note: String,
}

/// Key intent on the result screen (§3: `R` re-checks, `B` back to S2,
/// `c` copies, `Enter`/`D` dones when all ok).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultKey {
    /// Re-run `status --json` (+ plan) now.
    Recheck,
    /// Back to S2 (selection preserved by the caller).
    Back,
    /// Copy the summary.
    Copy,
    /// Done (only when [`ResultScreen::can_done`]).
    Done,
}

/// Screen-level action returned by [`ResultScreen::apply`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultAction {
    /// State changed; stay on screen.
    None,
    /// Back to S2.
    Back,
    /// Done; the caller exits 0 (only reachable when all ok).
    Done,
}

/// S4 verify state. Owns the last `status` + `plan` snapshots.
pub struct ResultScreen {
    backend: std::sync::Arc<dyn Backend>,
    envs: Vec<String>,
    keep: Vec<String>,
    parts: Vec<PartRow>,
    summary: String,
    follow_ups: Vec<String>,
    checks: u32,
    last_error: Option<String>,
    last_drift: Vec<String>,
}

impl ResultScreen {
    /// Build for the selected envs and the S2 keep-list (for reruns).
    pub fn new(backend: std::sync::Arc<dyn Backend>, envs: Vec<String>, keep: Vec<String>) -> Self {
        Self {
            backend,
            envs,
            keep,
            parts: Vec::new(),
            summary: String::new(),
            follow_ups: Vec::new(),
            checks: 0,
            last_error: None,
            last_drift: Vec::new(),
        }
    }

    /// Auto-run on entry: `status --json` plus plan, rebuilding the table.
    /// A backend failure is recorded, never panics.
    pub fn refresh(&mut self, backend: &dyn Backend) {
        match backend.status(&self.envs) {
            Ok(report) => {
                self.rebuild(&report.scoped(&self.keep));
                self.last_error = None;
            }
            Err(e) => {
                self.last_error = Some(format!("status failed: {e}"));
            }
        }
        self.checks += 1;
    }

    /// Map a key to an intent (`Enter`/`D` only fire when all ok).
    pub fn map_key(&self, key: &junie_tui::core::event::Key) -> Option<ResultKey> {
        use ratatui::crossterm::event::KeyCode;
        if key.is_char('R') || key.is_char('r') {
            return Some(ResultKey::Recheck);
        }
        if key.is_char('B') || key.is_char('b') {
            return Some(ResultKey::Back);
        }
        if key.is_char('c') {
            return Some(ResultKey::Copy);
        }
        if (key.is(KeyCode::Enter) || key.is_char('D') || key.is_char('d')) && self.can_done() {
            return Some(ResultKey::Done);
        }
        None
    }

    /// Apply an intent. `Recheck` re-runs against the given backend and `Copy`
    /// queues the summary through the shared shell clipboard.
    pub fn apply(
        &mut self,
        key: ResultKey,
        backend: &dyn Backend,
        clipboard: &Clipboard,
    ) -> ResultAction {
        match key {
            ResultKey::Recheck => {
                self.refresh(backend);
                ResultAction::None
            }
            ResultKey::Back => ResultAction::Back,
            ResultKey::Copy => {
                self.copy_summary(clipboard);
                ResultAction::None
            }
            ResultKey::Done => {
                if self.can_done() {
                    ResultAction::Done
                } else {
                    ResultAction::None
                }
            }
        }
    }

    /// Done is enabled only when nothing drifts.
    pub fn can_done(&self) -> bool {
        !self.parts.is_empty() && self.parts.iter().all(|p| p.state == PartState::Ok)
    }

    /// Drifted kept specs (`manager:name`) for the rerun command.
    /// Only kept packages count: omitted entries stay omitted (one-shot).
    pub fn drifted_specs(&self) -> Vec<String> {
        self.last_drift.clone()
    }

    /// Exact rerun command for the drifted subset, if any.
    pub fn rerun(&self) -> Option<String> {
        self.rerun_text()
    }

    /// Summary line (`✓ N of M parts ok · K drift`).
    pub fn summary(&self) -> &str {
        &self.summary
    }

    /// Read-only follow-ups (never auto-executed).
    pub fn follow_ups(&self) -> &[String] {
        &self.follow_ups
    }

    /// Drift-table rows, in part order.
    pub fn parts(&self) -> &[PartRow] {
        &self.parts
    }

    /// Re-check count.
    pub fn checks(&self) -> u32 {
        self.checks
    }

    /// Last backend error, if any.
    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    /// Grid columns for the drift table (`part`, `state`, `note`).
    pub fn grid_columns() -> Vec<ColumnSpec> {
        use junie_tui::widgets::grid::CellKind;
        vec![
            ColumnSpec::new("part", CellKind::Text),
            ColumnSpec::new("state", CellKind::Text),
            ColumnSpec::new("note", CellKind::Text),
        ]
    }

    /// Grid rows for the drift table, matching [`grid_columns`](Self::grid_columns).
    pub fn grid_rows(&self) -> Vec<Vec<CellValue>> {
        self.parts
            .iter()
            .map(|p| {
                vec![
                    CellValue::Text(p.part.clone()),
                    CellValue::Text(
                        match p.state {
                            PartState::Ok => "ok",
                            PartState::Drift => "drift",
                        }
                        .to_string(),
                    ),
                    CellValue::Text(p.note.clone()),
                ]
            })
            .collect()
    }

    /// Follow-ups as props for the follow-up pane.
    pub fn follow_up_props(&self) -> Vec<Prop> {
        self.follow_ups
            .iter()
            .map(|f| {
                let mut prop = Prop::new("follow-up", f);
                prop.tone = Tone::Secondary;
                prop
            })
            .collect()
    }

    /// Copy the summary through the shell clipboard (OSC52).
    pub fn copy_summary(
        &self,
        clipboard: &junie_tui::shell::clipboard::Clipboard,
    ) -> junie_tui::shell::clipboard::Copied {
        clipboard.copy(&self.summary)
    }

    fn rebuild(&mut self, report: &StatusReport) {
        let mut parts = Vec::with_capacity(report.areas.len());
        let mut drifted = 0usize;
        let mut drifted_specs = Vec::new();
        for area in &report.areas {
            let changes = area.changes();
            let (state, note) = match area.health() {
                Health::NeedsChanges => {
                    drifted += 1;
                    drifted_specs.extend(changes.iter().filter_map(|e| e.spec.clone()));
                    (
                        PartState::Drift,
                        format!("{} of {} differ", changes.len(), area.total()),
                    )
                }
                Health::Unknown => (PartState::Ok, "could not check".to_string()),
                Health::Skipped => (PartState::Ok, "skipped".to_string()),
                Health::UpToDate => (PartState::Ok, format!("{} up to date", area.total())),
            };
            parts.push(PartRow {
                part: area.key.label().to_string(),
                state,
                note,
            });
        }
        let ok = parts.len() - drifted;
        self.summary = format!(
            "{ok} of {} areas up to date, {drifted} need changes",
            parts.len()
        );
        self.last_drift = drifted_specs;
        self.follow_ups = follow_ups(report, self.rerun_text());
        self.parts = parts;
    }

    fn rerun_text(&self) -> Option<String> {
        if self.last_drift.is_empty() {
            return None;
        }
        let mut specs = self.last_drift.clone();
        specs.sort();
        let mut phase = vec![
            "bootstrap".to_string(),
            "packages".to_string(),
            "apply".to_string(),
        ];
        phase.extend(specs);
        Some(rerun_command(
            self.backend.mise_bin(),
            self.backend.config_dir(),
            &self.envs,
            &phase,
        ))
    }
}

/// Read-only follow-ups, surfaced and never auto-executed.
fn follow_ups(report: &StatusReport, rerun: Option<String>) -> Vec<String> {
    let mut out = Vec::new();
    if report.notes.contains(&Note::LoginShellNotRecorded) {
        out.push("open a fresh login shell to verify activation".to_string());
    }
    if !report.is_clean() {
        out.push(match rerun {
            Some(cmd) => format!("rerun: {cmd}"),
            None => "apply again to update the remaining areas".to_string(),
        });
    }
    out
}

const S4_GRID: WidgetId = WidgetId::of("emc.s4.grid");

pub(crate) struct ResultOwner {
    pub(crate) state: ResultScreen,
    grid: DataGrid,
}

impl ResultOwner {
    pub(crate) fn new(state: ResultScreen) -> Self {
        let mut owner = Self {
            grid: DataGrid::new(S4_GRID, ResultScreen::grid_columns()).editable(false),
            state,
        };
        owner.sync_rows();
        owner
    }

    pub(crate) fn primary_focus(&self) -> WidgetId {
        S4_GRID
    }

    fn sync_rows(&mut self) {
        self.grid.set_rows(GridRows {
            rows: self.state.grid_rows(),
            total: RowTotal::Exact(self.state.parts().len()),
            more: false,
        });
    }

    fn grid_response(
        &mut self,
        out: Outcome,
        action: Option<GridAction>,
        focus: &mut Focus,
        ring: &FocusRing,
        clipboard: &Clipboard,
    ) -> Response<ResultAction> {
        match action {
            Some(GridAction::Copy(text)) => {
                clipboard.copy(&text);
                Response::changed()
            }
            Some(GridAction::Refresh) => {
                let backend = self.state.backend.clone();
                self.state.refresh(&*backend);
                self.sync_rows();
                Response::changed()
            }
            Some(GridAction::LeaveForward) => {
                focus.next(ring);
                Response::changed()
            }
            Some(GridAction::LeaveBackward) => {
                focus.prev(ring);
                Response::changed()
            }
            Some(GridAction::ReadOnly { .. }) => Response::consumed(),
            _ => out.into(),
        }
    }

    pub(crate) fn key(
        &mut self,
        key: &Key,
        focus: &mut Focus,
        ring: &FocusRing,
        backend: &dyn Backend,
        clipboard: &Clipboard,
    ) -> Response<ResultAction> {
        if let Some(mapped) = self.state.map_key(key) {
            let action = self.state.apply(mapped, backend, clipboard);
            if matches!(mapped, ResultKey::Recheck) {
                self.sync_rows();
            }
            return match action {
                ResultAction::Back | ResultAction::Done => Response::emit(action),
                ResultAction::None => Response::changed(),
            };
        }
        if focus.current() != Some(S4_GRID) {
            return Response::ignored();
        }
        let (out, action) = self.grid.on_key(key);
        self.grid_response(out, action, focus, ring, clipboard)
    }

    pub(crate) fn pointer(
        &mut self,
        ev: &PointerEvent,
        focus: &mut Focus,
        ring: &FocusRing,
        clipboard: &Clipboard,
    ) -> Response<ResultAction> {
        let Some(response) = self.grid.route(ev) else {
            return Response::ignored();
        };
        if ev.is(Phase::Click) {
            focus.focus(S4_GRID);
        }
        let (out, action) = response.into_parts();
        self.grid_response(out, action, focus, ring, clipboard)
    }

    pub(crate) fn blur(&mut self, change: FocusChange, clipboard: &Clipboard) -> Outcome {
        if change.from != S4_GRID {
            return Outcome::Ignored;
        }
        let (out, action) = self.grid.on_blur();
        if let Some(GridAction::Copy(text)) = action {
            clipboard.copy(&text);
        }
        out
    }

    pub(crate) fn draw(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let theme: Theme = *ctx.theme;
        let bg: Color = theme.canvas;
        render_header(
            4,
            "Result",
            Rect::new(area.x, area.y, area.width, 1),
            buf,
            &theme,
        );
        let body = Rect::new(
            area.x,
            area.y + 1,
            area.width,
            area.height.saturating_sub(3),
        );
        let table_h =
            (self.state.parts().len() as u16 + 3).min(body.height.saturating_sub(4).max(4));
        let table_area = Rect::new(body.x, body.y, body.width, table_h);
        self.grid.render(table_area, buf, ctx, bg);
        let follow_area = Rect::new(
            body.x,
            table_area.bottom(),
            body.width,
            body.height.saturating_sub(table_area.bottom() - body.y + 1),
        );
        props::render(follow_area, buf, &theme, &self.state.follow_up_props(), bg);
        render_hints(
            &[
                ("R", "re-check"),
                ("c", "copy"),
                ("B", "back"),
                ("Enter", "done"),
            ],
            Rect::new(area.x, area.bottom().saturating_sub(2), area.width, 1),
            buf,
            &theme,
        );
        render_status(
            self.state.summary(),
            Rect::new(area.x, area.bottom().saturating_sub(1), area.width, 1),
            buf,
            ctx,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::MiseBackend;
    use crate::backend::mock::MockBackend;
    use junie_tui::core::event::Key;
    use ratatui::crossterm::event::{KeyCode, KeyModifiers};

    fn test_backend() -> std::sync::Arc<dyn Backend> {
        std::sync::Arc::new(MiseBackend::new(std::path::PathBuf::from(
            "/tmp/emc-test-result",
        )))
    }

    fn result() -> ResultScreen {
        ResultScreen::new(
            test_backend(),
            vec!["terminal".to_string()],
            vec!["brew-cask:jetbrains-toolbox".to_string()],
        )
    }

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

    #[test]
    fn refresh_clean_fixture_enables_done() {
        let mock = MockBackend::new();
        let mut screen = result();
        screen.refresh(&mock);
        assert_eq!(screen.checks(), 1);
        assert!(screen.last_error().is_none());
        assert!(!screen.parts().is_empty());
        assert!(screen.can_done(), "clean fixture has no drift");
        assert!(screen.summary().contains("0 need changes"));
        assert!(screen.rerun().is_none());
        assert!(
            screen
                .follow_ups()
                .iter()
                .any(|f| f.contains("fresh login shell")),
            "unrecorded login shell surfaces a follow-up"
        );
        assert_eq!(ResultScreen::grid_columns().len(), 3);
        assert_eq!(screen.grid_rows().len(), screen.parts().len());
        assert!(!screen.follow_up_props().is_empty());
        let copied = screen.copy_summary(&junie_tui::shell::clipboard::Clipboard::new());
        assert!(
            matches!(copied, junie_tui::shell::clipboard::Copied::Copied { .. }),
            "summary copies through the shell clipboard"
        );
    }

    #[test]
    fn drift_blocks_done_with_rerun() {
        let mut screen = result();
        let report = StatusReport::parse(
            r#"{"packages":{"brew":{"available":true,"packages":[
              {"package":"jq","desired_state":"present","state":"missing"}]}}}"#,
            "t",
        )
        .expect("parses");
        screen.keep = vec!["brew:jq".to_string()];
        screen.rebuild(&report.scoped(&screen.keep.clone()));
        assert_eq!(screen.parts()[0].state, PartState::Drift);
        assert!(!screen.can_done());
        assert_eq!(screen.map_key(&key(KeyCode::Enter)), None);
        let rerun = screen.rerun().expect("drifted rerun");
        assert!(rerun.contains("packages apply brew:jq"), "{rerun}");
        assert_eq!(screen.drifted_specs(), vec!["brew:jq".to_string()]);
    }

    #[test]
    fn omitted_packages_never_gate_done() {
        let mut screen = result();
        let report = StatusReport::parse(
            r#"{"packages":{"brew":{"available":true,"packages":[
              {"package":"jq","desired_state":"present","state":"missing"}]}}}"#,
            "t",
        )
        .expect("parses");
        screen.rebuild(&report.scoped(&[]));
        assert!(screen.can_done());
        assert!(screen.rerun().is_none());
    }

    #[test]
    fn keys_match_section_3() {
        let mock = MockBackend::new();
        let mut screen = result();
        screen.refresh(&mock);
        assert_eq!(screen.map_key(&char_key('R')), Some(ResultKey::Recheck));
        assert_eq!(screen.map_key(&char_key('r')), Some(ResultKey::Recheck));
        assert_eq!(screen.map_key(&char_key('B')), Some(ResultKey::Back));
        assert_eq!(screen.map_key(&char_key('c')), Some(ResultKey::Copy));
        assert_eq!(screen.map_key(&key(KeyCode::Enter)), Some(ResultKey::Done));
        assert_eq!(screen.map_key(&char_key('D')), Some(ResultKey::Done));
        assert_eq!(
            screen.apply(ResultKey::Back, &mock, &Clipboard::new()),
            ResultAction::Back
        );
        assert_eq!(
            screen.apply(ResultKey::Done, &mock, &Clipboard::new()),
            ResultAction::Done
        );
    }

    #[test]
    fn recheck_reruns_backend() {
        let mock = MockBackend::new();
        let mut screen = result();
        screen.refresh(&mock);
        assert_eq!(
            screen.apply(ResultKey::Recheck, &mock, &Clipboard::new()),
            ResultAction::None
        );
        assert_eq!(screen.checks(), 2);
    }
}
