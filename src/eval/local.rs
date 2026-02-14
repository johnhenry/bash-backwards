use super::{Evaluator, EvalError};
use crate::ast::Value;

impl Evaluator {
    /// Create local variable, preserving structured value types
    /// Usage: value NAME local
    /// For structured data (List, Table, Map), stores in local_values
    /// For primitives, uses env vars for shell compatibility
    pub(crate) fn builtin_local_stack(&mut self) -> Result<(), EvalError> {
        if self.local_scopes.is_empty() {
            return Err(EvalError::ExecError(
                "local: can only be used inside a function".into(),
            ));
        }

        // Pop the variable name (must be a string)
        let name = self.pop_string()?;

        // Pop the value (preserve its type)
        let value = self.stack.pop().ok_or_else(|| {
            EvalError::StackUnderflow("local requires a value".into())
        })?;

        // Check if this is a structured value that needs special storage
        let is_structured = matches!(
            &value,
            Value::List(_) | Value::Table { .. } | Value::Map(_) |
            Value::Media { .. } | Value::Bytes(_) | Value::BigInt(_) |
            Value::Block(_)
        );

        if is_structured {
            // Store in local_values to preserve the Value type
            if let Some(scope) = self.local_values.last_mut() {
                scope.insert(name.clone(), value);
            }
            // Also save env var state for cleanup (even if we don't use it)
            let current_scope = self.local_scopes.last_mut().unwrap();
            if !current_scope.contains_key(&name) {
                let old_value = std::env::var(&name).ok();
                current_scope.insert(name, old_value);
            }
        } else {
            // Primitive value - use env vars for shell compatibility
            let string_value = match &value {
                Value::Literal(s) | Value::Output(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                Value::Bool(b) => b.to_string(),
                Value::Nil => String::new(),
                _ => value.as_arg().unwrap_or_default(),
            };

            let current_scope = self.local_scopes.last_mut().unwrap();
            if !current_scope.contains_key(&name) {
                current_scope.insert(name.clone(), std::env::var(&name).ok());
            }
            std::env::set_var(&name, string_value);
        }

        self.last_exit_code = 0;
        Ok(())
    }
}
