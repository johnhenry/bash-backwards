//! Watch mode for hsab
//!
//! Watches files for changes and re-runs a block when changes occur.
//!
//! Usage:
//!   "src/*.rs" [cargo build] watch        # Watch with defaults
//!   "src/*.rs" [cargo build] 500 watch    # Watch with 500ms debounce

use super::{Evaluator, EvalError};
use crate::ast::{Expr, Value};
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher, Event};
use std::path::Path;
use std::sync::mpsc::{channel, RecvTimeoutError};
use std::time::{Duration, Instant};
use std::collections::HashSet;

impl Evaluator {
    /// watch: "pattern" [block] watch -> (blocks until Ctrl+C)
    /// Watch files matching pattern, re-run block on changes
    pub(crate) fn builtin_watch(&mut self) -> Result<(), EvalError> {
        // Pop arguments: [block] pattern (or [block] debounce pattern)
        let block = self.pop_block()?;

        // Check for optional debounce value
        let (pattern, debounce_ms) = if let Some(Value::Number(_)) = self.stack.last() {
            let debounce = self.stack.pop().unwrap();
            let d = if let Value::Number(n) = debounce { n as u64 } else { 200 };
            let p = self.stack.pop()
                .ok_or_else(|| EvalError::StackUnderflow("watch requires pattern".into()))?
                .as_arg()
                .ok_or_else(|| EvalError::TypeError {
                    expected: "pattern string".into(),
                    got: "non-string".into(),
                })?;
            (p, d)
        } else {
            let p = self.stack.pop()
                .ok_or_else(|| EvalError::StackUnderflow("watch requires pattern".into()))?
                .as_arg()
                .ok_or_else(|| EvalError::TypeError {
                    expected: "pattern string".into(),
                    got: "non-string".into(),
                })?;
            (p, 200) // Default 200ms debounce
        };

        // Run the watch loop
        self.run_watch_loop(&pattern, &block, debounce_ms)
    }

