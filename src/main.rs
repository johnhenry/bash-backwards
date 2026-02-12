//! hsab v2 - A stack-based postfix shell
//!
//! Usage:
//!   hsab              Start interactive REPL
//!   hsab -c "cmd"     Execute a single command
//!   hsab script.hsab  Execute a script file

use hsab::{display, lex, parse, Evaluator, Value};
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
    hsab init               Install stdlib to ~/.hsab/lib/
    hsab -l, --login        Start as login shell (sources profile)
    hsab -c <command>       Execute a single command
    hsab <script.hsab>      Execute a script file
    hsab --help             Show this help message
    hsab --version          Show version

STARTUP:
    ~/.hsabrc               Executed on REPL startup (if exists)
    ~/.hsab/lib/stdlib.hsabrc  Auto-loaded if present (run 'hsab init')
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
    path-join               Join path: /dir file.txt path-join -> /dir/file.txt
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
    Alt+↑                   Push first word from input to stack
    Alt+↓                   Pop one from stack to input
    Alt+A                   Push ALL words from input to stack
    Alt+a                   Pop ALL from stack to input
    Alt+k                   Clear/discard the entire stack
    (Ctrl+O also pops one, for terminal compatibility)

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

    load_rc_content(eval, &content, "~/.hsabrc");
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
    execute_line_with_options(eval, input, print_output, true)
}

/// Execute a single line with display options
fn execute_line_with_options(
    eval: &mut Evaluator,
    input: &str,
    print_output: bool,
    use_format: bool,
) -> Result<i32, String> {
    let tokens = lex(input).map_err(|e| e.to_string())?;

    // Empty input is OK
    if tokens.is_empty() {
        return Ok(0);
    }

    let program = parse(tokens).map_err(|e| e.to_string())?;
    let result = eval.eval(&program).map_err(|e| e.to_string())?;

    if print_output {
        // Get terminal width for formatting
        let term_width = terminal_width();

        // Format and print each stack item
        for val in &result.stack {
            if val.as_arg().is_none() {
                continue; // Skip nil/marker
            }

            // Use pretty formatting for Tables, Records, and Errors when in REPL
            if use_format && is_structured(val) {
                println!("{}", display::format_value(val, term_width));
            } else if let Some(s) = val.as_arg() {
                println!("{}", s);
            }
        }
    }

    Ok(result.exit_code)
}

/// Check if a value is a structured type that benefits from formatting
fn is_structured(val: &Value) -> bool {
    matches!(
        val,
        Value::Table { .. } | Value::Map(_) | Value::Error { .. }
    )
}

/// Get terminal width, defaulting to 80
fn terminal_width() -> usize {
    terminal_size::terminal_size()
        .map(|(w, _)| w.0 as usize)
        .unwrap_or(80)
}


/// Set prompt context variables for PS1/PS2/STACK_HINT functions
fn set_prompt_context(eval: &Evaluator, cmd_num: usize) {
    // Version info
    let version_parts: Vec<&str> = VERSION.split('.').collect();
    std::env::set_var("_VERSION", VERSION);
    std::env::set_var("_VERSION_MAJOR", version_parts.get(0).unwrap_or(&"0"));
    std::env::set_var("_VERSION_MINOR", version_parts.get(1).unwrap_or(&"0"));
    std::env::set_var("_VERSION_PATCH", version_parts.get(2).unwrap_or(&"0"));

    // Shell state
    let depth = eval.stack().iter().filter(|v| v.as_arg().is_some()).count();
    std::env::set_var("_DEPTH", depth.to_string());
    std::env::set_var("_EXIT", eval.last_exit_code().to_string());
    std::env::set_var("_JOBS", eval.job_count().to_string());
    std::env::set_var("_CMD_NUM", cmd_num.to_string());
    std::env::set_var("_SHLVL", std::env::var("SHLVL").unwrap_or_else(|_| "1".to_string()));

    // Environment
    std::env::set_var("_CWD", eval.cwd().display().to_string());
    std::env::set_var("_USER", std::env::var("USER").unwrap_or_else(|_| "".to_string()));
    std::env::set_var("_HOST", hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "".to_string()));

    // Time
    let now = chrono::Local::now();
    std::env::set_var("_TIME", now.format("%H:%M:%S").to_string());
    std::env::set_var("_DATE", now.format("%Y-%m-%d").to_string());

    // Git info (only if in a git repo)
    let git_branch = std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    std::env::set_var("_GIT_BRANCH", &git_branch);

    if !git_branch.is_empty() {
        let git_dirty = std::process::Command::new("git")
            .args(["status", "--porcelain"])
            .output()
            .ok()
            .map(|o| if o.stdout.is_empty() { "0" } else { "1" })
            .unwrap_or("0");
        std::env::set_var("_GIT_DIRTY", git_dirty);

        let git_repo = std::process::Command::new("git")
            .args(["rev-parse", "--show-toplevel"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| {
                let path = String::from_utf8_lossy(&o.stdout).trim().to_string();
                std::path::Path::new(&path)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default()
            })
            .unwrap_or_default();
        std::env::set_var("_GIT_REPO", git_repo);
    } else {
        std::env::set_var("_GIT_DIRTY", "0");
        std::env::set_var("_GIT_REPO", "");
    }
}

