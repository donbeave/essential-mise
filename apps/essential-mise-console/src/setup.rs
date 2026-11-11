//! `setup`: stages 1-6 orchestration.
//!
//! Mirrors the `bootstrap.sh` launcher's platform guards, then: Command Line
//! Tools, Homebrew, mise, repository access, config adoption, and the
//! manifest's `base_modules`. The plan is pure data (`setup_plan`) so
//! `--dry-run` prints it with zero subprocesses, and the executor narrates
//! from the same plan.

use crate::cli::{Cli, EXIT_ERROR, EXIT_OK, resolve_config_dir};
use crate::manifest::{Manifest, check_min_console};
use crate::source::{ConfigSource, SourceKind};

/// Number of setup stages (`stage N/6`).
pub const STAGE_COUNT: u8 = 6;

/// Installer URLs, matching `bootstrap.sh` exactly.
pub const BREW_INSTALL_URL: &str =
    "https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh";
/// Installer URLs, matching `bootstrap.sh` exactly.
pub const MISE_INSTALL_URL: &str = "https://mise.run";

/// Printed when no config source is known and the config dir is missing or
/// empty.
pub const MISSING_CONFIG_MESSAGE: &str =
    "Pass --config OWNER/REPO. To create one, see docs/create-your-config.md.";

/// Homebrew-installed GitHub CLI, used by the repository-access fallback.
const GH_BIN: &str = "/opt/homebrew/bin/gh";

/// Resolved inputs for the setup plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupConfig {
    /// Config dir (`--config-dir` / `MISE_CONFIG_DIR` / `~/.config/mise`).
    pub config_dir: std::path::PathBuf,
    /// mise binary for stages 3–6 (`--mise` or `$HOME/.local/bin/mise`).
    pub mise_bin: std::path::PathBuf,
    /// Home directory (installer targets, `.zprofile`).
    pub home: std::path::PathBuf,
    /// Config repository from `--config` / `ESSENTIAL_MISE_CONFIG`.
    pub source: Option<ConfigSource>,
    /// Manifest `base_modules` when already known (dry-run preview of an
    /// existing checkout); the executor re-reads the manifest after stage 5.
    pub base_modules: Option<Vec<String>>,
}

impl SetupConfig {
    /// Resolve from parsed CLI globals and the `--config` value (falling back
    /// to `ESSENTIAL_MISE_CONFIG`). `Err` is a user-facing message.
    pub fn from_cli(cli: &Cli, config_arg: Option<&str>) -> Result<Self, String> {
        let config_dir = resolve_config_dir(cli.config_dir.as_deref());
        let home = std::env::var("HOME").expect("HOME must be set for setup");
        let mise_bin = cli
            .mise
            .clone()
            .unwrap_or_else(|| std::path::PathBuf::from(&home).join(".local/bin/mise"));
        let from_env = std::env::var("ESSENTIAL_MISE_CONFIG")
            .ok()
            .filter(|v| !v.trim().is_empty());
        let source = config_arg
            .map(str::to_string)
            .or(from_env)
            .map(|raw| ConfigSource::parse(&raw))
            .transpose()?;
        Ok(Self {
            config_dir,
            mise_bin,
            home: std::path::PathBuf::from(home),
            source,
            base_modules: None,
        })
    }

    /// Whether the config dir is missing or an empty directory, so stage 5
    /// must adopt a repository.
    pub fn needs_adoption(&self) -> bool {
        let dir = &self.config_dir;
        !dir.exists()
            || (dir.is_dir()
                && std::fs::read_dir(dir)
                    .map(|mut d| d.next().is_none())
                    .unwrap_or(false))
    }
}

/// One setup command: a program plus argv, printed in the staged plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupCommand {
    /// Environment assignments printed before the program.
    pub env: Vec<(String, String)>,
    /// Program to spawn.
    pub program: String,
    /// Arguments.
    pub args: Vec<String>,
}

impl SetupCommand {
    fn new(program: impl Into<String>, args: &[&str]) -> Self {
        Self {
            env: Vec::new(),
            program: program.into(),
            args: args.iter().map(|a| a.to_string()).collect(),
        }
    }
}

impl std::fmt::Display for SetupCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (key, value) in &self.env {
            write!(f, "{key}={value} ")?;
        }
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
    /// Stage number, 1–[`STAGE_COUNT`].
    pub number: u8,
    /// Title, mirroring the script's `say` lines.
    pub title: String,
    /// What this stage does (printed in the dry-run plan).
    pub detail: String,
    /// Commands, in order (`<tmp>` = the downloaded installer path).
    pub commands: Vec<SetupCommand>,
}

