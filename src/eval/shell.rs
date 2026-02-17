use super::{Evaluator, EvalError, JobStatus};
use crate::ast::Value;
use crate::resolver::ExecutableResolver;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

impl Evaluator {
    pub(crate) fn builtin_cd(&mut self, args: &[String]) -> Result<(), EvalError> {
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

    pub(crate) fn builtin_pwd(&mut self) -> Result<(), EvalError> {
        self.stack
            .push(Value::Output(self.cwd.to_string_lossy().to_string() + "\n"));
        self.last_exit_code = 0;
        Ok(())
    }

    pub(crate) fn builtin_echo(&mut self, args: &[String]) -> Result<(), EvalError> {
        let output = args.join(" ");
        self.stack.push(Value::Output(format!("{}\n", output)));
        self.last_exit_code = 0;
        Ok(())
    }

    pub(crate) fn builtin_true(&mut self) -> Result<(), EvalError> {
        self.stack.push(Value::Bool(true));
        self.last_exit_code = 0;
        Ok(())
    }

    pub(crate) fn builtin_false(&mut self) -> Result<(), EvalError> {
        self.stack.push(Value::Bool(false));
        self.last_exit_code = 1;
        Ok(())
    }

    pub(crate) fn builtin_test(&mut self, args: &[String]) -> Result<(), EvalError> {
        let args: Vec<String> = args.iter().rev().cloned().collect();
        let result = match args.as_slice() {
            [path, flag] if flag == "-f" => Path::new(path).is_file(),
            [path, flag] if flag == "-d" => Path::new(path).is_dir(),
            [path, flag] if flag == "-e" => Path::new(path).exists(),
            [path, flag] if flag == "-r" => Path::new(path).exists(),
            [path, flag] if flag == "-w" => Path::new(path).exists(),
            [path, flag] if flag == "-x" => self.is_executable(path),
            [path, flag] if flag == "-s" => {
                Path::new(path)
                    .metadata()
                    .map(|m| m.len() > 0)
                    .unwrap_or(false)
            }
            [s, flag] if flag == "-z" => s.is_empty(),
            [s, flag] if flag == "-n" => !s.is_empty(),
            [s1, s2, op] if op == "=" || op == "==" => s1 == s2,
            [s1, s2, op] if op == "!=" => s1 != s2,
            [n1, n2, op] if op == "-eq" => self.cmp_nums(n1, n2, |a, b| a == b),
            [n1, n2, op] if op == "-ne" => self.cmp_nums(n1, n2, |a, b| a != b),
            [n1, n2, op] if op == "-lt" => self.cmp_nums(n1, n2, |a, b| a < b),
            [n1, n2, op] if op == "-le" => self.cmp_nums(n1, n2, |a, b| a <= b),
            [n1, n2, op] if op == "-gt" => self.cmp_nums(n1, n2, |a, b| a > b),
            [n1, n2, op] if op == "-ge" => self.cmp_nums(n1, n2, |a, b| a >= b),
            [s] => !s.is_empty(),
            [] => false,
            _ => false,
        };

        self.last_exit_code = if result { 0 } else { 1 };
        Ok(())
    }

    pub(crate) fn cmp_nums<F>(&self, a: &str, b: &str, cmp: F) -> bool
    where
        F: Fn(i64, i64) -> bool,
    {
        match (a.parse::<i64>(), b.parse::<i64>()) {
            (Ok(a), Ok(b)) => cmp(a, b),
            _ => false,
        }
    }

    pub(crate) fn is_executable(&self, path: &str) -> bool {
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

    pub(crate) fn builtin_export(&mut self, args: &[String]) -> Result<(), EvalError> {
        for arg in args.iter() {
            if let Some((key, value)) = arg.split_once('=') {
                std::env::set_var(key, value);
            } else if args.len() >= 2 {
                let name = &args[0];
                let value = &args[1];
                std::env::set_var(name, value);
                break;
            }
        }
        self.last_exit_code = 0;
        Ok(())
    }

    pub(crate) fn builtin_unset(&mut self, args: &[String]) -> Result<(), EvalError> {
        for var in args {
            std::env::remove_var(var);
        }
        self.last_exit_code = 0;
        Ok(())
    }

    pub(crate) fn builtin_env(&mut self) -> Result<(), EvalError> {
        let mut output = String::new();
        for (key, value) in std::env::vars() {
            output.push_str(&format!("{}={}\n", key, value));
        }
        self.stack.push(Value::Output(output));
        self.last_exit_code = 0;
        Ok(())
    }

    pub(crate) fn builtin_jobs(&mut self) -> Result<(), EvalError> {
        self.update_job_statuses();

        let mut output = String::new();
        for job in &self.jobs {
            let status_str = match &job.status {
                JobStatus::Running => "Running",
                JobStatus::Stopped => "Stopped",
                JobStatus::Done(code) => {
                    if *code == 0 { "Done" } else { "Exit" }
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

    pub(crate) fn update_job_statuses(&mut self) {
        for job in &mut self.jobs {
            if job.status == JobStatus::Running {
                if let Some(ref mut child) = job.child {
                    match child.try_wait() {
                        Ok(Some(status)) => {
                            job.status = JobStatus::Done(status.code().unwrap_or(-1));
                        }
                        Ok(None) => {}
                        Err(_) => {
                            job.status = JobStatus::Done(-1);
                        }
                    }
                }
            }
        }
    }

    pub(crate) fn builtin_fg(&mut self, args: &[String]) -> Result<(), EvalError> {
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

    pub(crate) fn builtin_bg(&mut self, args: &[String]) -> Result<(), EvalError> {
        let job_id: Option<usize> = args
            .first()
            .and_then(|s| s.trim_start_matches('%').parse().ok());

        let job_info = if let Some(id) = job_id {
            self.jobs
                .iter()
                .find(|j| j.id == id && j.status == JobStatus::Stopped)
                .map(|j| (j.id, j.pgid, j.command.clone()))
        } else {
            self.jobs
                .iter()
                .rev()
                .find(|j| j.status == JobStatus::Stopped)
                .map(|j| (j.id, j.pgid, j.command.clone()))
        };

        match job_info {
            Some((id, pgid, cmd)) => {
                crate::signals::continue_process(pgid)
                    .map_err(|e| EvalError::ExecError(format!("bg: {}", e)))?;

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

    pub(crate) fn builtin_exit(&mut self, args: &[String]) -> Result<(), EvalError> {
        let code = args.first().and_then(|s| s.parse::<i32>().ok()).unwrap_or(0);
        std::process::exit(code);
    }

    pub(crate) fn builtin_tty(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.is_empty() {
            return Err(EvalError::ExecError("tty: no command specified".into()));
        }

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

    pub(crate) fn builtin_which(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.is_empty() {
            return Err(EvalError::ExecError("which: no command specified".into()));
        }

        let mut output_lines = Vec::new();
        let mut found_any = false;

        for cmd in args {
            if ExecutableResolver::is_hsab_builtin(cmd) {
                output_lines.push(format!("{}: hsab builtin", cmd));
                found_any = true;
                continue;
            }

            if self.definitions.contains_key(cmd) {
                output_lines.push(format!("{}: hsab definition", cmd));
                found_any = true;
                continue;
            }

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

    pub(crate) fn builtin_type(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.is_empty() {
            return Err(EvalError::ExecError("type: no command specified".into()));
        }

        let mut output_lines = Vec::new();
        let mut found_any = false;

        for cmd in args {
            if ExecutableResolver::is_hsab_builtin(cmd) {
                output_lines.push(format!("{} is a hsab builtin", cmd));
                found_any = true;
                continue;
            }

            if self.definitions.contains_key(cmd) {
                output_lines.push(format!("{} is a hsab function", cmd));
                found_any = true;
                continue;
            }

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

    pub(crate) fn builtin_source(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.is_empty() {
            return Err(EvalError::ExecError("source: no file specified".into()));
        }

        let path_str = self.expand_tilde(&args[args.len() - 1]);
        let path = PathBuf::from(&path_str);

        let content = std::fs::read_to_string(&path)
            .map_err(|e| EvalError::ExecError(format!("source: {}: {}", path_str, e)))?;

        let tokens = crate::lex(&content)
            .map_err(|e| EvalError::ExecError(format!("source: parse error: {}", e)))?;

        if tokens.is_empty() {
            self.last_exit_code = 0;
            return Ok(());
        }

        let program = crate::parse(tokens)
            .map_err(|e| EvalError::ExecError(format!("source: parse error: {}", e)))?;

        for expr in &program.expressions {
            self.eval_expr(expr)?;
        }

        self.last_exit_code = 0;
        Ok(())
    }

    pub(crate) fn builtin_hash(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.iter().any(|a| a == "-r") {
            self.resolver.clear_cache();
            self.last_exit_code = 0;
            return Ok(());
        }

        if !args.is_empty() {
            for cmd in args {
                self.resolver.resolve_and_cache(cmd);
            }
            self.last_exit_code = 0;
            return Ok(());
        }

        let entries = self.resolver.get_cache_entries();
        if entries.is_empty() {
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

    pub(crate) fn builtin_read(&mut self, args: &[String]) -> Result<(), EvalError> {
        use std::io::{self, BufRead};

        let stdin = io::stdin();
        let mut line = String::new();

        match stdin.lock().read_line(&mut line) {
            Ok(0) => {
                self.last_exit_code = 1;
            }
            Ok(_) => {
                let value = line.trim_end_matches('\n').trim_end_matches('\r').to_string();

                if args.is_empty() {
                    self.stack.push(Value::Output(value));
                } else {
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

    pub(crate) fn builtin_printf(&mut self, args: &[String]) -> Result<(), EvalError> {
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
                        if arg_idx < printf_args.len() {
                            output.push_str(&printf_args[arg_idx]);
                            arg_idx += 1;
                        }
                    }
                    Some('d') | Some('i') => {
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

    pub(crate) fn builtin_wait(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.is_empty() {
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

    pub(crate) fn builtin_kill(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.is_empty() {
            return Err(EvalError::ExecError("kill: usage: PID [-signal] kill".into()));
        }

        let mut signal = 15i32;
        let mut pid_str = &args[0];

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

    pub(crate) fn builtin_pushd(&mut self, args: &[String]) -> Result<(), EvalError> {
        let target = if args.is_empty() {
            if self.dir_stack.is_empty() {
                return Err(EvalError::ExecError("pushd: no other directory".into()));
            }
            self.dir_stack.pop().unwrap()
        } else {
            let path = self.expand_tilde(&args[0]);
            PathBuf::from(path)
        };

        self.dir_stack.push(self.cwd.clone());

        if target.is_dir() {
            self.cwd = target.canonicalize().unwrap_or(target);
            std::env::set_current_dir(&self.cwd)?;

            let mut output = self.cwd.display().to_string();
            for dir in self.dir_stack.iter().rev() {
                output.push(' ');
                output.push_str(&dir.display().to_string());
            }
            output.push('\n');
            self.stack.push(Value::Output(output));
            self.last_exit_code = 0;
        } else {
            self.dir_stack.pop();
            return Err(EvalError::ExecError(format!(
                "pushd: {}: No such directory",
                target.display()
            )));
        }

        Ok(())
    }

    pub(crate) fn builtin_popd(&mut self, _args: &[String]) -> Result<(), EvalError> {
        if self.dir_stack.is_empty() {
            return Err(EvalError::ExecError("popd: directory stack empty".into()));
        }

        let target = self.dir_stack.pop().unwrap();

        if target.is_dir() {
            self.cwd = target.canonicalize().unwrap_or(target);
            std::env::set_current_dir(&self.cwd)?;

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

    pub(crate) fn builtin_dirs(&mut self, args: &[String]) -> Result<(), EvalError> {
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

    pub(crate) fn builtin_alias(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.is_empty() {
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

        if let Some(Value::Block(block)) = self.stack.last().cloned() {
            self.stack.pop();
            self.aliases.insert(name.clone(), block);
            self.last_exit_code = 0;
            return Ok(());
        }

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

    pub(crate) fn builtin_unalias(&mut self, args: &[String]) -> Result<(), EvalError> {
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

    pub(crate) fn builtin_trap(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.is_empty() {
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

        let sig_str = &args[0];
        let signal = self.parse_signal(sig_str)?;

        if let Some(Value::Block(block)) = self.stack.last().cloned() {
            self.stack.pop();
            if block.is_empty() {
                self.traps.remove(&signal);
            } else {
                self.traps.insert(signal, block);
            }
            self.last_exit_code = 0;
            return Ok(());
        }

        if let Some(block) = self.traps.get(&signal) {
            let body_str = self.exprs_to_string(block);
            let sig_name = self.signal_name(signal);
            self.stack
                .push(Value::Output(format!("trap -- '[{}]' {}\n", body_str, sig_name)));
        }

        self.last_exit_code = 0;
        Ok(())
    }

    pub(crate) fn parse_signal(&self, s: &str) -> Result<i32, EvalError> {
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

    pub(crate) fn signal_name(&self, sig: i32) -> &'static str {
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

    pub(crate) fn builtin_return(&mut self, args: &[String]) -> Result<(), EvalError> {
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
}