/// Evaluate a prompt definition (PS1, PS2, STACK_HINT) and return the output string
fn eval_prompt_definition(eval: &mut Evaluator, name: &str) -> Option<String> {
    if !eval.has_definition(name) {
        return None;
    }

    // Save current stack
    let saved_stack = eval.stack().to_vec();

    // Clear stack for prompt evaluation
    eval.clear_stack();

    // Execute the definition
    let result = execute_line(eval, name, false);

    // Get the output from stack
    let prompt = if result.is_ok() {
        eval.stack()
            .iter()
            .filter_map(|v| v.as_arg())
            .collect::<Vec<_>>()
            .join("")
    } else {
        // On error, return None to use default
        eval.restore_stack(saved_stack);
        return None;
    };

    // Restore stack
    eval.restore_stack(saved_stack);

    if prompt.is_empty() {
        None
    } else {
        Some(prompt)
    }
}

/// Extract hint format (prefix, suffix) from STACK_HINT definition
/// Calls STACK_HINT with a test string "X" and parses the result to find prefix/suffix
fn extract_hint_format(eval: &mut Evaluator) -> (String, String) {
    let default = (" [".to_string(), "]".to_string());

    if !eval.has_definition("STACK_HINT") {
        return default;
    }

    // Save current stack
    let saved_stack = eval.stack().to_vec();

    // Clear and push a marker string
    eval.clear_stack();
    eval.push_value(Value::Literal("X".to_string()));

    // Execute STACK_HINT
    if execute_line(eval, "STACK_HINT", false).is_ok() {
        if let Some(result) = eval.stack().last().and_then(|v| v.as_arg()) {
            // Parse result to extract prefix and suffix around "X"
            if let Some(pos) = result.find('X') {
                let prefix = result[..pos].to_string();
                let suffix = result[pos + 1..].to_string();
                eval.restore_stack(saved_stack);
                return (prefix, suffix);
            }
        }
    }

    // Restore stack on failure
    eval.restore_stack(saved_stack);
    default
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
    /// Hint format: (prefix, suffix) for wrapping stack items
    /// e.g., ("│ ", " │") produces "│ a, b, c │"
    hint_format: (String, String),
}

impl SharedState {
    fn new() -> Self {
        SharedState {
            stack: Vec::new(),
            pending_push: Vec::new(),
            pending_prepend: None,
            pops_to_apply: 0,
            hint_format: (" [".to_string(), "]".to_string()), // Default format
        }
    }

    /// Clear pending operations (e.g., after .clear)
    fn clear(&mut self) {
        self.pending_prepend = None;
        self.pops_to_apply = 0;
    }

