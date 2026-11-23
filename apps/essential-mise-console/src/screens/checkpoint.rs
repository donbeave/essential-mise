//! Checkpoint after a successful apply.
//!
//! The screen stays on step 3 until `check --json` is exactly satisfied
//! (`ok` is true and `status` is `"ok"`). Quit, Esc, and Ctrl+C ask first
//! and then leave with exit 3. The check runs on a worker thread.

use std::path::PathBuf;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::Color;
use termrock::core::event::Key;
use termrock::core::focus::{Focus, FocusRing};
use termrock::core::id::WidgetId;
use termrock::core::pointer::PointerEvent;
use termrock::core::response::Response;
use termrock::theme::{Theme, Tone};
use termrock::ui::ctx::RenderCtx;
use termrock::ui::modal::ModalStack;
use termrock::widgets::dialog::{Dialog, DialogResult};

use super::chrome::render_step_header;
use super::paint::{self, Marks, bold, footer, primary_line, put, seg};
use crate::checkpoint::{self, CheckpointHandle, JobKind, JobOutput, Record, Report, shown_lines};

const CONFIRM: WidgetId = WidgetId::of("emc.checkpoint.confirm");

/// Leave the console. The app uses exit code 3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CheckpointAction {
    /// The confirm dialog was accepted.
    Quit,
}

enum PhaseState {
    Checking,
    Report(Report),
    Failed(String),
}

/// Checkpoint state. The owner paints it and owns the confirm dialog.
pub(crate) struct CheckpointScreen {
    config_dir: PathBuf,
    script: Option<String>,
    phase: PhaseState,
    worker: Option<CheckpointHandle>,
    open_error: Option<String>,
    #[cfg(test)]
    checks: u32,
}

impl CheckpointScreen {
    pub(crate) fn new(config_dir: PathBuf, record: Record) -> Self {
        let (script, phase) = match record {
            Record::Ready(sentinel) => (Some(sentinel.script), PhaseState::Checking),
            Record::Error(text) => (None, PhaseState::Failed(text)),
            Record::None => (
                None,
                PhaseState::Failed("the accounts check did not run".to_string()),
            ),
        };
        Self {
            config_dir,
            script,
            phase,
            worker: None,
            open_error: None,
            #[cfg(test)]
            checks: 0,
        }
    }

    /// Start the first check. A malformed sentinel has no script, so this
    /// does not spawn. A second call while a worker is running is ignored.
    pub(crate) fn start(&mut self) {
        self.check_again();
    }

    pub(crate) fn check_again(&mut self) {
        if self.worker.is_some() {
            return;
        }
        let Some(script) = self.script.clone() else {
            return;
        };
        #[cfg(test)]
        {
            self.checks += 1;
        }
        self.open_error = None;
        self.worker = Some(checkpoint::spawn_job(
            self.config_dir.clone(),
            script,
            JobKind::Check,
        ));
    }

    pub(crate) fn open(&mut self) {
        if self.worker.is_some() || !self.can_open() {
            return;
        }
        let Some(script) = self.script.clone() else {
            return;
        };
        self.open_error = None;
        self.worker = Some(checkpoint::spawn_job(
            self.config_dir.clone(),
            script,
            JobKind::Open,
        ));
    }

    pub(crate) fn checking(&self) -> bool {
        self.worker.is_some()
    }

    #[cfg(test)]
    pub(crate) fn checks(&self) -> u32 {
        self.checks
    }

    pub(crate) fn can_open(&self) -> bool {
        matches!(&self.phase, PhaseState::Report(report) if report.gaps())
    }

    /// Apply a finished worker result. `true` means the report is exactly
    /// satisfied and the app should open Result.
    pub(crate) fn finish_job(&mut self, output: JobOutput) -> bool {
        self.worker = None;
        match output {
            JobOutput::Report(result) => self.apply_report(result),
            JobOutput::Opened(Ok(())) => false,
            JobOutput::Opened(Err(text)) => {
                self.open_error = Some(text);
                false
            }
        }
    }

    pub(crate) fn poll(&mut self) -> Option<JobOutput> {
        self.worker.as_ref().and_then(CheckpointHandle::poll)
    }

    /// Lines for the normal terminal. Empty `lines` on an unsatisfied report
    /// use the fixed sentence.
    pub(crate) fn exit_lines(&self) -> Vec<String> {
        let mut lines = match &self.phase {
            PhaseState::Report(report) => shown_lines(report),
            PhaseState::Failed(text) => vec![text.clone()],
            PhaseState::Checking => Vec::new(),
        };
        if let Some(text) = &self.open_error {
            lines.push(text.clone());
        }
        lines
    }

    fn apply_report(&mut self, result: Result<Report, String>) -> bool {
        match result {
            Ok(report) if report.satisfied() => true,
            Ok(report) => {
                self.phase = PhaseState::Report(report);
                false
            }
            Err(text) => {
                self.phase = PhaseState::Failed(text);
                false
            }
        }
    }
}

