//! Clap command tree for `essential-mise-console`, verbatim from
//! `implementation-spec.md` §2.
//!
//! Behavior lands in later tasks (T3.2 implements `plan --json` and
//! `apply --dry-run` with golden tests; T3.4 `setup`; T4 `tui`). This module
//! owns the parseable tree, the exit-code contract, config-dir resolution
//! (L6), and the stock `exec` hint. `-E` and `-y` are global (repeatable and
//! visible on every subcommand); `--manager` on `packages` is console-side
//! filtering via `origin_map` (`packages status` has no `-m`).

use std::path::{Path, PathBuf};

use super::backend::{Action, ApplyLine, ApplyOutcome, ApplyPhase, ApplyRequest, Backend};

/// Exit 0: ok.
pub const EXIT_OK: u8 = 0;
/// Exit 1: a part failed (or usage error).
pub const EXIT_ERROR: u8 = 1;
/// Exit 3: partial apply (phase 1 ok, phase 2 failed). 2 is taken by plan
/// would-change, so partial apply must not reuse it.
pub const EXIT_PARTIAL: u8 = 3;

/// `essential-mise-console`: module multi-select, package checklist,
/// two-phase apply, and verify over stock `mise bootstrap`.
#[derive(Debug, Clone, clap::Parser)]
#[command(name = "essential-mise-console", version)]
pub struct Cli {
    /// Use this `mise` binary instead of `mise` on `PATH`.
    #[arg(long, global = true, value_name = "PATH")]
    pub mise: Option<PathBuf>,

    /// Config dir; else `$MISE_CONFIG_DIR`, else `~/.config/mise` (L6).
    #[arg(long, global = true, value_name = "DIR")]
    pub config_dir: Option<PathBuf>,

    /// Load these environments (repeatable or comma-separated, stock form).
    #[arg(short = 'E', long = "env", global = true, value_name = "ENV")]
    pub env: Vec<String>,

    /// Skip confirmation prompts.
    #[arg(short = 'y', long = "yes", global = true)]
    pub yes: bool,

    /// Increase verbosity (repeatable).
    #[arg(short = 'v', global = true, action = clap::ArgAction::Count, conflicts_with = "quiet")]
    pub verbose: u8,

    /// Silence non-essential output.
    #[arg(short = 'q', long = "quiet", global = true)]
    pub quiet: bool,

