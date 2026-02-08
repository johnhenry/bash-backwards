//! hsab - A postfix notation shell transpiler
//!
//! Usage:
//!   hsab              Start interactive REPL
//!   hsab -c "cmd"     Execute a single command
//!   hsab script.hsab  Execute a script file
//!   hsab --emit "cmd" Show generated bash without executing

use hsab::{Shell, ShellError};
use rustyline::error::ReadlineError;
use rustyline::{DefaultEditor, Result as RlResult};
use std::env;
use std::fs;
use std::process::ExitCode;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn print_help() {
    println!(
        r#"hsab-{}£ Hash Backwards - A postfix notation shell

USAGE:
    hsab                    Start interactive REPL
    hsab -c <command>       Execute a single command
    hsab <script.hsab>      Execute a script file
    hsab --emit <command>   Show generated bash without executing
    hsab --help             Show this help message
    hsab --version          Show version

SYNTAX (executable-aware parsing):
    args exec               Auto-detect: exec args
    %(args cmd) exec        Piped: exec | cmd args
    %(args cmd)             Explicit group for pipes
    cmd %(args cmd2) &&     And: cmd && cmd2 args
    cmd %(args cmd2) ||     Or: cmd || cmd2 args
    cmd %(file) >           Redirect: cmd > file
    cmd &                   Background: cmd &
    #!bash <raw bash>       Bash passthrough

HSAB VARIABLES:
    %_                      Last argument of previous command
    %!                      Stdout of previous command
    %?                      Exit code of previous command
    %cmd                    The bash command that was generated
    %@                      All args of previous command
    %0, %1, %2...           Individual lines of output (0-indexed)

EXAMPLES (executable-aware syntax):
    -la ls                        ls -la
    hello grep                    grep hello
    %(hello grep) -la ls          ls -la | grep hello
    %(-r sort) file.txt cat       cat file.txt | sort -r

EXAMPLES (traditional group syntax):
    %(hello grep) ls              ls | grep hello
    %(-5 head) %(txt grep) ls     ls | grep txt | head -5
    ls %(done echo) &&            ls && echo done
    hello echo %(out.txt) >       echo hello > out.txt

EXAMPLES (hsab variables):
    ls                            # List files
    %0 cat                        # cat the first file from ls output
    %! wc -l                      # count lines of previous output
"#,
        VERSION
    );
}

fn print_version() {
    println!("hsab-{}£", VERSION);
}

