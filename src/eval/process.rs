use super::{Evaluator, EvalError, Job, JobStatus};
use crate::ast::{Expr, Value};
use std::fs::File;
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

impl Evaluator {
    /// Apply a block to args on the stack
    pub(crate) fn apply_block(&mut self) -> Result<(), EvalError> {
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
    pub(crate) fn execute_pipe(&mut self) -> Result<(), EvalError> {
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
    pub(crate) fn execute_redirect(&mut self, mode: &str) -> Result<(), EvalError> {
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
                ">>" => std::fs::OpenOptions::new().append(true).create(true).open(file)?,
                _ => continue,
            };
            f.write_all(output.as_bytes())?;
        }

        Ok(())
    }

    /// Execute stdin redirect: [cmd] [file] <
    pub(crate) fn execute_stdin_redirect(&mut self, cmd: &[Expr], input_file: &str) -> Result<(), EvalError> {
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
    pub(crate) fn execute_redirect_err(&mut self, mode: &str) -> Result<(), EvalError> {
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
            "2>>" => std::fs::OpenOptions::new()
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
    pub(crate) fn execute_redirect_both(&mut self) -> Result<(), EvalError> {
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
    pub(crate) fn execute_redirect_err_to_out(&mut self) -> Result<(), EvalError> {
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
    pub(crate) fn execute_background(&mut self) -> Result<(), EvalError> {
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
    pub(crate) fn execute_and(&mut self) -> Result<(), EvalError> {
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
    pub(crate) fn execute_or(&mut self) -> Result<(), EvalError> {
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

    /// Parallel: [[cmd1] [cmd2] ...] parallel - run blocks in parallel, wait for all
    pub(crate) fn exec_parallel(&mut self) -> Result<(), EvalError> {
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
    pub(crate) fn exec_fork(&mut self) -> Result<(), EvalError> {
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
    pub(crate) fn process_subst(&mut self) -> Result<(), EvalError> {
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
    pub(crate) fn process_fifo(&mut self) -> Result<(), EvalError> {
        let block = self.pop_block()?;
        let (cmd, args) = self.block_to_cmd_args(&block)?;

        // Create unique fifo path
        static NEXT_FIFO_ID: AtomicU64 = AtomicU64::new(0);
        let suffix = NEXT_FIFO_ID.fetch_add(1, Ordering::SeqCst);
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

    pub(crate) fn builtin_timeout(&mut self) -> Result<(), EvalError> {
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

    pub(crate) fn builtin_pipestatus(&mut self) -> Result<(), EvalError> {
        let list: Vec<Value> = self
            .pipestatus
            .iter()
            .map(|&c| Value::Number(c as f64))
            .collect();
        self.stack.push(Value::List(list));
        self.last_exit_code = 0;
        Ok(())
    }
}
