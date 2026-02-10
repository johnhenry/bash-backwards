//! hsab v2 - A stack-based postfix shell
//!
//! Usage:
//!   hsab              Start interactive REPL
//!   hsab -c "cmd"     Execute a single command
//!   hsab script.hsab  Execute a script file

use hsab::{lex, parse, Evaluator, Value};
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use rustyline::completion::{Completer, Pair};
use rustyline::{Cmd, ConditionalEventHandler, Editor, Event, EventContext, KeyCode, KeyEvent, Modifiers, Movement, RepeatCount};
use rustyline::{Helper, Result as RlResult};
use std::borrow::Cow;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn print_help() {
    println!(
        r#"hsab-{}£ Hash Backwards - A standalone stack-based postfix shell

USAGE:
    hsab                    Start interactive REPL
    hsab -l, --login        Start as login shell (sources profile)
    hsab -c <command>       Execute a single command
    hsab <script.hsab>      Execute a script file
    hsab --help             Show this help message
    hsab --version          Show version

STARTUP:
    ~/.hsabrc               Executed on REPL startup (if exists)
    ~/.hsab_profile         Executed on login shell startup (-l flag)
    HSAB_BANNER=1           Show startup banner (quiet by default)

CORE CONCEPT:
    Values push to stack, executables pop args and push output.
    dest src cp             Stack: [dest] -> [dest, src] -> cp pops both
                            Result: cp dest src

SYNTAX:
    literal                 Push to stack
    "quoted"                Push quoted string
    \"\"\"...\"\"\"                 Multiline string (triple quotes)
    $VAR                    Push variable (expanded natively)
    ~/path                  Tilde expansion to home directory
    *.txt                   Glob expansion
    [expr ...]              Push block (deferred execution)
    :name                   Define: [block] :name stores block as word
    @                       Apply: execute top block
    |                       Pipe: producer [consumer] |
    > >> <                  Redirect stdout: [cmd] [file] >
    2> 2>>                  Redirect stderr: [cmd] [file] 2>
    &>                      Redirect both: [cmd] [file] &>
    && ||                   Logic: [left] [right] &&
    &                       Background: [cmd] &

STACK OPS:
    dup                     Duplicate top: a b -> a b b
    swap                    Swap top two: a b -> b a
    drop                    Remove top: a b -> a
    over                    Copy second: a b -> a b a
    rot                     Rotate three: a b c -> b c a
    depth                   Push stack size: a b c depth -> a b c 3

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
    break                   Exit current loop early

PARALLEL:
    parallel                [[cmd1] [cmd2]] parallel - run in parallel
    fork                    [cmd1] [cmd2] 2 fork - background N blocks

PROCESS SUBST:
    subst                   [cmd] subst - create temp file with output
    fifo                    [cmd] fifo - create named pipe with output

JSON / STRUCTURED DATA:
    json                    Parse JSON string to structured data
    unjson                  Convert structured data to JSON string

RESOURCE LIMITS:
    timeout                 N [cmd] timeout - kill after N seconds

JOB CONTROL:
    jobs                    List background jobs
    fg                      Bring job to foreground: %1 fg
    bg                      Resume job in background: %1 bg

SHELL BUILTINS:
    cd                      Change directory (with ~ expansion)
    pwd                     Print working directory
    echo                    Echo arguments (no fork)
    test / [                File and string tests (postfix: file.txt -f test)
    export                  Set environment variable: VAR=val export
    unset                   Remove environment variable
    env                     List all environment variables
    true / false            Exit with 0 / 1
    tty                     Run interactive command: file.txt vim tty
    bash                    Run bash command: "for i in 1 2 3; do echo $i; done" bash
    source / .              Execute file in current context: file.hsab source
    hash                    Show/manage command hash table: ls hash, -r hash

COMMENTS:
    # comment               Inline comments (ignored)

REPL COMMANDS:
    .help, .h               Show this help
    .stack, .s              Show current stack
    .pop, .p                Pop and show top value
    .clear, .c              Clear the stack
    .use, .u                Move top stack item to input
    .use=N, .u=N            Move N stack items to input
    exit, quit              Exit the REPL

KEYBOARD SHORTCUTS:
    Ctrl+O                  Pop from stack, insert value at start of input
    Alt+O                   Push first word from input to stack
    Ctrl+,                  Clear the entire stack

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
    ~/Documents ls                # Tilde expansion
    *.rs wc -l                    # Glob expansion
    /tmp cd pwd                   # Change directory
    5 [10 sleep] timeout          # Kill after 5 seconds
    '{{"name":"test"}}' json      # Parse JSON to structured data
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

        // Skip empty lines and comments
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if let Err(e) = execute_line(eval, trimmed, true) {
            eprintln!("Warning: ~/.hsabrc line {}: {}", line_num + 1, e);
        }

        // Clear the stack after each line in rc file (definitions shouldn't leave leftovers)
        eval.clear_stack();
    }
}

/// Load and execute ~/.hsab_profile if it exists (for login shells)
fn load_hsab_profile(eval: &mut Evaluator) {
    // Profile search paths in order of priority
    let profile_paths = [
        dirs_home().map(|h| h.join(".hsab_profile")),
        dirs_home().map(|h| h.join(".profile")),
    ];

    for path in profile_paths.iter().flatten() {
        if path.exists() {
            if let Ok(content) = fs::read_to_string(path) {
                for (line_num, line) in content.lines().enumerate() {
                    let trimmed = line.trim();

                    // Skip empty lines and comments
                    if trimmed.is_empty() || trimmed.starts_with('#') {
                        continue;
                    }

                    if let Err(e) = execute_line(eval, trimmed, false) {
                        eprintln!("Warning: {} line {}: {}", path.display(), line_num + 1, e);
                    }

                    // Clear the stack after each line in profile
                    eval.clear_stack();
                }
            }
            break; // Only source first found profile
        }
    }

    // Set LOGIN_SHELL environment variable
    std::env::set_var("LOGIN_SHELL", "1");
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

// ============================================
// Keyboard shortcut handlers for stack operations
// ============================================

/// Shared state between the REPL and key handlers
struct SharedState {
    /// Stack values that can be popped to input (Ctrl+O)
    stack: Vec<Value>,
    /// Words to be pushed to stack after key handler returns (Alt+O)
    pending_push: Vec<String>,
    /// Pending prepend: value waiting to be prepended after cursor moves to end
    pending_prepend: Option<String>,
    /// Number of pops to apply to the real evaluator stack after readline returns
    pops_to_apply: usize,
}

impl SharedState {
    fn new() -> Self {
        SharedState {
            stack: Vec::new(),
            pending_push: Vec::new(),
            pending_prepend: None,
            pops_to_apply: 0,
        }
    }

    /// Clear pending operations (e.g., after .clear)
    fn clear(&mut self) {
        self.pending_prepend = None;
        self.pops_to_apply = 0;
    }
}

/// Handler for Ctrl+O: Pop from stack and insert value into input
struct PopToInputHandler {
    state: Arc<Mutex<SharedState>>,
}

impl ConditionalEventHandler for PopToInputHandler {
    fn handle(&self, _evt: &Event, _n: RepeatCount, _positive: bool, ctx: &EventContext) -> Option<Cmd> {
        let mut state = self.state.lock().ok()?;

        // First check if we have a pending prepend to complete
        if let Some(text) = state.pending_prepend.take() {
            let current_line = ctx.line().to_string();
            let new_line = if current_line.is_empty() {
                format!("{} ", text)
            } else {
                format!("{} {}", text, current_line)
            };
            return Some(Cmd::Replace(Movement::BeginningOfLine, Some(new_line)));
        }

        // Pop from stack
        if let Some(value) = state.stack.pop() {
            // Track that we need to pop from the real evaluator stack too
            state.pops_to_apply += 1;

            // Get actual value as string, quote if needed
            let insert_text = match value.as_arg() {
                Some(s) if s.contains(' ') || s.contains('\n') => {
                    format!("\"{}\"", s.replace('\"', "\\\"").replace('\n', "\\n"))
                }
                Some(s) => s,
                None => return Some(Cmd::Noop),
            };

            let current_line = ctx.line().to_string();
            let pos = ctx.pos();
            let len = current_line.len();

            if pos >= len {
                // Cursor at end (common case): do the replace now
                let new_line = if current_line.is_empty() {
                    format!("{} ", insert_text)
                } else {
                    format!("{} {}", insert_text, current_line)
                };
                return Some(Cmd::Replace(Movement::BeginningOfLine, Some(new_line)));
            } else {
                // Cursor not at end: move to end first, complete on next keypress
                state.pending_prepend = Some(insert_text);
                return Some(Cmd::Move(Movement::EndOfLine));
            }
        }
        Some(Cmd::Noop)
    }
}

/// Handler for Ctrl+P: Push first word from input to stack
struct PushToStackHandler {
    state: Arc<Mutex<SharedState>>,
}

impl ConditionalEventHandler for PushToStackHandler {
    fn handle(&self, _evt: &Event, _n: RepeatCount, _positive: bool, ctx: &EventContext) -> Option<Cmd> {
        let line = ctx.line().to_string();

        // Find the first word (non-whitespace sequence)
        let trimmed = line.trim_start();
        if trimmed.is_empty() {
            return Some(Cmd::Noop);
        }

        // Find where the first word ends
        let first_word_end = trimmed.find(char::is_whitespace).unwrap_or(trimmed.len());
        let first_word = &trimmed[..first_word_end];
        if first_word.is_empty() {
            return Some(Cmd::Noop);
        }

        // Store the word to be pushed to stack when Enter is pressed
        // Also add to state.stack for immediate visual feedback in the hint
        if let Ok(mut state) = self.state.lock() {
            state.pending_push.push(first_word.to_string());
            state.stack.push(Value::Literal(first_word.to_string()));
        }

        // Build new line without the first word
        let after_word = trimmed[first_word_end..].trim_start();
        let new_line = after_word.to_string();

        Some(Cmd::Replace(Movement::WholeLine, Some(new_line)))
    }
}

/// Handler for Ctrl+,: Clear the stack
struct ClearStackHandler {
    state: Arc<Mutex<SharedState>>,
}

impl ConditionalEventHandler for ClearStackHandler {
    fn handle(&self, _evt: &Event, _n: RepeatCount, _positive: bool, _ctx: &EventContext) -> Option<Cmd> {
        if let Ok(mut state) = self.state.lock() {
            // Mark all items in stack copy as needing to be popped from real stack
            let count = state.stack.len();
            state.stack.clear();
            state.clear();
            state.pops_to_apply = count;  // Set after clearing so it's not overwritten
        }
        // No change to the input line
        Some(Cmd::Noop)
    }
}

/// Helper struct for rustyline with live stack display and tab completion
struct HsabHelper {
    state: Arc<Mutex<SharedState>>,
    builtins: HashSet<&'static str>,
    definitions: HashSet<String>,
}

impl Helper for HsabHelper {}

impl Completer for HsabHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &rustyline::Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        // Find the word being completed
        let start = line[..pos]
            .rfind(char::is_whitespace)
            .map(|i| i + 1)
            .unwrap_or(0);
        let prefix = &line[start..pos];

        if prefix.is_empty() {
            return Ok((start, Vec::new()));
        }

        let completions = if prefix.contains('/') || prefix.starts_with('.') || prefix.starts_with('~') {
            self.complete_path(prefix)
        } else {
            self.complete_command(prefix)
        };

        let pairs: Vec<Pair> = completions
            .into_iter()
            .map(|c| Pair {
                display: c.clone(),
                replacement: c,
            })
            .collect();

        Ok((start, pairs))
    }
}

