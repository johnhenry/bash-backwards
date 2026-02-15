//! Stack-native shell operations
//!
//! These operations return useful values to the stack instead of being side-effect only.
//! On error, they return nil (compositional, pipelines don't break).

use super::{Evaluator, EvalError};
use crate::ast::Value;
use std::fs;
use std::path::Path;

impl Evaluator {
    // ============================================
    // File Creation
    // ============================================

    /// touch: "path" → path (or nil on error)
    /// Creates an empty file, returns the canonical path
    pub(crate) fn builtin_touch(&mut self) -> Result<(), EvalError> {
        let path_str = self.pop_string()?;
        let path = Path::new(&path_str);

        match fs::File::create(path) {
            Ok(_) => {
                let canonical = path.canonicalize()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| path_str.clone());
                self.stack.push(Value::Literal(canonical));
            }
            Err(_) => {
                self.stack.push(Value::Nil);
            }
        }
        Ok(())
    }

    /// mkdir: "path" → path (or nil on error)
    /// Creates a directory, returns the canonical path
    pub(crate) fn builtin_mkdir_native(&mut self) -> Result<(), EvalError> {
        let path_str = self.pop_string()?;
        let path = Path::new(&path_str);

        match fs::create_dir(path) {
            Ok(_) => {
                let canonical = path.canonicalize()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| path_str.clone());
                self.stack.push(Value::Literal(canonical));
            }
            Err(_) => {
                self.stack.push(Value::Nil);
            }
        }
        Ok(())
    }

    /// mkdir-p: "path" → path (or nil on error)
    /// Creates a directory and all parent directories
    pub(crate) fn builtin_mkdir_p(&mut self) -> Result<(), EvalError> {
        let path_str = self.pop_string()?;
        let path = Path::new(&path_str);

        match fs::create_dir_all(path) {
            Ok(_) => {
                let canonical = path.canonicalize()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| path_str.clone());
                self.stack.push(Value::Literal(canonical));
            }
            Err(_) => {
                self.stack.push(Value::Nil);
            }
        }
        Ok(())
    }

    /// mktemp: → path (or nil on error)
    /// Creates a temporary file, returns the path
    pub(crate) fn builtin_mktemp(&mut self) -> Result<(), EvalError> {
        let tmp_dir = std::env::temp_dir();
        let unique_name = format!("hsab-{}", std::process::id());
        let tmp_path = tmp_dir.join(unique_name);

        // Generate a unique name
        let mut counter = 0;
        let mut path = tmp_path.clone();
        while path.exists() {
            counter += 1;
            path = tmp_dir.join(format!("hsab-{}-{}", std::process::id(), counter));
        }

        match fs::File::create(&path) {
            Ok(_) => {
                let canonical = path.canonicalize()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| path.to_string_lossy().to_string());
                self.stack.push(Value::Literal(canonical));
            }
            Err(_) => {
                self.stack.push(Value::Nil);
            }
        }
        Ok(())
    }

    /// mktemp-d: → path (or nil on error)
    /// Creates a temporary directory, returns the path
    pub(crate) fn builtin_mktemp_d(&mut self) -> Result<(), EvalError> {
        let tmp_dir = std::env::temp_dir();
        let unique_name = format!("hsab-dir-{}", std::process::id());

        // Generate a unique name
        let mut counter = 0;
        let mut path = tmp_dir.join(&unique_name);
        while path.exists() {
            counter += 1;
            path = tmp_dir.join(format!("hsab-dir-{}-{}", std::process::id(), counter));
        }

        match fs::create_dir(&path) {
            Ok(_) => {
                let canonical = path.canonicalize()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| path.to_string_lossy().to_string());
                self.stack.push(Value::Literal(canonical));
            }
            Err(_) => {
                self.stack.push(Value::Nil);
            }
        }
        Ok(())
    }

    // ============================================
    // File Operations
    // ============================================

    /// cp: "src" "dst" → dst_path (or nil on error)
    /// Copies a file, returns the destination path
    pub(crate) fn builtin_cp(&mut self) -> Result<(), EvalError> {
        let dst = self.pop_string()?;
        let src = self.pop_string()?;

        match fs::copy(&src, &dst) {
            Ok(_) => {
                let dst_path = Path::new(&dst);
                let canonical = dst_path.canonicalize()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or(dst);
                self.stack.push(Value::Literal(canonical));
            }
            Err(_) => {
                self.stack.push(Value::Nil);
            }
        }
        Ok(())
    }

    /// mv: "src" "dst" → dst_path (or nil on error)
    /// Moves/renames a file, returns the destination path
    pub(crate) fn builtin_mv(&mut self) -> Result<(), EvalError> {
        let dst = self.pop_string()?;
        let src = self.pop_string()?;

        match fs::rename(&src, &dst) {
            Ok(_) => {
                let dst_path = Path::new(&dst);
                let canonical = dst_path.canonicalize()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or(dst);
                self.stack.push(Value::Literal(canonical));
            }
            Err(_) => {
                self.stack.push(Value::Nil);
            }
        }
        Ok(())
    }

    /// rm: "path" → count (or nil on error)
    /// Removes a file, returns 1 on success
    pub(crate) fn builtin_rm(&mut self) -> Result<(), EvalError> {
        let path_str = self.pop_string()?;
        let path = Path::new(&path_str);

        // Check if it's a glob pattern
        if path_str.contains('*') || path_str.contains('?') || path_str.contains('[') {
            // Glob expansion
            let mut count = 0;
            if let Ok(entries) = glob::glob(&path_str) {
                for entry in entries.flatten() {
                    if entry.is_file() && fs::remove_file(&entry).is_ok() {
                        count += 1;
                    }
                }
            }
            if count > 0 {
                self.stack.push(Value::Number(count as f64));
            } else {
                self.stack.push(Value::Nil);
            }
        } else {
            // Single file
            match fs::remove_file(path) {
                Ok(_) => {
                    self.stack.push(Value::Number(1.0));
                }
                Err(_) => {
                    self.stack.push(Value::Nil);
                }
            }
        }
        Ok(())
    }

    /// rm-r: "path" → count (or nil on error)
    /// Recursively removes a directory, returns count of items removed
    pub(crate) fn builtin_rm_r(&mut self) -> Result<(), EvalError> {
        let path_str = self.pop_string()?;
        let path = Path::new(&path_str);

        if path.is_dir() {
            // Count items before removal
            let count = Self::count_items(path);
            match fs::remove_dir_all(path) {
                Ok(_) => {
                    self.stack.push(Value::Number(count as f64));
                }
                Err(_) => {
                    self.stack.push(Value::Nil);
                }
            }
        } else if path.is_file() {
            match fs::remove_file(path) {
                Ok(_) => {
                    self.stack.push(Value::Number(1.0));
                }
                Err(_) => {
                    self.stack.push(Value::Nil);
                }
            }
        } else {
            self.stack.push(Value::Nil);
        }
        Ok(())
    }

    /// Helper: count items in a directory recursively
    fn count_items(path: &Path) -> usize {
        let mut count = 0;
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                count += 1;
                if entry.path().is_dir() {
                    count += Self::count_items(&entry.path());
                }
            }
        }
        count + 1 // Include the directory itself
    }

    /// ln: "target" "link" → link_path (or nil on error)
    /// Creates a symbolic link
    #[cfg(unix)]
    pub(crate) fn builtin_ln(&mut self) -> Result<(), EvalError> {
        let link = self.pop_string()?;
        let target = self.pop_string()?;

        match std::os::unix::fs::symlink(&target, &link) {
            Ok(_) => {
                let link_path = Path::new(&link);
                // Don't canonicalize symlinks (would resolve them)
                let abs_path = if link_path.is_absolute() {
                    link.clone()
                } else {
                    self.cwd.join(&link).to_string_lossy().to_string()
                };
                self.stack.push(Value::Literal(abs_path));
            }
            Err(_) => {
                self.stack.push(Value::Nil);
            }
        }
        Ok(())
    }

    #[cfg(not(unix))]
    pub(crate) fn builtin_ln(&mut self) -> Result<(), EvalError> {
        let _ = self.pop_string()?;
        let _ = self.pop_string()?;
        self.stack.push(Value::Nil);
        Ok(())
    }

    /// realpath: "path" → canonical_path (or nil on error)
    /// Returns the canonical absolute path
    pub(crate) fn builtin_realpath(&mut self) -> Result<(), EvalError> {
        let path_str = self.pop_string()?;
        let path = Path::new(&path_str);

        // If relative, join with cwd first
        let full_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.cwd.join(path)
        };

        match full_path.canonicalize() {
            Ok(canonical) => {
                self.stack.push(Value::Literal(canonical.to_string_lossy().to_string()));
            }
            Err(_) => {
                self.stack.push(Value::Nil);
            }
        }
        Ok(())
    }

    // ============================================
    // Directory Operations
    // ============================================

    /// cd (stack-native): "path" → new_path (or nil on error)
    /// Changes directory and returns the new canonical path
    /// If no argument on stack, defaults to home directory
    pub(crate) fn builtin_cd_native(&mut self) -> Result<(), EvalError> {
        // If stack is empty, use home directory
        let path_str = if self.stack.is_empty() {
            self.home_dir.clone()
        } else {
            self.pop_string()?
        };

        // Handle ~ expansion using existing home_dir field
        let expanded = if path_str.starts_with('~') {
            if path_str == "~" {
                self.home_dir.clone()
            } else if path_str.starts_with("~/") {
                format!("{}{}", self.home_dir, &path_str[1..])
            } else {
                path_str.clone()
            }
        } else {
            path_str.clone()
        };

        let path = Path::new(&expanded);
        let target = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.cwd.join(path)
        };

        match target.canonicalize() {
            Ok(canonical) => {
                if canonical.is_dir() {
                    self.cwd = canonical.clone();
                    std::env::set_current_dir(&canonical).ok();
                    self.stack.push(Value::Literal(canonical.to_string_lossy().to_string()));
                } else {
                    self.stack.push(Value::Nil);
                }
            }
            Err(_) => {
                self.stack.push(Value::Nil);
            }
        }
        Ok(())
    }

    /// which: "cmd" → path (or nil if not found)
    /// Returns the full path of a command
    pub(crate) fn builtin_which_native(&mut self) -> Result<(), EvalError> {
        let cmd = self.pop_string()?;

        match self.resolver.find_executable(&cmd) {
            Some(path) => {
                self.stack.push(Value::Literal(path));
            }
            None => {
                self.stack.push(Value::Nil);
            }
        }
        Ok(())
    }

    // ============================================
    // Path Parts
    // ============================================

    /// dirname: "path" → directory_portion
    pub(crate) fn builtin_dirname_native(&mut self) -> Result<(), EvalError> {
        let path_str = self.pop_string()?;
        let path = Path::new(&path_str);

        match path.parent() {
            Some(parent) => {
                let result = parent.to_string_lossy().to_string();
                self.stack.push(Value::Literal(if result.is_empty() { ".".to_string() } else { result }));
            }
            None => {
                self.stack.push(Value::Literal(".".to_string()));
            }
        }
        Ok(())
    }

    /// basename: "path" → filename_portion
    pub(crate) fn builtin_basename_native(&mut self) -> Result<(), EvalError> {
        let path_str = self.pop_string()?;
        let path = Path::new(&path_str);

        match path.file_name() {
            Some(name) => {
                self.stack.push(Value::Literal(name.to_string_lossy().to_string()));
            }
            None => {
                self.stack.push(Value::Literal(String::new()));
            }
        }
        Ok(())
    }

    /// extname: "path" → extension (including dot, e.g., ".txt")
    pub(crate) fn builtin_extname(&mut self) -> Result<(), EvalError> {
        let path_str = self.pop_string()?;
        let path = Path::new(&path_str);

        match path.extension() {
            Some(ext) => {
                self.stack.push(Value::Literal(format!(".{}", ext.to_string_lossy())));
            }
            None => {
                self.stack.push(Value::Literal(String::new()));
            }
        }
        Ok(())
    }

    // ============================================
    // Enhanced Listing
    // ============================================

    /// ls (stack-native): ["dir"] → [filenames]
    /// Returns a list of filenames in the directory
    pub(crate) fn builtin_ls_native(&mut self) -> Result<(), EvalError> {
        // Check if there's a path on the stack - check and clone before popping
        let (is_dir_on_stack, dir) = match self.stack.last() {
            Some(Value::Literal(s)) | Some(Value::Output(s)) => {
                let path = Path::new(s);
                if path.is_dir() {
                    (true, s.clone())
                } else {
                    (false, ".".to_string())
                }
            }
            _ => (false, ".".to_string()),
        };

        // Pop after checking
        if is_dir_on_stack {
            self.stack.pop();
        }

        let dir = dir;

        let path = if dir == "." {
            self.cwd.clone()
        } else {
            let p = Path::new(&dir);
            if p.is_absolute() {
                p.to_path_buf()
            } else {
                self.cwd.join(p)
            }
        };

        match fs::read_dir(&path) {
            Ok(entries) => {
                let mut files: Vec<Value> = Vec::new();
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    files.push(Value::Literal(name));
                }
                files.sort_by(|a, b| {
                    a.as_arg().unwrap_or_default().cmp(&b.as_arg().unwrap_or_default())
                });
                self.stack.push(Value::List(files));
            }
            Err(_) => {
                self.stack.push(Value::Nil);
            }
        }
        Ok(())
    }

    /// glob: "pattern" → [matching_paths]
    /// Returns a list of paths matching the glob pattern
    pub(crate) fn builtin_glob(&mut self) -> Result<(), EvalError> {
        let pattern = self.pop_string()?;

        // If pattern is relative, make it absolute
        let full_pattern = if Path::new(&pattern).is_absolute() {
            pattern
        } else {
            self.cwd.join(&pattern).to_string_lossy().to_string()
        };

        match glob::glob(&full_pattern) {
            Ok(entries) => {
                let mut paths: Vec<Value> = Vec::new();
                for entry in entries.flatten() {
                    paths.push(Value::Literal(entry.to_string_lossy().to_string()));
                }
                paths.sort_by(|a, b| {
                    a.as_arg().unwrap_or_default().cmp(&b.as_arg().unwrap_or_default())
                });
                self.stack.push(Value::List(paths));
            }
            Err(_) => {
                self.stack.push(Value::Nil);
            }
        }
        Ok(())
    }
}
