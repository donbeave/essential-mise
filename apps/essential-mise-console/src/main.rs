use std::io::IsTerminal as _;
use std::process::ExitCode;

use clap::Parser as _;
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
    ExitCode::from(cli::run(&cli))
}
