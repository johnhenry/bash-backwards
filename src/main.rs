//! hsab v2 - A stack-based postfix shell
//!
//! Usage:
//!   hsab              Start interactive REPL
//!   hsab -c "cmd"     Execute a single command
//!   hsab script.hsab  Execute a script file

use hsab::{lex, parse, Evaluator};
use rustyline::error::ReadlineError;
use rustyline::{DefaultEditor, Result as RlResult};
use std::env;
use std::fs;
use std::process::ExitCode;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn print_help() {
    println!(
        r#"hsab-{}£ Hash Backwards - A stack-based postfix shell

USAGE:
    hsab                    Start interactive REPL
    hsab -c <command>       Execute a single command
    hsab <script.hsab>      Execute a script file
    hsab --help             Show this help message
    hsab --version          Show version

STARTUP:
    ~/.hsabrc               Executed on REPL startup (if exists)
    HSAB_BANNER=1           Show startup banner (quiet by default)

CORE CONCEPT:
    Values push to stack, executables pop args and push output.
    dest src cp             Stack: [dest] -> [dest, src] -> cp pops both
                            Result: cp dest src

SYNTAX:
    literal                 Push to stack
    "quoted"                Push quoted string
    \"\"\"...\"\"\"                 Multiline string (triple quotes)
    $VAR                    Push variable (expanded by bash)
    [expr ...]              Push block (deferred execution)
    :name                   Define: [block] :name stores block as word
    @                       Apply: execute top block
    |                       Pipe: producer [consumer] |
    > >> <                  Redirect: [cmd] [file] >
    && ||                   Logic: [left] [right] &&
    &                       Background: [cmd] &
    #!bash <raw>            Bash passthrough

STACK OPS:
    dup                     Duplicate top: a b -> a b b
    swap                    Swap top two: a b -> b a
    drop                    Remove top: a b -> a
    over                    Copy second: a b -> a b a
    rot                     Rotate three: a b c -> b c a

PATH OPS:
    join                    Join path: /dir file.txt -> /dir/file.txt
    basename                Get name: /path/file.txt -> file
    dirname                 Get dir: /path/file.txt -> /path
    suffix                  Add suffix: file _bak -> file_bak
    reext                   Replace ext: file.txt .md -> file.md

LIST OPS:
    spread                  Split value by lines onto stack (with marker)
    each                    Apply block to each item: spread [block] each
    keep                    Filter: keep items where predicate passes
    collect                 Gather items back into single value

CONTROL FLOW:
    if                      Conditional: [cond] [then] [else] if
    times                   Repeat: 5 [hello echo] times
    while                   Loop: [cond] [body] while
    until                   Loop: [cond] [body] until

PARALLEL:
    parallel                [[cmd1] [cmd2]] parallel - run in parallel
    fork                    [cmd1] [cmd2] 2 fork - background N blocks

PROCESS SUBST:
    subst                   [cmd] subst - create temp file with output

INTERACTIVE:
    tty                     [cmd] tty - run with TTY access (vim, less, etc.)

COMMENTS:
    # comment               Inline comments (ignored)

REPL COMMANDS:
    .help, .h               Show this help
    .stack, .s              Show current stack
    .pop, .p                Pop and show top value
    .clear, .c              Clear the stack
    exit, quit              Exit the REPL

EXAMPLES:
    hello echo                    # echo hello
    -la ls                        # ls -la
    world hello echo              # echo world hello (LIFO)
    pwd ls                        # ls $(pwd) (command substitution)
    [hello echo] @                # Apply block: echo hello
    ls [grep txt] |               # Pipe: ls | grep txt
    file.txt dup .bak reext cp    # cp file.txt file.bak
    [dup .bak reext cp] :backup   # Define 'backup' word
    file.txt backup               # Use it: cp file.txt file.bak
"#,
        VERSION
    );
}

fn print_version() {
    println!("hsab-{}£", VERSION);
}

/// Load and execute ~/.hsabrc if it exists
fn load_hsabrc(eval: &mut Evaluator) {
    let rc_path = match dirs_home() {
        Some(home) => home.join(".hsabrc"),
        None => return,
    };

    let content = match fs::read_to_string(&rc_path) {
        Ok(c) => c,
        Err(_) => return,
    };

    for (line_num, line) in content.lines().enumerate() {
        let trimmed = line.trim();

        // Skip empty lines and comments (but not #!bash)
        if trimmed.is_empty() || (trimmed.starts_with('#') && !trimmed.starts_with("#!bash")) {
            continue;
        }

        if let Err(e) = execute_line(eval, trimmed, true) {
            eprintln!("Warning: ~/.hsabrc line {}: {}", line_num + 1, e);
        }

        // Clear the stack after each line (each line is independent)
        eval.take_leftovers();
    }
}

/// Execute a single line of hsab code
fn execute_line(eval: &mut Evaluator, input: &str, print_output: bool) -> Result<i32, String> {
    let tokens = lex(input).map_err(|e| e.to_string())?;

    // Empty input is OK
    if tokens.is_empty() {
        return Ok(0);
    }

    let program = parse(tokens).map_err(|e| e.to_string())?;
    let result = eval.eval(&program).map_err(|e| e.to_string())?;

    if print_output && !result.output.is_empty() {
        println!("{}", result.output);
    }

    Ok(result.exit_code)
}