impl HsabHelper {
    fn complete_command(&self, prefix: &str) -> Vec<String> {
        let mut completions = Vec::new();

        // Check builtins
        for &b in &self.builtins {
            if b.starts_with(prefix) {
                completions.push(b.to_string());
            }
        }

        // Check user definitions
        for d in &self.definitions {
            if d.starts_with(prefix) {
                completions.push(d.clone());
            }
        }

        // Check PATH for executables (limit to avoid slowness)
        if let Ok(path) = std::env::var("PATH") {
            let mut found = 0;
            'outer: for dir in path.split(':') {
                if let Ok(entries) = std::fs::read_dir(dir) {
                    for entry in entries.filter_map(|e| e.ok()) {
                        if let Some(name) = entry.file_name().to_str() {
                            if name.starts_with(prefix) && !completions.contains(&name.to_string()) {
                                completions.push(name.to_string());
                                found += 1;
                                if found >= 50 {
                                    break 'outer;
                                }
                            }
                        }
                    }
                }
            }
        }

        completions.sort();
        completions.dedup();
        completions
    }

    fn complete_path(&self, prefix: &str) -> Vec<String> {
        let expanded = if prefix.starts_with('~') {
            if let Ok(home) = std::env::var("HOME") {
                if prefix == "~" {
                    home.clone()
                } else {
                    prefix.replacen('~', &home, 1)
                }
            } else {
                prefix.to_string()
            }
        } else {
            prefix.to_string()
        };

        let (dir, file_prefix) = if expanded.contains('/') {
            let idx = expanded.rfind('/').unwrap();
            (&expanded[..=idx], &expanded[idx + 1..])
        } else {
            ("./", expanded.as_str())
        };

        let mut completions = Vec::new();
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                if let Some(name) = entry.file_name().to_str() {
                    if name.starts_with(file_prefix) {
                        let full = if prefix.starts_with('~') {
                            // Keep tilde prefix in output
                            let home = std::env::var("HOME").unwrap_or_default();
                            let full_path = format!("{}{}", dir, name);
                            full_path.replacen(&home, "~", 1)
                        } else {
                            format!("{}{}", dir, name)
                        };
                        // Add trailing slash for directories
                        let is_dir = entry.path().is_dir();
                        completions.push(if is_dir { format!("{}/", full) } else { full });
                    }
                }
            }
        }
        completions.sort();
        completions
    }
}