    /// Run the watch loop - blocks until interrupted
    fn run_watch_loop(
        &mut self,
        pattern: &str,
        block: &[Expr],
        debounce_ms: u64,
    ) -> Result<(), EvalError> {
        // Resolve the pattern to find which directories to watch
        let (watch_paths, glob_pattern) = self.resolve_watch_pattern(pattern)?;

        if watch_paths.is_empty() {
            return Err(EvalError::ExecError(format!(
                "watch: no directories found for pattern '{}'", pattern
            )));
        }

        // Create channel for file events
        let (tx, rx) = channel();

        // Create watcher
        let mut watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    let _ = tx.send(event);
                }
            },
            Config::default(),
        ).map_err(|e| EvalError::ExecError(format!("watch: failed to create watcher: {}", e)))?;

        // Watch the directories (canonicalize to handle symlinks like /tmp -> /private/tmp)
        for path in &watch_paths {
            let watch_path = Path::new(path);
            let canonical_path = watch_path.canonicalize().unwrap_or_else(|_| watch_path.to_path_buf());
            watcher.watch(&canonical_path, RecursiveMode::Recursive)
                .map_err(|e| EvalError::ExecError(format!(
                    "watch: failed to watch '{}': {}", path, e
                )))?;
        }

        // Compile the glob pattern for filtering
        let glob = glob::Pattern::new(&glob_pattern)
            .map_err(|e| EvalError::ExecError(format!(
                "watch: invalid pattern '{}': {}", pattern, e
            )))?;

        // Print initial message
        eprintln!("\x1b[36m◉ Watching: {}\x1b[0m", pattern);
        eprintln!("\x1b[90m  Press Ctrl+C to stop\x1b[0m");

        // Run block initially
        eprintln!("\x1b[33m▶ Running initial build...\x1b[0m");
        let start = Instant::now();
        match self.run_block_capture(block) {
            Ok(_) => {
                let elapsed = start.elapsed();
                eprintln!("\x1b[32m✓ Completed in {:.2}s\x1b[0m", elapsed.as_secs_f64());
            }
            Err(e) => {
                eprintln!("\x1b[31m✗ Failed: {}\x1b[0m", e);
            }
        }

        // Debounce state
        let debounce = Duration::from_millis(debounce_ms);
        let mut last_run = Instant::now();
        let mut pending_changes: HashSet<String> = HashSet::new();

        // Watch loop
        loop {
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(event) => {
                    // Filter events by our glob pattern
                    for path in event.paths {
                        if let Some(path_str) = path.to_str() {
                            // Check if path matches our glob
                            if glob.matches(path_str) || self.path_matches_pattern(path_str, &glob_pattern) {
                                pending_changes.insert(path_str.to_string());
                            }
                        }
                    }
                }
                Err(RecvTimeoutError::Timeout) => {
                    // Check if we have pending changes and debounce time has passed
                    if !pending_changes.is_empty() && last_run.elapsed() >= debounce {
                        // Clear terminal line and show what changed
                        let changed: Vec<_> = pending_changes.drain().collect();
                        eprintln!("\n\x1b[33m▶ Changed: {}\x1b[0m",
                            changed.iter()
                                .map(|p| Path::new(p).file_name()
                                    .map(|n| n.to_string_lossy().to_string())
                                    .unwrap_or_else(|| p.clone()))
                                .collect::<Vec<_>>()
                                .join(", "));

                        // Run the block
                        let start = Instant::now();
                        match self.run_block_capture(block) {
                            Ok(_) => {
                                let elapsed = start.elapsed();
                                eprintln!("\x1b[32m✓ Completed in {:.2}s\x1b[0m", elapsed.as_secs_f64());
                            }
                            Err(e) => {
                                eprintln!("\x1b[31m✗ Failed: {}\x1b[0m", e);
                            }
                        }

                        last_run = Instant::now();
                    }
                }
                Err(RecvTimeoutError::Disconnected) => {
                    // Channel closed, exit
                    break;
                }
            }

            // Check for Ctrl+C (the ctrlc handler will set a flag)
            // For now, we rely on the process being killed
            // A more sophisticated approach would use a shared atomic flag
        }

        eprintln!("\n\x1b[36m◉ Watch stopped\x1b[0m");
        Ok(())
    }

    /// Resolve a watch pattern to directories to watch and a glob pattern
    fn resolve_watch_pattern(&self, pattern: &str) -> Result<(Vec<String>, String), EvalError> {
        // Handle patterns like "src/*.rs", "**/*.rs", "src/main.rs"
        let path = Path::new(pattern);

        // Find the first component that contains a glob character
        let mut watch_dir = self.cwd.clone();
        let mut glob_start = 0;
        let mut glob_suffix = String::new();

        for (i, component) in path.components().enumerate() {
            let comp_str = component.as_os_str().to_string_lossy();
            if comp_str.contains('*') || comp_str.contains('?') || comp_str.contains('[') {
                glob_start = i;
                // Collect the glob portion of the pattern
                glob_suffix = path.components()
                    .skip(i)
                    .map(|c| c.as_os_str().to_string_lossy().to_string())
                    .collect::<Vec<_>>()
                    .join("/");
                break;
            }
            // Only add non-glob components to watch dir
            if i == 0 && comp_str != "." {
                watch_dir = self.cwd.join(component);
            } else if i > 0 {
                watch_dir = watch_dir.join(component);
            }
            glob_start = i + 1;
        }

        // If pattern starts with glob, watch cwd
        let watch_path = if glob_start == 0 {
            self.cwd.to_string_lossy().to_string()
        } else {
            watch_dir.to_string_lossy().to_string()
        };

        // Build the full glob pattern, canonicalizing the base directory
        // This handles symlinks like /tmp -> /private/tmp on macOS
        let full_pattern = if glob_start == 0 {
            // Pattern starts with glob, use cwd
            let canonical_cwd = self.cwd.canonicalize()
                .unwrap_or_else(|_| self.cwd.clone());
            format!("{}/{}", canonical_cwd.display(), pattern)
        } else if path.is_absolute() {
            // Absolute path - canonicalize the base directory
            let canonical_base = watch_dir.canonicalize()
                .unwrap_or(watch_dir);
            if glob_suffix.is_empty() {
                canonical_base.to_string_lossy().to_string()
            } else {
                format!("{}/{}", canonical_base.display(), glob_suffix)
            }
        } else {
            // Relative path - join with cwd and canonicalize base
            let abs_base = self.cwd.join(&watch_dir);
            let canonical_base = abs_base.canonicalize()
                .unwrap_or(abs_base);
            if glob_suffix.is_empty() {
                canonical_base.to_string_lossy().to_string()
            } else {
                format!("{}/{}", canonical_base.display(), glob_suffix)
            }
        };

        Ok((vec![watch_path], full_pattern))
    }

    /// Check if a path matches a pattern (handles ** and other glob features)
    fn path_matches_pattern(&self, path: &str, pattern: &str) -> bool {
        // Simple matching - check if the path ends with relevant parts
        let path_parts: Vec<&str> = path.split('/').collect();
        let pattern_parts: Vec<&str> = pattern.split('/').collect();

        // Handle ** wildcard
        if pattern.contains("**") {
            // For **, just check if filename matches
            if let Some(last_pattern) = pattern_parts.last() {
                if let Some(last_path) = path_parts.last() {
                    if let Ok(glob) = glob::Pattern::new(last_pattern) {
                        return glob.matches(last_path);
                    }
                }
            }
        }

        false
    }

    /// Run a block and print output (for watch mode)
    fn run_block_capture(&mut self, block: &[Expr]) -> Result<(), EvalError> {
        // Save capture mode
        let old_capture = self.capture_mode;
        self.capture_mode = false; // Allow interactive output

        // Run each expression in the block
        for expr in block {
            self.eval_expr(expr)?;
        }

        // Print any Output values on the stack (from echo, commands, etc.)
        self.flush_output_to_stdout();

        // Restore capture mode
        self.capture_mode = old_capture;
        Ok(())
    }

    /// Flush Output values from stack to stdout
    fn flush_output_to_stdout(&mut self) {
        use std::io::Write;
        let mut new_stack = Vec::new();
        for value in self.stack.drain(..) {
            match value {
                Value::Output(s) => {
                    print!("{}", s);
                }
                other => new_stack.push(other),
            }
        }
        self.stack = new_stack;
        let _ = std::io::stdout().flush();
    }
}
