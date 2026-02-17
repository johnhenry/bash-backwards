use super::{Evaluator, EvalError};
use crate::ast::Value;
use std::path::Path;

impl Evaluator {
    // ========================================
    // Predicates (stack-native versions)
    // ========================================

    /// String equality (stack-native)
    /// Usage: "a" "b" eq? -> Bool
    pub(crate) fn builtin_eq_stack(&mut self) -> Result<(), EvalError> {
        let b = self.pop_string()?;
        let a = self.pop_string()?;
        let result = a == b;
        self.stack.push(Value::Bool(result));
        self.last_exit_code = if result { 0 } else { 1 };
        Ok(())
    }

    /// String inequality (stack-native)
    /// Usage: "a" "b" ne? -> Bool
    pub(crate) fn builtin_ne_stack(&mut self) -> Result<(), EvalError> {
        let b = self.pop_string()?;
        let a = self.pop_string()?;
        let result = a != b;
        self.stack.push(Value::Bool(result));
        self.last_exit_code = if result { 0 } else { 1 };
        Ok(())
    }

    /// Numeric equality (stack-native)
    /// Usage: 5 5 =? -> Bool
    pub(crate) fn builtin_num_eq_stack(&mut self) -> Result<(), EvalError> {
        let b = self.pop_number("=?")?;
        let a = self.pop_number("=?")?;
        let result = a == b;
        self.stack.push(Value::Bool(result));
        self.last_exit_code = if result { 0 } else { 1 };
        Ok(())
    }

    /// Numeric inequality (stack-native)
    /// Usage: 5 10 !=? -> Bool
    pub(crate) fn builtin_num_ne_stack(&mut self) -> Result<(), EvalError> {
        let b = self.pop_number("!=?")?;
        let a = self.pop_number("!=?")?;
        let result = a != b;
        self.stack.push(Value::Bool(result));
        self.last_exit_code = if result { 0 } else { 1 };
        Ok(())
    }

    /// Numeric less than (stack-native)
    /// Usage: 5 10 lt? -> Bool
    pub(crate) fn builtin_lt_stack(&mut self) -> Result<(), EvalError> {
        let b = self.pop_number("lt?")?;
        let a = self.pop_number("lt?")?;
        let result = a < b;
        self.stack.push(Value::Bool(result));
        self.last_exit_code = if result { 0 } else { 1 };
        Ok(())
    }

    /// Numeric greater than (stack-native)
    /// Usage: 10 5 gt? -> Bool
    pub(crate) fn builtin_gt_stack(&mut self) -> Result<(), EvalError> {
        let b = self.pop_number("gt?")?;
        let a = self.pop_number("gt?")?;
        let result = a > b;
        self.stack.push(Value::Bool(result));
        self.last_exit_code = if result { 0 } else { 1 };
        Ok(())
    }

    /// Numeric less than or equal (stack-native)
    /// Usage: 5 10 le? -> Bool
    pub(crate) fn builtin_le_stack(&mut self) -> Result<(), EvalError> {
        let b = self.pop_number("le?")?;
        let a = self.pop_number("le?")?;
        let result = a <= b;
        self.stack.push(Value::Bool(result));
        self.last_exit_code = if result { 0 } else { 1 };
        Ok(())
    }

    /// Numeric greater than or equal (stack-native)
    /// Usage: 10 5 ge? -> Bool
    pub(crate) fn builtin_ge_stack(&mut self) -> Result<(), EvalError> {
        let b = self.pop_number("ge?")?;
        let a = self.pop_number("ge?")?;
        let result = a >= b;
        self.stack.push(Value::Bool(result));
        self.last_exit_code = if result { 0 } else { 1 };
        Ok(())
    }

    // ========================================
    // Arithmetic primitives (stack-native versions)
    // ========================================

    /// Add two numbers (stack-native)
    /// Usage: 5 3 plus -> 8
    pub(crate) fn builtin_plus_stack(&mut self) -> Result<(), EvalError> {
        let b = self.pop_number("plus")?;
        let a = self.pop_number("plus")?;
        self.stack.push(Value::Number(a + b));
        self.last_exit_code = 0;
        Ok(())
    }

    /// Subtract two numbers (stack-native)
    /// Usage: 10 3 minus -> 7
    pub(crate) fn builtin_minus_stack(&mut self) -> Result<(), EvalError> {
        let b = self.pop_number("minus")?;
        let a = self.pop_number("minus")?;
        self.stack.push(Value::Number(a - b));
        self.last_exit_code = 0;
        Ok(())
    }

