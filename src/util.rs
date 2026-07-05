//! Small shared utilities.

use std::sync::{Mutex, MutexGuard, PoisonError};

/// Lock a mutex, recovering the data if the mutex was poisoned (issue #31).
///
/// A panic while a lock is held poisons the mutex, and every subsequent
/// `.lock().unwrap()` then panics too — for a long-lived REPL that turns one
/// bug in a key handler or future thread into a full shell crash. The state
/// guarded by these mutexes (REPL mirror stack, future results) carries no
/// cross-field invariants, so recovering the possibly partially-updated data
/// is strictly better than dying.
pub fn lock_or_recover<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}
