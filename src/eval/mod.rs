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

mod helpers;
mod stack;
mod path;
mod string;
mod list;
mod control;
mod process;
mod command;
mod shell;
mod structured;
mod serialization;
mod aggregation;
mod math;
mod vector;
mod combinators;
mod encoding;
mod bigint;
mod terminal;
mod image;
mod modules;
mod local;
mod plugin;
mod snapshot;
mod async_ops;
mod http;
mod watch;
mod shell_native;
mod tests;

use crate::ast::{Expr, Program, Value};
use crate::resolver::ExecutableResolver;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Child;
#[cfg(feature = "plugins")]
use std::sync::{Arc, Mutex};
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
pub(crate) struct Job {
    pub(crate) id: usize,
    pub(crate) pid: u32,
    pub(crate) pgid: u32,  // Process group ID for signal delivery
    pub(crate) command: String,
    #[allow(dead_code)]
    pub(crate) child: Option<Child>,
    pub(crate) status: JobStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum JobStatus {
    Running,
    #[allow(dead_code)]
    Stopped,
    Done(i32),
}

/// The evaluator maintains state and executes programs
pub struct Evaluator {
    /// The value stack
    pub(crate) stack: Vec<Value>,
    /// Executable resolver for detecting commands
    pub(crate) resolver: ExecutableResolver,
    /// Last exit code
    pub(crate) last_exit_code: i32,
    /// User-defined words (functions)
    pub(crate) definitions: HashMap<String, Vec<Expr>>,
    /// Current working directory
    pub(crate) cwd: PathBuf,
    /// Home directory for ~ expansion
    pub(crate) home_dir: String,
    /// Background jobs
    pub(crate) jobs: Vec<Job>,
    /// Next job ID
    pub(crate) next_job_id: usize,
    /// Exit codes from last pipeline
    pub(crate) pipestatus: Vec<i32>,
    /// Whether to capture command output (vs run interactively)
    /// True when output will be consumed by next command/operator
    pub(crate) capture_mode: bool,
    /// Directory stack for pushd/popd
    pub(crate) dir_stack: Vec<PathBuf>,
    /// Command aliases - maps name to expansion (block of expressions)
    pub(crate) aliases: HashMap<String, Vec<Expr>>,
    /// Signal traps (signal number -> block to execute)
    pub(crate) traps: HashMap<i32, Vec<Expr>>,
    /// Stack of local variable scopes (for nested definitions)
    /// Each scope maps var name -> original value (None if didn't exist)
    pub(crate) local_scopes: Vec<HashMap<String, Option<String>>>,
    /// Stack of structured local values (Lists, Tables, Maps, etc.)
    /// These are checked before env vars during variable expansion
    pub(crate) local_values: Vec<HashMap<String, Value>>,
    /// Flag to signal early return from a definition
    pub(crate) returning: bool,
    /// Trace mode - print stack after each operation
    pub(crate) trace_mode: bool,
    /// Debug mode - enable step debugger
    pub(crate) debug_mode: bool,
    /// Step mode - pause before each expression
    pub(crate) step_mode: bool,
    /// Breakpoints - expression patterns to pause on
    pub(crate) breakpoints: std::collections::HashSet<String>,
    /// Loaded modules (by canonical path) to prevent double-loading
    pub(crate) loaded_modules: std::collections::HashSet<PathBuf>,
    /// Current definition call depth (for recursion limit)
    pub(crate) call_depth: usize,
    /// Maximum recursion depth (default 10000, configurable via HSAB_MAX_RECURSION)
    pub(crate) max_call_depth: usize,
    /// Limbo storage for popped values awaiting resolution
    pub limbo: HashMap<String, Value>,
    /// Preview length for limbo references
    pub(crate) preview_len: usize,
    /// Named stack snapshots
    pub(crate) snapshots: HashMap<String, Vec<Value>>,
    /// Counter for auto-generated snapshot names
    pub(crate) snapshot_counter: u32,
    /// Counter for generating unique future IDs
    pub(crate) future_counter: u32,
    /// Handles to background threads for futures (for cleanup)
    pub(crate) future_handles: HashMap<String, std::thread::JoinHandle<()>>,
    /// Plugin host for WASM plugin support
    #[cfg(feature = "plugins")]
    pub(crate) plugin_host: Option<PluginHost>,
    /// Shared stack reference for plugins
    #[cfg(feature = "plugins")]
    pub(crate) shared_stack: Arc<Mutex<Vec<Value>>>,
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
            local_values: Vec::new(),
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
            limbo: HashMap::new(),
            preview_len: std::env::var("HSAB_PREVIEW_LEN")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(8),
            snapshots: HashMap::new(),
            snapshot_counter: 0,
            future_counter: 0,
            future_handles: HashMap::new(),
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
    pub(crate) fn matches_breakpoint(&self, expr: &Expr) -> bool {
        if self.breakpoints.is_empty() {
            return false;
        }
        let expr_str = self.expr_to_string(expr);
        self.breakpoints.iter().any(|bp| expr_str.contains(bp))
    }

