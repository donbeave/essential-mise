//! S3 Apply: live two-phase apply with progress, cancel, and rerun.
//!
//! The app starts the worker on entry. The screen owner appends each external
//! line to a retained viewport and routes input through the viewport, dynamic
//! action button, and typed modal stack. Active phases use indeterminate
//! progress. Success stays visible until the user chooses Result. Failure copy
//! uses the shell clipboard (`y` inside, `c` outside). Confirmed cancellation
//! calls [`ApplyHandle::cancel`](crate::backend::ApplyHandle::cancel), waits
//! for the worker, then leaves the cancelled output available for review.
//! The queue is sender-side bounded ([`APPLY_LOG_CAP`](crate::screens::apply::APPLY_LOG_CAP)); overflow drops the
//! oldest lines and records a gap marker for the log pane.
//! Hook `PENDING` log lines never change the outcome; the Result screen reads
//! them from the log as typed follow-ups ([`crate::followup`]).

use std::collections::VecDeque;

use junie_tui::core::event::{External, Key, Outcome};
use junie_tui::core::focus::{Focus, FocusRing};
use junie_tui::core::id::WidgetId;
use junie_tui::core::pointer::{MouseTarget, PointerEvent};
use junie_tui::core::response::Response;
use junie_tui::runtime::{ExternalReceiver, external_channel};
use junie_tui::shell::clipboard::Clipboard;
use junie_tui::theme::{Theme, Tone};
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::modal::ModalStack;
use junie_tui::widgets::button::Button;
use junie_tui::widgets::dialog::{Dialog, DialogResult};
use junie_tui::widgets::info_dialog::{InfoDialog, InfoResult};
use junie_tui::widgets::progress::ProgressBar;
use junie_tui::widgets::props::Prop;
use junie_tui::widgets::steps::{Step, StepRail, StepState};
use junie_tui::widgets::viewport::{Span as ViewportSpan, TextViewport, ViewportAction};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::Color;

use super::chrome::{render_header, render_hints, render_status};
use super::rerun_fmt::rerun_command;
use crate::backend::{
    ApplyHandle, ApplyLine, ApplyOutcome, ApplyPhase, ApplyRequest, Backend, Stream,
};

/// Sender-side log bound, mirroring the viewport `.max_lines(5_000)` cap.
pub const APPLY_LOG_CAP: usize = 5_000;

/// Backend phase-header prefixes the screen watches to track the current
/// phase (same crate, co-evolving protocol).
pub const PHASE1_PREFIX: &str = "phase 1: base";
/// Phase-2 header prefix (see [`PHASE1_PREFIX`]).
pub const PHASE2_PREFIX: &str = "phase 2: packages";

/// Key intent on the apply screen. Viewport navigation is routed directly to
/// the retained `TextViewport` by the screen owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyKey {
    /// Copy the rerun command.
    Copy,
    /// Open the destructive cancel confirm.
    Cancel,
    /// Confirm cancellation.
    ConfirmCancel,
    /// Dismiss the cancel confirm or failure modal.
    Dismiss,
    /// Start the worker (idle only).
    Start,
    /// Continue to S4 (only when finished).
    Done,
}

/// Screen-level action returned by [`ApplyScreen::apply`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyAction {
    /// State changed; stay on screen.
    None,
    /// Continue to S4 after reviewing the outcome.
    Done,
}

/// S3 apply state machine.
pub struct ApplyScreen {
    backend: std::sync::Arc<dyn Backend>,
    req: ApplyRequest,
    handle: Option<ApplyHandle>,
    receiver: Option<ExternalReceiver>,
    lines: VecDeque<ApplyLine>,
    dropped: u64,
    current_phase: ApplyPhase,
    phases_done: usize,
    phases_total: usize,
    started_at: Option<std::time::Instant>,
    outcome: Option<ApplyOutcome>,
    rerun: Option<String>,
    cancel_armed: bool,
    cancel_requested: bool,
    modal_open: bool,
}

