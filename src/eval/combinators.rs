use super::{Evaluator, EvalError};
use crate::ast::{Expr, Value};

impl Evaluator {
    /// fanout: Run one value through multiple blocks, collect all results
    /// value [block1] [block2] [block3] fanout -> result1 result2 result3
    pub(crate) fn builtin_fanout(&mut self) -> Result<(), EvalError> {
        // Collect all blocks from stack (until we hit a non-block value)
        let mut blocks: Vec<Vec<Expr>> = Vec::new();
        while let Some(Value::Block(exprs)) = self.stack.last().cloned() {
            self.stack.pop();
            blocks.push(exprs);
        }
        blocks.reverse(); // Restore original order (first pushed = first executed)

        if blocks.is_empty() {
            return Err(EvalError::ExecError("fanout: no blocks provided".into()));
        }

        // Pop the input value
        let input = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("fanout: no input value".into()))?;

        // Run input through each block, collecting results
        let mut results: Vec<Value> = Vec::new();
        for block in &blocks {
            // Push input for this block
            self.stack.push(input.clone());

            // Execute block
            for expr in block {
                self.eval_expr(expr)?;
            }

            // Collect result (top of stack after block execution)
            if let Some(result) = self.stack.pop() {
                results.push(result);
            } else {
                results.push(Value::Nil);
            }
        }

        // Push all results onto stack
        for result in results {
            self.stack.push(result);
        }

