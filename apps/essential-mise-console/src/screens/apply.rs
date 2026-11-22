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

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::Color;
use termrock::core::event::{External, Key, Outcome};
use termrock::core::focus::{Focus, FocusRing};
use termrock::core::id::WidgetId;
use termrock::core::intent::Phase;
use termrock::core::pointer::{MouseTarget, PointerEvent};
use termrock::core::response::Response;
use termrock::runtime::{ExternalReceiver, external_channel};
use termrock::shell::clipboard::Clipboard;
use termrock::theme::{Theme, Tone};
use termrock::ui::ctx::RenderCtx;
use termrock::ui::modal::ModalStack;
use termrock::widgets::button::Button;
use termrock::widgets::dialog::{Dialog, DialogResult};
use termrock::widgets::notice::Notice;
use termrock::widgets::steps::{Step, StepRail, StepState};
use termrock::widgets::viewport::{Span as ViewportSpan, TextViewport, ViewportAction};

use super::chrome::render_step_header;
use super::paint::{self, Marks, bold, footer, primary_line, put, seg};
use crate::backend::{
    ApplyHandle, ApplyLine, ApplyOutcome, ApplyPhase, ApplyRequest, Backend, Stream,
};
use crate::followup::{self, FollowUp};

/// Sender-side log bound, mirroring the viewport `.max_lines(5_000)` cap.
pub const APPLY_LOG_CAP: usize = 5_000;
const APPLY_DRAIN_LIMIT: usize = 256;

/// Backend phase-header prefixes the screen watches to track the current
/// phase (same crate, co-evolving protocol).
pub const PHASE1_PREFIX: &str = "phase 1: base";
/// Phase-2 header prefix (see [`PHASE1_PREFIX`]).
pub const PHASE2_PREFIX: &str = "phase 2: packages";

/// Key intent on the apply screen. Viewport navigation is routed directly to
/// the retained `TextViewport` by the screen owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyKey {
    /// Copy the command that Try again runs.
    Copy,
    /// Open the confirm that asks before stopping.
    Cancel,
    /// Confirm stopping.
    ConfirmCancel,
    /// Dismiss the confirm.
    Dismiss,
}

/// Screen-level action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyAction {
    /// State changed; stay on screen.
    None,
    /// Retry the apply from the failed phase.
    Retry,
    /// Review the remaining packages.
    ReviewPackages,
    /// Leave the console.
    Quit,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AdministratorAccess {
    Pending,
    Confirmed,
    Failed,
}

/// S3 apply state machine.
pub struct ApplyScreen {
    backend: std::sync::Arc<dyn Backend>,
    req: ApplyRequest,
    handle: Option<ApplyHandle>,
    receiver: Option<ExternalReceiver>,
    lines: VecDeque<ApplyLine>,
    dropped: u64,
    follow_ups: Vec<FollowUp>,
    administrator: AdministratorAccess,
    current_phase: ApplyPhase,
    phases_done: usize,
    phases_total: usize,
    started_at: Option<std::time::Instant>,
    outcome: Option<ApplyOutcome>,
    rerun: Option<String>,
    log_path: Option<std::path::PathBuf>,
    cancel_armed: bool,
    cancel_requested: bool,
}