impl ApplyScreen {
    /// Build for a request. Phase count follows the keep-list: empty means
    /// phase 2 is skipped (a bare `packages apply` means *every* package).
    pub fn new(backend: std::sync::Arc<dyn Backend>, req: ApplyRequest) -> Self {
        let phases_total = if req.keep.is_empty() && req.manager.is_none() {
            1
        } else {
            2
        };
        Self {
            backend,
            req,
            handle: None,
            receiver: None,
            lines: VecDeque::new(),
            dropped: 0,
            current_phase: ApplyPhase::Base,
            phases_done: 0,
            phases_total,
            started_at: None,
            outcome: None,
            rerun: None,
            cancel_armed: false,
            cancel_requested: false,
            modal_open: false,
        }
    }

    /// Start the worker if idle; the receiver stays on the screen for the
    /// drain pump. A spawn failure becomes a `Failed` outcome with the
    /// failure modal open (same path as a phase failure).
    pub fn start(&mut self) {
        if self.started_at.is_some() {
            return;
        }
        self.started_at = Some(std::time::Instant::now());
        let (tx, rx) = external_channel();
        match self.backend.spawn_apply(&self.req, tx) {
            Ok(handle) => {
                self.handle = Some(handle);
                self.receiver = Some(rx);
            }
            Err(e) => {
                self.on_outcome(ApplyOutcome::Failed {
                    code: None,
                    tail: format!("apply failed to start: {e}"),
                    phase: ApplyPhase::Base,
                });
            }
        }
    }

    /// Whether the worker has been started.
    pub fn started(&self) -> bool {
        self.started_at.is_some()
    }

    /// Drain pending lines from the worker channel (no-op before start).
    pub fn drain(&self) -> Vec<junie_tui::core::event::External> {
        self.receiver
            .as_ref()
            .map(|rx| rx.drain())
            .unwrap_or_default()
    }

    /// The worker channel for the runtime auto-drain.
    pub fn receiver(&self) -> Option<&ExternalReceiver> {
        self.receiver.as_ref()
    }

    /// Consume one drained event; true when it was an apply line.
    /// Mirrors `app.rs::on_external`: downcast, dispatch, ignore the rest.
    pub fn on_external(&mut self, external: &junie_tui::core::event::External) -> bool {
        match external.downcast_ref::<ApplyLine>() {
            Some(line) => {
                self.push_line(line.clone());
                true
            }
            None => false,
        }
    }

    /// Map an application command to an intent. `y` copies only inside the
    /// failure modal; scrolling belongs to the retained viewport.
    pub fn map_key(&self, key: &junie_tui::core::event::Key) -> Option<ApplyKey> {
        use ratatui::crossterm::event::KeyCode;
        if (key.is(KeyCode::Esc) || key.ctrl_char('c'))
            && self.outcome.is_none()
            && !self.cancel_requested
        {
            return Some(ApplyKey::Cancel);
        }
        if (key.is_char('c') && self.rerun.is_some()) || (key.is_char('y') && self.modal_open) {
            return Some(ApplyKey::Copy);
        }
        if key.is(KeyCode::Enter) {
            if self.outcome.is_some() {
                return Some(ApplyKey::Done);
            }
            if !self.started() {
                return Some(ApplyKey::Start);
            }
        }
        None
    }

    /// Apply an intent; returns the screen-level action.
    pub fn apply(&mut self, key: ApplyKey, clipboard: &Clipboard) -> ApplyAction {
        match key {
            ApplyKey::Copy => {
                self.copy_rerun(clipboard);
                ApplyAction::None
            }
            ApplyKey::Cancel => {
                self.cancel_armed = true;
                ApplyAction::None
            }
            ApplyKey::ConfirmCancel => {
                self.confirm_cancel();
                ApplyAction::None
            }
            ApplyKey::Dismiss => {
                self.cancel_armed = false;
                self.modal_open = false;
                ApplyAction::None
            }
            ApplyKey::Start => {
                self.start();
                ApplyAction::None
            }
            ApplyKey::Done => {
                if self.outcome.is_some() {
                    ApplyAction::Done
                } else {
                    ApplyAction::None
                }
            }
        }
    }

    /// Non-blocking completion check: joins a finished worker once, records
    /// the outcome, and builds the exact rerun command.
    pub fn try_join(&mut self) -> Option<ApplyOutcome> {
        if self.outcome.is_some() {
            return None;
        }
        let handle = self.handle.take()?;
        if !handle.is_finished() {
            self.handle = Some(handle);
            return None;
        }
        let mut outcome = handle.join();
        if self.cancel_requested {
            outcome = ApplyOutcome::Cancelled;
        }
        let ret = outcome.clone();
        self.on_outcome(outcome);
        Some(ret)
    }

