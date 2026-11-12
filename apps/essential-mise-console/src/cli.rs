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

/// All nine subcommands.
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
    /// Stages 1–6 orchestration: CLT, Homebrew, mise, repository access,
    /// config adoption, base modules.
    Setup {
        /// Print the staged plan; spawn nothing.
        #[arg(long)]
        dry_run: bool,
        /// Config repository: OWNER/REPO, an HTTPS or SSH URL, or a file://
        /// URL. Falls back to `$ESSENTIAL_MISE_CONFIG`. Required only when
        /// the config dir is missing or empty.
        #[arg(long, value_name = "SOURCE")]
        config: Option<String>,
    },
    /// Check a config repository against the `#:` header contract.
    Lint,
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
        Some(Commands::Lint) => run_lint(cli, out),
        Some(Commands::Setup { dry_run, config }) => {
            let code = crate::setup::run_setup(cli, config.as_deref(), *dry_run, out);
            if *dry_run || code != EXIT_OK {
                code
            } else {
                run_tui(backend, cli, None, out)
            }
        }
        Some(Commands::Tui { screen }) => run_tui(backend, cli, *screen, out),
    }
}

/// `lint`: one line per problem on stdout; exit 0 when the config repository
/// is valid, 1 otherwise.
fn run_lint(cli: &Cli, out: &mut dyn std::io::Write) -> u8 {
    let config_dir = resolve_config_dir(cli.config_dir.as_deref());
    let problems = crate::lint::lint(&config_dir);
    for problem in &problems {
        let _ = writeln!(out, "{problem}");
    }
    if problems.is_empty() {
        let _ = writeln!(out, "lint: {} is valid", config_dir.display());
        EXIT_OK
    } else {
        EXIT_ERROR
    }
}

/// Mock backend for screenshots and headless runs (`EMC_MOCK=1`): the T2.2
/// fixtures plus canned S2 origins for the fixture envs
/// (`neovim`, `terminal`, `shell`). `EMC_MOCK_FAIL=packages|base` scripts a
/// phase failure. `EMC_MOCK_DELAY_MS` keeps the worker active for live-state
/// and cancellation journeys.
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
    if let Ok(delay) = std::env::var("EMC_MOCK_DELAY_MS")
        && let Ok(delay) = delay.parse::<u64>()
    {
        mock = mock.with_apply_delay(std::time::Duration::from_millis(delay));
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
    if let Err(e) = backend.authenticate() {
        eprintln!("essential-mise-console: cannot start the console: {e}");
        return EXIT_ERROR;
    }
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
    if !dry_run && let Err(e) = backend.authenticate() {
        eprintln!("essential-mise-console: apply cannot start: {e}");
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
