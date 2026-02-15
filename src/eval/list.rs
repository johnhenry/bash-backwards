use super::{Evaluator, EvalError};
use crate::ast::Value;

impl Evaluator {
    /// Spread: split a value into separate stack items
    /// - String: split by newlines
    /// - List: push each item
    /// - Map: push each value (order undefined)
    pub(crate) fn list_spread(&mut self) -> Result<(), EvalError> {
        let value = self.pop_value_or_err()?;

        // Push marker to indicate start of spread items
        self.stack.push(Value::Marker);

        match value {
            Value::List(items) => {
                // Spread list items onto stack
                for item in items {
                    self.stack.push(item);
                }
            }
            Value::Map(map) => {
                // Spread map values onto stack (order undefined per spec)
                for (_key, val) in map {
                    self.stack.push(val);
                }
            }
            _ => {
                // String/other: split by newlines (original behavior)
                let text = value.as_arg().unwrap_or_default();
                for line in text.lines() {
                    if !line.is_empty() {
                        self.stack.push(Value::Literal(line.to_string()));
                    }
                }
            }
        }

        Ok(())
    }

    /// fields: {record} ["key1" "key2"] fields -> val1 val2
    /// Extract specific fields from a record (no marker)
    pub(crate) fn builtin_fields(&mut self) -> Result<(), EvalError> {
        let keys_val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("fields requires list of keys".into()))?;
        let record_val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("fields requires a record".into()))?;

        let keys: Vec<String> = match keys_val {
            Value::List(items) => items.into_iter()
                .filter_map(|v| v.as_arg())
                .collect(),
            _ => return Err(EvalError::TypeError {
                expected: "list of keys".into(),
                got: format!("{:?}", keys_val),
            }),
        };

        let map = match record_val {
            Value::Map(m) => m,
            _ => return Err(EvalError::TypeError {
                expected: "record".into(),
                got: format!("{:?}", record_val),
            }),
        };

        // Push values for each key (Nil if missing)
        for key in keys {
            let val = map.get(&key).cloned().unwrap_or(Value::Nil);
            self.stack.push(val);
        }

        self.last_exit_code = 0;
        Ok(())
    }

    /// fields-keys: {record} fields-keys -> marker k1 v1 k2 v2 ...
    /// Extract key-value pairs from a record
    pub(crate) fn builtin_fields_keys(&mut self) -> Result<(), EvalError> {
        let record_val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("fields-keys requires a record".into()))?;

        let map = match record_val {
            Value::Map(m) => m,
            _ => return Err(EvalError::TypeError {
                expected: "record".into(),
                got: format!("{:?}", record_val),
            }),
        };

        // Push marker
        self.stack.push(Value::Marker);

        // Push key-value pairs (order undefined)
        for (key, val) in map {
            self.stack.push(Value::Literal(key));
            self.stack.push(val);
        }

        self.last_exit_code = 0;
        Ok(())
    }

    /// spread-head: [list] spread-head -> head [tail]
    /// Split first element from rest
    pub(crate) fn builtin_spread_head(&mut self) -> Result<(), EvalError> {
        let list_val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("spread-head requires a list".into()))?;

        let items = match list_val {
            Value::List(items) => items,
            _ => return Err(EvalError::TypeError {
                expected: "list".into(),
                got: format!("{:?}", list_val),
            }),
        };

        if items.is_empty() {
            self.stack.push(Value::Nil);
            self.stack.push(Value::List(vec![]));
        } else {
            let mut items = items;
            let head = items.remove(0);
            self.stack.push(head);
            self.stack.push(Value::List(items));
        }

        self.last_exit_code = 0;
        Ok(())
    }

    /// spread-tail: [list] spread-tail -> [init] last
    /// Split last element from init
    pub(crate) fn builtin_spread_tail(&mut self) -> Result<(), EvalError> {
        let list_val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("spread-tail requires a list".into()))?;

        let items = match list_val {
            Value::List(items) => items,
            _ => return Err(EvalError::TypeError {
                expected: "list".into(),
                got: format!("{:?}", list_val),
            }),
        };

        if items.is_empty() {
            self.stack.push(Value::List(vec![]));
            self.stack.push(Value::Nil);
        } else {
            let mut items = items;
            let last = items.pop().unwrap();
            self.stack.push(Value::List(items));
            self.stack.push(last);
        }

        self.last_exit_code = 0;
        Ok(())
    }

    /// spread-n: [list] N spread-n -> item1 item2 ... itemN [rest]
    /// Take first N elements, leave rest as list
    pub(crate) fn builtin_spread_n(&mut self) -> Result<(), EvalError> {
        let n_val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("spread-n requires count".into()))?;
        let list_val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("spread-n requires a list".into()))?;

        let n: usize = match n_val {
            Value::Number(num) => num as usize,
            Value::Literal(s) | Value::Output(s) => s.parse().map_err(|_| EvalError::TypeError {
                expected: "integer".into(),
                got: s,
            })?,
            _ => return Err(EvalError::TypeError {
                expected: "integer".into(),
                got: format!("{:?}", n_val),
            }),
        };

        let items = match list_val {
            Value::List(items) => items,
            _ => return Err(EvalError::TypeError {
                expected: "list".into(),
                got: format!("{:?}", list_val),
            }),
        };

        // Take up to N items
        let take_count = n.min(items.len());
        for i in 0..take_count {
            self.stack.push(items[i].clone());
        }

        // Push rest as list
        let rest: Vec<Value> = items.into_iter().skip(take_count).collect();
        self.stack.push(Value::List(rest));

        self.last_exit_code = 0;
        Ok(())
    }

    /// spread-to: value ["name1" "name2" ...] spread-to -> (binds to locals)
    /// Bind values to named locals
    pub(crate) fn builtin_spread_to(&mut self) -> Result<(), EvalError> {
        let names_val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("spread-to requires list of names".into()))?;
        let value = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("spread-to requires a value".into()))?;

        let names: Vec<String> = match names_val {
            Value::List(items) => items.into_iter()
                .filter_map(|v| v.as_arg())
                .collect(),
            _ => return Err(EvalError::TypeError {
                expected: "list of names".into(),
                got: format!("{:?}", names_val),
            }),
        };

        // Get values to bind
        let values: Vec<Value> = match value {
            Value::List(items) => items,
            Value::Map(map) => {
                // For records, extract values in order of names
                names.iter()
                    .map(|name| map.get(name).cloned().unwrap_or(Value::Nil))
                    .collect()
            }
            _ => return Err(EvalError::TypeError {
                expected: "list or record".into(),
                got: format!("{:?}", value),
            }),
        };

        // Check length match for lists
        if values.len() < names.len() {
            return Err(EvalError::ExecError(format!(
                "spread-to: {} names but only {} values",
                names.len(), values.len()
            )));
        }

        // Bind each name to corresponding value
        // Ensure we have a scope to bind to
        if self.local_values.is_empty() {
            self.local_values.push(std::collections::HashMap::new());
        }
        if let Some(scope) = self.local_values.last_mut() {
            for (name, val) in names.into_iter().zip(values.into_iter()) {
                scope.insert(name, val);
            }
        }

        self.last_exit_code = 0;
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