    /// Whether the destructive cancel confirm is open.
    pub fn cancel_armed(&self) -> bool {
        self.cancel_armed
    }

    /// Whether cancellation is waiting for worker cleanup.
    pub fn cancelling(&self) -> bool {
        self.cancel_requested && self.outcome.is_none()
    }

    /// Whether the failure modal is open.
    pub fn modal_open(&self) -> bool {
        self.modal_open
    }

    /// Current outcome, if the worker finished.
    pub fn outcome(&self) -> Option<&ApplyOutcome> {
        self.outcome.as_ref()
    }

    /// Exact rerun command for the modal and drift notes, if any.
    pub fn rerun(&self) -> Option<&str> {
        self.rerun.as_deref()
    }

    /// Buffered lines, oldest first.
    pub fn lines(&self) -> impl Iterator<Item = &ApplyLine> {
        self.lines.iter()
    }

    /// Lines dropped by the sender-side bound (gap marker for the pane).
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    /// `(done, total)` phases for terminal progress.
    pub fn progress(&self) -> (usize, usize) {
        (self.phases_done, self.phases_total)
    }

    /// Human-readable current activity for the progress and status bars.
    pub fn activity(&self) -> String {
        match self.outcome {
            Some(ApplyOutcome::Success) => "Apply complete".to_string(),
            Some(ApplyOutcome::Failed { .. }) => "Apply failed".to_string(),
            Some(ApplyOutcome::Cancelled) => "Apply cancelled".to_string(),
            None if self.cancelling() => "Cancelling apply".to_string(),
            None if !self.started() => "Ready to apply".to_string(),
            None => match self.current_phase {
                ApplyPhase::Base => "Applying base configuration".to_string(),
                ApplyPhase::Packages => "Installing selected packages".to_string(),
            },
        }
    }

    /// Seconds since the worker started.
    pub fn elapsed_seconds(&self) -> u64 {
        self.started_at
            .map(|started| started.elapsed().as_secs())
            .unwrap_or(0)
    }

    /// Most recent worker output, including backend narration.
    pub fn latest_output(&self) -> Option<&str> {
        self.lines.back().map(|line| line.text.as_str())
    }

    /// Step-rail states: base, packages (skipped when single-phase), done.
    pub fn step_states(&self) -> Vec<(&'static str, StepState)> {
        let failed = matches!(self.outcome, Some(ApplyOutcome::Failed { .. }));
        let cancelled = matches!(self.outcome, Some(ApplyOutcome::Cancelled));
        let base = if (failed || cancelled) && self.current_phase == ApplyPhase::Base {
            if failed {
                StepState::Failed
            } else {
                StepState::Blocked
            }
        } else if self.phases_done >= 1 || self.current_phase == ApplyPhase::Packages {
            StepState::Done
        } else if self.outcome.is_none() && self.started() {
            StepState::Running
        } else if self.outcome.is_none() {
            StepState::Queued
        } else {
            StepState::Done
        };
        let mut steps = vec![("base", base)];
        if self.phases_total > 1 {
            let packages = if (failed || cancelled) && self.current_phase == ApplyPhase::Packages {
                if failed {
                    StepState::Failed
                } else {
                    StepState::Blocked
                }
            } else if self.phases_done >= 2 {
                StepState::Done
            } else if self.current_phase == ApplyPhase::Packages {
                StepState::Running
            } else {
                StepState::Queued
            };
            steps.push(("packages", packages));
        }
        steps
    }

    /// Copy the rerun command through the shell clipboard (OSC52).
    pub fn copy_rerun(&self, clipboard: &Clipboard) -> Option<junie_tui::shell::clipboard::Copied> {
        self.rerun.as_deref().map(|r| clipboard.copy(r))
    }

    fn push_line(&mut self, line: ApplyLine) {
        if line.stream == Stream::System {
            if line.text.starts_with(PHASE2_PREFIX) {
                self.current_phase = ApplyPhase::Packages;
                self.phases_done = self.phases_done.max(1);
            } else if line.text.starts_with(PHASE1_PREFIX) {
                self.current_phase = ApplyPhase::Base;
            }
        }
        if self.lines.len() == APPLY_LOG_CAP {
            self.lines.pop_front();
            self.dropped += 1;
        }
        self.lines.push_back(line);
    }