        self.last_exit_code = 0;
        Ok(())
    }

    /// zip: Pair two lists element-wise
    /// list1 list2 zip -> [[a1,b1], [a2,b2], ...]
    pub(crate) fn builtin_zip(&mut self) -> Result<(), EvalError> {
        let list2 = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("zip: requires two lists".into()))?;
        let list1 = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("zip: requires two lists".into()))?;

        let items1 = match list1 {
            Value::List(items) => items,
            _ => return Err(EvalError::TypeError {
                expected: "List".into(),
                got: format!("{:?}", list1),
            }),
        };

        let items2 = match list2 {
            Value::List(items) => items,
            _ => return Err(EvalError::TypeError {
                expected: "List".into(),
                got: format!("{:?}", list2),
            }),
        };

        // Zip together (stops at shorter list)
        let zipped: Vec<Value> = items1.into_iter()
            .zip(items2.into_iter())
            .map(|(a, b)| Value::List(vec![a, b]))
            .collect();

        self.stack.push(Value::List(zipped));
        self.last_exit_code = 0;
        Ok(())
    }

    /// cross: Cartesian product of two lists
    /// list1 list2 cross -> [[a1,b1], [a1,b2], [a2,b1], [a2,b2], ...]
    pub(crate) fn builtin_cross(&mut self) -> Result<(), EvalError> {
        let list2 = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("cross: requires two lists".into()))?;
        let list1 = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("cross: requires two lists".into()))?;

        let items1 = match list1 {
            Value::List(items) => items,
            _ => return Err(EvalError::TypeError {
                expected: "List".into(),
                got: format!("{:?}", list1),
            }),
        };

        let items2 = match list2 {
            Value::List(items) => items,
            _ => return Err(EvalError::TypeError {
                expected: "List".into(),
                got: format!("{:?}", list2),
            }),
        };

        // Cartesian product
        let mut product: Vec<Value> = Vec::new();
        for a in &items1 {
            for b in &items2 {
                product.push(Value::List(vec![a.clone(), b.clone()]));
            }
        }

        self.stack.push(Value::List(product));
        self.last_exit_code = 0;
        Ok(())
    }

    /// retry: Retry a block N times until success
    /// N [block] retry -> result (or error after N failures)
    pub(crate) fn builtin_retry(&mut self) -> Result<(), EvalError> {
        let block = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("retry: requires a block".into()))?;
        let count = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("retry: requires retry count".into()))?;

        let block_exprs = match block {
            Value::Block(exprs) => exprs,
            _ => return Err(EvalError::TypeError {
                expected: "Block".into(),
                got: format!("{:?}", block),
            }),
        };

        let max_tries = match count {
            Value::Number(n) => n as usize,
            Value::Literal(s) | Value::Output(s) => s.parse::<usize>().map_err(|_|
                EvalError::TypeError {
                    expected: "Number".into(),
                    got: "String".into(),
                })?,
            _ => return Err(EvalError::TypeError {
                expected: "Number".into(),
                got: format!("{:?}", count),
            }),
        };

        if max_tries == 0 {
            return Err(EvalError::ExecError("retry: count must be > 0".into()));
        }

        let mut last_error: Option<EvalError> = None;

        for attempt in 1..=max_tries {
            // Try executing the block
            let result = (|| {
                for expr in &block_exprs {
                    self.eval_expr(expr)?;
                }
                Ok(())
            })();

            match result {
                Ok(()) if self.last_exit_code == 0 => {
                    // Success!
                    return Ok(());
                }
                Ok(()) => {
                    // Block completed but with non-zero exit code
                    last_error = Some(EvalError::ExecError(
                        format!("retry: attempt {}/{} failed with exit code {}",
                                attempt, max_tries, self.last_exit_code)
                    ));
                }
                Err(e) => {
                    last_error = Some(e);
                }
            }

            // Clear any error values from failed attempt if not last try
            if attempt < max_tries {
                // Small delay between retries (could make configurable)
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }

        // All retries failed
        Err(last_error.unwrap_or_else(||
            EvalError::ExecError("retry: all attempts failed".into())))
    }

    /// retry-delay: Retry with configurable delay between attempts
    /// [block] N ms retry-delay -> result
    /// Stack: [block] count delay_ms (delay on top)
    pub(crate) fn builtin_retry_delay(&mut self) -> Result<(), EvalError> {
        // Pop in LIFO order: delay_ms, count, block
        let delay_ms = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("retry-delay: requires delay in ms".into()))?;
        let count = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("retry-delay: requires retry count".into()))?;
        let block = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("retry-delay: requires a block".into()))?;

        let block_exprs = match block {
            Value::Block(exprs) => exprs,
            _ => return Err(EvalError::TypeError {
                expected: "Block".into(),
                got: format!("{:?}", block),
            }),
        };

        let max_tries = match count {
            Value::Number(n) => n as usize,
            Value::Literal(s) | Value::Output(s) => s.parse::<usize>().map_err(|_|
                EvalError::TypeError {
                    expected: "Number".into(),
                    got: "String".into(),
                })?,
            _ => return Err(EvalError::TypeError {
                expected: "Number".into(),
                got: format!("{:?}", count),
            }),
        };

        let delay: u64 = match delay_ms {
            Value::Number(n) => n as u64,
            Value::Literal(s) | Value::Output(s) => s.parse::<u64>().map_err(|_|
                EvalError::TypeError {
                    expected: "Number (milliseconds)".into(),
                    got: "String".into(),
                })?,
            _ => return Err(EvalError::TypeError {
                expected: "Number (milliseconds)".into(),
                got: format!("{:?}", delay_ms),
            }),
        };

        if max_tries == 0 {
            return Err(EvalError::ExecError("retry-delay: count must be > 0".into()));
        }

        let mut last_error: Option<EvalError> = None;

        for attempt in 1..=max_tries {
            let result = (|| {
                for expr in &block_exprs {
                    self.eval_expr(expr)?;
                }
                Ok(())
            })();

            match result {
                Ok(()) if self.last_exit_code == 0 => {
                    return Ok(());
                }
                Ok(()) => {
                    last_error = Some(EvalError::ExecError(
                        format!("retry-delay: attempt {}/{} failed with exit code {}",
                                attempt, max_tries, self.last_exit_code)
                    ));
                }
                Err(e) => {
                    last_error = Some(e);
                }
            }

            if attempt < max_tries {
                std::thread::sleep(std::time::Duration::from_millis(delay));
            }
        }

        Err(last_error.unwrap_or_else(||
            EvalError::ExecError("retry-delay: all attempts failed".into())))
    }

    /// compose: Combine multiple blocks into a single pipeline block
    /// [block1] [block2] [block3] compose -> [block1 block2 block3]
    /// Or from a list: list-of-blocks compose -> single-block
    pub(crate) fn builtin_compose(&mut self) -> Result<(), EvalError> {
        // Check if top of stack is a list of blocks or individual blocks
        let top = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("compose: requires blocks".into()))?;

        let blocks: Vec<Vec<Expr>> = match top {
            // If it's a list, extract blocks from it
            Value::List(items) => {
                let mut blocks = Vec::new();
                for item in items {
                    match item {
                        Value::Block(exprs) => blocks.push(exprs),
                        _ => return Err(EvalError::TypeError {
                            expected: "Block".into(),
                            got: format!("{:?}", item),
                        }),
                    }
                }
                blocks
            }
            // If it's a single block, collect it and any other blocks from stack
            Value::Block(exprs) => {
                let mut blocks = vec![exprs];
                // Collect more blocks from stack
                while let Some(Value::Block(more_exprs)) = self.stack.last().cloned() {
                    self.stack.pop();
                    blocks.push(more_exprs);
                }
                blocks.reverse(); // Restore original order
                blocks
            }
            _ => return Err(EvalError::TypeError {
                expected: "Block or List of Blocks".into(),
                got: format!("{:?}", top),
            }),
        };

        if blocks.is_empty() {
            return Err(EvalError::ExecError("compose: no blocks to compose".into()));
        }

        // Concatenate all expressions into a single block
        let mut composed: Vec<Expr> = Vec::new();
        for block in blocks {
            composed.extend(block);
        }

        self.stack.push(Value::Block(composed));
        self.last_exit_code = 0;
        Ok(())
    }
}