/// Paints [`CheckpointScreen`] and maps keys to [`CheckpointAction`]s.
pub(crate) struct CheckpointOwner {
    pub(crate) state: CheckpointScreen,
    modals: ModalStack<Box<Dialog>>,
}

impl CheckpointOwner {
    pub(crate) fn new(state: CheckpointScreen) -> Self {
        Self {
            state,
            modals: ModalStack::new(),
        }
    }

    pub(crate) fn primary_focus(&self) -> WidgetId {
        if let Some(dialog) = self.modals.top() {
            return dialog.initial_focus;
        }
        paint::PRIMARY
    }

    pub(crate) fn layer_open(&self) -> bool {
        !self.modals.is_empty()
    }

    pub(crate) fn hints(&self) -> Vec<(&'static str, String)> {
        if self.layer_open() {
            return vec![("Enter", "choose".into()), ("Esc", "stay".into())];
        }
        let mut hints = vec![("Enter", "check again".into())];
        if self.state.can_open() {
            hints.push(("o", "open the accounts pane".into()));
        }
        hints.push(("Esc", "leave (asks first)".into()));
        hints.push(("q", "quit (asks first)".into()));
        hints.push(("Ctrl+C", "quit (asks first)".into()));
        hints.push(("?", "help".into()));
        hints
    }

    pub(crate) fn key(&mut self, key: &Key, focus: &mut Focus) -> Response<CheckpointAction> {
        if let Some(dialog) = self.modals.top_mut() {
            let mut modal_ring = FocusRing::default();
            for action in dialog.actions.iter().filter(|action| action.can_activate()) {
                modal_ring.register(action.id);
            }
            let out = dialog.on_key(key, focus, &modal_ring);
            let response = self.take_modal_result(focus);
            return response.merge(out.into());
        }
        if key.is(KeyCode::Enter) {
            self.state.check_again();
            return Response::changed();
        }
        if key.is_char('o') {
            self.state.open();
            return Response::changed();
        }
        if key.is_char('q') || key.is(KeyCode::Esc) || key.ctrl_char('c') {
            return self.arm_confirm(focus).into();
        }
        Response::ignored()
    }

    pub(crate) fn pointer(
        &mut self,
        ev: &PointerEvent,
        focus: &mut Focus,
    ) -> Response<CheckpointAction> {
        if let Some(dialog) = self.modals.top_mut() {
            let response = dialog.pointer_in(ev, focus);
            let result = self.take_modal_result(focus);
            return result.merge(response.outcome().into());
        }
        let _ = ev;
        Response::ignored()
    }

    pub(crate) fn outside(&mut self, focus: &mut Focus) -> Response<CheckpointAction> {
        if self.modals.top().is_none() {
            return Response::consumed();
        }
        self.modals.pop(focus);
        Response::changed()
    }

    pub(crate) fn draw(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let theme: Theme = *ctx.theme;
        let bg: Color = theme.canvas;
        let marks = Marks::for_theme(&theme);
        paint::clear(buf, area, bg);
        render_step_header(
            3,
            "Accounts",
            Rect::new(area.x, area.y, area.width, 1),
            buf,
            &theme,
        );
        let width = area.width.saturating_sub(2);
        let mut rows = Vec::new();
        let show_spinner =
            self.state.checking() || matches!(self.state.phase, PhaseState::Checking);
        if show_spinner {
            rows.push(vec![
                seg(
                    format!("{} ", marks.frame(ctx.interaction.tick)),
                    Tone::Active,
                ),
                bold(
                    format!("Checking the accounts{}", marks.ellipsis),
                    Tone::Normal,
                ),
            ]);
        }
        match &self.state.phase {
            PhaseState::Checking => {}
            PhaseState::Report(report) => {
                rows.push(vec![bold(report.title.clone(), Tone::Normal)]);
                for line in shown_lines(report) {
                    rows.push(vec![seg(line, Tone::Normal)]);
                }
            }
            PhaseState::Failed(text) => rows.push(vec![
                bold(format!("{} ", marks.fail), Tone::Error),
                seg(text.clone(), Tone::Error),
            ]),
        }
        if let Some(text) = &self.state.open_error {
            rows.push(vec![
                bold(format!("{} ", marks.fail), Tone::Error),
                seg(text.clone(), Tone::Error),
            ]);
        }
        let body_h = area.height.saturating_sub(3) as usize;
        for (index, row) in rows.into_iter().take(body_h).enumerate() {
            put(
                buf,
                area.x + 1,
                area.y + 1 + index as u16,
                width,
                &row,
                &theme,
                bg,
            );
        }
        let primary_y = area.bottom().saturating_sub(2);
        primary_line(
            Rect::new(area.x, primary_y, area.width, 1),
            buf,
            ctx,
            bg,
            "Enter",
            "Check again",
        );
        if let Some(dialog) = self.modals.top_mut() {
            dialog.render(area, buf, ctx);
        }
        footer(
            Rect::new(area.x, area.bottom().saturating_sub(1), area.width, 1),
            buf,
            &theme,
            &self.hints(),
        );
    }