    /// Subcommand; omitted means the TUI modules screen on a tty, and a
    /// fail-closed error when piped.
    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// All eight subcommands.
#[derive(Debug, Clone, clap::Subcommand)]
pub enum Commands {
    /// List environments with descriptions.
    Modules {
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
        /// Substring filter over names and descriptions.
        query: Option<String>,
    },
    /// Checklist data: packages with console-side `declared_by` origins.
    Packages {
        /// Only show packages from this manager (console-side filter).
        #[arg(long, value_name = "M")]
        manager: Option<String>,
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Show what applying would change.
    Plan {
        /// Emit machine-readable JSON.
        #[arg(short = 'J', long = "json")]
        json: bool,
        /// Exit 0 clean, 2 would-change, 1 error-or-unknown.
        #[arg(long)]
        detailed_exitcode: bool,
        /// How many jobs to run in parallel.
        #[arg(short = 'j', long = "jobs", value_name = "JOBS")]
        jobs: Option<u64>,
    },
    /// Two-phase apply: base parts, then `packages apply <kept…>`.
    Apply {
        /// Skip these parts in phase 1 (comma-separated).
        #[arg(long, value_delimiter = ',', conflicts_with = "only_parts")]
        skip: Vec<String>,
        /// Run only these parts in phase 1 (comma-separated).
        #[arg(long = "only", value_delimiter = ',', value_name = "PART")]
        only_parts: Vec<String>,
        /// Only apply packages for this manager (phase-2 passthrough).
        #[arg(short = 'm', long = "manager", value_name = "M")]
        manager: Option<String>,
        /// Kept packages as `manager:package`; empty skips phase 2.
        #[arg(value_name = "PKG")]
        packages: Vec<String>,
        /// Refresh package manager metadata first (default on).
        #[arg(long, default_value_t = true, action = clap::ArgAction::SetTrue)]
        update: bool,
        /// Upgrade installed packages and tools as well (default on).
        #[arg(long, default_value_t = true, action = clap::ArgAction::SetTrue)]
        upgrade: bool,
        /// Preview only; never spawns a child.
        #[arg(short = 'n', long = "dry-run")]
        dry_run: bool,
    },
    /// Show current state.
    Status {
        /// Emit machine-readable JSON.
        #[arg(short = 'J', long = "json")]
        json: bool,
        /// Exit 1 on drift (all parts still listed).
        #[arg(long)]
        missing: bool,
    },
    /// Re-run `status --json` plus `plan` per env; prints the stock
    /// `foundation doctor` hint.
    Verify {
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Stages 1–5 orchestration: CLT, Homebrew, mise, adoption, foundation.
    Setup {
        /// Print the staged plan; spawn nothing.
        #[arg(long)]
        dry_run: bool,
    },
    /// Open the interactive console at an optional screen.
    Tui {
        /// Screen to open on.
        screen: Option<TuiScreen>,
    },
}

/// TUI screens, in journey order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum TuiScreen {
    /// Module multi-select.
    Modules,
    /// Per-package checklist.
    Packages,
    /// Live two-phase apply.
    Apply,
    /// Verify screen.
    Result,
}

/// Resolve the config dir per L6: `--config-dir`, else `$MISE_CONFIG_DIR`,
/// else `~/.config/mise`.
pub fn resolve_config_dir(flag: Option<&Path>) -> PathBuf {
    if let Some(dir) = flag {
        return dir.to_path_buf();
    }
    if let Ok(dir) = std::env::var("MISE_CONFIG_DIR")
        && !dir.is_empty()
    {
        return PathBuf::from(dir);
    }
    let home = std::env::var("HOME")
        .expect("HOME must be set when neither --config-dir nor MISE_CONFIG_DIR is set");
    PathBuf::from(home).join(".config/mise")
}

/// Flatten stock `-E` forms: repeated flags and comma-separated lists.
pub fn resolve_envs(flags: &[String]) -> Vec<String> {
    flags
        .iter()
        .flat_map(|f| f.split(','))
        .filter(|e| !e.is_empty())
        .map(|e| e.to_string())
        .collect()
}

/// Stock `exec` hint printed wherever the console declines to run a command
/// itself (`exec` stays out of the console).
pub fn exec_hint(envs: &[String]) -> String {
    if envs.is_empty() {
        "mise exec -- <command>".to_string()
    } else {
        format!("mise -E {} exec -- <command>", envs.join(","))
    }
}

fn unimplemented(command: &str) -> u8 {
    eprintln!("essential-mise-console: '{command}' lands in a later task (T3.4+); see --help");
    EXIT_ERROR
}

/// Dispatch a parsed CLI against a backend, writing envelopes and logs to
/// `out` and human notes to stderr. Returns the process exit code.
pub fn run(backend: std::sync::Arc<dyn Backend>, cli: &Cli, out: &mut dyn std::io::Write) -> u8 {
    match &cli.command {
        None => run_tui(backend, cli, None, out),
        Some(Commands::Modules { .. }) => unimplemented("modules"),
        Some(Commands::Packages { .. }) => unimplemented("packages"),
        Some(Commands::Plan {
            json,
            detailed_exitcode,
            jobs,
        }) => run_plan(&backend, cli, *json, *detailed_exitcode, *jobs, out),
        Some(Commands::Apply {
            skip,
            only_parts,
            manager,
            packages,
            update,
            upgrade,
            dry_run,
        }) => run_apply(
            &backend,
            cli,
            skip,
            only_parts,
            manager.as_deref(),
            packages,
            *update,
            *upgrade,
            *dry_run,
            out,
        ),
        Some(Commands::Status { .. }) => unimplemented("status"),
        Some(Commands::Verify { .. }) => unimplemented("verify"),
        Some(Commands::Setup { dry_run }) => run_setup(&SetupConfig::from_cli(cli), *dry_run, out),
        Some(Commands::Tui { screen }) => run_tui(backend, cli, *screen, out),
    }
}

/// Mock backend for screenshots and headless runs (`EMC_MOCK=1`): the T2.2
/// fixtures plus canned S2 origins for the fixture envs
/// (`neovim`, `terminal`, `shell`). `EMC_MOCK_FAIL=packages|base` scripts a
/// phase failure for the failure-modal baseline.
pub fn mock_backend() -> super::backend::mock::MockBackend {
    use super::backend::ApplyPhase;
    let mut mock = super::backend::mock::MockBackend::new()
        .with_origin("brew", "neovim", &["neovim"])
        .with_origin("brew", "ripgrep", &["neovim"])
        .with_origin("brew", "starship", &["terminal", "shell"])
        .with_origin("brew", "bat", &["terminal"])
        .with_origin("npm", "pyright", &["shell"]);
    match std::env::var("EMC_MOCK_FAIL").as_deref() {
        Ok("packages") => mock = mock.with_failure(ApplyPhase::Packages),
        Ok("base") => mock = mock.with_failure(ApplyPhase::Base),
        _ => {}
    }
    mock
}

/// `tui`: open the interactive console (mock when `EMC_MOCK=1` is set;
/// the binary entrypoint selects the backend).
pub fn run_tui(
    backend: std::sync::Arc<dyn Backend>,
    cli: &Cli,
    screen: Option<TuiScreen>,
    _out: &mut dyn std::io::Write,
) -> u8 {
    let config_dir = resolve_config_dir(cli.config_dir.as_deref());
    let mut app = match crate::app::App::new(backend, &config_dir) {
        Ok(app) => app,
        Err(e) => {
            eprintln!("essential-mise-console: cannot start the console: {e}");
            return EXIT_ERROR;
        }
    };
    if let Some(screen) = screen {
        app.jump_to(screen);
    }
    match junie_tui::runtime::run(&mut app) {
        Ok(()) => app.exit_code().unwrap_or(EXIT_ERROR),
        Err(e) => {
            eprintln!("essential-mise-console: console loop failed: {e}");
            EXIT_ERROR
        }
    }
}

/// `plan`: query the backend, print the console envelope
/// `{"envs","resources","summary"}` on stdout and a human line on stderr.
fn run_plan(
    backend: &std::sync::Arc<dyn Backend>,
    cli: &Cli,
    json: bool,
    detailed_exitcode: bool,
    jobs: Option<u64>,
    out: &mut dyn std::io::Write,
) -> u8 {
    let envs = resolve_envs(&cli.env);
    let entries = match backend.plan(&envs, jobs) {
        Ok(entries) => entries,
        Err(e) => {
            eprintln!("essential-mise-console: plan failed: {e}");
            return EXIT_ERROR;
        }
    };
    let mut create = 0u64;
    let mut update = 0u64;
    let mut remove = 0u64;
    let mut unchanged = 0u64;
    let mut unknown = 0u64;
    let mut resources = Vec::with_capacity(entries.len());
    for e in &entries {
        match e.action {
            Action::Create => create += 1,
            Action::Update => update += 1,
            Action::Remove => remove += 1,
            Action::Noop => unchanged += 1,
            Action::Unknown => unknown += 1,
        }
        resources.push(serde_json::json!({
            "kind": e.kind,
            "name": e.name,
            "current": e.current,
            "desired": e.desired,
            "action": e.action.as_str(),
        }));
    }
    if json {
        let doc = serde_json::json!({
            "envs": envs,
            "resources": resources,
            "summary": {
                "create": create,
                "update": update,
                "remove": remove,
                "unchanged": unchanged,
                "unknown": unknown,
            },
        });
        if writeln!(
            out,
            "{}",
            serde_json::to_string_pretty(&doc).expect("envelope serializes")
        )
        .is_err()
        {
            eprintln!("essential-mise-console: failed to write plan output");
            return EXIT_ERROR;
        }
    } else {
        for e in &entries {
            let _ = writeln!(
                out,
                "{} {} {}: {} -> {}",
                e.action.as_str(),
                e.kind,
                e.name,
                e.current,
                e.desired
            );
        }
    }
    eprintln!(
        "plan: {} create, {} update, {} remove, {} unchanged, {} unknown",
        create, update, remove, unchanged, unknown
    );
    if detailed_exitcode {
        if unknown > 0 {
            EXIT_ERROR
        } else if create + update + remove > 0 {
            2
        } else {
            EXIT_OK
        }
    } else {
        EXIT_OK
    }
}

/// `apply`: two-phase apply with exit mapping 0 ok / 1 failed / 3 partial
/// (phase 1 ok, phase 2 failed). `--dry-run` previews and never spawns.
#[allow(clippy::too_many_arguments)]
fn run_apply(
    backend: &std::sync::Arc<dyn Backend>,
    cli: &Cli,
    skip: &[String],
    only: &[String],
    manager: Option<&str>,
    packages: &[String],
    update: bool,
    upgrade: bool,
    dry_run: bool,
    out: &mut dyn std::io::Write,
) -> u8 {
    let envs = resolve_envs(&cli.env);
    let keep: Vec<String> = packages.to_vec();
    if !dry_run && !cli.yes && !confirm(&envs, &keep) {
        return EXIT_ERROR;
    }
    let req = ApplyRequest {
        envs,
        keep,
        manager: manager.map(|m| m.to_string()),
        skip_parts: skip.to_vec(),
        only_parts: only.to_vec(),
        update,
        upgrade,
        dry_run,
    };
    let (tx, rx) = junie_tui::runtime::external_channel();
    let handle = match backend.spawn_apply(&req, tx) {
        Ok(handle) => handle,
        Err(e) => {
            eprintln!("essential-mise-console: apply failed to start: {e}");
            return EXIT_ERROR;
        }
    };
    let outcome = handle.join();
    for m in rx.drain() {
        if let Some(line) = m.downcast_ref::<ApplyLine>() {
            let _ = writeln!(out, "{}", line.text);
        }
    }
    match outcome {
        ApplyOutcome::Success => EXIT_OK,
        ApplyOutcome::Failed {
            phase: ApplyPhase::Packages,
            ..
        } => EXIT_PARTIAL,
        ApplyOutcome::Failed { .. } => EXIT_ERROR,
        ApplyOutcome::Cancelled => EXIT_ERROR,
    }
}

/// Confirm a mutating apply on stderr/stdin. Anything but `y`/`yes` aborts.
fn confirm(envs: &[String], keep: &[String]) -> bool {
    use std::io::Write as _;
    let _ = write!(
        std::io::stderr(),
        "Apply to [{}] ({} packages kept)? [y/N] ",
        envs.join(","),
        keep.len()
    );
    let _ = std::io::stderr().flush();
    let mut line = String::new();
    if std::io::stdin().read_line(&mut line).is_err() {
        return false;
    }
    matches!(line.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

// ---------------------------------------------------------------------------
// `setup`: stages 1–5 orchestration (T3.4).
//
// Mirrors `bootstrap.sh` stages 1–5 (lines 39–156): platform guards, CLT,
// Homebrew, mise, adoption, foundation. URLs, flags, and fail messages match
// the script verbatim; the plan is pure data (`setup_plan`) so `--dry-run`
// prints it with zero subprocesses, and the T4 TUI reuses it in-process.

/// Installer URLs, matching `bootstrap.sh` exactly.
pub const BREW_INSTALL_URL: &str =
    "https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh";
/// Installer URLs, matching `bootstrap.sh` exactly.
pub const MISE_INSTALL_URL: &str = "https://mise.run";
/// Adoption source, matching `bootstrap.sh:4`.
pub const ADOPT_REPO: &str = "donbeave/essential-mise";

/// Resolved inputs for the setup plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupConfig {
    /// Config dir (`--config-dir` / `MISE_CONFIG_DIR` / `~/.config/mise`).
    pub config_dir: std::path::PathBuf,
    /// mise binary for stages 3–5 (`--mise` or `$HOME/.local/bin/mise`).
    pub mise_bin: std::path::PathBuf,
    /// Home directory (installer targets, `.zprofile`).
    pub home: std::path::PathBuf,
}

impl SetupConfig {
    /// Resolve from parsed CLI globals.
    pub fn from_cli(cli: &Cli) -> Self {
        let config_dir = resolve_config_dir(cli.config_dir.as_deref());
        let home = std::env::var("HOME").expect("HOME must be set for setup");
        let mise_bin = cli
            .mise
            .clone()
            .unwrap_or_else(|| std::path::PathBuf::from(&home).join(".local/bin/mise"));
        Self {
            config_dir,
            mise_bin,
            home: std::path::PathBuf::from(home),
        }
    }
}

/// One setup command: a program plus argv, printed in the staged plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupCommand {
    /// Program to spawn.
    pub program: String,
    /// Arguments.
    pub args: Vec<String>,
}

impl std::fmt::Display for SetupCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.program)?;
        for a in &self.args {
            write!(f, " {a}")?;
        }
        Ok(())
    }
}