    /// Multiply two numbers (stack-native)
    /// Usage: 4 5 mul -> 20
    pub(crate) fn builtin_mul_stack(&mut self) -> Result<(), EvalError> {
        let b = self.pop_number("mul")?;
        let a = self.pop_number("mul")?;
        self.stack.push(Value::Number(a * b));
        self.last_exit_code = 0;
        Ok(())
    }

    /// Divide two numbers (stack-native, float division)
    /// Usage: 10 3 div -> 3.333...
    pub(crate) fn builtin_div_stack(&mut self) -> Result<(), EvalError> {
        let b = self.pop_number("div")?;
        let a = self.pop_number("div")?;
        if b == 0.0 {
            return Err(EvalError::ExecError("div: division by zero".to_string()));
        }
        self.stack.push(Value::Number(a / b));
        self.last_exit_code = 0;
        Ok(())
    }

    /// Modulo (stack-native)
    /// Usage: 10 3 mod -> 1
    pub(crate) fn builtin_mod_stack(&mut self) -> Result<(), EvalError> {
        let b = self.pop_number("mod")?;
        let a = self.pop_number("mod")?;
        if b == 0.0 {
            return Err(EvalError::ExecError("mod: division by zero".to_string()));
        }
        self.stack.push(Value::Number(a % b));
        self.last_exit_code = 0;
        Ok(())
    }

    // ========================================
    // Math primitives (for stats support)
    // ========================================

    /// Float power: base exponent pow -> base^exponent
    pub(crate) fn builtin_pow(&mut self) -> Result<(), EvalError> {
        let exp = self.pop_number("pow")?;
        let base = self.pop_number("pow")?;
        let result = base.powf(exp);
        self.stack.push(Value::Number(result));
        self.last_exit_code = 0;
        Ok(())
    }

    /// Square root: n sqrt -> sqrt(n)
    pub(crate) fn builtin_sqrt(&mut self) -> Result<(), EvalError> {
        let n = self.pop_number("sqrt")?;
        if n < 0.0 {
            return Err(EvalError::ExecError("sqrt: negative number".to_string()));
        }
        let result = n.sqrt();
        self.stack.push(Value::Number(result));
        self.last_exit_code = 0;
        Ok(())
    }

    /// Floor: round down to nearest integer
    /// Usage: 3.7 floor -> 3
    pub(crate) fn builtin_floor(&mut self) -> Result<(), EvalError> {
        let n = self.pop_number("floor")?;
        self.stack.push(Value::Number(n.floor()));
        self.last_exit_code = 0;
        Ok(())
    }

    /// Ceil: round up to nearest integer
    /// Usage: 3.2 ceil -> 4
    pub(crate) fn builtin_ceil(&mut self) -> Result<(), EvalError> {
        let n = self.pop_number("ceil")?;
        self.stack.push(Value::Number(n.ceil()));
        self.last_exit_code = 0;
        Ok(())
    }

    /// Round: round to nearest integer (half rounds away from zero)
    /// Usage: 3.5 round -> 4, 3.4 round -> 3
    pub(crate) fn builtin_round(&mut self) -> Result<(), EvalError> {
        let n = self.pop_number("round")?;
        self.stack.push(Value::Number(n.round()));
        self.last_exit_code = 0;
        Ok(())
    }

    /// Integer division (truncates toward zero)
    /// Usage: 10 3 idiv -> 3
    pub(crate) fn builtin_idiv(&mut self) -> Result<(), EvalError> {
        let b = self.pop_number("idiv")?;
        let a = self.pop_number("idiv")?;
        if b == 0.0 {
            return Err(EvalError::ExecError("idiv: division by zero".to_string()));
        }
        self.stack.push(Value::Number((a / b).trunc()));
        self.last_exit_code = 0;
        Ok(())
    }

    /// Sort list numerically: [nums] sort-nums -> [sorted]
    pub(crate) fn builtin_sort_nums(&mut self) -> Result<(), EvalError> {
        let value = self.stack.pop().ok_or_else(|| {
            EvalError::ExecError("sort-nums requires a list on stack".to_string())
        })?;

        match value {
            Value::List(mut items) => {
                // Extract numeric values and sort
                items.sort_by(|a, b| {
                    let a_num = match a {
                        Value::Number(n) => *n,
                        Value::Literal(s) | Value::Output(s) => s.trim().parse().unwrap_or(f64::NAN),
                        _ => f64::NAN,
                    };
                    let b_num = match b {
                        Value::Number(n) => *n,
                        Value::Literal(s) | Value::Output(s) => s.trim().parse().unwrap_or(f64::NAN),
                        _ => f64::NAN,
                    };
                    a_num.partial_cmp(&b_num).unwrap_or(std::cmp::Ordering::Equal)
                });
                self.stack.push(Value::List(items));
                self.last_exit_code = 0;
            }
            other => {
                self.stack.push(other);
                return Err(EvalError::ExecError("sort-nums requires a list".to_string()));
            }
        }

        Ok(())
    }

