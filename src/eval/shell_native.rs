//! Stack-native shell operations
//!
//! These operations return useful values to the stack instead of being side-effect only.
//! On error, they return nil (compositional, pipelines don't break).

use super::{EvalError, Evaluator};
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
                let canonical = path
                    .canonicalize()
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
                let canonical = path
                    .canonicalize()
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
                let canonical = path
                    .canonicalize()
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
                let canonical = path
                    .canonicalize()
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
                let canonical = path
                    .canonicalize()
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
                let canonical = dst_path
                    .canonicalize()
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
                let canonical = dst_path
                    .canonicalize()
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
                self.stack.push(Value::Int(count as i64));
            } else {
                self.stack.push(Value::Nil);
            }
        } else {
            // Single file
            match fs::remove_file(path) {
                Ok(_) => {
                    self.stack.push(Value::Int(1));
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
                    self.stack.push(Value::Int(count as i64));
                }
                Err(_) => {
                    self.stack.push(Value::Nil);
                }
            }
        } else if path.is_file() {
            match fs::remove_file(path) {
                Ok(_) => {
                    self.stack.push(Value::Int(1));
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
                self.stack
                    .push(Value::Literal(canonical.to_string_lossy().to_string()));
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
        let expanded = if path_str == "~" {
            self.home_dir.clone()
        } else if let Some(rest) = path_str.strip_prefix("~/") {
            format!("{}/{}", self.home_dir, rest)
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
                    self.stack
                        .push(Value::Literal(canonical.to_string_lossy().to_string()));
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

    /// extname: "path" → extension (including dot, e.g., ".txt")
    pub(crate) fn builtin_extname(&mut self) -> Result<(), EvalError> {
        let path_str = self.pop_string()?;
        let path = Path::new(&path_str);

        match path.extension() {
            Some(ext) => {
                self.stack
                    .push(Value::Literal(format!(".{}", ext.to_string_lossy())));
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
                    a.as_arg()
                        .unwrap_or_default()
                        .cmp(&b.as_arg().unwrap_or_default())
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
                    a.as_arg()
                        .unwrap_or_default()
                        .cmp(&b.as_arg().unwrap_or_default())
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

// ============================================
// Structured-returning core builtins (issue #27)
// ============================================

impl Evaluator {
    /// ls-t: [path] ls-t -> Table{name, type, size, modified}
    ///
    /// Structured directory listing. `type` is file/dir/symlink/other
    /// (symlinks are not followed). Additive: plain `ls` is unchanged.
    pub(crate) fn builtin_ls_t(&mut self) -> Result<(), EvalError> {
        use std::os::unix::fs::MetadataExt;
        use std::path::PathBuf;

        let dir_path = if let Some(val) = self.stack.last() {
            if let Some(s) = val.as_arg() {
                if !s.starts_with('-') && (Path::new(&s).exists() || s.contains('/')) {
                    self.stack.pop();
                    PathBuf::from(self.expand_tilde(&s))
                } else {
                    self.cwd.clone()
                }
            } else {
                self.cwd.clone()
            }
        } else {
            self.cwd.clone()
        };

        let entries = fs::read_dir(&dir_path).map_err(|e| {
            EvalError::IoError(std::io::Error::new(
                e.kind(),
                format!("{}: {}", dir_path.display(), e),
            ))
        })?;

        let columns = vec![
            "name".to_string(),
            "type".to_string(),
            "size".to_string(),
            "modified".to_string(),
        ];

        let mut rows: Vec<Vec<Value>> = Vec::new();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            // symlink_metadata: do not follow links, so type can be "symlink"
            let (file_type, size, modified) = match entry.path().symlink_metadata() {
                Ok(meta) => {
                    let ft = if meta.file_type().is_symlink() {
                        "symlink"
                    } else if meta.is_dir() {
                        "dir"
                    } else if meta.is_file() {
                        "file"
                    } else {
                        "other"
                    };
                    (ft.to_string(), meta.len() as i64, meta.mtime())
                }
                Err(_) => ("unknown".to_string(), 0, 0),
            };

            rows.push(vec![
                Value::Literal(name),
                Value::Literal(file_type),
                Value::Int(size),
                Value::Int(modified),
            ]);
        }

        rows.sort_by(|a, b| {
            let name_a = a.first().and_then(|v| v.as_arg()).unwrap_or_default();
            let name_b = b.first().and_then(|v| v.as_arg()).unwrap_or_default();
            name_a.cmp(&name_b)
        });

        self.stack.push(Value::Table { columns, rows });
        self.last_exit_code = 0;
        Ok(())
    }

    /// env-t: env-t -> Record of environment variables (insertion order)
    pub(crate) fn builtin_env_t(&mut self) -> Result<(), EvalError> {
        let map: indexmap::IndexMap<String, Value> = std::env::vars()
            .map(|(k, v)| (k, Value::Literal(v)))
            .collect();
        self.stack.push(Value::Map(map));
        self.last_exit_code = 0;
        Ok(())
    }

    /// which-t: "name" which-t -> Record{name, path, type}
    ///
    /// type is one of builtin / definition / executable / not-found;
    /// path is Nil unless type is executable.
    pub(crate) fn builtin_which_t(&mut self) -> Result<(), EvalError> {
        let name = self.pop_string()?;

        let (kind, path) = if crate::resolver::ExecutableResolver::is_hsab_builtin(&name) {
            ("builtin", Value::Nil)
        } else if self.definitions.contains_key(&name) {
            ("definition", Value::Nil)
        } else if let Some(p) = self.resolver.find_executable(&name) {
            ("executable", Value::Literal(p))
        } else {
            ("not-found", Value::Nil)
        };

        let mut map = indexmap::IndexMap::new();
        map.insert("name".to_string(), Value::Literal(name));
        map.insert("path".to_string(), path);
        map.insert("type".to_string(), Value::Literal(kind.to_string()));
        self.stack.push(Value::Map(map));
        self.last_exit_code = 0;
        Ok(())
    }

    /// history-t: history-t -> Table{index, command}
    ///
    /// MVP (issue #27): reads the saved REPL history file
    /// (~/.hsab_history). The current session's in-memory entries are only
    /// visible after they are flushed on REPL exit.
    pub(crate) fn builtin_history_t(&mut self) -> Result<(), EvalError> {
        let columns = vec!["index".to_string(), "command".to_string()];
        let mut rows: Vec<Vec<Value>> = Vec::new();

        let history_path = std::env::var("HOME")
            .map(|h| Path::new(&h).join(".hsab_history"))
            .ok();

        if let Some(path) = history_path {
            if let Ok(content) = fs::read_to_string(&path) {
                for (i, line) in content
                    .lines()
                    .filter(|l| !l.is_empty() && *l != "#V2")
                    .enumerate()
                {
                    rows.push(vec![Value::Int(i as i64), Value::Literal(line.to_string())]);
                }
            }
        }

        self.stack.push(Value::Table { columns, rows });
        self.last_exit_code = 0;
        Ok(())
    }

    /// ps-t: ps-t -> Table{pid, name, cpu, mem, status}
    ///
    /// cpu is cumulative CPU seconds, mem is resident set size in bytes.
    #[cfg(target_os = "linux")]
    pub(crate) fn builtin_ps_t(&mut self) -> Result<(), EvalError> {
        let columns = vec![
            "pid".to_string(),
            "name".to_string(),
            "cpu".to_string(),
            "mem".to_string(),
            "status".to_string(),
        ];
        let mut rows: Vec<Vec<Value>> = Vec::new();

        let clk_tck = 100.0; // standard USER_HZ on Linux

        if let Ok(entries) = fs::read_dir("/proc") {
            for entry in entries.flatten() {
                let fname = entry.file_name().to_string_lossy().to_string();
                let pid: i64 = match fname.parse() {
                    Ok(p) => p,
                    Err(_) => continue,
                };

                let stat = match fs::read_to_string(format!("/proc/{}/stat", pid)) {
                    Ok(s) => s,
                    Err(_) => continue,
                };

                // comm is parenthesized and may contain spaces: find the
                // enclosing parens, then split the rest
                let (name, rest) = match (stat.find('('), stat.rfind(')')) {
                    (Some(open), Some(close)) if close > open => (
                        stat[open + 1..close].to_string(),
                        stat[close + 1..].trim().to_string(),
                    ),
                    _ => continue,
                };

                let fields: Vec<&str> = rest.split_whitespace().collect();
                // fields[0] is state (field 3 of stat); utime/stime are
                // fields 14/15 of stat = fields[11]/fields[12] here;
                // rss pages = field 24 = fields[21]
                let status = fields.first().unwrap_or(&"?").to_string();
                let utime: f64 = fields.get(11).and_then(|s| s.parse().ok()).unwrap_or(0.0);
                let stime: f64 = fields.get(12).and_then(|s| s.parse().ok()).unwrap_or(0.0);
                let rss_pages: i64 = fields.get(21).and_then(|s| s.parse().ok()).unwrap_or(0);
                let page_size = 4096i64;

                rows.push(vec![
                    Value::Int(pid),
                    Value::Literal(name),
                    Value::Number((utime + stime) / clk_tck),
                    Value::Int(rss_pages * page_size),
                    Value::Literal(status),
                ]);
            }
        }

        rows.sort_by_key(|row| match row.first() {
            Some(Value::Int(pid)) => *pid,
            _ => 0,
        });

        self.stack.push(Value::Table { columns, rows });
        self.last_exit_code = 0;
        Ok(())
    }

    /// ps-t (macOS): shells out to `ps` and parses the fixed columns.
    #[cfg(target_os = "macos")]
    pub(crate) fn builtin_ps_t(&mut self) -> Result<(), EvalError> {
        use std::process::Command;

        let columns = vec![
            "pid".to_string(),
            "name".to_string(),
            "cpu".to_string(),
            "mem".to_string(),
            "status".to_string(),
        ];
        let mut rows: Vec<Vec<Value>> = Vec::new();

        let output = Command::new("ps")
            .args(["-axo", "pid=,cputime=,rss=,state=,comm="])
            .output()
            .map_err(|e| EvalError::ExecError(format!("ps-t: {}", e)))?;

        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() < 5 {
                continue;
            }
            let pid: i64 = fields[0].parse().unwrap_or(0);
            // cputime is MM:SS.ss
            let cpu: f64 = {
                let parts: Vec<&str> = fields[1].split(':').collect();
                let mins: f64 = parts.first().and_then(|m| m.parse().ok()).unwrap_or(0.0);
                let secs: f64 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0.0);
                mins * 60.0 + secs
            };
            let mem: i64 = fields[2].parse::<i64>().unwrap_or(0) * 1024; // rss is KB
            let status = fields[3].to_string();
            let name = fields[4..].join(" ");

            rows.push(vec![
                Value::Int(pid),
                Value::Literal(name),
                Value::Number(cpu),
                Value::Int(mem),
                Value::Literal(status),
            ]);
        }

        self.stack.push(Value::Table { columns, rows });
        self.last_exit_code = 0;
        Ok(())
    }

    /// ps-t (other platforms): graceful empty table
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    pub(crate) fn builtin_ps_t(&mut self) -> Result<(), EvalError> {
        self.stack.push(Value::Table {
            columns: vec![
                "pid".to_string(),
                "name".to_string(),
                "cpu".to_string(),
                "mem".to_string(),
                "status".to_string(),
            ],
            rows: vec![],
        });
        self.last_exit_code = 0;
        Ok(())
    }
}
