//! Stock-`mise bootstrap` backend: read-only queries plus two-phase apply.
//!
//! Command construction follows locked input L6: `mise -C <config-dir> -E
//! <envs> bootstrap …` with `MISE_CEILING_PATHS=<config-dir>`. Phase 1 runs
//! `bootstrap --skip packages --upgrade --update`; phase 2 runs
//! `bootstrap packages apply <kept…>` and is skipped when the keep-list is
//! empty (a bare `packages apply` means *every* package).

use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use junie_tui::runtime::ExternalSender;

use crate::status::StatusReport;

/// A single planned resource change, from `mise bootstrap plan --json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanEntry {
    /// Resource kind (the `id.kind` field, e.g. `packages`).
    pub kind: String,
    /// Resource name (the `id.name` field).
    pub name: String,
    /// Currently observed state.
    pub current: String,
    /// Configured desired state.
    pub desired: String,
    /// What applying would do.
    pub action: Action,
}

/// Planned change kind. Unknown strings parse to [`Action::Unknown`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Resource would be created.
    Create,
    /// Resource would be updated.
    Update,
    /// Resource would be removed.
    Remove,
    /// Resource already matches (package rows only).
    Noop,
    /// Unrecognized action string.
    Unknown,
}

impl Action {
    /// Parse an action string tolerantly; anything unrecognized is [`Action::Unknown`].
    pub fn parse(s: &str) -> Self {
        match s {
            "create" => Self::Create,
            "update" => Self::Update,
            "remove" => Self::Remove,
            "noop" => Self::Noop,
            _ => Self::Unknown,
        }
    }

    /// Canonical string form (round-trips through [`Action::parse`]).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Update => "update",
            Self::Remove => "remove",
            Self::Noop => "noop",
            Self::Unknown => "unknown",
        }
    }
}

/// Which stream an [`ApplyLine`] arrived on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream {
    /// Child stdout.
    Stdout,
    /// Child stderr.
    Stderr,
    /// Backend-generated narration (phase headers, dry-run preview).
    System,
}

/// One streamed log line from a running apply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyLine {
    /// Monotonic sequence number within the apply.
    pub seq: u64,
    /// Origin stream.
    pub stream: Stream,
    /// Line text without the trailing newline.
    pub text: String,
}

/// Terminal state of an apply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyOutcome {
    /// Both phases finished with exit 0.
    Success,
    /// A phase failed; `tail` holds the last log lines for the modal.
    Failed {
        /// Failing exit code, when known.
        code: Option<i32>,
        /// Last log lines for the failure modal.
        tail: String,
        /// Which phase failed (drives the CLI 1/3 exit mapping).
        phase: ApplyPhase,
    },
    /// Cancelled via [`ApplyHandle::cancel`].
    Cancelled,
}

/// Which apply phase a line or outcome belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyPhase {
    /// Phase 1: base parts (`bootstrap --skip packages …`).
    Base,
    /// Phase 2: `bootstrap packages apply <kept…>`.
    Packages,
}

/// What to apply: selected envs plus the package keep-list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyRequest {
    /// Selected environments (passed as repeated `-E` flags).
    pub envs: Vec<String>,
    /// Kept packages as `manager:package`. Empty skips phase 2 entirely.
    pub keep: Vec<String>,
    /// Parts to skip in phase 1 (`--skip`). Mutually exclusive with `only_parts`.
    pub skip_parts: Vec<String>,
    /// Parts to run in phase 1 (`--only`). Mutually exclusive with `skip_parts`.
    pub only_parts: Vec<String>,
    /// Run phase 1 at all. A re-apply that only touches packages sets this
    /// to `false`.
    pub run_base: bool,
    /// Pass only packages for this manager to phase 2 (`packages apply -m`).
    pub manager: Option<String>,
    /// Pass `--update` (default true, matches `bootstrap.sh` step 6).
    pub update: bool,
    /// Pass `--upgrade` (default true, matches `bootstrap.sh` step 6).
    pub upgrade: bool,
    /// Preview only: emit the planned commands, spawn nothing.
    pub dry_run: bool,
}

