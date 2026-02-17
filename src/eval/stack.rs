use super::{Evaluator, EvalError};
use crate::ast::Value;

impl Evaluator {
    pub(crate) fn stack_dup(&mut self) -> Result<(), EvalError> {
        let top = self
            .stack
            .last()
            .cloned()
            .ok_or_else(|| EvalError::StackUnderflow("dup".into()))?;
        self.stack.push(top);
        Ok(())
    }

    pub(crate) fn stack_swap(&mut self) -> Result<(), EvalError> {
        let len = self.stack.len();
        if len < 2 {
            return Err(EvalError::StackUnderflow("swap".into()));
        }
        self.stack.swap(len - 1, len - 2);
        Ok(())
    }

    pub(crate) fn stack_drop(&mut self) -> Result<(), EvalError> {
        self.stack
            .pop()
            .ok_or_else(|| EvalError::StackUnderflow("drop".into()))?;
        Ok(())
    }

    pub(crate) fn stack_over(&mut self) -> Result<(), EvalError> {
        let len = self.stack.len();
        if len < 2 {
            return Err(EvalError::StackUnderflow("over".into()));
        }
        let second = self.stack[len - 2].clone();
        self.stack.push(second);
        Ok(())
    }

    pub(crate) fn stack_rot(&mut self) -> Result<(), EvalError> {
        let len = self.stack.len();
        if len < 3 {
            return Err(EvalError::StackUnderflow("rot".into()));
        }
        let third = self.stack.remove(len - 3);
        self.stack.push(third);
        Ok(())
    }

    pub(crate) fn stack_depth(&mut self) -> Result<(), EvalError> {
        let depth = self.stack.len();
        self.stack.push(Value::Literal(depth.to_string()));
        Ok(())
    }

    /// dig: Pull the Nth item from top to the top of the stack
    /// Usage: 1 2 3 4 5  3 dig -> 1 2 4 5 3 (pulls item at position 3 from top)
    pub(crate) fn stack_dig(&mut self) -> Result<(), EvalError> {
        let n = self.pop_number("dig")? as usize;
        let len = self.stack.len();
        if n < 1 || n > len {
            return Err(EvalError::ExecError(
                format!("dig: index {} out of range (stack has {} items)", n, len)
            ));
        }
        let item = self.stack.remove(len - n);
        self.stack.push(item);
        Ok(())
    }

    /// peek: Non-destructively print the top of the stack to stderr
    /// The value remains on the stack
    pub(crate) fn stack_peek(&mut self) -> Result<(), EvalError> {
        let top = self
            .stack
            .last()
            .ok_or_else(|| EvalError::StackUnderflow("peek".into()))?;
        let display = match top {
            Value::Literal(s) => s.clone(),
            Value::Output(s) => s.trim_end_matches('\n').to_string(),
            Value::Number(n) => {
                if n.fract() == 0.0 && n.abs() < i64::MAX as f64 {
                    format!("{}", *n as i64)
                } else {
                    n.to_string()
                }
            }
            Value::Bool(b) => b.to_string(),
            Value::Nil => "nil".to_string(),
            Value::Marker => "<marker>".to_string(),
            Value::Block(exprs) => format!("[block:{}]", exprs.len()),
            Value::List(items) => format!("[list:{}]", items.len()),
            Value::Map(m) => format!("{{record:{}}}", m.len()),
            Value::Table { columns, rows } => format!("<table:{}x{}>", columns.len(), rows.len()),
            Value::Error { message, .. } => format!("Error: {}", message),
            Value::Media { mime_type, data, .. } => format!("<media:{}:{}B>", mime_type, data.len()),
            Value::Link { url, .. } => format!("<link:{}>", url),
            Value::Bytes(data) => format!("<bytes:{}B>", data.len()),
            Value::BigInt(n) => format!("<bigint:{}>", n),
            Value::Future { id, .. } => format!("<future:{}>", id),
        };
        eprintln!("[peek] {}", display);
        Ok(())
    }

    /// peek-all: Non-destructively print the entire stack to stderr
    /// All values remain on the stack
    pub(crate) fn stack_peek_all(&mut self) -> Result<(), EvalError> {
        if self.stack.is_empty() {
            eprintln!("[peek-all] (empty stack)");
            return Ok(());
        }
        eprintln!("[peek-all] stack ({} items):", self.stack.len());
        for (i, val) in self.stack.iter().enumerate() {
            let display = match val {
                Value::Literal(s) => s.clone(),
                Value::Output(s) => s.trim_end_matches('\n').to_string(),
                Value::Number(n) => {
                    if n.fract() == 0.0 && n.abs() < i64::MAX as f64 {
                        format!("{}", *n as i64)
                    } else {
                        n.to_string()
                    }
                }
                Value::Bool(b) => b.to_string(),
                Value::Nil => "nil".to_string(),
                Value::Marker => "<marker>".to_string(),
                Value::Block(exprs) => format!("[block:{}]", exprs.len()),
                Value::List(items) => format!("[list:{}]", items.len()),
                Value::Map(m) => format!("{{record:{}}}", m.len()),
                Value::Table { columns, rows } => format!("<table:{}x{}>", columns.len(), rows.len()),
                Value::Error { message, .. } => format!("Error: {}", message),
                Value::Media { mime_type, data, .. } => format!("<media:{}:{}B>", mime_type, data.len()),
                Value::Link { url, .. } => format!("<link:{}>", url),
                Value::Bytes(data) => format!("<bytes:{}B>", data.len()),
                Value::BigInt(n) => format!("<bigint:{}>", n),
                Value::Future { id, .. } => format!("<future:{}>", id),
            };
            eprintln!("  {}. {}", i, display);
        }
        Ok(())
    }

    /// bury: Push the top item down to the Nth position from top
    /// Usage: 1 2 3 4 5  3 bury -> 1 2 5 3 4 (buries top item to position 3)
    pub(crate) fn stack_bury(&mut self) -> Result<(), EvalError> {
        let n = self.pop_number("bury")? as usize;
        let top = self.stack.pop().ok_or_else(|| EvalError::StackUnderflow("bury".into()))?;
        let len = self.stack.len();
        if n < 1 || n > len + 1 {
            return Err(EvalError::ExecError(
                format!("bury: index {} out of range (stack has {} items)", n, len)
            ));
        }
        self.stack.insert(len + 1 - n, top);
        Ok(())
    }
}