    fn confirm_cancel(&mut self) {
        self.cancel_armed = false;
        if self.outcome.is_some() || self.cancel_requested {
            return;
        }
        self.cancel_requested = true;
        if let Some(handle) = self.handle.as_mut() {
            handle.cancel();
        } else {
            self.on_outcome(ApplyOutcome::Cancelled);
        }
    }

    fn on_outcome(&mut self, outcome: ApplyOutcome) {
        match &outcome {
            ApplyOutcome::Success => {
                self.phases_done = self.phases_total;
            }
            ApplyOutcome::Failed { phase, .. } => {
                self.current_phase = *phase;
                self.phases_done = match phase {
                    ApplyPhase::Packages => 1,
                    ApplyPhase::Base => 0,
                };
                self.rerun = Some(self.rerun_for(*phase));
                self.modal_open = true;
            }
            ApplyOutcome::Cancelled => {
                self.phases_done = match self.current_phase {
                    ApplyPhase::Packages => 1,
                    ApplyPhase::Base => 0,
                };
                self.rerun = Some(self.rerun_for(self.current_phase));
            }
        }
        self.outcome = Some(outcome);
    }

    fn rerun_for(&self, phase: ApplyPhase) -> String {
        let argv = match phase {
            ApplyPhase::Base => self.backend.phase1_argv(&self.req),
            ApplyPhase::Packages => self
                .backend
                .phase2_argv(&self.req)
                .unwrap_or_else(|| self.backend.phase1_argv(&self.req)),
        };
        rerun_command(
            self.backend.mise_bin(),
            self.backend.config_dir(),
            &self.req.envs,
            &argv,
        )
    }
}

const S3_LOG: WidgetId = WidgetId::of("emc.s3.log");
const S3_CANCEL: WidgetId = WidgetId::of("emc.s3.cancel");
const S3_CANCEL_CONFIRM: WidgetId = WidgetId::of("emc.s3.cancel_confirm");
const S3_FAILURE: WidgetId = WidgetId::of("emc.s3.modal");

pub(crate) enum ApplyModal {
    Cancel(Box<Dialog>),
    Failure(Box<InfoDialog>),
}

impl ApplyModal {
    fn initial_focus(&self) -> WidgetId {
        match self {
            Self::Cancel(dialog) => dialog.initial_focus,
            Self::Failure(dialog) => dialog.initial_focus(),
        }
    }

    fn allows_outside_click(&self) -> bool {
        match self {
            Self::Cancel(dialog) => dialog.dismiss.outside_click,
            Self::Failure(dialog) => dialog.dismiss.outside_click,
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        match self {
            Self::Cancel(dialog) => dialog.render(area, buf, ctx),
            Self::Failure(dialog) => dialog.render(area, buf, ctx),
        }
    }
}

const S3_DONE: WidgetId = WidgetId::of("emc.s3.done");

pub(crate) struct ApplyOwner {
    pub(crate) state: ApplyScreen,
    pub(crate) viewport: TextViewport,
    cancel: Button,
    done: Button,
    modals: ModalStack<ApplyModal>,
}

impl ApplyOwner {
    pub(crate) fn new(state: ApplyScreen) -> Self {
        Self {
            state,
            viewport: TextViewport::new(S3_LOG)
                .wrap(true)
                .max_lines(APPLY_LOG_CAP),
            cancel: Button::danger(S3_CANCEL, "Cancel"),
            done: Button::primary(S3_DONE, "View results"),
            modals: ModalStack::new(),
        }
    }

    pub(crate) fn primary_focus(&self) -> WidgetId {
        if let Some(modal) = self.modals.top() {
            return modal.initial_focus();
        }
        if matches!(self.state.outcome(), Some(ApplyOutcome::Success)) {
            S3_DONE
        } else {
            S3_LOG
        }
    }

    pub(crate) fn layer_open(&self) -> bool {
        !self.modals.is_empty()
    }

    pub(crate) fn on_external(&mut self, external: &External) -> bool {
        let Some(line) = external.downcast_ref::<ApplyLine>() else {
            return false;
        };
        if !self.state.on_external(external) {
            return false;
        }
        self.viewport.push(view_line(line));
        true
    }

