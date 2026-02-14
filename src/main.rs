//! hsab v2 - A stack-based postfix shell
//!
//! Usage:
//!   hsab              Start interactive REPL
//!   hsab -c "cmd"     Execute a single command
//!   hsab script.hsab  Execute a script file

mod cli;
mod prompt;
mod rcfile;
mod repl;
mod terminal;

use std::env;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let cli = cli::parse_args(&args);

    if cli.help {
        cli::print_help();
        return ExitCode::SUCCESS;
    }

    if cli.version {
        cli::print_version();
        return ExitCode::SUCCESS;
    }

    if cli.init {
        return cli::run_init();
    }

    if let Some(cmd) = cli.command {
        return cli::execute_command_with_login(&cmd, cli.login, cli.trace);
    }

    if let Some(script) = cli.script {
        return cli::execute_script(&script, cli.trace);
    }

    match repl::run_repl_with_login(cli.login, cli.trace) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("REPL error: {}", e);
            ExitCode::FAILURE
        }
    }
}
