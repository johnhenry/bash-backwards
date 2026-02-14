use super::{Evaluator, EvalError};
use crate::ast::Value;

impl Evaluator {
    /// If: [condition] [then] [else] if
    pub(crate) fn control_if(&mut self) -> Result<(), EvalError> {
        let else_block = self.pop_block()?;
        let then_block = self.pop_block()?;
        let cond_block = self.pop_block()?;

        // Save outer capture mode
        let outer_capture_mode = self.capture_mode;

        // Execute condition block with full stack access
        // Condition can read/modify stack, we just check exit code
        self.capture_mode = true;
        for expr in &cond_block {
            self.eval_expr(expr)?;
        }

        // Check result - use exit code
        let condition_met = self.last_exit_code == 0;

        // Execute appropriate branch - capture all but restore for last
        let branch = if condition_met { then_block } else { else_block };
        for (i, expr) in branch.iter().enumerate() {
            let is_last = i == branch.len() - 1;
            self.capture_mode = if is_last { outer_capture_mode } else { true };
            self.eval_expr(expr)?;
        }

        Ok(())
    }

    /// Times: N [block] times - repeat block N times
    pub(crate) fn control_times(&mut self) -> Result<(), EvalError> {
        let block = self.pop_block()?;
        let n_str = self.pop_string()?;

        let n: usize = n_str.parse().map_err(|_| EvalError::TypeError {
            expected: "integer".into(),
            got: n_str,
        })?;

        'outer: for _ in 0..n {
            for expr in &block {
                match self.eval_expr(expr) {
                    Ok(()) => {}
                    Err(EvalError::BreakLoop) => break 'outer,
                    Err(e) => return Err(e),
                }
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
