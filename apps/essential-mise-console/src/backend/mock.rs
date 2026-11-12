//! [`MockBackend`](crate::backend::mock::MockBackend): deterministic [`Backend`] over the T2.2 fixtures.
//!
//! Used by unit tests and, later, all T4 screenshot tests. Parses the exact
//! committed `tests/fixtures/plan.json` and `tests/fixtures/status.json`, so
//! mock behavior tracks the real schema.

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use junie_tui::runtime::ExternalSender;

use crate::status::StatusReport;

use super::{
    Action, ApplyHandle, ApplyLine, ApplyOutcome, ApplyPhase, ApplyRequest, Backend, BackendError,
    MiseBackend, PlanEntry, ResId, Stream,
};

/// Fixed check time shown by mock reports.
pub const MOCK_CHECKED_AT: &str = "12:04:31";

/// One recorded [`MockBackend`] call, for golden-test assertions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MockCall {
    /// `plan` was called.
    Plan,
    /// `status` was called.
    Status,
    /// `spawn_apply` was called; records whether it was a dry run.
    SpawnApply {
        /// The request's `dry_run` flag.
        dry_run: bool,
    },
    /// `origin_map` was called.
    OriginMap,
}

/// Deterministic backend parsing the committed fixtures.
#[derive(Debug, Clone)]
pub struct MockBackend {
    plan_entries: Vec<PlanEntry>,
    status_reports: Arc<Mutex<std::collections::VecDeque<StatusReport>>>,
    status_delay: std::time::Duration,
    status_error: Option<String>,
    log_lines: Vec<String>,
    origins: HashMap<ResId, Vec<String>>,
    fail: Option<ApplyPhase>,
    apply_delay: std::time::Duration,
    calls: Arc<Mutex<Vec<MockCall>>>,
    requests: Arc<Mutex<Vec<ApplyRequest>>>,
    argv: MiseBackend,
}

impl MockBackend {
    /// Build from the committed T2.2 fixtures.
    pub fn new() -> Self {
        let plan_doc: super::PlanDoc =
            serde_json::from_str(include_str!("../../tests/fixtures/plan.json"))
                .expect("plan fixture parses");
        let plan_entries = plan_doc
            .resources
            .into_iter()
            .map(|r| PlanEntry {
                kind: r.id.kind,
                name: r.id.name,
                current: r.current,
                desired: r.desired,
                action: Action::parse(&r.action),
            })
            .collect();
        let clean = StatusReport::parse(
            include_str!("../../tests/fixtures/status-applied.json"),
            MOCK_CHECKED_AT,
        )
        .expect("status fixture parses");
        Self {
            plan_entries,
            status_reports: Arc::new(Mutex::new(std::collections::VecDeque::from([clean]))),
            status_delay: std::time::Duration::ZERO,
            status_error: None,
            log_lines: Vec::new(),
            origins: HashMap::new(),
            fail: None,
            apply_delay: std::time::Duration::from_millis(25),
            calls: Arc::new(Mutex::new(Vec::new())),
            requests: Arc::new(Mutex::new(Vec::new())),
            argv: MiseBackend::new("/tmp/emc-mock-config").with_mise_bin("mise"),
        }
    }

    /// Record a call.
    fn record(&self, call: MockCall) {
        self.calls
            .lock()
            .expect("mock call log poisoned")
            .push(call);
    }

    /// All recorded calls, in order.
    pub fn calls(&self) -> Vec<MockCall> {
        self.calls.lock().expect("mock call log poisoned").clone()
    }

    /// Every apply request received, in order.
    pub fn requests(&self) -> Vec<ApplyRequest> {
        self.requests
            .lock()
            .expect("mock requests poisoned")
            .clone()
    }

    /// Script `spawn_apply` to fail in this phase (exit-code goldens).
    /// A scripted failure wins over `dry_run`.
    pub fn with_failure(mut self, phase: ApplyPhase) -> Self {
        self.fail = Some(phase);
        self
    }

    /// Keep the successful mock worker active long enough to exercise live UI.
    pub fn with_apply_delay(mut self, delay: std::time::Duration) -> Self {
        self.apply_delay = delay;
        self
    }

    /// Script the reports returned by successive `status` calls; the last one
    /// repeats. Lets a test show "needs changes" first and "up to date" after
    /// a re-apply.
    pub fn with_status_sequence(self, reports: Vec<StatusReport>) -> Self {
        assert!(!reports.is_empty(), "at least one scripted report");
        *self.status_reports.lock().expect("mock status poisoned") = reports.into();
        self
    }

    /// Make `status` take this long, so tests can observe the checking state.
    pub fn with_status_delay(mut self, delay: std::time::Duration) -> Self {
        self.status_delay = delay;
        self
    }

    /// Report the machine as needing one change (the `status-one-differs`
    /// fixture). With `then_clean`, the first check made after an apply
    /// reports it fixed.
    pub fn with_status_differs(self, then_clean: bool) -> Self {
        let load =
            |text: &str| StatusReport::parse(text, MOCK_CHECKED_AT).expect("status fixture parses");
        let differs = load(include_str!("../../tests/fixtures/status-one-differs.json"));
        // The Review check and the first Result check both see the change.
        let mut sequence = vec![differs.clone(), differs];
        if then_clean {
            sequence.push(load(include_str!(
                "../../tests/fixtures/status-applied.json"
            )));
        }
        self.with_status_sequence(sequence)
    }

