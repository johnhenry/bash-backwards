//! Evaluator for hsab v2 - Stack-based execution
//!
//! The evaluator maintains a stack and executes expressions:
//! - Literals push themselves to the stack
//! - Executables pop args, run, push output
//! - Blocks are deferred execution units
//! - Operators manipulate the stack or control execution

use crate::ast::{Expr, Program, Value};
use crate::resolver::ExecutableResolver;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio, Child, ChildStdin, ChildStdout};
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

/// Persistent bash subprocess for command execution
struct BashProcess {
    #[allow(dead_code)]
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    marker_counter: u64,
}

impl BashProcess {
    fn new() -> std::io::Result<Self> {
        let mut child = Command::new("bash")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;

        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());

        Ok(BashProcess {
            child,
            stdin,
            stdout,
            marker_counter: 0,
        })
    }

    fn execute(&mut self, cmd: &str) -> std::io::Result<(String, i32)> {
        self.marker_counter += 1;
        let marker = format!("__HSAB_V2_{}__", self.marker_counter);

        writeln!(self.stdin, "{}", cmd)?;
        writeln!(self.stdin, "printf '\\n{}:%d\\n' $?", marker)?;
        self.stdin.flush()?;

        let mut output = String::new();
        let mut exit_code = 0;

        loop {
            let mut line = String::new();
            let bytes_read = self.stdout.read_line(&mut line)?;

            if bytes_read == 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "bash process ended",
                ));
            }

            if line.contains(&marker) {
                if let Some(code_str) = line.trim().strip_prefix(&format!("{}:", marker)) {
                    exit_code = code_str.parse().unwrap_or(-1);
                }
                break;
            }
            output.push_str(&line);
        }

        // Clean up trailing newlines from marker
        if output == "\n" {
            output.clear();
        } else if output.ends_with("\n\n") {
            output.pop();
        }

        Ok((output, exit_code))
    }
}

/// The evaluator maintains state and executes programs
pub struct Evaluator {
    /// The value stack
    stack: Vec<Value>,
    /// Executable resolver for detecting commands
    resolver: ExecutableResolver,
    /// Persistent bash process
    bash: Option<BashProcess>,
    /// Last exit code
    last_exit_code: i32,
    /// User-defined words (functions)
    definitions: HashMap<String, Vec<Expr>>,
}

impl Default for Evaluator {
    fn default() -> Self {
        Self::new()
    }
}