/// One numbered setup stage with human narration and commands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupStage {
    /// Stage number, 1–5.
    pub number: u8,
    /// Title, mirroring the script's `say` lines.
    pub title: String,
    /// What this stage does (printed in the dry-run plan).
    pub detail: String,
    /// Commands, in order (`<tmp>` = the downloaded installer path).
    pub commands: Vec<SetupCommand>,
}

/// Pure staged plan for stages 1–5. No subprocesses, no I/O.
pub fn setup_plan(cfg: &SetupConfig) -> Vec<SetupStage> {
    let mise = cfg.mise_bin.display().to_string();
    let config_dir = cfg.config_dir.display().to_string();
    vec![
        SetupStage {
            number: 1,
            title: "Apple Command Line Tools (Git)".to_string(),
            detail: "Skipped when `xcode-select -p` and `git --version` succeed; \
                     otherwise triggers the install dialog and waits on /dev/tty."
                .to_string(),
            commands: vec![SetupCommand {
                program: "/usr/bin/xcode-select".to_string(),
                args: vec!["--install".to_string()],
            }],
        },
        SetupStage {
            number: 2,
            title: "Homebrew (latest)".to_string(),
            detail: "Skipped when /opt/homebrew/bin/brew is executable; otherwise \
                     downloads the official installer with curl -fsSL, verifies it \
                     is a non-empty shell script, runs it, then `brew update`."
                .to_string(),
            commands: vec![
                SetupCommand {
                    program: "/usr/bin/curl".to_string(),
                    args: vec![
                        "-fsSL".to_string(),
                        BREW_INSTALL_URL.to_string(),
                        "-o".to_string(),
                        "<tmp>".to_string(),
                    ],
                },
                SetupCommand {
                    program: "/bin/bash".to_string(),
                    args: vec!["<tmp>".to_string()],
                },
                SetupCommand {
                    program: "/opt/homebrew/bin/brew".to_string(),
                    args: vec!["update".to_string()],
                },
            ],
        },
        SetupStage {
            number: 3,
            title: "mise (latest stable)".to_string(),
            detail: "Downloads https://mise.run with curl -fsSL, verifies it is a \
                     non-empty shell script, runs it, then self-updates."
                .to_string(),
            commands: vec![
                SetupCommand {
                    program: "/usr/bin/curl".to_string(),
                    args: vec![
                        "-fsSL".to_string(),
                        MISE_INSTALL_URL.to_string(),
                        "-o".to_string(),
                        "<tmp>".to_string(),
                    ],
                },
                SetupCommand {
                    program: "/bin/sh".to_string(),
                    args: vec!["<tmp>".to_string()],
                },
                SetupCommand {
                    program: mise.clone(),
                    args: vec![
                        "self-update".to_string(),
                        "--yes".to_string(),
                        "--no-plugins".to_string(),
                        "--minimum-release-age".to_string(),
                        "0s".to_string(),
                    ],
                },
            ],
        },
        SetupStage {
            number: 4,
            title: "First-run adoption of public configuration".to_string(),
            detail: "Runs only when the config dir is missing or empty; otherwise \
                     validates the worktree and its donbeave/essential-mise origin."
                .to_string(),
            commands: vec![SetupCommand {
                program: mise.clone(),
                args: vec![
                    "bootstrap".to_string(),
                    "--adopt".to_string(),
                    ADOPT_REPO.to_string(),
                ],
            }],
        },
        SetupStage {
            number: 5,
            title: "Foundation (mas CLI and zsh activation)".to_string(),
            detail: format!(
                "`mise -E foundation bootstrap --upgrade --update` with \
                 MISE_CEILING_PATHS={config_dir}."
            ),
            commands: vec![SetupCommand {
                program: mise,
                args: vec![
                    "-E".to_string(),
                    "foundation".to_string(),
                    "bootstrap".to_string(),
                    "--upgrade".to_string(),
                    "--update".to_string(),
                ],
            }],
        },
    ]
}

