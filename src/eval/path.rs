use super::{Evaluator, EvalError};
use crate::ast::Value;

impl Evaluator {
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