/// `git ls-remote` probe for a source, never prompting for credentials.
fn probe_command(url: &str) -> SetupCommand {
    SetupCommand {
        env: vec![("GIT_TERMINAL_PROMPT".to_string(), "0".to_string())],
        program: "/usr/bin/git".to_string(),
        args: vec!["ls-remote".to_string(), url.to_string()],
    }
}

fn repository_access_stage(cfg: &SetupConfig) -> SetupStage {
    let title = "Repository access".to_string();
    let Some(source) = &cfg.source else {
        return SetupStage {
            number: 4,
            title,
            detail: "Probes the config repository with `git ls-remote`. Skipped when no \
                     source is given and no checkout exists yet."
                .to_string(),
            commands: Vec::new(),
        };
    };
    let (detail, commands) = match source.kind {
        SourceKind::File => ("file:// sources are not probed.".to_string(), Vec::new()),
        SourceKind::GithubHttps => (
            "Probes the repository with `git ls-remote`. When that fails, installs `gh` \
             with Homebrew, signs in through the browser, registers `gh` as the git \
             credential helper, and retries once."
                .to_string(),
            vec![
                probe_command(&source.url),
                SetupCommand::new("/opt/homebrew/bin/brew", &["install", "gh"]),
                SetupCommand::new(
                    GH_BIN,
                    &[
                        "auth",
                        "login",
                        "--hostname",
                        "github.com",
                        "--git-protocol",
                        "https",
                        "--web",
                    ],
                ),
                SetupCommand::new(GH_BIN, &["auth", "setup-git"]),
                probe_command(&source.url),
            ],
        ),
        SourceKind::GithubSsh | SourceKind::OtherSsh => (
            "Probes the repository with `git ls-remote`. When that fails, prints SSH key \
             instructions and stops."
                .to_string(),
            vec![probe_command(&source.url)],
        ),
        SourceKind::OtherHttps => (
            "Probes the repository with `git ls-remote`. When that fails, prints how to \
             configure git credentials and stops."
                .to_string(),
            vec![probe_command(&source.url)],
        ),
    };
    SetupStage {
        number: 4,
        title,
        detail,
        commands,
    }
}

fn adopt_stage(cfg: &SetupConfig, mise: &str) -> SetupStage {
    let url = cfg
        .source
        .as_ref()
        .map_or("<--config>".to_string(), |s| s.url.clone());
    SetupStage {
        number: 5,
        title: "Adopt your configuration repository".to_string(),
        detail: "Runs only when the config dir is missing or empty; otherwise validates \
                 the worktree and, when --config is given, that it matches origin. \
                 Uncommitted changes are allowed."
            .to_string(),
        commands: vec![SetupCommand::new(mise, &["bootstrap", "--adopt", &url])],
    }
}

fn base_modules_stage(cfg: &SetupConfig, mise: &str) -> SetupStage {
    let config_dir = cfg.config_dir.display();
    let modules = cfg.base_modules.clone().unwrap_or_default();
    let mut args = Vec::new();
    if cfg.base_modules.is_some() {
        for module in &modules {
            args.push("-E".to_string());
            args.push(module.clone());
        }
    } else {
        args.push("-E".to_string());
        args.push("<base_modules>".to_string());
    }
    args.extend(
        ["bootstrap", "--upgrade", "--update"]
            .iter()
            .map(|a| a.to_string()),
    );
    let commands = if cfg.base_modules.as_ref().is_some_and(|m| m.is_empty()) {
        Vec::new()
    } else {
        vec![SetupCommand {
            env: Vec::new(),
            program: mise.to_string(),
            args,
        }]
    };
    SetupStage {
        number: 6,
        title: "Base modules".to_string(),
        detail: format!(
            "Applies the manifest's `setup.base_modules` with `mise bootstrap --upgrade \
             --update` and MISE_CEILING_PATHS={config_dir}. Also enforces `min_console`."
        ),
        commands,
    }
}

