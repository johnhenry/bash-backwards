use super::{Evaluator, EvalError};
use crate::ast::Value;
use std::path::PathBuf;

impl Evaluator {
    /// Resolve a path to its canonical absolute form
    /// Handles .., ., symlinks, and relative paths
    /// ".." realpath -> /parent/of/cwd
    /// "." realpath -> /current/dir
    /// "../foo" realpath -> /parent/foo
    pub(crate) fn path_realpath(&mut self) -> Result<(), EvalError> {
        let path_str = self.pop_string()?;
        let path = PathBuf::from(&path_str);

        // If relative, make it relative to cwd
        let absolute = if path.is_relative() {
            self.cwd.join(&path)
        } else {
            path
        };

        // Try to canonicalize (resolves symlinks, .., .)
        // Fall back to lexical normalization if path doesn't exist
        let resolved = match absolute.canonicalize() {
            Ok(p) => p,
            Err(_) => {
                // Path doesn't exist, do lexical normalization
                Self::normalize_path(&absolute)
            }
        };

        self.stack.push(Value::Literal(resolved.to_string_lossy().to_string()));
        Ok(())
    }

    /// Lexical path normalization (no filesystem access)
    /// Handles . and .. components without requiring path to exist
    fn normalize_path(path: &PathBuf) -> PathBuf {
        use std::path::Component;
        let mut components = Vec::new();

        for component in path.components() {
            match component {
                Component::ParentDir => {
                    // Pop if we can, unless we're at root
                    if !components.is_empty() {
                        if let Some(Component::Normal(_)) = components.last() {
                            components.pop();
                        } else {
                            components.push(component);
                        }
                    }
                }
                Component::CurDir => {
                    // Skip . components
                }
                _ => {
                    components.push(component);
                }
            }
        }

        components.iter().collect()
    }
    pub(crate) fn path_join(&mut self) -> Result<(), EvalError> {
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

    pub(crate) fn path_suffix(&mut self) -> Result<(), EvalError> {
        let suffix = self.pop_string()?;
        let base = self.pop_string()?;
        self.stack.push(Value::Literal(format!("{}{}", base, suffix)));
        Ok(())
    }

    /// Get directory name: /path/to/file.txt -> /path/to
    pub(crate) fn path_dirname(&mut self) -> Result<(), EvalError> {
        let path = self.pop_string()?;
        let result = match path.rfind('/') {
            Some(0) => "/".to_string(),        // Root: /file -> /
            Some(idx) => path[..idx].to_string(),
            None => ".".to_string(),            // No slash: file -> .
        };
        self.stack.push(Value::Literal(result));
        Ok(())
    }

    /// Get base name without extension: /path/to/file.txt -> file
    pub(crate) fn path_basename(&mut self) -> Result<(), EvalError> {
        let path = self.pop_string()?;
        // First get the filename (after last /)
        let filename = match path.rfind('/') {
            Some(idx) => &path[idx + 1..],
            None => &path,
        };
        // Then remove extension (after first .)
        let basename = match filename.find('.') {
            Some(idx) if idx > 0 => &filename[..idx],
            _ => filename,
        };
        self.stack.push(Value::Literal(basename.to_string()));
        Ok(())
    }

    /// reext: Replace extension
    /// path newext reext -> path with new extension
    /// "file.txt" ".md" reext -> "file.md"
    pub(crate) fn builtin_reext(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.len() < 2 {
            return Err(EvalError::ExecError("reext: path and new extension required".into()));
        }
        self.restore_excess_args(args, 2);
        // Args in LIFO: [newext, path] for "path newext reext"
        let new_ext = &args[0];
        let path_str = &args[1];

        // Split at last dot, replace extension
        let result = if let Some(dot_pos) = path_str.rfind('.') {
            format!("{}{}", &path_str[..dot_pos], new_ext)
        } else {
            // No extension, just append the new one
            format!("{}{}", path_str, new_ext)
        };

        self.stack.push(Value::Literal(result));
        self.last_exit_code = 0;
        Ok(())
    }
}
