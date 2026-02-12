//! Evaluator for hsab v2 - Stack-based execution with native command execution
//!
//! The evaluator maintains a stack and executes expressions:
//! - Literals push themselves to the stack
//! - Executables pop args, run, push output
//! - Blocks are deferred execution units
//! - Operators manipulate the stack or control execution
//!
//! # Builtin Dispatch Pattern
//!
//! Builtins are dispatched via two mechanisms:
//!
//! 1. **Expr enum variants in `eval_expr()`**: Language constructs that need special
//!    parsing or are fundamental stack operations. These are recognized by the lexer/parser
//!    and converted to specific Expr variants (e.g., `Expr::Dup`, `Expr::If`, `Expr::Pipe`).
//!    Examples: dup, swap, drop, if, times, while, pipe, marker, collect, spread, each
//!
//! 2. **String matching in `try_builtin()`**: Shell-like builtins and operations on
//!    structured data that consume arguments from the stack in LIFO order.
//!    Examples: cd, pwd, echo, file?, eq?, plus, record, get, to-json, sum
//!
//! The single source of truth for all builtins is `ExecutableResolver::default_builtins()`
//! in resolver.rs, which is used by `is_hsab_builtin()` to determine if a word is a builtin.

use crate::ast::{Expr, Program, Value};
use crate::resolver::ExecutableResolver;
use glob::glob;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(feature = "plugins")]
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use thiserror::Error;

#[cfg(feature = "plugins")]
use crate::plugin::PluginHost;

