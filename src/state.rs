//! Shell state management for hsab-specific variables
//!
//! Tracks state between commands for hsab's `%` variables:
//! - `%_` - Last argument of previous command
//! - `%!` - Stdout of previous command
//! - `%?` - Exit code of previous command
//! - `%cmd` - The bash command that was generated
//! - `%@` - All args of previous command (space-separated)
//! - `%lines` - Stdout as newline-separated (same as %!)
//! - `%0`, `%1`, `%2`... - Individual lines of output (0-indexed)

use std::collections::HashMap;

/// Shell state that persists between commands
#[derive(Debug, Clone, Default)]
pub struct ShellState {
    /// Last argument of previous command
    pub last_arg: String,
    /// All arguments of previous command
    pub all_args: Vec<String>,
    /// Stdout of previous command
    pub last_stdout: String,
    /// Stderr of previous command
    pub last_stderr: String,
    /// Exit code of previous command
    pub last_exit_code: i32,
    /// The bash command that was generated
    pub last_bash_cmd: String,
    /// Cached lines of stdout for indexed access
    stdout_lines: Vec<String>,
}

impl ShellState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Update state after a command execution
    pub fn update(
        &mut self,
        args: Vec<String>,
        stdout: String,
        stderr: String,
        exit_code: i32,
        bash_cmd: String,
    ) {
        self.last_arg = args.last().cloned().unwrap_or_default();
        self.all_args = args;
        self.last_stdout = stdout.clone();
        self.last_stderr = stderr;
        self.last_exit_code = exit_code;
        self.last_bash_cmd = bash_cmd;

        // Cache lines for indexed access
        self.stdout_lines = stdout.lines().map(String::from).collect();
    }

    /// Get a line by index (0-indexed), returns empty string if out of bounds
    pub fn get_line(&self, index: usize) -> &str {
        self.stdout_lines.get(index).map(|s| s.as_str()).unwrap_or("")
    }

    /// Get number of output lines
    pub fn line_count(&self) -> usize {
        self.stdout_lines.len()
    }

    /// Expand a `%` variable to its value
    pub fn expand_variable(&self, var: &str) -> Option<String> {
        match var {
            "%" => Some("%".to_string()), // Escaped %
            "%_" => Some(self.last_arg.clone()),
            "%!" => Some(self.last_stdout.trim_end().to_string()),
            "%?" => Some(self.last_exit_code.to_string()),
            "%cmd" => Some(self.last_bash_cmd.clone()),
            "%@" => Some(self.all_args.join(" ")),
            "%lines" => Some(self.last_stdout.trim_end().to_string()),
            _ => {
                // Check for %N (line index)
                if var.starts_with('%') {
                    if let Ok(index) = var[1..].parse::<usize>() {
                        return Some(self.get_line(index).to_string());
                    }
                }
                None
            }
        }
    }

    /// Expand all `%` variables in a string
    pub fn expand_all(&self, input: &str) -> String {
        let mut result = input.to_string();

        // Order matters - longer patterns first to avoid partial matches
        let patterns = [
            "%lines", "%cmd", "%@", "%!", "%?", "%_", "%%",
        ];

        for pattern in patterns {
            if let Some(replacement) = self.expand_variable(pattern) {
                result = result.replace(pattern, &replacement);
            }
        }

        // Handle %N patterns (numeric line indices)
        let mut i = 0;
        while i < result.len() {
            if result[i..].starts_with('%') {
                // Find the end of the number
                let start = i + 1;
                let mut end = start;
                while end < result.len() && result[end..].chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                    end += 1;
                }
                if end > start {
                    if let Ok(index) = result[start..end].parse::<usize>() {
                        let replacement = self.get_line(index);
                        result = format!("{}{}{}", &result[..i], replacement, &result[end..]);
                        i += replacement.len();
                        continue;
                    }
                }
            }
            i += 1;
        }

        result
    }

    /// Get all variables as a HashMap (useful for environment injection)
    pub fn as_env_vars(&self) -> HashMap<String, String> {
        let mut vars = HashMap::new();
        vars.insert("_".to_string(), self.last_arg.clone());
        vars.insert("HSAB_STDOUT".to_string(), self.last_stdout.clone());
        vars.insert("HSAB_EXIT".to_string(), self.last_exit_code.to_string());
        vars.insert("HSAB_CMD".to_string(), self.last_bash_cmd.clone());
        vars.insert("HSAB_ARGS".to_string(), self.all_args.join(" "));
        vars
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_last_arg() {
        let mut state = ShellState::new();
        state.update(
            vec!["arg1".to_string(), "arg2".to_string()],
            "output".to_string(),
            "".to_string(),
            0,
            "echo arg1 arg2".to_string(),
        );

        assert_eq!(state.expand_variable("%_"), Some("arg2".to_string()));
    }

    #[test]
    fn test_expand_stdout() {
        let mut state = ShellState::new();
        state.update(
            vec![],
            "line1\nline2\nline3\n".to_string(),
            "".to_string(),
            0,
            "cmd".to_string(),
        );

        assert_eq!(state.expand_variable("%!"), Some("line1\nline2\nline3".to_string()));
    }

    #[test]
    fn test_expand_exit_code() {
        let mut state = ShellState::new();
        state.update(vec![], "".to_string(), "".to_string(), 42, "cmd".to_string());

        assert_eq!(state.expand_variable("%?"), Some("42".to_string()));
    }

    #[test]
    fn test_expand_line_index() {
        let mut state = ShellState::new();
        state.update(
            vec![],
            "first\nsecond\nthird\n".to_string(),
            "".to_string(),
            0,
            "cmd".to_string(),
        );

        assert_eq!(state.expand_variable("%0"), Some("first".to_string()));
        assert_eq!(state.expand_variable("%1"), Some("second".to_string()));
        assert_eq!(state.expand_variable("%2"), Some("third".to_string()));
        assert_eq!(state.expand_variable("%99"), Some("".to_string())); // Out of bounds
    }

    #[test]
    fn test_expand_all_args() {
        let mut state = ShellState::new();
        state.update(
            vec!["a".to_string(), "b".to_string(), "c".to_string()],
            "".to_string(),
            "".to_string(),
            0,
            "cmd".to_string(),
        );

        assert_eq!(state.expand_variable("%@"), Some("a b c".to_string()));
    }

    #[test]
    fn test_expand_all_in_string() {
        let mut state = ShellState::new();
        state.update(
            vec!["file.txt".to_string()],
            "hello\nworld\n".to_string(),
            "".to_string(),
            0,
            "cat file.txt".to_string(),
        );

        let input = "Last arg was %_, first line was %0";
        let expanded = state.expand_all(input);
        assert_eq!(expanded, "Last arg was file.txt, first line was hello");
    }

    #[test]
    fn test_expand_cmd() {
        let mut state = ShellState::new();
        state.update(
            vec![],
            "".to_string(),
            "".to_string(),
            0,
            "ls -la | grep foo".to_string(),
        );

        assert_eq!(state.expand_variable("%cmd"), Some("ls -la | grep foo".to_string()));
    }
}