/// Package identity; `origin_map` values list the declaring envs.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResId {
    /// Package manager (e.g. `brew`); `unknown` when the key has no colon.
    pub kind: String,
    /// Package name.
    pub name: String,
}

/// Backend failure modes.
#[derive(Debug)]
pub enum BackendError {
    /// Spawning or waiting on a child failed.
    Spawn(std::io::Error),
    /// Administrator authentication failed before an apply.
    Authentication(String),
    /// Output or config did not match the expected schema.
    Parse(String),
    /// A read-only query exceeded its deadline.
    Timeout,
}

impl std::fmt::Display for BackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Spawn(e) => write!(f, "failed to run mise: {e}"),
            Self::Authentication(e) => write!(f, "administrator authentication failed: {e}"),
            Self::Parse(e) => write!(f, "schema mismatch: {e}"),
            Self::Timeout => write!(f, "mise query timed out"),
        }
    }
}

impl std::error::Error for BackendError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Spawn(e) => Some(e),
            Self::Authentication(_) | Self::Parse(_) | Self::Timeout => None,
        }
    }
}

impl From<std::io::Error> for BackendError {
    fn from(e: std::io::Error) -> Self {
        Self::Spawn(e)
    }
}

/// Stock-`mise bootstrap` backend over a config dir.
pub trait Backend: Send + Sync + 'static {
    /// Authenticate with the user's terminal before starting an apply.
    fn authenticate(&self) -> Result<(), BackendError>;
    /// Run `bootstrap plan --json` for the requested envs.
    fn plan(&self, envs: &[String], jobs: Option<u64>) -> Result<Vec<PlanEntry>, BackendError>;
    /// Run `bootstrap status --json` for the requested envs and parse it.
    fn status(&self, envs: &[String]) -> Result<StatusReport, BackendError>;
    /// Run phase 1 then phase 2 in a worker thread, streaming lines to `tx`.
    fn spawn_apply(
        &self,
        req: &ApplyRequest,
        tx: ExternalSender,
    ) -> Result<ApplyHandle, BackendError>;
    /// Map each `manager:package` key in every `config.<env>.toml`
    /// `[bootstrap.packages]` table to its declaring envs.
    fn origin_map(&self, config_dir: &Path) -> Result<HashMap<ResId, Vec<String>>, BackendError>;
    /// Phase 1 argv fragment (shared with rerun formatting).
    fn phase1_argv(&self, req: &ApplyRequest) -> Vec<String>;
    /// Phase 2 argv fragment, or `None` when phase 2 is skipped.
    fn phase2_argv(&self, req: &ApplyRequest) -> Option<Vec<String>>;
    /// mise binary invoked for every child.
    fn mise_bin(&self) -> &Path;
    /// Config dir passed as `-C` and `MISE_CEILING_PATHS`.
    fn config_dir(&self) -> &Path;
}

fn default_mise_bin(home: Option<&Path>) -> PathBuf {
    home.and_then(|home| {
        let installed = home.join(".local/bin/mise");
        installed.is_file().then_some(installed)
    })
    .unwrap_or_else(|| PathBuf::from("mise"))
}

/// Live [`Backend`] over stock `mise bootstrap`.
#[derive(Debug, Clone)]
pub struct MiseBackend {
    /// Config dir passed as `-C` and `MISE_CEILING_PATHS`.
    config_dir: PathBuf,
    /// Mise binary installed by setup, or `mise` on `PATH` when absent.
    mise_bin: PathBuf,
}

impl MiseBackend {
    /// Build a backend for `config_dir`. Use `$HOME/.local/bin/mise` when
    /// setup installed it there; otherwise resolve `mise` through `PATH`.
    pub fn new(config_dir: impl Into<PathBuf>) -> Self {
        Self {
            config_dir: config_dir.into(),
            mise_bin: default_mise_bin(std::env::var_os("HOME").as_deref().map(Path::new)),
        }
    }

    /// Override the mise binary path.
    pub fn with_mise_bin(mut self, bin: impl Into<PathBuf>) -> Self {
        self.mise_bin = bin.into();
        self
    }

