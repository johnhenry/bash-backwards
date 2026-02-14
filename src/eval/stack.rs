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
}
