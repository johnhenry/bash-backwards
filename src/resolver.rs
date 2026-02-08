//! Executable resolver for detecting commands vs arguments
//!
//! Provides detection of executable commands by checking:
//! 1. A built-in list of common shell commands (~100)
//! 2. PATH lookup with caching

use std::collections::{HashMap, HashSet};
use std::env;
use std::path::Path;

/// Stack operations for argument manipulation
pub const STACK_OPS: &[&str] = &["dup", "swap", "drop", "over", "rot"];

/// Path operations for filename manipulation
pub const PATH_OPS: &[&str] = &["join", "basename", "dirname", "suffix", "reext"];

/// All hsab builtins (stack + path + list + control ops)
pub const HSAB_BUILTINS: &[&str] = &[
    // Stack ops
    "dup", "swap", "drop", "over", "rot",
    // Path ops
    "join", "basename", "dirname", "suffix", "reext",
    // List ops
    "spread", "each", "collect", "keep",
    // Control ops
    "if", "times",
];

/// Resolves whether a word is an executable command
pub struct ExecutableResolver {
    /// Common shell builtins and commands for fast lookup
    builtins: HashSet<&'static str>,
    /// Cached PATH lookup results
    path_cache: HashMap<String, bool>,
    /// Parsed PATH directories
    path_dirs: Vec<String>,
}

impl Default for ExecutableResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl ExecutableResolver {
    /// Create a new resolver with default builtins and PATH from environment
    pub fn new() -> Self {
        let path_dirs = env::var("PATH")
            .unwrap_or_default()
            .split(':')
            .map(String::from)
            .collect();

        ExecutableResolver {
            builtins: Self::default_builtins(),
            path_cache: HashMap::new(),
            path_dirs,
        }
    }

    /// Create a resolver with custom PATH (for testing)
    #[cfg(test)]
    pub fn with_path(path_dirs: Vec<String>) -> Self {
        ExecutableResolver {
            builtins: Self::default_builtins(),
            path_cache: HashMap::new(),
            path_dirs,
        }
    }

    /// Check if a word is an hsab builtin (stack/path op)
    pub fn is_hsab_builtin(word: &str) -> bool {
        HSAB_BUILTINS.contains(&word)
    }

    /// Check if a word is a stack operation
    pub fn is_stack_op(word: &str) -> bool {
        STACK_OPS.contains(&word)
    }

    /// Check if a word is a path operation
    pub fn is_path_op(word: &str) -> bool {
        PATH_OPS.contains(&word)
    }

    /// Check if a word is an executable command
    pub fn is_executable(&mut self, word: &str) -> bool {
        // Skip flags (starting with -)
        if word.starts_with('-') {
            return false;
        }

        // Skip quoted strings
        if word.starts_with('"') || word.starts_with('\'') {
            return false;
        }

        // Skip variables
        if word.starts_with('$') {
            return false;
        }

        // Check if it's a path (contains /) - might be an executable file
        if word.contains('/') {
            return self.is_executable_path(word);
        }

        // Skip hsab builtins (stack/path ops) - they're handled specially
        if Self::is_hsab_builtin(word) {
            return false;
        }

        // Check builtins first (fast)
        if self.builtins.contains(word) {
            return true;
        }

        // Check PATH cache
        if let Some(&result) = self.path_cache.get(word) {
            return result;
        }

        // Scan PATH, cache result
        let exists = self.check_path(word);
        self.path_cache.insert(word.to_string(), exists);
        exists
    }

