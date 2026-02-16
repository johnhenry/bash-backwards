/// Generate a structured builtin that pops typed values, calls a function, and pushes the result.
///
/// Usage:
///   stack_builtin!(builtin_name, "op-name", (a: Number, b: Number) -> Number, |a, b| { a + b });
///   stack_builtin!(builtin_name, "op-name", (list: NumberList) -> Number, |list: Vec<f64>| { list.iter().sum() });
///   stack_builtin!(builtin_name, "op-name", (n: Number) -> Number, |n| { n.sqrt() });
///
/// Supported pop types:
///   Number     -> pop_number(op_name) -> f64
///   NumberList -> pop Value::List, extract f64s -> Vec<f64>
///   Value      -> stack.pop() -> Value (raw)
///
/// Supported push types:
///   Number -> push Value::Number(result)
///   Value  -> push result directly (must be a Value)

macro_rules! stack_builtin {
    // Two Number params -> Number result
    ($name:ident, $op:expr, ($a:ident : Number, $b:ident : Number) -> Number, $body:expr) => {
        pub(crate) fn $name(&mut self) -> Result<(), EvalError> {
            let $b = self.pop_number($op)?;
            let $a = self.pop_number($op)?;
            let result: f64 = $body;
            self.stack.push(Value::Number(result));
            self.last_exit_code = 0;
            Ok(())
        }
    };

    // One Number param -> Number result
    ($name:ident, $op:expr, ($a:ident : Number) -> Number, $body:expr) => {
        pub(crate) fn $name(&mut self) -> Result<(), EvalError> {
            let $a = self.pop_number($op)?;
            let result: f64 = $body;
            self.stack.push(Value::Number(result));
            self.last_exit_code = 0;
            Ok(())
        }
    };

    // NumberList param -> Number result
    ($name:ident, $op:expr, ($list:ident : NumberList) -> Number, $body:expr) => {
        pub(crate) fn $name(&mut self) -> Result<(), EvalError> {
            let val = self.stack.pop().ok_or_else(||
                EvalError::StackUnderflow(format!("{} requires a list", $op)))?;
            let $list: Vec<f64> = match val {
                Value::List(items) => items.iter().filter_map(|v| match v {
                    Value::Number(n) => Some(*n),
                    Value::Literal(s) | Value::Output(s) => s.trim().parse().ok(),
                    _ => None,
                }).collect(),
                _ => return Err(EvalError::TypeError {
                    expected: "List".into(),
                    got: format!("{:?}", val),
                }),
            };
            let result: f64 = $body;
            self.stack.push(Value::Number(result));
            self.last_exit_code = 0;
            Ok(())
        }
    };

    // NumberList param -> Value result (for functions that return lists etc.)
    ($name:ident, $op:expr, ($list:ident : NumberList) -> Value, $body:expr) => {
        pub(crate) fn $name(&mut self) -> Result<(), EvalError> {
            let val = self.stack.pop().ok_or_else(||
                EvalError::StackUnderflow(format!("{} requires a list", $op)))?;
            let $list: Vec<f64> = match val {
                Value::List(items) => items.iter().filter_map(|v| match v {
                    Value::Number(n) => Some(*n),
                    Value::Literal(s) | Value::Output(s) => s.trim().parse().ok(),
                    _ => None,
                }).collect(),
                _ => return Err(EvalError::TypeError {
                    expected: "List".into(),
                    got: format!("{:?}", val),
                }),
            };
            let result: Value = $body;
            self.stack.push(result);
            self.last_exit_code = 0;
            Ok(())
        }
    };
}