    /// Logarithm with arbitrary base: value base log-base -> log_base(value)
    /// Usage: 100 10 log-base -> 2 (log base 10 of 100)
    pub(crate) fn builtin_log_base(&mut self) -> Result<(), EvalError> {
        let base = self.pop_number("log-base")?;
        let value = self.pop_number("log-base")?;
        if base <= 0.0 || base == 1.0 {
            return Err(EvalError::ExecError(
                format!("log-base: base must be positive and not 1, got {}", base)
            ));
        }
        if value <= 0.0 {
            return Err(EvalError::ExecError(
                format!("log-base: value must be positive, got {}", value)
            ));
        }
        let result = value.ln() / base.ln();
        self.stack.push(Value::Number(result));
        self.last_exit_code = 0;
        Ok(())
    }

    // ========================================
    // File/Directory predicates
    // ========================================

    pub(crate) fn builtin_file_predicate(&mut self, args: &[String]) -> Result<(), EvalError> {
        let path = args.first().ok_or_else(|| {
            EvalError::ExecError("file?: path required".into())
        })?;
        let result = Path::new(path).is_file();
        self.stack.push(Value::Bool(result));
        self.last_exit_code = if result { 0 } else { 1 };
        Ok(())
    }

    /// Check if path is a directory
    /// Usage: "path" dir?
    pub(crate) fn builtin_dir_predicate(&mut self, args: &[String]) -> Result<(), EvalError> {
        let path = args.first().ok_or_else(|| {
            EvalError::ExecError("dir?: path required".into())
        })?;
        let result = Path::new(path).is_dir();
        self.stack.push(Value::Bool(result));
        self.last_exit_code = if result { 0 } else { 1 };
        Ok(())
    }

    /// Check if path exists (file or directory)
    /// Usage: "path" exists?
    pub(crate) fn builtin_exists_predicate(&mut self, args: &[String]) -> Result<(), EvalError> {
        let path = args.first().ok_or_else(|| {
            EvalError::ExecError("exists?: path required".into())
        })?;
        self.restore_excess_args(args, 1);
        let result = Path::new(path).exists();
        self.stack.push(Value::Bool(result));
        self.last_exit_code = if result { 0 } else { 1 };
        Ok(())
    }

    /// Check if string is empty
    /// Usage: "string" empty?
    pub(crate) fn builtin_empty_predicate(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.is_empty() {
            return Err(EvalError::ExecError("empty?: string required".into()));
        }
        self.restore_excess_args(args, 1);
        let s = &args[0];
        let result = s.is_empty();
        self.stack.push(Value::Bool(result));
        self.last_exit_code = if result { 0 } else { 1 };
        Ok(())
    }

    /// Check if string contains a substring
    /// Usage: "string" "substr" contains?
    pub(crate) fn builtin_contains_predicate(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.len() < 2 {
            return Err(EvalError::ExecError("contains?: string and substring required".into()));
        }
        self.restore_excess_args(args, 2);
        // Args in LIFO: [needle, haystack] for "haystack needle contains?"
        let needle = &args[0];
        let haystack = &args[1];
        let result = haystack.contains(needle.as_str());
        self.stack.push(Value::Bool(result));
        self.last_exit_code = if result { 0 } else { 1 };
        Ok(())
    }

    /// Check if string starts with a prefix
    /// Usage: "string" "prefix" starts?
    pub(crate) fn builtin_starts_predicate(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.len() < 2 {
            return Err(EvalError::ExecError("starts?: string and prefix required".into()));
        }
        self.restore_excess_args(args, 2);
        // Args in LIFO: [prefix, string] for "string prefix starts?"
        let prefix = &args[0];
        let s = &args[1];
        let result = s.starts_with(prefix.as_str());
        self.stack.push(Value::Bool(result));
        self.last_exit_code = if result { 0 } else { 1 };
        Ok(())
    }

    /// Check if string ends with a suffix
    /// Usage: "string" "suffix" ends?
    pub(crate) fn builtin_ends_predicate(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.len() < 2 {
            return Err(EvalError::ExecError("ends?: string and suffix required".into()));
        }
        self.restore_excess_args(args, 2);
        // Args in LIFO: [suffix, string] for "string suffix ends?"
        let suffix = &args[0];
        let s = &args[1];
        let result = s.ends_with(suffix.as_str());
        self.stack.push(Value::Bool(result));
        self.last_exit_code = if result { 0 } else { 1 };
        Ok(())
    }

    // pop_number is defined in helpers.rs
}