impl ApplyScreen {
    /// Build for a request. Phase count follows the keep-list: empty means
    /// phase 2 is skipped (a bare `packages apply` means *every* package).
    pub fn new(backend: std::sync::Arc<dyn Backend>, req: ApplyRequest) -> Self {
        let run_base = req.run_base;
        let packages_phase = !req.keep.is_empty() || req.manager.is_some();
        let phases_total = usize::from(run_base) + usize::from(packages_phase);
        Self {
            backend,
            req,
            handle: None,
            receiver: None,
            lines: VecDeque::new(),
            dropped: 0,
            follow_ups: Vec::new(),
            administrator: AdministratorAccess::Pending,
            current_phase: if run_base {
                ApplyPhase::Base
            } else {
                ApplyPhase::Packages
            },
            phases_done: 0,
            phases_total,
            started_at: None,
            outcome: None,
            rerun: None,
            log_path: None,
            cancel_armed: false,
            cancel_requested: false,
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
                self.log_path = Some(handle.log_path().to_path_buf());
                self.handle = Some(handle);
                self.receiver = Some(rx);
            }
            Err(e) => {
                self.on_outcome(ApplyOutcome::Failed {
                    code: None,
                    detail: format!("apply failed to start: {e}"),
                    phase: ApplyPhase::Base,
                });
            }
        }
    }

    /// Whether the worker has been started.
    pub fn started(&self) -> bool {
        self.started_at.is_some()
    }

    /// Preserve the request so a cancelled sudo prompt can be retried.
    pub fn authentication_failed(&mut self, detail: String) {
        self.administrator = AdministratorAccess::Failed;
        let text = format!("administrator access failed: {detail}");
        self.lines.push_back(ApplyLine {
            seq: 1,
            stream: Stream::System,
            text: text.clone(),
        });
        self.on_outcome(ApplyOutcome::Failed {
            code: None,
            detail: text,
            phase: ApplyPhase::Base,
        });
    }

    /// Drain pending lines from the worker channel (no-op before start).
    pub fn drain(&self) -> Vec<termrock::core::event::External> {
        self.receiver
            .as_ref()
            .map(|rx| {
                std::iter::from_fn(|| rx.try_recv())
                    .take(APPLY_DRAIN_LIMIT)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The worker channel for the runtime auto-drain.
    pub fn receiver(&self) -> Option<&ExternalReceiver> {
        self.receiver.as_ref()
    }

    /// Consume one drained event; true when it was an apply line.
    /// Mirrors `app.rs::on_external`: downcast, dispatch, ignore the rest.
    pub fn on_external(&mut self, external: &termrock::core::event::External) -> bool {
        match external.downcast_ref::<ApplyLine>() {
            Some(line) => {
                self.push_line(line.clone());
                true
            }
            None => false,
        }
    }

    /// Map a key to an intent. While the apply runs, Esc, q and Ctrl+C ask
    /// before stopping; once it has stopped, `c` copies the command.
    pub fn map_key(&self, key: &termrock::core::event::Key) -> Option<ApplyKey> {
        use ratatui::crossterm::event::KeyCode;
        if (key.is(KeyCode::Esc) || key.is_char('q') || key.ctrl_char('c'))
            && self.outcome.is_none()
            && !self.cancel_requested
        {
            return Some(ApplyKey::Cancel);
        }
        if key.is_char('c') && self.rerun.is_some() {
            return Some(ApplyKey::Copy);
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
                ApplyAction::None
            }
        }
    }

    fn worker_finished(&self) -> bool {
        self.handle.as_ref().is_some_and(ApplyHandle::is_finished)
    }

    fn join_finished_worker(&mut self) -> Option<ApplyOutcome> {
        let handle = self.handle.take()?;
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

    /// The request this screen runs.
    pub fn request(&self) -> &ApplyRequest {
        &self.req
    }

    pub(crate) fn retry_request(&self) -> ApplyRequest {
        let mut request = self.request().clone();
        if self.packages_failed() {
            request.run_base = false;
        }
        request
    }

    pub(crate) fn can_review_packages(&self) -> bool {
        self.packages_failed() && self.req.manager.is_none()
    }

    fn packages_failed(&self) -> bool {
        matches!(
            self.outcome,
            Some(ApplyOutcome::Failed {
                phase: ApplyPhase::Packages,
                ..
            })
        )
    }

    pub(crate) fn failure_detail(&self) -> Option<&str> {
        match &self.outcome {
            Some(ApplyOutcome::Failed { detail, .. }) if !detail.is_empty() => Some(detail),
            _ => None,
        }
    }

    pub(crate) fn exit_summary(&self) -> Vec<String> {
        let Some(banner) = self.banner() else {
            return Vec::new();
        };
        let mut lines = vec![banner];
        if let Some(detail) = self.failure_detail() {
            lines.push(detail.to_owned());
        }
        if let Some(command) = self.rerun() {
            lines.push("To try again:".to_string());
            lines.push(format!("  $ {command}"));
        }
        if let Some(path) = &self.log_path {
            lines.push(format!("Full apply log: {}", path.display()));
        }
        lines
    }

    pub(crate) fn take_follow_ups(&mut self) -> Vec<FollowUp> {
        std::mem::take(&mut self.follow_ups)
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

    /// Human-readable current activity.
    pub fn activity(&self) -> String {
        match self.outcome {
            Some(ApplyOutcome::Success) => "Applied".to_string(),
            Some(ApplyOutcome::Failed { .. }) => "Stopped".to_string(),
            Some(ApplyOutcome::Cancelled) => "Stopped".to_string(),
            None if self.cancelling() => "Stopping".to_string(),
            None => match self.current_phase {
                ApplyPhase::Base => "Applying modules and settings".to_string(),
                ApplyPhase::Packages => "Installing packages".to_string(),
            },
        }
    }

    /// Plain-word label of the phase that stopped.
    fn phase_label(phase: ApplyPhase) -> &'static str {
        match phase {
            ApplyPhase::Base => "Modules and settings",
            ApplyPhase::Packages => "Packages",
        }
    }

    /// The banner shown once the apply has stopped early.
    pub fn banner(&self) -> Option<String> {
        match &self.outcome {
            Some(ApplyOutcome::Failed { code, phase, .. }) => {
                let admin = self.administrator == AdministratorAccess::Failed;
                let place = if admin {
                    "Administrator access"
                } else {
                    Self::phase_label(*phase)
                };
                Some(match code {
                    Some(code) => format!("Stopped at: {place} (exit code {code})"),
                    None => format!("Stopped at: {place}"),
                })
            }
            Some(ApplyOutcome::Cancelled) => {
                Some("Stopped. Some changes may already have been made.".to_string())
            }
            _ => None,
        }
    }

    /// The step rail in plain words: administrator access, modules and
    /// settings, packages, then the check of the result.
    pub fn rail(&self) -> Vec<(&'static str, StepState)> {
        let stopped = matches!(
            self.outcome,
            Some(ApplyOutcome::Failed { .. } | ApplyOutcome::Cancelled)
        );
        let failed = matches!(self.outcome, Some(ApplyOutcome::Failed { .. }));
        let success = matches!(self.outcome, Some(ApplyOutcome::Success));
        let admin = if self.administrator == AdministratorAccess::Failed {
            StepState::Failed
        } else if self.administrator == AdministratorAccess::Confirmed || success {
            StepState::Done
        } else if stopped {
            StepState::Blocked
        } else {
            StepState::Running
        };
        let phase_state = |phase: ApplyPhase, planned: bool, done_at: usize| {
            if !planned {
                StepState::Skipped
            } else if success || self.phases_done >= done_at {
                StepState::Done
            } else if stopped && self.current_phase == phase {
                if failed && admin != StepState::Failed {
                    StepState::Failed
                } else {
                    StepState::Blocked
                }
            } else if stopped || admin != StepState::Done {
                StepState::Queued
            } else if self.current_phase == phase {
                StepState::Running
            } else {
                StepState::Queued
            }
        };
        let packages_planned = !self.req.keep.is_empty() || self.req.manager.is_some();
        let base_done_at = 1;
        let packages_done_at = usize::from(self.req.run_base) + 1;
        vec![
            ("Administrator access", admin),
            (
                "Modules and settings",
                phase_state(ApplyPhase::Base, self.req.run_base, base_done_at),
            ),
            (
                "Packages",
                phase_state(ApplyPhase::Packages, packages_planned, packages_done_at),
            ),
            (
                "Check result",
                if success {
                    StepState::Running
                } else {
                    StepState::Queued
                },
            ),
        ]
    }

    /// Seconds since the worker started.
    pub fn elapsed_seconds(&self) -> u64 {
        self.started_at
            .map(|started| started.elapsed().as_secs())
            .unwrap_or(0)
    }

    /// Elapsed time as `mm:ss`.
    pub fn elapsed_text(&self) -> String {
        let secs = self.elapsed_seconds();
        format!("{:02}:{:02}", secs / 60, secs % 60)
    }

    /// Most recent worker output, including backend narration.
    pub fn latest_output(&self) -> Option<&str> {
        self.lines.back().map(|line| line.text.as_str())
    }

    /// Copy the rerun command through the shell clipboard (OSC52).
    pub fn copy_rerun(&self, clipboard: &Clipboard) -> Option<termrock::shell::clipboard::Copied> {
        self.rerun.as_deref().map(|r| clipboard.copy(r))
    }

    fn push_line(&mut self, line: ApplyLine) {
        followup::record_log_line(&mut self.follow_ups, &line.text);
        if line.stream == Stream::System {
            if line.text == "administrator access confirmed"
                || line.text.starts_with(PHASE1_PREFIX)
                || line.text.starts_with(PHASE2_PREFIX)
            {
                self.administrator = AdministratorAccess::Confirmed;
            } else if line.text.starts_with("administrator access expired;")
                || line.text.starts_with("administrator access failed:")
                || line.text.starts_with("cannot check administrator access:")
                || line
                    .text
                    .starts_with("cannot keep administrator access active:")
            {
                self.administrator = AdministratorAccess::Failed;
            }
            if line.text.starts_with(PHASE2_PREFIX) {
                self.current_phase = ApplyPhase::Packages;
                self.phases_done = self.phases_done.max(usize::from(self.req.run_base));
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
                    ApplyPhase::Packages => usize::from(self.req.run_base),
                    ApplyPhase::Base => 0,
                };
            }
            ApplyOutcome::Cancelled => {
                self.phases_done = match self.current_phase {
                    ApplyPhase::Packages => usize::from(self.req.run_base),
                    ApplyPhase::Base => 0,
                };
            }
        }
        self.outcome = Some(outcome);
        if self.outcome != Some(ApplyOutcome::Success) {
            self.rerun = Some(self.rerun_command());
        }
    }

    /// Format the same request as Try again.
    fn rerun_command(&self) -> String {
        crate::reapply::command_line(&crate::reapply::commands(
            &*self.backend,
            &self.retry_request(),
        ))
    }
}

const S3_LOG: WidgetId = WidgetId::of("emc.s3.log");
const S3_CANCEL_CONFIRM: WidgetId = WidgetId::of("emc.s3.cancel_confirm");
const S3_REVIEW: WidgetId = WidgetId::of("emc.s3.review");

pub(crate) struct ApplyOwner {
    pub(crate) state: ApplyScreen,
    pub(crate) viewport: TextViewport,
    modals: ModalStack<Box<Dialog>>,
    rail: StepRail,
}

impl ApplyOwner {
    pub(crate) fn new(state: ApplyScreen) -> Self {
        Self {
            rail: StepRail::new(
                WidgetId::of("emc.s3.steps"),
                state
                    .rail()
                    .into_iter()
                    .map(|(label, status)| {
                        let mut step = Step::new(label);
                        step.state = status;
                        step
                    })
                    .collect(),
            )
            .selectable(false),
            state,
            viewport: TextViewport::new(S3_LOG)
                .wrap(true)
                .max_lines(APPLY_LOG_CAP),
            modals: ModalStack::new(),
        }
    }

    pub(crate) fn primary_focus(&self) -> WidgetId {
        if self.modals.top().is_some() {
            return S3_CANCEL_CONFIRM;
        }
        S3_LOG
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

    pub(crate) fn poll_worker(&mut self) -> (Outcome, Option<ApplyOutcome>) {
        if !self.state.worker_finished() {
            return (Outcome::Ignored, None);
        }
        let pending = self.state.drain();
        for event in &pending {
            self.on_external(event);
        }
        if pending.len() == APPLY_DRAIN_LIMIT {
            return (Outcome::Changed, None);
        }
        (Outcome::Changed, self.state.join_finished_worker())
    }

    /// Open the "Stop applying?" question when the screen asked for it.
    pub(crate) fn sync_modal_from_domain(&mut self, focus: &mut Focus) -> Outcome {
        if !self.modals.is_empty() || !self.state.cancel_armed() {
            return Outcome::Ignored;
        }
        let dialog = Dialog::destructive(
            S3_CANCEL_CONFIRM,
            "Stop applying?",
            "Some changes may already have been made. Applying again is safe: it only changes what differs.",
            "Stop",
        );
        let initial = dialog.initial_focus;
        self.modals.push(Box::new(dialog), focus, Some(initial));
        Outcome::Changed
    }

    /// What the footer and the help overlay list for the current state.
    pub(crate) fn hints(&self) -> Vec<(&'static str, String)> {
        if self.layer_open() {
            return vec![("Enter", "choose".into()), ("Esc", "keep going".into())];
        }
        let mut hints: Vec<(&'static str, String)> =
            vec![("↑↓", "scroll".into()), ("f", "follow".into())];
        if self.state.outcome().is_some() {
            if self.state.rerun().is_some() {
                hints.push(("c", "copy command".into()));
            }
            hints.push(("q", "quit".into()));
        } else {
            hints.push(("Esc", "stop (asks first)".into()));
        }
        hints.push(("?", "help".into()));
        hints
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
        let Some(dialog) = self.modals.top_mut() else {
            return Response::ignored();
        };
        match dialog.result.take() {
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
        }
    }

    pub(crate) fn key(
        &mut self,
        key: &Key,
        focus: &mut Focus,
        clipboard: &Clipboard,
    ) -> Response<ApplyAction> {
        if let Some(dialog) = self.modals.top_mut() {
            let mut modal_ring = FocusRing::default();
            for action in dialog.actions.iter().filter(|action| action.can_activate()) {
                modal_ring.register(action.id);
            }
            let out = dialog.on_key(key, focus, &modal_ring);
            let response = self.take_modal_result(focus, clipboard);
            return response.merge(out.into());
        }
        if self.state.outcome().is_some() {
            if key.is(KeyCode::Enter) {
                return Response::emit(ApplyAction::Retry);
            }
            if key.is(KeyCode::Esc) && self.state.can_review_packages() {
                return Response::emit(ApplyAction::ReviewPackages);
            }
            if key.is_char('q') || key.is(KeyCode::Esc) {
                return Response::emit(ApplyAction::Quit);
            }
            if key.is_char('c') && self.state.rerun().is_some() {
                self.state.apply(ApplyKey::Copy, clipboard);
                return Outcome::Changed.into();
            }
        } else if self.state.map_key(key) == Some(ApplyKey::Cancel) {
            return self.arm_cancel(focus, clipboard).into();
        }
        let (out, action) = self.viewport.on_key(key);
        if let Some(ViewportAction::Copy(text)) = action {
            clipboard.copy(&text);
        }
        out.into()
    }

    pub(crate) fn pointer(
        &mut self,
        ev: &PointerEvent,
        focus: &mut Focus,
        clipboard: &Clipboard,
    ) -> Response<ApplyAction> {
        if let Some(dialog) = self.modals.top_mut() {
            let response = dialog.pointer_in(ev, focus);
            let result = self.take_modal_result(focus, clipboard);
            return result.merge(response.outcome().into());
        }
        if self.state.can_review_packages() && ev.is(Phase::Click) && ev.on(S3_REVIEW) {
            return Response::emit(ApplyAction::ReviewPackages);
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
        if self.modals.top().is_none() {
            return Response::consumed();
        }
        self.modals.pop(focus);
        self.state.apply(ApplyKey::Dismiss, clipboard);
        Response::changed()
    }

    pub(crate) fn draw(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let theme: Theme = *ctx.theme;
        let bg: Color = theme.canvas;
        let marks = Marks::for_theme(&theme);
        paint::clear(buf, area, bg);
        render_step_header(
            3,
            "Applying",
            Rect::new(area.x, area.y, area.width, 1),
            buf,
            &theme,
        );
        let width = area.width.saturating_sub(2);
        let tick = ctx.interaction.tick;
        let stopped = self.state.banner();
        // Line 1: what is happening, with the elapsed time on the right.
        let (lead, tone) = match &stopped {
            Some(text) => (format!("{} {text}", marks.fail), Tone::Error),
            None => (
                "Setting up your Mac. Keep this window open.".to_string(),
                Tone::Normal,
            ),
        };
        let elapsed = format!("Elapsed {}", self.state.elapsed_text());
        let mut row = vec![bold(lead.clone(), tone)];
        let used = lead.chars().count() + 1;
        if used + elapsed.chars().count() + 2 <= width as usize {
            let pad = width as usize - used - elapsed.chars().count();
            row.push(seg(" ".repeat(pad), Tone::Normal));
            row.push(seg(elapsed, Tone::Muted));
        }
        put(buf, area.x + 1, area.y + 1, width, &row, &theme, bg);
        for (index, (_, status)) in self.state.rail().into_iter().enumerate() {
            self.rail.set_state(index, status);
        }
        self.rail
            .render(Rect::new(area.x + 1, area.y + 2, width, 4), buf, ctx, bg);
        let diagnostic_y = area.y + 6;
        let log_y = if let Some(detail) = self.state.failure_detail() {
            let notice = Notice::error(detail);
            let height = notice.height(width);
            notice.render(
                Rect::new(area.x + 1, diagnostic_y, width, height),
                buf,
                ctx,
                bg,
            );
            diagnostic_y + height
        } else if stopped.is_none() {
            let mut current = format!("{} {}", marks.frame(tick), self.state.activity());
            if let Some(text) = self.state.latest_output() {
                current.push_str(": ");
                current.push_str(text);
            }
            put(
                buf,
                area.x + 1,
                diagnostic_y,
                width,
                &[seg(
                    paint::fit(&current, width as usize, marks.ellipsis),
                    Tone::Secondary,
                )],
                &theme,
                bg,
            );
            diagnostic_y + 1
        } else {
            diagnostic_y
        };
        let primary_y = area.bottom().saturating_sub(2);
        let details = Rect::new(area.x, log_y, area.width, primary_y.saturating_sub(log_y));
        let inner = paint::framed(details, buf, &theme, bg, "Details");
        self.viewport.render(inner, buf, ctx, bg);
        let primary = Rect::new(area.x, primary_y, area.width, 1);
        if self.state.outcome().is_some() {
            if self.state.outcome() != Some(&ApplyOutcome::Success) {
                self.draw_recovery_actions(primary, buf, ctx, bg);
            }
        } else {
            primary_line(primary, buf, ctx, bg, "Esc", "Stop (asks first)");
        }
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

    fn draw_recovery_actions(&self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, bg: Color) {
        let mut retry = Button::primary(paint::PRIMARY, "Enter  Try again").focusable(false);
        let retry_width = retry.width().min(area.width);
        retry.render(Rect::new(area.x, area.y, retry_width, 1), buf, ctx, bg);
        if self.state.can_review_packages() {
            let x = (area.x + retry_width + 1).min(area.right());
            Button::secondary(S3_REVIEW, "Esc  Review packages")
                .focusable(false)
                .render(Rect::new(x, area.y, area.right() - x, 1), buf, ctx, bg);
        }
    }
}

fn view_line(line: &ApplyLine) -> Vec<ViewportSpan> {
    vec![ViewportSpan::new(
        line.text.clone(),
        match line.stream {
            Stream::Stdout => Tone::Normal,
            Stream::Stderr if looks_like_failure(&line.text) => Tone::Error,
            Stream::Stderr => Tone::Normal,
            Stream::System => Tone::Secondary,
        },
    )]
}

/// mise and Homebrew write ordinary progress to stderr, so only lines that
/// say something went wrong are painted as errors.
fn looks_like_failure(text: &str) -> bool {
    let lower = text.to_lowercase();
    ["error", "failed", "fatal", "denied", "cannot", "could not"]
        .iter()
        .any(|word| lower.contains(word))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_failure_like_stderr_lines_are_painted_as_errors() {
        let line = |text: &str| ApplyLine {
            seq: 0,
            stream: Stream::Stderr,
            text: text.to_string(),
        };
        assert_eq!(
            view_line(&line("mise brew:bat download bat-0.26.1.tar.gz"))[0].tone,
            Tone::Normal
        );
        assert_eq!(
            view_line(&line("mise ERROR: could not link bat"))[0].tone,
            Tone::Error
        );
    }
    use crate::backend::MiseBackend;
    use ratatui::crossterm::event::{KeyCode, KeyModifiers};
    use termrock::core::event::Key;

    fn screen() -> ApplyScreen {
        let backend =
            std::sync::Arc::new(MiseBackend::new(std::path::PathBuf::from("/tmp/emc-test")));
        let req = ApplyRequest {
            envs: vec!["terminal".to_string()],
            keep: vec!["brew:jq".to_string()],
            manager: None,
            skip_parts: Vec::new(),
            only_parts: Vec::new(),
            run_base: true,
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
        screen.on_external(&termrock::core::event::External::new(ApplyLine {
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
        assert_eq!(
            screen.banner().as_deref(),
            Some("Stopped. Some changes may already have been made.")
        );
    }

    #[test]
    fn rerun_format_matches_backend_argv() {
        let mut screen = screen();
        screen.on_external(&termrock::core::event::External::new(ApplyLine {
            seq: 2,
            stream: Stream::System,
            text: "phase 2: packages: mise ...".to_string(),
        }));
        screen.confirm_cancel();
        let rerun = screen.rerun().expect("rerun after phase 2");
        assert!(
            rerun.contains("bootstrap packages apply brew:jq"),
            "the rerun covers every phase of the request: {rerun}"
        );
        assert!(rerun.contains("-E terminal"), "envs ride along: {rerun}");
    }

    #[test]
    fn failed_outcome_names_where_it_stopped_and_offers_the_command() {
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
            run_base: true,
            update: true,
            upgrade: true,
            dry_run: false,
        };
        let (tx, rx) = external_channel();
        let handle = failing.spawn_apply(&req, tx).expect("mock spawn");
        let outcome = handle.join();
        let mut screen = ApplyScreen::new(backend, req);
        while let Some(m) = rx.try_recv() {
            screen.on_external(&m);
        }
        screen.on_outcome(outcome.clone());
        assert_eq!(
            screen.banner().as_deref(),
            Some("Stopped at: Packages (exit code 1)")
        );
        assert!(matches!(
            screen.outcome(),
            Some(ApplyOutcome::Failed { .. })
        ));
        let rerun = screen.rerun().expect("failed builds rerun");
        assert!(
            rerun.contains("packages apply"),
            "rerun targets phase 2: {rerun}"
        );
        assert_eq!(screen.map_key(&char_key('c')), Some(ApplyKey::Copy));
        assert_eq!(screen.map_key(&char_key('y')), None);
    }

    #[test]
    fn keys_ask_before_stopping_and_nothing_is_dead() {
        let screen = screen();
        assert_eq!(screen.map_key(&key(KeyCode::Esc)), Some(ApplyKey::Cancel));
        assert_eq!(screen.map_key(&char_key('q')), Some(ApplyKey::Cancel));
        assert_eq!(screen.map_key(&char_key('c')), None, "nothing to copy yet");
        assert_eq!(screen.map_key(&key(KeyCode::Enter)), None);
    }

    #[test]
    fn administrator_progress_survives_log_eviction_and_package_sudo_errors() {
        let mut screen = screen();
        screen.push_line(ApplyLine {
            seq: 1,
            stream: Stream::System,
            text: "administrator access confirmed".into(),
        });
        screen.push_line(ApplyLine {
            seq: 2,
            stream: Stream::System,
            text: "phase 2: packages".into(),
        });
        for index in 0..=APPLY_LOG_CAP {
            screen.push_line(ApplyLine {
                seq: index as u64 + 3,
                stream: Stream::Stdout,
                text: "package output mentions sudo".into(),
            });
        }
        screen.on_outcome(ApplyOutcome::Failed {
            code: Some(1),
            detail: "package failed to run sudo".into(),
            phase: ApplyPhase::Packages,
        });
        let rail = screen.rail();
        assert_eq!(rail[0].1, StepState::Done);
        assert_eq!(rail[1].1, StepState::Done);
        assert_eq!(rail[2].1, StepState::Failed);
        assert_eq!(
            screen.banner().as_deref(),
            Some("Stopped at: Packages (exit code 1)")
        );
    }

    #[test]
    fn administrator_failure_is_retained_after_log_eviction() {
        let mut screen = screen();
        screen.authentication_failed("sudo: authentication cancelled".into());
        for index in 0..=APPLY_LOG_CAP {
            screen.push_line(ApplyLine {
                seq: index as u64 + 2,
                stream: Stream::Stdout,
                text: "additional output".into(),
            });
        }
        assert_eq!(screen.rail()[0].1, StepState::Failed);
        assert_eq!(
            screen.banner().as_deref(),
            Some("Stopped at: Administrator access")
        );
    }

    #[test]
    fn rail_uses_plain_words_and_marks_skipped_phases() {
        let mut screen = screen();
        let rail = screen.rail();
        let labels: Vec<&str> = rail.iter().map(|(l, _)| *l).collect();
        assert_eq!(
            labels,
            vec![
                "Administrator access",
                "Modules and settings",
                "Packages",
                "Check result"
            ]
        );
        screen.req.run_base = false;
        let skipped = screen.rail();
        assert_eq!(skipped[1].1, StepState::Skipped);
    }
}