/// Get default builtins for tab completion
fn default_builtins() -> HashSet<&'static str> {
    [
        // Shell builtins
        "cd", "pwd", "echo", "true", "false", "test", "[",
        "export", "unset", "env", "exit", "jobs", "fg", "bg",
        "tty", "bash", "bashsource", "which", "source", ".", "hash",
        // Stack operations
        "dup", "swap", "drop", "over", "rot", "depth",
        // Path operations
        "join", "suffix", "basename", "dirname", "reext",
        // String operations
        "split1", "rsplit1",
        // List operations
        "marker", "spread", "each", "keep", "collect",
        // Control flow
        "if", "times", "while", "until", "break",
        // Parallel
        "parallel", "fork",
        // Process substitution
        "subst", "fifo",
        // JSON
        "json", "unjson",
        // Other
        "timeout", "pipestatus",
        // Common external commands
        "ls", "cat", "grep", "find", "rm", "mv", "cp", "mkdir",
        "touch", "chmod", "head", "tail", "wc", "sort", "uniq",
        "git", "cargo", "make", "vim", "nano",
    ]
    .into_iter()
    .collect()
}

impl Hinter for HsabHelper {
    type Hint = String;

    fn hint(&self, _line: &str, _pos: usize, _ctx: &rustyline::Context<'_>) -> Option<String> {
        // Show stack as a hint below the input (filter out nil values)
        if let Ok(state) = self.state.lock() {
            let items: Vec<String> = state.stack.iter().filter_map(|v| {
                match v.as_arg() {
                    Some(s) if s.len() > 20 => Some(format!("{}...", &s[..17])),
                    Some(s) => Some(s),
                    None => None,  // Filter out nil values
                }
            }).collect();
            if items.is_empty() {
                return None;
            }
            Some(format!("\n {}", items.join(", ")))
        } else {
            None
        }
    }
}

