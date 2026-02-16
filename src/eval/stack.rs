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
