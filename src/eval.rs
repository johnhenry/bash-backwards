//! Evaluator for hsab v2 - Stack-based execution with native command execution
//!
//! The evaluator maintains a stack and executes expressions:
//! - Literals push themselves to the stack
//! - Executables pop args, run, push output
//! - Blocks are deferred execution units
//! - Operators manipulate the stack or control execution

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
use std::time::{Duration, Instant};
use thiserror::Error;

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
        }
    }

    /// Get a reference to the current stack (for debugging)
    pub fn stack(&self) -> &[Value] {
        &self.stack
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

    /// Expand glob patterns in a string
    fn expand_glob(&self, pattern: &str) -> Vec<String> {
        // Only expand if contains glob characters
        if !pattern.contains('*') && !pattern.contains('?') && !pattern.contains('[') {
            return vec![pattern.to_string()];
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
            // Look ahead to determine if output should be captured
            // Pass remaining expressions so we can look past blocks
            let remaining = &exprs[i + 1..];
            self.capture_mode = self.should_capture(remaining);

            match self.eval_expr(expr) {
                Ok(()) => {}
                Err(EvalError::BreakLoop) => return Err(EvalError::BreakOutsideLoop),
                Err(e) => return Err(e),
            }
        }
        Ok(())
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
                Expr::Join | Expr::Suffix | Expr::Split1 | Expr::Rsplit1 => true,

                // List operations (Marker just pushes, doesn't consume)
                Expr::Marker => false,
                Expr::Spread | Expr::Each | Expr::Keep | Expr::Collect => true,

                // Control flow consumes blocks/values
                Expr::If | Expr::Times | Expr::While | Expr::Until => true,

                // Parallel execution
                Expr::Parallel | Expr::Fork => true,

                // Process substitution
                Expr::Subst | Expr::Fifo => true,

                // JSON operations
                Expr::Json | Expr::Unjson => true,

                // Other operations
                Expr::Timeout | Expr::Pipestatus => true,
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
                    // Execute the defined word's body
                    for e in &body {
                        self.eval_expr(e)?;
                    }
                } else if s == "." && !self.stack.is_empty() {
                    // Special case: "." is source command only when there's something to source
                    // This allows "." alone to be treated as current directory literal,
                    // while "file.hsab ." works as source command
                    self.execute_command(".")?;
                } else if self.resolver.is_executable(s) {
                    // Check if it's an executable
                    self.execute_command(s)?;
                } else {
                    // Push as literal
                    self.stack.push(Value::Literal(s.clone()));
                }
            }

            Expr::Quoted { content, .. } => {
                // Push the content without surrounding quotes - quotes are just delimiters
                self.stack.push(Value::Literal(content.clone()));
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

            // String operations
            Expr::Split1 => self.string_split1()?,
            Expr::Rsplit1 => self.string_rsplit1()?,

            // List operations
            Expr::Marker => self.stack.push(Value::Marker),
            Expr::Spread => self.list_spread()?,
            Expr::Each => self.list_each()?,
            Expr::Collect => self.list_collect()?,
            Expr::Keep => self.list_keep()?,

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
            "bash" => Some(self.builtin_bash(args)),
            "bashsource" => Some(self.builtin_bashsource(args)),
            "which" => Some(self.builtin_which(args)),
            "source" | "." => Some(self.builtin_source(args)),
            "hash" => Some(self.builtin_hash(args)),
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

    fn builtin_export(&mut self, args: &[String]) -> Result<(), EvalError> {
        for arg in args {
            if let Some((key, value)) = arg.split_once('=') {
                std::env::set_var(key, value);
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

    /// Run a bash command string (for complex bash constructs)
    /// Usage: "for i in 1 2 3; do echo $i; done" bash
    fn builtin_bash(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.is_empty() {
            return Err(EvalError::ExecError("bash: no command specified".into()));
        }

        // Join all args as the bash command
        let bash_cmd = args.join(" ");

        let output = Command::new("bash")
            .arg("-c")
            .arg(&bash_cmd)
            .current_dir(&self.cwd)
            .output()
            .map_err(|e| EvalError::ExecError(format!("bash: {}", e)))?;

        self.last_exit_code = output.status.code().unwrap_or(-1);

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        if !stdout.is_empty() {
            self.stack.push(Value::Output(stdout));
        }

        // Print stderr if any
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.is_empty() {
            eprint!("{}", stderr);
        }

        Ok(())
    }

    fn builtin_bashsource(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.is_empty() {
            return Err(EvalError::ExecError(
                "bashsource: no file specified".into(),
            ));
        }

        let file_path = self.expand_tilde(&args[0]);

        // Verify file exists
        if !Path::new(&file_path).exists() {
            return Err(EvalError::ExecError(format!(
                "bashsource: {}: No such file",
                file_path
            )));
        }

        // Determine shell based on file extension or default to zsh
        let shell = if file_path.ends_with(".bashrc") || file_path.ends_with(".bash_profile") {
            "bash"
        } else {
            "zsh"
        };

        // Source the file and output environment
        let source_cmd = format!("source {} && env", file_path);

        let output = Command::new(shell)
            .arg("-c")
            .arg(&source_cmd)
            .current_dir(&self.cwd)
            .output()
            .map_err(|e| EvalError::ExecError(format!("bashsource: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(EvalError::ExecError(format!(
                "bashsource: failed to source {}: {}",
                file_path, stderr
            )));
        }

        // Parse env output and import variables
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if let Some((key, value)) = line.split_once('=') {
                // Skip empty keys or internal shell variables
                if !key.is_empty() && !key.starts_with('_') {
                    std::env::set_var(key, value);
                }
            }
        }

        self.last_exit_code = 0;
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
                "cd" | "pwd"
                    | "echo"
                    | "true"
                    | "false"
                    | "test"
                    | "["
                    | "export"
                    | "unset"
                    | "env"
                    | "jobs"
                    | "fg"
                    | "bg"
                    | "exit"
                    | "tty"
                    | "bash"
                    | "bashsource"
                    | "which"
                    | "source"
                    | "."
                    | "hash"
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

    // ==================== CONTROL FLOW ====================

    /// If: [condition] [then] [else] if
    fn control_if(&mut self) -> Result<(), EvalError> {
        let else_block = self.pop_block()?;
        let then_block = self.pop_block()?;
        let cond_block = self.pop_block()?;

        // Save outer capture mode
        let outer_capture_mode = self.capture_mode;

        // Execute condition block - always capture since output is discarded
        self.stack.push(Value::Marker);
        self.capture_mode = true;
        for expr in &cond_block {
            self.eval_expr(expr)?;
        }
        // Clean up anything pushed during condition
        while let Some(v) = self.stack.pop() {
            if v.is_marker() {
                break;
            }
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
        let result = eval_str("/path file.txt join").unwrap();
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
}