    /// Base `mise -C <dir> -E <envs> bootstrap` command per L6.
    fn base_command(&self, envs: &[String]) -> Command {
        let mut cmd = Command::new(&self.mise_bin);
        cmd.arg("-C");
        cmd.arg(&self.config_dir);
        for env in envs {
            cmd.arg("-E");
            cmd.arg(env);
        }
        cmd.arg("bootstrap");
        cmd.env("MISE_CEILING_PATHS", &self.config_dir);
        cmd
    }
}

/// Tolerant `plan --json` document: unknown fields ignored, all defaulted.
#[derive(Debug, Default, serde::Deserialize)]
struct PlanDoc {
    #[serde(default)]
    resources: Vec<PlanResource>,
}

/// Tolerant plan resource: entry keys exactly `{action,current,desired,id}`.
#[derive(Debug, Default, serde::Deserialize)]
struct PlanResource {
    #[serde(default)]
    id: PlanId,
    #[serde(default)]
    current: String,
    #[serde(default)]
    desired: String,
    #[serde(default)]
    action: String,
}

/// Tolerant resource id.
#[derive(Debug, Default, serde::Deserialize)]
struct PlanId {
    #[serde(default)]
    kind: String,
    #[serde(default)]
    name: String,
}

/// Tolerant `config.<env>.toml` document: only `[bootstrap.packages]` keys matter.
#[derive(Debug, Default, serde::Deserialize)]
struct EnvDoc {
    #[serde(default)]
    bootstrap: EnvBootstrap,
}

/// Tolerant `[bootstrap]` section.
#[derive(Debug, Default, serde::Deserialize)]
struct EnvBootstrap {
    /// Package keys mapped to arbitrary values (versions or tables); values ignored.
    #[serde(default)]
    packages: HashMap<String, toml::Value>,
}

/// Deadline for read-only queries.
/// Local wall-clock time as `HH:MM:SS` (from `date`, so the local time zone
/// applies without a time-zone dependency); UTC when `date` is unavailable.
fn wall_clock_hms() -> String {
    if let Ok(out) = Command::new("/bin/date").arg("+%H:%M:%S").output()
        && out.status.success()
    {
        let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if text.len() == 8 {
            return text;
        }
    }
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
        % 86_400;
    format!(
        "{:02}:{:02}:{:02}",
        secs / 3600,
        secs % 3600 / 60,
        secs % 60
    )
}

const QUERY_TIMEOUT: Duration = Duration::from_secs(300);