#[derive(Error, Debug)]
pub enum EvalError {
    #[error("Stack underflow: {0}")]
    StackUnderflow(String),
    #[error("Type error: expected {expected}, got {got}")]
    TypeError { expected: String, got: String },
    #[error("Execution error: {0}")]
    ExecError(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Break outside of loop")]
    BreakOutsideLoop,
    /// Internal: signals break from loop (not a real error)
    #[error("")]
    BreakLoop,
}

/// Result of evaluation
#[derive(Debug, Clone)]
pub struct EvalResult {
    /// Final output (from last command or explicit output)
    pub output: String,
    /// Exit code of last command
    pub exit_code: i32,
    /// Remaining stack (for inspection/debugging)
    pub stack: Vec<Value>,
}

/// Job tracking for background processes
#[derive(Debug)]
struct Job {
    id: usize,
    pid: u32,
    pgid: u32,  // Process group ID for signal delivery
    command: String,
    #[allow(dead_code)]
    child: Option<Child>,
    status: JobStatus,
}

#[derive(Debug, Clone, PartialEq)]
enum JobStatus {
    Running,
    #[allow(dead_code)]
    Stopped,
    Done(i32),
}

/// The evaluator maintains state and executes programs
pub struct Evaluator {
    /// The value stack
    stack: Vec<Value>,
    /// Executable resolver for detecting commands
    resolver: ExecutableResolver,
    /// Last exit code
    last_exit_code: i32,
    /// User-defined words (functions)
    definitions: HashMap<String, Vec<Expr>>,
    /// Current working directory
    cwd: PathBuf,
    /// Home directory for ~ expansion
    home_dir: String,
    /// Background jobs
    jobs: Vec<Job>,
    /// Next job ID
    next_job_id: usize,
    /// Exit codes from last pipeline
    pipestatus: Vec<i32>,
    /// Whether to capture command output (vs run interactively)
    /// True when output will be consumed by next command/operator
    capture_mode: bool,
    /// Directory stack for pushd/popd
    dir_stack: Vec<PathBuf>,
    /// Command aliases - maps name to expansion (block of expressions)
    aliases: HashMap<String, Vec<Expr>>,
    /// Signal traps (signal number -> block to execute)
    traps: HashMap<i32, Vec<Expr>>,
    /// Stack of local variable scopes (for nested definitions)
    /// Each scope maps var name -> original value (None if didn't exist)
    local_scopes: Vec<HashMap<String, Option<String>>>,
    /// Flag to signal early return from a definition
    returning: bool,
    /// Trace mode - print stack after each operation
    trace_mode: bool,
    /// Debug mode - enable step debugger
    debug_mode: bool,
    /// Step mode - pause before each expression
    step_mode: bool,
    /// Breakpoints - expression patterns to pause on
    breakpoints: std::collections::HashSet<String>,
    /// Loaded modules (by canonical path) to prevent double-loading
    loaded_modules: std::collections::HashSet<PathBuf>,
    /// Current definition call depth (for recursion limit)
    call_depth: usize,
    /// Maximum recursion depth (default 10000, configurable via HSAB_MAX_RECURSION)
    max_call_depth: usize,
    /// Plugin host for WASM plugin support
    #[cfg(feature = "plugins")]
    plugin_host: Option<PluginHost>,
    /// Shared stack reference for plugins
    #[cfg(feature = "plugins")]
    shared_stack: Arc<Mutex<Vec<Value>>>,
}

impl Default for Evaluator {
    fn default() -> Self {
        Self::new()
    }
}

impl Evaluator {
    pub fn new() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/".to_string());
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));

        // Create shared stack for plugins
        #[cfg(feature = "plugins")]
        let shared_stack = Arc::new(Mutex::new(Vec::new()));

        // Initialize plugin host
        #[cfg(feature = "plugins")]
        let plugin_host = {
            match PluginHost::new(Arc::clone(&shared_stack)) {
                Ok(mut host) => {
                    // Auto-load plugins from ~/.hsab/plugins/
                    if let Err(e) = host.load_plugins_dir() {
                        eprintln!("Warning: Failed to load plugins: {}", e);
                    }
                    Some(host)
                }
                Err(e) => {
                    eprintln!("Warning: Failed to initialize plugin system: {}", e);
                    None
                }
            }
        };

        Evaluator {
            stack: Vec::new(),
            resolver: ExecutableResolver::new(),
            last_exit_code: 0,
            definitions: HashMap::new(),
            cwd,
            home_dir: home,
            jobs: Vec::new(),
            next_job_id: 1,
            pipestatus: Vec::new(),
            capture_mode: false,
            dir_stack: Vec::new(),
            aliases: HashMap::new(),
            traps: HashMap::new(),
            local_scopes: Vec::new(),
            returning: false,
            trace_mode: false,
            debug_mode: false,
            step_mode: false,
            breakpoints: std::collections::HashSet::new(),
            loaded_modules: std::collections::HashSet::new(),
            call_depth: 0,
            max_call_depth: std::env::var("HSAB_MAX_RECURSION")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(10000),
            #[cfg(feature = "plugins")]
            plugin_host,
            #[cfg(feature = "plugins")]
            shared_stack,
        }
    }

    /// Get a reference to the current stack (for debugging)
    pub fn stack(&self) -> &[Value] {
        &self.stack
    }

    /// Enable or disable trace mode
    pub fn set_trace_mode(&mut self, enabled: bool) {
        self.trace_mode = enabled;
    }

    // === Debugger control methods ===

    /// Enable or disable debug mode
    pub fn set_debug_mode(&mut self, enabled: bool) {
        self.debug_mode = enabled;
        if !enabled {
            self.step_mode = false;
        }
    }

    /// Check if debug mode is enabled
    pub fn is_debug_mode(&self) -> bool {
        self.debug_mode
    }

    /// Enable step mode (pause before each expression)
    pub fn set_step_mode(&mut self, enabled: bool) {
        self.step_mode = enabled;
    }

    /// Check if step mode is enabled
    pub fn is_step_mode(&self) -> bool {
        self.step_mode
    }

    /// Add a breakpoint on an expression pattern
    pub fn add_breakpoint(&mut self, pattern: String) {
        self.breakpoints.insert(pattern);
    }

    /// Remove a breakpoint
    pub fn remove_breakpoint(&mut self, pattern: &str) -> bool {
        self.breakpoints.remove(pattern)
    }

    /// Clear all breakpoints
    pub fn clear_breakpoints(&mut self) {
        self.breakpoints.clear();
    }

    /// Get all breakpoints
    pub fn breakpoints(&self) -> &std::collections::HashSet<String> {
        &self.breakpoints
    }

    /// Check if an expression matches any breakpoint
    fn matches_breakpoint(&self, expr: &Expr) -> bool {
        if self.breakpoints.is_empty() {
            return false;
        }
        let expr_str = self.expr_to_string(expr);
        self.breakpoints.iter().any(|bp| expr_str.contains(bp))
    }

    /// Convert an expression to a string for breakpoint matching
    fn expr_to_string(&self, expr: &Expr) -> String {
        match expr {
            Expr::Literal(s) => s.clone(),
            Expr::Quoted { content, .. } => format!("\"{}\"", content),
            Expr::Variable(s) => format!("${}", s),
            Expr::Block(_) => "[block]".to_string(),
            Expr::Apply => "@".to_string(),
            Expr::Pipe => "|".to_string(),
            Expr::Dup => "dup".to_string(),
            Expr::Swap => "swap".to_string(),
            Expr::Drop => "drop".to_string(),
            Expr::Over => "over".to_string(),
            Expr::Rot => "rot".to_string(),
            Expr::Depth => "depth".to_string(),
            Expr::Join => "path-join".to_string(),
            Expr::Suffix => "suffix".to_string(),
            Expr::Dirname => "dirname".to_string(),
            Expr::Basename => "basename".to_string(),
            Expr::Split1 => "split1".to_string(),
            Expr::Rsplit1 => "rsplit1".to_string(),
            Expr::Marker => "marker".to_string(),
            Expr::Spread => "spread".to_string(),
            Expr::Each => "each".to_string(),
            Expr::Keep => "keep".to_string(),
            Expr::Collect => "collect".to_string(),
            Expr::Map => "map".to_string(),
            Expr::Filter => "filter".to_string(),
            Expr::If => "if".to_string(),
            Expr::Times => "times".to_string(),
            Expr::While => "while".to_string(),
            Expr::Until => "until".to_string(),
            Expr::Break => "break".to_string(),
            Expr::Parallel => "parallel".to_string(),
            Expr::Fork => "fork".to_string(),
            Expr::Subst => "subst".to_string(),
            Expr::Fifo => "fifo".to_string(),
            Expr::Json => "json".to_string(),
            Expr::Unjson => "unjson".to_string(),
            Expr::Timeout => "timeout".to_string(),
            Expr::Pipestatus => "pipestatus".to_string(),
            Expr::Import => ".import".to_string(),
            Expr::Background => "&".to_string(),
            Expr::Define(name) => format!(":{}:", name),
            Expr::RedirectOut | Expr::RedirectAppend | Expr::RedirectIn => ">".to_string(),
            Expr::RedirectErr | Expr::RedirectErrAppend | Expr::RedirectBoth => "2>".to_string(),
            Expr::RedirectErrToOut => "2>&1".to_string(),
            Expr::And => "&&".to_string(),
            Expr::Or => "||".to_string(),
            Expr::ScopedBlock { .. } => "(...)".to_string(),
        }
    }

    /// Format current debug state for display
    pub fn format_debug_state(&self, expr: &Expr) -> String {
        let expr_str = self.expr_to_string(expr);

        // Format stack (show all items, max 10)
        let stack_items: Vec<String> = self.stack.iter().enumerate()
            .map(|(i, v)| {
                let val_str = match v {
                    Value::Literal(s) => {
                        if s.len() > 30 {
                            format!("\"{}...\"", &s[..27])
                        } else {
                            format!("\"{}\"", s)
                        }
                    }
                    Value::Number(n) => format!("{}", n),
                    Value::Bool(b) => format!("{}", b),
                    Value::Output(s) => {
                        let trimmed = s.trim();
                        if trimmed.len() > 30 {
                            format!("out:\"{}...\"", &trimmed[..27])
                        } else {
                            format!("out:\"{}\"", trimmed)
                        }
                    }
                    Value::Block(exprs) => format!("[block:{}]", exprs.len()),
                    Value::Map(m) => format!("{{record:{}}}", m.len()),
                    Value::Table { rows, .. } => format!("<table:{}>", rows.len()),
                    Value::List(items) => format!("[list:{}]", items.len()),
                    Value::Nil => "nil".to_string(),
                    Value::Marker => "|marker|".to_string(),
                    Value::Error { message, .. } => format!("Error:{}", message),
                };
                format!("  {}. {}", i, val_str)
            })
            .collect();

        let stack_str = if stack_items.is_empty() {
            "  (empty)".to_string()
        } else {
            stack_items.join("\n")
        };

        format!(
            "\x1b[33m╔══ DEBUG ═══════════════════════════════════════\x1b[0m\n\
             \x1b[33m║\x1b[0m \x1b[1mExpr:\x1b[0m {}\n\
             \x1b[33m║\x1b[0m \x1b[1mStack ({} items):\x1b[0m\n{}\n\
             \x1b[33m╚════════════════════════════════════════════════\x1b[0m",
            expr_str,
            self.stack.len(),
            stack_str
        )
    }

    /// Clear the stack
    pub fn clear_stack(&mut self) {
        self.stack.clear();
    }

    /// Pop a value from the stack (for REPL .pop command)
    pub fn pop_value(&mut self) -> Option<Value> {
        self.stack.pop()
    }

    /// Push a value to the stack (for REPL Ctrl+Alt+← shortcut)
    pub fn push_value(&mut self, value: Value) {
        self.stack.push(value);
    }

    /// Pop N items from the stack and return as a space-separated string.
    /// Used by `.use N` REPL command to move stack items to input.
    pub fn pop_n_as_string(&mut self, n: usize) -> String {
        let mut items = Vec::new();
        for _ in 0..n {
            if let Some(value) = self.stack.pop() {
                if let Some(s) = value.as_arg() {
                    items.push(s);
                }
            } else {
                break;
            }
        }
        // Reverse because we popped in LIFO order
        items.reverse();
        items.join(" ")
    }

    /// Get the number of items on the stack
    pub fn stack_len(&self) -> usize {
        self.stack.len()
    }

    /// Get names of all user-defined words (for tab completion)
    pub fn definition_names(&self) -> std::collections::HashSet<String> {
        self.definitions.keys().cloned().collect()
    }

    /// Check if a definition exists
    pub fn has_definition(&self, name: &str) -> bool {
        self.definitions.contains_key(name)
    }

    /// Restore stack from a saved state
    pub fn restore_stack(&mut self, stack: Vec<Value>) {
        self.stack = stack;
    }

    /// Get the last exit code
    pub fn last_exit_code(&self) -> i32 {
        self.last_exit_code
    }

    /// Set the last exit code (used to restore after prompt evaluation)
    pub fn set_last_exit_code(&mut self, code: i32) {
        self.last_exit_code = code;
    }

    /// Get the number of background jobs
    pub fn job_count(&self) -> usize {
        self.jobs.len()
    }

    /// Get the current working directory
    pub fn cwd(&self) -> &std::path::PathBuf {
        &self.cwd
    }

    /// Expand tilde (~) to home directory
    fn expand_tilde(&self, path: &str) -> String {
        if path == "~" {
            return self.home_dir.clone();
        }
        if let Some(rest) = path.strip_prefix("~/") {
            return format!("{}/{}", self.home_dir, rest);
        }
        path.to_string()
    }

    /// Interpolate variables in a double-quoted string
    /// Supports $VAR and ${VAR} syntax
    fn interpolate_string(&self, s: &str) -> String {
        let mut result = String::with_capacity(s.len());
        let mut chars = s.chars().peekable();

        while let Some(c) = chars.next() {
            if c == '$' {
                if chars.peek() == Some(&'{') {
                    // ${VAR} syntax
                    chars.next(); // consume '{'
                    let mut var_name = String::new();
                    while let Some(&ch) = chars.peek() {
                        if ch == '}' {
                            chars.next(); // consume '}'
                            break;
                        }
                        var_name.push(chars.next().unwrap());
                    }
                    if let Ok(val) = std::env::var(&var_name) {
                        result.push_str(&val);
                    }
                } else if chars.peek().map(|c| c.is_ascii_alphabetic() || *c == '_').unwrap_or(false) {
                    // $VAR syntax - collect alphanumeric and underscore
                    let mut var_name = String::new();
                    while let Some(&ch) = chars.peek() {
                        if ch.is_ascii_alphanumeric() || ch == '_' {
                            var_name.push(chars.next().unwrap());
                        } else {
                            break;
                        }
                    }
                    if let Ok(val) = std::env::var(&var_name) {
                        result.push_str(&val);
                    }
                } else {
                    // Lone $ or $followed-by-non-alpha
                    result.push('$');
                }
            } else if c == '\\' {
                // Handle escape sequences
                if let Some(&next) = chars.peek() {
                    match next {
                        '$' => {
                            chars.next();
                            result.push('$');
                        }
                        '\\' => {
                            chars.next();
                            result.push('\\');
                        }
                        _ => result.push(c),
                    }
                } else {
                    result.push(c);
                }
            } else {
                result.push(c);
            }
        }

        result
    }

    /// Expand glob patterns in a string
    fn expand_glob(&self, pattern: &str) -> Vec<String> {
        // Only expand if contains glob characters
        if !pattern.contains('*') && !pattern.contains('?') && !pattern.contains('[') {
            return vec![pattern.to_string()];
        }

        // Don't glob-expand words that end with ? if they look like predicates
        // (e.g., file?, dir?, eq?, lt?, ge?, contains?)
        if pattern.ends_with('?') && !pattern.contains('/') && !pattern.contains('*') {
            // Check if it's a single word (predicate name)
            if !pattern.chars().any(|c| c.is_whitespace()) {
                return vec![pattern.to_string()];
            }
        }

        // Expand relative to current working directory
        let full_pattern = if pattern.starts_with('/') {
            pattern.to_string()
        } else {
            format!("{}/{}", self.cwd.display(), pattern)
        };

        match glob(&full_pattern) {
            Ok(paths) => {
                let expanded: Vec<String> = paths
                    .filter_map(|p| p.ok())
                    .map(|p| p.to_string_lossy().to_string())
                    .collect();
                if expanded.is_empty() {
                    vec![pattern.to_string()] // No matches, return original
                } else {
                    expanded
                }
            }
            Err(_) => vec![pattern.to_string()],
        }
    }

    /// Expand both tilde and glob
    fn expand_arg(&self, arg: &str) -> Vec<String> {
        let expanded = self.expand_tilde(arg);
        self.expand_glob(&expanded)
    }

    /// Evaluate a program
    pub fn eval(&mut self, program: &Program) -> Result<EvalResult, EvalError> {
        self.eval_exprs(&program.expressions)?;

        // Collect output from stack
        let output = self
            .stack
            .iter()
            .filter_map(|v| v.as_arg())
            .collect::<Vec<_>>()
            .join("\n");

        Ok(EvalResult {
            output,
            exit_code: self.last_exit_code,
            stack: self.stack.clone(),
        })
    }

    /// Evaluate a list of expressions with look-ahead for capture mode
    fn eval_exprs(&mut self, exprs: &[Expr]) -> Result<(), EvalError> {
        for (i, expr) in exprs.iter().enumerate() {
            // Debug mode: check for breakpoints and step mode
            if self.debug_mode {
                let should_pause = self.step_mode || self.matches_breakpoint(expr);
                if should_pause {
                    // Show debug state
                    eprintln!("{}", self.format_debug_state(expr));
                    eprintln!("\x1b[90m(n)ext, (c)ontinue, (s)tack, (q)uit debug: \x1b[0m");

                    // Read debug command from stdin
                    loop {
                        let mut input = String::new();
                        if std::io::stdin().read_line(&mut input).is_ok() {
                            let cmd = input.trim().to_lowercase();
                            match cmd.as_str() {
                                "n" | "next" | "" => {
                                    // Step to next expression
                                    self.step_mode = true;
                                    break;
                                }
                                "c" | "continue" => {
                                    // Continue until next breakpoint
                                    self.step_mode = false;
                                    break;
                                }
                                "s" | "stack" => {
                                    // Show full stack
                                    eprintln!("\x1b[33mStack ({} items):\x1b[0m", self.stack.len());
                                    for (idx, val) in self.stack.iter().enumerate() {
                                        eprintln!("  {}. {:?}", idx, val);
                                    }
                                    eprintln!("\x1b[90m(n)ext, (c)ontinue, (s)tack, (q)uit debug: \x1b[0m");
                                }
                                "q" | "quit" => {
                                    // Quit debug mode
                                    self.debug_mode = false;
                                    self.step_mode = false;
                                    eprintln!("\x1b[33mDebug mode disabled\x1b[0m");
                                    break;
                                }
                                "b" | "breakpoints" => {
                                    // List breakpoints
                                    if self.breakpoints.is_empty() {
                                        eprintln!("\x1b[33mNo breakpoints set\x1b[0m");
                                    } else {
                                        eprintln!("\x1b[33mBreakpoints:\x1b[0m");
                                        for bp in &self.breakpoints {
                                            eprintln!("  - {}", bp);
                                        }
                                    }
                                    eprintln!("\x1b[90m(n)ext, (c)ontinue, (s)tack, (q)uit debug: \x1b[0m");
                                }
                                _ => {
                                    eprintln!("\x1b[31mUnknown command: {}\x1b[0m", cmd);
                                    eprintln!("\x1b[90m(n)ext, (c)ontinue, (s)tack, (q)uit debug: \x1b[0m");
                                }
                            }
                        } else {
                            break;
                        }
                    }
                }
            }

            // Look ahead to determine if output should be captured
            // Pass remaining expressions so we can look past blocks
            let remaining = &exprs[i + 1..];
            self.capture_mode = self.should_capture(remaining);

            match self.eval_expr(expr) {
                Ok(()) => {
                    // Trace mode: print expression and stack state
                    if self.trace_mode {
                        self.print_trace(expr);
                    }
                }
                Err(EvalError::BreakLoop) => return Err(EvalError::BreakOutsideLoop),
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    /// Print trace output showing expression and stack state
    fn print_trace(&self, expr: &Expr) {
        // Format expression
        let expr_str = match expr {
            Expr::Literal(s) => s.clone(),
            Expr::Quoted { content, .. } => format!("\"{}\"", content),
            Expr::Variable(s) => s.clone(),
            Expr::Block(_) => "[...]".to_string(),
            Expr::Apply => "@".to_string(),
            Expr::Pipe => "|".to_string(),
            Expr::Dup => "dup".to_string(),
            Expr::Swap => "swap".to_string(),
            Expr::Drop => "drop".to_string(),
            Expr::Over => "over".to_string(),
            Expr::Rot => "rot".to_string(),
            _ => format!("{:?}", expr),
        };

        // Format stack (show top 5 items)
        let stack_items: Vec<String> = self.stack.iter().rev().take(5)
            .map(|v| match v {
                Value::Literal(s) => format!("\"{}\"", s),
                Value::Number(n) => format!("{}", n),
                Value::Bool(b) => format!("{}", b),
                Value::Output(s) => {
                    let trimmed = s.trim();
                    if trimmed.len() > 20 {
                        format!("{}...", &trimmed[..17])
                    } else {
                        trimmed.to_string()
                    }
                }
                Value::Block(_) => "[...]".to_string(),
                Value::Map(_) => "{...}".to_string(),
                Value::Table { rows, .. } => format!("<table:{}>", rows.len()),
                Value::List(items) => format!("[{}]", items.len()),
                Value::Nil => "nil".to_string(),
                Value::Marker => "|".to_string(),
                Value::Error { .. } => "Error".to_string(),
            })
            .collect();

        let stack_str = if stack_items.is_empty() {
            "(empty)".to_string()
        } else {
            stack_items.into_iter().rev().collect::<Vec<_>>().join(" ")
        };

        eprintln!("\x1b[90m>>> {} │ {}\x1b[0m", expr_str, stack_str);
    }

    /// Determine if output should be captured based on what comes next
    /// Looks past blocks to find consuming operations like pipes
    fn should_capture(&mut self, remaining: &[Expr]) -> bool {
        let next = remaining.first();
        match next {
            None => false, // End of input - run interactively
            Some(expr) => match expr {
                // These consume stack values
                Expr::Pipe => true,
                Expr::RedirectOut | Expr::RedirectAppend | Expr::RedirectIn => true,
                Expr::RedirectErr | Expr::RedirectErrAppend | Expr::RedirectBoth => true,
                Expr::And | Expr::Or => true,
                Expr::Apply => true,

                // Stack operations consume values
                Expr::Dup | Expr::Swap | Expr::Drop | Expr::Over | Expr::Rot | Expr::Depth => true,

                // Path/String operations consume values
                Expr::Join | Expr::Suffix | Expr::Dirname | Expr::Basename => true,
                Expr::Split1 | Expr::Rsplit1 => true,

                // List operations (Marker just pushes, doesn't consume)
                Expr::Marker => false,
                Expr::Spread | Expr::Each | Expr::Keep | Expr::Collect => true,
                Expr::Map | Expr::Filter => true,

                // Control flow consumes blocks/values
                Expr::If | Expr::Times | Expr::While | Expr::Until => true,

                // Parallel execution
                Expr::Parallel | Expr::Fork => true,

                // Process substitution
                Expr::Subst | Expr::Fifo => true,

                // JSON operations
                Expr::Json | Expr::Unjson => true,

                // Other operations
                Expr::Timeout | Expr::Pipestatus | Expr::Import => true,
                Expr::Background => true,
                Expr::Define(_) => true,

                // Literals: if it's an executable, it will consume args
                Expr::Literal(s) => {
                    self.definitions.contains_key(s)
                        || self.resolver.is_executable(s)
                        || ExecutableResolver::is_hsab_builtin(s)
                }

                // Quoted strings and variables are just pushed, don't consume
                Expr::Quoted { .. } => false,
                Expr::Variable(_) => false,

                // Blocks are just pushed, but look past them to see if
                // there's a consuming operation after (like pipe)
                Expr::Block(_) => self.should_capture(&remaining[1..]),

                // Break doesn't consume
                Expr::Break => false,

                // Redirect variants we missed
                Expr::RedirectErrToOut => true,

                // Scoped blocks - look inside the body
                Expr::ScopedBlock { body, .. } => {
                    if body.is_empty() {
                        false
                    } else {
                        // Check if body's first expression is consuming
                        self.should_capture(body)
                    }
                }
            },
        }
    }

    /// Evaluate a single expression
    fn eval_expr(&mut self, expr: &Expr) -> Result<(), EvalError> {
        match expr {
            Expr::Literal(s) => {
                // Check if it's a user-defined word first
                if let Some(body) = self.definitions.get(s).cloned() {
                    // Check recursion limit before executing
                    if self.call_depth >= self.max_call_depth {
                        return Err(EvalError::ExecError(
                            format!("Recursion limit exceeded ({} calls). Set HSAB_MAX_RECURSION to increase.",
                                    self.max_call_depth)
                        ));
                    }
                    self.call_depth += 1;

                    // Execute the defined word's body with local scope support
                    self.local_scopes.push(HashMap::new());
                    self.returning = false;

                    let mut exec_result = Ok(());
                    for e in &body {
                        if self.returning {
                            break;
                        }
                        if let Err(e) = self.eval_expr(e) {
                            exec_result = Err(e);
                            break;
                        }
                    }

                    // Restore local variables
                    if let Some(scope) = self.local_scopes.pop() {
                        for (name, original) in scope {
                            match original {
                                Some(value) => std::env::set_var(&name, value),
                                None => std::env::remove_var(&name),
                            }
                        }
                    }
                    self.returning = false;

                    // Decrement call depth after execution
                    self.call_depth -= 1;

                    // Return any error that occurred during execution
                    exec_result?;
                } else if let Some(body) = self.aliases.get(s).cloned() {
                    // Check if it's an alias - execute the alias body
                    for e in &body {
                        self.eval_expr(e)?;
                    }
                } else if s == "." && !self.stack.is_empty() {
                    // Special case: "." is source command only when there's something to source
                    // This allows "." alone to be treated as current directory literal,
                    // while "file.hsab ." works as source command
                    self.execute_command(".")?;
                } else if self.try_structured_builtin(s)? {
                    // Handled as structured data builtin (typeof, record, get, etc.)
                } else if self.try_plugin_command_if_enabled(s)? {
                    // Handled as plugin command
                } else if self.resolver.is_executable(s) {
                    // Check if it's an executable
                    self.execute_command(s)?;
                } else {
                    // Push as literal
                    self.stack.push(Value::Literal(s.clone()));
                }
            }

            Expr::Quoted { content, double } => {
                // Push the content without surrounding quotes - quotes are just delimiters
                // Double-quoted strings support variable interpolation
                let result = if *double {
                    self.interpolate_string(content)
                } else {
                    content.clone()
                };
                self.stack.push(Value::Literal(result));
            }

            Expr::Variable(s) => {
                // Expand variable using std::env
                let var_name = s
                    .trim_start_matches('$')
                    .trim_start_matches('{')
                    .trim_end_matches('}');
                match std::env::var(var_name) {
                    Ok(value) => self.stack.push(Value::Literal(value)),
                    Err(_) => self.stack.push(Value::Literal(String::new())),
                }
            }

            Expr::Block(inner) => {
                // Push block as deferred execution
                self.stack.push(Value::Block(inner.clone()));
            }

            Expr::Apply => {
                self.apply_block()?;
            }

            Expr::Pipe => {
                self.execute_pipe()?;
            }

            Expr::RedirectOut => {
                self.execute_redirect(">")?;
            }

            Expr::RedirectAppend => {
                self.execute_redirect(">>")?;
            }

            Expr::RedirectIn => {
                self.execute_redirect("<")?;
            }

            Expr::RedirectErr => {
                self.execute_redirect_err("2>")?;
            }

            Expr::RedirectErrAppend => {
                self.execute_redirect_err("2>>")?;
            }

            Expr::RedirectBoth => {
                self.execute_redirect_both()?;
            }

            Expr::RedirectErrToOut => {
                // 2>&1 redirects stderr to stdout for the command block on the stack
                self.execute_redirect_err_to_out()?;
            }

            Expr::Background => {
                self.execute_background()?;
            }

            Expr::And => {
                self.execute_and()?;
            }

            Expr::Or => {
                self.execute_or()?;
            }

            // Stack operations
            Expr::Dup => self.stack_dup()?,
            Expr::Swap => self.stack_swap()?,
            Expr::Drop => self.stack_drop()?,
            Expr::Over => self.stack_over()?,
            Expr::Rot => self.stack_rot()?,
            Expr::Depth => self.stack_depth()?,

            // Path operations
            Expr::Join => self.path_join()?,
            Expr::Suffix => self.path_suffix()?,
            Expr::Dirname => self.path_dirname()?,
            Expr::Basename => self.path_basename()?,

            // String operations
            Expr::Split1 => self.string_split1()?,
            Expr::Rsplit1 => self.string_rsplit1()?,

            // List operations
            Expr::Marker => self.stack.push(Value::Marker),
            Expr::Spread => self.list_spread()?,
            Expr::Each => self.list_each()?,
            Expr::Collect => self.list_collect()?,
            Expr::Keep => self.list_keep()?,
            Expr::Map => self.list_map()?,
            Expr::Filter => self.list_filter()?,

            // Control flow
            Expr::If => self.control_if()?,
            Expr::Times => self.control_times()?,
            Expr::While => self.control_while()?,
            Expr::Until => self.control_until()?,
            Expr::Break => return Err(EvalError::BreakLoop),

            // Parallel execution
            Expr::Parallel => self.exec_parallel()?,
            Expr::Fork => self.exec_fork()?,

            // Process substitution
            Expr::Subst => self.process_subst()?,
            Expr::Fifo => self.process_fifo()?,

            // JSON / Structured data
            Expr::Json => self.json_parse()?,
            Expr::Unjson => self.json_stringify()?,

            // Resource limits
            Expr::Timeout => self.builtin_timeout()?,

            // Pipeline status
            Expr::Pipestatus => self.builtin_pipestatus()?,

            // Module system
            Expr::Import => self.module_import()?,

            Expr::Define(name) => {
                // Pop block from stack and store as named word
                let block = self.pop_block()?;
                self.definitions.insert(name.clone(), block);
            }

            Expr::ScopedBlock { assignments, body } => {
                self.eval_scoped_block(assignments, body)?;
            }
        }

        Ok(())
    }

    /// Evaluate a scoped block with temporary variable assignments
    /// Variables are set before body execution, then restored/unset after
    fn eval_scoped_block(
        &mut self,
        assignments: &[(String, String)],
        body: &[Expr],
    ) -> Result<(), EvalError> {
        // Save current values for any vars we're about to shadow
        let mut saved_vars: Vec<(String, Option<String>)> = Vec::new();

        for (name, _) in assignments {
            let current = std::env::var(name).ok();
            saved_vars.push((name.clone(), current));
        }

        // Set the new variable values
        for (name, value) in assignments {
            std::env::set_var(name, value);
        }

        // Execute the body
        let result = self.eval_exprs(body);

        // Restore/unset variables
        for (name, original) in saved_vars {
            match original {
                Some(value) => std::env::set_var(&name, value),
                None => std::env::remove_var(&name),
            }
        }

        result
    }

    /// Try to execute a builtin command
    fn try_builtin(&mut self, cmd: &str, args: &[String]) -> Option<Result<(), EvalError>> {
        match cmd {
            "cd" => Some(self.builtin_cd(args)),
            "pwd" => Some(self.builtin_pwd()),
            "echo" => Some(self.builtin_echo(args)),
            "true" => Some(self.builtin_true()),
            "false" => Some(self.builtin_false()),
            "test" | "[" => Some(self.builtin_test(args)),
            "export" => Some(self.builtin_export(args)),
            "unset" => Some(self.builtin_unset(args)),
            "env" => Some(self.builtin_env()),
            "jobs" => Some(self.builtin_jobs()),
            "fg" => Some(self.builtin_fg(args)),
            "bg" => Some(self.builtin_bg(args)),
            "exit" => Some(self.builtin_exit(args)),
            "tty" => Some(self.builtin_tty(args)),
            "which" => Some(self.builtin_which(args)),
            "source" | "." => Some(self.builtin_source(args)),
            "hash" => Some(self.builtin_hash(args)),
            "type" => Some(self.builtin_type(args)),
            "read" => Some(self.builtin_read(args)),
            "printf" => Some(self.builtin_printf(args)),
            "wait" => Some(self.builtin_wait(args)),
            "kill" => Some(self.builtin_kill(args)),
            "pushd" => Some(self.builtin_pushd(args)),
            "popd" => Some(self.builtin_popd(args)),
            "dirs" => Some(self.builtin_dirs(args)),
            "alias" => Some(self.builtin_alias(args)),
            "unalias" => Some(self.builtin_unalias(args)),
            "trap" => Some(self.builtin_trap(args)),
            "local" => Some(self.builtin_local(args)),
            "return" => Some(self.builtin_return(args)),
            // Stack-native predicates
            "file?" => Some(self.builtin_file_predicate(args)),
            "dir?" => Some(self.builtin_dir_predicate(args)),
            "exists?" => Some(self.builtin_exists_predicate(args)),
            "empty?" => Some(self.builtin_empty_predicate(args)),
            "eq?" => Some(self.builtin_eq_predicate(args)),
            "ne?" => Some(self.builtin_neq_predicate(args)),
            "=?" => Some(self.builtin_numeric_eq_predicate(args)),
            "!=?" => Some(self.builtin_numeric_neq_predicate(args)),
            "lt?" => Some(self.builtin_numeric_lt_predicate(args)),
            "gt?" => Some(self.builtin_numeric_gt_predicate(args)),
            "le?" => Some(self.builtin_numeric_le_predicate(args)),
            "ge?" => Some(self.builtin_numeric_ge_predicate(args)),
            // Arithmetic primitives
            "plus" => Some(self.builtin_add(args)),
            "minus" => Some(self.builtin_sub(args)),
            "mul" => Some(self.builtin_mul(args)),
            "div" => Some(self.builtin_div(args)),
            "mod" => Some(self.builtin_mod(args)),
            // String primitives
            "len" => Some(self.builtin_len(args)),
            "slice" => Some(self.builtin_slice(args)),
            "indexof" => Some(self.builtin_indexof(args)),
            "str-replace" => Some(self.builtin_str_replace(args)),
            "format" => Some(self.builtin_format(args)),
            // Path operations (native implementation for performance)
            "reext" => Some(self.builtin_reext(args)),
            // Phase 0: Type introspection
            "typeof" => Some(self.builtin_typeof()),
            // Phase 1: Record operations
            "record" => Some(self.builtin_record()),
            "get" => Some(self.builtin_get()),
            "set" => Some(self.builtin_set()),
            "del" => Some(self.builtin_del()),
            "has?" => Some(self.builtin_has()),
            "keys" => Some(self.builtin_keys()),
            "values" => Some(self.builtin_values()),
            "merge" => Some(self.builtin_merge()),
            // Phase 2: Table operations
            "table" => Some(self.builtin_table()),
            "where" => Some(self.builtin_where()),
            "sort-by" => Some(self.builtin_sort_by()),
            "select" => Some(self.builtin_select()),
            "first" => Some(self.builtin_first()),
            "last" => Some(self.builtin_last()),
            "nth" => Some(self.builtin_nth()),
            // Phase 3: Error handling
            "try" => Some(self.builtin_try()),
            "error?" => Some(self.builtin_error_predicate()),
            "throw" => Some(self.builtin_throw()),
            // Phase 4: Serialization bridge
            "into-json" => Some(self.builtin_into_json()),
            "into-csv" => Some(self.builtin_into_csv()),
            "into-lines" => Some(self.builtin_into_lines()),
            "into-kv" => Some(self.builtin_into_kv()),
            "to-json" => Some(self.builtin_to_json()),
            "to-csv" => Some(self.builtin_to_csv()),
            "to-lines" => Some(self.builtin_to_lines()),
            "to-kv" => Some(self.builtin_to_kv()),
            "to-tsv" => Some(self.builtin_to_tsv()),
            "to-delimited" => Some(self.builtin_to_delimited()),
            // File operations
            "save" => Some(self.builtin_save()),
            // Additional aggregations
            "reduce" => Some(self.builtin_reduce()),
            // Additional list/table operations
            "reject" => Some(self.builtin_reject()),
            "reject-where" => Some(self.builtin_reject_where()),
            "duplicates" => Some(self.builtin_duplicates()),
            // Vector operations (for embeddings)
            "dot-product" => Some(self.builtin_dot_product()),
            "magnitude" => Some(self.builtin_magnitude()),
            "normalize" => Some(self.builtin_normalize()),
            "cosine-similarity" => Some(self.builtin_cosine_similarity()),
            "euclidean-distance" => Some(self.builtin_euclidean_distance()),
            // Plugin management builtins
            #[cfg(feature = "plugins")]
            "plugin-load" => Some(self.builtin_plugin_load(args)),
            #[cfg(feature = "plugins")]
            "plugin-unload" => Some(self.builtin_plugin_unload(args)),
            #[cfg(feature = "plugins")]
            "plugin-reload" => Some(self.builtin_plugin_reload(args)),
            #[cfg(feature = "plugins")]
            "plugin-list" => Some(self.builtin_plugin_list()),
            #[cfg(feature = "plugins")]
            "plugin-info" => Some(self.builtin_plugin_info(args)),
            _ => None,
        }
    }

    /// Execute a command, popping args from stack
    fn execute_command(&mut self, cmd: &str) -> Result<(), EvalError> {
        // Collect args from stack (LIFO - pop until we hit a block, marker, or empty)
        let mut args = Vec::new();
        while let Some(value) = self.stack.last() {
            match value {
                Value::Block(_) => break,
                Value::Marker => break,
                Value::Nil => {
                    self.stack.pop();
                    // Skip nil values
                }
                _ => {
                    if let Some(arg) = value.as_arg() {
                        // Expand globs and tilde for each argument
                        args.extend(self.expand_arg(&arg));
                    }
                    self.stack.pop();
                }
            }
        }

        // Try builtin first
        if let Some(result) = self.try_builtin(cmd, &args) {
            return result;
        }

        // Execute native command
        let (output, exit_code) = self.execute_native(cmd, args)?;
        self.last_exit_code = exit_code;

        if output.is_empty() {
            self.stack.push(Value::Nil);
        } else {
            self.stack.push(Value::Output(output));
        }

        Ok(())
    }

    /// Execute a native command using std::process::Command
    /// Uses capture_mode to decide whether to capture output or run interactively
    fn execute_native(&mut self, cmd: &str, args: Vec<String>) -> Result<(String, i32), EvalError> {
        // Only run interactively if:
        // 1. capture_mode is false (nothing will consume the output)
        // 2. stdout is a TTY (we're in an interactive context)
        let run_interactive = !self.capture_mode && Self::is_interactive();

        if run_interactive {
            // Run interactively - output goes directly to terminal
            let status = Command::new(cmd)
                .args(&args)
                .current_dir(&self.cwd)
                .stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .status()
                .map_err(|e| EvalError::ExecError(format!("{}: {}", cmd, e)))?;

            Ok((String::new(), status.code().unwrap_or(-1)))
        } else {
            // Capture output (for piping, scripts, tests, or when output is consumed)
            let output = Command::new(cmd)
                .args(&args)
                .current_dir(&self.cwd)
                .output()
                .map_err(|e| EvalError::ExecError(format!("{}: {}", cmd, e)))?;

            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let exit_code = output.status.code().unwrap_or(-1);

            Ok((stdout, exit_code))
        }
    }

    /// Check if we're running in an interactive context (TTY)
    fn is_interactive() -> bool {
        use std::io::IsTerminal;
        std::io::stdout().is_terminal() && std::io::stdin().is_terminal()
    }

    /// Apply a block to args on the stack
    fn apply_block(&mut self) -> Result<(), EvalError> {
        let block = self.pop_block()?;

        // Save the outer capture mode - this applies to the block's final result
        let outer_capture_mode = self.capture_mode;

        // Evaluate the block's expressions with proper look-ahead
        // The last expression inherits the outer capture mode
        for (i, expr) in block.iter().enumerate() {
            let is_last = i == block.len() - 1;
            if is_last {
                // Last expression: use outer capture mode
                self.capture_mode = outer_capture_mode;
            } else {
                // Not last: look ahead within the block
                let remaining = &block[i + 1..];
                self.capture_mode = self.should_capture(remaining);
            }
            self.eval_expr(expr)?;
        }

        Ok(())
    }

    /// Execute a pipe: cmd1 [cmd2] |
    fn execute_pipe(&mut self) -> Result<(), EvalError> {
        // Pop the consumer block and producer output
        let consumer = self.pop_block()?;
        let input = self.pop_value_or_err()?;

        // Get input as string
        let input_str = input.as_arg().unwrap_or_default();

        // Build consumer command from block
        let (cmd, args) = self.block_to_cmd_args(&consumer)?;

        // Execute with stdin piped
        let mut child = Command::new(&cmd)
            .args(&args)
            .current_dir(&self.cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| EvalError::ExecError(format!("{}: {}", cmd, e)))?;

        // Write input to stdin
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(input_str.as_bytes());
        }

        let output = child
            .wait_with_output()
            .map_err(|e| EvalError::ExecError(e.to_string()))?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        self.last_exit_code = output.status.code().unwrap_or(-1);

        // Track pipestatus
        self.pipestatus.clear();
        self.pipestatus.push(self.last_exit_code);

        // Push result
        if stdout.is_empty() {
            self.stack.push(Value::Nil);
        } else {
            self.stack.push(Value::Output(stdout));
        }

        Ok(())
    }

    /// Execute redirect (supports multiple files via writing to each)
    fn execute_redirect(&mut self, mode: &str) -> Result<(), EvalError> {
        let file_block = self.pop_block()?;
        let cmd = self.pop_block()?;

        // Extract filenames from block
        let files: Vec<String> = file_block
            .iter()
            .filter_map(|e| match e {
                Expr::Literal(s) => Some(self.expand_tilde(s)),
                Expr::Quoted { content, .. } => Some(content.clone()),
                _ => None,
            })
            .collect();

        if files.is_empty() {
            return Err(EvalError::TypeError {
                expected: "filename".into(),
                got: "empty block".into(),
            });
        }

        // Handle stdin redirect differently
        if mode == "<" {
            return self.execute_stdin_redirect(&cmd, &files[0]);
        }

        // Execute command
        let (cmd_name, args) = self.block_to_cmd_args(&cmd)?;
        let (output, exit_code) = self.execute_native(&cmd_name, args)?;
        self.last_exit_code = exit_code;

        // Write to file(s)
        for file in &files {
            let mut f = match mode {
                ">" => File::create(file)?,
                ">>" => OpenOptions::new().append(true).create(true).open(file)?,
                _ => continue,
            };
            f.write_all(output.as_bytes())?;
        }

        Ok(())
    }

    /// Execute stdin redirect: [cmd] [file] <
    fn execute_stdin_redirect(&mut self, cmd: &[Expr], input_file: &str) -> Result<(), EvalError> {
        let (cmd_name, args) = self.block_to_cmd_args(cmd)?;

        // Open the input file
        let file = File::open(input_file)
            .map_err(|e| EvalError::ExecError(format!("{}: {}", input_file, e)))?;

        // Execute command with stdin from file
        let output = Command::new(&cmd_name)
            .args(&args)
            .current_dir(&self.cwd)
            .stdin(Stdio::from(file))
            .output()
            .map_err(|e| EvalError::ExecError(format!("{}: {}", cmd_name, e)))?;

        self.last_exit_code = output.status.code().unwrap_or(-1);

        // Push stdout to stack
        if !output.stdout.is_empty() {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            self.stack.push(Value::Output(stdout));
        }

        Ok(())
    }

    /// Execute stderr redirect
    fn execute_redirect_err(&mut self, mode: &str) -> Result<(), EvalError> {
        let file_block = self.pop_block()?;
        let cmd = self.pop_block()?;

        // Extract filenames from block
        let files: Vec<String> = file_block
            .iter()
            .filter_map(|e| match e {
                Expr::Literal(s) => Some(self.expand_tilde(s)),
                Expr::Quoted { content, .. } => Some(content.clone()),
                _ => None,
            })
            .collect();

        if files.is_empty() {
            return Err(EvalError::TypeError {
                expected: "filename".into(),
                got: "empty block".into(),
            });
        }

        // Execute command, capturing stderr separately
        let (cmd_name, args) = self.block_to_cmd_args(&cmd)?;

        let file = match mode {
            "2>" => File::create(&files[0])?,
            "2>>" => OpenOptions::new()
                .append(true)
                .create(true)
                .open(&files[0])?,
            _ => return Err(EvalError::ExecError("Invalid redirect mode".into())),
        };

        let output = Command::new(&cmd_name)
            .args(&args)
            .current_dir(&self.cwd)
            .stderr(Stdio::from(file))
            .output()
            .map_err(|e| EvalError::ExecError(format!("{}: {}", cmd_name, e)))?;

        self.last_exit_code = output.status.code().unwrap_or(-1);

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        if !stdout.is_empty() {
            self.stack.push(Value::Output(stdout));
        }

        Ok(())
    }

    /// Execute &> (redirect both stdout and stderr to file)
    fn execute_redirect_both(&mut self) -> Result<(), EvalError> {
        let file_block = self.pop_block()?;
        let cmd = self.pop_block()?;

        // Extract filenames from block
        let files: Vec<String> = file_block
            .iter()
            .filter_map(|e| match e {
                Expr::Literal(s) => Some(self.expand_tilde(s)),
                Expr::Quoted { content, .. } => Some(content.clone()),
                _ => None,
            })
            .collect();

        if files.is_empty() {
            return Err(EvalError::TypeError {
                expected: "filename".into(),
                got: "empty block".into(),
            });
        }

        // Execute command
        let (cmd_name, args) = self.block_to_cmd_args(&cmd)?;

        let file = File::create(&files[0])?;
        let file_clone = file.try_clone()?;

        let output = Command::new(&cmd_name)
            .args(&args)
            .current_dir(&self.cwd)
            .stdout(Stdio::from(file))
            .stderr(Stdio::from(file_clone))
            .output()
            .map_err(|e| EvalError::ExecError(format!("{}: {}", cmd_name, e)))?;

        self.last_exit_code = output.status.code().unwrap_or(-1);

        Ok(())
    }

    /// Execute stderr to stdout redirect: [cmd] 2>&1
    fn execute_redirect_err_to_out(&mut self) -> Result<(), EvalError> {
        let cmd = self.pop_block()?;
        let (cmd_name, args) = self.block_to_cmd_args(&cmd)?;

        // Execute command with stderr merged into stdout
        let output = Command::new(&cmd_name)
            .args(&args)
            .current_dir(&self.cwd)
            .stderr(Stdio::piped())
            .stdout(Stdio::piped())
            .output()
            .map_err(|e| EvalError::ExecError(format!("{}: {}", cmd_name, e)))?;

        self.last_exit_code = output.status.code().unwrap_or(-1);

        // Combine stdout and stderr
        let mut combined = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.is_empty() {
            combined.push_str(&stderr);
        }

        if !combined.is_empty() {
            self.stack.push(Value::Output(combined));
        }

        Ok(())
    }

    /// Execute background
    fn execute_background(&mut self) -> Result<(), EvalError> {
        let cmd = self.pop_block()?;
        let (cmd_name, args) = self.block_to_cmd_args(&cmd)?;
        let cmd_str = format!("{} {}", cmd_name, args.join(" "));

        let child = Command::new(&cmd_name)
            .args(&args)
            .current_dir(&self.cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| EvalError::ExecError(e.to_string()))?;

        let pid = child.id();
        let job_id = self.next_job_id;
        self.next_job_id += 1;

        self.jobs.push(Job {
            id: job_id,
            pid,
            pgid: pid,  // Process group ID same as PID for background jobs
            command: cmd_str.clone(),
            child: Some(child),
            status: JobStatus::Running,
        });

        // Print job info like bash does
        eprintln!("[{}] {}", job_id, pid);

        self.last_exit_code = 0;
        Ok(())
    }

    /// Execute && (and)
    fn execute_and(&mut self) -> Result<(), EvalError> {
        let right = self.pop_block()?;
        let left = self.pop_block()?;

        // Execute left
        for expr in &left {
            self.eval_expr(expr)?;
        }

        // Only execute right if left succeeded
        if self.last_exit_code == 0 {
            for expr in &right {
                self.eval_expr(expr)?;
            }
        }
        Ok(())
    }

    /// Execute || (or)
    fn execute_or(&mut self) -> Result<(), EvalError> {
        let right = self.pop_block()?;
        let left = self.pop_block()?;

        // Execute left
        for expr in &left {
            self.eval_expr(expr)?;
        }

        // Only execute right if left failed
        if self.last_exit_code != 0 {
            for expr in &right {
                self.eval_expr(expr)?;
            }
        }
        Ok(())
    }

    /// Convert a block to command + args
    fn block_to_cmd_args(&self, exprs: &[Expr]) -> Result<(String, Vec<String>), EvalError> {
        let mut parts: Vec<String> = Vec::new();

        for expr in exprs {
            match expr {
                Expr::Literal(s) => parts.push(s.clone()),
                Expr::Quoted { content, .. } => parts.push(content.clone()),
                Expr::Variable(s) => {
                    let var_name = s
                        .trim_start_matches('$')
                        .trim_start_matches('{')
                        .trim_end_matches('}');
                    if let Ok(val) = std::env::var(var_name) {
                        parts.push(val);
                    }
                }
                _ => {}
            }
        }

        if parts.is_empty() {
            return Err(EvalError::ExecError("Empty command".into()));
        }

        // Last non-flag word is command (postfix semantics)
        let cmd_idx = parts
            .iter()
            .rposition(|s| !s.starts_with('-'))
            .unwrap_or(parts.len() - 1);
        let cmd = parts.remove(cmd_idx);

        // Expand args
        let expanded_args: Vec<String> = parts
            .into_iter()
            .flat_map(|arg| self.expand_arg(&arg))
            .collect();

        Ok((cmd, expanded_args))
    }

    // ==================== STRUCTURED DATA BUILTINS ====================
    // These are handled specially before execute_command to preserve Value types

    /// Try to handle structured data builtins directly (without stringifying args)
    /// Returns true if handled, false if should fall through to execute_command
    fn try_structured_builtin(&mut self, cmd: &str) -> Result<bool, EvalError> {
        match cmd {
            // Phase 0
            "typeof" => { self.builtin_typeof()?; Ok(true) }
            // Phase 1: Record ops
            "record" => { self.builtin_record()?; Ok(true) }
            "get" => { self.builtin_get()?; Ok(true) }
            "set" => { self.builtin_set()?; Ok(true) }
            "del" => { self.builtin_del()?; Ok(true) }
            "has?" => { self.builtin_has()?; Ok(true) }
            "keys" => { self.builtin_keys()?; Ok(true) }
            "values" => { self.builtin_values()?; Ok(true) }
            "merge" => { self.builtin_merge()?; Ok(true) }
            // Phase 2: Table ops
            "table" => { self.builtin_table()?; Ok(true) }
            "where" => { self.builtin_where()?; Ok(true) }
            "sort-by" => { self.builtin_sort_by()?; Ok(true) }
            "select" => { self.builtin_select()?; Ok(true) }
            "first" => { self.builtin_first()?; Ok(true) }
            "last" => { self.builtin_last()?; Ok(true) }
            "nth" => { self.builtin_nth()?; Ok(true) }
            // Phase 3: Error handling
            "try" => { self.builtin_try()?; Ok(true) }
            "error?" => { self.builtin_error_predicate()?; Ok(true) }
            "throw" => { self.builtin_throw()?; Ok(true) }
            // Phase 4: Serialization
            "into-json" => { self.builtin_into_json()?; Ok(true) }
            "into-csv" => { self.builtin_into_csv()?; Ok(true) }
            "into-lines" => { self.builtin_into_lines()?; Ok(true) }
            "into-kv" => { self.builtin_into_kv()?; Ok(true) }
            "to-json" => { self.builtin_to_json()?; Ok(true) }
            "to-csv" => { self.builtin_to_csv()?; Ok(true) }
            "to-lines" => { self.builtin_to_lines()?; Ok(true) }
            "to-kv" => { self.builtin_to_kv()?; Ok(true) }
            "to-tsv" => { self.builtin_to_tsv()?; Ok(true) }
            "to-delimited" => { self.builtin_to_delimited()?; Ok(true) }
            // Phase 5: Stack utilities
            "tap" => { self.builtin_tap()?; Ok(true) }
            "dip" => { self.builtin_dip()?; Ok(true) }
            // Phase 6: Aggregations
            "sum" => { self.builtin_sum()?; Ok(true) }
            "avg" => { self.builtin_avg()?; Ok(true) }
            "min" => { self.builtin_min()?; Ok(true) }
            "max" => { self.builtin_max()?; Ok(true) }
            "count" => { self.builtin_count()?; Ok(true) }
            "reduce" => { self.builtin_reduce()?; Ok(true) }
            // Phase 8: Extended table ops
            "group-by" => { self.builtin_group_by()?; Ok(true) }
            "unique" => { self.builtin_unique()?; Ok(true) }
            "reverse" => { self.builtin_reverse()?; Ok(true) }
            "flatten" => { self.builtin_flatten()?; Ok(true) }
            "reject" => { self.builtin_reject()?; Ok(true) }
            "reject-where" => { self.builtin_reject_where()?; Ok(true) }
            "duplicates" => { self.builtin_duplicates()?; Ok(true) }
            // Phase 9: Vector operations
            "dot-product" => { self.builtin_dot_product()?; Ok(true) }
            "magnitude" => { self.builtin_magnitude()?; Ok(true) }
            "normalize" => { self.builtin_normalize()?; Ok(true) }
            "cosine-similarity" => { self.builtin_cosine_similarity()?; Ok(true) }
            "euclidean-distance" => { self.builtin_euclidean_distance()?; Ok(true) }
            // Phase 11: Additional parsers
            "into-tsv" => { self.builtin_into_tsv()?; Ok(true) }
            "into-delimited" => { self.builtin_into_delimited()?; Ok(true) }
            // Structured builtins
            "ls-table" => { self.builtin_ls_table()?; Ok(true) }
            "open" => { self.builtin_open()?; Ok(true) }
            "save" => { self.builtin_save()?; Ok(true) }
            _ => Ok(false),
        }
    }

    // ==================== BUILTINS ====================

    fn builtin_cd(&mut self, args: &[String]) -> Result<(), EvalError> {
        let dir = if args.is_empty() {
            PathBuf::from(&self.home_dir)
        } else {
            let expanded = self.expand_tilde(&args[0]);
            PathBuf::from(expanded)
        };

        // Resolve relative paths
        let new_cwd = if dir.is_absolute() {
            dir.clone()
        } else {
            self.cwd.join(&dir)
        };

        // Canonicalize and verify it exists
        let canonical = new_cwd.canonicalize().map_err(|e| {
            EvalError::ExecError(format!("cd: {}: {}", new_cwd.display(), e))
        })?;

        if !canonical.is_dir() {
            return Err(EvalError::ExecError(format!(
                "cd: {}: Not a directory",
                dir.display()
            )));
        }

        // Also update the actual process directory so child processes inherit it
        std::env::set_current_dir(&canonical).map_err(|e| {
            EvalError::ExecError(format!("cd: {}: {}", canonical.display(), e))
        })?;

        self.cwd = canonical;
        self.last_exit_code = 0;
        Ok(())
    }

    fn builtin_pwd(&mut self) -> Result<(), EvalError> {
        self.stack
            .push(Value::Output(self.cwd.to_string_lossy().to_string() + "\n"));
        self.last_exit_code = 0;
        Ok(())
    }

    fn builtin_echo(&mut self, args: &[String]) -> Result<(), EvalError> {
        let output = args.join(" ");
        self.stack.push(Value::Output(format!("{}\n", output)));
        self.last_exit_code = 0;
        Ok(())
    }

    fn builtin_true(&mut self) -> Result<(), EvalError> {
        self.last_exit_code = 0;
        Ok(())
    }

    fn builtin_false(&mut self) -> Result<(), EvalError> {
        self.last_exit_code = 1;
        Ok(())
    }

    fn builtin_test(&mut self, args: &[String]) -> Result<(), EvalError> {
        // Args come in LIFO order from stack, reverse for natural postfix order
        // In hsab postfix: "Cargo.toml -f test" -> stack: [Cargo.toml, -f]
        //   -> LIFO: [-f, Cargo.toml] -> reversed: [Cargo.toml, -f]
        // In hsab postfix: "a a = test" -> stack: [a, a, =]
        //   -> LIFO: [=, a, a] -> reversed: [a, a, =]
        let args: Vec<String> = args.iter().rev().cloned().collect();
        let result = match args.as_slice() {
            // File tests (postfix: "path flag" -> after reversal: [path, flag])
            [path, flag] if flag == "-f" => Path::new(path).is_file(),
            [path, flag] if flag == "-d" => Path::new(path).is_dir(),
            [path, flag] if flag == "-e" => Path::new(path).exists(),
            [path, flag] if flag == "-r" => Path::new(path).exists(), // Simplified
            [path, flag] if flag == "-w" => Path::new(path).exists(), // Simplified
            [path, flag] if flag == "-x" => self.is_executable(path),
            [path, flag] if flag == "-s" => {
                Path::new(path)
                    .metadata()
                    .map(|m| m.len() > 0)
                    .unwrap_or(false)
            }

            // String tests (postfix: "str flag" -> after reversal: [str, flag])
            [s, flag] if flag == "-z" => s.is_empty(),
            [s, flag] if flag == "-n" => !s.is_empty(),
            // Postfix binary ops: "a b op" -> after reversal: [a, b, op]
            [s1, s2, op] if op == "=" || op == "==" => s1 == s2,
            [s1, s2, op] if op == "!=" => s1 != s2,

            // Numeric comparisons (postfix: "5 3 -gt" -> after reversal: [5, 3, -gt])
            [n1, n2, op] if op == "-eq" => self.cmp_nums(n1, n2, |a, b| a == b),
            [n1, n2, op] if op == "-ne" => self.cmp_nums(n1, n2, |a, b| a != b),
            [n1, n2, op] if op == "-lt" => self.cmp_nums(n1, n2, |a, b| a < b),
            [n1, n2, op] if op == "-le" => self.cmp_nums(n1, n2, |a, b| a <= b),
            [n1, n2, op] if op == "-gt" => self.cmp_nums(n1, n2, |a, b| a > b),
            [n1, n2, op] if op == "-ge" => self.cmp_nums(n1, n2, |a, b| a >= b),

            // Single arg = non-empty string test
            [s] => !s.is_empty(),

            [] => false,
            _ => false,
        };

        self.last_exit_code = if result { 0 } else { 1 };
        Ok(())
    }

    fn cmp_nums<F>(&self, a: &str, b: &str, cmp: F) -> bool
    where
        F: Fn(i64, i64) -> bool,
    {
        match (a.parse::<i64>(), b.parse::<i64>()) {
            (Ok(a), Ok(b)) => cmp(a, b),
            _ => false,
        }
    }

    fn is_executable(&self, path: &str) -> bool {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            Path::new(path)
                .metadata()
                .map(|m| m.permissions().mode() & 0o111 != 0)
                .unwrap_or(false)
        }
        #[cfg(not(unix))]
        {
            Path::new(path).exists()
        }
    }

    /// Export environment variable
    /// Stack-native: value NAME export
    /// Legacy: NAME=VALUE export
    fn builtin_export(&mut self, args: &[String]) -> Result<(), EvalError> {
        // Args come in LIFO order from stack
        // For "value NAME export": args = ["NAME", "value"]
        // For "NAME=VALUE export": args = ["NAME=VALUE"]

        for arg in args.iter() {
            if let Some((key, value)) = arg.split_once('=') {
                // Legacy KEY=VALUE syntax
                std::env::set_var(key, value);
            } else if args.len() >= 2 {
                // Stack-native: value NAME export
                // args[0] is NAME (last pushed), args[1] is value
                let name = &args[0];
                let value = &args[1];
                std::env::set_var(name, value);
                break; // Only process once for stack-native form
            }
        }
        self.last_exit_code = 0;
        Ok(())
    }

    fn builtin_unset(&mut self, args: &[String]) -> Result<(), EvalError> {
        for var in args {
            std::env::remove_var(var);
        }
        self.last_exit_code = 0;
        Ok(())
    }

    fn builtin_env(&mut self) -> Result<(), EvalError> {
        let mut output = String::new();
        for (key, value) in std::env::vars() {
            output.push_str(&format!("{}={}\n", key, value));
        }
        self.stack.push(Value::Output(output));
        self.last_exit_code = 0;
        Ok(())
    }

    fn builtin_jobs(&mut self) -> Result<(), EvalError> {
        // Update job statuses
        self.update_job_statuses();

        let mut output = String::new();
        for job in &self.jobs {
            let status_str = match &job.status {
                JobStatus::Running => "Running",
                JobStatus::Stopped => "Stopped",
                JobStatus::Done(code) => {
                    if *code == 0 {
                        "Done"
                    } else {
                        "Exit"
                    }
                }
            };
            output.push_str(&format!(
                "[{}]\t{}\t{}\t{}\n",
                job.id, job.pid, status_str, job.command
            ));
        }

        if !output.is_empty() {
            self.stack.push(Value::Output(output));
        }
        self.last_exit_code = 0;
        Ok(())
    }

    fn update_job_statuses(&mut self) {
        for job in &mut self.jobs {
            if job.status == JobStatus::Running {
                if let Some(ref mut child) = job.child {
                    match child.try_wait() {
                        Ok(Some(status)) => {
                            job.status = JobStatus::Done(status.code().unwrap_or(-1));
                        }
                        Ok(None) => {} // Still running
                        Err(_) => {
                            job.status = JobStatus::Done(-1);
                        }
                    }
                }
            }
        }
    }

    fn builtin_fg(&mut self, args: &[String]) -> Result<(), EvalError> {
        let job_id: Option<usize> = args
            .first()
            .and_then(|s| s.trim_start_matches('%').parse().ok());

        let job = if let Some(id) = job_id {
            self.jobs.iter_mut().find(|j| j.id == id)
        } else {
            self.jobs
                .iter_mut()
                .filter(|j| j.status == JobStatus::Running)
                .last()
        };

        match job {
            Some(job) => {
                eprintln!("{}", job.command);
                if let Some(ref mut child) = job.child {
                    let status = child
                        .wait()
                        .map_err(|e| EvalError::ExecError(e.to_string()))?;
                    self.last_exit_code = status.code().unwrap_or(-1);
                    job.status = JobStatus::Done(self.last_exit_code);
                }
                Ok(())
            }
            None => Err(EvalError::ExecError("fg: no current job".into())),
        }
    }

    fn builtin_bg(&mut self, args: &[String]) -> Result<(), EvalError> {
        let job_id: Option<usize> = args
            .first()
            .and_then(|s| s.trim_start_matches('%').parse().ok());

        // Find a stopped job to resume
        let job_info = if let Some(id) = job_id {
            self.jobs
                .iter()
                .find(|j| j.id == id && j.status == JobStatus::Stopped)
                .map(|j| (j.id, j.pgid, j.command.clone()))
        } else {
            // Find the most recent stopped job
            self.jobs
                .iter()
                .rev()
                .find(|j| j.status == JobStatus::Stopped)
                .map(|j| (j.id, j.pgid, j.command.clone()))
        };

        match job_info {
            Some((id, pgid, cmd)) => {
                // Send SIGCONT to resume the process
                crate::signals::continue_process(pgid)
                    .map_err(|e| EvalError::ExecError(format!("bg: {}", e)))?;

                // Update job status
                if let Some(job) = self.jobs.iter_mut().find(|j| j.id == id) {
                    job.status = JobStatus::Running;
                }

                eprintln!("[{}]+ {} &", id, cmd);
                self.last_exit_code = 0;
                Ok(())
            }
            None => Err(EvalError::ExecError("bg: no stopped job".into())),
        }
    }

    fn builtin_exit(&mut self, args: &[String]) -> Result<(), EvalError> {
        let code = args.first().and_then(|s| s.parse::<i32>().ok()).unwrap_or(0);
        std::process::exit(code);
    }

    /// Run command with inherited stdio (for interactive commands like vim, less, top)
    /// Usage: file.txt vim tty
    fn builtin_tty(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.is_empty() {
            return Err(EvalError::ExecError("tty: no command specified".into()));
        }

        // Last arg is the command (postfix order), rest are arguments
        let cmd = &args[args.len() - 1];
        let cmd_args = &args[..args.len() - 1];

        let status = Command::new(cmd)
            .args(cmd_args)
            .current_dir(&self.cwd)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .map_err(|e| EvalError::ExecError(format!("{}: {}", cmd, e)))?;

        self.last_exit_code = status.code().unwrap_or(-1);
        Ok(())
    }

    fn builtin_which(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.is_empty() {
            return Err(EvalError::ExecError("which: no command specified".into()));
        }

        let mut output_lines = Vec::new();
        let mut found_any = false;

        for cmd in args {
            // Check if it's an hsab builtin
            if ExecutableResolver::is_hsab_builtin(cmd) {
                output_lines.push(format!("{}: hsab builtin", cmd));
                found_any = true;
                continue;
            }

            // Check if it's a user-defined word
            if self.definitions.contains_key(cmd) {
                output_lines.push(format!("{}: hsab definition", cmd));
                found_any = true;
                continue;
            }

            // Check if it's a shell builtin we handle
            if matches!(
                cmd.as_str(),
                "cd" | "pwd" | "echo" | "printf" | "read"
                    | "true" | "false" | "test" | "["
                    | "export" | "unset" | "env" | "local" | "return"
                    | "jobs" | "fg" | "bg" | "wait" | "kill"
                    | "exit" | "tty"
                    | "which" | "type" | "source" | "." | "hash"
                    | "pushd" | "popd" | "dirs"
                    | "alias" | "unalias" | "trap"
            ) {
                output_lines.push(format!("{}: shell builtin", cmd));
                found_any = true;
                continue;
            }

            // Check PATH for executable
            if let Some(path) = self.resolver.find_executable(cmd) {
                output_lines.push(path);
                found_any = true;
            } else {
                output_lines.push(format!("{} not found", cmd));
            }
        }

        if !output_lines.is_empty() {
            self.stack
                .push(Value::Output(output_lines.join("\n") + "\n"));
        }

        self.last_exit_code = if found_any { 0 } else { 1 };
        Ok(())
    }

    /// Type builtin - show how a command would be interpreted (bash-style output)
    /// Usage: ls type  ->  "ls is /bin/ls"
    fn builtin_type(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.is_empty() {
            return Err(EvalError::ExecError("type: no command specified".into()));
        }

        let mut output_lines = Vec::new();
        let mut found_any = false;

        for cmd in args {
            // Check if it's an hsab builtin (stack ops, etc.)
            if ExecutableResolver::is_hsab_builtin(cmd) {
                output_lines.push(format!("{} is a hsab builtin", cmd));
                found_any = true;
                continue;
            }

            // Check if it's a user-defined word
            if self.definitions.contains_key(cmd) {
                output_lines.push(format!("{} is a hsab function", cmd));
                found_any = true;
                continue;
            }

            // Check if it's a shell builtin we handle
            if matches!(
                cmd.as_str(),
                "cd" | "pwd" | "echo" | "printf" | "read"
                    | "true" | "false" | "test" | "["
                    | "export" | "unset" | "env" | "local" | "return"
                    | "jobs" | "fg" | "bg" | "wait" | "kill"
                    | "exit" | "tty"
                    | "which" | "type" | "source" | "." | "hash"
                    | "pushd" | "popd" | "dirs"
                    | "alias" | "unalias" | "trap"
            ) {
                output_lines.push(format!("{} is a shell builtin", cmd));
                found_any = true;
                continue;
            }

            // Check PATH for executable
            if let Some(path) = self.resolver.find_executable(cmd) {
                output_lines.push(format!("{} is {}", cmd, path));
                found_any = true;
            } else {
                output_lines.push(format!("type: {}: not found", cmd));
            }
        }

        if !output_lines.is_empty() {
            self.stack
                .push(Value::Output(output_lines.join("\n") + "\n"));
        }

        self.last_exit_code = if found_any { 0 } else { 1 };
        Ok(())
    }

    /// Source a file - execute it in the current evaluator context
    /// Usage: file.hsab source  or  file.hsab .
    fn builtin_source(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.is_empty() {
            return Err(EvalError::ExecError("source: no file specified".into()));
        }

        // Last arg is the file path (postfix order)
        let path_str = self.expand_tilde(&args[args.len() - 1]);
        let path = PathBuf::from(&path_str);

        // Read the file content
        let content = std::fs::read_to_string(&path)
            .map_err(|e| EvalError::ExecError(format!("source: {}: {}", path_str, e)))?;

        // Parse and execute in current evaluator context
        let tokens = crate::lex(&content)
            .map_err(|e| EvalError::ExecError(format!("source: parse error: {}", e)))?;

        if tokens.is_empty() {
            self.last_exit_code = 0;
            return Ok(());
        }

        let program = crate::parse(tokens)
            .map_err(|e| EvalError::ExecError(format!("source: parse error: {}", e)))?;

        // Execute each expression in the current context
        for expr in &program.expressions {
            self.eval_expr(expr)?;
        }

        self.last_exit_code = 0;
        Ok(())
    }

    // ==================== MODULE SYSTEM ====================

    /// Import a module: "path.hsab" .import or "path.hsab" alias .import
    fn module_import(&mut self) -> Result<(), EvalError> {
        // Pop the top value - could be path or alias
        let top = self.pop_string()?;

        // Check if top is a path (contains / or .) or an alias (simple identifier)
        let (path_str, alias) = if top.contains('/') || top.contains('.') {
            // Top is a path, no alias
            (top, None)
        } else {
            // Top is an alias, path should be next on stack
            let path = self.pop_string()?;
            (path, Some(top))
        };

        // Resolve module path using search paths
        let resolved_path = self.resolve_module_path(&path_str)?;

        // Get canonical path for tracking
        let canonical = resolved_path.canonicalize().unwrap_or_else(|_| resolved_path.clone());

        // Skip if already loaded
        if self.loaded_modules.contains(&canonical) {
            self.last_exit_code = 0;
            return Ok(());
        }

        // Mark as loaded before executing (handles circular imports)
        self.loaded_modules.insert(canonical);

        // Determine namespace from filename or alias
        let namespace = match alias {
            Some(a) => a,
            None => {
                // Extract filename without extension
                resolved_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
                    .ok_or_else(|| EvalError::ExecError("import: invalid module path".into()))?
            }
        };

        // Read and parse the module
        let content = std::fs::read_to_string(&resolved_path)
            .map_err(|e| EvalError::ExecError(format!("import: {}: {}", path_str, e)))?;

        let tokens = crate::lex(&content)
            .map_err(|e| EvalError::ExecError(format!("import: parse error: {}", e)))?;

        if tokens.is_empty() {
            self.last_exit_code = 0;
            return Ok(());
        }

        let program = crate::parse(tokens)
            .map_err(|e| EvalError::ExecError(format!("import: parse error: {}", e)))?;

        // Save current definitions (with their values) to detect new/changed ones
        let before_defs: HashMap<String, Vec<Expr>> = self.definitions.clone();

        // Execute module in current context
        for expr in &program.expressions {
            self.eval_expr(expr)?;
        }

        // Find definitions that were added or changed during module execution
        let module_defs: Vec<String> = self.definitions
            .iter()
            .filter(|(name, body)| {
                // Include if: new name OR same name but different body
                match before_defs.get(*name) {
                    None => true,  // New definition
                    Some(old_body) => old_body != *body,  // Changed definition
                }
            })
            .map(|(name, _)| name.clone())
            .collect();

        for name in module_defs {
            // Skip private definitions (underscore prefix)
            if name.starts_with('_') {
                self.definitions.remove(&name);
                continue;
            }

            // Move definition to namespaced name
            if let Some(block) = self.definitions.remove(&name) {
                let namespaced = format!("{}::{}", namespace, name);
                self.definitions.insert(namespaced.clone(), block);

                // Restore the original definition if it existed
                if let Some(original) = before_defs.get(&name) {
                    self.definitions.insert(name, original.clone());
                }
            }
        }

        self.last_exit_code = 0;
        Ok(())
    }

    /// Resolve module path using search paths
    /// Search order: . -> ./lib/ -> ~/.hsab/lib/ -> $HSAB_PATH
    fn resolve_module_path(&self, path_str: &str) -> Result<PathBuf, EvalError> {
        let path = PathBuf::from(path_str);

        // If absolute path, use directly
        if path.is_absolute() {
            if path.exists() {
                return Ok(path);
            }
            return Err(EvalError::ExecError(format!("import: module not found: {}", path_str)));
        }

        // Build search paths
        let mut search_paths = vec![
            self.cwd.clone(),                           // Current directory
            self.cwd.join("lib"),                       // ./lib/
        ];

        // Add ~/.hsab/lib/
        if let Ok(home) = std::env::var("HOME") {
            search_paths.push(PathBuf::from(home).join(".hsab").join("lib"));
        }

        // Add HSAB_PATH directories
        if let Ok(hsab_path) = std::env::var("HSAB_PATH") {
            for dir in hsab_path.split(':') {
                if !dir.is_empty() {
                    search_paths.push(PathBuf::from(dir));
                }
            }
        }

        // Search for the module
        for search_dir in search_paths {
            let full_path = search_dir.join(&path);
            if full_path.exists() {
                return Ok(full_path);
            }
        }

        Err(EvalError::ExecError(format!("import: module not found: {}", path_str)))
    }

    /// Hash builtin - manage command hash table
    /// Usage: hash         - show cached commands
    ///        ls hash      - hash 'ls' command
    ///        -r hash      - clear the hash table
    fn builtin_hash(&mut self, args: &[String]) -> Result<(), EvalError> {
        // Check for -r flag to clear cache
        if args.iter().any(|a| a == "-r") {
            self.resolver.clear_cache();
            self.last_exit_code = 0;
            return Ok(());
        }

        // If args provided, hash those specific commands
        if !args.is_empty() {
            for cmd in args {
                // Force a PATH lookup and cache it
                self.resolver.resolve_and_cache(cmd);
            }
            self.last_exit_code = 0;
            return Ok(());
        }

        // No args - show the hash table
        let entries = self.resolver.get_cache_entries();
        if entries.is_empty() {
            // Empty hash table, no output
            self.last_exit_code = 0;
            return Ok(());
        }

        let mut output = String::new();
        for (cmd, path) in entries {
            output.push_str(&format!("{}\t{}\n", cmd, path));
        }
        self.stack.push(Value::Output(output));
        self.last_exit_code = 0;
        Ok(())
    }

    fn builtin_timeout(&mut self) -> Result<(), EvalError> {
        let block = self.pop_block()?;
        let seconds_str = self.pop_string()?;

        let seconds: u64 = seconds_str.parse().map_err(|_| EvalError::TypeError {
            expected: "integer seconds".into(),
            got: seconds_str,
        })?;

        let (cmd, args) = self.block_to_cmd_args(&block)?;

        let mut child = Command::new(&cmd)
            .args(&args)
            .current_dir(&self.cwd)
            .spawn()
            .map_err(|e| EvalError::ExecError(e.to_string()))?;

        let timeout = Duration::from_secs(seconds);
        let start = Instant::now();

        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    self.last_exit_code = status.code().unwrap_or(-1);
                    return Ok(());
                }
                Ok(None) => {
                    if start.elapsed() > timeout {
                        let _ = child.kill();
                        self.last_exit_code = 124; // Standard timeout exit code
                        return Ok(());
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(e) => return Err(EvalError::ExecError(e.to_string())),
            }
        }
    }

    fn builtin_pipestatus(&mut self) -> Result<(), EvalError> {
        let list: Vec<Value> = self
            .pipestatus
            .iter()
            .map(|&c| Value::Number(c as f64))
            .collect();
        self.stack.push(Value::List(list));
        self.last_exit_code = 0;
        Ok(())
    }

    // ==================== JSON ====================

    fn json_parse(&mut self) -> Result<(), EvalError> {
        let s = self.pop_string()?;
        let json: JsonValue = serde_json::from_str(&s)
            .map_err(|e| EvalError::ExecError(format!("JSON parse error: {}", e)))?;
        let value = crate::ast::json_to_value(json);
        self.stack.push(value);
        Ok(())
    }

    fn json_stringify(&mut self) -> Result<(), EvalError> {
        let value = self.pop_value_or_err()?;
        let json = crate::ast::value_to_json(&value);
        let output = serde_json::to_string_pretty(&json)
            .map_err(|e| EvalError::ExecError(format!("JSON error: {}", e)))?;
        self.stack.push(Value::Output(output));
        Ok(())
    }

    // ==================== STACK OPERATIONS ====================

    fn stack_dup(&mut self) -> Result<(), EvalError> {
        let top = self
            .stack
            .last()
            .cloned()
            .ok_or_else(|| EvalError::StackUnderflow("dup".into()))?;
        self.stack.push(top);
        Ok(())
    }

    fn stack_swap(&mut self) -> Result<(), EvalError> {
        let len = self.stack.len();
        if len < 2 {
            return Err(EvalError::StackUnderflow("swap".into()));
        }
        self.stack.swap(len - 1, len - 2);
        Ok(())
    }

    fn stack_drop(&mut self) -> Result<(), EvalError> {
        self.stack
            .pop()
            .ok_or_else(|| EvalError::StackUnderflow("drop".into()))?;
        Ok(())
    }

    fn stack_over(&mut self) -> Result<(), EvalError> {
        let len = self.stack.len();
        if len < 2 {
            return Err(EvalError::StackUnderflow("over".into()));
        }
        let second = self.stack[len - 2].clone();
        self.stack.push(second);
        Ok(())
    }

    fn stack_rot(&mut self) -> Result<(), EvalError> {
        let len = self.stack.len();
        if len < 3 {
            return Err(EvalError::StackUnderflow("rot".into()));
        }
        let third = self.stack.remove(len - 3);
        self.stack.push(third);
        Ok(())
    }

    fn stack_depth(&mut self) -> Result<(), EvalError> {
        let depth = self.stack.len();
        self.stack.push(Value::Literal(depth.to_string()));
        Ok(())
    }

    // ==================== PATH OPERATIONS ====================

    fn path_join(&mut self) -> Result<(), EvalError> {
        let file = self.pop_string()?;
        let dir = self.pop_string()?;
        let joined = if dir.ends_with('/') {
            format!("{}{}", dir, file)
        } else {
            format!("{}/{}", dir, file)
        };
        self.stack.push(Value::Literal(joined));
        Ok(())
    }

    fn path_suffix(&mut self) -> Result<(), EvalError> {
        let suffix = self.pop_string()?;
        let base = self.pop_string()?;
        self.stack.push(Value::Literal(format!("{}{}", base, suffix)));
        Ok(())
    }

    /// Get directory name: /path/to/file.txt → /path/to
    fn path_dirname(&mut self) -> Result<(), EvalError> {
        let path = self.pop_string()?;
        let result = match path.rfind('/') {
            Some(0) => "/".to_string(),        // Root: /file → /
            Some(idx) => path[..idx].to_string(),
            None => ".".to_string(),            // No slash: file → .
        };
        self.stack.push(Value::Literal(result));
        Ok(())
    }

    /// Get base name without extension: /path/to/file.txt → file
    fn path_basename(&mut self) -> Result<(), EvalError> {
        let path = self.pop_string()?;
        // First get the filename (after last /)
        let filename = match path.rfind('/') {
            Some(idx) => &path[idx + 1..],
            None => &path,
        };
        // Then remove extension (after first .)
        let basename = match filename.find('.') {
            Some(idx) if idx > 0 => &filename[..idx],
            _ => filename,
        };
        self.stack.push(Value::Literal(basename.to_string()));
        Ok(())
    }

    // ==================== STRING OPERATIONS ====================

    /// Split at first occurrence of delimiter
    /// "a.b.c" "." split1 → "a", "b.c"
    /// If not found: "abc" "." split1 → "abc", ""
    fn string_split1(&mut self) -> Result<(), EvalError> {
        let delim = self.pop_string()?;
        let s = self.pop_string()?;

        match s.find(&delim) {
            Some(idx) => {
                let (left, right) = s.split_at(idx);
                self.stack.push(Value::Literal(left.to_string()));
                self.stack
                    .push(Value::Literal(right[delim.len()..].to_string()));
            }
            None => {
                self.stack.push(Value::Literal(s));
                self.stack.push(Value::Literal(String::new()));
            }
        }
        Ok(())
    }

    /// Split at last occurrence of delimiter
    /// "a.b.c" "." rsplit1 → "a.b", "c"
    /// If not found: "abc" "." rsplit1 → "", "abc"
    fn string_rsplit1(&mut self) -> Result<(), EvalError> {
        let delim = self.pop_string()?;
        let s = self.pop_string()?;

        match s.rfind(&delim) {
            Some(idx) => {
                let (left, right) = s.split_at(idx);
                self.stack.push(Value::Literal(left.to_string()));
                self.stack
                    .push(Value::Literal(right[delim.len()..].to_string()));
            }
            None => {
                self.stack.push(Value::Literal(String::new()));
                self.stack.push(Value::Literal(s));
            }
        }
        Ok(())
    }

    // ==================== LIST OPERATIONS ====================

    /// Spread: split a multi-line value into separate stack items
    fn list_spread(&mut self) -> Result<(), EvalError> {
        let value = self.pop_value_or_err()?;
        let text = value.as_arg().unwrap_or_default();

        // Push marker to indicate start of spread items
        self.stack.push(Value::Marker);

        // Split by newlines and push each line
        for line in text.lines() {
            if !line.is_empty() {
                self.stack.push(Value::Literal(line.to_string()));
            }
        }

        Ok(())
    }

    /// Each: apply a block to each item on the stack until hitting a marker
    fn list_each(&mut self) -> Result<(), EvalError> {
        let block = self.pop_block()?;

        // Collect items until we hit a marker
        let mut items = Vec::new();
        while let Some(value) = self.stack.last() {
            if value.is_marker() {
                self.stack.pop(); // Remove the marker
                break;
            }
            items.push(self.stack.pop().unwrap());
        }

        // Items are in reverse order (LIFO), so reverse them
        items.reverse();

        // Apply block to each item
        'outer: for item in items {
            self.stack.push(item);
            for expr in &block {
                match self.eval_expr(expr) {
                    Ok(()) => {}
                    Err(EvalError::BreakLoop) => break 'outer,
                    Err(e) => return Err(e),
                }
            }
        }

        Ok(())
    }

    /// Collect: gather stack items until marker into a single value
    fn list_collect(&mut self) -> Result<(), EvalError> {
        let mut items = Vec::new();

        while let Some(value) = self.stack.last() {
            if value.is_marker() {
                self.stack.pop(); // Remove the marker
                break;
            }
            if let Some(s) = value.as_arg() {
                items.push(s);
            }
            self.stack.pop();
        }

        // Items are in reverse order (LIFO), so reverse them
        items.reverse();

        // Join with newlines and push as output
        let collected = items.join("\n");
        if collected.is_empty() {
            self.stack.push(Value::Nil);
        } else {
            self.stack.push(Value::Output(collected));
        }

        Ok(())
    }

    /// Keep: filter items, keeping only those where predicate returns exit code 0
    fn list_keep(&mut self) -> Result<(), EvalError> {
        let predicate = self.pop_block()?;

        // Collect items until we hit a marker
        let mut items = Vec::new();
        while let Some(value) = self.stack.last() {
            if value.is_marker() {
                self.stack.pop(); // Remove the marker
                break;
            }
            items.push(self.stack.pop().unwrap());
        }

        // Items are in reverse order (LIFO), so reverse them
        items.reverse();

        // Collect kept items separately, then push all at once with marker
        let mut kept = Vec::new();

        // Test each item with predicate, keep if passes
        for item in items {
            // Push a temporary marker to isolate this test
            self.stack.push(Value::Marker);

            // Push item for predicate to consume
            self.stack.push(item.clone());

            // Execute predicate
            for expr in &predicate {
                self.eval_expr(expr)?;
            }

            // Clean up: remove everything down to (and including) the temp marker
            while let Some(v) = self.stack.pop() {
                if v.is_marker() {
                    break;
                }
            }

            // Check if predicate passed (exit code 0)
            if self.last_exit_code == 0 {
                kept.push(item);
            }
        }

        // Push final marker and all kept items
        self.stack.push(Value::Marker);
        for item in kept {
            self.stack.push(item);
        }

        Ok(())
    }

    /// Map: [block] map - apply block to each item and collect results
    /// Equivalent to: each collect
    fn list_map(&mut self) -> Result<(), EvalError> {
        // Apply each, then collect
        self.list_each()?;
        self.list_collect()?;
        Ok(())
    }

    /// Filter: [predicate] filter - keep items where predicate passes and collect
    /// Equivalent to: keep collect
    fn list_filter(&mut self) -> Result<(), EvalError> {
        // Apply keep, then collect
        self.list_keep()?;
        self.list_collect()?;
        Ok(())
    }

    // ==================== CONTROL FLOW ====================

    /// If: [condition] [then] [else] if
    fn control_if(&mut self) -> Result<(), EvalError> {
        let else_block = self.pop_block()?;
        let then_block = self.pop_block()?;
        let cond_block = self.pop_block()?;

        // Save outer capture mode
        let outer_capture_mode = self.capture_mode;

        // Execute condition block with full stack access
        // Condition can read/modify stack, we just check exit code
        self.capture_mode = true;
        for expr in &cond_block {
            self.eval_expr(expr)?;
        }

        // Check result - use exit code
        let condition_met = self.last_exit_code == 0;

        // Execute appropriate branch - capture all but restore for last
        let branch = if condition_met { then_block } else { else_block };
        for (i, expr) in branch.iter().enumerate() {
            let is_last = i == branch.len() - 1;
            self.capture_mode = if is_last { outer_capture_mode } else { true };
            self.eval_expr(expr)?;
        }

        Ok(())
    }

    /// Times: N [block] times - repeat block N times
    fn control_times(&mut self) -> Result<(), EvalError> {
        let block = self.pop_block()?;
        let n_str = self.pop_string()?;

        let n: usize = n_str.parse().map_err(|_| EvalError::TypeError {
            expected: "integer".into(),
            got: n_str,
        })?;

        'outer: for _ in 0..n {
            for expr in &block {
                match self.eval_expr(expr) {
                    Ok(()) => {}
                    Err(EvalError::BreakLoop) => break 'outer,
                    Err(e) => return Err(e),
                }
            }
        }

        Ok(())
    }

    /// While: [condition] [body] while - repeat while condition passes (exit code 0)
    fn control_while(&mut self) -> Result<(), EvalError> {
        let body = self.pop_block()?;
        let cond = self.pop_block()?;

        'outer: loop {
            // Isolate condition evaluation with marker
            self.stack.push(Value::Marker);

            // Evaluate condition
            for expr in &cond {
                self.eval_expr(expr)?;
            }

            // Clean up anything pushed during condition (until marker)
            while let Some(v) = self.stack.pop() {
                if v.is_marker() {
                    break;
                }
            }

            // Stop if condition fails
            if self.last_exit_code != 0 {
                break;
            }

            // Execute body (output stays on stack)
            for expr in &body {
                match self.eval_expr(expr) {
                    Ok(()) => {}
                    Err(EvalError::BreakLoop) => break 'outer,
                    Err(e) => return Err(e),
                }
            }
        }

        Ok(())
    }

    /// Until: [condition] [body] until - repeat until condition passes (exit code 0)
    fn control_until(&mut self) -> Result<(), EvalError> {
        let body = self.pop_block()?;
        let cond = self.pop_block()?;

        'outer: loop {
            // Isolate condition evaluation with marker
            self.stack.push(Value::Marker);

            // Evaluate condition
            for expr in &cond {
                self.eval_expr(expr)?;
            }

            // Clean up anything pushed during condition (until marker)
            while let Some(v) = self.stack.pop() {
                if v.is_marker() {
                    break;
                }
            }

            // Stop if condition succeeds
            if self.last_exit_code == 0 {
                break;
            }

            // Execute body (output stays on stack)
            for expr in &body {
                match self.eval_expr(expr) {
                    Ok(()) => {}
                    Err(EvalError::BreakLoop) => break 'outer,
                    Err(e) => return Err(e),
                }
            }
        }

        Ok(())
    }

    // ==================== PARALLEL EXECUTION ====================

    /// Parallel: [[cmd1] [cmd2] ...] parallel - run blocks in parallel, wait for all
    fn exec_parallel(&mut self) -> Result<(), EvalError> {
        let blocks = self.pop_block()?;

        // Extract commands from inner blocks
        let mut cmds: Vec<(String, Vec<String>)> = Vec::new();
        for expr in blocks {
            if let Expr::Block(inner) = expr {
                if let Ok((cmd, args)) = self.block_to_cmd_args(&inner) {
                    cmds.push((cmd, args));
                }
            }
        }

        if cmds.is_empty() {
            return Ok(());
        }

        // Spawn all commands
        let cwd = self.cwd.clone();
        let handles: Vec<_> = cmds
            .into_iter()
            .map(|(cmd, args)| {
                let cwd = cwd.clone();
                std::thread::spawn(move || {
                    Command::new(&cmd)
                        .args(&args)
                        .current_dir(&cwd)
                        .output()
                        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
                        .unwrap_or_default()
                })
            })
            .collect();

        // Wait for all and collect output
        let mut combined_output = String::new();
        for handle in handles {
            if let Ok(output) = handle.join() {
                combined_output.push_str(&output);
            }
        }

        if !combined_output.is_empty() {
            self.stack.push(Value::Output(combined_output));
        }

        self.last_exit_code = 0;
        Ok(())
    }

    /// Fork: [cmd1] [cmd2] ... N fork - background N blocks from stack
    fn exec_fork(&mut self) -> Result<(), EvalError> {
        // Pop count
        let n_str = self.pop_string()?;
        let n: usize = n_str.parse().map_err(|_| EvalError::TypeError {
            expected: "integer".into(),
            got: n_str,
        })?;

        // Pop N blocks and background each
        for _ in 0..n {
            let block = self.pop_block()?;
            let (cmd, args) = self.block_to_cmd_args(&block)?;
            let cmd_str = format!("{} {}", cmd, args.join(" "));

            let child = Command::new(&cmd)
                .args(&args)
                .current_dir(&self.cwd)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|e| EvalError::ExecError(e.to_string()))?;

            let pid = child.id();
            let job_id = self.next_job_id;
            self.next_job_id += 1;

            self.jobs.push(Job {
                id: job_id,
                pid,
                pgid: pid,  // Process group ID same as PID for background jobs
                command: cmd_str,
                child: Some(child),
                status: JobStatus::Running,
            });

            eprintln!("[{}] {}", job_id, pid);
        }

        self.last_exit_code = 0;
        Ok(())
    }

    /// Subst: [cmd] subst - run cmd, push temp file path
    fn process_subst(&mut self) -> Result<(), EvalError> {
        let block = self.pop_block()?;
        let (cmd, args) = self.block_to_cmd_args(&block)?;

        // Create unique temp file
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let suffix = COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_path = format!("/tmp/hsab_subst_{}_{}", std::process::id(), suffix);

        // Run command, write output to temp file
        let output = Command::new(&cmd)
            .args(&args)
            .current_dir(&self.cwd)
            .output()
            .map_err(|e| EvalError::ExecError(e.to_string()))?;

        self.last_exit_code = output.status.code().unwrap_or(-1);

        let mut f = File::create(&temp_path)?;
        f.write_all(&output.stdout)?;

        // Push temp file path to stack
        self.stack.push(Value::Literal(temp_path));

        Ok(())
    }

    /// Fifo: [cmd] fifo - create named pipe, spawn cmd writing to it, push path
    fn process_fifo(&mut self) -> Result<(), EvalError> {
        let block = self.pop_block()?;
        let (cmd, args) = self.block_to_cmd_args(&block)?;

        // Create unique fifo path
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let suffix = COUNTER.fetch_add(1, Ordering::SeqCst);
        let fifo_path = format!("/tmp/hsab_fifo_{}_{}", std::process::id(), suffix);

        // Create the named pipe using mkfifo
        #[cfg(unix)]
        {
            use std::ffi::CString;

            let c_path = CString::new(fifo_path.clone())
                .map_err(|e| EvalError::ExecError(format!("fifo: invalid path: {}", e)))?;

            // mkfifo with permissions 0644
            let result = unsafe { libc::mkfifo(c_path.as_ptr(), 0o644) };
            if result != 0 {
                let err = std::io::Error::last_os_error();
                return Err(EvalError::ExecError(format!("fifo: mkfifo failed: {}", err)));
            }

            // Spawn command in background, redirecting stdout to the fifo
            // Run command first, then open fifo to write (opening blocks until reader opens)
            let fifo_path_clone = fifo_path.clone();
            let cwd = self.cwd.clone();
            std::thread::spawn(move || {
                // Run the command first to get output
                if let Ok(output) = Command::new(&cmd)
                    .args(&args)
                    .current_dir(&cwd)
                    .output()
                {
                    // Now open fifo and write (this blocks until a reader opens)
                    if let Ok(mut fifo) = std::fs::OpenOptions::new()
                        .write(true)
                        .open(&fifo_path_clone)
                    {
                        let _ = fifo.write_all(&output.stdout);
                    }
                }
            });
        }

        #[cfg(not(unix))]
        {
            // On non-Unix, fall back to subst behavior
            return self.process_subst();
        }

        // Push fifo path to stack
        self.stack.push(Value::Literal(fifo_path));
        self.last_exit_code = 0;
        Ok(())
    }

    // ==================== HELPERS ====================

    fn pop_value_or_err(&mut self) -> Result<Value, EvalError> {
        self.stack
            .pop()
            .ok_or_else(|| EvalError::StackUnderflow("pop".into()))
    }

    fn pop_block(&mut self) -> Result<Vec<Expr>, EvalError> {
        match self.pop_value_or_err()? {
            Value::Block(exprs) => Ok(exprs),
            other => Err(EvalError::TypeError {
                expected: "block".into(),
                got: format!("{:?}", other),
            }),
        }
    }

    fn pop_string(&mut self) -> Result<String, EvalError> {
        let value = self.pop_value_or_err()?;
        value.as_arg().ok_or_else(|| EvalError::TypeError {
            expected: "string".into(),
            got: format!("{:?}", value),
        })
    }

    // ==================== NEW BUILTINS ====================

    /// Read a line from stdin
    /// Stack-native: read (pushes line to stack)
    /// Legacy: VARNAME read (sets env var)
    fn builtin_read(&mut self, args: &[String]) -> Result<(), EvalError> {
        use std::io::{self, BufRead};

        let stdin = io::stdin();
        let mut line = String::new();

        match stdin.lock().read_line(&mut line) {
            Ok(0) => {
                // EOF
                self.last_exit_code = 1;
            }
            Ok(_) => {
                // Remove trailing newline
                let value = line.trim_end_matches('\n').trim_end_matches('\r').to_string();

                if args.is_empty() {
                    // Stack-native: push to stack
                    self.stack.push(Value::Output(value));
                } else {
                    // Legacy: set env var
                    let var_name = &args[0];
                    std::env::set_var(var_name, &value);
                }
                self.last_exit_code = 0;
            }
            Err(e) => {
                return Err(EvalError::ExecError(format!("read: {}", e)));
            }
        }

        Ok(())
    }

    /// Printf-style formatted output
    /// Usage: arg1 arg2 "format" printf
    fn builtin_printf(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.is_empty() {
            return Err(EvalError::ExecError("printf: format string required".into()));
        }

        let format = &args[0];
        let printf_args = &args[1..];

        let mut output = String::new();
        let mut arg_idx = 0;
        let mut chars = format.chars().peekable();

        while let Some(c) = chars.next() {
            if c == '%' {
                match chars.next() {
                    Some('s') => {
                        // String
                        if arg_idx < printf_args.len() {
                            output.push_str(&printf_args[arg_idx]);
                            arg_idx += 1;
                        }
                    }
                    Some('d') | Some('i') => {
                        // Integer
                        if arg_idx < printf_args.len() {
                            if let Ok(n) = printf_args[arg_idx].parse::<i64>() {
                                output.push_str(&n.to_string());
                            } else {
                                output.push_str(&printf_args[arg_idx]);
                            }
                            arg_idx += 1;
                        }
                    }
                    Some('f') => {
                        // Float
                        if arg_idx < printf_args.len() {
                            if let Ok(n) = printf_args[arg_idx].parse::<f64>() {
                                output.push_str(&format!("{:.6}", n));
                            } else {
                                output.push_str(&printf_args[arg_idx]);
                            }
                            arg_idx += 1;
                        }
                    }
                    Some('%') => output.push('%'),
                    Some('n') => output.push('\n'),
                    Some('t') => output.push('\t'),
                    Some(other) => {
                        output.push('%');
                        output.push(other);
                    }
                    None => output.push('%'),
                }
            } else if c == '\\' {
                // Handle escape sequences
                match chars.next() {
                    Some('n') => output.push('\n'),
                    Some('t') => output.push('\t'),
                    Some('r') => output.push('\r'),
                    Some('\\') => output.push('\\'),
                    Some(other) => {
                        output.push('\\');
                        output.push(other);
                    }
                    None => output.push('\\'),
                }
            } else {
                output.push(c);
            }
        }

        self.stack.push(Value::Output(output));
        self.last_exit_code = 0;
        Ok(())
    }

    /// Wait for background jobs to complete
    /// Usage: wait (all jobs) or %1 wait (specific job)
    fn builtin_wait(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.is_empty() {
            // Wait for all jobs
            let mut last_exit = 0;
            for job in &mut self.jobs {
                if let Some(ref mut child) = job.child {
                    match child.wait() {
                        Ok(status) => {
                            last_exit = status.code().unwrap_or(-1);
                            job.status = JobStatus::Done(last_exit);
                        }
                        Err(e) => {
                            return Err(EvalError::ExecError(format!("wait: {}", e)));
                        }
                    }
                }
            }
            self.last_exit_code = last_exit;
        } else {
            // Wait for specific job
            let job_spec = &args[0];
            let job_id: usize = if job_spec.starts_with('%') {
                job_spec[1..].parse().unwrap_or(0)
            } else {
                job_spec.parse().unwrap_or(0)
            };

            if let Some(job) = self.jobs.iter_mut().find(|j| j.id == job_id) {
                if let Some(ref mut child) = job.child {
                    match child.wait() {
                        Ok(status) => {
                            let exit_code = status.code().unwrap_or(-1);
                            job.status = JobStatus::Done(exit_code);
                            self.last_exit_code = exit_code;
                        }
                        Err(e) => {
                            return Err(EvalError::ExecError(format!("wait: {}", e)));
                        }
                    }
                }
            } else {
                return Err(EvalError::ExecError(format!("wait: no such job: {}", job_spec)));
            }
        }

        Ok(())
    }

    /// Send signal to process
    /// Usage: PID kill (SIGTERM) or PID -9 kill or PID -SIGKILL kill
    fn builtin_kill(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.is_empty() {
            return Err(EvalError::ExecError("kill: usage: PID [-signal] kill".into()));
        }

        let mut signal = 15i32; // SIGTERM default
        let mut pid_str = &args[0];

        // Check for signal arg
        if args.len() >= 2 {
            let sig_arg = &args[0];
            pid_str = &args[1];

            if sig_arg.starts_with('-') {
                let sig_spec = &sig_arg[1..];
                signal = match sig_spec.to_uppercase().as_str() {
                    "HUP" | "SIGHUP" | "1" => 1,
                    "INT" | "SIGINT" | "2" => 2,
                    "QUIT" | "SIGQUIT" | "3" => 3,
                    "KILL" | "SIGKILL" | "9" => 9,
                    "TERM" | "SIGTERM" | "15" => 15,
                    "STOP" | "SIGSTOP" | "17" => 17,
                    "CONT" | "SIGCONT" | "19" => 19,
                    _ => sig_spec.parse().unwrap_or(15),
                };
            }
        }

        // Handle job specs (%1, %2, etc.)
        let pid: i32 = if pid_str.starts_with('%') {
            let job_id: usize = pid_str[1..].parse().unwrap_or(0);
            if let Some(job) = self.jobs.iter().find(|j| j.id == job_id) {
                job.pid as i32
            } else {
                return Err(EvalError::ExecError(format!("kill: no such job: {}", pid_str)));
            }
        } else {
            pid_str.parse().map_err(|_| {
                EvalError::ExecError(format!("kill: invalid pid: {}", pid_str))
            })?
        };

        // Use libc to send signal
        #[cfg(unix)]
        {
            let result = unsafe { libc::kill(pid, signal) };
            if result != 0 {
                let err = std::io::Error::last_os_error();
                return Err(EvalError::ExecError(format!("kill: {}", err)));
            }
        }

        #[cfg(not(unix))]
        {
            return Err(EvalError::ExecError("kill: not supported on this platform".into()));
        }

        self.last_exit_code = 0;
        Ok(())
    }

    /// Push directory onto stack and cd
    /// Usage: /path pushd
    fn builtin_pushd(&mut self, args: &[String]) -> Result<(), EvalError> {
        let target = if args.is_empty() {
            // Swap top two directories
            if self.dir_stack.is_empty() {
                return Err(EvalError::ExecError("pushd: no other directory".into()));
            }
            self.dir_stack.pop().unwrap()
        } else {
            let path = self.expand_tilde(&args[0]);
            PathBuf::from(path)
        };

        // Push current directory onto stack
        self.dir_stack.push(self.cwd.clone());

        // Change to new directory
        if target.is_dir() {
            self.cwd = target.canonicalize().unwrap_or(target);
            std::env::set_current_dir(&self.cwd)?;

            // Print directory stack
            let mut output = self.cwd.display().to_string();
            for dir in self.dir_stack.iter().rev() {
                output.push(' ');
                output.push_str(&dir.display().to_string());
            }
            output.push('\n');
            self.stack.push(Value::Output(output));
            self.last_exit_code = 0;
        } else {
            // Restore stack on failure
            self.dir_stack.pop();
            return Err(EvalError::ExecError(format!(
                "pushd: {}: No such directory",
                target.display()
            )));
        }

        Ok(())
    }

    /// Pop directory from stack and cd
    /// Usage: popd
    fn builtin_popd(&mut self, _args: &[String]) -> Result<(), EvalError> {
        if self.dir_stack.is_empty() {
            return Err(EvalError::ExecError("popd: directory stack empty".into()));
        }

        let target = self.dir_stack.pop().unwrap();

        if target.is_dir() {
            self.cwd = target.canonicalize().unwrap_or(target);
            std::env::set_current_dir(&self.cwd)?;

            // Print directory stack
            let mut output = self.cwd.display().to_string();
            for dir in self.dir_stack.iter().rev() {
                output.push(' ');
                output.push_str(&dir.display().to_string());
            }
            output.push('\n');
            self.stack.push(Value::Output(output));
            self.last_exit_code = 0;
        } else {
            return Err(EvalError::ExecError(format!(
                "popd: {}: No such directory",
                target.display()
            )));
        }

        Ok(())
    }

    /// Show directory stack
    /// Usage: dirs
    fn builtin_dirs(&mut self, args: &[String]) -> Result<(), EvalError> {
        let clear = args.iter().any(|a| a == "-c");

        if clear {
            self.dir_stack.clear();
            self.last_exit_code = 0;
            return Ok(());
        }

        let mut output = self.cwd.display().to_string();
        for dir in self.dir_stack.iter().rev() {
            output.push(' ');
            output.push_str(&dir.display().to_string());
        }
        output.push('\n');
        self.stack.push(Value::Output(output));
        self.last_exit_code = 0;
        Ok(())
    }

    /// Create or list aliases (block-only, use definitions for complex cases)
    /// Usage: alias (list all) or [block] name alias
    fn builtin_alias(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.is_empty() {
            // List all aliases
            let mut output = String::new();
            let mut aliases: Vec<_> = self.aliases.iter().collect();
            aliases.sort_by_key(|(k, _)| *k);
            for (name, body) in aliases {
                let body_str = self.exprs_to_string(body);
                output.push_str(&format!("alias {}='[{}]'\n", name, body_str));
            }
            if !output.is_empty() {
                self.stack.push(Value::Output(output));
            }
            self.last_exit_code = 0;
            return Ok(());
        }

        let name = &args[0];

        // Check if there's a block on stack for: [block] name alias
        if let Some(Value::Block(block)) = self.stack.last().cloned() {
            self.stack.pop();
            self.aliases.insert(name.clone(), block);
            self.last_exit_code = 0;
            return Ok(());
        }

        // Show specific alias
        if let Some(body) = self.aliases.get(name) {
            let body_str = self.exprs_to_string(body);
            self.stack
                .push(Value::Output(format!("alias {}='[{}]'\n", name, body_str)));
            self.last_exit_code = 0;
        } else {
            return Err(EvalError::ExecError(format!("alias: {}: not found", name)));
        }

        Ok(())
    }

    /// Convert expressions back to string for display
    fn exprs_to_string(&self, exprs: &[Expr]) -> String {
        exprs
            .iter()
            .map(|e| match e {
                Expr::Literal(s) => s.clone(),
                Expr::Quoted { content, double } => {
                    if *double {
                        format!("\"{}\"", content)
                    } else {
                        format!("'{}'", content)
                    }
                }
                Expr::Variable(s) => s.clone(),
                Expr::Block(inner) => format!("[{}]", self.exprs_to_string(inner)),
                _ => format!("{:?}", e),
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Remove aliases
    /// Usage: name unalias or -a unalias (remove all)
    fn builtin_unalias(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.is_empty() {
            return Err(EvalError::ExecError("unalias: usage: name unalias".into()));
        }

        if args.iter().any(|a| a == "-a") {
            self.aliases.clear();
            self.last_exit_code = 0;
            return Ok(());
        }

        for name in args {
            if self.aliases.remove(name).is_none() {
                // Not an error in bash, just no-op
            }
        }
        self.last_exit_code = 0;
        Ok(())
    }

    /// Set signal trap
    /// Usage: [block] SIGNAL trap or trap (list) or SIGNAL trap (show specific)
    fn builtin_trap(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.is_empty() {
            // List all traps
            let mut output = String::new();
            let mut traps: Vec<_> = self.traps.iter().collect();
            traps.sort_by_key(|(k, _)| *k);
            for (sig, block) in traps {
                let sig_name = self.signal_name(*sig);
                let body_str = self.exprs_to_string(block);
                output.push_str(&format!("trap -- '[{}]' {}\n", body_str, sig_name));
            }
            if !output.is_empty() {
                self.stack.push(Value::Output(output));
            }
            self.last_exit_code = 0;
            return Ok(());
        }

        // Parse signal from first arg
        let sig_str = &args[0];
        let signal = self.parse_signal(sig_str)?;

        // Check if there's a block on the stack for: [block] SIGNAL trap
        if let Some(Value::Block(block)) = self.stack.last().cloned() {
            self.stack.pop();
            if block.is_empty() {
                // Empty block clears the trap
                self.traps.remove(&signal);
            } else {
                self.traps.insert(signal, block);
            }
            self.last_exit_code = 0;
            return Ok(());
        }

        // No block - show that specific trap
        if let Some(block) = self.traps.get(&signal) {
            let body_str = self.exprs_to_string(block);
            let sig_name = self.signal_name(signal);
            self.stack
                .push(Value::Output(format!("trap -- '[{}]' {}\n", body_str, sig_name)));
        }

        self.last_exit_code = 0;
        Ok(())
    }

    /// Parse signal name/number to signal number
    fn parse_signal(&self, s: &str) -> Result<i32, EvalError> {
        let signal = match s.to_uppercase().as_str() {
            "HUP" | "SIGHUP" | "1" => 1,
            "INT" | "SIGINT" | "2" => 2,
            "QUIT" | "SIGQUIT" | "3" => 3,
            "KILL" | "SIGKILL" | "9" => 9,
            "TERM" | "SIGTERM" | "15" => 15,
            "STOP" | "SIGSTOP" | "17" => 17,
            "CONT" | "SIGCONT" | "19" => 19,
            "USR1" | "SIGUSR1" | "10" => 10,
            "USR2" | "SIGUSR2" | "12" => 12,
            "EXIT" | "0" => 0,
            _ => s.parse().unwrap_or(-1),
        };

        if signal < 0 {
            Err(EvalError::ExecError(format!("trap: invalid signal: {}", s)))
        } else {
            Ok(signal)
        }
    }

    /// Convert signal number to name
    fn signal_name(&self, sig: i32) -> &'static str {
        match sig {
            0 => "EXIT",
            1 => "HUP",
            2 => "INT",
            3 => "QUIT",
            9 => "KILL",
            10 => "USR1",
            12 => "USR2",
            15 => "TERM",
            17 => "STOP",
            19 => "CONT",
            _ => "UNKNOWN",
        }
    }

    /// Create local variable (only meaningful inside definitions)
    /// Stack-native: value NAME local
    /// Legacy: NAME=value local or NAME local (declare only)
    fn builtin_local(&mut self, args: &[String]) -> Result<(), EvalError> {
        if self.local_scopes.is_empty() {
            return Err(EvalError::ExecError(
                "local: can only be used inside a function".into(),
            ));
        }

        if args.is_empty() {
            return Err(EvalError::ExecError("local: variable name required".into()));
        }

        // Check for stack-native form: value NAME local
        // Args come in LIFO: ["NAME", "value"] for "value NAME local"
        if args.len() >= 2 && !args[0].contains('=') && !args[1].contains('=') {
            // Restore excess args (only consume NAME and value)
            self.restore_excess_args(args, 2);

            let name = &args[0];
            let value = &args[1];

            // Save current value if not already saved
            let current_scope = self.local_scopes.last_mut().unwrap();
            if !current_scope.contains_key(name) {
                current_scope.insert(name.to_string(), std::env::var(name).ok());
            }
            std::env::set_var(name, value);
            self.last_exit_code = 0;
            return Ok(());
        }

        let current_scope = self.local_scopes.last_mut().unwrap();

        // Legacy forms
        for arg in args {
            // Check for NAME=VALUE syntax
            if let Some(eq_pos) = arg.find('=') {
                let name = &arg[..eq_pos];
                let value = &arg[eq_pos + 1..];

                // Save current value if not already saved
                if !current_scope.contains_key(name) {
                    current_scope.insert(name.to_string(), std::env::var(name).ok());
                }
                std::env::set_var(name, value);
            } else {
                // Just declare as local (save current value for restoration)
                if !current_scope.contains_key(arg) {
                    current_scope.insert(arg.clone(), std::env::var(arg).ok());
                }
            }
        }

        self.last_exit_code = 0;
        Ok(())
    }

    /// Return from definition early
    /// Usage: return or N return (with exit code)
    fn builtin_return(&mut self, args: &[String]) -> Result<(), EvalError> {
        if self.local_scopes.is_empty() {
            return Err(EvalError::ExecError(
                "return: can only be used inside a function".into(),
            ));
        }

        let exit_code: i32 = if args.is_empty() {
            self.last_exit_code
        } else {
            args[0].parse().unwrap_or(0)
        };

        self.last_exit_code = exit_code;
        self.returning = true;
        Ok(())
    }

    // =========================================
    // Stack-native predicates
    // =========================================

    /// Check if path is a file
    /// Usage: "path" file?
    fn builtin_file_predicate(&mut self, args: &[String]) -> Result<(), EvalError> {
        let path = args.first().ok_or_else(|| {
            EvalError::ExecError("file?: path required".into())
        })?;
        self.last_exit_code = if Path::new(path).is_file() { 0 } else { 1 };
        Ok(())
    }

    /// Check if path is a directory
    /// Usage: "path" dir?
    fn builtin_dir_predicate(&mut self, args: &[String]) -> Result<(), EvalError> {
        let path = args.first().ok_or_else(|| {
            EvalError::ExecError("dir?: path required".into())
        })?;
        self.last_exit_code = if Path::new(path).is_dir() { 0 } else { 1 };
        Ok(())
    }

    /// Check if path exists (file or directory)
    /// Usage: "path" exists?
    fn builtin_exists_predicate(&mut self, args: &[String]) -> Result<(), EvalError> {
        let path = args.first().ok_or_else(|| {
            EvalError::ExecError("exists?: path required".into())
        })?;
        self.restore_excess_args(args, 1);
        self.last_exit_code = if Path::new(path).exists() { 0 } else { 1 };
        Ok(())
    }

    /// Check if string is empty
    /// Usage: "string" empty?
    fn builtin_empty_predicate(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.is_empty() {
            return Err(EvalError::ExecError("empty?: string required".into()));
        }
        self.restore_excess_args(args, 1);
        let s = &args[0];
        self.last_exit_code = if s.is_empty() { 0 } else { 1 };
        Ok(())
    }

    /// Check if two strings are equal
    /// Usage: "a" "b" eq?
    fn builtin_eq_predicate(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.len() < 2 {
            return Err(EvalError::ExecError("eq?: two arguments required".into()));
        }
        self.restore_excess_args(args, 2);
        // Args come in LIFO order: ["b", "a"] for "a" "b" eq?
        let b = &args[0];
        let a = &args[1];
        self.last_exit_code = if a == b { 0 } else { 1 };
        Ok(())
    }

    /// Check if two strings are not equal
    /// Usage: "a" "b" ne?
    fn builtin_neq_predicate(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.len() < 2 {
            return Err(EvalError::ExecError("ne?: two arguments required".into()));
        }
        self.restore_excess_args(args, 2);
        // Args come in LIFO order: ["b", "a"] for "a" "b" ne?
        let b = &args[0];
        let a = &args[1];
        self.last_exit_code = if a != b { 0 } else { 1 };
        Ok(())
    }

    /// Push back unused arguments to stack (for predicates that only need 2 args)
    /// Args are in LIFO order, so we push back from end towards start
    fn restore_excess_args(&mut self, args: &[String], used: usize) {
        // Push back args[used..] in reverse order to restore original stack order
        for i in (used..args.len()).rev() {
            self.stack.push(Value::Literal(args[i].clone()));
        }
    }

    /// Check if two numbers are equal
    /// Usage: 5 5 =?
    fn builtin_numeric_eq_predicate(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.len() < 2 {
            return Err(EvalError::ExecError("=?: two arguments required".into()));
        }
        // Restore any excess arguments first
        self.restore_excess_args(args, 2);
        let b: i64 = args[0].parse().unwrap_or(0);
        let a: i64 = args[1].parse().unwrap_or(0);
        self.last_exit_code = if a == b { 0 } else { 1 };
        Ok(())
    }

    /// Check if two numbers are not equal
    /// Usage: 5 10 !=?
    fn builtin_numeric_neq_predicate(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.len() < 2 {
            return Err(EvalError::ExecError("!=?: two arguments required".into()));
        }
        self.restore_excess_args(args, 2);
        let b: i64 = args[0].parse().unwrap_or(0);
        let a: i64 = args[1].parse().unwrap_or(0);
        self.last_exit_code = if a != b { 0 } else { 1 };
        Ok(())
    }

    /// Check if first number < second
    /// Usage: 5 10 lt?
    fn builtin_numeric_lt_predicate(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.len() < 2 {
            return Err(EvalError::ExecError("lt?: two arguments required".into()));
        }
        self.restore_excess_args(args, 2);
        let b: i64 = args[0].parse().unwrap_or(0);
        let a: i64 = args[1].parse().unwrap_or(0);
        self.last_exit_code = if a < b { 0 } else { 1 };
        Ok(())
    }

    /// Check if first number > second
    /// Usage: 10 5 gt?
    fn builtin_numeric_gt_predicate(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.len() < 2 {
            return Err(EvalError::ExecError("gt?: two arguments required".into()));
        }
        self.restore_excess_args(args, 2);
        let b: i64 = args[0].parse().unwrap_or(0);
        let a: i64 = args[1].parse().unwrap_or(0);
        self.last_exit_code = if a > b { 0 } else { 1 };
        Ok(())
    }

    /// Check if first number <= second
    /// Usage: 5 10 le?
    fn builtin_numeric_le_predicate(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.len() < 2 {
            return Err(EvalError::ExecError("le?: two arguments required".into()));
        }
        self.restore_excess_args(args, 2);
        let b: i64 = args[0].parse().unwrap_or(0);
        let a: i64 = args[1].parse().unwrap_or(0);
        self.last_exit_code = if a <= b { 0 } else { 1 };
        Ok(())
    }

    /// Check if first number >= second
    /// Usage: 10 5 ge?
    fn builtin_numeric_ge_predicate(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.len() < 2 {
            return Err(EvalError::ExecError("ge?: two arguments required".into()));
        }
        self.restore_excess_args(args, 2);
        let b: i64 = args[0].parse().unwrap_or(0);
        let a: i64 = args[1].parse().unwrap_or(0);
        self.last_exit_code = if a >= b { 0 } else { 1 };
        Ok(())
    }

    // =========================================
    // Arithmetic primitives
    // =========================================

    /// Add two numbers
    /// Usage: 5 3 plus → 8
    fn builtin_add(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.len() < 2 {
            return Err(EvalError::ExecError("plus: two arguments required".into()));
        }
        self.restore_excess_args(args, 2);
        let b: i64 = args[0].parse().unwrap_or(0);
        let a: i64 = args[1].parse().unwrap_or(0);
        self.stack.push(Value::Output((a + b).to_string()));
        self.last_exit_code = 0;
        Ok(())
    }

    /// Subtract two numbers
    /// Usage: 10 3 minus → 7
    fn builtin_sub(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.len() < 2 {
            return Err(EvalError::ExecError("minus: two arguments required".into()));
        }
        self.restore_excess_args(args, 2);
        let b: i64 = args[0].parse().unwrap_or(0);
        let a: i64 = args[1].parse().unwrap_or(0);
        self.stack.push(Value::Output((a - b).to_string()));
        self.last_exit_code = 0;
        Ok(())
    }

    /// Multiply two numbers
    /// Usage: 4 5 mul → 20
    fn builtin_mul(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.len() < 2 {
            return Err(EvalError::ExecError("mul: two arguments required".into()));
        }
        self.restore_excess_args(args, 2);
        let b: i64 = args[0].parse().unwrap_or(0);
        let a: i64 = args[1].parse().unwrap_or(0);
        self.stack.push(Value::Output((a * b).to_string()));
        self.last_exit_code = 0;
        Ok(())
    }

    /// Divide two numbers (integer division)
    /// Usage: 10 3 div → 3
    fn builtin_div(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.len() < 2 {
            return Err(EvalError::ExecError("div: two arguments required".into()));
        }
        self.restore_excess_args(args, 2);
        let b: i64 = args[0].parse().unwrap_or(0);
        let a: i64 = args[1].parse().unwrap_or(0);
        if b == 0 {
            return Err(EvalError::ExecError("div: division by zero".into()));
        }
        self.stack.push(Value::Output((a / b).to_string()));
        self.last_exit_code = 0;
        Ok(())
    }

    /// Modulo (remainder)
    /// Usage: 10 3 mod → 1
    fn builtin_mod(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.len() < 2 {
            return Err(EvalError::ExecError("mod: two arguments required".into()));
        }
        self.restore_excess_args(args, 2);
        let b: i64 = args[0].parse().unwrap_or(0);
        let a: i64 = args[1].parse().unwrap_or(0);
        if b == 0 {
            return Err(EvalError::ExecError("mod: division by zero".into()));
        }
        self.stack.push(Value::Output((a % b).to_string()));
        self.last_exit_code = 0;
        Ok(())
    }

    // =========================================
    // String primitives
    // =========================================

    /// Get string length
    /// Usage: "hello" len → 5
    fn builtin_len(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.is_empty() {
            return Err(EvalError::ExecError("len: string required".into()));
        }
        self.restore_excess_args(args, 1);
        let s = &args[0];
        self.stack.push(Value::Output(s.chars().count().to_string()));
        self.last_exit_code = 0;
        Ok(())
    }

    /// Extract substring
    /// Usage: "hello" 1 3 slice → "ell" (start at index 1, take 3 chars)
    fn builtin_slice(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.len() < 3 {
            return Err(EvalError::ExecError("slice: string start length required".into()));
        }
        self.restore_excess_args(args, 3);
        // Args in LIFO: [length, start, string] for "string start length slice"
        let length: usize = args[0].parse().unwrap_or(0);
        let start: usize = args[1].parse().unwrap_or(0);
        let s = &args[2];

        let result: String = s.chars().skip(start).take(length).collect();
        self.stack.push(Value::Output(result));
        self.last_exit_code = 0;
        Ok(())
    }

    /// Find substring, returns index or -1 if not found
    /// Usage: "hello" "ll" indexof → 2
    fn builtin_indexof(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.len() < 2 {
            return Err(EvalError::ExecError("indexof: string needle required".into()));
        }
        self.restore_excess_args(args, 2);
        // Args in LIFO: [needle, haystack] for "haystack needle indexof"
        let needle = &args[0];
        let haystack = &args[1];

        let result = match haystack.find(needle.as_str()) {
            Some(idx) => idx as i64,
            None => -1,
        };
        self.stack.push(Value::Output(result.to_string()));
        self.last_exit_code = 0;
        Ok(())
    }

    /// Replace all occurrences of a substring
    /// Usage: "hello" "l" "L" str-replace → "heLLo"
    fn builtin_str_replace(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.len() < 3 {
            return Err(EvalError::ExecError("str-replace: string from to required".into()));
        }
        self.restore_excess_args(args, 3);
        // Args in LIFO: [to, from, string] for "string from to str-replace"
        let to = &args[0];
        let from = &args[1];
        let s = &args[2];

        let result = s.replace(from, to);
        self.stack.push(Value::Output(result));
        self.last_exit_code = 0;
        Ok(())
    }

    /// String interpolation: name "Hello, {}!" format -> "Hello, Alice!"
    /// Positional: alice bob "{1} meets {0}" format -> "alice meets bob"
    fn builtin_format(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.is_empty() {
            return Err(EvalError::ExecError("format: template string required".into()));
        }

        // Convention: value1 value2 template format (template pushed LAST, just before format)
        // For Alice "Hello, {}!" format:
        //   Stack = ["Alice", "Hello, {}!"], pops → args = ["Hello, {}!", "Alice"]
        // Template is FIRST in args (last pushed = top of stack = first popped)
        let template = &args[0];
        // Values are the rest, already in push order after reversing
        let values: Vec<&str> = args[1..].iter().rev().map(|s| s.as_str()).collect();

        let mut result = template.clone();
        let mut next_idx = 0;

        // Replace {} with next value
        while let Some(pos) = result.find("{}") {
            if next_idx >= values.len() {
                break;
            }
            result = format!("{}{}{}", &result[..pos], values[next_idx], &result[pos + 2..]);
            next_idx += 1;
        }

        // Replace {0}, {1}, etc. with positional values
        for (i, val) in values.iter().enumerate() {
            let placeholder = format!("{{{}}}", i);
            result = result.replace(&placeholder, val);
        }

        self.stack.push(Value::Output(result));
        self.last_exit_code = 0;
        Ok(())
    }

    // ========================================
    // Phase 0: Type introspection
    // ========================================

    fn builtin_typeof(&mut self) -> Result<(), EvalError> {
        let val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("typeof requires a value".into()))?;

        let type_name = match val {
            Value::Literal(_) | Value::Output(_) => "String",
            Value::Number(_) => "Number",
            Value::Bool(_) => "Boolean",
            Value::List(_) => "List",
            Value::Map(_) => "Record",
            Value::Table { .. } => "Table",
            Value::Block(_) => "Block",
            Value::Nil => "Null",
            Value::Marker => "Marker",
            Value::Error { .. } => "Error",
        };

        self.stack.push(Value::Literal(type_name.to_string()));
        self.last_exit_code = 0;
        Ok(())
    }

    // ========================================
    // Phase 1: Record operations
    // ========================================

    fn builtin_record(&mut self) -> Result<(), EvalError> {
        // Collect key-value pairs from stack until marker, non-string key, or empty
        let mut pairs: Vec<(String, Value)> = Vec::new();

        while self.stack.len() >= 2 {
            if matches!(self.stack.last(), Some(Value::Marker)) {
                self.stack.pop(); // consume marker
                break;
            }

            // Peek at potential key (second from top) - must be a string
            let potential_key = self.stack.get(self.stack.len() - 2);
            match potential_key {
                Some(Value::Literal(_)) | Some(Value::Output(_)) => {
                    // Valid string key, continue
                }
                _ => {
                    // Not a valid string key, stop collecting
                    break;
                }
            }

            let value = self.stack.pop().unwrap();
            let key_val = self.stack.pop().unwrap();
            let key = key_val.as_arg().unwrap(); // Safe because we checked above
            pairs.push((key, value));
        }

        // Consume marker if present
        if matches!(self.stack.last(), Some(Value::Marker)) {
            self.stack.pop();
        }

        pairs.reverse(); // Restore original order
        let map: HashMap<String, Value> = pairs.into_iter().collect();
        self.stack.push(Value::Map(map));
        self.last_exit_code = 0;
        Ok(())
    }

    fn builtin_get(&mut self) -> Result<(), EvalError> {
        let key_val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("get requires key".into()))?;
        let key = key_val.as_arg().ok_or_else(||
            EvalError::TypeError { expected: "String".into(), got: format!("{:?}", key_val) })?;

        let target = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("get requires record/table".into()))?;

        // Check if key contains dots for deep access
        if key.contains('.') {
            let result = self.deep_get(&target, &key);
            self.stack.push(result);
            self.last_exit_code = 0;
            return Ok(());
        }

        match target {
            Value::Map(map) => {
                match map.get(&key) {
                    Some(v) => self.stack.push(v.clone()),
                    None => self.stack.push(Value::Nil),
                }
            }
            Value::List(items) => {
                // Numeric index for lists
                if let Ok(idx) = key.parse::<usize>() {
                    self.stack.push(items.get(idx).cloned().unwrap_or(Value::Nil));
                } else {
                    self.stack.push(Value::Nil);
                }
            }
            Value::Table { columns, rows } => {
                // Get column as list
                if let Some(col_idx) = columns.iter().position(|c| c == &key) {
                    let values: Vec<Value> = rows.iter()
                        .map(|row| row.get(col_idx).cloned().unwrap_or(Value::Nil))
                        .collect();
                    self.stack.push(Value::List(values));
                } else {
                    self.stack.push(Value::Nil);
                }
            }
            Value::Error { kind, message, code, source, command } => {
                // Allow getting fields from error
                let field = match key.as_str() {
                    "kind" => Some(Value::Literal(kind)),
                    "message" => Some(Value::Literal(message)),
                    "code" => code.map(|c| Value::Number(c as f64)),
                    "source" => source.map(Value::Literal),
                    "command" => command.map(Value::Literal),
                    _ => None,
                };
                self.stack.push(field.unwrap_or(Value::Nil));
            }
            _ => return Err(EvalError::TypeError {
                expected: "Record, Table, List, or Error".into(),
                got: format!("{:?}", target),
            }),
        }

        self.last_exit_code = 0;
        Ok(())
    }

    /// Deep get with dot-notation path like "server.port" or "items.0"
    fn deep_get(&self, val: &Value, path: &str) -> Value {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = val.clone();

        for part in parts {
            current = match current {
                Value::Map(map) => {
                    map.get(part).cloned().unwrap_or(Value::Nil)
                }
                Value::List(items) => {
                    if let Ok(idx) = part.parse::<usize>() {
                        items.get(idx).cloned().unwrap_or(Value::Nil)
                    } else {
                        Value::Nil
                    }
                }
                _ => Value::Nil,
            };

            // Early exit if we hit Nil
            if matches!(current, Value::Nil) {
                break;
            }
        }

        current
    }

    fn builtin_set(&mut self) -> Result<(), EvalError> {
        let value = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("set requires value".into()))?;
        let key_val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("set requires key".into()))?;
        let key = key_val.as_arg().ok_or_else(||
            EvalError::TypeError { expected: "String".into(), got: format!("{:?}", key_val) })?;
        let target = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("set requires record".into()))?;

        // Check if key contains dots for deep set
        if key.contains('.') {
            let result = self.deep_set(target, &key, value)?;
            self.stack.push(result);
            self.last_exit_code = 0;
            return Ok(());
        }

        match target {
            Value::Map(mut map) => {
                map.insert(key, value);
                self.stack.push(Value::Map(map));
            }
            _ => return Err(EvalError::TypeError {
                expected: "Record".into(),
                got: format!("{:?}", target),
            }),
        }

        self.last_exit_code = 0;
        Ok(())
    }

    /// Deep set a value at a dot-path (e.g., "server.port")
    fn deep_set(&self, target: Value, path: &str, value: Value) -> Result<Value, EvalError> {
        let parts: Vec<&str> = path.split('.').collect();
        if parts.is_empty() {
            return Ok(target);
        }

        self.deep_set_recursive(target, &parts, value)
    }

    fn deep_set_recursive(&self, target: Value, path: &[&str], value: Value) -> Result<Value, EvalError> {
        if path.is_empty() {
            return Ok(value);
        }

        let key = path[0];
        let remaining = &path[1..];

        match target {
            Value::Map(mut map) => {
                if remaining.is_empty() {
                    // Last key - set the value directly
                    map.insert(key.to_string(), value);
                } else {
                    // Need to recurse
                    let current = map.get(key).cloned().unwrap_or_else(|| Value::Map(HashMap::new()));
                    let new_val = self.deep_set_recursive(current, remaining, value)?;
                    map.insert(key.to_string(), new_val);
                }
                Ok(Value::Map(map))
            }
            Value::Nil => {
                // Create nested structure
                let mut map = HashMap::new();
                if remaining.is_empty() {
                    map.insert(key.to_string(), value);
                } else {
                    let new_val = self.deep_set_recursive(Value::Nil, remaining, value)?;
                    map.insert(key.to_string(), new_val);
                }
                Ok(Value::Map(map))
            }
            _ => Err(EvalError::TypeError {
                expected: "Record".into(),
                got: format!("{:?}", target),
            }),
        }
    }

    fn builtin_del(&mut self) -> Result<(), EvalError> {
        let key_val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("del requires key".into()))?;
        let key = key_val.as_arg().ok_or_else(||
            EvalError::TypeError { expected: "String".into(), got: format!("{:?}", key_val) })?;
        let target = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("del requires record".into()))?;

        match target {
            Value::Map(mut map) => {
                map.remove(&key);
                self.stack.push(Value::Map(map));
            }
            _ => return Err(EvalError::TypeError {
                expected: "Record".into(),
                got: format!("{:?}", target),
            }),
        }

        self.last_exit_code = 0;
        Ok(())
    }

    fn builtin_has(&mut self) -> Result<(), EvalError> {
        let key_val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("has? requires key".into()))?;
        let key = key_val.as_arg().ok_or_else(||
            EvalError::TypeError { expected: "String".into(), got: format!("{:?}", key_val) })?;
        let target = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("has? requires record".into()))?;

        let has_key = match target {
            Value::Map(map) => map.contains_key(&key),
            Value::Table { columns, .. } => columns.contains(&key),
            _ => false,
        };

        self.last_exit_code = if has_key { 0 } else { 1 };
        Ok(())
    }

    fn builtin_keys(&mut self) -> Result<(), EvalError> {
        let target = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("keys requires record".into()))?;

        match target {
            Value::Map(map) => {
                let keys: Vec<Value> = map.keys()
                    .map(|k| Value::Literal(k.clone()))
                    .collect();
                self.stack.push(Value::List(keys));
            }
            Value::Table { columns, .. } => {
                let keys: Vec<Value> = columns.iter()
                    .map(|k| Value::Literal(k.clone()))
                    .collect();
                self.stack.push(Value::List(keys));
            }
            _ => return Err(EvalError::TypeError {
                expected: "Record or Table".into(),
                got: format!("{:?}", target),
            }),
        }

        self.last_exit_code = 0;
        Ok(())
    }

    fn builtin_values(&mut self) -> Result<(), EvalError> {
        let target = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("values requires record".into()))?;

        match target {
            Value::Map(map) => {
                let values: Vec<Value> = map.values().cloned().collect();
                self.stack.push(Value::List(values));
            }
            _ => return Err(EvalError::TypeError {
                expected: "Record".into(),
                got: format!("{:?}", target),
            }),
        }

        self.last_exit_code = 0;
        Ok(())
    }

    fn builtin_merge(&mut self) -> Result<(), EvalError> {
        let right = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("merge requires two records".into()))?;
        let left = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("merge requires two records".into()))?;

        match (left, right) {
            (Value::Map(mut left_map), Value::Map(right_map)) => {
                left_map.extend(right_map);
                self.stack.push(Value::Map(left_map));
            }
            _ => return Err(EvalError::TypeError {
                expected: "two Records".into(),
                got: "non-record values".into(),
            }),
        }

        self.last_exit_code = 0;
        Ok(())
    }

    // ========================================
    // Phase 2: Table operations
    // ========================================

    fn builtin_table(&mut self) -> Result<(), EvalError> {
        // Collect records from stack until marker
        let mut records: Vec<HashMap<String, Value>> = Vec::new();

        while let Some(val) = self.stack.pop() {
            match val {
                Value::Marker => break,
                Value::Map(map) => records.push(map),
                _ => return Err(EvalError::TypeError {
                    expected: "Record".into(),
                    got: format!("{:?}", val),
                }),
            }
        }

        records.reverse(); // Restore original order

        if records.is_empty() {
            self.stack.push(Value::Table { columns: vec![], rows: vec![] });
            self.last_exit_code = 0;
            return Ok(());
        }

        // Get columns from first record
        let columns: Vec<String> = records[0].keys().cloned().collect();

        // Build rows
        let rows: Vec<Vec<Value>> = records.iter()
            .map(|rec| {
                columns.iter()
                    .map(|col| rec.get(col).cloned().unwrap_or(Value::Nil))
                    .collect()
            })
            .collect();

        self.stack.push(Value::Table { columns, rows });
        self.last_exit_code = 0;
        Ok(())
    }

    fn builtin_where(&mut self) -> Result<(), EvalError> {
        let predicate = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("where requires predicate block".into()))?;
        let pred_block = match predicate {
            Value::Block(exprs) => exprs,
            _ => return Err(EvalError::TypeError {
                expected: "Block".into(),
                got: format!("{:?}", predicate),
            }),
        };

        let table = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("where requires table".into()))?;

        match table {
            Value::Table { columns, rows } => {
                let mut filtered_rows = Vec::new();

                for row in rows {
                    // Create record from row
                    let record: HashMap<String, Value> = columns.iter()
                        .zip(row.iter())
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect();

                    // Save stack, push record, run predicate
                    let saved_stack = std::mem::take(&mut self.stack);
                    self.stack.push(Value::Map(record.clone()));

                    for expr in &pred_block {
                        self.eval_expr(expr)?;
                    }

                    let keep = self.last_exit_code == 0;
                    self.stack = saved_stack;

                    if keep {
                        filtered_rows.push(row);
                    }
                }

                self.stack.push(Value::Table { columns, rows: filtered_rows });
            }
            _ => return Err(EvalError::TypeError {
                expected: "Table".into(),
                got: format!("{:?}", table),
            }),
        }

        self.last_exit_code = 0;
        Ok(())
    }

    fn builtin_sort_by(&mut self) -> Result<(), EvalError> {
        let key_val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("sort-by requires key/column".into()))?;

        let data = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("sort-by requires table or list".into()))?;

        match data {
            Value::Table { columns, mut rows } => {
                // Existing table logic
                let col = key_val.as_arg().ok_or_else(||
                    EvalError::TypeError { expected: "String".into(), got: format!("{:?}", key_val) })?;

                if let Some(col_idx) = columns.iter().position(|c| c == &col) {
                    rows.sort_by(|a, b| {
                        let av = a.get(col_idx).and_then(|v| v.as_arg()).unwrap_or_default();
                        let bv = b.get(col_idx).and_then(|v| v.as_arg()).unwrap_or_default();
                        Self::compare_strings(&av, &bv)
                    });
                }
                self.stack.push(Value::Table { columns, rows });
            }
            Value::List(mut items) => {
                // Sort list items by key field (for records/maps)
                let key_name = key_val.as_arg().unwrap_or_default();

                items.sort_by(|a, b| {
                    let av = Self::extract_sort_key(a, &key_name);
                    let bv = Self::extract_sort_key(b, &key_name);
                    Self::compare_strings(&av, &bv)
                });

                self.stack.push(Value::List(items));
            }
            _ => return Err(EvalError::TypeError {
                expected: "Table or List".into(),
                got: format!("{:?}", data),
            }),
        }

        self.last_exit_code = 0;
        Ok(())
    }

    /// Helper: extract sort key from value
    fn extract_sort_key(val: &Value, key: &str) -> String {
        match val {
            Value::Map(m) => m.get(key)
                .and_then(|v| v.as_arg())
                .unwrap_or_default(),
            _ => val.as_arg().unwrap_or_default(),
        }
    }

    /// Helper: compare with numeric awareness
    fn compare_strings(a: &str, b: &str) -> std::cmp::Ordering {
        match (a.parse::<f64>(), b.parse::<f64>()) {
            (Ok(an), Ok(bn)) => an.partial_cmp(&bn).unwrap_or(std::cmp::Ordering::Equal),
            _ => a.cmp(b),
        }
    }

    fn builtin_select(&mut self) -> Result<(), EvalError> {
        let cols_val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("select requires column list".into()))?;

        let cols: Vec<String> = match cols_val {
            Value::List(items) => items.iter()
                .filter_map(|v| v.as_arg())
                .collect(),
            Value::Block(exprs) => {
                // Convert block of literals to list of strings
                exprs.iter()
                    .filter_map(|e| match e {
                        Expr::Literal(s) => Some(s.clone()),
                        Expr::Quoted { content, .. } => Some(content.clone()),
                        _ => None,
                    })
                    .collect()
            }
            _ => return Err(EvalError::TypeError {
                expected: "List or Block".into(),
                got: format!("{:?}", cols_val),
            }),
        };

        let table = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("select requires table".into()))?;

        match table {
            Value::Table { columns, rows } => {
                // Find indices of columns to keep
                let indices: Vec<usize> = cols.iter()
                    .filter_map(|c| columns.iter().position(|col| col == c))
                    .collect();

                let new_rows: Vec<Vec<Value>> = rows.iter()
                    .map(|row| {
                        indices.iter()
                            .map(|&i| row.get(i).cloned().unwrap_or(Value::Nil))
                            .collect()
                    })
                    .collect();

                self.stack.push(Value::Table { columns: cols, rows: new_rows });
            }
            _ => return Err(EvalError::TypeError {
                expected: "Table".into(),
                got: format!("{:?}", table),
            }),
        }

        self.last_exit_code = 0;
        Ok(())
    }

    fn builtin_first(&mut self) -> Result<(), EvalError> {
        let n_val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("first requires count".into()))?;
        let n: usize = n_val.as_arg()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1);

        let table = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("first requires table".into()))?;

        match table {
            Value::Table { columns, rows } => {
                let new_rows: Vec<Vec<Value>> = rows.into_iter().take(n).collect();
                self.stack.push(Value::Table { columns, rows: new_rows });
            }
            Value::List(items) => {
                let new_items: Vec<Value> = items.into_iter().take(n).collect();
                self.stack.push(Value::List(new_items));
            }
            _ => return Err(EvalError::TypeError {
                expected: "Table or List".into(),
                got: format!("{:?}", table),
            }),
        }

        self.last_exit_code = 0;
        Ok(())
    }

    fn builtin_last(&mut self) -> Result<(), EvalError> {
        let n_val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("last requires count".into()))?;
        let n: usize = n_val.as_arg()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1);

        let table = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("last requires table".into()))?;

        match table {
            Value::Table { columns, rows } => {
                let len = rows.len();
                let skip = len.saturating_sub(n);
                let new_rows: Vec<Vec<Value>> = rows.into_iter().skip(skip).collect();
                self.stack.push(Value::Table { columns, rows: new_rows });
            }
            Value::List(items) => {
                let len = items.len();
                let skip = len.saturating_sub(n);
                let new_items: Vec<Value> = items.into_iter().skip(skip).collect();
                self.stack.push(Value::List(new_items));
            }
            _ => return Err(EvalError::TypeError {
                expected: "Table or List".into(),
                got: format!("{:?}", table),
            }),
        }

        self.last_exit_code = 0;
        Ok(())
    }

    fn builtin_nth(&mut self) -> Result<(), EvalError> {
        let n_val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("nth requires index".into()))?;
        let n: usize = n_val.as_arg()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        let table = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("nth requires table".into()))?;

        match table {
            Value::Table { columns, rows } => {
                if n < rows.len() {
                    let row = &rows[n];
                    let record: HashMap<String, Value> = columns.iter()
                        .zip(row.iter())
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect();
                    self.stack.push(Value::Map(record));
                } else {
                    self.stack.push(Value::Nil);
                }
            }
            Value::List(items) => {
                if n < items.len() {
                    self.stack.push(items[n].clone());
                } else {
                    self.stack.push(Value::Nil);
                }
            }
            _ => return Err(EvalError::TypeError {
                expected: "Table or List".into(),
                got: format!("{:?}", table),
            }),
        }

        self.last_exit_code = 0;
        Ok(())
    }

    // ========================================
    // Phase 3: Error handling
    // ========================================

    fn builtin_try(&mut self) -> Result<(), EvalError> {
        let block = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("try requires block".into()))?;

        let exprs = match block {
            Value::Block(exprs) => exprs,
            _ => return Err(EvalError::TypeError {
                expected: "Block".into(),
                got: format!("{:?}", block),
            }),
        };

        // Save current state
        let saved_stack = self.stack.clone();

        // Try to execute
        let result = (|| -> Result<(), EvalError> {
            for expr in &exprs {
                self.eval_expr(expr)?;
            }
            Ok(())
        })();

        match result {
            Ok(()) => {
                // Success - stack has results
                self.last_exit_code = 0;
            }
            Err(e) => {
                // Error - restore stack and push error value
                self.stack = saved_stack;
                self.stack.push(Value::Error {
                    kind: "eval_error".to_string(),
                    message: e.to_string(),
                    code: Some(self.last_exit_code),
                    source: None,
                    command: None,
                });
                self.last_exit_code = 1;
            }
        }

        Ok(())
    }

    fn builtin_error_predicate(&mut self) -> Result<(), EvalError> {
        let val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("error? requires value".into()))?;

        let is_error = matches!(val, Value::Error { .. });
        self.stack.push(val); // Put it back

        self.last_exit_code = if is_error { 0 } else { 1 };
        Ok(())
    }

    fn builtin_throw(&mut self) -> Result<(), EvalError> {
        let msg_val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("throw requires message".into()))?;
        let message = msg_val.as_arg().unwrap_or_else(|| "unknown error".to_string());

        self.stack.push(Value::Error {
            kind: "thrown".to_string(),
            message,
            code: None,
            source: None,
            command: None,
        });

        self.last_exit_code = 1;
        Ok(())
    }

    // ========================================
    // Phase 4: Serialization bridge
    // ========================================

    fn builtin_into_json(&mut self) -> Result<(), EvalError> {
        let val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("into-json requires string".into()))?;
        let text = val.as_arg().ok_or_else(||
            EvalError::TypeError { expected: "String".into(), got: format!("{:?}", val) })?;

        let json: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| EvalError::ExecError(format!("into-json: {}", e)))?;

        self.stack.push(crate::ast::json_to_value(json));
        self.last_exit_code = 0;
        Ok(())
    }

    fn builtin_into_csv(&mut self) -> Result<(), EvalError> {
        let val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("into-csv requires string".into()))?;
        let text = val.as_arg().ok_or_else(||
            EvalError::TypeError { expected: "String".into(), got: format!("{:?}", val) })?;

        let mut lines = text.lines();
        let header = lines.next().ok_or_else(||
            EvalError::ExecError("into-csv: empty input".into()))?;

        let columns: Vec<String> = header.split(',')
            .map(|s| s.trim().to_string())
            .collect();

        let rows: Vec<Vec<Value>> = lines
            .filter(|line| !line.trim().is_empty())
            .map(|line| {
                line.split(',')
                    .map(|s| {
                        let trimmed = s.trim();
                        // Try to parse as number
                        if let Ok(n) = trimmed.parse::<f64>() {
                            Value::Number(n)
                        } else {
                            Value::Literal(trimmed.to_string())
                        }
                    })
                    .collect()
            })
            .collect();

        self.stack.push(Value::Table { columns, rows });
        self.last_exit_code = 0;
        Ok(())
    }

    fn builtin_into_lines(&mut self) -> Result<(), EvalError> {
        let val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("into-lines requires string".into()))?;
        let text = val.as_arg().ok_or_else(||
            EvalError::TypeError { expected: "String".into(), got: format!("{:?}", val) })?;

        let lines: Vec<Value> = text.lines()
            .map(|s| Value::Literal(s.to_string()))
            .collect();

        self.stack.push(Value::List(lines));
        self.last_exit_code = 0;
        Ok(())
    }

    fn builtin_into_kv(&mut self) -> Result<(), EvalError> {
        let val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("into-kv requires string".into()))?;
        let text = val.as_arg().ok_or_else(||
            EvalError::TypeError { expected: "String".into(), got: format!("{:?}", val) })?;

        let mut map = HashMap::new();
        for line in text.lines() {
            if let Some(eq_pos) = line.find('=') {
                let key = line[..eq_pos].trim().to_string();
                let value = line[eq_pos + 1..].trim().to_string();
                map.insert(key, Value::Literal(value));
            }
        }

        self.stack.push(Value::Map(map));
        self.last_exit_code = 0;
        Ok(())
    }

    fn builtin_to_json(&mut self) -> Result<(), EvalError> {
        let val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("to-json requires value".into()))?;

        let json = crate::ast::value_to_json(&val);
        let text = serde_json::to_string(&json)
            .map_err(|e| EvalError::ExecError(format!("to-json: {}", e)))?;

        self.stack.push(Value::Output(text));
        self.last_exit_code = 0;
        Ok(())
    }

    fn builtin_to_csv(&mut self) -> Result<(), EvalError> {
        let val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("to-csv requires table".into()))?;

        match val {
            Value::Table { columns, rows } => {
                let mut lines = vec![columns.join(",")];
                for row in rows {
                    let line: Vec<String> = row.iter()
                        .map(|v| v.as_arg().unwrap_or_default())
                        .collect();
                    lines.push(line.join(","));
                }
                self.stack.push(Value::Output(lines.join("\n")));
            }
            _ => return Err(EvalError::TypeError {
                expected: "Table".into(),
                got: format!("{:?}", val),
            }),
        }

        self.last_exit_code = 0;
        Ok(())
    }

    fn builtin_to_lines(&mut self) -> Result<(), EvalError> {
        let val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("to-lines requires list".into()))?;

        match val {
            Value::List(items) => {
                let lines: Vec<String> = items.iter()
                    .filter_map(|v| v.as_arg())
                    .collect();
                self.stack.push(Value::Output(lines.join("\n")));
            }
            _ => return Err(EvalError::TypeError {
                expected: "List".into(),
                got: format!("{:?}", val),
            }),
        }

        self.last_exit_code = 0;
        Ok(())
    }

    fn builtin_to_kv(&mut self) -> Result<(), EvalError> {
        let val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("to-kv requires record".into()))?;

        match val {
            Value::Map(map) => {
                let mut pairs: Vec<_> = map.iter()
                    .map(|(k, v)| {
                        let val_str = v.as_arg().unwrap_or_default();
                        format!("{}={}", k, val_str)
                    })
                    .collect();
                pairs.sort(); // Consistent ordering
                self.stack.push(Value::Output(pairs.join("\n")));
            }
            _ => return Err(EvalError::TypeError {
                expected: "Record".into(),
                got: format!("{:?}", val),
            }),
        }

        self.last_exit_code = 0;
        Ok(())
    }

    // ========================================
    // Phase 5: Stack utilities
    // ========================================

    /// tap: Execute block for side effect, keep original value
    /// Stack effect: a [block] → a
    fn builtin_tap(&mut self) -> Result<(), EvalError> {
        let block = match self.stack.pop() {
            Some(Value::Block(b)) => b,
            Some(other) => return Err(EvalError::TypeError {
                expected: "Block".into(),
                got: format!("{:?}", other),
            }),
            None => return Err(EvalError::StackUnderflow("tap requires block".into())),
        };

        // Remember the value we want to keep (clone it, don't pop)
        let original = self.stack.last().cloned()
            .ok_or_else(|| EvalError::StackUnderflow("tap requires a value under block".into()))?;

        // Remember stack depth BEFORE the original (so we can restore completely)
        let depth_before_original = self.stack.len() - 1;

        // Execute block (original is still on stack for block to use)
        for expr in &block {
            self.eval_expr(expr)?;
        }

        // Discard everything including what block produced, back to before original
        self.stack.truncate(depth_before_original);

        // Restore the original value
        self.stack.push(original);

        self.last_exit_code = 0;
        Ok(())
    }

    /// dip: Pop top, execute block, push value back
    /// Stack effect: a b [block] → a (block results) b
    fn builtin_dip(&mut self) -> Result<(), EvalError> {
        let block = match self.stack.pop() {
            Some(Value::Block(b)) => b,
            Some(other) => return Err(EvalError::TypeError {
                expected: "Block".into(),
                got: format!("{:?}", other),
            }),
            None => return Err(EvalError::StackUnderflow("dip requires block".into())),
        };

        // Save the top value
        let saved = self.stack.pop()
            .ok_or_else(|| EvalError::StackUnderflow("dip requires a value under block".into()))?;

        // Execute block on remaining stack
        for expr in &block {
            self.eval_expr(expr)?;
        }

        // Restore saved value
        self.stack.push(saved);
        self.last_exit_code = 0;
        Ok(())
    }

    // ========================================
    // Phase 6: Aggregation operations
    // ========================================

    fn builtin_sum(&mut self) -> Result<(), EvalError> {
        let val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("sum requires a list".into()))?;

        let total: f64 = match val {
            Value::List(items) => {
                items.iter().filter_map(|v| match v {
                    Value::Number(n) => Some(*n),
                    Value::Literal(s) | Value::Output(s) => s.trim().parse().ok(),
                    _ => None,
                }).sum()
            }
            _ => return Err(EvalError::TypeError {
                expected: "List".into(),
                got: format!("{:?}", val),
            }),
        };

        self.stack.push(Value::Number(total));
        self.last_exit_code = 0;
        Ok(())
    }

    fn builtin_avg(&mut self) -> Result<(), EvalError> {
        let val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("avg requires a list".into()))?;

        let (total, count) = match val {
            Value::List(items) => {
                let nums: Vec<f64> = items.iter().filter_map(|v| match v {
                    Value::Number(n) => Some(*n),
                    Value::Literal(s) | Value::Output(s) => s.trim().parse().ok(),
                    _ => None,
                }).collect();
                (nums.iter().sum::<f64>(), nums.len())
            }
            _ => return Err(EvalError::TypeError {
                expected: "List".into(),
                got: format!("{:?}", val),
            }),
        };

        let avg = if count > 0 { total / count as f64 } else { 0.0 };
        self.stack.push(Value::Number(avg));
        self.last_exit_code = 0;
        Ok(())
    }

    fn builtin_min(&mut self) -> Result<(), EvalError> {
        let val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("min requires a list".into()))?;

        let result = match val {
            Value::List(items) => {
                items.iter().filter_map(|v| match v {
                    Value::Number(n) => Some(*n),
                    Value::Literal(s) | Value::Output(s) => s.trim().parse().ok(),
                    _ => None,
                }).fold(f64::INFINITY, f64::min)
            }
            _ => return Err(EvalError::TypeError {
                expected: "List".into(),
                got: format!("{:?}", val),
            }),
        };

        if result.is_infinite() {
            self.stack.push(Value::Nil);
        } else {
            self.stack.push(Value::Number(result));
        }
        self.last_exit_code = 0;
        Ok(())
    }

    fn builtin_max(&mut self) -> Result<(), EvalError> {
        let val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("max requires a list".into()))?;

        let result = match val {
            Value::List(items) => {
                items.iter().filter_map(|v| match v {
                    Value::Number(n) => Some(*n),
                    Value::Literal(s) | Value::Output(s) => s.trim().parse().ok(),
                    _ => None,
                }).fold(f64::NEG_INFINITY, f64::max)
            }
            _ => return Err(EvalError::TypeError {
                expected: "List".into(),
                got: format!("{:?}", val),
            }),
        };

        if result.is_infinite() {
            self.stack.push(Value::Nil);
        } else {
            self.stack.push(Value::Number(result));
        }
        self.last_exit_code = 0;
        Ok(())
    }

    fn builtin_count(&mut self) -> Result<(), EvalError> {
        let val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("count requires a value".into()))?;

        let n = match &val {
            Value::List(items) => items.len(),
            Value::Table { rows, .. } => rows.len(),
            Value::Literal(s) | Value::Output(s) => s.lines().count(),
            Value::Map(m) => m.len(),
            _ => 1,
        };

        self.stack.push(Value::Number(n as f64));
        self.last_exit_code = 0;
        Ok(())
    }

    // ========================================
    // Phase 8: Extended table operations
    // ========================================

    fn builtin_group_by(&mut self) -> Result<(), EvalError> {
        let col = self.pop_string()?;
        let table = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("group-by requires a table".into()))?;

        match table {
            Value::Table { columns, rows } => {
                let col_idx = columns.iter().position(|c| c == &col)
                    .ok_or_else(|| EvalError::ExecError(
                        format!("group-by: column '{}' not found", col)
                    ))?;

                let mut groups: HashMap<String, Vec<Vec<Value>>> = HashMap::new();

                for row in rows {
                    let key = row.get(col_idx)
                        .and_then(|v| v.as_arg())
                        .unwrap_or_default();
                    groups.entry(key).or_default().push(row);
                }

                // Convert groups to Record of Tables
                let map: HashMap<String, Value> = groups.into_iter()
                    .map(|(k, rows)| {
                        (k, Value::Table { columns: columns.clone(), rows })
                    })
                    .collect();

                self.stack.push(Value::Map(map));
            }
            _ => return Err(EvalError::TypeError {
                expected: "Table".into(),
                got: format!("{:?}", table),
            }),
        }

        self.last_exit_code = 0;
        Ok(())
    }

    fn builtin_unique(&mut self) -> Result<(), EvalError> {
        use std::collections::HashSet;

        let val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("unique requires a value".into()))?;

        match val {
            Value::List(items) => {
                let mut seen = HashSet::new();
                let unique: Vec<Value> = items.into_iter()
                    .filter(|v| {
                        let key = v.as_arg().unwrap_or_default();
                        seen.insert(key)
                    })
                    .collect();
                self.stack.push(Value::List(unique));
            }
            Value::Table { columns, rows } => {
                let mut seen = HashSet::new();
                let unique: Vec<Vec<Value>> = rows.into_iter()
                    .filter(|row| {
                        let key: String = row.iter()
                            .filter_map(|v| v.as_arg())
                            .collect::<Vec<_>>()
                            .join("\t");
                        seen.insert(key)
                    })
                    .collect();
                self.stack.push(Value::Table { columns, rows: unique });
            }
            _ => self.stack.push(val),
        }

        self.last_exit_code = 0;
        Ok(())
    }

    fn builtin_reverse(&mut self) -> Result<(), EvalError> {
        let val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("reverse requires a value".into()))?;

        match val {
            Value::List(mut items) => {
                items.reverse();
                self.stack.push(Value::List(items));
            }
            Value::Table { columns, mut rows } => {
                rows.reverse();
                self.stack.push(Value::Table { columns, rows });
            }
            Value::Literal(s) => {
                self.stack.push(Value::Literal(s.chars().rev().collect()));
            }
            Value::Output(s) => {
                self.stack.push(Value::Output(s.chars().rev().collect()));
            }
            _ => self.stack.push(val),
        }

        self.last_exit_code = 0;
        Ok(())
    }

    fn builtin_flatten(&mut self) -> Result<(), EvalError> {
        let val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("flatten requires a list".into()))?;

        match val {
            Value::List(items) => {
                let mut flattened = Vec::new();
                for item in items {
                    match item {
                        Value::List(inner) => flattened.extend(inner),
                        other => flattened.push(other),
                    }
                }
                self.stack.push(Value::List(flattened));
            }
            _ => self.stack.push(val),
        }

        self.last_exit_code = 0;
        Ok(())
    }

    // ========================================
    // Phase 11: Additional parsers
    // ========================================

    fn builtin_into_tsv(&mut self) -> Result<(), EvalError> {
        let text = self.pop_string()?;
        self.parse_delimited_text(&text, "\t")
    }

    fn builtin_into_delimited(&mut self) -> Result<(), EvalError> {
        let delim = self.pop_string()?;
        let text = self.pop_string()?;
        self.parse_delimited_text(&text, &delim)
    }

    fn parse_delimited_text(&mut self, text: &str, delim: &str) -> Result<(), EvalError> {
        let lines: Vec<&str> = text.lines().collect();
        if lines.is_empty() {
            self.stack.push(Value::Table { columns: vec![], rows: vec![] });
            return Ok(());
        }

        let columns: Vec<String> = lines[0].split(delim)
            .map(|s| s.trim().to_string())
            .collect();

        let rows: Vec<Vec<Value>> = lines[1..].iter()
            .filter(|l| !l.trim().is_empty())
            .map(|line| {
                line.split(delim)
                    .map(|s| Value::Literal(s.trim().to_string()))
                    .collect()
            })
            .collect();

        self.stack.push(Value::Table { columns, rows });
        self.last_exit_code = 0;
        Ok(())
    }

    // =====================================
    // Structured Data Builtins
    // =====================================

    /// ls-table: List directory contents as a structured table
    /// Optionally takes a path from the stack, defaults to current directory
    fn builtin_ls_table(&mut self) -> Result<(), EvalError> {
        use std::fs;
        use std::os::unix::fs::MetadataExt;

        // Check if there's a path argument on the stack
        let dir_path = if let Some(val) = self.stack.last() {
            if let Some(s) = val.as_arg() {
                if !s.starts_with('-') && (Path::new(&s).exists() || s.contains('/')) {
                    self.stack.pop();
                    PathBuf::from(self.expand_tilde(&s))
                } else {
                    self.cwd.clone()
                }
            } else {
                self.cwd.clone()
            }
        } else {
            self.cwd.clone()
        };

        let entries = fs::read_dir(&dir_path).map_err(|e| {
            EvalError::IoError(std::io::Error::new(e.kind(), format!("{}: {}", dir_path.display(), e)))
        })?;

        let columns = vec![
            "name".to_string(),
            "type".to_string(),
            "size".to_string(),
            "modified".to_string(),
        ];

        let mut rows: Vec<Vec<Value>> = Vec::new();

        for entry in entries {
            if let Ok(entry) = entry {
                let name = entry.file_name().to_string_lossy().to_string();
                let metadata = entry.metadata();

                let (file_type, size, modified) = if let Ok(meta) = &metadata {
                    let ft = if meta.is_dir() { "dir" } else if meta.is_file() { "file" } else { "other" };
                    let sz = meta.len();
                    let mod_time = meta.mtime();
                    (ft.to_string(), sz, mod_time)
                } else {
                    ("unknown".to_string(), 0, 0)
                };

                rows.push(vec![
                    Value::Literal(name),
                    Value::Literal(file_type),
                    Value::Number(size as f64),
                    Value::Number(modified as f64),
                ]);
            }
        }

        // Sort by name
        rows.sort_by(|a, b| {
            let name_a = a.first().and_then(|v| v.as_arg()).unwrap_or_default();
            let name_b = b.first().and_then(|v| v.as_arg()).unwrap_or_default();
            name_a.cmp(&name_b)
        });

        self.stack.push(Value::Table { columns, rows });
        self.last_exit_code = 0;
        Ok(())
    }

    /// open: Open a file and parse it based on extension
    /// Supports: .json, .csv, .tsv, plain text
    fn builtin_open(&mut self) -> Result<(), EvalError> {
        use std::fs;

        let path_val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("open requires file path".into()))?;
        let path_str = path_val.as_arg().ok_or_else(||
            EvalError::TypeError { expected: "String".into(), got: format!("{:?}", path_val) })?;
        let path = PathBuf::from(self.expand_tilde(&path_str));

        let content = fs::read_to_string(&path).map_err(|e| {
            EvalError::IoError(std::io::Error::new(e.kind(), format!("{}: {}", path.display(), e)))
        })?;

        // Determine format based on extension
        let ext = path.extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_default();

        match ext.as_str() {
            "json" => {
                // Parse as JSON
                self.stack.push(Value::Literal(content));
                self.json_parse()?;
            }
            "csv" => {
                // Parse as CSV
                self.stack.push(Value::Literal(content));
                self.builtin_into_csv()?;
            }
            "tsv" => {
                // Parse as TSV
                self.stack.push(Value::Literal(content));
                self.builtin_into_tsv()?;
            }
            _ => {
                // Plain text - just push as output
                self.stack.push(Value::Output(content));
            }
        }

        self.last_exit_code = 0;
        Ok(())
    }

    // =====================================
    // Path Operations
    // =====================================

    /// reext: Replace extension
    /// path newext reext -> path with new extension
    /// "file.txt" ".md" reext -> "file.md"
    fn builtin_reext(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.len() < 2 {
            return Err(EvalError::ExecError("reext: path and new extension required".into()));
        }
        self.restore_excess_args(args, 2);
        // Args in LIFO: [newext, path] for "path newext reext"
        let new_ext = &args[0];
        let path_str = &args[1];

        // Split at last dot, replace extension
        let result = if let Some(dot_pos) = path_str.rfind('.') {
            format!("{}{}", &path_str[..dot_pos], new_ext)
        } else {
            // No extension, just append the new one
            format!("{}{}", path_str, new_ext)
        };

        self.stack.push(Value::Literal(result));
        self.last_exit_code = 0;
        Ok(())
    }

    // =====================================
    // Additional Serialization Operations
    // =====================================

    /// to-tsv: Convert table to TSV string format
    fn builtin_to_tsv(&mut self) -> Result<(), EvalError> {
        let val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("to-tsv requires table".into()))?;

        match val {
            Value::Table { columns, rows } => {
                let mut lines = vec![columns.join("\t")];
                for row in rows {
                    let line: Vec<String> = row.iter()
                        .map(|v| v.as_arg().unwrap_or_default())
                        .collect();
                    lines.push(line.join("\t"));
                }
                self.stack.push(Value::Output(lines.join("\n")));
            }
            _ => return Err(EvalError::TypeError {
                expected: "Table".into(),
                got: format!("{:?}", val),
            }),
        }

        self.last_exit_code = 0;
        Ok(())
    }

    /// to-delimited: Convert table to custom-delimited string format
    /// table delimiter to-delimited -> delimited string
    fn builtin_to_delimited(&mut self) -> Result<(), EvalError> {
        let delim_val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("to-delimited requires delimiter".into()))?;
        let table_val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("to-delimited requires table".into()))?;

        let delim = delim_val.as_arg().unwrap_or_else(|| ",".to_string());

        match table_val {
            Value::Table { columns, rows } => {
                let mut lines = vec![columns.join(&delim)];
                for row in rows {
                    let line: Vec<String> = row.iter()
                        .map(|v| v.as_arg().unwrap_or_default())
                        .collect();
                    lines.push(line.join(&delim));
                }
                self.stack.push(Value::Output(lines.join("\n")));
            }
            _ => return Err(EvalError::TypeError {
                expected: "Table".into(),
                got: format!("{:?}", table_val),
            }),
        }

        self.last_exit_code = 0;
        Ok(())
    }

    /// save: Write data to file, auto-formatting based on extension
    /// data "path.json" save -> writes JSON
    /// data "path.csv" save -> writes CSV
    fn builtin_save(&mut self) -> Result<(), EvalError> {
        use std::fs;

        let path_val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("save requires file path".into()))?;
        let data_val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("save requires data".into()))?;

        let path_str = path_val.as_arg().ok_or_else(||
            EvalError::TypeError { expected: "String".into(), got: format!("{:?}", path_val) })?;
        let path = PathBuf::from(self.expand_tilde(&path_str));

        // Determine format based on extension
        let ext = path.extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_default();

        let content = match ext.as_str() {
            "json" => {
                // Convert to JSON
                let json = crate::ast::value_to_json(&data_val);
                serde_json::to_string_pretty(&json)
                    .unwrap_or_else(|_| data_val.as_arg().unwrap_or_default())
            }
            "csv" => {
                // Convert to CSV
                match &data_val {
                    Value::Table { columns, rows } => {
                        let mut lines = vec![columns.join(",")];
                        for row in rows {
                            let line: Vec<String> = row.iter()
                                .map(|v| v.as_arg().unwrap_or_default())
                                .collect();
                            lines.push(line.join(","));
                        }
                        lines.join("\n")
                    }
                    _ => data_val.as_arg().unwrap_or_default(),
                }
            }
            "tsv" => {
                // Convert to TSV
                match &data_val {
                    Value::Table { columns, rows } => {
                        let mut lines = vec![columns.join("\t")];
                        for row in rows {
                            let line: Vec<String> = row.iter()
                                .map(|v| v.as_arg().unwrap_or_default())
                                .collect();
                            lines.push(line.join("\t"));
                        }
                        lines.join("\n")
                    }
                    _ => data_val.as_arg().unwrap_or_default(),
                }
            }
            _ => {
                // Plain text
                data_val.as_arg().unwrap_or_default()
            }
        };

        fs::write(&path, content).map_err(|e| {
            EvalError::IoError(std::io::Error::new(e.kind(), format!("{}: {}", path.display(), e)))
        })?;

        self.last_exit_code = 0;
        Ok(())
    }

    // =====================================
    // Additional Aggregation Operations
    // =====================================

    /// reduce: Aggregate list to single value using a block
    /// list init [block] reduce -> result
    /// The block receives (accumulator, current-item) and should return new accumulator
    fn builtin_reduce(&mut self) -> Result<(), EvalError> {
        let block = self.pop_block()?;
        let init = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("reduce requires initial value".into()))?;
        let list = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("reduce requires list".into()))?;

        let items = match list {
            Value::List(items) => items,
            _ => return Err(EvalError::TypeError {
                expected: "List".into(),
                got: format!("{:?}", list),
            }),
        };

        let mut acc = init;
        for item in items {
            // Push accumulator and current item
            self.stack.push(acc);
            self.stack.push(item);
            // Execute the block
            for expr in &block {
                self.eval_expr(expr)?;
            }
            // Pop the result as new accumulator
            acc = self.stack.pop().ok_or_else(||
                EvalError::StackUnderflow("reduce block must return a value".into()))?;
        }

        self.stack.push(acc);
        self.last_exit_code = 0;
        Ok(())
    }

    // =====================================
    // Additional List/Table Operations
    // =====================================

    /// reject: Inverse of keep - removes items matching predicate
    /// list [predicate] reject -> filtered list (items where predicate is false)
    fn builtin_reject(&mut self) -> Result<(), EvalError> {
        let block = self.pop_block()?;
        let list = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("reject requires list".into()))?;

        let items = match list {
            Value::List(items) => items,
            _ => return Err(EvalError::TypeError {
                expected: "List".into(),
                got: format!("{:?}", list),
            }),
        };

        let mut kept = Vec::new();
        for item in items {
            // Push item and execute predicate
            self.stack.push(item.clone());
            for expr in &block {
                self.eval_expr(expr)?;
            }
            // Keep if predicate FAILS (exit code != 0)
            if self.last_exit_code != 0 {
                kept.push(item);
            }
        }

        self.stack.push(Value::List(kept));
        self.last_exit_code = 0;
        Ok(())
    }

    /// reject-where: Inverse of where - removes rows matching predicate from tables
    /// table [predicate] reject-where -> filtered table
    fn builtin_reject_where(&mut self) -> Result<(), EvalError> {
        let block = self.pop_block()?;
        let table = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("reject-where requires table".into()))?;

        let (columns, rows) = match table {
            Value::Table { columns, rows } => (columns, rows),
            _ => return Err(EvalError::TypeError {
                expected: "Table".into(),
                got: format!("{:?}", table),
            }),
        };

        let mut kept_rows = Vec::new();
        for row in rows {
            // Create a record for this row
            let mut map = std::collections::HashMap::new();
            for (i, col) in columns.iter().enumerate() {
                if let Some(val) = row.get(i) {
                    map.insert(col.clone(), val.clone());
                }
            }
            let record = Value::Map(map);

            // Push record and execute predicate
            self.stack.push(record);
            for expr in &block {
                self.eval_expr(expr)?;
            }

            // Keep if predicate FAILS (exit code != 0)
            if self.last_exit_code != 0 {
                kept_rows.push(row);
            }
        }

        self.stack.push(Value::Table { columns, rows: kept_rows });
        self.last_exit_code = 0;
        Ok(())
    }

    /// duplicates: Return only items that appear more than once (supplementary to unique)
    /// list duplicates -> list of duplicate items
    fn builtin_duplicates(&mut self) -> Result<(), EvalError> {
        let val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("duplicates requires list".into()))?;

        let items = match val {
            Value::List(items) => items,
            _ => return Err(EvalError::TypeError {
                expected: "List".into(),
                got: format!("{:?}", val),
            }),
        };

        // Count occurrences
        let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for item in &items {
            let key = item.as_arg().unwrap_or_default();
            *counts.entry(key).or_insert(0) += 1;
        }

        // Keep only items that appear more than once
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        let duplicates: Vec<Value> = items.into_iter()
            .filter(|item| {
                let key = item.as_arg().unwrap_or_default();
                if counts.get(&key).copied().unwrap_or(0) > 1 && !seen.contains(&key) {
                    seen.insert(key);
                    true
                } else {
                    false
                }
            })
            .collect();

        self.stack.push(Value::List(duplicates));
        self.last_exit_code = 0;
        Ok(())
    }

    // =====================================
    // Vector Operations (for Embeddings)
    // =====================================

    /// Helper: Pop a numeric list from the stack
    fn pop_number_list(&mut self) -> Result<Vec<f64>, EvalError> {
        let val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("vector operation requires list".into()))?;

        match val {
            Value::List(items) => {
                items.iter()
                    .map(|v| match v {
                        Value::Number(n) => Ok(*n),
                        Value::Literal(s) | Value::Output(s) => {
                            s.trim().parse::<f64>().map_err(|_|
                                EvalError::TypeError {
                                    expected: "Number".into(),
                                    got: format!("'{}'", s),
                                })
                        }
                        _ => Err(EvalError::TypeError {
                            expected: "Number".into(),
                            got: format!("{:?}", v),
                        }),
                    })
                    .collect()
            }
            _ => Err(EvalError::TypeError {
                expected: "List".into(),
                got: format!("{:?}", val),
            }),
        }
    }

    /// dot-product: Compute dot product of two vectors
    /// vec1 vec2 dot-product -> scalar
    fn builtin_dot_product(&mut self) -> Result<(), EvalError> {
        let vec2 = self.pop_number_list()?;
        let vec1 = self.pop_number_list()?;

        if vec1.len() != vec2.len() {
            return Err(EvalError::ExecError(format!(
                "dot-product: vectors must have same length ({} vs {})",
                vec1.len(), vec2.len()
            )));
        }

        let result: f64 = vec1.iter().zip(vec2.iter())
            .map(|(a, b)| a * b)
            .sum();

        self.stack.push(Value::Number(result));
        self.last_exit_code = 0;
        Ok(())
    }

    /// magnitude: Compute L2 norm (magnitude) of a vector
    /// vec magnitude -> scalar
    fn builtin_magnitude(&mut self) -> Result<(), EvalError> {
        let vec = self.pop_number_list()?;

        let sum_sq: f64 = vec.iter().map(|x| x * x).sum();
        let result = sum_sq.sqrt();

        self.stack.push(Value::Number(result));
        self.last_exit_code = 0;
        Ok(())
    }

    /// normalize: Convert vector to unit vector
    /// vec normalize -> unit vector
    fn builtin_normalize(&mut self) -> Result<(), EvalError> {
        let vec = self.pop_number_list()?;

        let sum_sq: f64 = vec.iter().map(|x| x * x).sum();
        let mag = sum_sq.sqrt();

        let result: Vec<Value> = if mag == 0.0 {
            vec.iter().map(|_| Value::Number(0.0)).collect()
        } else {
            vec.iter().map(|x| Value::Number(x / mag)).collect()
        };

        self.stack.push(Value::List(result));
        self.last_exit_code = 0;
        Ok(())
    }

    /// cosine-similarity: Compute cosine similarity between two vectors
    /// vec1 vec2 cosine-similarity -> scalar (-1 to 1)
    fn builtin_cosine_similarity(&mut self) -> Result<(), EvalError> {
        let vec2 = self.pop_number_list()?;
        let vec1 = self.pop_number_list()?;

        if vec1.len() != vec2.len() {
            return Err(EvalError::ExecError(format!(
                "cosine-similarity: vectors must have same length ({} vs {})",
                vec1.len(), vec2.len()
            )));
        }

        let dot: f64 = vec1.iter().zip(vec2.iter())
            .map(|(a, b)| a * b)
            .sum();
        let mag1: f64 = vec1.iter().map(|x| x * x).sum::<f64>().sqrt();
        let mag2: f64 = vec2.iter().map(|x| x * x).sum::<f64>().sqrt();

        let result = if mag1 == 0.0 || mag2 == 0.0 {
            0.0
        } else {
            dot / (mag1 * mag2)
        };

        self.stack.push(Value::Number(result));
        self.last_exit_code = 0;
        Ok(())
    }

    /// euclidean-distance: Compute Euclidean distance between two vectors
    /// vec1 vec2 euclidean-distance -> scalar
    fn builtin_euclidean_distance(&mut self) -> Result<(), EvalError> {
        let vec2 = self.pop_number_list()?;
        let vec1 = self.pop_number_list()?;

        if vec1.len() != vec2.len() {
            return Err(EvalError::ExecError(format!(
                "euclidean-distance: vectors must have same length ({} vs {})",
                vec1.len(), vec2.len()
            )));
        }

        let sum_sq: f64 = vec1.iter().zip(vec2.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum();
        let result = sum_sq.sqrt();

        self.stack.push(Value::Number(result));
        self.last_exit_code = 0;
        Ok(())
    }

    // =====================================
    // Plugin System Methods
    // =====================================

    /// Try to execute a plugin command (returns true if handled)
    fn try_plugin_command_if_enabled(&mut self, cmd: &str) -> Result<bool, EvalError> {
        #[cfg(feature = "plugins")]
        {
            // First, check if this command is provided by a plugin
            let has_cmd = self.plugin_host.as_ref().map(|h| h.has_command(cmd)).unwrap_or(false);

            if !has_cmd {
                // Check for hot reloads even if this isn't a plugin command
                if let Some(ref mut host) = self.plugin_host {
                    if let Ok(reloaded) = host.check_hot_reload() {
                        for name in &reloaded {
                            eprintln!("Plugin reloaded: {}", name);
                        }
                    }
                }
                return Ok(false);
            }

            // Sync stack to shared stack before calling plugin
            self.sync_stack_to_plugins();

            // Collect args from stack (for passing as JSON)
            let mut args = Vec::new();
            while let Some(value) = self.stack.last() {
                match value {
                    Value::Block(_) | Value::Marker | Value::Nil => break,
                    _ => {
                        if let Some(arg) = value.as_arg() {
                            args.push(arg);
                        }
                        self.stack.pop();
                    }
                }
            }
            args.reverse(); // LIFO -> correct order

            // Call the plugin command
            if let Some(ref mut host) = self.plugin_host {
                match host.call(cmd, &args) {
                    Ok(exit_code) => {
                        // Sync stack back from plugins
                        self.sync_stack_from_plugins();
                        self.last_exit_code = exit_code;
                        return Ok(true);
                    }
                    Err(e) => {
                        return Err(EvalError::ExecError(format!("Plugin error: {}", e)));
                    }
                }
            }
        }

        Ok(false)
    }

    /// Sync the evaluator's stack to the shared plugin stack
    #[cfg(feature = "plugins")]
    fn sync_stack_to_plugins(&self) {
        if let Ok(mut shared) = self.shared_stack.lock() {
            shared.clear();
            shared.extend(self.stack.clone());
        }
    }

    /// Sync the shared plugin stack back to the evaluator
    #[cfg(feature = "plugins")]
    fn sync_stack_from_plugins(&mut self) {
        if let Ok(shared) = self.shared_stack.lock() {
            self.stack = shared.clone();
        }
    }

    // =====================================
    // Plugin Management Builtins
    // =====================================

    /// Load a plugin: "path/to/plugin" plugin-load
    #[cfg(feature = "plugins")]
    fn builtin_plugin_load(&mut self, args: &[String]) -> Result<(), EvalError> {
        let path = args.first().ok_or_else(|| {
            EvalError::ExecError("plugin-load requires a path argument".to_string())
        })?;

        let path = self.expand_tilde(path);
        let plugin_path = std::path::Path::new(&path);

        if let Some(ref mut host) = self.plugin_host {
            host.load_plugin(plugin_path).map_err(|e| {
                EvalError::ExecError(format!("Failed to load plugin: {}", e))
            })?;
            self.last_exit_code = 0;
        } else {
            return Err(EvalError::ExecError("Plugin system not initialized".to_string()));
        }

        Ok(())
    }

    /// Unload a plugin: "plugin-name" plugin-unload
    #[cfg(feature = "plugins")]
    fn builtin_plugin_unload(&mut self, args: &[String]) -> Result<(), EvalError> {
        let name = args.first().ok_or_else(|| {
            EvalError::ExecError("plugin-unload requires a plugin name".to_string())
        })?;

        if let Some(ref mut host) = self.plugin_host {
            host.unload_plugin(name).map_err(|e| {
                EvalError::ExecError(format!("Failed to unload plugin: {}", e))
            })?;
            self.last_exit_code = 0;
        } else {
            return Err(EvalError::ExecError("Plugin system not initialized".to_string()));
        }

        Ok(())
    }

    /// Force reload a plugin: "plugin-name" plugin-reload
    #[cfg(feature = "plugins")]
    fn builtin_plugin_reload(&mut self, args: &[String]) -> Result<(), EvalError> {
        let name = args.first().ok_or_else(|| {
            EvalError::ExecError("plugin-reload requires a plugin name".to_string())
        })?;

        if let Some(ref mut host) = self.plugin_host {
            host.reload_plugin(name).map_err(|e| {
                EvalError::ExecError(format!("Failed to reload plugin: {}", e))
            })?;
            println!("Plugin reloaded: {}", name);
            self.last_exit_code = 0;
        } else {
            return Err(EvalError::ExecError("Plugin system not initialized".to_string()));
        }

        Ok(())
    }

    /// List all loaded plugins
    #[cfg(feature = "plugins")]
    fn builtin_plugin_list(&mut self) -> Result<(), EvalError> {
        if let Some(ref host) = self.plugin_host {
            let plugins = host.list_plugins();
            if plugins.is_empty() {
                println!("No plugins loaded");
                println!("Plugin directory: {}", host.plugin_dir().display());
            } else {
                println!("Loaded plugins:");
                for info in plugins {
                    println!("  {} v{} - {}", info.name, info.version, info.description);
                    println!("    Commands: {}", info.commands.join(", "));
                    println!("    Path: {}", info.path.display());
                }
            }
            self.last_exit_code = 0;
        } else {
            println!("Plugin system not initialized");
            self.last_exit_code = 1;
        }

        Ok(())
    }

    /// Show details about a specific plugin: "plugin-name" plugin-info
    #[cfg(feature = "plugins")]
    fn builtin_plugin_info(&mut self, args: &[String]) -> Result<(), EvalError> {
        let name = args.first().ok_or_else(|| {
            EvalError::ExecError("plugin-info requires a plugin name".to_string())
        })?;

        if let Some(ref host) = self.plugin_host {
            if let Some(info) = host.get_plugin_info(name) {
                println!("Plugin: {}", info.name);
                println!("Version: {}", info.version);
                println!("Description: {}", info.description);
                println!("Commands: {}", info.commands.join(", "));
                println!("Path: {}", info.path.display());
                self.last_exit_code = 0;
            } else {
                println!("Plugin not found: {}", name);
                self.last_exit_code = 1;
            }
        } else {
            return Err(EvalError::ExecError("Plugin system not initialized".to_string()));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;
    use crate::parser::parse;

    fn eval_str(input: &str) -> Result<EvalResult, EvalError> {
        let tokens = lex(input).expect("lex failed");
        let program = parse(tokens).expect("parse failed");
        let mut eval = Evaluator::new();
        eval.eval(&program)
    }

    #[test]
    fn eval_literal() {
        let result = eval_str("hello world").unwrap();
        assert_eq!(result.output, "hello\nworld");
    }

    #[test]
    fn eval_command() {
        let result = eval_str("hello echo").unwrap();
        assert!(result.output.contains("hello"));
    }

    #[test]
    fn eval_command_substitution() {
        let result = eval_str("pwd ls").unwrap();
        // ls $(pwd) should list current directory
        assert!(result.exit_code == 0);
    }

    #[test]
    fn eval_stack_dup() {
        let result = eval_str("hello dup").unwrap();
        assert_eq!(result.stack.len(), 2);
    }

    #[test]
    fn eval_stack_swap() {
        let result = eval_str("a b swap").unwrap();
        assert_eq!(result.output, "b\na");
    }

    #[test]
    fn eval_path_join() {
        let result = eval_str("/path file.txt path-join").unwrap();
        assert_eq!(result.output, "/path/file.txt");
    }

    #[test]
    fn eval_string_split1() {
        let result = eval_str("\"a.b.c\" \".\" split1").unwrap();
        assert_eq!(result.output, "a\nb.c");
    }

    #[test]
    fn eval_string_rsplit1() {
        let result = eval_str("\"a.b.c\" \".\" rsplit1").unwrap();
        assert_eq!(result.output, "a.b\nc");
    }

    #[test]
    fn eval_define_and_use() {
        // Define a word, then use it
        let tokens = lex("[dup swap] :test").expect("lex");
        let program = parse(tokens).expect("parse");
        let mut eval = Evaluator::new();
        eval.eval(&program).expect("eval define");

        // Now use the defined word
        let tokens2 = lex("a b test").expect("lex");
        let program2 = parse(tokens2).expect("parse");
        let result = eval.eval(&program2).expect("eval use");

        assert_eq!(result.output, "a\nb\nb");
    }

    #[test]
    fn eval_variable_expansion() {
        std::env::set_var("HSAB_TEST_VAR", "test_value");
        let result = eval_str("$HSAB_TEST_VAR echo").unwrap();
        assert!(result.output.contains("test_value"));
        std::env::remove_var("HSAB_TEST_VAR");
    }

    #[test]
    fn eval_builtin_true_false() {
        let result = eval_str("true").unwrap();
        assert_eq!(result.exit_code, 0);

        let result = eval_str("false").unwrap();
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn eval_builtin_test() {
        // Test file existence
        let result = eval_str("Cargo.toml -f test").unwrap();
        assert_eq!(result.exit_code, 0);

        // Test string comparison
        let result = eval_str("a a = test").unwrap();
        assert_eq!(result.exit_code, 0);

        let result = eval_str("a b = test").unwrap();
        assert_eq!(result.exit_code, 1);
    }

    // === Debugger tests ===

    #[test]
    fn test_debugger_mode_toggle() {
        let mut eval = Evaluator::new();
        assert!(!eval.is_debug_mode());

        eval.set_debug_mode(true);
        assert!(eval.is_debug_mode());

        eval.set_debug_mode(false);
        assert!(!eval.is_debug_mode());
    }

    #[test]
    fn test_debugger_step_mode() {
        let mut eval = Evaluator::new();
        assert!(!eval.is_step_mode());

        eval.set_debug_mode(true);
        eval.set_step_mode(true);
        assert!(eval.is_step_mode());

        // Turning off debug mode should also turn off step mode
        eval.set_debug_mode(false);
        assert!(!eval.is_step_mode());
    }

    #[test]
    fn test_debugger_breakpoints() {
        let mut eval = Evaluator::new();

        // Add breakpoints
        eval.add_breakpoint("echo".to_string());
        eval.add_breakpoint("dup".to_string());
        assert_eq!(eval.breakpoints().len(), 2);

        // Remove a breakpoint
        assert!(eval.remove_breakpoint("echo"));
        assert_eq!(eval.breakpoints().len(), 1);

        // Clear all breakpoints
        eval.clear_breakpoints();
        assert!(eval.breakpoints().is_empty());
    }

    #[test]
    fn test_debugger_breakpoint_matching() {
        let mut eval = Evaluator::new();
        eval.add_breakpoint("echo".to_string());

        // Test matching
        let echo_expr = Expr::Literal("echo".to_string());
        let ls_expr = Expr::Literal("ls".to_string());

        assert!(eval.matches_breakpoint(&echo_expr));
        assert!(!eval.matches_breakpoint(&ls_expr));
    }

    #[test]
    fn test_debugger_expr_to_string() {
        let eval = Evaluator::new();

        // Test various expression types
        assert_eq!(eval.expr_to_string(&Expr::Literal("test".to_string())), "test");
        assert_eq!(eval.expr_to_string(&Expr::Dup), "dup");
        assert_eq!(eval.expr_to_string(&Expr::Swap), "swap");
        assert_eq!(eval.expr_to_string(&Expr::Pipe), "|");
        assert_eq!(eval.expr_to_string(&Expr::Apply), "@");
        assert_eq!(eval.expr_to_string(&Expr::If), "if");
    }

    #[test]
    fn test_debugger_format_state() {
        let mut eval = Evaluator::new();
        eval.stack.push(Value::Literal("test".to_string()));
        eval.stack.push(Value::Number(42.0));

        let expr = Expr::Literal("echo".to_string());
        let state = eval.format_debug_state(&expr);

        // Verify the debug state contains expected elements
        assert!(state.contains("echo"));
        assert!(state.contains("Stack (2 items)"));
        assert!(state.contains("\"test\""));
        assert!(state.contains("42"));
    }
}
