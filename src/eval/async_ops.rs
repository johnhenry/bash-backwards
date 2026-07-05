//! Async and concurrent operations for hsab
//!
//! Provides futures, parallel execution with limits, and delays.
//! Note: `timeout` is in process.rs, `retry` is in combinators.rs

use super::{Evaluator, EvalError};
use crate::ast::{Expr, Value, FutureState};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use crate::util::lock_or_recover;

impl Evaluator {
    // === Core Async Operations ===

    /// async: #[block] async -> Future
    /// Run a block in the background and return a Future handle
    pub(crate) fn builtin_async(&mut self) -> Result<(), EvalError> {
        let block = self.pop_block()?;

        // Generate unique ID for this future
        self.future_counter += 1;
        let id = format!("{:04x}", self.future_counter);

        // Create shared state
        let state = Arc::new(Mutex::new(FutureState::Pending));
        let state_clone = Arc::clone(&state);

        // Clone what we need for the thread
        let cwd = self.cwd.clone();
        let definitions = self.definitions.clone();
        let locals = self.local_values.clone();

        // Spawn thread to execute the block
        let handle = thread::spawn(move || {
            let mut eval = Evaluator::new();
            eval.cwd = cwd;
            eval.definitions = definitions;
            eval.local_values = locals;

            // Execute the block
            match eval.eval_block(&block) {
                Ok(_) => {
                    // Get result from stack (top value or Nil)
                    let result = eval.stack.pop().unwrap_or(Value::Nil);
                    let mut guard = lock_or_recover(&state_clone);
                    *guard = FutureState::Completed(Box::new(result));
                }
                Err(e) => {
                    let mut guard = lock_or_recover(&state_clone);
                    *guard = FutureState::Failed(e.to_string());
                }
            }
        });

        // Store handle for potential cancellation
        self.future_handles.insert(id.clone(), handle);
        // Register in the futures registry so futures-list can enumerate it
        self.futures.insert(id.clone(), Arc::clone(&state));

        // Push Future value onto stack
        self.stack.push(Value::Future { id, state });
        self.last_exit_code = 0;
        Ok(())
    }

    /// await: Future await -> result
    /// Block until future completes and return the result
    pub(crate) fn builtin_await(&mut self) -> Result<(), EvalError> {
        let future = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("await requires a Future".into()))?;

