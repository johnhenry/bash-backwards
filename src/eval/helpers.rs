use super::{Evaluator, EvalError};
use crate::ast::{Expr, Value};
use glob::glob;
use num_bigint::BigUint;
use std::collections::HashMap;

impl Evaluator {
    /// Expand tilde (~) to home directory
    pub(crate) fn expand_tilde(&self, path: &str) -> String {
        if path == "~" {
            return self.home_dir.clone();
        }
        if let Some(rest) = path.strip_prefix("~/") {
            return format!("{}/{}", self.home_dir, rest);
        }
        path.to_string()
    }

    /// Interpolate variables in a double-quoted string
    /// Supports $VAR and ${VAR} syntax
    pub(crate) fn interpolate_string(&self, s: &str) -> String {
        let mut result = String::with_capacity(s.len());
        let mut chars = s.chars().peekable();

        while let Some(c) = chars.next() {
            if c == '$' {
                if chars.peek() == Some(&'{') {
                    // ${VAR} syntax
                    chars.next(); // consume '{'
                    let mut var_name = String::new();
                    while let Some(&ch) = chars.peek() {
                        if ch == '}' {
                            chars.next(); // consume '}'
                            break;
                        }
                        var_name.push(chars.next().unwrap());
                    }
                    if let Some(val) = self.lookup_var_as_string(&var_name) {
                        result.push_str(&val);
                    }
                } else if chars.peek().map(|c| c.is_ascii_alphabetic() || *c == '_').unwrap_or(false) {
                    // $VAR syntax - collect alphanumeric and underscore
                    let mut var_name = String::new();
                    while let Some(&ch) = chars.peek() {
                        if ch.is_ascii_alphanumeric() || ch == '_' {
                            var_name.push(chars.next().unwrap());
                        } else {
                            break;
                        }
                    }
                    if let Some(val) = self.lookup_var_as_string(&var_name) {
                        result.push_str(&val);
                    }
                } else {
                    // Lone $ or $followed-by-non-alpha
                    result.push('$');
                }
            } else if c == '\\' {
                // Handle escape sequences
                if let Some(&next) = chars.peek() {
                    match next {
                        '$' => {
                            chars.next();
                            result.push('$');
                        }
                        '\\' => {
                            chars.next();
                            result.push('\\');
                        }
                        _ => result.push(c),
                    }
                } else {
                    result.push(c);
                }
            } else {
                result.push(c);
            }
        }

        result
    }

    /// Expand glob patterns in a string
    pub(crate) fn expand_glob(&self, pattern: &str) -> Vec<String> {
        // Only expand if contains glob characters
        if !pattern.contains('*') && !pattern.contains('?') && !pattern.contains('[') {
            return vec![pattern.to_string()];
        }

        // Don't glob-expand words that end with ? if they look like predicates
        // (e.g., file?, dir?, eq?, lt?, ge?, contains?)
        if pattern.ends_with('?') && !pattern.contains('/') && !pattern.contains('*') {
            // Check if it's a single word (predicate name)
            if !pattern.chars().any(|c| c.is_whitespace()) {
                return vec![pattern.to_string()];
            }
        }

        // Expand relative to current working directory
        let full_pattern = if pattern.starts_with('/') {
            pattern.to_string()
        } else {
            format!("{}/{}", self.cwd.display(), pattern)
        };

        match glob(&full_pattern) {
            Ok(paths) => {
                let expanded: Vec<String> = paths
                    .filter_map(|p| p.ok())
                    .map(|p| p.to_string_lossy().to_string())
                    .collect();
                if expanded.is_empty() {
                    vec![pattern.to_string()] // No matches, return original
                } else {
                    expanded
                }
            }
            Err(_) => vec![pattern.to_string()],
        }
    }

    /// Expand both tilde and glob
    pub(crate) fn expand_arg(&self, arg: &str) -> Vec<String> {
        let expanded = self.expand_tilde(arg);
        self.expand_glob(&expanded)
    }

    pub(crate) fn pop_value_or_err(&mut self) -> Result<Value, EvalError> {
        self.stack
            .pop()
            .ok_or_else(|| EvalError::StackUnderflow("pop".into()))
    }

    pub(crate) fn pop_block(&mut self) -> Result<Vec<Expr>, EvalError> {
        match self.pop_value_or_err()? {
            Value::Block(exprs) => Ok(exprs),
            other => Err(EvalError::TypeError {
                expected: "block".into(),
                got: format!("{:?}", other),
            }),
        }
    }

    pub(crate) fn pop_string(&mut self) -> Result<String, EvalError> {
        let value = self.pop_value_or_err()?;
        value.as_arg().ok_or_else(|| EvalError::TypeError {
            expected: "string".into(),
            got: format!("{:?}", value),
        })
    }

    /// Helper to pop a number from stack (handles Literal/Output too)
    pub(crate) fn pop_number(&mut self, op: &str) -> Result<f64, EvalError> {
        let value = self.stack.pop().ok_or_else(|| {
            EvalError::ExecError(format!("{} requires a number on stack", op))
        })?;
        match &value {
            Value::Number(n) => Ok(*n),
            Value::Literal(s) | Value::Output(s) => {
                // Shell compatibility: non-numeric strings parse as 0
                Ok(s.trim().parse::<f64>().unwrap_or(0.0))
            }
            _ => {
                self.stack.push(value);
                Err(EvalError::ExecError(format!("{} requires a number", op)))
            }
        }
    }

