use super::{Evaluator, EvalError};
use crate::ast::{Expr, Value};

impl Evaluator {
    /// Check if a value is "truthy" for conditional purposes
    /// - Bool(true) => true, Bool(false) => false
    /// - Number: non-zero => true
    /// - Literal/Output: non-empty => true
    /// - Nil => false
    /// - Block, List, Map, etc. => true (they exist)
    pub(crate) fn is_truthy(value: &Value) -> bool {
        match value {
            Value::Bool(b) => *b,
            Value::Number(n) => *n != 0.0,
            Value::Literal(s) | Value::Output(s) => !s.is_empty(),
            Value::Nil => false,
            _ => true,
        }
    }

    /// If: [else] [then] condition if
    /// Pops condition (top), then-block, and optionally else-block.
    /// Condition is a VALUE (not a block).
    pub(crate) fn control_if(&mut self) -> Result<(), EvalError> {
        // Pop condition value (top of stack)
        let condition = self.pop_value_or_err()?;

        // Pop then-block (must be a block)
        let then_block = self.pop_block()?;

        // Optionally pop else-block (if top of stack is a block)
        let else_block = if matches!(self.stack.last(), Some(Value::Block(_))) {
            Some(self.pop_block()?)
        } else {
            None
        };

        let condition_met = Self::is_truthy(&condition);

        // Save outer capture mode
        let outer_capture_mode = self.capture_mode;

        // Track whether this if-branch was taken (for elseif/else chaining)
        self.last_if_taken = condition_met;

        if condition_met {
            // Execute then-block
            for (i, expr) in then_block.iter().enumerate() {
                let is_last = i == then_block.len() - 1;
                self.capture_mode = if is_last { outer_capture_mode } else { true };
                self.eval_expr(expr)?;
            }
        } else if let Some(else_block) = else_block {
            // Execute else-block
            for (i, expr) in else_block.iter().enumerate() {
                let is_last = i == else_block.len() - 1;
                self.capture_mode = if is_last { outer_capture_mode } else { true };
                self.eval_expr(expr)?;
            }
        }

        Ok(())
    }

    /// ElseIf: [then] condition elseif
    /// Only checks condition and runs then-block if no prior if/elseif branch was taken.
    pub(crate) fn control_elseif(&mut self) -> Result<(), EvalError> {
        // Pop condition value (top of stack)
        let condition = self.pop_value_or_err()?;

        // Pop then-block (must be a block)
        let then_block = self.pop_block()?;

        // If a prior branch was already taken, skip
        if self.last_if_taken {
            return Ok(());
        }

        let condition_met = Self::is_truthy(&condition);

        // Save outer capture mode
        let outer_capture_mode = self.capture_mode;

        if condition_met {
            self.last_if_taken = true;
            for (i, expr) in then_block.iter().enumerate() {
                let is_last = i == then_block.len() - 1;
                self.capture_mode = if is_last { outer_capture_mode } else { true };
                self.eval_expr(expr)?;
            }
        }

        Ok(())
    }

    /// Else: [block] else
    /// Runs block only if no prior if/elseif branch was taken.
    pub(crate) fn control_else(&mut self) -> Result<(), EvalError> {
        // Pop the else block
        let else_block = self.pop_block()?;

        // If a prior branch was already taken, skip
        if self.last_if_taken {
            return Ok(());
        }

        // Save outer capture mode
        let outer_capture_mode = self.capture_mode;

        self.last_if_taken = true;
        for (i, expr) in else_block.iter().enumerate() {
            let is_last = i == else_block.len() - 1;
            self.capture_mode = if is_last { outer_capture_mode } else { true };
            self.eval_expr(expr)?;
        }

        Ok(())
    }

    /// Times: [block] N times - repeat block N times
    /// New order: block first, then N.
    /// Each iteration is isolated with a marker so commands inside
    /// don't consume values from previous iterations.
    pub(crate) fn control_times(&mut self) -> Result<(), EvalError> {
        let n_str = self.pop_string()?;
        let block = self.pop_block()?;

        let n: usize = n_str.parse().map_err(|_| EvalError::TypeError {
            expected: "integer".into(),
            got: n_str,
        })?;

        'outer: for _ in 0..n {
            // Isolate each iteration with a marker so commands inside
            // don't consume values from previous iterations or outer scope
            self.stack.push(Value::Marker);

            for expr in &block {
                match self.eval_expr(expr) {
                    Ok(()) => {}
                    Err(EvalError::BreakLoop) => {
                        // Clean up marker before breaking
                        while let Some(v) = self.stack.pop() {
                            if v.is_marker() { break; }
                        }
                        break 'outer;
                    }
                    Err(e) => return Err(e),
                }
            }

            // Move results above marker back onto main stack
            let mut results = Vec::new();
            while let Some(v) = self.stack.pop() {
                if v.is_marker() {
                    break;
                }
                results.push(v);
            }
            for v in results.into_iter().rev() {
                self.stack.push(v);
            }
        }

        Ok(())
    }

    /// While: [condition] [body] while - repeat while condition passes (exit code 0)
    pub(crate) fn control_while(&mut self) -> Result<(), EvalError> {
        let body = self.pop_block()?;
        let cond = self.pop_block()?;

        'outer: loop {
            // Isolate condition evaluation with marker
            self.stack.push(Value::Marker);

            // Evaluate condition
            for expr in &cond {
                self.eval_expr(expr)?;
            }

            // Clean up anything pushed during condition (until marker)
            while let Some(v) = self.stack.pop() {
                if v.is_marker() {
                    break;
                }
            }

            // Stop if condition fails
            if self.last_exit_code != 0 {
                break;
            }

            // Execute body (output stays on stack)
            for expr in &body {
                match self.eval_expr(expr) {
                    Ok(()) => {}
                    Err(EvalError::BreakLoop) => break 'outer,
                    Err(e) => return Err(e),
                }
            }
        }

        Ok(())
    }

    /// Until: [condition] [body] until - repeat until condition passes (exit code 0)
    pub(crate) fn control_until(&mut self) -> Result<(), EvalError> {
        let body = self.pop_block()?;
        let cond = self.pop_block()?;

        'outer: loop {
            // Isolate condition evaluation with marker
            self.stack.push(Value::Marker);

            // Evaluate condition
            for expr in &cond {
                self.eval_expr(expr)?;
            }

            // Clean up anything pushed during condition (until marker)
            while let Some(v) = self.stack.pop() {
                if v.is_marker() {
                    break;
                }
            }

            // Stop if condition succeeds
            if self.last_exit_code == 0 {
                break;
            }

            // Execute body (output stays on stack)
            for expr in &body {
                match self.eval_expr(expr) {
                    Ok(()) => {}
                    Err(EvalError::BreakLoop) => break 'outer,
                    Err(e) => return Err(e),
                }
            }
        }

        Ok(())
    }
}
