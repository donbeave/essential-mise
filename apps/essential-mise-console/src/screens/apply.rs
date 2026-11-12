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
use junie_tui::widgets::dialog::{Dialog, DialogResult};
use junie_tui::widgets::steps::StepState;
use junie_tui::widgets::viewport::{Span as ViewportSpan, TextViewport, ViewportAction};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::Color;

use super::chrome::render_step_header;
use super::paint::{self, Marks, bold, footer, primary_line, put, seg};
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
    /// Run the same request again.
    Retry,
    /// Leave the console.
    Quit,
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

    /// Map a key to an intent. While the apply runs, Esc, q and Ctrl+C ask
    /// before stopping; once it has stopped, `c` copies the command.
    pub fn map_key(&self, key: &junie_tui::core::event::Key) -> Option<ApplyKey> {
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

    /// The request this screen runs (Try again runs it again).
    pub fn request(&self) -> &ApplyRequest {
        &self.req
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
            Some(ApplyOutcome::Failed { code, tail, phase }) => {
                let admin = tail.contains("sudo") || self.admin_failed();
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

    fn admin_failed(&self) -> bool {
        self.lines.iter().any(|l| {
            l.text.contains("administrator access expired")
                || l.text.contains("cannot check administrator")
        })
    }

    fn admin_confirmed(&self) -> bool {
        self.lines.iter().any(|l| {
            l.text.contains("administrator access confirmed") || l.text.starts_with("phase ")
        })
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
        let admin = if self.admin_failed() {
            StepState::Failed
        } else if self.admin_confirmed() || success {
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
    pub fn copy_rerun(&self, clipboard: &Clipboard) -> Option<junie_tui::shell::clipboard::Copied> {
        self.rerun.as_deref().map(|r| clipboard.copy(r))
    }

    fn push_line(&mut self, line: ApplyLine) {
        if line.stream == Stream::System {
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
                self.rerun = Some(self.rerun_command());
            }
            ApplyOutcome::Cancelled => {
                self.phases_done = match self.current_phase {
                    ApplyPhase::Packages => usize::from(self.req.run_base),
                    ApplyPhase::Base => 0,
                };
                self.rerun = Some(self.rerun_command());
            }
        }
        self.outcome = Some(outcome);
    }

    /// The command Try again runs: every phase of this request, formatted
    /// from the same argv the backend executes.
    fn rerun_command(&self) -> String {
        crate::reapply::command_line(&crate::reapply::commands(&*self.backend, &self.req))
    }
}

const S3_LOG: WidgetId = WidgetId::of("emc.s3.log");
const S3_CANCEL_CONFIRM: WidgetId = WidgetId::of("emc.s3.cancel_confirm");

pub(crate) struct ApplyOwner {
    pub(crate) state: ApplyScreen,
    pub(crate) viewport: TextViewport,
    modals: ModalStack<Box<Dialog>>,
}

impl ApplyOwner {
    pub(crate) fn new(state: ApplyScreen) -> Self {
        Self {
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
        ring: &FocusRing,
        clipboard: &Clipboard,
    ) -> Response<ApplyAction> {
        if let Some(dialog) = self.modals.top_mut() {
            let out = dialog.on_key(key, focus, ring);
            let response = self.take_modal_result(focus, clipboard);
            let (result_out, action) = response.into_parts();
            return (out.or(result_out), action).into();
        }
        if self.state.outcome().is_some() {
            if key.is(KeyCode::Enter) {
                return Response::emit(ApplyAction::Retry);
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
            let (result_out, action) = result.into_parts();
            return (response.outcome().or(result_out), action).into();
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
        // Line 2: the step rail, each state a glyph and a word.
        let mut rail: Vec<paint::Seg> = Vec::new();
        for (label, state) in self.state.rail() {
            let (glyph, tone) = match state {
                StepState::Done => (marks.ok.to_string(), Tone::Success),
                StepState::Running => (marks.frame(tick).to_string(), Tone::Active),
                StepState::Failed => (marks.fail.to_string(), Tone::Error),
                StepState::Skipped => (marks.skip.to_string(), Tone::Muted),
                StepState::Blocked | StepState::Queued => ("○".to_string(), Tone::Muted),
            };
            let glyph = if theme.glyphs == junie_tui::theme::Glyphs::ASCII && glyph == "○" {
                "o".to_string()
            } else {
                glyph
            };
            rail.push(seg(format!("{glyph} "), tone));
            rail.push(seg(
                format!("{label}  "),
                if state == StepState::Skipped {
                    Tone::Muted
                } else {
                    Tone::Normal
                },
            ));
        }
        put(buf, area.x + 1, area.y + 2, width, &rail, &theme, bg);
        // Line 3: the current item.
        let current = match (&stopped, self.state.latest_output()) {
            (None, Some(text)) => {
                format!("{} {}: {text}", marks.frame(tick), self.state.activity())
            }
            (None, None) => format!("{} {}", marks.frame(tick), self.state.activity()),
            (Some(_), Some(text)) => format!("Last output: {text}"),
            (Some(_), None) => String::new(),
        };
        put(
            buf,
            area.x + 1,
            area.y + 3,
            width,
            &[seg(
                paint::fit(&current, width as usize, marks.ellipsis),
                Tone::Secondary,
            )],
            &theme,
            bg,
        );
        let box_h = area.height.saturating_sub(7);
        let details = Rect::new(area.x, area.y + 4, area.width, box_h);
        let title = "Details  f follow, arrows scroll";
        let inner = paint::framed(details, buf, &theme, bg, title);
        self.viewport.render(inner, buf, ctx, bg);
        let primary_y = area.bottom().saturating_sub(2);
        let primary = Rect::new(area.x, primary_y, area.width, 1);
        if self.state.outcome().is_some() {
            if self.state.outcome() != Some(&ApplyOutcome::Success) {
                primary_line(primary, buf, &theme, bg, "Enter", "Try again");
            }
        } else {
            primary_line(primary, buf, &theme, bg, "Esc", "Stop (asks first)");
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
        assert_eq!(
            screen.banner().as_deref(),
            Some("Stopped. Some changes may already have been made.")
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
        for m in rx.drain() {
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
