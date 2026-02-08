//! Unified Shell - the central coordinator for hsab
//!
//! The Shell owns all state and orchestrates the full pipeline:
//! 1. Expand %vars (state)
//! 2. Tokenize (lexer)
//! 3. Parse with executable detection (parser + resolver)
//! 4. Transform postfix → infix (transformer)
//! 5. Generate bash (emitter)
//! 6. Execute via persistent bash subprocess
//! 7. Update state

use crate::ast::Ast;
use crate::emitter::emit;
use crate::lexer::{lex, LexError, Token};
use crate::parser::{ParseError, Parser};
use crate::state::ShellState;
use crate::transformer::transform;

use std::io::{self, BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ShellError {
    #[error("Lexer error: {0}")]
    Lex(#[from] LexError),
    #[error("Parse error: {0}")]
    Parse(#[from] ParseError),
    #[error("Execution error: {0}")]
    Exec(#[from] io::Error),
    #[error("Empty input")]
    EmptyInput,
    #[error("Bash process died")]
    BashDied,
}

/// Result of executing a command
#[derive(Debug, Clone)]
pub struct Execution {
    /// The generated bash command
    pub bash: String,
    /// Stdout from the command
    pub stdout: String,
    /// Stderr from the command (empty in interactive mode - passes through)
    pub stderr: String,
    /// Exit code
    pub exit_code: i32,
    /// Leftover tokens that weren't consumed (for REPL to put back on input)
    pub leftovers: String,
    /// Arguments that were passed to the command (for state tracking)
    pub args: Vec<String>,
}

impl Execution {
    pub fn success(&self) -> bool {
        self.exit_code == 0
    }

    pub fn exit_code(&self) -> i32 {
        self.exit_code
    }
}

/// Persistent bash subprocess for executing commands
struct BashProcess {
    #[allow(dead_code)]
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    marker_counter: u64,
}

impl BashProcess {
    fn new() -> io::Result<Self> {
        let mut child = Command::new("bash")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit()) // Let stderr pass through immediately
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

    /// Execute a command and return (stdout, exit_code)
    fn execute(&mut self, cmd: &str) -> io::Result<(String, i32)> {
        self.marker_counter += 1;
        let marker = format!("__HSAB_{}__", self.marker_counter);

        // Send command, then echo marker with exit code
        // The leading \n ensures marker is on its own line even if command
        // output doesn't end with a newline
        writeln!(self.stdin, "{}", cmd)?;
        writeln!(self.stdin, "printf '\\n{}:%d\\n' $?", marker)?;
        self.stdin.flush()?;

        // Read until we see the marker
        let mut output = String::new();
        let mut exit_code = 0;

        loop {
            let mut line = String::new();
            let bytes_read = self.stdout.read_line(&mut line)?;

            if bytes_read == 0 {
                // EOF - bash process died
                return Err(io::Error::new(io::ErrorKind::BrokenPipe, "bash process ended"));
            }

            if line.contains(&marker) {
                // Parse exit code from "MARKER:CODE"
                if let Some(code_str) = line.trim().strip_prefix(&format!("{}:", marker)) {
                    exit_code = code_str.parse().unwrap_or(-1);
                }
                break;
            }
            output.push_str(&line);
        }

        // Remove artifacts from our marker's leading \n:
        // - If output is just "\n" (no-output command), clear it
        // - If output ends with "\n\n", remove the extra newline
        if output == "\n" {
            output.clear();
        } else if output.ends_with("\n\n") {
            output.pop();
        }

        Ok((output, exit_code))
    }

    /// Check if bash process is still alive
    fn is_alive(&mut self) -> bool {
        match self.child.try_wait() {
            Ok(None) => true,      // Still running
            Ok(Some(_)) => false,  // Exited
            Err(_) => false,       // Error checking
        }
    }
}

/// The unified shell that owns all state and coordinates execution
pub struct Shell {
    /// Shell state (%vars and environment)
    pub state: ShellState,
    /// Persistent bash subprocess
    bash: Option<BashProcess>,
}

impl Default for Shell {
    fn default() -> Self {
        Self::new()
    }
}

impl Shell {
    /// Create a new shell instance with a persistent bash subprocess
    pub fn new() -> Self {
        let bash = BashProcess::new().ok();
        Shell {
            state: ShellState::new(),
            bash,
        }
    }

    /// Ensure bash process is running, restart if needed
    fn ensure_bash(&mut self) -> Result<&mut BashProcess, ShellError> {
        // Check if we need to (re)start bash
        let needs_restart = match &mut self.bash {
            None => true,
            Some(ref mut bash) => !bash.is_alive(),
        };

        if needs_restart {
            self.bash = Some(BashProcess::new()?);
        }

        self.bash.as_mut().ok_or(ShellError::BashDied)
    }

    /// Execute an hsab command, returning results and any leftovers
    pub fn execute(&mut self, input: &str) -> Result<Execution, ShellError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(ShellError::EmptyInput);
        }

        // Handle bash passthrough
        if trimmed.starts_with("#!bash") {
            let bash_code = trimmed.strip_prefix("#!bash").unwrap_or(trimmed).trim();
            return self.run_in_bash(bash_code, String::new(), vec![]);
        }

        // 1. Expand %vars
        let expanded = self.state.expand_all(trimmed);

        // 2. Tokenize
        let tokens = lex(&expanded)?;
        if tokens.is_empty() {
            return Err(ShellError::EmptyInput);
        }

        // 3. Parse (with leftovers detection)
        let (ast, leftovers, args) = self.parse_with_leftovers(tokens)?;

        // 4. Transform postfix → infix
        let transformed = transform(ast);

        // 5. Generate bash
        let bash = emit(&transformed);

        // 6. Execute in persistent bash
        self.run_in_bash(&bash, leftovers, args)
    }

    /// Run a bash command in the persistent subprocess
    fn run_in_bash(&mut self, bash: &str, leftovers: String, args: Vec<String>) -> Result<Execution, ShellError> {
        let bash_proc = self.ensure_bash()?;
        let (stdout, exit_code) = bash_proc.execute(bash)?;

        // Update state
        self.state.update(
            args.clone(),
            stdout.clone(),
            String::new(), // stderr passes through, not captured
            exit_code,
            bash.to_string(),
        );

        Ok(Execution {
            bash: bash.to_string(),
            stdout,
            stderr: String::new(),
            exit_code,
            leftovers,
            args,
        })
    }

    /// Execute interactively (for REPL) - same as execute but output streams through
    pub fn execute_interactive(&mut self, input: &str) -> Result<Execution, ShellError> {
        // In the persistent bash model, execute and execute_interactive are the same
        // because stdout is captured but stderr passes through
        self.execute(input)
    }

    /// Parse tokens, returning (AST, leftovers string, args)
    fn parse_with_leftovers(
        &mut self,
        tokens: Vec<Token>,
    ) -> Result<(Ast, String, Vec<String>), ParseError> {
        let mut parser = Parser::new(tokens.clone());
        let ast = parser.parse()?;

        // Extract args from the AST for state tracking
        let args = Self::extract_args(&ast);

        // Get leftover tokens (tokens not consumed by parser)
        let consumed = parser.position();
        let leftover_tokens: Vec<_> = tokens.into_iter().skip(consumed).collect();
        let leftovers = Self::tokens_to_string(&leftover_tokens);

        Ok((ast, leftovers, args))
    }

    /// Extract arguments from an AST (for state tracking)
    fn extract_args(ast: &Ast) -> Vec<String> {
        match ast {
            Ast::Command { args, .. } => args.clone(),
            Ast::Pipe { consumer, .. } => Self::extract_args(consumer),
            Ast::And { right, .. } => Self::extract_args(right),
            Ast::Or { right, .. } => Self::extract_args(right),
            Ast::Redirect { cmd, .. } => Self::extract_args(cmd),
            Ast::Background { cmd } => Self::extract_args(cmd),
            Ast::Subshell { inner } => Self::extract_args(inner),
            Ast::BashPassthrough(_) => vec![],
        }
    }

    /// Convert tokens back to a string (for leftovers)
    fn tokens_to_string(tokens: &[Token]) -> String {
        tokens
            .iter()
            .map(|t| match t {
                Token::Word(s) => s.clone(),
                Token::DoubleQuoted(s) => format!("\"{}\"", s),
                Token::SingleQuoted(s) => format!("'{}'", s),
                Token::Variable(s) => s.clone(),
                Token::GroupStart => "%(".to_string(),
                Token::GroupEnd => ")".to_string(),
                Token::SubshellStart => "$(".to_string(),
                Token::BashPassthrough(s) => format!("\\{{{}}}", s),
                Token::Operator(op) => format!("{:?}", op).to_lowercase(),
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Compile without executing (for --emit)
    pub fn compile(&mut self, input: &str) -> Result<String, ShellError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(ShellError::EmptyInput);
        }

        if trimmed.starts_with("#!bash") {
            return Ok(trimmed.strip_prefix("#!bash").unwrap_or(trimmed).trim().to_string());
        }

        let expanded = self.state.expand_all(trimmed);
        let tokens = lex(&expanded)?;
        if tokens.is_empty() {
            return Err(ShellError::EmptyInput);
        }

        let mut parser = Parser::new(tokens);
        let ast = parser.parse()?;
        let transformed = transform(ast);
        let bash = emit(&transformed);

        Ok(bash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_execute() {
        let mut shell = Shell::new();
        let result = shell.execute("hello echo").unwrap();
        assert!(result.success());
        assert_eq!(result.stdout.trim(), "hello");
    }

    #[test]
    fn test_shell_state_updates() {
        let mut shell = Shell::new();

        // Execute a command
        let _ = shell.execute("hello echo").unwrap();

        // State should be updated
        assert_eq!(shell.state.last_arg, "hello");
        assert_eq!(shell.state.last_exit_code, 0);
        assert!(shell.state.last_bash_cmd.contains("echo"));
    }

    #[test]
    fn test_shell_percent_vars() {
        let mut shell = Shell::new();

        // First command
        let _ = shell.execute("hello echo").unwrap();

        // Second command using %_
        let result = shell.compile("%_ echo").unwrap();
        assert_eq!(result, "echo hello");
    }

    #[test]
    fn test_shell_exit_code() {
        let mut shell = Shell::new();

        // Successful command
        let _ = shell.execute("true").unwrap();
        assert_eq!(shell.state.last_exit_code, 0);

        // Failed command
        let _ = shell.execute("false").unwrap();
        assert_eq!(shell.state.last_exit_code, 1);
    }

    #[test]
    fn test_shell_line_indexing() {
        let mut shell = Shell::new();

        // Command with multi-line output
        let _ = shell.execute("#!bash echo -e 'line1\\nline2\\nline3'").unwrap();

        // Check line indexing
        assert_eq!(shell.state.get_line(0), "line1");
        assert_eq!(shell.state.get_line(1), "line2");
        assert_eq!(shell.state.get_line(2), "line3");

        // Use in next command
        let result = shell.compile("%0 echo").unwrap();
        assert_eq!(result, "echo line1");
    }

    #[test]
    fn test_shell_compile() {
        let mut shell = Shell::new();

        // With leftover behavior, only first executable is parsed
        // "-la ls hello grep" → "ls -la" (hello grep are leftovers)
        let bash = shell.compile("-la ls hello grep").unwrap();
        assert_eq!(bash, "ls -la");

        // To get a pipe, use explicit groups:
        let bash = shell.compile("%(hello grep) -la ls").unwrap();
        assert_eq!(bash, "ls -la | grep hello");
    }

    #[test]
    fn test_bash_passthrough() {
        let mut shell = Shell::new();
        let result = shell.execute("#!bash echo 'raw bash'").unwrap();
        assert!(result.success());
        assert_eq!(result.stdout.trim(), "raw bash");
    }

    #[test]
    fn test_persistent_variables() {
        let mut shell = Shell::new();

        // Set a variable
        let _ = shell.execute("#!bash export MYVAR=hello").unwrap();

        // Use the variable in next command
        let result = shell.execute("#!bash echo $MYVAR").unwrap();
        assert_eq!(result.stdout.trim(), "hello");
    }

    #[test]
    fn test_persistent_variable_with_hsab_syntax() {
        let mut shell = Shell::new();

        // Set a variable using hsab postfix
        let _ = shell.execute("MYVAR=world export").unwrap();

        // Use the variable
        let result = shell.execute("$MYVAR echo").unwrap();
        assert_eq!(result.stdout.trim(), "world");
    }
}
