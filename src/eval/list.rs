use super::{Evaluator, EvalError};
use crate::ast::Value;

impl Evaluator {
    /// Spread: split a multi-line value into separate stack items
    pub(crate) fn list_spread(&mut self) -> Result<(), EvalError> {
        let value = self.pop_value_or_err()?;
        let text = value.as_arg().unwrap_or_default();

        // Push marker to indicate start of spread items
        self.stack.push(Value::Marker);

        // Split by newlines and push each line
        for line in text.lines() {
            if !line.is_empty() {
                self.stack.push(Value::Literal(line.to_string()));
            }
        }

        Ok(())
    }

    /// Each: apply a block to each item on the stack until hitting a marker
    pub(crate) fn list_each(&mut self) -> Result<(), EvalError> {
        let block = self.pop_block()?;

        // Collect items until we hit a marker
        let mut items = Vec::new();
        while let Some(value) = self.stack.last() {
            if value.is_marker() {
                self.stack.pop(); // Remove the marker
                break;
            }
            items.push(self.stack.pop().unwrap());
        }

        // Items are in reverse order (LIFO), so reverse them
        items.reverse();

        // Apply block to each item
        'outer: for item in items {
            self.stack.push(item);
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

    /// Collect: gather stack items until marker into a single value
    pub(crate) fn list_collect(&mut self) -> Result<(), EvalError> {
        let mut items = Vec::new();

        while let Some(value) = self.stack.last() {
            if value.is_marker() {
                self.stack.pop(); // Remove the marker
                break;
            }
            if let Some(s) = value.as_arg() {
                items.push(s);
            }
            self.stack.pop();
        }

        // Items are in reverse order (LIFO), so reverse them
        items.reverse();

        // Join with newlines and push as output
        let collected = items.join("\n");
        if collected.is_empty() {
            self.stack.push(Value::Nil);
        } else {
            self.stack.push(Value::Output(collected));
        }

        Ok(())
    }

    /// Keep: filter items, keeping only those where predicate returns exit code 0
    pub(crate) fn list_keep(&mut self) -> Result<(), EvalError> {
        let predicate = self.pop_block()?;

        // Collect items until we hit a marker
        let mut items = Vec::new();
        while let Some(value) = self.stack.last() {
            if value.is_marker() {
                self.stack.pop(); // Remove the marker
                break;
            }
            items.push(self.stack.pop().unwrap());
        }

        // Items are in reverse order (LIFO), so reverse them
        items.reverse();

        // Collect kept items separately, then push all at once with marker
        let mut kept = Vec::new();

        // Test each item with predicate, keep if passes
        for item in items {
            // Push a temporary marker to isolate this test
            self.stack.push(Value::Marker);

            // Push item for predicate to consume
            self.stack.push(item.clone());

            // Execute predicate
            for expr in &predicate {
                self.eval_expr(expr)?;
            }

            // Clean up: remove everything down to (and including) the temp marker
            while let Some(v) = self.stack.pop() {
                if v.is_marker() {
                    break;
                }
            }

            // Check if predicate passed (exit code 0)
            if self.last_exit_code == 0 {
                kept.push(item);
            }
        }

        // Push final marker and all kept items
        self.stack.push(Value::Marker);
        for item in kept {
            self.stack.push(item);
        }

        Ok(())
    }

    /// Map: [block] map - apply block to each item and collect results
    /// Equivalent to: each collect
    pub(crate) fn list_map(&mut self) -> Result<(), EvalError> {
        // Apply each, then collect
        self.list_each()?;
        self.list_collect()?;
        Ok(())
    }

    /// Filter: [predicate] filter - keep items where predicate passes and collect
    /// Equivalent to: keep collect
    pub(crate) fn list_filter(&mut self) -> Result<(), EvalError> {
        // Apply keep, then collect
        self.list_keep()?;
        self.list_collect()?;
        Ok(())
    }
}