impl Highlighter for HsabHelper {
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        Cow::Borrowed(line)
    }

    fn highlight_char(&self, _line: &str, _pos: usize) -> bool {
        false
    }

    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        // Dim the stack hint
        Cow::Owned(format!("\x1b[90m{}\x1b[0m", hint))
    }
}

impl Validator for HsabHelper {}

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

        // Skip empty lines and comments
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        match execute_line(&mut eval, trimmed, true) {
            Ok(exit_code) => {
                // Clear the stack after each line (like .hsabrc loading)
                // Output was already printed by execute_line
                eval.clear_stack();

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

/// Parse command-line arguments
struct CliArgs {
    login: bool,
    command: Option<String>,
    script: Option<String>,
    help: bool,
    version: bool,
}

fn parse_args(args: &[String]) -> CliArgs {
    let mut cli = CliArgs {
        login: false,
        command: None,
        script: None,
        help: false,
        version: false,
    };

    let mut i = 1; // Skip program name
    while i < args.len() {
        match args[i].as_str() {
            "-l" | "--login" => {
                cli.login = true;
            }
            "-c" => {
                // Everything after -c is the command
                if i + 1 < args.len() {
                    cli.command = Some(args[i + 1..].join(" "));
                    break;
                }
            }
            "--help" | "-h" => {
                cli.help = true;
            }
            "--version" | "-V" => {
                cli.version = true;
            }
            path => {
                // Assume it's a script file if not a flag
                if !path.starts_with('-') {
                    cli.script = Some(path.to_string());
                }
            }
        }
        i += 1;
    }

    cli
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let cli = parse_args(&args);

    if cli.help {
        print_help();
        return ExitCode::SUCCESS;
    }

    if cli.version {
        print_version();
        return ExitCode::SUCCESS;
    }

    // Execute command with optional login mode
    if let Some(cmd) = cli.command {
        return execute_command_with_login(&cmd, cli.login);
    }

    // Execute script
    if let Some(script) = cli.script {
        return execute_script(&script);
    }

    // Start REPL (with optional login mode)
    match run_repl_with_login(cli.login) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("REPL error: {}", e);
            ExitCode::FAILURE
        }
    }
}