/// Check if triple quotes are balanced in the input
fn is_triple_quotes_balanced(input: &str) -> bool {
    let mut in_triple_double = false;
    let mut in_triple_single = false;
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if i + 2 < chars.len() {
            let triple: String = chars[i..i+3].iter().collect();
            if triple == "\"\"\"" && !in_triple_single {
                in_triple_double = !in_triple_double;
                i += 3;
                continue;
            }
            if triple == "'''" && !in_triple_double {
                in_triple_single = !in_triple_single;
                i += 3;
                continue;
            }
        }
        i += 1;
    }

    !in_triple_double && !in_triple_single
}

/// Run the interactive REPL
fn run_repl() -> RlResult<()> {
    let mut rl = DefaultEditor::new()?;
    let mut eval = Evaluator::new();

    // Load ~/.hsabrc if it exists
    load_hsabrc(&mut eval);

    // Try to load history
    let history_path = dirs_home().map(|h| h.join(".hsab_history"));
    if let Some(ref path) = history_path {
        let _ = rl.load_history(path);
    }

    // Show banner only if HSAB_BANNER is set
    if env::var("HSAB_BANNER").is_ok() {
        println!("hsab-{}£ Hash Backwards - stack-based postfix shell", VERSION);
        println!("  Type 'exit' or Ctrl-D to quit, '.help' for usage");
    }

    // Track leftovers to pre-fill the next prompt
    let mut prefill = String::new();
    // Track multiline input (for triple-quoted strings)
    let mut multiline_buffer = String::new();
    let prompt_normal = format!("hsab-{}£ ", VERSION);
    let prompt_leftover = format!("hsab-{}¢ ", VERSION);
    let prompt_multiline = format!("hsab-{}… ", VERSION);

    loop {
        // Determine which prompt to use
        let prompt = if !multiline_buffer.is_empty() {
            &prompt_multiline
        } else if !prefill.is_empty() {
            &prompt_leftover
        } else {
            &prompt_normal
        };

        // Use readline_with_initial if we have leftovers to put back
        let readline = if prefill.is_empty() || !multiline_buffer.is_empty() {
            rl.readline(prompt)
        } else {
            let initial = format!("{} ", prefill); // Add space after leftovers
            prefill.clear();
            rl.readline_with_initial(prompt, (&initial, ""))
        };

        match readline {
            Ok(line) => {
                // If we're in multiline mode, accumulate
                if !multiline_buffer.is_empty() {
                    multiline_buffer.push('\n');
                    multiline_buffer.push_str(&line);

                    // Check if we now have balanced triple quotes
                    if is_triple_quotes_balanced(&multiline_buffer) {
                        let complete_input = std::mem::take(&mut multiline_buffer);
                        let _ = rl.add_history_entry(&complete_input);
                        match execute_line(&mut eval, &complete_input, true) {
                            Ok(exit_code) => {
                                let leftovers = eval.take_leftovers();
                                if !leftovers.is_empty() {
                                    prefill = leftovers;
                                }
                                if exit_code != 0 {
                                    eprintln!("Exit code: {}", exit_code);
                                }
                            }
                            Err(e) => eprintln!("Error: {}", e),
                        }
                    }
                    continue;
                }

                let trimmed = line.trim();

                if trimmed.is_empty() {
                    continue;
                }

                // Check for unclosed triple quotes
                if !is_triple_quotes_balanced(trimmed) {
                    multiline_buffer = line.to_string();
                    continue;
                }

                // Add to history
                let _ = rl.add_history_entry(trimmed);

                // Handle built-in REPL commands (dot-prefix)
                match trimmed {
                    "exit" | "quit" => break,
                    ".help" | ".h" => {
                        print_help();
                        continue;
                    }
                    ".stack" | ".s" => {
                        // Debug command to show current stack
                        println!("Stack: {:?}", eval.stack());
                        continue;
                    }
                    ".clear" | ".c" => {
                        // Clear the stack
                        eval.clear_stack();
                        println!("Stack cleared");
                        continue;
                    }
                    ".pop" | ".p" => {
                        // Pop and display top of stack
                        if let Some(value) = eval.pop_value() {
                            println!("{:?}", value);
                        } else {
                            println!("Stack empty");
                        }
                        continue;
                    }
                    _ => {}
                }

                // Execute the line
                match execute_line(&mut eval, trimmed, true) {
                    Ok(exit_code) => {
                        // Check for leftover literals on the stack
                        let leftovers = eval.take_leftovers();
                        if !leftovers.is_empty() {
                            prefill = leftovers;
                        }

                        if exit_code != 0 {
                            eprintln!("Exit code: {}", exit_code);
                        }
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                // Ctrl-C - clear any prefill and continue
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
    let mut eval = Evaluator::new();
    match execute_line(&mut eval, cmd, true) {
        Ok(exit_code) => {
            if exit_code == 0 {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(exit_code as u8)
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

    let mut eval = Evaluator::new();

    for (line_num, line) in content.lines().enumerate() {
        let trimmed = line.trim();

        // Skip empty lines and comments (but not #!bash)
        if trimmed.is_empty() || (trimmed.starts_with('#') && !trimmed.starts_with("#!bash")) {
            continue;
        }

        match execute_line(&mut eval, trimmed, true) {
            Ok(exit_code) => {
                if exit_code != 0 {
                    eprintln!("Error at line {}: command failed with exit code {}",
                             line_num + 1, exit_code);
                    return ExitCode::FAILURE;
                }
            }
            Err(e) => {
                eprintln!("Error at line {}: {}", line_num + 1, e);
                return ExitCode::FAILURE;
            }
        }
    }

    ExitCode::SUCCESS
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
            } else {
                eprintln!("Too many arguments");
                print_help();
                ExitCode::FAILURE
            }
        }
    }
}