/// Run the interactive REPL with the unified Shell
fn run_repl() -> RlResult<()> {
    let mut rl = DefaultEditor::new()?;
    let mut shell = Shell::new();

    // Try to load history
    let history_path = dirs_home().map(|h| h.join(".hsab_history"));
    if let Some(ref path) = history_path {
        let _ = rl.load_history(path);
    }

    println!("hsab-{}£ Hash Backwards - postfix shell", VERSION);
    println!("  Type 'exit' or Ctrl-D to quit, 'help' for usage");
    println!("  %vars: %_ (last arg), %! (stdout), %? (exit code), %0-%N (lines)");

    // Track any leftovers to pre-fill the next prompt
    let mut prefill = String::new();
    let prompt_normal = format!("hsab-{}£ ", VERSION);
    let prompt_leftover = format!("hsab-{}¢ ", VERSION);

    loop {
        // Use readline_with_initial if we have leftovers to put back
        let readline = if prefill.is_empty() {
            rl.readline(&prompt_normal)
        } else {
            let initial = format!("{} ", prefill); // Add space after leftovers
            prefill.clear();
            rl.readline_with_initial(&prompt_leftover, (&initial, ""))
        };

        match readline {
            Ok(line) => {
                let trimmed = line.trim();

                if trimmed.is_empty() {
                    continue;
                }

                // Add to history
                let _ = rl.add_history_entry(trimmed);

                // Handle built-in commands
                match trimmed {
                    "exit" | "quit" => break,
                    "help" => {
                        print_help();
                        continue;
                    }
                    "state" => {
                        // Debug command to show current state
                        println!("  %_ = {:?}", shell.state.last_arg);
                        println!("  %? = {}", shell.state.last_exit_code);
                        println!("  %cmd = {:?}", shell.state.last_bash_cmd);
                        println!("  %@ = {:?}", shell.state.all_args);
                        println!("  lines = {}", shell.state.line_count());
                        continue;
                    }
                    _ => {}
                }

                // Execute the line using the unified Shell
                match shell.execute_interactive(trimmed) {
                    Ok(execution) => {
                        // Print captured output
                        if !execution.stdout.is_empty() {
                            print!("{}", execution.stdout);
                        }

                        // If there are leftovers, put them back on input (¢ prompt)
                        if !execution.leftovers.is_empty() {
                            prefill = execution.leftovers.clone();
                        }

                        // Show exit code if non-zero
                        if !execution.success() {
                            eprintln!("Exit code: {}", execution.exit_code());
                        }
                    }
                    Err(ShellError::EmptyInput) => {
                        // Ignore empty input
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                // Ctrl-C - clear any prefill and show new prompt
                prefill.clear();
                continue;
            }
            Err(ReadlineError::Eof) => {
                // Ctrl-D - exit
                break;
            }
            Err(err) => {
                eprintln!("Error: {:?}", err);
                break;
            }
        }
    }

    // Save history
    if let Some(ref path) = history_path {
        let _ = rl.save_history(path);
    }

    Ok(())
}

/// Execute a single command
fn execute_command(cmd: &str) -> ExitCode {
    let mut shell = Shell::new();
    match shell.execute_interactive(cmd) {
        Ok(execution) => {
            // Print captured output
            if !execution.stdout.is_empty() {
                print!("{}", execution.stdout);
            }
            if execution.success() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(execution.exit_code() as u8)
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            ExitCode::FAILURE
        }
    }
}

/// Execute a script file
fn execute_script(path: &str) -> ExitCode {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error reading {}: {}", path, e);
            return ExitCode::FAILURE;
        }
    };

    let mut shell = Shell::new();

    for (line_num, line) in content.lines().enumerate() {
        let trimmed = line.trim();

        // Skip empty lines and comments (but not #!bash)
        if trimmed.is_empty() || (trimmed.starts_with('#') && !trimmed.starts_with("#!bash")) {
            continue;
        }

        match shell.execute(trimmed) {
            Ok(result) => {
                // Print output
                if !result.stdout.is_empty() {
                    print!("{}", result.stdout);
                }
                if !result.stderr.is_empty() {
                    eprint!("{}", result.stderr);
                }

                if !result.success() {
                    eprintln!("Error at line {}: command failed with exit code {}",
                             line_num + 1, result.exit_code());
                    return ExitCode::FAILURE;
                }
            }
            Err(ShellError::EmptyInput) => {
                // Skip empty lines
            }
            Err(e) => {
                eprintln!("Error at line {}: {}", line_num + 1, e);
                return ExitCode::FAILURE;
            }
        }
    }

    ExitCode::SUCCESS
}

/// Show generated bash without executing
fn emit_command(cmd: &str) -> ExitCode {
    let mut shell = Shell::new();
    match shell.compile(cmd) {
        Ok(bash) => {
            println!("{}", bash);
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            ExitCode::FAILURE
        }
    }
}

/// Get home directory
fn dirs_home() -> Option<std::path::PathBuf> {
    env::var_os("HOME").map(std::path::PathBuf::from)
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();

    match args.len() {
        1 => {
            // No arguments - start REPL
            match run_repl() {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("REPL error: {}", e);
                    ExitCode::FAILURE
                }
            }
        }
        2 => {
            // Single argument
            match args[1].as_str() {
                "--help" | "-h" => {
                    print_help();
                    ExitCode::SUCCESS
                }
                "--version" | "-V" => {
                    print_version();
                    ExitCode::SUCCESS
                }
                path => {
                    // Assume it's a script file
                    execute_script(path)
                }
            }
        }
        3 => {
            // Two arguments
            match args[1].as_str() {
                "-c" => execute_command(&args[2]),
                "--emit" | "-e" => emit_command(&args[2]),
                _ => {
                    eprintln!("Unknown option: {}", args[1]);
                    print_help();
                    ExitCode::FAILURE
                }
            }
        }
        _ => {
            // Multiple arguments after -c
            if args[1] == "-c" {
                let cmd = args[2..].join(" ");
                execute_command(&cmd)
            } else if args[1] == "--emit" || args[1] == "-e" {
                let cmd = args[2..].join(" ");
                emit_command(&cmd)
            } else {
                eprintln!("Too many arguments");
                print_help();
                ExitCode::FAILURE
            }
        }
    }
}