        match future {
            Value::Future { id, state } => {
                // Wait for completion by polling
                loop {
                    let guard = lock_or_recover(&state);
                    match &*guard {
                        FutureState::Pending => {
                            drop(guard);
                            thread::sleep(Duration::from_millis(10));
                        }
                        FutureState::Completed(value) => {
                            self.stack.push((**value).clone());
                            self.last_exit_code = 0;
                            // Clean up handle
                            drop(guard);
                            if let Some(handle) = self.future_handles.remove(&id) {
                                let _ = handle.join();
                            }
                            return Ok(());
                        }
                        FutureState::Failed(msg) => {
                            let msg = msg.clone();
                            // Clean up handle
                            drop(guard);
                            if let Some(handle) = self.future_handles.remove(&id) {
                                let _ = handle.join();
                            }
                            return Err(EvalError::ExecError(format!("future failed: {}", msg)));
                        }
                        FutureState::Cancelled => {
                            drop(guard);
                            if let Some(handle) = self.future_handles.remove(&id) {
                                let _ = handle.join();
                            }
                            return Err(EvalError::ExecError("future was cancelled".into()));
                        }
                    }
                }
            }
            _ => Err(EvalError::TypeError {
                expected: "Future".into(),
                got: future.type_name().to_string(),
            }),
        }
    }

    /// future-status: Future future-status -> "pending" | "completed" | "failed" | "cancelled"
    pub(crate) fn builtin_future_status(&mut self) -> Result<(), EvalError> {
        let future = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("future-status requires a Future".into()))?;

        match &future {
            Value::Future { state, .. } => {
                let guard = lock_or_recover(&state);
                let status = match &*guard {
                    FutureState::Pending => "pending",
                    FutureState::Completed(_) => "completed",
                    FutureState::Failed(_) => "failed",
                    FutureState::Cancelled => "cancelled",
                };
                drop(guard);
                // Put future back on stack (non-consuming)
                self.stack.push(future);
                self.stack.push(Value::Literal(status.to_string()));
                self.last_exit_code = 0;
                Ok(())
            }
            _ => Err(EvalError::TypeError {
                expected: "Future".into(),
                got: future.type_name().to_string(),
            }),
        }
    }

    /// future-result: Future future-result -> {ok: value} | {err: message}
    /// Non-throwing result access
    pub(crate) fn builtin_future_result(&mut self) -> Result<(), EvalError> {
        let future = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("future-result requires a Future".into()))?;

        match future {
            Value::Future { id, state } => {
                // Wait for completion
                loop {
                    let guard = lock_or_recover(&state);
                    match &*guard {
                        FutureState::Pending => {
                            drop(guard);
                            thread::sleep(Duration::from_millis(10));
                        }
                        FutureState::Completed(value) => {
                            let mut result = indexmap::IndexMap::new();
                            result.insert("ok".to_string(), (**value).clone());
                            self.stack.push(Value::Map(result));
                            self.last_exit_code = 0;
                            drop(guard);
                            if let Some(handle) = self.future_handles.remove(&id) {
                                let _ = handle.join();
                            }
                            return Ok(());
                        }
                        FutureState::Failed(msg) => {
                            let mut result = indexmap::IndexMap::new();
                            result.insert("err".to_string(), Value::Literal(msg.clone()));
                            self.stack.push(Value::Map(result));
                            self.last_exit_code = 1;
                            drop(guard);
                            if let Some(handle) = self.future_handles.remove(&id) {
                                let _ = handle.join();
                            }
                            return Ok(());
                        }
                        FutureState::Cancelled => {
                            let mut result = indexmap::IndexMap::new();
                            result.insert("err".to_string(), Value::Literal("cancelled".into()));
                            self.stack.push(Value::Map(result));
                            self.last_exit_code = 1;
                            drop(guard);
                            if let Some(handle) = self.future_handles.remove(&id) {
                                let _ = handle.join();
                            }
                            return Ok(());
                        }
                    }
                }
            }
            _ => Err(EvalError::TypeError {
                expected: "Future".into(),
                got: future.type_name().to_string(),
            }),
        }
    }

    /// future-cancel: Future future-cancel -> ()
    /// Cancel a running future
    pub(crate) fn builtin_future_cancel(&mut self) -> Result<(), EvalError> {
        let future = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("future-cancel requires a Future".into()))?;

        match future {
            Value::Future { id: _, state } => {
                // Mark as cancelled
                let mut guard = lock_or_recover(&state);
                if matches!(*guard, FutureState::Pending) {
                    *guard = FutureState::Cancelled;
                }
                drop(guard);

                // Note: We can't actually kill the thread in Rust safely,
                // but we mark it as cancelled so await will return an error.
                // The thread will complete but its result will be ignored.

                self.last_exit_code = 0;
                Ok(())
            }
            _ => Err(EvalError::TypeError {
                expected: "Future".into(),
                got: future.type_name().to_string(),
            }),
        }
    }

    // === Delay Operations ===

    /// delay: ms delay -> ()
    /// Sleep for specified milliseconds (blocking)
    pub(crate) fn builtin_delay(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.is_empty() {
            return Err(EvalError::ExecError("delay requires milliseconds".into()));
        }

        let ms: u64 = args[0].parse().map_err(|_| EvalError::TypeError {
            expected: "integer milliseconds".into(),
            got: args[0].clone(),
        })?;

        thread::sleep(Duration::from_millis(ms));
        self.last_exit_code = 0;
        Ok(())
    }

    /// delay-async: ms delay-async -> Future
    /// Return a Future that resolves after the delay
    pub(crate) fn builtin_delay_async(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.is_empty() {
            return Err(EvalError::ExecError("delay-async requires milliseconds".into()));
        }

        let ms: u64 = args[0].parse().map_err(|_| EvalError::TypeError {
            expected: "integer milliseconds".into(),
            got: args[0].clone(),
        })?;

        // Generate unique ID
        self.future_counter += 1;
        let id = format!("{:04x}", self.future_counter);

        // Create shared state
        let state = Arc::new(Mutex::new(FutureState::Pending));
        let state_clone = Arc::clone(&state);

        // Spawn thread that sleeps then completes
        let handle = thread::spawn(move || {
            thread::sleep(Duration::from_millis(ms));
            let mut guard = lock_or_recover(&state_clone);
            if matches!(*guard, FutureState::Pending) {
                *guard = FutureState::Completed(Box::new(Value::Nil));
            }
        });

        self.future_handles.insert(id.clone(), handle);
        self.futures.insert(id.clone(), Arc::clone(&state));
        self.stack.push(Value::Future { id, state });
        self.last_exit_code = 0;
        Ok(())
    }

    // === Parallel with Limit ===

    /// parallel-n: #[#[blocks]] N parallel-n -> [results]
    /// Run blocks in parallel with concurrency limit
    pub(crate) fn builtin_parallel_n(&mut self) -> Result<(), EvalError> {
        let n_val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("parallel-n requires concurrency limit".into()))?;
        let blocks_val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("parallel-n requires list of blocks".into()))?;

        let limit: usize = match n_val {
            Value::Number(n) => n as usize,
            Value::Literal(s) => s.parse().map_err(|_| EvalError::TypeError {
                expected: "integer".into(),
                got: s,
            })?,
            _ => return Err(EvalError::TypeError {
                expected: "integer".into(),
                got: n_val.type_name().to_string(),
            }),
        };

        let blocks: Vec<Vec<Expr>> = match blocks_val {
            Value::List(items) => {
                items.into_iter().filter_map(|v| {
                    if let Value::Block(exprs) = v {
                        Some(exprs)
                    } else {
                        None
                    }
                }).collect()
            }
            // Also handle a block containing blocks (e.g., #[#[a] #[b] #[c]])
            Value::Block(exprs) => {
                exprs.into_iter().filter_map(|e| {
                    if let Expr::Block(inner) = e {
                        Some(inner)
                    } else {
                        None
                    }
                }).collect()
            }
            _ => return Err(EvalError::TypeError {
                expected: "list of blocks".into(),
                got: blocks_val.type_name().to_string(),
            }),
        };

        if blocks.is_empty() {
            self.stack.push(Value::List(vec![]));
            self.last_exit_code = 0;
            return Ok(());
        }

        let cwd = self.cwd.clone();
        let definitions = self.definitions.clone();
        let locals = self.local_values.clone();

        // Process blocks in batches of `limit`
        let mut results = Vec::new();

        for chunk in blocks.chunks(limit) {
            let handles: Vec<_> = chunk.iter().map(|block| {
                let block = block.clone();
                let cwd = cwd.clone();
                let definitions = definitions.clone();
                let locals = locals.clone();

                thread::spawn(move || {
                    let mut eval = Evaluator::new();
                    eval.cwd = cwd;
                    eval.definitions = definitions;
                    eval.local_values = locals;

                    match eval.eval_block(&block) {
                        Ok(_) => eval.stack.pop().unwrap_or(Value::Nil),
                        Err(e) => Value::Error {
                            kind: "EvalError".into(),
                            message: e.to_string(),
                            code: None,
                            source: None,
                            command: None,
                        },
                    }
                })
            }).collect();

            // Wait for this batch
            for handle in handles {
                results.push(handle.join().unwrap_or(Value::Nil));
            }
        }

        self.stack.push(Value::List(results));
        self.last_exit_code = 0;
        Ok(())
    }

    // === Parallel Map ===

    /// parallel-map: list #[block] N parallel-map -> [results]
    /// Apply a block to each item in a list with bounded concurrency.
    /// Each thread gets one item pushed onto its stack, then runs the block.
    /// Results are collected in the original order.
    pub(crate) fn builtin_parallel_map(&mut self) -> Result<(), EvalError> {
        let limit = self.pop_number("parallel-map")? as usize;
        let block = self.pop_block()?;
        let list = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("parallel-map requires a list".into()))?;

        let items = match list {
            Value::List(items) => items,
            Value::Block(exprs) => {
                // Evaluate the block to produce a list of values
                let saved_stack = std::mem::take(&mut self.stack);
                self.eval_block(&exprs)?;
                let items = std::mem::replace(&mut self.stack, saved_stack);
                items
            }
            _ => return Err(EvalError::TypeError {
                expected: "List".into(),
                got: list.type_name().to_string(),
            }),
        };

        if items.is_empty() || limit == 0 {
            self.stack.push(Value::List(vec![]));
            self.last_exit_code = 0;
            return Ok(());
        }

        let cwd = self.cwd.clone();
        let definitions = self.definitions.clone();
        let locals = self.local_values.clone();

        let mut results = Vec::with_capacity(items.len());

        for chunk in items.chunks(limit) {
            let handles: Vec<_> = chunk.iter().map(|item| {
                let item = item.clone();
                let block = block.clone();
                let cwd = cwd.clone();
                let definitions = definitions.clone();
                let locals = locals.clone();

                thread::spawn(move || {
                    let mut eval = Evaluator::new();
                    eval.cwd = cwd;
                    eval.definitions = definitions;
                    eval.local_values = locals;

                    // Push the item onto the stack, then run the block
                    eval.stack.push(item);
                    match eval.eval_block(&block) {
                        Ok(_) => eval.stack.pop().unwrap_or(Value::Nil),
                        Err(e) => Value::Error {
                            kind: "EvalError".into(),
                            message: e.to_string(),
                            code: None,
                            source: None,
                            command: None,
                        },
                    }
                })
            }).collect();

            for handle in handles {
                results.push(handle.join().unwrap_or(Value::Nil));
            }
        }

        self.stack.push(Value::List(results));
        self.last_exit_code = 0;
        Ok(())
    }

    // === Race ===

    /// race: #[#[blocks]] race -> result
    /// Run blocks in parallel, return first to complete
    pub(crate) fn builtin_race(&mut self) -> Result<(), EvalError> {
        let blocks_val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("race requires list of blocks".into()))?;

        let blocks: Vec<Vec<Expr>> = match blocks_val {
            Value::List(items) => {
                items.into_iter().filter_map(|v| {
                    if let Value::Block(exprs) = v {
                        Some(exprs)
                    } else {
                        None
                    }
                }).collect()
            }
            // Also handle a block containing blocks (e.g., #[#[a] #[b] #[c]])
            Value::Block(exprs) => {
                exprs.into_iter().filter_map(|e| {
                    if let Expr::Block(inner) = e {
                        Some(inner)
                    } else {
                        None
                    }
                }).collect()
            }
            _ => return Err(EvalError::TypeError {
                expected: "list of blocks".into(),
                got: blocks_val.type_name().to_string(),
            }),
        };

        if blocks.is_empty() {
            self.stack.push(Value::Nil);
            self.last_exit_code = 0;
            return Ok(());
        }

        let cwd = self.cwd.clone();
        let definitions = self.definitions.clone();
        let locals = self.local_values.clone();

        // Shared result - first to complete wins
        let result: Arc<Mutex<Option<Value>>> = Arc::new(Mutex::new(None));

        let handles: Vec<_> = blocks.iter().map(|block| {
            let block = block.clone();
            let cwd = cwd.clone();
            let definitions = definitions.clone();
            let locals = locals.clone();
            let result = Arc::clone(&result);

            thread::spawn(move || {
                let mut eval = Evaluator::new();
                eval.cwd = cwd;
                eval.definitions = definitions;
                eval.local_values = locals;

                let value = match eval.eval_block(&block) {
                    Ok(_) => eval.stack.pop().unwrap_or(Value::Nil),
                    Err(e) => Value::Error {
                        kind: "EvalError".into(),
                        message: e.to_string(),
                        code: None,
                        source: None,
                        command: None,
                    },
                };

                // Try to be the first to set result
                let mut guard = lock_or_recover(&result);
                if guard.is_none() {
                    *guard = Some(value);
                }
            })
        }).collect();

        // Wait for any result
        loop {
            let guard = lock_or_recover(&result);
            if let Some(value) = guard.clone() {
                drop(guard);
                self.stack.push(value);
                self.last_exit_code = 0;
                // Let threads finish in background
                for handle in handles {
                    let _ = handle.join();
                }
                return Ok(());
            }
            drop(guard);
            thread::sleep(Duration::from_millis(10));
        }
    }

    // === Future Combinators ===

    /// await-all: [futures] await-all -> [results]
    /// Await all futures in a list
    pub(crate) fn builtin_await_all(&mut self) -> Result<(), EvalError> {
        let list = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("await-all requires list of futures".into()))?;

        let futures: Vec<Value> = match list {
            Value::List(items) => items,
            _ => return Err(EvalError::TypeError {
                expected: "list".into(),
                got: list.type_name().to_string(),
            }),
        };

        let mut results = Vec::new();

        for future in futures {
            match future {
                Value::Future { id, state } => {
                    // Wait for this future
                    loop {
                        let guard = lock_or_recover(&state);
                        match &*guard {
                            FutureState::Pending => {
                                drop(guard);
                                thread::sleep(Duration::from_millis(10));
                            }
                            FutureState::Completed(value) => {
                                results.push((**value).clone());
                                drop(guard);
                                if let Some(handle) = self.future_handles.remove(&id) {
                                    let _ = handle.join();
                                }
                                break;
                            }
                            FutureState::Failed(msg) => {
                                results.push(Value::Error {
                                    kind: "FutureError".into(),
                                    message: msg.clone(),
                                    code: None,
                                    source: None,
                                    command: None,
                                });
                                drop(guard);
                                if let Some(handle) = self.future_handles.remove(&id) {
                                    let _ = handle.join();
                                }
                                break;
                            }
                            FutureState::Cancelled => {
                                results.push(Value::Error {
                                    kind: "FutureError".into(),
                                    message: "cancelled".into(),
                                    code: None,
                                    source: None,
                                    command: None,
                                });
                                drop(guard);
                                if let Some(handle) = self.future_handles.remove(&id) {
                                    let _ = handle.join();
                                }
                                break;
                            }
                        }
                    }
                }
                other => {
                    // Non-future values pass through
                    results.push(other);
                }
            }
        }

        self.stack.push(Value::List(results));
        self.last_exit_code = 0;
        Ok(())
    }

    /// future-await-n: future1 future2 ... futureN N future-await-n -> result1 result2 ... resultN
    /// Await N futures from the stack, push results back in order
    pub(crate) fn builtin_future_await_n(&mut self) -> Result<(), EvalError> {
        let n_val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("future-await-n requires count".into()))?;

        let n: usize = match n_val {
            Value::Number(num) => num as usize,
            Value::Literal(s) | Value::Output(s) => s.parse().map_err(|_| EvalError::TypeError {
                expected: "integer".into(),
                got: s,
            })?,
            _ => return Err(EvalError::TypeError {
                expected: "integer".into(),
                got: n_val.type_name().to_string(),
            }),
        };

        if n == 0 {
            return Ok(());
        }

        // Collect N futures from stack (in LIFO order)
        let mut futures = Vec::with_capacity(n);
        for _ in 0..n {
            let val = self.stack.pop().ok_or_else(||
                EvalError::StackUnderflow(format!("future-await-n requires {} futures", n)))?;
            futures.push(val);
        }

        // Reverse to get original order (first pushed = first in list)
        futures.reverse();

        // Await each future and collect results
        let mut results = Vec::with_capacity(n);
        for future in futures {
            match future {
                Value::Future { id, state } => {
                    // Wait for this future
                    let result = loop {
                        let guard = lock_or_recover(&state);
                        match &*guard {
                            FutureState::Pending => {
                                drop(guard);
                                thread::sleep(Duration::from_millis(10));
                            }
                            FutureState::Completed(value) => {
                                let val = (**value).clone();
                                drop(guard);
                                if let Some(handle) = self.future_handles.remove(&id) {
                                    let _ = handle.join();
                                }
                                break val;
                            }
                            FutureState::Failed(msg) => {
                                let msg = msg.clone();
                                drop(guard);
                                if let Some(handle) = self.future_handles.remove(&id) {
                                    let _ = handle.join();
                                }
                                break Value::Error {
                                    kind: "FutureError".into(),
                                    message: msg,
                                    code: None,
                                    source: None,
                                    command: None,
                                };
                            }
                            FutureState::Cancelled => {
                                drop(guard);
                                if let Some(handle) = self.future_handles.remove(&id) {
                                    let _ = handle.join();
                                }
                                break Value::Error {
                                    kind: "FutureError".into(),
                                    message: "cancelled".into(),
                                    code: None,
                                    source: None,
                                    command: None,
                                };
                            }
                        }
                    };
                    results.push(result);
                }
                other => {
                    // Non-future values pass through as-is
                    results.push(other);
                }
            }
        }

        // Push results back to stack in order
        for result in results {
            self.stack.push(result);
        }

        self.last_exit_code = 0;
        Ok(())
    }

    /// future-race: [futures] future-race -> result
    /// Return result of first future to complete
    pub(crate) fn builtin_future_race(&mut self) -> Result<(), EvalError> {
        let list = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("future-race requires list of futures".into()))?;

        let futures: Vec<(String, Arc<Mutex<FutureState>>)> = match list {
            Value::List(items) => {
                items.into_iter().filter_map(|v| {
                    if let Value::Future { id, state } = v {
                        Some((id, state))
                    } else {
                        None
                    }
                }).collect()
            }
            _ => return Err(EvalError::TypeError {
                expected: "list of futures".into(),
                got: list.type_name().to_string(),
            }),
        };

        if futures.is_empty() {
            self.stack.push(Value::Nil);
            self.last_exit_code = 0;
            return Ok(());
        }

        // Poll all futures until one completes
        loop {
            for (id, state) in &futures {
                let guard = lock_or_recover(&state);
                match &*guard {
                    FutureState::Pending => continue,
                    FutureState::Completed(value) => {
                        self.stack.push((**value).clone());
                        self.last_exit_code = 0;
                        drop(guard);
                        // Cancel others
                        for (other_id, other_state) in &futures {
                            if other_id != id {
                                let mut g = lock_or_recover(&other_state);
                                if matches!(*g, FutureState::Pending) {
                                    *g = FutureState::Cancelled;
                                }
                            }
                        }
                        return Ok(());
                    }
                    FutureState::Failed(msg) => {
                        // First failure also counts as a result in race
                        let msg = msg.clone();
                        drop(guard);
                        return Err(EvalError::ExecError(format!("future failed: {}", msg)));
                    }
                    FutureState::Cancelled => continue,
                }
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    /// futures-list: futures-list -> [{id status}...]
    /// List every future spawned by this evaluator (async, delay-async,
    /// future-map), in spawn order, with its current status.
    ///
    /// Lifecycle policy: futures stay registered for the lifetime of the
    /// evaluator, including after await/future-cancel; terminal entries
    /// report completed/failed/cancelled rather than being removed.
    pub(crate) fn builtin_futures_list(&mut self) -> Result<(), EvalError> {
        let records: Vec<Value> = self
            .futures
            .iter()
            .map(|(id, state)| {
                let status = match &*lock_or_recover(state) {
                    FutureState::Pending => "pending",
                    FutureState::Completed(_) => "completed",
                    FutureState::Failed(_) => "failed",
                    FutureState::Cancelled => "cancelled",
                };
                let mut record = indexmap::IndexMap::new();
                record.insert("id".to_string(), Value::Literal(id.clone()));
                record.insert("status".to_string(), Value::Literal(status.to_string()));
                Value::Map(record)
            })
            .collect();

        self.stack.push(Value::List(records));
        self.last_exit_code = 0;
        Ok(())
    }

    /// future-map: Future #[block] future-map -> Future
    /// Transform result without awaiting - returns new Future that applies block to result
    pub(crate) fn builtin_future_map(&mut self) -> Result<(), EvalError> {
        let transform_block = self.pop_block()?;
        let future = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("future-map requires a Future".into()))?;

        let (orig_id, orig_state) = match future {
            Value::Future { id, state } => (id, state),
            _ => return Err(EvalError::TypeError {
                expected: "Future".into(),
                got: future.type_name().to_string(),
            }),
        };

        // Generate new future ID
        self.future_counter += 1;
        let new_id = format!("{:04x}", self.future_counter);

        // Create new state for mapped future
        let new_state = Arc::new(Mutex::new(FutureState::Pending));
        let new_state_clone = Arc::clone(&new_state);

        // Clone what we need for the thread
        let cwd = self.cwd.clone();
        let definitions = self.definitions.clone();
        let locals = self.local_values.clone();

        // Spawn thread to wait for original and apply transform
        let handle = thread::spawn(move || {
            // Wait for original future
            let original_result = loop {
                let guard = lock_or_recover(&orig_state);
                match &*guard {
                    FutureState::Pending => {
                        drop(guard);
                        thread::sleep(Duration::from_millis(10));
                    }
                    FutureState::Completed(value) => {
                        break Ok((**value).clone());
                    }
                    FutureState::Failed(msg) => {
                        break Err(msg.clone());
                    }
                    FutureState::Cancelled => {
                        break Err("cancelled".to_string());
                    }
                }
            };

            match original_result {
                Ok(value) => {
                    // Apply transform block to the value
                    let mut eval = Evaluator::new();
                    eval.cwd = cwd;
                    eval.definitions = definitions;
                    eval.local_values = locals;

                    // Push the value onto stack, then run transform
                    eval.stack.push(value);
                    match eval.eval_block(&transform_block) {
                        Ok(_) => {
                            let result = eval.stack.pop().unwrap_or(Value::Nil);
                            let mut guard = lock_or_recover(&new_state_clone);
                            *guard = FutureState::Completed(Box::new(result));
                        }
                        Err(e) => {
                            let mut guard = lock_or_recover(&new_state_clone);
                            *guard = FutureState::Failed(e.to_string());
                        }
                    }
                }
                Err(msg) => {
                    // Propagate failure
                    let mut guard = lock_or_recover(&new_state_clone);
                    *guard = FutureState::Failed(msg);
                }
            }
        });

        // Store handle and push new Future
        self.future_handles.insert(new_id.clone(), handle);
        self.futures.insert(new_id.clone(), Arc::clone(&new_state));

        // Clean up original future handle if we have it
        if let Some(orig_handle) = self.future_handles.remove(&orig_id) {
            // Let it run in background (the new thread is waiting on it)
            std::mem::drop(orig_handle);
        }

        self.stack.push(Value::Future { id: new_id, state: new_state });
        self.last_exit_code = 0;
        Ok(())
    }

    /// Helper: evaluate a block of expressions
    fn eval_block(&mut self, block: &[Expr]) -> Result<(), EvalError> {
        for expr in block {
            self.eval_expr(expr)?;
        }
        Ok(())
    }
}
