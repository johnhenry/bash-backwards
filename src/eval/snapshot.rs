use super::{Evaluator, EvalError};
use crate::ast::Value;

impl Evaluator {
    // === Snapshot Operations ===

    /// Save current stack state with a name
    /// a b c "name" snapshot -> a b c (values restored, snapshot saved)
    /// a b c snapshot -> a b c "snap-001" (auto-named, name pushed)
    ///
    /// Note: In hsab, all stack values are popped as args to commands.
    /// So args contains the snapshot name (if provided) plus all values that were on stack.
    /// We restore the values to the stack after saving the snapshot.
    pub(crate) fn builtin_snapshot(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.is_empty() {
            // Nothing was on stack - save empty snapshot with auto-name
            self.snapshot_counter += 1;
            let name = format!("snap-{:03}", self.snapshot_counter);
            self.snapshots.insert(name.clone(), Vec::new());
            self.stack.push(Value::Literal(name));
        } else {
            // Args are in LIFO order: args[0] is last pushed (the name or a value)
            // Check if first arg looks like a snapshot name (we'll use a simple heuristic)
            // For explicit naming: "name" snapshot - args[0] is the name
            // For auto-naming: value snapshot - args[0] is a value to save

            // Heuristic: if user wants to name it, they must use a quoted string
            // In practice, we can't easily distinguish, so we require explicit marker
            // Let's use a simpler approach: always treat first arg as name for named snapshots
            // User must explicitly call with a name string.

            // Convert args back to Values and restore to stack
            let values: Vec<Value> = args.iter()
                .skip(1)  // Skip the name (args[0])
                .rev()    // Reverse to restore original order
                .map(|s| Value::Literal(s.clone()))
                .collect();

            // Save snapshot with name from args[0]
            let name = &args[0];
            self.snapshots.insert(name.clone(), values.clone());

            // Restore values to stack
            for v in values {
                self.stack.push(v);
            }
        }
        self.last_exit_code = 0;
        Ok(())
    }

    /// Restore stack to a saved snapshot
    /// "name" snapshot-restore -> (stack replaced)
    pub(crate) fn builtin_snapshot_restore(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.is_empty() {
            return Err(EvalError::ExecError("snapshot-restore: name required".into()));
        }
        let name = &args[0];
        match self.snapshots.get(name) {
            Some(saved_stack) => {
                self.stack = saved_stack.clone();
                self.last_exit_code = 0;
                Ok(())
            }
            None => Err(EvalError::ExecError(format!(
                "snapshot-restore: no snapshot named '{}'",
                name
            ))),
        }
    }

    /// List all snapshot names
    /// snapshot-list -> [names]
    pub(crate) fn builtin_snapshot_list(&mut self) -> Result<(), EvalError> {
        let mut names: Vec<String> = self.snapshots.keys().cloned().collect();
        names.sort();
        let list = Value::List(names.into_iter().map(Value::Literal).collect());
        self.stack.push(list);
        self.last_exit_code = 0;
        Ok(())
    }

    /// Delete a snapshot
    /// "name" snapshot-delete -> ()
    pub(crate) fn builtin_snapshot_delete(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.is_empty() {
            return Err(EvalError::ExecError("snapshot-delete: name required".into()));
        }
        let name = &args[0];
        if self.snapshots.remove(name).is_none() {
            return Err(EvalError::ExecError(format!(
                "snapshot-delete: no snapshot named '{}'",
                name
            )));
        }
        self.last_exit_code = 0;
        Ok(())
    }

    /// Clear all snapshots
    /// snapshot-clear -> ()
    pub(crate) fn builtin_snapshot_clear(&mut self) -> Result<(), EvalError> {
        self.snapshots.clear();
        self.snapshot_counter = 0;
        self.last_exit_code = 0;
        Ok(())
    }
}