    /// Make `status` fail with this message.
    pub fn with_status_error(mut self, message: &str) -> Self {
        self.status_error = Some(message.to_string());
        self
    }

    /// Emit these log lines from a successful apply, after the phase header.
    pub fn with_log_lines(mut self, lines: &[&str]) -> Self {
        self.log_lines = lines.iter().map(|l| (*l).to_string()).collect();
        self
    }

    /// The log lines a real apply prints when it changed macOS settings and
    /// a plugin hook declined to overwrite an install.
    pub fn with_follow_up_log(self) -> Self {
        self.with_log_lines(&[
            "bootstrap: follow-up - relaunch apps that read changed macOS defaults (killall Dock / Finder / SystemUIServer)",
            "PENDING example-plugin/Tool: concurrent native configuration writes can lose changes; existing installations were not changed",
        ])
    }

    /// Add a canned origin entry (screenshot tests populate per screen).
    pub fn with_origin(mut self, kind: &str, name: &str, envs: &[&str]) -> Self {
        self.origins.insert(
            ResId {
                kind: kind.to_string(),
                name: name.to_string(),
            },
            envs.iter().map(|e| e.to_string()).collect(),
        );
        self
    }
}

impl Default for MockBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl Backend for MockBackend {
    fn authenticate(&self) -> Result<(), BackendError> {
        Ok(())
    }

    fn plan(&self, _envs: &[String], _jobs: Option<u64>) -> Result<Vec<PlanEntry>, BackendError> {
        self.record(MockCall::Plan);
        Ok(self.plan_entries.clone())
    }

    fn status(&self, _envs: &[String]) -> Result<StatusReport, BackendError> {
        self.record(MockCall::Status);
        if !self.status_delay.is_zero() {
            std::thread::sleep(self.status_delay);
        }
        if let Some(message) = &self.status_error {
            return Err(BackendError::Parse(message.clone()));
        }
        let mut reports = self.status_reports.lock().expect("mock status poisoned");
        let report = if reports.len() > 1 {
            reports.pop_front().expect("non-empty")
        } else {
            reports.front().expect("non-empty").clone()
        };
        Ok(report)
    }

    fn spawn_apply(
        &self,
        req: &ApplyRequest,
        tx: ExternalSender,
    ) -> Result<ApplyHandle, BackendError> {
        self.requests
            .lock()
            .expect("mock requests poisoned")
            .push(req.clone());
        self.record(MockCall::SpawnApply {
            dry_run: req.dry_run,
        });
        static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let log_path = std::env::temp_dir().join(format!(
            "emc-mock-apply-{}-{}.log",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::File::create(&log_path).map_err(BackendError::Spawn)?;
        let fail = self.fail;
        if let Some(phase) = fail {
            tx.send(ApplyLine {
                seq: 1,
                stream: Stream::System,
                text: format!("mock: failing in {phase:?} (scripted)"),
            });
            let worker = std::thread::Builder::new()
                .name("emc-mock-apply".to_string())
                .spawn(move || ApplyOutcome::Failed {
                    code: Some(1),
                    tail: format!("mock: {phase:?} failed (scripted)"),
                    phase,
                })
                .map_err(BackendError::Spawn)?;
            return Ok(ApplyHandle {
                worker: Some(worker),
                cancel_flag: Arc::new(AtomicBool::new(false)),
                current: Arc::new(Mutex::new(None)),
                log_path,
            });
        }
        tx.send(ApplyLine {
            seq: 1,
            stream: Stream::System,
            text: "phase 1: base: mock: apply preview (no subprocesses spawned)".to_string(),
        });
        for (offset, text) in self.log_lines.iter().enumerate() {
            tx.send(ApplyLine {
                seq: 2 + offset as u64,
                stream: Stream::Stdout,
                text: text.clone(),
            });
        }
        let delay = self.apply_delay;
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let worker_flag = Arc::clone(&cancel_flag);
        let worker = std::thread::Builder::new()
            .name("emc-mock-apply".to_string())
            .spawn(move || {
                let started = std::time::Instant::now();
                while started.elapsed() < delay {
                    if worker_flag.load(Ordering::Relaxed) {
                        return ApplyOutcome::Cancelled;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                ApplyOutcome::Success
            })
            .map_err(BackendError::Spawn)?;
        Ok(ApplyHandle {
            worker: Some(worker),
            cancel_flag,
            current: Arc::new(Mutex::new(None)),
            log_path,
        })
    }

    fn origin_map(&self, _config_dir: &Path) -> Result<HashMap<ResId, Vec<String>>, BackendError> {
        self.record(MockCall::OriginMap);
        Ok(self.origins.clone())
    }

    fn phase1_argv(&self, req: &ApplyRequest) -> Vec<String> {
        self.argv.phase1_argv(req)
    }

    fn phase2_argv(&self, req: &ApplyRequest) -> Option<Vec<String>> {
        self.argv.phase2_argv(req)
    }

    fn mise_bin(&self) -> &Path {
        self.argv.mise_bin()
    }

    fn config_dir(&self) -> &Path {
        self.argv.config_dir()
    }
}
