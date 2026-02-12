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
pub const PATH_OPS: &[&str] = &["path-join", "suffix"];

/// All hsab builtins (stack + path + list + control + parallel + JSON ops)
pub const HSAB_BUILTINS: &[&str] = &[
    // Stack ops
    "dup", "swap", "drop", "over", "rot", "depth",
    // Path ops
    "path-join", "suffix",
    // String ops
    "split1", "rsplit1",
    // List ops
    "marker", "spread", "each", "collect", "keep",
    // Control ops
    "if", "times", "while", "until", "break",
    // Parallel ops
    "parallel", "fork",
    // Process substitution
    "subst", "fifo",
    // JSON / Structured data
    "json", "unjson",
    // Resource limits
    "timeout",
    // Pipeline status
    "pipestatus",
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
        // Check builtins FIRST - this catches special chars like +, -, *, /, %
        if self.builtins.contains(word) {
            return true;
        }

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
        self.find_in_path(name).is_some()
    }

    /// Find an executable in PATH and return its full path
    fn find_in_path(&self, name: &str) -> Option<String> {
        for dir in &self.path_dirs {
            let path = Path::new(dir).join(name);
            if path.is_file() {
                // Check if executable (on Unix)
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Ok(metadata) = path.metadata() {
                        if metadata.permissions().mode() & 0o111 != 0 {
                            return Some(path.to_string_lossy().to_string());
                        }
                    }
                }
                #[cfg(not(unix))]
                {
                    return Some(path.to_string_lossy().to_string());
                }
            }
        }
        None
    }

    /// Find executable by name, returning full path
    pub fn find_executable(&mut self, name: &str) -> Option<String> {
        // Check if it's a path (contains /)
        if name.contains('/') {
            let path = Path::new(name);
            if path.is_file() {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Ok(metadata) = path.metadata() {
                        if metadata.permissions().mode() & 0o111 != 0 {
                            return Some(name.to_string());
                        }
                    }
                }
                #[cfg(not(unix))]
                {
                    return Some(name.to_string());
                }
            }
            return None;
        }

        // Search PATH
        self.find_in_path(name)
    }

    /// Default set of common shell commands
    fn default_builtins() -> HashSet<&'static str> {
        // Only include hsab's own shell builtins - these are implemented in try_builtin
        // and should always be recognized as executables regardless of PATH.
        // All other commands are discovered via PATH lookup.
        // Note: "." is NOT included here - it's handled specially in eval.rs
        // so that "." alone is treated as current directory literal, but
        // "file.hsab ." works as source command.
        [
            // Core shell builtins implemented in hsab
            "cd", "pwd", "echo", "test", "true", "false", "[",
            "export", "unset", "env", "jobs", "fg", "bg", "exit",
            "tty", "source", "hash", "type", "which",
            // New builtins
            "read", "printf", "wait", "kill",
            "pushd", "popd", "dirs",
            "alias", "unalias",
            "trap", "local", "return",
            // Stack-native predicates
            "file?", "dir?", "exists?", "empty?",
            "eq?", "ne?", "=?", "!=?",
            "lt?", "gt?", "le?", "ge?",
            // Arithmetic primitives
            "plus", "minus", "mul", "div", "mod",
            // String primitives
            "len", "slice", "indexof", "str-replace",
            // Phase 0: Type introspection
            "typeof",
            // Phase 1: Record operations
            "record", "get", "set", "del", "has?", "keys", "values", "merge",
            // Phase 2: Table operations
            "table", "where", "sort-by", "select", "first", "last", "nth",
            // Phase 3: Error handling
            "try", "error?", "throw",
            // Phase 4: Serialization bridge
            "into-json", "into-csv", "into-lines", "into-kv",
            "to-json", "to-csv", "to-lines",
            // Phase 5: Stack utilities
            "tap", "dip",
            // Phase 6: Aggregations
            "sum", "avg", "min", "max", "count",
            // Phase 8: Extended table ops
            "group-by", "unique", "reverse", "flatten",
            // Phase 11: Additional parsers
            "into-tsv", "into-delimited",
        ].into_iter().collect()
    }

    /// Add custom commands to the builtin set (for testing or extension)
    #[allow(dead_code)]
    pub fn add_builtin(&mut self, cmd: &'static str) {
        self.builtins.insert(cmd);
    }

    /// Clear the PATH cache
    pub fn clear_cache(&mut self) {
        self.path_cache.clear();
    }

    /// Force resolve a command and cache it (for hash builtin)
    pub fn resolve_and_cache(&mut self, name: &str) {
        // If already cached, skip
        if self.path_cache.contains_key(name) {
            return;
        }

        // Look up in PATH and cache result
        let exists = self.check_path(name);
        self.path_cache.insert(name.to_string(), exists);
    }

    /// Get all cached entries with their resolved paths (for hash builtin)
    pub fn get_cache_entries(&self) -> Vec<(String, String)> {
        let mut entries = Vec::new();
        for (cmd, &exists) in &self.path_cache {
            if exists {
                // Find the actual path
                if let Some(path) = self.find_in_path(cmd) {
                    entries.push((cmd.clone(), path));
                }
            }
        }
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        entries
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