/// Run a command to completion with a deadline; kill on expiry.
fn run_with_timeout(
    cmd: &mut Command,
    timeout: Duration,
) -> Result<std::process::Output, BackendError> {
    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(BackendError::Spawn)?;
    let start = std::time::Instant::now();
    loop {
        match child.try_wait().map_err(BackendError::Spawn)? {
            Some(_) => {
                return child.wait_with_output().map_err(BackendError::Spawn);
            }
            None => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(BackendError::Timeout);
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

impl Backend for MiseBackend {
    fn authenticate(&self) -> Result<(), BackendError> {
        eprintln!("essential-mise-console: checking administrator access for installation");
        let status = Command::new("/usr/bin/sudo")
            .arg("-v")
            .status()
            .map_err(BackendError::Spawn)?;
        if status.success() {
            Ok(())
        } else {
            Err(BackendError::Authentication(format!(
                "sudo -v exited {status}; rerun the command and approve the terminal prompt"
            )))
        }
    }

    fn plan(&self, envs: &[String], jobs: Option<u64>) -> Result<Vec<PlanEntry>, BackendError> {
        let mut cmd = self.base_command(envs);
        cmd.arg("plan").arg("--json");
        if let Some(j) = jobs {
            cmd.arg("--jobs").arg(j.to_string());
        }
        let out = run_with_timeout(&mut cmd, QUERY_TIMEOUT)?;
        if !out.status.success() {
            return Err(BackendError::Parse(format!(
                "mise bootstrap plan exited {}: {}",
                out.status,
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
        // Tolerant: missing `resources` parses as empty; unknown actions map
        // to `Unknown` and never panic. The schema is pinned by T2.2 fixtures.
        let doc: PlanDoc = serde_json::from_slice(&out.stdout)
            .map_err(|e| BackendError::Parse(format!("plan --json: {e}")))?;
        Ok(doc
            .resources
            .into_iter()
            .map(|r| PlanEntry {
                kind: r.id.kind,
                name: r.id.name,
                current: r.current,
                desired: r.desired,
                action: Action::parse(&r.action),
            })
            .collect())
    }

    fn status(&self, envs: &[String]) -> Result<StatusReport, BackendError> {
        let mut cmd = self.base_command(envs);
        cmd.arg("status").arg("--json");
        let out = run_with_timeout(&mut cmd, QUERY_TIMEOUT)?;
        if !out.status.success() {
            return Err(BackendError::Parse(format!(
                "mise bootstrap status exited {}: {}",
                out.status,
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
        let raw = String::from_utf8_lossy(&out.stdout);
        StatusReport::parse(&raw, wall_clock_hms()).map_err(|e| BackendError::Parse(e.to_string()))
    }

    fn spawn_apply(
        &self,
        req: &ApplyRequest,
        tx: ExternalSender,
    ) -> Result<ApplyHandle, BackendError> {
        static LOG_SEQ: AtomicU64 = AtomicU64::new(0);
        let log_path = std::env::temp_dir().join(format!(
            "emc-apply-{}-{}.log",
            std::process::id(),
            LOG_SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        let log_file = std::fs::File::create(&log_path).map_err(BackendError::Spawn)?;

        let worker_req = req.clone();
        let worker_mise = self.mise_bin.clone();
        let worker_dir = self.config_dir.clone();
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let worker_flag = Arc::clone(&cancel_flag);
        let current: Arc<Mutex<Option<Child>>> = Arc::new(Mutex::new(None));
        let worker_current = Arc::clone(&current);

        let phase1 = req.run_base.then(|| self.phase1_argv(req));
        let phase2 = self.phase2_argv(req);

        let thread_name = format!("emc-apply-{}", worker_req.envs.join(","));
        let worker = std::thread::Builder::new()
            .name(thread_name)
            .spawn(move || {
                run_apply_worker(
                    worker_mise,
                    worker_dir,
                    worker_req,
                    phase1,
                    phase2,
                    tx,
                    log_file,
                    worker_flag,
                    worker_current,
                )
            })
            .map_err(BackendError::Spawn)?;

        Ok(ApplyHandle {
            worker: Some(worker),
            cancel_flag,
            current,
            log_path,
        })
    }

    fn origin_map(&self, config_dir: &Path) -> Result<HashMap<ResId, Vec<String>>, BackendError> {
        let mut paths = std::fs::read_dir(config_dir)
            .map_err(BackendError::Spawn)?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(BackendError::Spawn)?
            .into_iter()
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| {
                        name.strip_prefix("config.")
                            .and_then(|name| name.strip_suffix(".toml"))
                            .is_some_and(|env| !env.is_empty())
                    })
            })
            .collect::<Vec<_>>();
        paths.sort();

        let mut map: HashMap<ResId, Vec<String>> = HashMap::new();
        for path in paths {
            let env = path
                .file_name()
                .and_then(|name| name.to_str())
                .and_then(|name| name.strip_prefix("config."))
                .and_then(|name| name.strip_suffix(".toml"))
                .expect("filtered module config filename")
                .to_string();
            let text = std::fs::read_to_string(&path).map_err(BackendError::Spawn)?;
            let doc: EnvDoc = toml::from_str(&text)
                .map_err(|e| BackendError::Parse(format!("{}: {e}", path.display())))?;
            for key in doc.bootstrap.packages.keys() {
                let (kind, name) = match key.split_once(':') {
                    Some((k, n)) => (k.to_string(), n.to_string()),
                    None => ("unknown".to_string(), key.clone()),
                };
                let envs = map.entry(ResId { kind, name }).or_default();
                if !envs.contains(&env) {
                    envs.push(env.clone());
                }
            }
        }
        Ok(map)
    }

    fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    fn mise_bin(&self) -> &Path {
        &self.mise_bin
    }

    fn phase1_argv(&self, req: &ApplyRequest) -> Vec<String> {
        let mut argv = vec!["bootstrap".to_string()];
        if !req.only_parts.is_empty() {
            // `--only` cannot be combined with `--skip`; it already leaves
            // packages out unless they are named.
            argv.push("--only".to_string());
            argv.push(req.only_parts.join(","));
        } else {
            argv.push("--skip".to_string());
            let mut skipped = vec!["packages".to_string()];
            skipped.extend(req.skip_parts.iter().cloned());
            argv.push(skipped.join(","));
        }
        if req.update {
            argv.push("--update".to_string());
        }
        if req.upgrade {
            argv.push("--upgrade".to_string());
        }
        if req.dry_run {
            argv.push("--dry-run".to_string());
        }
        argv
    }

    fn phase2_argv(&self, req: &ApplyRequest) -> Option<Vec<String>> {
        if req.keep.is_empty() && req.manager.is_none() {
            return None;
        }
        let mut argv = vec![
            "bootstrap".to_string(),
            "packages".to_string(),
            "apply".to_string(),
        ];
        if let Some(ref m) = req.manager {
            argv.push("--manager".to_string());
            argv.push(m.clone());
        }
        if req.dry_run {
            argv.push("--dry-run".to_string());
        }
        argv.extend(req.keep.iter().cloned());
        Some(argv)
    }
}

/// Number of log tail lines kept for the failure modal.
const FAILURE_TAIL_LINES: usize = 20;

/// Worker body: noninteractive sudo validation, phase 1, phase 2, streaming + tee.
/// Interactive authentication belongs to [`Backend::authenticate`] before the
/// TUI starts. The `while /usr/bin/sudo -n -v; do /bin/sleep 60; done`
/// keepalive is killed on every exit path (done or cancelled).
#[allow(clippy::too_many_arguments)]
fn run_apply_worker(
    mise_bin: PathBuf,
    config_dir: PathBuf,
    req: ApplyRequest,
    phase1: Option<Vec<String>>,
    phase2: Option<Vec<String>>,
    tx: ExternalSender,
    log_file: std::fs::File,
    cancel: Arc<AtomicBool>,
    current: Arc<Mutex<Option<Child>>>,
) -> ApplyOutcome {
    use std::io::Write as _;
    let mut log = std::io::BufWriter::new(log_file);
    let mut seq: u64 = 0;
    let mut emit = |stream: Stream, text: String, log: &mut std::io::BufWriter<std::fs::File>| {
        seq += 1;
        let _ = writeln!(log, "[{stream:?}:{seq}] {text}");
        let _ = log.flush();
        tx.send(ApplyLine { seq, stream, text });
    };

    let cancelled = || cancel.load(Ordering::Relaxed);

    if req.dry_run {
        if let Some(ref p1) = phase1 {
            emit(
                Stream::System,
                format!(
                    "dry-run: {}",
                    display_argv(&mise_bin, &full_argv(&config_dir, &req, p1))
                ),
                &mut log,
            );
        }
        if let Some(ref p2) = phase2 {
            emit(
                Stream::System,
                format!(
                    "dry-run: {}",
                    display_argv(&mise_bin, &full_argv(&config_dir, &req, p2))
                ),
                &mut log,
            );
        } else {
            emit(
                Stream::System,
                "dry-run: keep-list empty, phase 2 skipped".to_string(),
                &mut log,
            );
        }
        return ApplyOutcome::Success;
    }

    emit(
        Stream::System,
        "checking administrator access".to_string(),
        &mut log,
    );
    let sudo_check = Command::new("/usr/bin/sudo")
        .args(["-n", "-v"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output();
    match sudo_check {
        Ok(output) if output.status.success() => {}
        Ok(output) => {
            let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
            emit(
                Stream::System,
                format!(
                    "administrator access expired; rerun the console and approve the terminal prompt{}",
                    if detail.is_empty() {
                        String::new()
                    } else {
                        format!(" ({detail})")
                    }
                ),
                &mut log,
            );
            return ApplyOutcome::Failed {
                code: output.status.code(),
                tail: "sudo -n -v failed; interactive authentication must happen before the TUI starts"
                    .to_string(),
                phase: ApplyPhase::Base,
            };
        }
        Err(e) => {
            emit(
                Stream::System,
                format!("cannot check administrator access: {e}"),
                &mut log,
            );
            return ApplyOutcome::Failed {
                code: None,
                tail: format!("failed to run sudo -n -v: {e}"),
                phase: ApplyPhase::Base,
            };
        }
    }
    emit(
        Stream::System,
        "administrator access confirmed".to_string(),
        &mut log,
    );
    let mut keepalive = Command::new("/bin/sh")
        .arg("-c")
        .arg("while /usr/bin/sudo -n -v; do /bin/sleep 60; done")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok();
    let finish = |keepalive: &mut Option<Child>| {
        if let Some(mut k) = keepalive.take() {
            let _ = k.kill();
            let _ = k.wait();
        }
    };

    let mut keepalive_guard = keepalive.take();
    if phase2.is_none() {
        emit(
            Stream::System,
            "keep-list empty, phase 2 skipped (bare `packages apply` means all)".to_string(),
            &mut log,
        );
    }
    let mut phases: Vec<(ApplyPhase, &str, Vec<String>)> = Vec::new();
    if let Some(ref p1) = phase1 {
        phases.push((
            ApplyPhase::Base,
            "phase 1: base",
            full_argv(&config_dir, &req, p1),
        ));
    }
    if let Some(ref p2) = phase2 {
        phases.push((
            ApplyPhase::Packages,
            "phase 2: packages",
            full_argv(&config_dir, &req, p2),
        ));
    }
    let outcome = run_phases(
        &mise_bin,
        &config_dir,
        phases,
        &mut emit,
        &mut log,
        &cancelled,
        &current,
    );
    finish(&mut keepalive_guard);
    outcome
}

/// Run phase 1 then phase 2, streaming lines and teeing to the log.
///
/// `Failed.tail` carries the last [`FAILURE_TAIL_LINES`] lines for the modal.
fn run_phases(
    mise_bin: &Path,
    config_dir: &Path,
    phases: Vec<(ApplyPhase, &str, Vec<String>)>,
    emit: &mut impl FnMut(Stream, String, &mut std::io::BufWriter<std::fs::File>),
    log: &mut std::io::BufWriter<std::fs::File>,
    cancelled: &impl Fn() -> bool,
    current: &Arc<Mutex<Option<Child>>>,
) -> ApplyOutcome {
    let tail: std::cell::RefCell<VecDeque<String>> =
        std::cell::RefCell::new(VecDeque::with_capacity(FAILURE_TAIL_LINES));
    let mut send = |stream: Stream, text: String, log: &mut std::io::BufWriter<std::fs::File>| {
        {
            let mut tail = tail.borrow_mut();
            if tail.len() == FAILURE_TAIL_LINES {
                tail.pop_front();
            }
            tail.push_back(text.clone());
        }
        emit(stream, text, log);
    };
    let last_lines = || {
        tail.borrow()
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join("\n")
    };
    for (phase, label, argv) in phases {
        send(Stream::System, format!("{label}: {}", argv.join(" ")), log);
        if cancelled() {
            return ApplyOutcome::Cancelled;
        }
        match stream_child(mise_bin, &argv, config_dir, &mut send, log, current) {
            Ok(Some(code)) => {
                return ApplyOutcome::Failed {
                    code: Some(code),
                    tail: format!("{label} exited {code}\n{}", last_lines()),
                    phase,
                };
            }
            Ok(None) => {}
            Err(tail) => {
                send(Stream::Stderr, tail.clone(), log);
                return ApplyOutcome::Failed {
                    code: None,
                    tail,
                    phase,
                };
            }
        }
        if cancelled() {
            return ApplyOutcome::Cancelled;
        }
    }
    ApplyOutcome::Success
}

/// Prefix `-C <dir> -E <envs>` to a phase argv for display and execution.
fn full_argv(config_dir: &Path, req: &ApplyRequest, phase: &[String]) -> Vec<String> {
    let mut argv = vec!["-C".to_string(), config_dir.display().to_string()];
    for env in &req.envs {
        argv.push("-E".to_string());
        argv.push(env.clone());
    }
    argv.extend(phase.iter().cloned());
    argv
}

fn display_argv(mise_bin: &Path, argv: &[String]) -> String {
    let mut parts = vec![mise_bin.display().to_string()];
    parts.extend(argv.iter().cloned());
    parts.join(" ")
}

/// Spawn one child, stream stdout/stderr lines with sequence numbers, wait.
/// Returns `Ok(Some(code))` on nonzero exit, `Ok(None)` on success,
/// `Err(tail)` when the child could not be spawned or waited on.
///
/// Shared worker pattern: `mise` apply phases (backend) and `setup` stages
/// (cli) both stream through here. `program`/`args` are spawned verbatim;
/// `ceiling` sets `MISE_CEILING_PATHS` (harmless for non-mise children).
pub(crate) fn stream_child(
    program: &Path,
    args: &[String],
    ceiling: &Path,
    emit: &mut impl FnMut(Stream, String, &mut std::io::BufWriter<std::fs::File>),
    log: &mut std::io::BufWriter<std::fs::File>,
    current: &Arc<Mutex<Option<Child>>>,
) -> Result<Option<i32>, String> {
    let mut child = Command::new(program)
        .args(args)
        .env("MISE_CEILING_PATHS", ceiling)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to spawn mise: {e}"))?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let (lines_tx, lines_rx) = std::sync::mpsc::channel::<(Stream, String)>();
    let mut pipes: Vec<(Stream, Box<dyn std::io::Read + Send>)> = Vec::new();
    if let Some(pipe) = stdout {
        pipes.push((Stream::Stdout, Box::new(pipe)));
    }
    if let Some(pipe) = stderr {
        pipes.push((Stream::Stderr, Box::new(pipe)));
    }
    for (stream, pipe) in pipes {
        let lines_tx = lines_tx.clone();
        std::thread::Builder::new()
            .name(format!("emc-apply-reader-{stream:?}"))
            .spawn(move || {
                for line in BufReader::new(pipe).lines() {
                    match line {
                        Ok(text) => {
                            if lines_tx.send((stream, text)).is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            })
            .map_err(|e| format!("failed to spawn log reader: {e}"))?;
    }
    drop(lines_tx);
    {
        let mut slot = current.lock().expect("apply child slot poisoned");
        *slot = Some(child);
    }

    for (stream, text) in lines_rx {
        emit(stream, text, log);
    }
    let mut slot = current.lock().expect("apply child slot poisoned");
    let mut child = slot.take().expect("apply child missing");
    drop(slot);
    let status = child
        .wait()
        .map_err(|e| format!("failed to wait on mise: {e}"))?;
    if status.success() {
        Ok(None)
    } else {
        Ok(status.code())
    }
}

/// Handle owning a running apply: join it or cancel it.
///
/// Cancellation sets the flag and kills the running child; the worker then
/// kills the sudo keepalive and reports [`ApplyOutcome::Cancelled`].
pub struct ApplyHandle {
    worker: Option<std::thread::JoinHandle<ApplyOutcome>>,
    cancel_flag: Arc<AtomicBool>,
    current: Arc<Mutex<Option<Child>>>,
    log_path: PathBuf,
}

impl ApplyHandle {
    /// True when the worker thread finished (join will not block).
    pub fn is_finished(&self) -> bool {
        self.worker.as_ref().is_some_and(|w| w.is_finished())
    }

    /// Cancel the apply: kill the running child; the worker cleans up.
    pub fn cancel(&mut self) -> ApplyOutcome {
        self.cancel_flag.store(true, Ordering::Relaxed);
        if let Ok(mut slot) = self.current.lock()
            && let Some(mut child) = slot.take()
        {
            let _ = child.kill();
            *slot = Some(child);
        }
        ApplyOutcome::Cancelled
    }

    /// Wait for the worker and return its outcome.
    pub fn join(mut self) -> ApplyOutcome {
        match self.worker.take() {
            Some(worker) => worker.join().unwrap_or(ApplyOutcome::Failed {
                code: None,
                tail: "apply worker panicked".to_string(),
                phase: ApplyPhase::Base,
            }),
            None => ApplyOutcome::Cancelled,
        }
    }

    /// Path of the temp file holding the FULL apply log.
    pub fn log_path(&self) -> &Path {
        &self.log_path
    }
}

/// Deterministic backend over the T2.2 fixtures (unit + screenshot tests).
pub mod mock;

#[cfg(test)]
mod tests;
