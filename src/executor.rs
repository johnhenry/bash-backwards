//! Executor: runs bash commands

use std::io;
use std::process::{Command, ExitStatus, Stdio};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ExecuteError {
    #[error("Failed to spawn bash: {0}")]
    SpawnError(#[from] io::Error),
    #[error("Bash execution failed with status: {0}")]
    ExecutionFailed(ExitStatus),
    #[error("Compilation error: {0}")]
    CompileError(String),
}

/// Result of executing a command
#[derive(Debug)]
pub struct ExecuteResult {
    pub stdout: String,
    pub stderr: String,
    pub status: ExitStatus,
}

impl ExecuteResult {
    pub fn success(&self) -> bool {
        self.status.success()
    }
}

/// Execute a bash command string
pub fn execute_bash(bash_code: &str) -> Result<ExecuteResult, ExecuteError> {
    let output = Command::new("bash")
        .arg("-c")
        .arg(bash_code)
        .stdin(Stdio::inherit())
        .output()?;

    Ok(ExecuteResult {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        status: output.status,
    })
}

/// Execute an hsab command (compile and run)
pub fn execute(hsab_code: &str) -> Result<ExecuteResult, ExecuteError> {
    use crate::transformer::compile_transformed;

    let bash_code = compile_transformed(hsab_code)
        .map_err(ExecuteError::CompileError)?;

    execute_bash(&bash_code)
}

/// Execute an hsab command interactively (inheriting stdin/stdout/stderr)
pub fn execute_interactive(hsab_code: &str) -> Result<ExitStatus, ExecuteError> {
    use crate::transformer::compile_transformed;

    let bash_code = compile_transformed(hsab_code)
        .map_err(ExecuteError::CompileError)?;

    let status = Command::new("bash")
        .arg("-c")
        .arg(&bash_code)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;

    Ok(status)
}

/// Check if a line is a bash passthrough line
pub fn is_bash_passthrough(line: &str) -> bool {
    line.trim_start().starts_with("#!bash")
}

/// Execute a line, handling bash passthrough
pub fn execute_line(line: &str) -> Result<ExecuteResult, ExecuteError> {
    let trimmed = line.trim();

    if trimmed.is_empty() {
        // Return a successful result for empty lines
        return execute_bash("true");
    }

    if is_bash_passthrough(trimmed) {
        // Remove the #!bash prefix and execute as raw bash
        let bash_code = trimmed.strip_prefix("#!bash").unwrap_or(trimmed).trim();
        execute_bash(bash_code)
    } else {
        execute(trimmed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execute_simple_command() {
        // Postfix: "hello echo" → bash: "echo hello"
        let result = execute("hello echo").unwrap();
        assert!(result.success());
        assert_eq!(result.stdout.trim(), "hello");
    }

    #[test]
    fn execute_bash_directly() {
        let result = execute_bash("echo world").unwrap();
        assert!(result.success());
        assert_eq!(result.stdout.trim(), "world");
    }

    #[test]
    fn execute_pipe() {
        // Test that a pipe compiles and executes correctly
        // Use a pattern that will definitely match
        let result = execute("%(Cargo grep) ls").unwrap();
        // This should compile to: ls | grep Cargo
        // Cargo.toml should always exist in the project root
        assert!(result.success());
        assert!(result.stdout.contains("Cargo"));
    }

    #[test]
    fn execute_simple_pipe() {
        // Use a simpler test that we know works
        let result = execute_bash("echo -e 'hello\nworld' | grep world").unwrap();
        assert!(result.success());
        assert_eq!(result.stdout.trim(), "world");
    }

    #[test]
    fn bash_passthrough_detection() {
        assert!(is_bash_passthrough("#!bash echo hello"));
        assert!(is_bash_passthrough("  #!bash echo hello"));
        assert!(!is_bash_passthrough("echo hello"));
    }

    #[test]
    fn execute_bash_passthrough_line() {
        let result = execute_line("#!bash echo passthrough").unwrap();
        assert!(result.success());
        assert_eq!(result.stdout.trim(), "passthrough");
    }

    #[test]
    fn execute_and_chain() {
        let result = execute_bash("true && echo success").unwrap();
        assert!(result.success());
        assert_eq!(result.stdout.trim(), "success");
    }

    #[test]
    fn execute_or_chain() {
        let result = execute_bash("false || echo fallback").unwrap();
        assert!(result.success());
        assert_eq!(result.stdout.trim(), "fallback");
    }
}
