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
    Setup,
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
    eprintln!("essential-mise-console: '{command}' lands in a later task (T3.2+); see --help");
    EXIT_ERROR
}

/// Dispatch a parsed CLI. Behavior lands per task; T3.1 owns the tree and
/// the exit-code contract.
pub fn run(cli: &Cli) -> u8 {
    match &cli.command {
        None => unimplemented("tui"),
        Some(Commands::Modules { .. }) => unimplemented("modules"),
        Some(Commands::Packages { .. }) => unimplemented("packages"),
        Some(Commands::Plan { .. }) => unimplemented("plan"),
        Some(Commands::Apply { .. }) => unimplemented("apply"),
        Some(Commands::Status { .. }) => unimplemented("status"),
        Some(Commands::Verify { .. }) => unimplemented("verify"),
        Some(Commands::Setup) => unimplemented("setup"),
        Some(Commands::Tui { .. }) => unimplemented("tui"),
    }
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