/// Run `setup`: dry-run prints the staged plan with zero subprocesses;
/// otherwise execute stages 1–5 fail-closed with the stage number in every
/// error, then point at the module flow (the T4 TUI continues in-process).
pub fn run_setup(cfg: &SetupConfig, dry_run: bool, out: &mut dyn std::io::Write) -> u8 {
    let stages = setup_plan(cfg);
    if dry_run {
        for s in &stages {
            let _ = writeln!(out, "stage {}/5: {}", s.number, s.title);
            let _ = writeln!(out, "  {}", s.detail);
            for c in &s.commands {
                let _ = writeln!(out, "  $ {c}");
            }
        }
        return EXIT_OK;
    }
    match execute_setup_plan(cfg, &stages, out) {
        Ok(()) => {
            let _ = writeln!(
                out,
                "setup complete; continue with `essential-mise-console tui` for module selection"
            );
            EXIT_OK
        }
        Err((stage, message)) => {
            eprintln!("essential-mise-console: setup failed at stage {stage}/5: {message}");
            EXIT_ERROR
        }
    }
}

/// Capture a short probe command; `Err` on spawn failure, nonzero exit, or
/// non-UTF8 output.
fn capture(program: &str, args: &[&str]) -> Result<String, String> {
    let out = std::process::Command::new(program)
        .args(args)
        .output()
        .map_err(|e| format!("failed to spawn {program}: {e}"))?;
    if !out.status.success() {
        return Err(format!("{program} exited {}", out.status));
    }
    String::from_utf8(out.stdout).map_err(|e| format!("{program} output not UTF-8: {e}"))
}

