//! S3 Apply: live two-phase apply with progress, cancel, and rerun.
//!
//! State + logic live here; painting lands in T4.6. Event flow mirrors the
//! `app.rs`/`external.rs` precedent exactly: the worker streams
//! [`ApplyLine`](crate::backend::ApplyLine) values through [`ExternalSender`](junie_tui::runtime::ExternalSender),
//! the app drains the receiver per frame and feeds each as
//! `Input::External`, and the screen consumes them in [`on_external`](crate::screens::apply::ApplyScreen::on_external)
//! via `downcast_ref` (the `cockpit.rs` screen holds no channel code and is
//! not the pattern). Failure copy goes through the shell
//! [`Clipboard`](junie_tui::shell::clipboard::Clipboard) (OSC52), `y`
//! inside the modal and `c` outside; cancel runs a destructive confirm then
//! [`ApplyHandle::cancel`](crate::backend::ApplyHandle::cancel).
//! The queue is sender-side bounded ([`APPLY_LOG_CAP`](crate::screens::apply::APPLY_LOG_CAP)); overflow drops the
//! oldest lines and records a gap marker for the log pane.
//! Hook `PENDING` routes to [`ApplyScreen::note_hook_pending`](crate::screens::apply::ApplyScreen::note_hook_pending) and renders
//! `Pending` without touching the outcome (exit 0 preserved).

use std::collections::VecDeque;

use junie_tui::runtime::{ExternalReceiver, external_channel};
use junie_tui::shell::clipboard::Clipboard;
use junie_tui::widgets::steps::StepState;

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