/// Execute a single command with optional login shell mode
fn execute_command_with_login(cmd: &str, is_login: bool) -> ExitCode {
    let mut eval = Evaluator::new();

    // Load profile if login shell
    if is_login {
        load_hsab_profile(&mut eval);
    }

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

/// Run the REPL with optional login shell mode
fn run_repl_with_login(is_login: bool) -> RlResult<()> {
    // Set up signal handlers for job control
    hsab::signals::setup_signal_handlers();

    let mut rl = Editor::new()?;

    // Set up shared state for keyboard handlers and stack display
    let shared_state = Arc::new(Mutex::new(SharedState::new()));

    // Set helper with shared state for live stack display and tab completion
    rl.set_helper(Some(HsabHelper {
        state: Arc::clone(&shared_state),
        builtins: default_builtins(),
        definitions: HashSet::new(),
    }));

    // Bind Ctrl+O to pop from stack to input
    rl.bind_sequence(
        KeyEvent(KeyCode::Char('O'), Modifiers::CTRL),
        rustyline::EventHandler::Conditional(Box::new(PopToInputHandler {
            state: Arc::clone(&shared_state),
        })),
    );

    // Bind Alt+O to push first word from input to stack
    rl.bind_sequence(
        KeyEvent(KeyCode::Char('o'), Modifiers::ALT),
        rustyline::EventHandler::Conditional(Box::new(PushToStackHandler {
            state: Arc::clone(&shared_state),
        })),
    );

    // Bind Ctrl+, to clear the stack
    rl.bind_sequence(
        KeyEvent(KeyCode::Char(','), Modifiers::CTRL),
        rustyline::EventHandler::Conditional(Box::new(ClearStackHandler {
            state: Arc::clone(&shared_state),
        })),
    );

    let mut eval = Evaluator::new();

    // Load profile if login shell
    if is_login {
        load_hsab_profile(&mut eval);
    }

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

    // Track items to pre-fill the next prompt (from .use command or Ctrl+Alt+→)
    let mut prefill = String::new();
    // Track multiline input (for triple-quoted strings)
    let mut multiline_buffer = String::new();
    let prompt_normal = format!("hsab-{}£ ", VERSION);
    let prompt_stack = format!("hsab-{}¢ ", VERSION);  // Stack has items
    let prompt_multiline = format!("hsab-{}… ", VERSION);

    loop {
        // Sync evaluator stack with shared state (for Ctrl+Alt+→)
        {
            let mut state = shared_state.lock().unwrap();
            state.stack = eval.stack().to_vec();
        }

        // Update definitions in helper for tab completion
        if let Some(helper) = rl.helper_mut() {
            helper.definitions = eval.definition_names();
        }

        // Determine which prompt to use (check state.stack for keyboard shortcut updates)
        // Filter out nil values when checking if stack has meaningful content
        let prompt = if !multiline_buffer.is_empty() {
            &prompt_multiline
        } else {
            let has_stack = shared_state.lock()
                .map(|s| s.stack.iter().any(|v| v.as_arg().is_some()))
                .unwrap_or(false);
            if !prefill.is_empty() || has_stack {
                &prompt_stack  // ¢ when stack has items or prefill pending
            } else {
                &prompt_normal
            }
        };

        // Use readline_with_initial if we have prefill from .use command
        let readline = if prefill.is_empty() || !multiline_buffer.is_empty() {
            rl.readline(prompt)
        } else {
            let initial = format!("{} ", prefill); // Add space after prefill
            prefill.clear();
            rl.readline_with_initial(prompt, (&initial, ""))
        };

        match readline {
            Ok(line) => {
                // Process any pending pushes from Ctrl+\ (before executing the line)
                // and apply pending pops from Ctrl+] to the real evaluator stack
                {
                    let mut state = shared_state.lock().unwrap();

                    // Push words from input to stack
                    for word in state.pending_push.drain(..) {
                        eval.push_value(Value::Literal(word));
                    }

                    // Pop items from real stack that were popped from the copy during Ctrl+]
                    for _ in 0..state.pops_to_apply {
                        eval.pop_value();
                    }
                    state.pops_to_apply = 0;
                }
                // If we're in multiline mode, accumulate
                if !multiline_buffer.is_empty() {
                    multiline_buffer.push('\n');
                    multiline_buffer.push_str(&line);

                    // Check if we now have balanced triple quotes
                    if is_triple_quotes_balanced(&multiline_buffer) {
                        let complete_input = std::mem::take(&mut multiline_buffer);
                        let _ = rl.add_history_entry(&complete_input);

                        let result = execute_line(&mut eval, &complete_input, true);

                        // Clear pending state after execution
                        {
                            let mut state = shared_state.lock().unwrap();
                            state.clear();
                        }

                        match result {
                            Ok(exit_code) => {
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
                        {
                            let mut state = shared_state.lock().unwrap();
                            state.clear();
                        }
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
                    ".use" | ".u" => {
                        // Move top stack item to input
                        let items = eval.pop_n_as_string(1);
                        if !items.is_empty() {
                            prefill = items;
                        } else {
                            println!("Stack empty");
                        }
                        continue;
                    }
                    _ if trimmed.starts_with(".use=") || trimmed.starts_with(".u=") => {
                        // Move N stack items to input
                        let n_str = trimmed.strip_prefix(".use=")
                            .or_else(|| trimmed.strip_prefix(".u="))
                            .unwrap_or("");
                        match n_str.parse::<usize>() {
                            Ok(n) => {
                                let items = eval.pop_n_as_string(n);
                                if !items.is_empty() {
                                    prefill = items;
                                } else {
                                    println!("Stack empty");
                                }
                            }
                            Err(_) => {
                                eprintln!("Invalid number: {}", n_str);
                            }
                        }
                        continue;
                    }
                    _ => {}
                }

                // Execute the line
                let result = execute_line(&mut eval, trimmed, true);

                // Clear pending state after execution
                {
                    let mut state = shared_state.lock().unwrap();
                    state.clear();
                }

                match result {
                    Ok(exit_code) => {
                        // Stack persists between lines - use .use to move items to input
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
                // Ctrl-C - clear any prefill and pending state, continue
                prefill.clear();
                {
                    let mut state = shared_state.lock().unwrap();
                    state.clear();
                }
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
