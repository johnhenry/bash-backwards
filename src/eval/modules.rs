use super::{Evaluator, EvalError};
use crate::ast::Expr;
use std::collections::HashMap;
use std::path::PathBuf;

impl Evaluator {
    pub(crate) fn module_import(&mut self) -> Result<(), EvalError> {
        // Pop the top value - could be path or alias
        let top = self.pop_string()?;

        // Check if top is a path (contains / or .) or an alias (simple identifier)
        let (path_str, alias) = if top.contains('/') || top.contains('.') {
            // Top is a path, no alias
            (top, None)
        } else {
            // Top is an alias, path should be next on stack
            let path = self.pop_string()?;
            (path, Some(top))
        };

        // Resolve module path using search paths
        let resolved_path = self.resolve_module_path(&path_str)?;

        // Get canonical path for tracking
        let canonical = resolved_path.canonicalize().unwrap_or_else(|_| resolved_path.clone());

        // Skip if already loaded
        if self.loaded_modules.contains(&canonical) {
            self.last_exit_code = 0;
            return Ok(());
        }

        // Mark as loaded before executing (handles circular imports)
        self.loaded_modules.insert(canonical);

        // Determine namespace from filename or alias
        let namespace = match alias {
            Some(a) => a,
            None => {
                // Extract filename without extension
                resolved_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
                    .ok_or_else(|| EvalError::ExecError("import: invalid module path".into()))?
            }
        };

        // Read and parse the module
        let content = std::fs::read_to_string(&resolved_path)
            .map_err(|e| EvalError::ExecError(format!("import: {}: {}", path_str, e)))?;

        let tokens = crate::lex(&content)
            .map_err(|e| EvalError::ExecError(format!("import: parse error: {}", e)))?;

        if tokens.is_empty() {
            self.last_exit_code = 0;
            return Ok(());
        }

        let program = crate::parse(tokens)
            .map_err(|e| EvalError::ExecError(format!("import: parse error: {}", e)))?;

        // Save current definitions (with their values) to detect new/changed ones
        let before_defs: HashMap<String, Vec<Expr>> = self.definitions.clone();

        // Execute module in current context
        for expr in &program.expressions {
            self.eval_expr(expr)?;
        }

        // Find definitions that were added or changed during module execution
        let module_defs: Vec<String> = self.definitions
            .iter()
            .filter(|(name, body)| {
                // Include if: new name OR same name but different body
                match before_defs.get(*name) {
                    None => true,  // New definition
                    Some(old_body) => old_body != *body,  // Changed definition
                }
            })
            .map(|(name, _)| name.clone())
            .collect();

        for name in module_defs {
            // Skip private definitions (underscore prefix)
            if name.starts_with('_') {
                self.definitions.remove(&name);
                continue;
            }

            // Move definition to namespaced name
            if let Some(block) = self.definitions.remove(&name) {
                let namespaced = format!("{}::{}", namespace, name);
                self.definitions.insert(namespaced.clone(), block);

                // Restore the original definition if it existed
                if let Some(original) = before_defs.get(&name) {
                    self.definitions.insert(name, original.clone());
                }
            }
        }

        self.last_exit_code = 0;
        Ok(())
    }

    /// Resolve module path using search paths
    /// Search order: . -> ./lib/ -> ~/.hsab/lib/ -> $HSAB_PATH
    pub(crate) fn resolve_module_path(&self, path_str: &str) -> Result<PathBuf, EvalError> {
        let path = PathBuf::from(path_str);

        // If absolute path, use directly
        if path.is_absolute() {
            if path.exists() {
                return Ok(path);
            }
            return Err(EvalError::ExecError(format!("import: module not found: {}", path_str)));
        }

        // Build search paths
        let mut search_paths = vec![
            self.cwd.clone(),                           // Current directory
            self.cwd.join("lib"),                       // ./lib/
        ];

        // Add ~/.hsab/lib/
        if let Ok(home) = std::env::var("HOME") {
            search_paths.push(PathBuf::from(home).join(".hsab").join("lib"));
        }

        // Add HSAB_PATH directories
        if let Ok(hsab_path) = std::env::var("HSAB_PATH") {
            for dir in hsab_path.split(':') {
                if !dir.is_empty() {
                    search_paths.push(PathBuf::from(dir));
                }
            }
        }

        // Search for the module
        for search_dir in search_paths {
            let full_path = search_dir.join(&path);
            if full_path.exists() {
                return Ok(full_path);
            }
        }

        Err(EvalError::ExecError(format!("import: module not found: {}", path_str)))
    }
}
