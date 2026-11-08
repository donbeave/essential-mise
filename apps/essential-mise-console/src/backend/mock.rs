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

use super::{
    Action, ApplyHandle, ApplyLine, ApplyOutcome, ApplyPhase, ApplyRequest, Backend, BackendError,
    MiseBackend, PlanEntry, ResId, StatusSnapshot, Stream, snapshot_from_raw,
};

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
    status_snapshot: StatusSnapshot,
    origins: HashMap<ResId, Vec<String>>,
    fail: Option<ApplyPhase>,
    calls: Arc<Mutex<Vec<MockCall>>>,
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
        let status_snapshot = snapshot_from_raw(include_str!("../../tests/fixtures/status.json"));
        Self {
            plan_entries,
            status_snapshot,
            origins: HashMap::new(),
            fail: None,
            calls: Arc::new(Mutex::new(Vec::new())),
            argv: MiseBackend::new("/tmp/emc-mock-config"),
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

    /// Script `spawn_apply` to fail in this phase (exit-code goldens).
    /// A scripted failure wins over `dry_run`.
    pub fn with_failure(mut self, phase: ApplyPhase) -> Self {
        self.fail = Some(phase);
        self
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
    fn plan(&self, _envs: &[String], _jobs: Option<u64>) -> Result<Vec<PlanEntry>, BackendError> {
        self.record(MockCall::Plan);
        Ok(self.plan_entries.clone())
    }

    fn status(&self, _envs: &[String]) -> Result<StatusSnapshot, BackendError> {
        self.record(MockCall::Status);
        Ok(self.status_snapshot.clone())
    }

    fn spawn_apply(
        &self,
        req: &ApplyRequest,
        tx: ExternalSender,
    ) -> Result<ApplyHandle, BackendError> {
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
            text: "mock: apply preview (no subprocesses spawned)".to_string(),
        });
        let worker = std::thread::Builder::new()
            .name("emc-mock-apply".to_string())
            .spawn(|| ApplyOutcome::Success)
            .map_err(BackendError::Spawn)?;
        Ok(ApplyHandle {
            worker: Some(worker),
            cancel_flag: Arc::new(AtomicBool::new(false)),
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