    pub(crate) fn sync_modal_from_domain(&mut self, focus: &mut Focus) -> Outcome {
        if !self.modals.is_empty() {
            return Outcome::Ignored;
        }
        if self.state.cancel_armed() {
            let dialog = Dialog::destructive(
                S3_CANCEL_CONFIRM,
                "Cancel apply?",
                "Partial changes may already have been applied. The exact rerun command will be available after verification.",
                "Cancel apply",
            );
            let initial = dialog.initial_focus;
            self.modals
                .push(ApplyModal::Cancel(Box::new(dialog)), focus, Some(initial));
            return Outcome::Changed;
        }
        if self.state.modal_open()
            && let Some(rerun) = self.state.rerun()
        {
            let mut prop = Prop::new("rerun", rerun);
            prop.copyable = true;
            let detail = match self.state.outcome() {
                Some(ApplyOutcome::Failed { tail, .. }) => {
                    tail.lines().map(str::to_string).collect()
                }
                _ => vec![rerun.to_string()],
            };
            let dialog = InfoDialog::new(S3_FAILURE, "Apply failed", vec![prop])
                .intro(vec![(
                    "Copy the rerun command and try again.".to_string(),
                    Tone::Error,
                )])
                .detail(detail);
            let initial = dialog.initial_focus();
            self.modals
                .push(ApplyModal::Failure(Box::new(dialog)), focus, Some(initial));
            return Outcome::Changed;
        }
        Outcome::Ignored
    }

    pub(crate) fn sync_completion_focus(&self, focus: &mut Focus) {
        if matches!(self.state.outcome(), Some(ApplyOutcome::Success)) {
            focus.focus(S3_DONE);
        }
    }

    fn arm_cancel(&mut self, focus: &mut Focus, clipboard: &Clipboard) -> Outcome {
        if self.state.outcome().is_some() || self.state.cancelling() {
            return Outcome::Consumed;
        }
        self.state.apply(ApplyKey::Cancel, clipboard);
        self.sync_modal_from_domain(focus).or(Outcome::Changed)
    }

    fn take_modal_result(
        &mut self,
        focus: &mut Focus,
        clipboard: &Clipboard,
    ) -> Response<ApplyAction> {
        let Some(modal) = self.modals.top_mut() else {
            return Response::ignored();
        };
        match modal {
            ApplyModal::Cancel(dialog) => match dialog.result.take() {
                Some(DialogResult::Action(1)) => {
                    self.modals.pop(focus);
                    let action = self.state.apply(ApplyKey::ConfirmCancel, clipboard);
                    Response::emit(action)
                }
                Some(DialogResult::Action(_) | DialogResult::Cancelled) => {
                    self.modals.pop(focus);
                    self.state.apply(ApplyKey::Dismiss, clipboard);
                    Response::changed()
                }
                None => Response::ignored(),
            },
            ApplyModal::Failure(dialog) => match dialog.result.take() {
                Some(InfoResult::Copy(text)) => {
                    clipboard.copy(&text);
                    Response::changed()
                }
                Some(InfoResult::Closed) => {
                    self.modals.pop(focus);
                    self.state.apply(ApplyKey::Dismiss, clipboard);
                    Response::emit(ApplyAction::Done)
                }
                Some(InfoResult::Action(_)) => Response::consumed(),
                None => Response::ignored(),
            },
        }
    }