/// Pure staged plan for stages 1–6. No subprocesses, no I/O.
pub fn setup_plan(cfg: &SetupConfig) -> Vec<SetupStage> {
    let mise = cfg.mise_bin.display().to_string();
    vec![
        SetupStage {
            number: 1,
            title: "Apple Command Line Tools (Git)".to_string(),
            detail: "Skipped when `xcode-select -p` and `git --version` succeed; \
                     otherwise triggers the install dialog and waits on /dev/tty."
                .to_string(),
            commands: vec![SetupCommand::new("/usr/bin/xcode-select", &["--install"])],
        },
        SetupStage {
            number: 2,
            title: "Homebrew (latest)".to_string(),
            detail: "Skipped when /opt/homebrew/bin/brew is executable; otherwise \
                     downloads the official installer with curl -fsSL, verifies it \
                     is a non-empty shell script, runs it, then `brew update`."
                .to_string(),
            commands: vec![
                SetupCommand::new("/usr/bin/curl", &["-fsSL", BREW_INSTALL_URL, "-o", "<tmp>"]),
                SetupCommand::new("/bin/bash", &["<tmp>"]),
                SetupCommand::new("/opt/homebrew/bin/brew", &["update"]),
            ],
        },
        SetupStage {
            number: 3,
            title: "mise (latest stable)".to_string(),
            detail: "Downloads https://mise.run with curl -fsSL, verifies it is a \
                     non-empty shell script, runs it, then self-updates."
                .to_string(),
            commands: vec![
                SetupCommand::new("/usr/bin/curl", &["-fsSL", MISE_INSTALL_URL, "-o", "<tmp>"]),
                SetupCommand::new("/bin/sh", &["<tmp>"]),
                SetupCommand::new(
                    mise.clone(),
                    &[
                        "self-update",
                        "--yes",
                        "--no-plugins",
                        "--minimum-release-age",
                        "0s",
                    ],
                ),
            ],
        },
        repository_access_stage(cfg),
        adopt_stage(cfg, &mise),
        base_modules_stage(cfg, &mise),
    ]
}

