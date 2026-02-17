use super::{Evaluator, EvalError};
use crate::ast::Value;
use std::collections::HashMap;

impl Evaluator {
    pub(crate) fn builtin_sum(&mut self) -> Result<(), EvalError> {
        let val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("sum requires a list".into()))?;

        let total: f64 = match val {
            Value::List(items) => {
                items.iter().filter_map(|v| match v {
                    Value::Number(n) => Some(*n),
                    Value::Literal(s) | Value::Output(s) => s.trim().parse().ok(),
                    _ => None,
                }).sum()
            }
            _ => return Err(EvalError::TypeError {
                expected: "List".into(),
                got: format!("{:?}", val),
            }),
        };

        self.stack.push(Value::Number(total));
        self.last_exit_code = 0;
        Ok(())
    }

    pub(crate) fn builtin_avg(&mut self) -> Result<(), EvalError> {
        let val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("avg requires a list".into()))?;

        let (total, count) = match val {
            Value::List(items) => {
                let nums: Vec<f64> = items.iter().filter_map(|v| match v {
                    Value::Number(n) => Some(*n),
                    Value::Literal(s) | Value::Output(s) => s.trim().parse().ok(),
                    _ => None,
                }).collect();
                (nums.iter().sum::<f64>(), nums.len())
            }
            _ => return Err(EvalError::TypeError {
                expected: "List".into(),
                got: format!("{:?}", val),
            }),
        };

        let avg = if count > 0 { total / count as f64 } else { 0.0 };
        self.stack.push(Value::Number(avg));
        self.last_exit_code = 0;
        Ok(())
    }

    pub(crate) fn builtin_min(&mut self) -> Result<(), EvalError> {
        let val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("min requires a list".into()))?;

        let result = match val {
            Value::List(items) => {
                items.iter().filter_map(|v| match v {
                    Value::Number(n) => Some(*n),
                    Value::Literal(s) | Value::Output(s) => s.trim().parse().ok(),
                    _ => None,
                }).fold(f64::INFINITY, f64::min)
            }
            _ => return Err(EvalError::TypeError {
                expected: "List".into(),
                got: format!("{:?}", val),
            }),
        };

        if result.is_infinite() {
            self.stack.push(Value::Nil);
        } else {
            self.stack.push(Value::Number(result));
        }
        self.last_exit_code = 0;
        Ok(())
    }

    pub(crate) fn builtin_max(&mut self) -> Result<(), EvalError> {
        let val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("max requires a list".into()))?;

        let result = match val {
            Value::List(items) => {
                items.iter().filter_map(|v| match v {
                    Value::Number(n) => Some(*n),
                    Value::Literal(s) | Value::Output(s) => s.trim().parse().ok(),
                    _ => None,
                }).fold(f64::NEG_INFINITY, f64::max)
            }
            _ => return Err(EvalError::TypeError {
                expected: "List".into(),
                got: format!("{:?}", val),
            }),
        };

        if result.is_infinite() {
            self.stack.push(Value::Nil);
        } else {
            self.stack.push(Value::Number(result));
        }
        self.last_exit_code = 0;
        Ok(())
    }

    pub(crate) fn builtin_count(&mut self) -> Result<(), EvalError> {
        let val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("count requires a value".into()))?;

        let n = match &val {
            Value::List(items) => items.len(),
            Value::Table { rows, .. } => rows.len(),
            Value::Literal(s) | Value::Output(s) => s.lines().count(),
            Value::Map(m) => m.len(),
            _ => 1,
        };

        self.stack.push(Value::Number(n as f64));
        self.last_exit_code = 0;
        Ok(())
    }

    /// reduce: Aggregate list to single value using a block
    /// list init #[block] reduce -> result
    /// The block receives (accumulator, current-item) and should return new accumulator
    pub(crate) fn builtin_reduce(&mut self) -> Result<(), EvalError> {
        let block = self.pop_block()?;
        let init = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("reduce requires initial value".into()))?;
        let list = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("reduce requires list".into()))?;

        let items = match list {
            Value::List(items) => items,
            _ => return Err(EvalError::TypeError {
                expected: "List".into(),
                got: format!("{:?}", list),
            }),
        };

        let mut acc = init;
        for item in items {
            // Push accumulator and current item
            self.stack.push(acc);
            self.stack.push(item);
            // Execute the block
            for expr in &block {
                self.eval_expr(expr)?;
            }
            // Pop the result as new accumulator
            acc = self.stack.pop().ok_or_else(||
                EvalError::StackUnderflow("reduce block must return a value".into()))?;
        }

        self.stack.push(acc);
        self.last_exit_code = 0;
        Ok(())
    }

    /// fold: Alias for reduce (catamorphism)
    /// list init #[block] fold -> result
    pub(crate) fn builtin_fold(&mut self) -> Result<(), EvalError> {
        self.builtin_reduce()
    }

    /// bend: Unfold/generate a list from a seed (anamorphism)
    /// seed [predicate] [step] bend -> list
    /// Starting from seed, while predicate is true: collect current value,
    /// run step to produce next seed. Returns collected values as a list.
    pub(crate) fn builtin_bend(&mut self) -> Result<(), EvalError> {
        let step_block = self.pop_block()?;
        let pred_block = self.pop_block()?;
        let seed = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("bend requires seed value".into()))?;

        let mut current = seed;
        let mut collected: Vec<Value> = Vec::new();

        // Safety limit to prevent infinite loops
        let max_iterations = 10000;
        let mut iterations = 0;

        loop {
            if iterations >= max_iterations {
                return Err(EvalError::ExecError(
                    format!("bend: exceeded {} iterations (possible infinite loop)", max_iterations)
                ));
            }
            iterations += 1;

            // Test predicate: push current, run predicate block
            self.stack.push(current.clone());
            for expr in &pred_block {
                self.eval_expr(expr)?;
            }

            // Check if predicate passed (exit code 0)
            // Pop whatever the predicate left on the stack
            while let Some(v) = self.stack.last() {
                if v.is_marker() {
                    break;
                }
                self.stack.pop();
            }

            if self.last_exit_code != 0 {
                // Predicate failed, stop generating
                break;
            }

            // Collect current value
            collected.push(current.clone());

            // Run step block to get next value
            self.stack.push(current);
            for expr in &step_block {
                self.eval_expr(expr)?;
            }

            // Pop the result as next seed
            current = self.stack.pop().ok_or_else(||
                EvalError::StackUnderflow("bend step block must return a value".into()))?;
        }

        self.stack.push(Value::List(collected));
        self.last_exit_code = 0;
        Ok(())
    }

    pub(crate) fn builtin_group_by(&mut self) -> Result<(), EvalError> {
        let col = self.pop_string()?;
        let table = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("group-by requires a table".into()))?;

        match table {
            Value::Table { columns, rows } => {
                let col_idx = columns.iter().position(|c| c == &col)
                    .ok_or_else(|| EvalError::ExecError(
                        format!("group-by: column '{}' not found", col)
                    ))?;

                let mut groups: HashMap<String, Vec<Vec<Value>>> = HashMap::new();

                for row in rows {
                    let key = row.get(col_idx)
                        .and_then(|v| v.as_arg())
                        .unwrap_or_default();
                    groups.entry(key).or_default().push(row);
                }

                // Convert groups to Record of Tables
                let map: HashMap<String, Value> = groups.into_iter()
                    .map(|(k, rows)| {
                        (k, Value::Table { columns: columns.clone(), rows })
                    })
                    .collect();

                self.stack.push(Value::Map(map));
            }
            _ => return Err(EvalError::TypeError {
                expected: "Table".into(),
                got: format!("{:?}", table),
            }),
        }

        self.last_exit_code = 0;
        Ok(())
    }

    pub(crate) fn builtin_unique(&mut self) -> Result<(), EvalError> {
        use std::collections::HashSet;

        let val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("unique requires a value".into()))?;

        match val {
            Value::List(items) => {
                let mut seen = HashSet::new();
                let unique: Vec<Value> = items.into_iter()
                    .filter(|v| {
                        let key = v.as_arg().unwrap_or_default();
                        seen.insert(key)
                    })
                    .collect();
                self.stack.push(Value::List(unique));
            }
            Value::Table { columns, rows } => {
                let mut seen = HashSet::new();
                let unique: Vec<Vec<Value>> = rows.into_iter()
                    .filter(|row| {
                        let key: String = row.iter()
                            .filter_map(|v| v.as_arg())
                            .collect::<Vec<_>>()
                            .join("\t");
                        seen.insert(key)
                    })
                    .collect();
                self.stack.push(Value::Table { columns, rows: unique });
            }
            _ => self.stack.push(val),
        }

        self.last_exit_code = 0;
        Ok(())
    }

    pub(crate) fn builtin_reverse(&mut self) -> Result<(), EvalError> {
        let val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("reverse requires a value".into()))?;

        match val {
            Value::List(mut items) => {
                items.reverse();
                self.stack.push(Value::List(items));
            }
            Value::Table { columns, mut rows } => {
                rows.reverse();
                self.stack.push(Value::Table { columns, rows });
            }
            Value::Literal(s) => {
                self.stack.push(Value::Literal(s.chars().rev().collect()));
            }
            Value::Output(s) => {
                self.stack.push(Value::Output(s.chars().rev().collect()));
            }
            _ => self.stack.push(val),
        }

        self.last_exit_code = 0;
        Ok(())
    }

    pub(crate) fn builtin_flatten(&mut self) -> Result<(), EvalError> {
        let val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("flatten requires a list".into()))?;

        match val {
            Value::List(items) => {
                let mut flattened = Vec::new();
                for item in items {
                    match item {
                        Value::List(inner) => flattened.extend(inner),
                        other => flattened.push(other),
                    }
                }
                self.stack.push(Value::List(flattened));
            }
            _ => self.stack.push(val),
        }

        self.last_exit_code = 0;
        Ok(())
    }

    /// reject: Inverse of keep - removes items matching predicate
    /// list [predicate] reject -> filtered list (items where predicate is false)
    pub(crate) fn builtin_reject(&mut self) -> Result<(), EvalError> {
        let block = self.pop_block()?;
        let list = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("reject requires list".into()))?;

        let items = match list {
            Value::List(items) => items,
            _ => return Err(EvalError::TypeError {
                expected: "List".into(),
                got: format!("{:?}", list),
            }),
        };

        let mut kept = Vec::new();
        for item in items {
            // Push item and execute predicate
            self.stack.push(item.clone());
            for expr in &block {
                self.eval_expr(expr)?;
            }
            // Keep if predicate FAILS (exit code != 0)
            if self.last_exit_code != 0 {
                kept.push(item);
            }
        }

        self.stack.push(Value::List(kept));
        self.last_exit_code = 0;
        Ok(())
    }

    /// reject-where: Inverse of where - removes rows matching predicate from tables
    /// table [predicate] reject-where -> filtered table
    pub(crate) fn builtin_reject_where(&mut self) -> Result<(), EvalError> {
        let block = self.pop_block()?;
        let table = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("reject-where requires table".into()))?;

        let (columns, rows) = match table {
            Value::Table { columns, rows } => (columns, rows),
            _ => return Err(EvalError::TypeError {
                expected: "Table".into(),
                got: format!("{:?}", table),
            }),
        };

        let mut kept_rows = Vec::new();
        for row in rows {
            // Create a record for this row
            let record: std::collections::HashMap<String, Value> = columns.iter()
                .zip(row.iter())
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();

            // Save stack to isolate predicate execution
            let saved_stack = std::mem::take(&mut self.stack);
            self.stack.push(Value::Map(record));

            for expr in &block {
                self.eval_expr(expr)?;
            }

            // Keep if predicate FAILS (exit code != 0)
            let keep = self.last_exit_code != 0;
            self.stack = saved_stack;

            if keep {
                kept_rows.push(row);
            }
        }

        self.stack.push(Value::Table { columns, rows: kept_rows });
        self.last_exit_code = 0;
        Ok(())
    }

    /// duplicates: Return only items that appear more than once (supplementary to unique)
    /// list duplicates -> list of duplicate items
    pub(crate) fn builtin_duplicates(&mut self) -> Result<(), EvalError> {
        let val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("duplicates requires list".into()))?;

        let items = match val {
            Value::List(items) => items,
            _ => return Err(EvalError::TypeError {
                expected: "List".into(),
                got: format!("{:?}", val),
            }),
        };

        // Count occurrences
        let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for item in &items {
            let key = item.as_arg().unwrap_or_default();
            *counts.entry(key).or_insert(0) += 1;
        }

        // Keep only items that appear more than once
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        let duplicates: Vec<Value> = items.into_iter()
            .filter(|item| {
                let key = item.as_arg().unwrap_or_default();
                if counts.get(&key).copied().unwrap_or(0) > 1 && !seen.contains(&key) {
                    seen.insert(key);
                    true
                } else {
                    false
                }
            })
            .collect();

        self.stack.push(Value::List(duplicates));
        self.last_exit_code = 0;
        Ok(())
    }
}
