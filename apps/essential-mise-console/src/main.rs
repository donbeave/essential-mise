use std::io::IsTerminal as _;
use std::process::ExitCode;
use std::sync::Arc;

use clap::Parser as _;
use essential_mise_console::backend::{Backend, MiseBackend};
use essential_mise_console::cli::{self, Cli, EXIT_ERROR};

fn main() -> ExitCode {
    let cli = Cli::parse();
    if cli.command.is_none() && !std::io::stdin().is_terminal() {
        eprintln!(
            "essential-mise-console: no subcommand given and stdin is not a terminal \
             (fail-closed; pass an explicit subcommand)"
        );
        return ExitCode::from(EXIT_ERROR);
    }
    let config_dir = cli::resolve_config_dir(cli.config_dir.as_deref());
    let backend: Arc<dyn Backend> = if std::env::var("EMC_MOCK").as_deref() == Ok("1") {
        Arc::new(cli::mock_backend())
    } else {
        Arc::new(MiseBackend::new(config_dir, cli::resolve_envs(&cli.env)))
    };
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    ExitCode::from(cli::run(backend, &cli, &mut out))
}