/// Print the staged plan (`--dry-run`) or execute it. `config_arg` is the raw
/// `--config` value.
pub fn run_setup(
    cli: &Cli,
    config_arg: Option<&str>,
    dry_run: bool,
    out: &mut dyn std::io::Write,
) -> u8 {
    let mut cfg = match SetupConfig::from_cli(cli, config_arg) {
        Ok(cfg) => cfg,
        Err(message) => {
            eprintln!("essential-mise-console: {message}");
            return EXIT_ERROR;
        }
    };
    if dry_run {
        cfg.base_modules = Manifest::load(&cfg.config_dir)
            .ok()
            .map(|m| m.setup.base_modules);
    }
    let stages = setup_plan(&cfg);
    if dry_run {
        for s in &stages {
            let _ = writeln!(out, "stage {}/{STAGE_COUNT}: {}", s.number, s.title);
            let _ = writeln!(out, "  {}", s.detail);
            for c in &s.commands {
                let _ = writeln!(out, "  $ {c}");
            }
        }
        return EXIT_OK;
    }
    if cfg.source.is_none() && cfg.needs_adoption() {
        eprintln!("essential-mise-console: {MISSING_CONFIG_MESSAGE}");
        return EXIT_ERROR;
    }
    match execute_setup_plan(&cfg, &stages, out) {
        Ok(()) => {
            let _ = writeln!(out, "setup complete; starting module selection");
            EXIT_OK
        }
        Err((stage, message)) => {
            eprintln!(
                "essential-mise-console: setup failed at stage {stage}/{STAGE_COUNT}: {message}"
            );
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
                |stream: crate::backend::Stream,
                 text: String,
                 log: &mut std::io::BufWriter<std::fs::File>| {
                    use std::io::Write as _;
                    let _ = writeln!(out, "{text}");
                    let _ = writeln!(log, "[{stream:?}] {text}");
                };
            let ceiling = &cfg.config_dir;
            match crate::backend::stream_child(
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
        let _ = writeln!(out, "stage 1/{STAGE_COUNT}: {}", stage_title(stages, 1));
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
        let _ = writeln!(out, "stage 2/{STAGE_COUNT}: {}", stage_title(stages, 2));
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
        let _ = writeln!(out, "stage 3/{STAGE_COUNT}: {}", stage_title(stages, 3));
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

    // The repository to probe and validate: `--config`, else the existing
    // checkout's origin when it names a parseable remote.
    let dir = &cfg.config_dir;
    let adopting = cfg.needs_adoption();
    let source = match &cfg.source {
        Some(source) => Some(source.clone()),
        None if !adopting => capture_git_origin(dir)
            .ok()
            .and_then(|origin| ConfigSource::parse(&origin).ok()),
        None => None,
    };

    // Stage 4: repository access.
    {
        let _ = writeln!(out, "stage 4/{STAGE_COUNT}: {}", stage_title(stages, 4));
        if let Some(source) = &source {
            ensure_repository_access(source).map_err(|m| (4, m))?;
        }
    }

    // Stage 5: adopt the config repository, or validate the checkout.
    {
        let _ = writeln!(out, "stage 5/{STAGE_COUNT}: {}", stage_title(stages, 5));
        if adopting {
            let source = cfg
                .source
                .as_ref()
                .ok_or_else(|| (5, MISSING_CONFIG_MESSAGE.to_string()))?;
            let work = std::env::temp_dir().join(format!("emc-adopt-{}", std::process::id()));
            std::fs::create_dir_all(&work)
                .map_err(|e| (5, format!("cannot create {work:?}: {e}")))?;
            let mise = cfg.mise_bin.display().to_string();
            let mut child = std::process::Command::new(&mise)
                .arg("bootstrap")
                .arg("--adopt")
                .arg(&source.url)
                .current_dir(&work)
                .env(
                    "MISE_CEILING_PATHS",
                    format!("{}:{}", work.display(), dir.display()),
                )
                .spawn()
                .map_err(|e| (5, format!("failed to spawn adopt: {e}")))?;
            let status = child
                .wait()
                .map_err(|e| (5, format!("adopt wait failed: {e}")))?;
            let _ = std::fs::remove_dir_all(&work);
            if !status.success() {
                return Err((5, format!("adoption failed: {status}")));
            }
        }
        validate_config_checkout(dir).map_err(|m| (5, m))?;
        if !adopting {
            let origin = capture_git_origin(dir).unwrap_or_default();
            if origin.is_empty() {
                return Err((
                    5,
                    "Existing config has no origin remote; refusing to overwrite or pull."
                        .to_string(),
                ));
            }
            if let Some(source) = &cfg.source
                && !source.matches_origin(&origin)
            {
                return Err((
                    5,
                    format!(
                        "Existing config origin ({origin}) does not match --config ({}); \
                         refusing to overwrite or pull.",
                        source.url
                    ),
                ));
            }
        }
    }

    // The manifest is readable only now: enforce `min_console`, then read
    // `base_modules`.
    let manifest = Manifest::load(dir).map_err(|e| (5, e.to_string()))?;
    check_min_console(&manifest, env!("CARGO_PKG_VERSION")).map_err(|m| (5, m))?;

    // Stage 6: base modules.
    {
        let _ = writeln!(out, "stage 6/{STAGE_COUNT}: {}", stage_title(stages, 6));
        let modules = &manifest.setup.base_modules;
        if modules.is_empty() {
            let _ = writeln!(out, "no base modules to apply");
        } else {
            let mise = cfg.mise_bin.display().to_string();
            let mut args: Vec<&str> = Vec::new();
            for module in modules {
                args.extend(["-E", module.as_str()]);
            }
            args.extend(["bootstrap", "--upgrade", "--update"]);
            let mut child = std::process::Command::new(&mise)
                .args(&args)
                .current_dir(&cfg.config_dir)
                .env("MISE_CEILING_PATHS", &cfg.config_dir)
                .spawn()
                .map_err(|e| (6, format!("failed to spawn base modules: {e}")))?;
            let status = child
                .wait()
                .map_err(|e| (6, format!("base modules wait failed: {e}")))?;
            if !status.success() {
                return Err((6, format!("base modules failed: {status}")));
            }
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

/// Probe a repository without ever prompting for credentials.
fn probe_repository(url: &str) -> bool {
    std::process::Command::new("/usr/bin/git")
        .args(["ls-remote", url])
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Run an interactive command with the terminal attached.
fn run_attached(program: &str, args: &[&str]) -> Result<(), String> {
    let status = std::process::Command::new(program)
        .args(args)
        .status()
        .map_err(|e| format!("failed to spawn {program}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} exited {status}"))
    }
}

/// Stage 4: make sure `git` can reach the config repository.
fn ensure_repository_access(source: &ConfigSource) -> Result<(), String> {
    if source.kind == SourceKind::File || probe_repository(&source.url) {
        return Ok(());
    }
    match source.kind {
        SourceKind::GithubHttps => {
            if !is_executable(std::path::Path::new(GH_BIN)) {
                run_attached("/opt/homebrew/bin/brew", &["install", "gh"])
                    .map_err(|e| format!("Failed to install the GitHub CLI: {e}"))?;
            }
            run_attached(
                GH_BIN,
                &[
                    "auth",
                    "login",
                    "--hostname",
                    "github.com",
                    "--git-protocol",
                    "https",
                    "--web",
                ],
            )
            .map_err(|e| format!("GitHub sign-in failed: {e}"))?;
            run_attached(GH_BIN, &["auth", "setup-git"])
                .map_err(|e| format!("Failed to register gh as the git credential helper: {e}"))?;
            if probe_repository(&source.url) {
                Ok(())
            } else {
                Err(format!(
                    "Cannot reach {} after signing in. Check the repository name and that your \
                     account can read it, then rerun this command.",
                    source.url
                ))
            }
        }
        SourceKind::GithubSsh | SourceKind::OtherSsh => Err(format!(
            "Cannot reach {} over SSH. Register an SSH key with the host, load it in \
             ssh-agent, check it with `ssh -T <user>@<host>`, then rerun this command.",
            source.url
        )),
        SourceKind::OtherHttps | SourceKind::File => Err(format!(
            "Cannot reach {}. Configure git credentials for that host, then rerun this command.",
            source.url
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(source: Option<&str>) -> SetupConfig {
        SetupConfig {
            config_dir: "/cfg".into(),
            mise_bin: "/bin/mise".into(),
            home: "/home".into(),
            source: source.map(|s| ConfigSource::parse(s).expect("source")),
            base_modules: None,
        }
    }

    fn commands(stage: &SetupStage) -> Vec<String> {
        stage.commands.iter().map(|c| c.to_string()).collect()
    }

    #[test]
    fn plan_has_six_numbered_stages() {
        let plan = setup_plan(&cfg(Some("OWNER/REPO")));
        let numbers: Vec<u8> = plan.iter().map(|s| s.number).collect();
        assert_eq!(numbers, [1, 2, 3, 4, 5, 6]);
        assert_eq!(plan[3].title, "Repository access");
        assert_eq!(usize::from(STAGE_COUNT), plan.len());
    }

    #[test]
    fn github_https_access_stage_probes_then_falls_back_to_gh() {
        let plan = setup_plan(&cfg(Some("OWNER/REPO")));
        let probe =
            "GIT_TERMINAL_PROMPT=0 /usr/bin/git ls-remote https://github.com/OWNER/REPO.git";
        assert_eq!(
            commands(&plan[3]),
            [
                probe,
                "/opt/homebrew/bin/brew install gh",
                "/opt/homebrew/bin/gh auth login --hostname github.com --git-protocol https --web",
                "/opt/homebrew/bin/gh auth setup-git",
                probe,
            ]
        );
    }

    #[test]
    fn ssh_access_stage_only_probes() {
        let plan = setup_plan(&cfg(Some("git@github.com:OWNER/REPO.git")));
        assert_eq!(
            commands(&plan[3]),
            ["GIT_TERMINAL_PROMPT=0 /usr/bin/git ls-remote git@github.com:OWNER/REPO.git"]
        );
    }

    #[test]
    fn file_and_missing_sources_skip_the_probe() {
        assert!(
            setup_plan(&cfg(Some("file:///tmp/repo")))[3]
                .commands
                .is_empty()
        );
        assert!(setup_plan(&cfg(None))[3].commands.is_empty());
    }

    #[test]
    fn adopt_stage_uses_the_normalized_source() {
        let plan = setup_plan(&cfg(Some("OWNER/REPO")));
        assert_eq!(
            commands(&plan[4]),
            ["/bin/mise bootstrap --adopt https://github.com/OWNER/REPO.git"]
        );
    }

    #[test]
    fn base_modules_stage_follows_the_manifest_when_known() {
        let mut c = cfg(None);
        assert_eq!(
            commands(&setup_plan(&c)[5]),
            ["/bin/mise -E <base_modules> bootstrap --upgrade --update"]
        );
        c.base_modules = Some(vec!["foundation".into(), "extra".into()]);
        assert_eq!(
            commands(&setup_plan(&c)[5]),
            ["/bin/mise -E foundation -E extra bootstrap --upgrade --update"]
        );
        c.base_modules = Some(Vec::new());
        assert!(setup_plan(&c)[5].commands.is_empty());
    }

    #[test]
    fn needs_adoption_when_dir_missing_or_empty() {
        let dir = std::env::temp_dir().join(format!("emc-setup-adopt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut c = cfg(None);
        c.config_dir = dir.clone();
        assert!(c.needs_adoption());
        std::fs::create_dir_all(&dir).expect("mkdir");
        assert!(c.needs_adoption());
        std::fs::write(dir.join("x"), "").expect("write");
        assert!(!c.needs_adoption());
        std::fs::remove_dir_all(&dir).ok();
    }
}