impl Evaluator {
    pub fn new() -> Self {
        Evaluator {
            stack: Vec::new(),
            resolver: ExecutableResolver::new(),
            bash: BashProcess::new().ok(),
            last_exit_code: 0,
            definitions: HashMap::new(),
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

    /// Extract leftover literals from the stack (values the user typed but weren't consumed)
    /// Returns them as a string and clears the entire stack.
    /// This ensures each REPL line is independent (like v1 behavior).
    pub fn take_leftovers(&mut self) -> String {
        let mut leftovers = Vec::new();

        for value in self.stack.drain(..) {
            if let Value::Literal(s) = value {
                // This is a user-typed value that wasn't consumed
                leftovers.push(s);
            }
            // Discard Output, Block, Nil - each line is independent
        }

        leftovers.join(" ")
    }

    /// Ensure bash process is running
    fn ensure_bash(&mut self) -> Result<&mut BashProcess, EvalError> {
        if self.bash.is_none() {
            self.bash = Some(BashProcess::new()?);
        }
        self.bash.as_mut().ok_or_else(|| {
            EvalError::ExecError("Failed to start bash".into())
        })
    }

    /// Evaluate a program
    pub fn eval(&mut self, program: &Program) -> Result<EvalResult, EvalError> {
        for expr in &program.expressions {
            self.eval_expr(expr)?;
        }

        // Collect output from stack
        let output = self.stack
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
                } else if self.resolver.is_executable(s) {
                    // Check if it's an executable
                    self.execute_command(s)?;
                } else {
                    // Push as literal
                    self.stack.push(Value::Literal(s.clone()));
                }
            }

            Expr::Quoted { content, double } => {
                let quoted = if *double {
                    format!("\"{}\"", content)
                } else {
                    format!("'{}'", content)
                };
                self.stack.push(Value::Literal(quoted));
            }

            Expr::Variable(s) => {
                // Variables are passed through to bash
                self.stack.push(Value::Literal(s.clone()));
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

            // Path operations
            Expr::Join => self.path_join()?,
            Expr::Basename => self.path_basename()?,
            Expr::Dirname => self.path_dirname()?,
            Expr::Suffix => self.path_suffix()?,
            Expr::Reext => self.path_reext()?,

            // List operations
            Expr::Spread => self.list_spread()?,
            Expr::Each => self.list_each()?,
            Expr::Collect => self.list_collect()?,
            Expr::Keep => self.list_keep()?,

            // Control flow
            Expr::If => self.control_if()?,
            Expr::Times => self.control_times()?,
            Expr::While => self.control_while()?,
            Expr::Until => self.control_until()?,

            // Parallel execution
            Expr::Parallel => self.exec_parallel()?,
            Expr::Fork => self.exec_fork()?,

            // Process substitution
            Expr::Subst => self.process_subst()?,

            // Interactive TTY
            Expr::Tty => self.exec_tty()?,

            Expr::BashPassthrough(code) => {
                let bash = self.ensure_bash()?;
                let (output, exit_code) = bash.execute(code)?;
                self.last_exit_code = exit_code;
                if !output.is_empty() {
                    self.stack.push(Value::Output(output));
                }
            }

            Expr::Define(name) => {
                // Pop block from stack and store as named word
                let block = self.pop_block()?;
                self.definitions.insert(name.clone(), block);
            }
        }

        Ok(())
    }

    /// Execute a command, popping args from stack
    fn execute_command(&mut self, cmd: &str) -> Result<(), EvalError> {
        // Collect args from stack (LIFO - pop until we hit a block, marker, or empty)
        let mut args = Vec::new();
        while let Some(value) = self.stack.last() {
            match value {
                Value::Block(_) => break,
                Value::Marker => break, // Don't pop through markers
                Value::Nil => {
                    self.stack.pop();
                    // Skip nil values
                }
                _ => {
                    if let Some(arg) = value.as_arg() {
                        args.push(arg);
                    }
                    self.stack.pop();
                }
            }
        }

        // Args are in reverse order (LIFO), keep them that way per design
        // Build command: cmd arg1 arg2 ... (args already in LIFO order)
        let bash_cmd = if args.is_empty() {
            cmd.to_string()
        } else {
            format!("{} {}", cmd, args.join(" "))
        };

        let bash = self.ensure_bash()?;
        let (output, exit_code) = bash.execute(&bash_cmd)?;
        self.last_exit_code = exit_code;

        if output.is_empty() {
            self.stack.push(Value::Nil);
        } else {
            self.stack.push(Value::Output(output));
        }

        Ok(())
    }