/// Download an official installer with `run_official_installer` hygiene
/// (`curl -fsSL`, non-empty, `#!` first line); returns the script path.
fn fetch_installer(url: &str, out: &mut dyn std::io::Write) -> Result<std::path::PathBuf, String> {
    let path = std::env::temp_dir().join(format!(
        "emc-installer-{}-{}.sh",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let status = std::process::Command::new("/usr/bin/curl")
        .args(["-fsSL", url, "-o"])
        .arg(&path)
        .status()
        .map_err(|e| format!("failed to download {url}: {e}"))?;
    if !status.success() {
        return Err(format!("Failed to download {url}"));
    }
    let bytes = std::fs::read(&path).map_err(|e| format!("failed to read {url}: {e}"))?;
    if bytes.is_empty() {
        return Err(format!("Downloaded installer is empty: {url}"));
    }
    let first_line = bytes.split(|b| *b == b'\n').next().unwrap_or(&[]);
    if !first_line.starts_with(b"#!") {
        return Err(format!("Downloaded installer is not a shell script: {url}"));
    }
    let _ = writeln!(out, "downloaded {url} ({} bytes)", bytes.len());
    Ok(path)
}

/// Execute the staged plan; `Err(stage, message)` on the first failure.
fn execute_setup_plan(
    cfg: &SetupConfig,
    stages: &[SetupStage],
    out: &mut dyn std::io::Write,
) -> Result<(), (u8, String)> {
    setup_guards().map_err(|m| (0, m))?;
    let log_path = std::env::temp_dir().join(format!("emc-setup-{}.log", std::process::id()));
    let log_file =
        std::fs::File::create(&log_path).map_err(|e| (0, format!("cannot create log: {e}")))?;
    let mut log = std::io::BufWriter::new(log_file);
    let current = std::sync::Arc::new(std::sync::Mutex::new(None));
    let mut run =
        |program: &str, args: &[String], out: &mut dyn std::io::Write| -> Result<(), String> {
            let mut emit =
                |stream: super::backend::Stream,
                 text: String,
                 log: &mut std::io::BufWriter<std::fs::File>| {
                    use std::io::Write as _;
                    let _ = writeln!(out, "{text}");
                    let _ = writeln!(log, "[{stream:?}] {text}");
                };
            let ceiling = &cfg.config_dir;
            match super::backend::stream_child(
                std::path::Path::new(program),
                args,
                ceiling,
                &mut emit,
                &mut log,
                &current,
            ) {
                Ok(None) => Ok(()),
                Ok(code) => Err(format!("{program} exited {}", code.unwrap_or(-1))),
                Err(e) => Err(e),
            }
        };

    // Stage 1: CLT, skipped when the tools already answer.
    let clt_ready = capture("/usr/bin/xcode-select", &["-p"]).is_ok()
        && capture("/usr/bin/git", &["--version"]).is_ok();
    if !clt_ready {
        let _ = writeln!(out, "stage 1/5: {}", stage_title(stages, 1));
        let _ = std::process::Command::new("/usr/bin/xcode-select")
            .arg("--install")
            .status();
        if std::fs::File::open("/dev/tty").is_err() {
            return Err((
                1,
                "Finish the Apple installation dialog in Terminal, then rerun this command."
                    .to_string(),
            ));
        }
        {
            use std::io::Write as _;
            let _ = write!(
                std::io::stderr(),
                "\nFinish the Apple installation dialog, then press Return to continue... "
            );
            let _ = std::io::stderr().flush();
        }
        let mut line = String::new();
        {
            use std::io::BufRead as _;
            let _ = std::io::BufReader::new(
                std::fs::File::open("/dev/tty").map_err(|_| (1, "no /dev/tty".to_string()))?,
            )
            .read_line(&mut line);
        }
        if capture("/usr/bin/xcode-select", &["-p"]).is_err() {
            return Err((
                1,
                "Command Line Tools are not ready. Finish installation, then rerun.".to_string(),
            ));
        }
        if capture("/usr/bin/git", &["--version"]).is_err() {
            return Err((
                1,
                "Git is not ready. Finish Command Line Tools installation, then rerun.".to_string(),
            ));
        }
    }

    // Stage 2: Homebrew.
    {
        let _ = writeln!(out, "stage 2/5: {}", stage_title(stages, 2));
        let brew = std::path::Path::new("/opt/homebrew/bin/brew");
        if !is_executable(brew) {
            let script = fetch_installer(BREW_INSTALL_URL, out).map_err(|m| (2, m))?;
            run("/bin/bash", &[script.display().to_string()], out)
                .map_err(|m| (2, format!("Installer failed: {BREW_INSTALL_URL}: {m}")))?;
            let _ = std::fs::remove_file(&script);
        }
        if !is_executable(brew) {
            return Err((
                2,
                format!("Homebrew installation did not produce {}", brew.display()),
            ));
        }
        let shellenv = capture("/opt/homebrew/bin/brew", &["shellenv"])
            .map_err(|_| (2, "brew shellenv failed.".to_string()))?;
        let prefix = capture("/opt/homebrew/bin/brew", &["--prefix"]).map_err(|e| (2, e))?;
        if prefix.trim() != "/opt/homebrew" {
            return Err((2, "Homebrew prefix is not /opt/homebrew.".to_string()));
        }
        let _ = shellenv;
        run("/opt/homebrew/bin/brew", &["update".to_string()], out).map_err(|_| {
            (
                2,
                "Failed to update Homebrew and its package metadata.".to_string(),
            )
        })?;
        ensure_zprofile_brew_line().map_err(|m| (2, m))?;
    }

    // Stage 3: mise.
    {
        let _ = writeln!(out, "stage 3/5: {}", stage_title(stages, 3));
        if !is_executable(std::path::Path::new(&cfg.mise_bin)) {
            let script = fetch_installer(MISE_INSTALL_URL, out).map_err(|m| (3, m))?;
            run("/bin/sh", &[script.display().to_string()], out)
                .map_err(|m| (3, format!("Installer failed: {MISE_INSTALL_URL}: {m}")))?;
            let _ = std::fs::remove_file(&script);
        }
        if !is_executable(std::path::Path::new(&cfg.mise_bin)) {
            return Err((
                3,
                format!("mise installer did not produce {}", cfg.mise_bin.display()),
            ));
        }
        let mise = cfg.mise_bin.display().to_string();
        run(
            &mise,
            &[
                "self-update".to_string(),
                "--yes".to_string(),
                "--no-plugins".to_string(),
                "--minimum-release-age".to_string(),
                "0s".to_string(),
            ],
            out,
        )
        .map_err(|_| {
            (
                3,
                "Failed to upgrade mise to the latest stable release.".to_string(),
            )
        })?;
    }

    // Stage 4: adoption or validation.
    {
        let _ = writeln!(out, "stage 4/5: {}", stage_title(stages, 4));
        let dir = &cfg.config_dir;
        let missing_or_empty = !dir.exists()
            || (dir.is_dir()
                && std::fs::read_dir(dir)
                    .map(|mut d| d.next().is_none())
                    .unwrap_or(false));
        if missing_or_empty {
            let work = std::env::temp_dir().join(format!("emc-adopt-{}", std::process::id()));
            std::fs::create_dir_all(&work)
                .map_err(|e| (4, format!("cannot create {work:?}: {e}")))?;
            let mise = cfg.mise_bin.display().to_string();
            let mut child = std::process::Command::new(&mise)
                .arg("bootstrap")
                .arg("--adopt")
                .arg(ADOPT_REPO)
                .current_dir(&work)
                .env(
                    "MISE_CEILING_PATHS",
                    format!("{}:{}", work.display(), dir.display()),
                )
                .spawn()
                .map_err(|e| (4, format!("failed to spawn adopt: {e}")))?;
            let status = child
                .wait()
                .map_err(|e| (4, format!("adopt wait failed: {e}")))?;
            let _ = std::fs::remove_dir_all(&work);
            if !status.success() {
                return Err((4, format!("adoption failed: {status}")));
            }
        }
        validate_config_checkout(dir).map_err(|m| (4, m))?;
        if !missing_or_empty {
            let origin = capture_git_origin(dir).unwrap_or_default();
            if !is_known_origin(&origin) {
                return Err((
                    4,
                    format!(
                        "Existing config has an unexpected or missing origin ({origin}); refusing to overwrite or pull."
                    ),
                ));
            }
        }
    }

    // Stage 5: foundation.
    {
        let _ = writeln!(out, "stage 5/5: {}", stage_title(stages, 5));
        let mise = cfg.mise_bin.display().to_string();
        let mut child = std::process::Command::new(&mise)
            .args(["-E", "foundation", "bootstrap", "--upgrade", "--update"])
            .current_dir(&cfg.config_dir)
            .env("MISE_CEILING_PATHS", &cfg.config_dir)
            .spawn()
            .map_err(|e| (5, format!("failed to spawn foundation: {e}")))?;
        let status = child
            .wait()
            .map_err(|e| (5, format!("foundation wait failed: {e}")))?;
        if !status.success() {
            return Err((5, format!("foundation failed: {status}")));
        }
    }

    {
        use std::io::Write as _;
        let _ = log.flush();
    }
    let _ = writeln!(out, "setup log: {}", log_path.display());
    Ok(())
}

/// Title lookup so the executor narrates from the same plan `--dry-run`
/// prints (single source of truth: [`setup_plan`]).
fn stage_title(stages: &[SetupStage], number: u8) -> &str {
    stages
        .iter()
        .find(|s| s.number == number)
        .map(|s| s.title.as_str())
        .unwrap_or("setup")
}

/// Platform guards mirroring `bootstrap.sh:39-48` (fail-closed, unused on the
/// single supported target except as documentation).
fn setup_guards() -> Result<(), String> {
    if std::env::consts::OS != "macos" {
        return Err("This setup runs only on macOS.".to_string());
    }
    if std::env::consts::ARCH != "aarch64" {
        return Err(
            "Use a native Apple Silicon Terminal; Intel and Rosetta are not supported.".to_string(),
        );
    }
    #[cfg(unix)]
    {
        let uid = capture("/usr/bin/id", &["-u"])
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        if uid == "0" {
            return Err(
                "Run as your normal macOS user; installers request elevated access only when necessary."
                    .to_string(),
            );
        }
    }
    if std::env::var("HOME").unwrap_or_default().is_empty() {
        return Err("HOME is not set.".to_string());
    }
    let version = capture("/usr/bin/sw_vers", &["-productVersion"])
        .map_err(|_| "Cannot determine macOS version.".to_string())?;
    let mut parts = version.trim().split('.');
    let major: u64 = parts
        .next()
        .unwrap_or("")
        .parse()
        .map_err(|_| format!("Cannot determine macOS version: {version}"))?;
    let minor: u64 = parts
        .next()
        .unwrap_or("0")
        .parse()
        .map_err(|_| format!("Cannot determine macOS version: {version}"))?;
    if major < 27 || (major == 27 && minor < 2) {
        return Err(
            "This setup requires macOS 27.2 or newer. Update macOS in System Settings, then run the same command again."
                .to_string(),
        );
    }
    Ok(())
}

/// Executable-bit check (mirrors `[[ -x … ]]`).
fn is_executable(path: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::metadata(path)
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// Ensure `~/.zprofile` sources Homebrew (`bootstrap.sh:76-79`).
fn ensure_zprofile_brew_line() -> Result<(), String> {
    let home = std::env::var("HOME").map_err(|_| "HOME is not set.".to_string())?;
    let zprofile = std::path::PathBuf::from(home).join(".zprofile");
    let line = "eval \"$(/opt/homebrew/bin/brew shellenv)\"";
    let current = std::fs::read_to_string(&zprofile).unwrap_or_default();
    if current.lines().any(|l| l == line) {
        return Ok(());
    }
    use std::io::Write as _;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&zprofile)
        .map_err(|e| format!("cannot update {}: {e}", zprofile.display()))?;
    writeln!(file, "\n# Homebrew (essential-mise stage 0)\n{line}")
        .map_err(|e| format!("cannot update {}: {e}", zprofile.display()))?;
    Ok(())
}

/// Validate the adopted config checkout (`bootstrap.sh:111-123`).
fn validate_config_checkout(dir: &std::path::Path) -> Result<(), String> {
    if !dir.is_dir() {
        return Err(format!(
            "Configuration directory was not created: {}",
            dir.display()
        ));
    }
    let git = |args: &[&str]| capture("/usr/bin/git", args);
    let inside = git(&[
        "-C",
        &dir.display().to_string(),
        "rev-parse",
        "--is-inside-work-tree",
    ])
    .map_err(|_| "Existing config is not a Git worktree.".to_string())?;
    if inside.trim() != "true" {
        return Err("Existing config is not a Git worktree.".to_string());
    }
    let bare = git(&[
        "-C",
        &dir.display().to_string(),
        "rev-parse",
        "--is-bare-repository",
    ])
    .map_err(|_| "Cannot determine Git repository type.".to_string())?;
    if bare.trim() != "false" {
        return Err("Existing config cannot be a bare repository.".to_string());
    }
    let top = git(&[
        "-C",
        &dir.display().to_string(),
        "rev-parse",
        "--show-toplevel",
    ])
    .map_err(|_| "Cannot determine the configuration worktree root.".to_string())?;
    if std::path::Path::new(top.trim()) != dir {
        return Err(format!(
            "Config path must be the actual worktree root, not {}",
            top.trim()
        ));
    }
    Ok(())
}

/// Read the config checkout's origin URL.
fn capture_git_origin(dir: &std::path::Path) -> Result<String, String> {
    capture(
        "/usr/bin/git",
        &[
            "-C",
            &dir.display().to_string(),
            "remote",
            "get-url",
            "origin",
        ],
    )
    .map(|s| s.trim().to_string())
}

/// Origin allow-list from `bootstrap.sh:146-151`.
fn is_known_origin(origin: &str) -> bool {
    matches!(
        origin,
        "https://github.com/donbeave/essential-mise"
            | "https://github.com/donbeave/essential-mise.git"
            | "git@github.com:donbeave/essential-mise"
            | "git@github.com:donbeave/essential-mise.git"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_envs_splits_commas_and_repeats() {
        assert_eq!(
            resolve_envs(&["a,b".to_string(), "c".to_string(), String::new()]),
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
        assert!(resolve_envs(&[]).is_empty());
    }
}