/// Key intent on the apply screen (§3: `Esc`/`Ctrl-C` cancels, `↑`/`↓`
/// scroll the viewport, `c` copies, `y` copies inside the modal,
/// `Enter` continues when done).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyKey {
    /// Scroll the log up (viewport consumes at render).
    ScrollUp,
    /// Scroll the log down (viewport consumes at render).
    ScrollDown,
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
    /// Cancellation confirmed; outcome is [`ApplyOutcome::Cancelled`].
    Cancelled,
    /// Finished; continue to S4.
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
    outcome: Option<ApplyOutcome>,
    rerun: Option<String>,
    cancel_armed: bool,
    modal_open: bool,
    pending_hooks: Vec<String>,
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
            outcome: None,
            rerun: None,
            cancel_armed: false,
            modal_open: false,
            pending_hooks: Vec::new(),
        }
    }

    /// Start the worker if idle; the receiver stays on the screen for the
    /// drain pump. A spawn failure becomes a `Failed` outcome with the
    /// failure modal open (same path as a phase failure).
    pub fn start(&mut self) {
        if self.handle.is_some() {
            return;
        }
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

    /// Whether the worker was spawned.
    pub fn started(&self) -> bool {
        self.handle.is_some()
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

    /// Map a key to an intent. `↑`/`↓` stay visible for the viewport;
    /// `y` copies only inside the failure modal.
    pub fn map_key(&self, key: &junie_tui::core::event::Key) -> Option<ApplyKey> {
        use ratatui::crossterm::event::KeyCode;
        if key.is(KeyCode::Esc) {
            return Some(ApplyKey::Cancel);
        }
        if key.ctrl_char('c') {
            return Some(ApplyKey::Cancel);
        }
        if key.is(KeyCode::Up) {
            return Some(ApplyKey::ScrollUp);
        }
        if key.is(KeyCode::Down) {
            return Some(ApplyKey::ScrollDown);
        }
        if key.is_char('c') {
            return Some(ApplyKey::Copy);
        }
        if key.is_char('y') && self.modal_open {
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
            ApplyKey::ScrollUp | ApplyKey::ScrollDown => ApplyAction::None,
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
                ApplyAction::Cancelled
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

    /// Non-blocking completion check: joins a finished worker, records the
    /// outcome, and builds the exact rerun command.
    pub fn try_join(&mut self) -> Option<ApplyOutcome> {
        if let Some(outcome) = &self.outcome {
            return Some(outcome.clone());
        }
        let handle = self.handle.take()?;
        if !handle.is_finished() {
            self.handle = Some(handle);
            return None;
        }
        let outcome = handle.join();
        let ret = outcome.clone();
        self.on_outcome(outcome);
        Some(ret)
    }

    /// Whether the destructive cancel confirm is open.
    pub fn cancel_armed(&self) -> bool {
        self.cancel_armed
    }

    /// Whether the failure modal is open.
    pub fn modal_open(&self) -> bool {
        self.modal_open
    }

    /// Current outcome, if the worker finished or cancel was confirmed.
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

    /// `(done, total)` phases for the progress bar.
    pub fn progress(&self) -> (usize, usize) {
        (self.phases_done, self.phases_total)
    }

    /// Step-rail states: base, packages (skipped when single-phase), done.
    pub fn step_states(&self) -> Vec<(&'static str, StepState)> {
        let failed = matches!(self.outcome, Some(ApplyOutcome::Failed { .. }));
        let base = if failed && self.current_phase == ApplyPhase::Base {
            StepState::Failed
        } else if self.phases_done >= 1 || self.current_phase == ApplyPhase::Packages {
            StepState::Done
        } else if self.outcome.is_none() {
            StepState::Running
        } else {
            StepState::Done
        };
        let mut steps = vec![("base", base)];
        if self.phases_total > 1 {
            let packages = if failed && self.current_phase == ApplyPhase::Packages {
                StepState::Failed
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

    /// Route a hook `PENDING` report: renders `Pending`, never fails.
    pub fn note_hook_pending(&mut self, hook: impl Into<String>) {
        self.pending_hooks.push(hook.into());
    }

    /// Pending hooks (exit 0 preserved; installs untouched).
    pub fn pending_hooks(&self) -> &[String] {
        &self.pending_hooks
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
        if self.outcome.is_some() {
            return;
        }
        if let Some(handle) = self.handle.as_mut() {
            handle.cancel();
        }
        self.phases_done = match self.current_phase {
            ApplyPhase::Packages => 1,
            ApplyPhase::Base => 0,
        };
        self.rerun = Some(self.rerun_for(self.current_phase));
        self.outcome = Some(ApplyOutcome::Cancelled);
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
            ApplyOutcome::Cancelled => {}
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::MiseBackend;
    use junie_tui::core::event::Key;
    use ratatui::crossterm::event::{KeyCode, KeyModifiers};

    fn screen() -> ApplyScreen {
        let backend = std::sync::Arc::new(MiseBackend::new(
            std::path::PathBuf::from("/tmp/emc-test"),
            vec!["terminal".to_string()],
        ));
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
            ApplyAction::Cancelled
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
        let backend = std::sync::Arc::new(MiseBackend::new(
            std::path::PathBuf::from("/tmp/emc-test"),
            vec!["terminal".to_string()],
        ));
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
    fn hook_pending_preserves_exit_zero_shape() {
        let mut screen = screen();
        screen.note_hook_pending("post-packages hook: login shell");
        assert_eq!(screen.pending_hooks().len(), 1);
        screen.on_outcome(ApplyOutcome::Success);
        assert_eq!(screen.outcome(), Some(&ApplyOutcome::Success));
        assert_eq!(screen.progress(), (2, 2));
    }

    #[test]
    fn keys_match_section_3() {
        let screen = screen();
        assert_eq!(screen.map_key(&key(KeyCode::Esc)), Some(ApplyKey::Cancel));
        assert_eq!(screen.map_key(&char_key('c')), Some(ApplyKey::Copy));
        assert_eq!(screen.map_key(&char_key('y')), None);
        assert_eq!(screen.map_key(&key(KeyCode::Up)), Some(ApplyKey::ScrollUp));
        assert_eq!(
            screen.map_key(&key(KeyCode::Down)),
            Some(ApplyKey::ScrollDown)
        );
        assert_eq!(
            screen.map_key(&key(KeyCode::Enter)),
            Some(ApplyKey::Start),
            "Enter starts the idle worker"
        );
    }
}