    /// Apply a block to args on the stack
    fn apply_block(&mut self) -> Result<(), EvalError> {
        let block = self.pop_block()?;

        // Evaluate the block's expressions
        for expr in &block {
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
        let consumer_cmd = self.block_to_bash(&consumer);

        // Execute pipe
        let bash_cmd = format!("echo {} | {}", shell_quote(&input_str), consumer_cmd);
        let bash = self.ensure_bash()?;
        let (output, exit_code) = bash.execute(&bash_cmd)?;
        self.last_exit_code = exit_code;

        if output.is_empty() {
            self.stack.push(Value::Nil);
        } else {
            self.stack.push(Value::Output(output));
        }

        Ok(())
    }

    /// Execute redirect (supports multiple files via tee)
    fn execute_redirect(&mut self, mode: &str) -> Result<(), EvalError> {
        let file_block = self.pop_block()?;
        let cmd = self.pop_block()?;

        // Extract filenames from block
        let files: Vec<String> = file_block
            .iter()
            .filter_map(|e| match e {
                Expr::Literal(s) => Some(s.clone()),
                Expr::Quoted { content, .. } => Some(content.clone()),
                _ => None,
            })
            .collect();

        let cmd_str = self.block_to_bash(&cmd);

        let bash_cmd = if files.len() == 1 {
            // Single file: simple redirect
            format!("{} {} {}", cmd_str, mode, files[0])
        } else if files.len() > 1 {
            // Multiple files: use tee
            let tee_flag = if mode == ">>" { "-a" } else { "" };
            // Write to first file via tee, remaining files as tee args
            let first = &files[0];
            let rest: Vec<_> = files[1..].iter().map(|f| f.as_str()).collect();
            format!(
                "{} | tee {} {} {} > /dev/null",
                cmd_str,
                tee_flag,
                rest.join(" "),
                first
            )
        } else {
            return Err(EvalError::TypeError {
                expected: "filename".into(),
                got: "empty block".into(),
            });
        };

        let bash = self.ensure_bash()?;
        let (output, exit_code) = bash.execute(&bash_cmd)?;
        self.last_exit_code = exit_code;

        if !output.is_empty() {
            self.stack.push(Value::Output(output));
        }

        Ok(())
    }

    /// Execute background
    fn execute_background(&mut self) -> Result<(), EvalError> {
        let cmd = self.pop_block()?;
        let cmd_str = self.block_to_bash(&cmd);

        let bash_cmd = format!("{} &", cmd_str);
        let bash = self.ensure_bash()?;
        let (output, exit_code) = bash.execute(&bash_cmd)?;
        self.last_exit_code = exit_code;

        if !output.is_empty() {
            self.stack.push(Value::Output(output));
        }

        Ok(())
    }

    /// Execute && (and)
    fn execute_and(&mut self) -> Result<(), EvalError> {
        let right = self.pop_block()?;
        let left = self.pop_block()?;

        let left_cmd = self.block_to_bash(&left);
        let right_cmd = self.block_to_bash(&right);

        let bash_cmd = format!("{} && {}", left_cmd, right_cmd);
        let bash = self.ensure_bash()?;
        let (output, exit_code) = bash.execute(&bash_cmd)?;
        self.last_exit_code = exit_code;

        if !output.is_empty() {
            self.stack.push(Value::Output(output));
        }

        Ok(())
    }

    /// Execute || (or)
    fn execute_or(&mut self) -> Result<(), EvalError> {
        let right = self.pop_block()?;
        let left = self.pop_block()?;

        let left_cmd = self.block_to_bash(&left);
        let right_cmd = self.block_to_bash(&right);

        let bash_cmd = format!("{} || {}", left_cmd, right_cmd);
        let bash = self.ensure_bash()?;
        let (output, exit_code) = bash.execute(&bash_cmd)?;
        self.last_exit_code = exit_code;

        if !output.is_empty() {
            self.stack.push(Value::Output(output));
        }

        Ok(())
    }

    /// Convert a block to a bash command string WITHOUT executing
    /// This is used for && || and similar operators where we need the bash command
    fn block_to_bash(&self, exprs: &[Expr]) -> String {
        // For a block like [hello echo], we want "echo hello" (postfix to prefix)
        // In postfix, the command comes LAST, args come first
        let mut parts: Vec<String> = Vec::new();

        for expr in exprs {
            match expr {
                Expr::Literal(s) => {
                    parts.push(s.clone());
                }
                Expr::Quoted { content, double } => {
                    let quoted = if *double {
                        format!("\"{}\"", content)
                    } else {
                        format!("'{}'", content)
                    };
                    parts.push(quoted);
                }
                Expr::Variable(s) => {
                    parts.push(s.clone());
                }
                _ => {
                    // Skip other expression types for now
                }
            }
        }

        if parts.is_empty() {
            return String::new();
        }

        // The last non-flag word is the command (postfix semantics)
        // Find the command: last word that isn't a flag
        let cmd_idx = parts.iter().rposition(|s| !s.starts_with('-') && !s.starts_with('\"') && !s.starts_with('\''));

        match cmd_idx {
            Some(idx) => {
                let cmd = parts.remove(idx);
                if parts.is_empty() {
                    cmd
                } else {
                    format!("{} {}", cmd, parts.join(" "))
                }
            }
            None => {
                // No command found, just join all parts
                parts.join(" ")
            }
        }
    }

    // Stack operations

    fn stack_dup(&mut self) -> Result<(), EvalError> {
        let top = self.stack.last()
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
        self.stack.pop()
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

    // Path operations

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

    fn path_basename(&mut self) -> Result<(), EvalError> {
        let path = self.pop_string()?;
        let basename = std::path::Path::new(&path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(&path)
            .to_string();
        self.stack.push(Value::Literal(basename));
        Ok(())
    }

    fn path_dirname(&mut self) -> Result<(), EvalError> {
        let path = self.pop_string()?;
        let dirname = std::path::Path::new(&path)
            .parent()
            .and_then(|p| p.to_str())
            .unwrap_or(".")
            .to_string();
        self.stack.push(Value::Literal(dirname));
        Ok(())
    }

    fn path_suffix(&mut self) -> Result<(), EvalError> {
        let suffix = self.pop_string()?;
        let base = self.pop_string()?;
        self.stack.push(Value::Literal(format!("{}{}", base, suffix)));
        Ok(())
    }

    fn path_reext(&mut self) -> Result<(), EvalError> {
        let new_ext = self.pop_string()?;
        let path = self.pop_string()?;

        let stem = std::path::Path::new(&path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(&path);

        let new_ext = if new_ext.starts_with('.') {
            new_ext
        } else {
            format!(".{}", new_ext)
        };

        self.stack.push(Value::Literal(format!("{}{}", stem, new_ext)));
        Ok(())
    }

    // List operations

    /// Spread: split a multi-line value into separate stack items
    /// Pushes a marker first, then each line as a Literal
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
        for item in items {
            self.stack.push(item);
            for expr in &block {
                self.eval_expr(expr)?;
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

    // Control flow

    /// If: [condition] [then] [else] if
    fn control_if(&mut self) -> Result<(), EvalError> {
        let else_block = self.pop_block()?;
        let then_block = self.pop_block()?;
        let cond_block = self.pop_block()?;

        // Execute condition block
        for expr in &cond_block {
            self.eval_expr(expr)?;
        }

        // Check result - use exit code
        let condition_met = self.last_exit_code == 0;

        // Execute appropriate branch
        let branch = if condition_met { then_block } else { else_block };
        for expr in &branch {
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

        for _ in 0..n {
            for expr in &block {
                self.eval_expr(expr)?;
            }
        }

        Ok(())
    }

    /// While: [condition] [body] while - repeat while condition passes (exit code 0)
    fn control_while(&mut self) -> Result<(), EvalError> {
        let body = self.pop_block()?;
        let cond = self.pop_block()?;

        loop {
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
                self.eval_expr(expr)?;
            }
        }

        Ok(())
    }

    /// Until: [condition] [body] until - repeat until condition passes (exit code 0)
    fn control_until(&mut self) -> Result<(), EvalError> {
        let body = self.pop_block()?;
        let cond = self.pop_block()?;

        loop {
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
                self.eval_expr(expr)?;
            }
        }

        Ok(())
    }

    /// Parallel: [[cmd1] [cmd2] ...] parallel - run blocks in parallel, wait for all
    fn exec_parallel(&mut self) -> Result<(), EvalError> {
        let blocks = self.pop_block()?;

        // Extract inner blocks
        let mut cmds: Vec<String> = Vec::new();
        for expr in blocks {
            if let Expr::Block(inner) = expr {
                cmds.push(self.block_to_bash(&inner));
            }
        }

        if cmds.is_empty() {
            return Ok(());
        }

        // Run all commands in parallel using bash subshells
        // Format: (cmd1) & (cmd2) & (cmd3) & wait
        let parallel_cmd = cmds
            .iter()
            .map(|c| format!("({}) &", c))
            .collect::<Vec<_>>()
            .join(" ");
        let bash_cmd = format!("{} wait", parallel_cmd);

        let bash = self.ensure_bash()?;
        let (output, exit_code) = bash.execute(&bash_cmd)?;
        self.last_exit_code = exit_code;

        if !output.is_empty() {
            self.stack.push(Value::Output(output));
        }

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

        // Pop N blocks
        let mut cmds: Vec<String> = Vec::new();
        for _ in 0..n {
            let block = self.pop_block()?;
            cmds.push(self.block_to_bash(&block));
        }

        // Reverse to maintain order
        cmds.reverse();

        // Background each command
        for cmd in cmds {
            let bash_cmd = format!("{} &", cmd);
            let bash = self.ensure_bash()?;
            bash.execute(&bash_cmd)?;
        }

        self.last_exit_code = 0;
        Ok(())
    }

    /// Subst: [cmd] subst - run cmd, push temp file path (like bash's <(cmd))
    fn process_subst(&mut self) -> Result<(), EvalError> {
        let block = self.pop_block()?;
        let cmd_str = self.block_to_bash(&block);

        // Create unique temp file
        let temp_path = format!("/tmp/hsab_subst_{}", std::process::id());

        // Generate unique suffix using counter
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let suffix = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let temp_path = format!("{}_{}", temp_path, suffix);

        // Run command, write output to temp file
        let bash_cmd = format!("{} > {}", cmd_str, temp_path);
        let bash = self.ensure_bash()?;
        let (_, exit_code) = bash.execute(&bash_cmd)?;
        self.last_exit_code = exit_code;

        // Push temp file path to stack
        self.stack.push(Value::Literal(temp_path));

        Ok(())
    }

    /// Tty: [cmd] tty - run command with direct TTY access (for interactive commands)
    fn exec_tty(&mut self) -> Result<(), EvalError> {
        let block = self.pop_block()?;
        let cmd_str = self.block_to_bash(&block);

        // Spawn a new bash process with inherited stdin/stdout/stderr (TTY access)
        let status = Command::new("bash")
            .arg("-c")
            .arg(&cmd_str)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .map_err(|e| EvalError::ExecError(format!("Failed to run tty command: {}", e)))?;

        self.last_exit_code = status.code().unwrap_or(-1);

        Ok(())
    }

    // Helper methods

    fn pop_value_or_err(&mut self) -> Result<Value, EvalError> {
        self.stack.pop()
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

/// Shell-quote a string for safe use in bash
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;
    use crate::lexer::lex;

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
    fn eval_path_basename() {
        let result = eval_str("/path/to/file.txt basename").unwrap();
        assert_eq!(result.output, "file");
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

        // test = [dup swap]: a b -> a b b -> a b b (dup) -> b b a (swap)
        // Wait, let's trace: stack starts empty
        // "a" -> [a]
        // "b" -> [a, b]
        // "test" expands to [dup swap]
        //   dup -> [a, b, b]
        //   swap -> [a, b, b] swap top two -> [a, b, b] ??? Let me think again
        // Actually: [a, b] -> dup -> [a, b, b] -> swap -> [a, b, b] with last two swapped -> [a, b, b]
        // Hmm, swap swaps indices len-1 and len-2
        // [a, b, b]: len=3, swap indices 2 and 1 -> [a, b, b] (both are 'b')
        // So result is "a\nb\nb"
        assert_eq!(result.output, "a\nb\nb");
    }
}