    /// Convert an expression to a string for breakpoint matching
    pub(crate) fn expr_to_string(&self, expr: &Expr) -> String {
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
            Expr::Realpath => "path-resolve".to_string(),
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
            Expr::LimboRef(id) => format!("`{}`", id),
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
                    Value::Media { data, .. } => format!("<media:{}B>", data.len()),
                    Value::Link { url, .. } => format!("<link:{}>", if url.len() > 20 { &url[..20] } else { url }),
                    Value::Bytes(data) => format!("<bytes:{}B>", data.len()),
                    Value::BigInt(n) => {
                        let s = n.to_string();
                        if s.len() > 30 {
                            format!("<bigint:{}...>", &s[..27])
                        } else {
                            format!("<bigint:{}>", s)
                        }
                    }
                    Value::Future { id, state } => {
                        use crate::ast::FutureState;
                        let guard = state.lock().unwrap();
                        let status = match &*guard {
                            FutureState::Pending => "pending",
                            FutureState::Completed(_) => "completed",
                            FutureState::Failed(_) => "failed",
                            FutureState::Cancelled => "cancelled",
                        };
                        format!("Future<{}:{}>", status, id)
                    }
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

    /// Push a value to the stack (for REPL Ctrl+Alt+<- shortcut)
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

    /// Get the number of items in limbo (for prompt display)
    pub fn limbo_count(&self) -> usize {
        self.limbo.len()
    }

    /// Get the number of pending futures (for prompt display)
    pub fn futures_count(&self) -> usize {
        // Count all handles - they get cleaned up on await/cancel
        self.future_handles.len()
    }

    /// Clear limbo storage (called after eval completes or on cancel)
    pub fn clear_limbo(&mut self) {
        self.limbo.clear();
    }

    /// Format a limbo reference with type and preview annotations
    pub fn format_limbo_ref(&self, id: &str, value: &Value) -> String {
        let formatted = self.format_limbo_preview(value);
        format!("`&{}:{}`", id, formatted)
    }

    /// Format a value preview for limbo reference display
    fn format_limbo_preview(&self, value: &Value) -> String {
        match value {
            Value::Literal(s) | Value::Output(s) => {
                let len = s.chars().count();
                let preview: String = s.chars().take(self.preview_len).collect();
                if len > self.preview_len {
                    format!("string[{}]:\"{}...\"", len, preview)
                } else {
                    format!("string:\"{}\"", s)
                }
            }
            Value::Number(n) => {
                // Format nicely - no trailing .0 for integers
                if n.fract() == 0.0 && n.abs() < i64::MAX as f64 {
                    format!("i64:{}", *n as i64)
                } else {
                    format!("f64:{}", n)
                }
            }
            Value::Bool(b) => format!("bool:{}", b),
            Value::Map(m) => {
                let fields: Vec<_> = m.keys().take(3).cloned().collect();
                let suffix = if m.len() > 3 { ", ..." } else { "" };
                format!("record:{{{}{}}}", fields.join(", "), suffix)
            }
            Value::List(items) => {
                let preview: Vec<String> = items.iter()
                    .take(3)
                    .map(|v| self.format_value_inline(v))
                    .collect();
                let suffix = if items.len() > 3 { ", ..." } else { "" };
                format!("vector[{}]:[{}{}]", items.len(), preview.join(", "), suffix)
            }
            Value::Table { columns, rows } => {
                format!("table[{}x{}]", columns.len(), rows.len())
            }
            Value::Media { mime_type, width, height, .. } => {
                match (width, height) {
                    (Some(w), Some(h)) => format!("{}[{}x{}]", mime_type, w, h),
                    _ => mime_type.clone(),
                }
            }
            Value::Block(_) => "block:[...]".to_string(),
            Value::BigInt(n) => {
                let s = n.to_string();
                if s.len() > self.preview_len {
                    format!("bigint:{}...", &s[..self.preview_len])
                } else {
                    format!("bigint:{}", s)
                }
            }
            Value::Bytes(b) => format!("bytes[{}]", b.len()),
            Value::Nil => "nil".to_string(),
            Value::Marker => "marker".to_string(),
            Value::Link { url, .. } => {
                if url.len() > self.preview_len {
                    format!("link:\"{}...\"", &url[..self.preview_len])
                } else {
                    format!("link:\"{}\"", url)
                }
            }
            Value::Error { kind, .. } => format!("error:{}", kind),
            Value::Future { id, state } => {
                use crate::ast::FutureState;
                let guard = state.lock().unwrap();
                let status = match &*guard {
                    FutureState::Pending => "pending",
                    FutureState::Completed(_) => "completed",
                    FutureState::Failed(_) => "failed",
                    FutureState::Cancelled => "cancelled",
                };
                format!("future<{}:{}>", status, id)
            }
        }
    }

    /// Format a value inline for limbo preview (compact form)
    fn format_value_inline(&self, value: &Value) -> String {
        match value {
            Value::Literal(s) | Value::Output(s) => {
                if s.len() > 8 {
                    format!("\"{}...\"", &s[..5])
                } else {
                    format!("\"{}\"", s)
                }
            }
            Value::Number(n) => {
                if n.fract() == 0.0 && n.abs() < i64::MAX as f64 {
                    format!("{}", *n as i64)
                } else {
                    format!("{}", n)
                }
            }
            Value::Bool(b) => format!("{}", b),
            Value::Nil => "nil".to_string(),
            _ => "...".to_string(),
        }
    }

    /// Look up a variable, checking local_values first, then env vars
    /// Returns the value as a string for interpolation purposes
    pub(crate) fn lookup_var_as_string(&self, var_name: &str) -> Option<String> {
        // Check local_values first (most recent scope to oldest)
        for scope in self.local_values.iter().rev() {
            if let Some(value) = scope.get(var_name) {
                return value.as_arg();
            }
        }
        // Fall back to environment variables
        std::env::var(var_name).ok()
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
    pub(crate) fn eval_exprs(&mut self, exprs: &[Expr]) -> Result<(), EvalError> {
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
    pub(crate) fn print_trace(&self, expr: &Expr) {
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
                Value::Media { data, .. } => format!("<img:{}B>", data.len()),
                Value::Link { .. } => "<link>".to_string(),
                Value::Bytes(data) => format!("<bytes:{}B>", data.len()),
                Value::BigInt(n) => {
                    let s = n.to_string();
                    if s.len() > 15 {
                        format!("<bigint:{}...>", &s[..12])
                    } else {
                        s
                    }
                }
                Value::Future { id, .. } => format!("<future:{}>", id),
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
    pub(crate) fn should_capture(&mut self, remaining: &[Expr]) -> bool {
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
                Expr::Join | Expr::Suffix | Expr::Dirname | Expr::Basename | Expr::Realpath => true,
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

                // Quoted strings, variables, and limbo refs are just pushed, but look past them
                // to see if there's a consuming operation after
                Expr::Quoted { .. } => self.should_capture(&remaining[1..]),
                Expr::Variable(_) => self.should_capture(&remaining[1..]),
                Expr::LimboRef(_) => self.should_capture(&remaining[1..]),

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
    pub(crate) fn eval_expr(&mut self, expr: &Expr) -> Result<(), EvalError> {
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
                    self.local_values.push(HashMap::new());
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

                    // Restore local variables and clean up structured values
                    if let Some(scope) = self.local_scopes.pop() {
                        for (name, original) in scope {
                            match original {
                                Some(value) => std::env::set_var(&name, value),
                                None => std::env::remove_var(&name),
                            }
                        }
                    }
                    self.local_values.pop();
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
                } else if s == "paste-here" {
                    // Special literal: expands to clipboard contents (like $VAR but for clipboard)
                    let clipboard_value = self.query_clipboard()?;
                    self.stack.push(Value::Literal(clipboard_value));
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
                // Expand variable - check local_values first, then env vars
                let var_name = s
                    .trim_start_matches('$')
                    .trim_start_matches('{')
                    .trim_end_matches('}');

                // Check local_values first (most recent scope to oldest)
                let mut found = false;
                for scope in self.local_values.iter().rev() {
                    if let Some(value) = scope.get(var_name) {
                        self.stack.push(value.clone());
                        found = true;
                        break;
                    }
                }

                // Fall back to environment variables
                if !found {
                    match std::env::var(var_name) {
                        Ok(value) => self.stack.push(Value::Literal(value)),
                        Err(_) => self.stack.push(Value::Literal(String::new())),
                    }
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
            Expr::Realpath => self.path_realpath()?,

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

            Expr::LimboRef(id) => {
                // Extract just the ID (before any colon) - ID is source of truth
                // Type/preview annotations are purely cosmetic
                let clean_id = id.split(':').next().unwrap_or(id);
                match self.limbo.remove(clean_id) {
                    Some(value) => self.stack.push(value),
                    None => {
                        // ID not found (edited, typo, already used, or from another session)
                        // Push Nil instead of erroring - graceful degradation
                        self.stack.push(Value::Nil);
                    }
                }
            }
        }

        Ok(())
    }

    /// Evaluate a scoped block with temporary variable assignments
    /// Variables are set before body execution, then restored/unset after
    pub(crate) fn eval_scoped_block(
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
}