    /// Compute stack hint from current stack state
    fn compute_hint(&self) -> Option<String> {
        let items: Vec<String> = self.stack.iter().filter_map(|v| {
            match v.as_arg() {
                Some(s) if s.len() > 20 => Some(format!("{}...", &s[..17])),
                Some(s) => Some(s),
                None => None,
            }
        }).collect();

        if items.is_empty() {
            return None;
        }

        let (prefix, suffix) = &self.hint_format;
        Some(format!("\n{}{}{}", prefix, items.join(", "), suffix))
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

/// Handler for Ctrl+Alt+Up: Push ALL words from input to stack
struct PushAllToStackHandler {
    state: Arc<Mutex<SharedState>>,
}

impl ConditionalEventHandler for PushAllToStackHandler {
    fn handle(&self, _evt: &Event, _n: RepeatCount, _positive: bool, ctx: &EventContext) -> Option<Cmd> {
        let line = ctx.line().to_string();
        let trimmed = line.trim();

        if trimmed.is_empty() {
            return Some(Cmd::Noop);
        }

        // Split into words and push each to stack
        let words: Vec<&str> = trimmed.split_whitespace().collect();
        if words.is_empty() {
            return Some(Cmd::Noop);
        }

        if let Ok(mut state) = self.state.lock() {
            for word in &words {
                state.pending_push.push(word.to_string());
                state.stack.push(Value::Literal(word.to_string()));
            }
        }

        // Clear the input line
        Some(Cmd::Replace(Movement::WholeLine, Some(String::new())))
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

/// Handler for Alt+Shift+Down: Pop ALL from stack and insert into input
struct PopAllToInputHandler {
    state: Arc<Mutex<SharedState>>,
}

impl ConditionalEventHandler for PopAllToInputHandler {
    fn handle(&self, _evt: &Event, _n: RepeatCount, _positive: bool, ctx: &EventContext) -> Option<Cmd> {
        let mut state = self.state.lock().ok()?;

        if state.stack.is_empty() {
            return Some(Cmd::Noop);
        }

        // Collect all stack items as strings (in stack order, will be reversed for prepending)
        let mut items: Vec<String> = Vec::new();
        while let Some(value) = state.stack.pop() {
            state.pops_to_apply += 1;
            if let Some(s) = value.as_arg() {
                let text = if s.contains(' ') || s.contains('\n') {
                    format!("\"{}\"", s.replace('\"', "\\\"").replace('\n', "\\n"))
                } else {
                    s
                };
                items.push(text);
            }
        }

        if items.is_empty() {
            return Some(Cmd::Noop);
        }

        // Items are popped in LIFO order, so reverse to get original push order
        items.reverse();
        let insert_text = items.join(" ");

        let current_line = ctx.line().to_string();
        let new_line = if current_line.is_empty() {
            format!("{} ", insert_text)
        } else {
            format!("{} {}", insert_text, current_line)
        };

        Some(Cmd::Replace(Movement::BeginningOfLine, Some(new_line)))
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
        "tty", "which", "source", ".", "hash",
        // Stack operations
        "dup", "swap", "drop", "over", "rot", "depth",
        // Path operations
        "path-join", "suffix", "basename", "dirname", "reext",
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
        // Compute stack hint in real-time from shared state
        // This allows the hint to update as keyboard shortcuts modify the stack
        if let Ok(state) = self.state.lock() {
            state.compute_hint()
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
fn execute_script(path: &str, trace: bool) -> ExitCode {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error reading {}: {}", path, e);
            return ExitCode::FAILURE;
        }
    };

    let mut eval = Evaluator::new();
    eval.set_trace_mode(trace);

    // Load stdlib if installed
    load_stdlib(&mut eval);

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

/// Get the stdlib path (~/.hsab/lib/stdlib.hsabrc)
fn stdlib_path() -> Option<std::path::PathBuf> {
    dirs_home().map(|h| h.join(".hsab").join("lib").join("stdlib.hsabrc"))
}

/// Load stdlib from ~/.hsab/lib/stdlib.hsabrc if it exists
fn load_stdlib(eval: &mut Evaluator) {
    let path = match stdlib_path() {
        Some(p) => p,
        None => return,
    };

    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return, // Silently skip if not installed
    };

    load_rc_content(eval, &content, "stdlib");
}

/// Load RC file content, handling multiline blocks
fn load_rc_content(eval: &mut Evaluator, content: &str, source: &str) {
    let mut buffer = String::new();
    let mut bracket_depth: i32 = 0;
    let mut start_line = 1;

    for (line_num, line) in content.lines().enumerate() {
        let trimmed = line.trim();

        // Skip empty lines and comment-only lines when not in a multiline block
        if bracket_depth == 0 && (trimmed.is_empty() || trimmed.starts_with('#')) {
            continue;
        }

        // Strip inline comments (but not inside quotes - simplified check)
        let code = if let Some(pos) = trimmed.find('#') {
            // Only strip if # is not inside quotes (very simplified)
            let before_hash = &trimmed[..pos];
            let quote_count = before_hash.matches('"').count() + before_hash.matches('\'').count();
            if quote_count % 2 == 0 {
                before_hash.trim()
            } else {
                trimmed
            }
        } else {
            trimmed
        };

        if code.is_empty() {
            continue;
        }

        // Track bracket depth
        for ch in code.chars() {
            match ch {
                '[' => bracket_depth += 1,
                ']' => bracket_depth = bracket_depth.saturating_sub(1),
                _ => {}
            }
        }

        // Accumulate into buffer
        if buffer.is_empty() {
            start_line = line_num + 1;
            buffer = code.to_string();
        } else {
            buffer.push(' ');
            buffer.push_str(code);
        }

        // Execute when brackets are balanced
        if bracket_depth == 0 && !buffer.is_empty() {
            if let Err(e) = execute_line(eval, &buffer, true) {
                eprintln!("Warning: {} line {}: {}", source, start_line, e);
            }
            eval.clear_stack();
            buffer.clear();
        }
    }

    // Handle any remaining content (shouldn't happen with valid files)
    if !buffer.is_empty() {
        if let Err(e) = execute_line(eval, &buffer, true) {
            eprintln!("Warning: {} line {}: {}", source, start_line, e);
        }
        eval.clear_stack();
    }
}

/// Initialize hsab stdlib: create ~/.hsab/lib/ and install stdlib.hsabrc
fn run_init() -> ExitCode {
    let home = match dirs_home() {
        Some(h) => h,
        None => {
            eprintln!("Error: Could not determine home directory");
            return ExitCode::FAILURE;
        }
    };

    let lib_dir = home.join(".hsab").join("lib");
    let stdlib_file = lib_dir.join("stdlib.hsabrc");

    // Create directory if it doesn't exist
    if let Err(e) = fs::create_dir_all(&lib_dir) {
        eprintln!("Error creating {}: {}", lib_dir.display(), e);
        return ExitCode::FAILURE;
    }

    // Check if stdlib already exists
    if stdlib_file.exists() {
        println!("Stdlib already installed at {}", stdlib_file.display());
        println!("To reinstall, remove the file first:");
        println!("  rm {}", stdlib_file.display());
        return ExitCode::SUCCESS;
    }

    // Write stdlib content
    if let Err(e) = fs::write(&stdlib_file, STDLIB_CONTENT) {
        eprintln!("Error writing {}: {}", stdlib_file.display(), e);
        return ExitCode::FAILURE;
    }

    println!("✓ Installed stdlib to {}", stdlib_file.display());
    println!();
    println!("The stdlib is now auto-loaded on startup. It includes:");
    println!("  • Arithmetic: abs, min, max, inc, dec");
    println!("  • String predicates: contains?, starts?, ends?");
    println!("  • Navigation: ll, la, l1, lt, lS");
    println!("  • Path ops: dirname, basename, reext, backup");
    println!("  • Git shortcuts: gs, gd, gl, ga, gcm");
    println!("  • Stack helpers: nip, tuck, -rot, 2drop, 2dup");
    println!("  • And more... see: {}", stdlib_file.display());

    ExitCode::SUCCESS
}

/// Embedded stdlib content (compiled into binary)
const STDLIB_CONTENT: &str = include_str!("../examples/stdlib.hsabrc");

/// Parse command-line arguments
struct CliArgs {
    login: bool,
    command: Option<String>,
    script: Option<String>,
    help: bool,
    version: bool,
    init: bool,
    trace: bool,
}

fn parse_args(args: &[String]) -> CliArgs {
    let mut cli = CliArgs {
        login: false,
        command: None,
        script: None,
        help: false,
        version: false,
        init: false,
        trace: false,
    };

    let mut i = 1; // Skip program name
    while i < args.len() {
        match args[i].as_str() {
            "init" => {
                cli.init = true;
            }
            "-l" | "--login" => {
                cli.login = true;
            }
            "--trace" => {
                cli.trace = true;
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

    // Handle init subcommand
    if cli.init {
        return run_init();
    }

    // Execute command with optional login mode
    if let Some(cmd) = cli.command {
        return execute_command_with_login(&cmd, cli.login, cli.trace);
    }

    // Execute script
    if let Some(script) = cli.script {
        return execute_script(&script, cli.trace);
    }

    // Start REPL (with optional login mode)
    match run_repl_with_login(cli.login, cli.trace) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("REPL error: {}", e);
            ExitCode::FAILURE
        }
    }
}

/// Execute a single command with optional login shell mode
fn execute_command_with_login(cmd: &str, is_login: bool, trace: bool) -> ExitCode {
    let mut eval = Evaluator::new();
    eval.set_trace_mode(trace);

    // Load profile if login shell
    if is_login {
        load_hsab_profile(&mut eval);
    }

    // Load stdlib first (provides defaults)
    load_stdlib(&mut eval);

    // Load ~/.hsabrc (user customizations override stdlib)
    load_hsabrc(&mut eval);

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
fn run_repl_with_login(is_login: bool, trace: bool) -> RlResult<()> {
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

    // Stack manipulation shortcuts:
    // - Alt+↑: Push first word from input to stack
    // - Alt+↓: Pop one from stack to input
    // - Ctrl+Alt+↑: Push ALL words from input to stack
    // - Ctrl+Alt+↓: Pop ALL from stack to input
    // - Ctrl+,: Clear stack (discard)
    // Note: Some terminals (iTerm2, Terminal.app) may need configuration:
    // - iTerm2: Preferences > Profiles > Keys > Option key acts as: Esc+
    // - Terminal.app: Preferences > Profiles > Keyboard > Use Option as Meta key

    // Bind Alt+Down to pop one from stack to input
    rl.bind_sequence(
        KeyEvent(KeyCode::Down, Modifiers::ALT),
        rustyline::EventHandler::Conditional(Box::new(PopToInputHandler {
            state: Arc::clone(&shared_state),
        })),
    );

    // Bind Alt+a to pop ALL from stack to input
    // (Letter-based shortcuts are more reliable than modifier+arrow)
    rl.bind_sequence(
        KeyEvent(KeyCode::Char('a'), Modifiers::ALT),
        rustyline::EventHandler::Conditional(Box::new(PopAllToInputHandler {
            state: Arc::clone(&shared_state),
        })),
    );

    // Bind Alt+Up to push first word from input to stack
    rl.bind_sequence(
        KeyEvent(KeyCode::Up, Modifiers::ALT),
        rustyline::EventHandler::Conditional(Box::new(PushToStackHandler {
            state: Arc::clone(&shared_state),
        })),
    );

    // Bind Alt+A (Alt+Shift+a) to push ALL words from input to stack
    rl.bind_sequence(
        KeyEvent(KeyCode::Char('A'), Modifiers::ALT),
        rustyline::EventHandler::Conditional(Box::new(PushAllToStackHandler {
            state: Arc::clone(&shared_state),
        })),
    );

    // Keep Ctrl+O as alternative for pop (compatibility with all terminals)
    rl.bind_sequence(
        KeyEvent(KeyCode::Char('O'), Modifiers::CTRL),
        rustyline::EventHandler::Conditional(Box::new(PopToInputHandler {
            state: Arc::clone(&shared_state),
        })),
    );

    // Bind Alt+k to clear/discard the stack (k = kill, like Ctrl+K in readline)
    rl.bind_sequence(
        KeyEvent(KeyCode::Char('k'), Modifiers::ALT),
        rustyline::EventHandler::Conditional(Box::new(ClearStackHandler {
            state: Arc::clone(&shared_state),
        })),
    );

    let mut eval = Evaluator::new();
    eval.set_trace_mode(trace);

    // Load profile if login shell
    if is_login {
        load_hsab_profile(&mut eval);
    }

    // Load stdlib first (provides defaults)
    load_stdlib(&mut eval);

    // Load ~/.hsabrc (user customizations override stdlib)
    load_hsabrc(&mut eval);

    // Extract hint format from STACK_HINT definition (for real-time stack display)
    {
        let format = extract_hint_format(&mut eval);
        let mut state = shared_state.lock().unwrap();
        state.hint_format = format;
    }

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
    // Command counter for $_CMD_NUM
    let mut cmd_num: usize = 0;
    // Fallback prompts if PS1/PS2 not defined
    let fallback_normal = format!("hsab-{}£ ", VERSION);
    let fallback_stack = format!("hsab-{}¢ ", VERSION);
    let fallback_multiline = format!("hsab-{}… ", VERSION);

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

        // Set prompt context variables before generating prompt
        set_prompt_context(&eval, cmd_num);

        // Determine which prompt to use
        let prompt: String = if !multiline_buffer.is_empty() {
            // Multiline: try PS2, fallback to default
            eval_prompt_definition(&mut eval, "PS2")
                .unwrap_or_else(|| fallback_multiline.clone())
        } else {
            // Normal: try PS1, fallback to default
            eval_prompt_definition(&mut eval, "PS1")
                .unwrap_or_else(|| {
                    // Use fallback with £/¢ based on stack
                    let has_stack = eval.stack().iter().any(|v| v.as_arg().is_some());
                    if !prefill.is_empty() || has_stack {
                        fallback_stack.clone()
                    } else {
                        fallback_normal.clone()
                    }
                })
        };

        // Use readline_with_initial if we have prefill from .use command
        let readline = if prefill.is_empty() || !multiline_buffer.is_empty() {
            rl.readline(&prompt)
        } else {
            let initial = format!("{} ", prefill); // Add space after prefill
            prefill.clear();
            rl.readline_with_initial(&prompt, (&initial, ""))
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
                        // Increment command counter
                        cmd_num += 1;
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