    pub(crate) fn key(
        &mut self,
        key: &Key,
        focus: &mut Focus,
        ring: &FocusRing,
        clipboard: &Clipboard,
    ) -> Response<ApplyAction> {
        if key.is(KeyCode::Enter) && matches!(self.modals.top(), Some(ApplyModal::Failure(_))) {
            self.modals.pop(focus);
            self.state.apply(ApplyKey::Dismiss, clipboard);
            return Response::emit(ApplyAction::Done);
        }
        if let Some(modal) = self.modals.top_mut() {
            let out = match modal {
                ApplyModal::Cancel(dialog) => dialog.on_key(key, focus, ring),
                ApplyModal::Failure(dialog) => dialog.on_key(key, focus, ring),
            };
            let response = self.take_modal_result(focus, clipboard);
            let (result_out, action) = response.into_parts();
            return (out.or(result_out), action).into();
        }

        if (key.is(KeyCode::Esc) || key.ctrl_char('c')) && self.state.outcome().is_none() {
            return self.arm_cancel(focus, clipboard).into();
        }
        if key.is_char('c') && self.state.rerun().is_some() {
            self.state.apply(ApplyKey::Copy, clipboard);
            return Outcome::Changed.into();
        }
        if focus.current() == Some(S3_DONE) {
            let (out, fired) = self.done.on_key(key);
            if fired {
                return Response::emit(ApplyAction::Done);
            }
            if out.consumed() {
                return out.into();
            }
        }
        if focus.current() == Some(S3_CANCEL) {
            let (out, fired) = self.cancel.on_key(key);
            if fired {
                return self.arm_cancel(focus, clipboard).into();
            }
            if out.consumed() {
                return out.into();
            }
        }
        if key.is(KeyCode::Enter) {
            if self.state.outcome().is_some() {
                return Response::emit(self.state.apply(ApplyKey::Done, clipboard));
            }
            if !self.state.started() {
                self.state.apply(ApplyKey::Start, clipboard);
                return Outcome::Changed.into();
            }
        }
        if matches!(focus.current(), Some(S3_LOG | S3_DONE)) {
            let (out, action) = self.viewport.on_key(key);
            if let Some(ViewportAction::Copy(text)) = action {
                clipboard.copy(&text);
            }
            return out.into();
        }
        Outcome::Ignored.into()
    }

    pub(crate) fn pointer(
        &mut self,
        ev: &PointerEvent,
        focus: &mut Focus,
        clipboard: &Clipboard,
    ) -> Response<ApplyAction> {
        if let Some(modal) = self.modals.top_mut() {
            let response = match modal {
                ApplyModal::Cancel(dialog) => dialog.pointer_in(ev, focus),
                ApplyModal::Failure(dialog) => dialog.pointer_in(ev, focus),
            };
            let result = self.take_modal_result(focus, clipboard);
            let (result_out, action) = result.into_parts();
            return (response.outcome().or(result_out), action).into();
        }
        if matches!(self.state.outcome(), Some(ApplyOutcome::Success))
            && let Some(response) = self.done.route(ev)
        {
            let (out, fired) = response.into_parts();
            if fired.is_some() {
                return Response::emit(ApplyAction::Done);
            }
            return out.into();
        }
        if self.state.outcome().is_none()
            && !self.state.cancelling()
            && let Some(response) = self.cancel.route(ev)
        {
            let (out, fired) = response.into_parts();
            if fired.is_some() {
                return self.arm_cancel(focus, clipboard).into();
            }
            return out.into();
        }
        if let Some(response) = self.viewport.route(ev) {
            return response.lift();
        }
        Response::ignored()
    }

    pub(crate) fn paste(&self, _text: &str) -> Outcome {
        if self.layer_open() {
            Outcome::Consumed
        } else {
            Outcome::Ignored
        }
    }

    pub(crate) fn outside(
        &mut self,
        focus: &mut Focus,
        clipboard: &Clipboard,
    ) -> Response<ApplyAction> {
        let allowed = self
            .modals
            .top()
            .is_some_and(ApplyModal::allows_outside_click);
        if !allowed {
            return Response::consumed();
        }
        match self.modals.top() {
            Some(ApplyModal::Cancel(_)) => {
                self.modals.pop(focus);
                self.state.apply(ApplyKey::Dismiss, clipboard);
                Response::changed()
            }
            Some(ApplyModal::Failure(_)) => {
                self.modals.pop(focus);
                self.state.apply(ApplyKey::Dismiss, clipboard);
                Response::emit(ApplyAction::Done)
            }
            None => Response::consumed(),
        }
    }