    /// Check if a path (like ./script.sh or /usr/bin/foo) is an executable file
    fn is_executable_path(&self, path_str: &str) -> bool {
        let path = Path::new(path_str);
        if path.is_file() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(metadata) = path.metadata() {
                    return metadata.permissions().mode() & 0o111 != 0;
                }
            }
            #[cfg(not(unix))]
            {
                return true;
            }
        }
        false
    }

    /// Check if an executable exists in PATH
    fn check_path(&self, name: &str) -> bool {
        for dir in &self.path_dirs {
            let path = Path::new(dir).join(name);
            if path.is_file() {
                // Check if executable (on Unix)
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Ok(metadata) = path.metadata() {
                        if metadata.permissions().mode() & 0o111 != 0 {
                            return true;
                        }
                    }
                }
                #[cfg(not(unix))]
                {
                    return true;
                }
            }
        }
        false
    }

    /// Default set of common shell commands
    fn default_builtins() -> HashSet<&'static str> {
        [
            // File operations
            "ls", "cat", "head", "tail", "less", "more", "file", "stat",
            "cp", "mv", "rm", "mkdir", "rmdir", "touch", "ln", "chmod", "chown",
            // Text processing
            "grep", "sed", "awk", "sort", "uniq", "cut", "wc", "tr", "tee",
            "diff", "comm", "join", "paste", "fold", "fmt", "column",
            // Search and find
            "find", "xargs", "locate", "which", "whereis", "type",
            // Output
            "echo", "printf", "read", "cat",
            // Tests and logic
            "test", "true", "false", "[", "[[",
            // Directory navigation
            "cd", "pwd", "pushd", "popd", "dirs",
            // Disk usage
            "du", "df", "mount", "umount",
            // Process management
            "ps", "kill", "killall", "top", "htop", "bg", "fg", "jobs",
            "nohup", "time", "timeout", "sleep", "wait",
            // Network
            "ssh", "scp", "rsync", "curl", "wget", "ping", "netstat", "nc",
            // Archives
            "tar", "gzip", "gunzip", "zip", "unzip", "bzip2", "xz",
            // Version control
            "git", "svn", "hg",
            // Build tools
            "make", "cmake", "cargo", "npm", "yarn", "pnpm", "pip",
            // Languages/runtimes
            "python", "python3", "ruby", "perl", "node", "deno", "bun",
            "java", "javac", "go", "rustc",
            // Editors
            "vim", "vi", "nano", "emacs", "ed",
            // Documentation
            "man", "info", "help",
            // Date/time
            "date", "cal",
            // Math
            "bc", "expr", "seq",
            // Misc utilities
            "shuf", "rev", "yes", "env", "export", "set", "unset",
            "alias", "unalias", "history", "source", ".",
            "basename", "dirname", "realpath", "readlink",
            "id", "whoami", "groups", "hostname", "uname",
            "clear", "reset", "tput",
            // Shell builtins/keywords
            "if", "then", "else", "elif", "fi",
            "for", "while", "until", "do", "done",
            "case", "esac", "select",
            "function", "return", "exit",
            "break", "continue",
            "local", "declare", "typeset", "readonly",
            "eval", "exec", "trap",
            // macOS specific
            "open", "pbcopy", "pbpaste", "say", "sw_vers",
        ].into_iter().collect()
    }

    /// Add custom commands to the builtin set (for testing or extension)
    #[allow(dead_code)]
    pub fn add_builtin(&mut self, cmd: &'static str) {
        self.builtins.insert(cmd);
    }

    /// Clear the PATH cache
    #[allow(dead_code)]
    pub fn clear_cache(&mut self) {
        self.path_cache.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtins_detected() {
        let mut resolver = ExecutableResolver::new();
        assert!(resolver.is_executable("ls"));
        assert!(resolver.is_executable("grep"));
        assert!(resolver.is_executable("cat"));
        assert!(resolver.is_executable("echo"));
    }

    #[test]
    fn test_flags_not_executable() {
        let mut resolver = ExecutableResolver::new();
        assert!(!resolver.is_executable("-la"));
        assert!(!resolver.is_executable("--help"));
        assert!(!resolver.is_executable("-n"));
    }

    #[test]
    fn test_random_words_not_executable() {
        let mut resolver = ExecutableResolver::with_path(vec![]);
        assert!(!resolver.is_executable("hello"));
        assert!(!resolver.is_executable("world"));
        assert!(!resolver.is_executable("foo"));
    }

    #[test]
    fn test_variables_not_executable() {
        let mut resolver = ExecutableResolver::new();
        assert!(!resolver.is_executable("$HOME"));
        assert!(!resolver.is_executable("${PATH}"));
    }

    #[test]
    fn test_quoted_not_executable() {
        let mut resolver = ExecutableResolver::new();
        assert!(!resolver.is_executable("\"hello\""));
        assert!(!resolver.is_executable("'world'"));
    }

    #[test]
    fn test_executable_paths() {
        let mut resolver = ExecutableResolver::new();
        // Executable paths are now detected
        assert!(resolver.is_executable("/bin/ls"));
        // Non-executable files are not
        assert!(!resolver.is_executable("src/main.rs")); // Not +x
        assert!(!resolver.is_executable("/nonexistent/path")); // Doesn't exist
    }

    #[test]
    fn test_path_lookup_cached() {
        let mut resolver = ExecutableResolver::new();

        // First lookup
        let result1 = resolver.is_executable("nonexistent_cmd_xyz");
        // Second lookup should hit cache
        let result2 = resolver.is_executable("nonexistent_cmd_xyz");

        assert_eq!(result1, result2);
        assert!(resolver.path_cache.contains_key("nonexistent_cmd_xyz"));
    }
}