    /// Helper to pop a BigInt from stack
    pub(crate) fn pop_bigint(&mut self, op: &str) -> Result<BigUint, EvalError> {
        let value = self.stack.pop().ok_or_else(|| {
            EvalError::ExecError(format!("{} requires BigInt on stack", op))
        })?;
        match value {
            Value::BigInt(n) => Ok(n),
            other => {
                self.stack.push(other);
                Err(EvalError::ExecError(format!("{} requires BigInt", op)))
            }
        }
    }

    /// Helper: Pop a numeric list from the stack
    pub(crate) fn pop_number_list(&mut self) -> Result<Vec<f64>, EvalError> {
        let val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("vector operation requires list".into()))?;

        match val {
            Value::List(items) => {
                items.iter()
                    .map(|v| match v {
                        Value::Number(n) => Ok(*n),
                        Value::Literal(s) | Value::Output(s) => {
                            s.trim().parse::<f64>().map_err(|_|
                                EvalError::TypeError {
                                    expected: "Number".into(),
                                    got: format!("'{}'", s),
                                })
                        }
                        _ => Err(EvalError::TypeError {
                            expected: "Number".into(),
                            got: format!("{:?}", v),
                        }),
                    })
                    .collect()
            }
            _ => Err(EvalError::TypeError {
                expected: "List".into(),
                got: format!("{:?}", val),
            }),
        }
    }

    /// Convert a block to command + args
    pub(crate) fn block_to_cmd_args(&self, exprs: &[Expr]) -> Result<(String, Vec<String>), EvalError> {
        let mut parts: Vec<String> = Vec::new();

        for expr in exprs {
            match expr {
                Expr::Literal(s) => parts.push(s.clone()),
                Expr::Quoted { content, .. } => parts.push(content.clone()),
                Expr::Variable(s) => {
                    let var_name = s
                        .trim_start_matches('$')
                        .trim_start_matches('{')
                        .trim_end_matches('}');
                    if let Ok(val) = std::env::var(var_name) {
                        parts.push(val);
                    }
                }
                _ => {}
            }
        }

        if parts.is_empty() {
            return Err(EvalError::ExecError("Empty command".into()));
        }

        // Last non-flag word is command (postfix semantics)
        let cmd_idx = parts
            .iter()
            .rposition(|s| !s.starts_with('-'))
            .unwrap_or(parts.len() - 1);
        let cmd = parts.remove(cmd_idx);

        // Expand args
        let expanded_args: Vec<String> = parts
            .into_iter()
            .flat_map(|arg| self.expand_arg(&arg))
            .collect();

        Ok((cmd, expanded_args))
    }

    /// Deep get with dot-notation path like "server.port" or "items.0"
    pub(crate) fn deep_get(&self, val: &Value, path: &str) -> Value {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = val.clone();

        for part in parts {
            current = match current {
                Value::Map(map) => {
                    map.get(part).cloned().unwrap_or(Value::Nil)
                }
                Value::List(items) => {
                    if let Ok(idx) = part.parse::<usize>() {
                        items.get(idx).cloned().unwrap_or(Value::Nil)
                    } else {
                        Value::Nil
                    }
                }
                _ => Value::Nil,
            };

            // Early exit if we hit Nil
            if matches!(current, Value::Nil) {
                break;
            }
        }

        current
    }

    /// Deep set a value at a dot-path (e.g., "server.port")
    pub(crate) fn deep_set(&self, target: Value, path: &str, value: Value) -> Result<Value, EvalError> {
        let parts: Vec<&str> = path.split('.').collect();
        if parts.is_empty() {
            return Ok(target);
        }

        self.deep_set_recursive(target, &parts, value)
    }

    pub(crate) fn deep_set_recursive(&self, target: Value, path: &[&str], value: Value) -> Result<Value, EvalError> {
        if path.is_empty() {
            return Ok(value);
        }

        let key = path[0];
        let remaining = &path[1..];

        match target {
            Value::Map(mut map) => {
                if remaining.is_empty() {
                    // Last key - set the value directly
                    map.insert(key.to_string(), value);
                } else {
                    // Need to recurse
                    let current = map.get(key).cloned().unwrap_or_else(|| Value::Map(HashMap::new()));
                    let new_val = self.deep_set_recursive(current, remaining, value)?;
                    map.insert(key.to_string(), new_val);
                }
                Ok(Value::Map(map))
            }
            Value::Nil => {
                // Create nested structure
                let mut map = HashMap::new();
                if remaining.is_empty() {
                    map.insert(key.to_string(), value);
                } else {
                    let new_val = self.deep_set_recursive(Value::Nil, remaining, value)?;
                    map.insert(key.to_string(), new_val);
                }
                Ok(Value::Map(map))
            }
            _ => Err(EvalError::TypeError {
                expected: "Record".into(),
                got: format!("{:?}", target),
            }),
        }
    }

    /// Push back unused arguments to stack (for builtins that only need N args)
    /// Args are in LIFO order, so we push back from end towards start
    pub(crate) fn restore_excess_args(&mut self, args: &[String], used: usize) {
        // Push back args[used..] in reverse order to restore original stack order
        for i in (used..args.len()).rev() {
            self.stack.push(Value::Literal(args[i].clone()));
        }
    }

    /// Convert expressions back to string for display
    pub(crate) fn exprs_to_string(&self, exprs: &[Expr]) -> String {
        exprs
            .iter()
            .map(|e| match e {
                Expr::Literal(s) => s.clone(),
                Expr::Quoted { content, double } => {
                    if *double {
                        format!("\"{}\"", content)
                    } else {
                        format!("'{}'", content)
                    }
                }
                Expr::Variable(s) => s.clone(),
                Expr::Block(inner) => format!("[{}]", self.exprs_to_string(inner)),
                _ => format!("{:?}", e),
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}