    pub(crate) fn draw(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        use junie_tui::widgets::progress::ProgressStatus;

        let theme: Theme = *ctx.theme;
        let bg: Color = theme.canvas;
        render_header(
            3,
            "Apply",
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
        let step_states = self.state.step_states();
        let mut rail = StepRail::new(
            WidgetId::of("emc.s3.rail"),
            step_states
                .iter()
                .map(|(label, _)| Step::new(label))
                .collect(),
        );
        for (i, (_, step_state)) in step_states.iter().enumerate() {
            rail.set_state(i, *step_state);
        }
        let rail_h = step_states.len() as u16;
        rail.render(
            Rect::new(body.x, body.y, body.width, rail_h.min(body.height)),
            buf,
            ctx,
            bg,
        );
        let activity = self.state.activity();
        let (done, total) = self.state.progress();
        let ratio = done as f64 / total.max(1) as f64;
        let progress = match self.state.outcome() {
            Some(ApplyOutcome::Success) => {
                ProgressBar::new(&activity, 1.0).status(ProgressStatus::Done)
            }
            Some(ApplyOutcome::Failed { .. }) => {
                ProgressBar::new(&activity, ratio).status(ProgressStatus::Error)
            }
            Some(ApplyOutcome::Cancelled) => {
                ProgressBar::new(&activity, ratio).status(ProgressStatus::Paused)
            }
            None if !self.state.started() => {
                ProgressBar::new(&activity, 0.0).status(ProgressStatus::Paused)
            }
            None => ProgressBar::indeterminate(&activity),
        };
        progress.render(
            Rect::new(
                body.x,
                body.y + rail_h.min(body.height.saturating_sub(1)),
                body.width,
                1.min(body.height.saturating_sub(rail_h + 1)),
            ),
            buf,
            ctx,
            bg,
        );
        let log_top = body.y + (rail_h + 1).min(body.height);
        let log_area = Rect::new(
            body.x,
            log_top,
            body.width,
            body.height.saturating_sub(log_top - body.y + 1),
        );
        self.viewport.render(log_area, buf, ctx, bg);
        let button_area = |width| {
            Rect::new(
                body.x + body.width.saturating_sub(width),
                log_area.bottom().saturating_sub(1).max(body.y),
                width,
                1,
            )
        };
        if matches!(self.state.outcome(), Some(ApplyOutcome::Success)) {
            let width = self.done.width().min(body.width);
            self.done.render(button_area(width), buf, ctx, bg);
        } else if self.state.outcome().is_none() && !self.state.cancelling() {
            let width = self.cancel.width().min(body.width);
            self.cancel.render(button_area(width), buf, ctx, bg);
        }
        if let Some(modal) = self.modals.top_mut() {
            modal.render(area, buf, ctx);
        }
        let mut hints = Vec::new();
        if self.state.outcome().is_none() && !self.state.cancelling() {
            hints.push(("Esc", "cancel"));
        }
        hints.extend([("↑↓", "scroll"), ("f", "follow")]);
        if self.state.rerun().is_some() {
            hints.push(("c", "copy rerun"));
        }
        if self.state.outcome().is_some() {
            hints.push(("Enter", "results"));
        } else if !self.state.started() {
            hints.push(("Enter", "start"));
        }
        render_hints(
            &hints,
            Rect::new(area.x, area.bottom().saturating_sub(2), area.width, 1),
            buf,
            &theme,
        );
        let mut status = format!("S3 · {activity}");
        if self.state.started() && self.state.outcome().is_none() {
            status.push_str(&format!(" · {}s", self.state.elapsed_seconds()));
        }
        if let Some(latest) = self.state.latest_output() {
            status.push_str(" · ");
            status.push_str(latest);
        }
        render_status(
            &status,
            Rect::new(area.x, area.bottom().saturating_sub(1), area.width, 1),
            buf,
            ctx,
        );
    }
}

fn view_line(line: &ApplyLine) -> Vec<ViewportSpan> {
    vec![ViewportSpan::new(
        line.text.clone(),
        match line.stream {
            Stream::Stdout => Tone::Normal,
            Stream::Stderr => Tone::Error,
            Stream::System => Tone::Secondary,
        },
    )]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::MiseBackend;
    use junie_tui::core::event::Key;
    use ratatui::crossterm::event::{KeyCode, KeyModifiers};

    fn screen() -> ApplyScreen {
        let backend =
            std::sync::Arc::new(MiseBackend::new(std::path::PathBuf::from("/tmp/emc-test")));
        let req = ApplyRequest {
            envs: vec!["terminal".to_string()],
            keep: vec!["brew:jq".to_string()],
            manager: None,
            skip_parts: Vec::new(),
            only_parts: Vec::new(),
            update: true,
            upgrade: true,
            dry_run: false,
        };
        ApplyScreen::new(backend, req)
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
    fn cancel_posts_cancelled_with_rerun() {
        let mut screen = screen();
        // A phase-1 header line sets the current phase without spawning.
        screen.on_external(&junie_tui::core::event::External::new(ApplyLine {
            seq: 1,
            stream: Stream::System,
            text: "phase 1: base: mise -C /tmp -E terminal bootstrap --skip packages".to_string(),
        }));
        assert_eq!(screen.map_key(&key(KeyCode::Esc)), Some(ApplyKey::Cancel));
        assert_eq!(
            screen.apply(ApplyKey::Cancel, &Clipboard::new()),
            ApplyAction::None
        );
        assert!(screen.cancel_armed());
        // Dismiss first: arming is reversible.
        assert_eq!(
            screen.apply(ApplyKey::Dismiss, &Clipboard::new()),
            ApplyAction::None
        );
        assert!(!screen.cancel_armed());
        assert_eq!(screen.outcome(), None);
        // Confirm: cancel wins deterministically with a phase-1 rerun.
        assert_eq!(
            screen.apply(ApplyKey::Cancel, &Clipboard::new()),
            ApplyAction::None
        );
        assert_eq!(
            screen.apply(ApplyKey::ConfirmCancel, &Clipboard::new()),
            ApplyAction::None
        );
        assert_eq!(screen.outcome(), Some(&ApplyOutcome::Cancelled));
        let rerun = screen.rerun().expect("cancel builds a rerun");
        assert!(
            rerun.contains("bootstrap --skip packages"),
            "rerun is exact: {rerun}"
        );
    }

    #[test]
    fn rerun_format_matches_backend_argv() {
        let mut screen = screen();
        screen.on_external(&junie_tui::core::event::External::new(ApplyLine {
            seq: 2,
            stream: Stream::System,
            text: "phase 2: packages: mise ...".to_string(),
        }));
        screen.confirm_cancel();
        let rerun = screen.rerun().expect("rerun after phase 2");
        assert!(
            rerun.contains("bootstrap packages apply brew:jq"),
            "phase-2 rerun is exact: {rerun}"
        );
        assert!(rerun.contains("-E terminal"), "envs ride along: {rerun}");
    }

    #[test]
    fn failed_outcome_opens_modal_with_phase_rerun() {
        use crate::backend::mock::MockBackend;
        let backend =
            std::sync::Arc::new(MiseBackend::new(std::path::PathBuf::from("/tmp/emc-test")));
        let failing = MockBackend::new().with_failure(ApplyPhase::Packages);
        let req = ApplyRequest {
            envs: vec!["terminal".to_string()],
            keep: vec!["brew:jq".to_string()],
            manager: None,
            skip_parts: Vec::new(),
            only_parts: Vec::new(),
            update: true,
            upgrade: true,
            dry_run: false,
        };
        let (tx, rx) = external_channel();
        let handle = failing.spawn_apply(&req, tx).expect("mock spawn");
        let outcome = handle.join();
        let mut screen = ApplyScreen::new(backend, req);
        for m in rx.drain() {
            screen.on_external(&m);
        }
        screen.on_outcome(outcome.clone());
        assert!(screen.modal_open());
        assert!(matches!(
            screen.outcome(),
            Some(ApplyOutcome::Failed { .. })
        ));
        let rerun = screen.rerun().expect("failed builds rerun");
        assert!(
            rerun.contains("packages apply"),
            "rerun targets phase 2: {rerun}"
        );
        assert_eq!(
            screen.map_key(&char_key('y')),
            Some(ApplyKey::Copy),
            "failure opens the modal, enabling y-to-copy"
        );
    }

    #[test]
    fn keys_match_section_3() {
        let screen = screen();
        assert_eq!(screen.map_key(&key(KeyCode::Esc)), Some(ApplyKey::Cancel));
        assert_eq!(screen.map_key(&char_key('c')), None);
        assert_eq!(screen.map_key(&char_key('y')), None);
        assert_eq!(screen.map_key(&key(KeyCode::Up)), None);
        assert_eq!(screen.map_key(&key(KeyCode::Down)), None);
        assert_eq!(
            screen.map_key(&key(KeyCode::Enter)),
            Some(ApplyKey::Start),
            "Enter starts the idle worker"
        );
    }
}