    fn arm_confirm(&mut self, focus: &mut Focus) -> termrock::core::event::Outcome {
        if !self.modals.is_empty() {
            return termrock::core::event::Outcome::Consumed;
        }
        let dialog = Dialog::destructive(
            CONFIRM,
            "Leave anyway?",
            "The apply has finished, and the Google accounts are not all set. Leave anyway? The accounts are not changed.",
            "Leave",
        );
        let initial = dialog.initial_focus;
        self.modals.push(Box::new(dialog), focus, Some(initial));
        termrock::core::event::Outcome::Changed
    }

    fn take_modal_result(&mut self, focus: &mut Focus) -> Response<CheckpointAction> {
        let Some(dialog) = self.modals.top_mut() else {
            return Response::ignored();
        };
        match dialog.result.take() {
            Some(DialogResult::Action(1)) => {
                self.modals.pop(focus);
                Response::emit(CheckpointAction::Quit)
            }
            Some(DialogResult::Action(_) | DialogResult::Cancelled) => {
                self.modals.pop(focus);
                Response::changed()
            }
            None => Response::ignored(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checkpoint::{Sentinel, parse_report};
    use ratatui::crossterm::event::KeyModifiers;

    fn focus() -> Focus {
        Focus::default()
    }

    fn key(code: KeyCode) -> Key {
        Key {
            code,
            mods: KeyModifiers::empty(),
        }
    }

    fn char_key(c: char) -> Key {
        key(KeyCode::Char(c))
    }

    fn gaps_owner() -> CheckpointOwner {
        let mut owner = CheckpointOwner::new(CheckpointScreen::new(
            PathBuf::from("/tmp/emc-checkpoint"),
            Record::Ready(Sentinel {
                script: "bin/example.ts".into(),
            }),
        ));
        let report = parse_report(
            r#"{"ok":false,"status":"gaps","title":"Google accounts","lines":["user@example.com is not an Internet Account on this Mac. Add it and choose Google. Turn Mail and Calendar on."]}"#,
        )
        .expect("report");
        assert!(!owner.state.apply_report(Ok(report)));
        owner
    }

    #[test]
    fn empty_unsatisfied_lines_use_the_fixed_sentence() {
        let mut owner = gaps_owner();
        let report =
            parse_report(r#"{"ok":false,"status":"ok","title":"Google accounts","lines":[]}"#)
                .expect("report");
        assert!(!owner.state.apply_report(Ok(report)));
        assert_eq!(
            owner.state.exit_lines(),
            vec![checkpoint::EMPTY_REPORT.to_string()]
        );
        assert!(!owner.state.can_open());
    }

    #[test]
    fn open_is_only_offered_for_gaps_and_a_failure_stays() {
        let mut owner = gaps_owner();
        assert!(owner.hints().iter().any(|(key, _)| *key == "o"));
        owner.state.open_error = Some("the pane did not open".into());
        assert!(
            owner
                .state
                .exit_lines()
                .iter()
                .any(|line| line.contains("pane"))
        );
        assert!(owner.state.can_open(), "a failed open does not advance");
        assert!(!owner.state.checking());

        let report = parse_report(
            r#"{"ok":false,"status":"schema-mismatch","title":"Google accounts","lines":["shape"]}"#,
        )
        .expect("schema");
        assert!(!owner.state.apply_report(Ok(report)));
        assert!(!owner.state.can_open());
        assert!(!owner.hints().iter().any(|(key, _)| *key == "o"));
        owner.state.open();
        assert!(!owner.state.checking(), "schema does not open");
    }

    #[test]
    fn a_second_check_while_one_is_running_is_ignored() {
        let mut owner = CheckpointOwner::new(CheckpointScreen::new(
            PathBuf::from("/tmp/emc-checkpoint-missing"),
            Record::Ready(Sentinel {
                script: "bin/example.ts".into(),
            }),
        ));
        owner.state.start();
        let checks = owner.state.checks;
        owner.state.check_again();
        assert_eq!(owner.state.checks, checks);
        assert!(owner.state.checking());
        let _ = owner.state.worker.take();
    }

    #[test]
    fn quit_asks_and_esc_on_the_dialog_stays() {
        let mut owner = gaps_owner();
        let mut focus = focus();
        let first = owner.key(&char_key('q'), &mut focus);
        assert!(first.actions().next().is_none());
        assert!(owner.layer_open());
        let _ = owner.key(&key(KeyCode::Esc), &mut focus);
        assert!(!owner.layer_open());
        let _ = owner.key(&key(KeyCode::Esc), &mut focus);
        assert!(owner.layer_open());
        let _ = owner.outside(&mut focus);
        assert!(!owner.layer_open(), "a click outside stays");
        let _ = owner.key(&char_key('q'), &mut focus);
        let _ = owner.key(&key(KeyCode::Right), &mut focus);
        let response = owner.key(&key(KeyCode::Enter), &mut focus);
        assert!(
            response
                .actions()
                .any(|action| *action == CheckpointAction::Quit)
        );
    }
}
