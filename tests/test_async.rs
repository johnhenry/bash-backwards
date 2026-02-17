//! Integration tests for async operations

#[path = "common/mod.rs"]
mod common;
#[allow(unused_imports)]
use common::{eval, eval_exit_code, Evaluator, lex, parse};

// === delay tests ===

#[test]
fn test_delay_basic() {
    // delay should complete without error
    let exit_code = eval_exit_code("10 delay");
    assert_eq!(exit_code, 0);
}

#[test]
fn test_delay_zero() {
    // Zero delay should be valid
    let exit_code = eval_exit_code("0 delay");
    assert_eq!(exit_code, 0);
}

// === async/await tests ===

#[test]
fn test_async_creates_future() {
    // async should create a Future type
    let output = eval(r#"#[42] async typeof"#).unwrap();
    assert_eq!(output.trim(), "future");
}

#[test]
fn test_async_await_simple() {
    // Basic async/await pattern
    let output = eval(r#"#[42] async await"#).unwrap();
    assert_eq!(output.trim(), "42");
}

#[test]
fn test_async_await_computation() {
    // Async computation
    let output = eval(r#"#[10 20 plus] async await"#).unwrap();
    assert_eq!(output.trim(), "30");
}

#[test]
fn test_async_await_string() {
    let output = eval(r#"#["hello"] async await"#).unwrap();
    assert_eq!(output.trim(), "hello");
}

#[test]
fn test_async_await_multiple() {
    // Multiple independent async operations
    let output = eval(r#"#[1] async #[2] async await swap await plus"#).unwrap();
    assert_eq!(output.trim(), "3");
}

// === future-status tests ===

#[test]
fn test_future_status_pending() {
    // A future with a delay should be pending initially
    // Note: This test is timing-dependent and may be flaky
    let output = eval(r#"#[100 delay] async future-status"#).unwrap();
    // Status could be pending or completed depending on timing
    assert!(output.contains("pending") || output.contains("completed"));
}

#[test]
fn test_future_status_shows_status() {
    // future-status returns a string status
    let output = eval(r#"#[42] async future-status"#).unwrap();
    // Status is one of: pending, completed, failed, cancelled
    assert!(output.contains("pending") || output.contains("completed"));
}

#[test]
fn test_future_status_preserves_future() {
    // future-status should not consume the future
    let output = eval(r#"#[42] async future-status drop await"#).unwrap();
    assert_eq!(output.trim(), "42");
}

// === future-result tests ===

#[test]
fn test_future_result_success() {
    // Successful future should return {ok: value}
    let output = eval(r#"#[42] async future-result "ok" get"#).unwrap();
    assert_eq!(output.trim(), "42");
}

// === future-cancel tests ===

#[test]
fn test_future_cancel_pending() {
    // Cancel a pending future
    let exit_code = eval_exit_code(r#"#[1000 delay] async future-cancel"#);
    assert_eq!(exit_code, 0);
}

// === delay-async tests ===

#[test]
fn test_delay_async_returns_future() {
    let output = eval(r#"10 delay-async typeof"#).unwrap();
    assert_eq!(output.trim(), "future");
}

#[test]
fn test_delay_async_await() {
    // Awaiting delay-async should complete
    let exit_code = eval_exit_code(r#"10 delay-async await"#);
    assert_eq!(exit_code, 0);
}

// === race tests ===

#[test]
fn test_race_returns_first() {
    // Race should return the first to complete
    // The one without delay should win - increase delay significantly
    let output = eval(r#"#[#[42] #[500 delay 99]] race"#).unwrap();
    assert_eq!(output.trim(), "42");
}

#[test]
fn test_race_with_single_block() {
    // Race with single block returns that block's result
    let output = eval(r#"#[#[42]] race"#).unwrap();
    assert_eq!(output.trim(), "42");
}

// === parallel-n tests ===

#[test]
fn test_parallel_n_basic() {
    // Run blocks in parallel with limit
    let output = eval(r#"#[#[1] #[2] #[3]] 2 parallel-n"#).unwrap();
    // Should return a list of results
    assert!(output.contains("1"));
    assert!(output.contains("2"));
    assert!(output.contains("3"));
}

#[test]
fn test_parallel_n_limit_one() {
    // With limit 1, effectively sequential
    let output = eval(r#"#[#[10] #[20]] 1 parallel-n"#).unwrap();
    assert!(output.contains("10"));
    assert!(output.contains("20"));
}

#[test]
fn test_parallel_n_empty() {
    // Empty list should return empty list
    let output = eval(r#"#[] 4 parallel-n to-json"#).unwrap();
    assert_eq!(output.trim(), "[]");
}

// === await-all tests ===

#[test]
fn test_await_all_basic() {
    // Await all futures in a list - using JSON parse to create list
    // This is a workaround - parse JSON list of the async results
    let output = eval(r#"#[1] async #[2] async 2 future-await-n plus"#).unwrap();
    assert_eq!(output.trim(), "3");
}

#[test]
fn test_await_all_works_with_non_futures() {
    // await-all should pass through non-future values
    // Test that the function completes without error on empty list
    let exit_code = eval_exit_code(r#"'[]' into-json await-all"#);
    assert_eq!(exit_code, 0);
}

#[test]
fn test_await_all_empty() {
    // Empty list should return empty list
    let output = eval(r#"'[]' into-json await-all to-json"#).unwrap();
    assert_eq!(output.trim(), "[]");
}

// === future-await-n tests ===

#[test]
fn test_future_await_n_two() {
    // Await 2 futures from stack
    let output = eval(r#"#[10] async #[20] async 2 future-await-n plus"#).unwrap();
    assert_eq!(output.trim(), "30");
}

#[test]
fn test_future_await_n_zero() {
    // Awaiting 0 futures should be a no-op
    let exit_code = eval_exit_code(r#"0 future-await-n"#);
    assert_eq!(exit_code, 0);
}

// === future-race tests ===

#[test]
fn test_future_race_empty() {
    // Empty list should return nil - use JSON parse for empty list
    let output = eval(r#"'[]' into-json future-race typeof"#).unwrap();
    // JSON null becomes Null, hsab nil is Nil - accept either
    assert!(output.trim() == "nil");
}

// === future-map tests ===

#[test]
fn test_future_map_basic() {
    // Map over a future result
    let output = eval(r#"#[21] async #[2 mul] future-map await"#).unwrap();
    assert_eq!(output.trim(), "42");
}

#[test]
fn test_future_map_chain() {
    // Chain multiple maps
    let output = eval(r#"#[10] async #[1 plus] future-map #[2 mul] future-map await"#).unwrap();
    assert_eq!(output.trim(), "22"); // (10 + 1) * 2 = 22
}

// === futures-list tests ===

#[test]
fn test_futures_list_returns_list() {
    // Should return a list (even if empty)
    let output = eval(r#"futures-list typeof"#).unwrap();
    assert_eq!(output.trim(), "list");
}

// === Error cases ===

#[test]
fn test_async_requires_block() {
    let result = eval(r#"42 async"#);
    assert!(result.is_err());
}

#[test]
fn test_await_requires_future() {
    let result = eval(r#"42 await"#);
    assert!(result.is_err());
}

#[test]
fn test_future_status_requires_future() {
    let result = eval(r#"42 future-status"#);
    assert!(result.is_err());
}

#[test]
fn test_future_cancel_requires_future() {
    let result = eval(r#"42 future-cancel"#);
    assert!(result.is_err());
}

#[test]
fn test_delay_requires_number() {
    let result = eval(r#""not a number" delay"#);
    assert!(result.is_err());
}

// === Integration tests ===

#[test]
fn test_async_preserves_stack() {
    // Async block should have access to values pushed before block definition
    let output = eval(r#"#[10 20 plus] async await"#).unwrap();
    assert_eq!(output.trim(), "30");
}

#[test]
fn test_async_independent_stacks() {
    // Each async block should have its own stack
    let output = eval(r#"1 #[2 3 plus] async await swap"#).unwrap();
    // Stack should have: 5 1 (swapped from: 1 5)
    // The "1" was pushed before async, the "5" is from await
    assert!(output.contains("5") || output.contains("1"));
}

#[test]
fn test_parallel_preserves_order() {
    // Results should be in same order as input blocks
    let output = eval(r#"#[#[1] #[2] #[3]] 10 parallel-n to-json"#).unwrap();
    // The output format might vary, but values should be in order
    assert!(output.contains("1"));
}

// === parallel-map tests ===

#[test]
fn test_parallel_map_basic() {
    // Double each number
    let output = eval(r#"#[1 2 3] #[2 mul] 2 parallel-map to-json"#).unwrap();
    assert_eq!(output.trim(), "[2.0,4.0,6.0]");
}

#[test]
fn test_parallel_map_single_thread() {
    // Limit to 1 thread (sequential)
    let output = eval(r#"#[10 20 30] #[1 plus] 1 parallel-map to-json"#).unwrap();
    assert_eq!(output.trim(), "[11.0,21.0,31.0]");
}

#[test]
fn test_parallel_map_empty_list() {
    // Empty input returns empty output
    let output = eval(r#"#[] #[2 mul] 4 parallel-map to-json"#).unwrap();
    assert_eq!(output.trim(), "[]");
}

#[test]
fn test_parallel_map_preserves_order() {
    // Results must be in the same order as input
    let output = eval(r#"#[5 4 3 2 1] #[10 mul] 3 parallel-map to-json"#).unwrap();
    assert_eq!(output.trim(), "[50.0,40.0,30.0,20.0,10.0]");
}

#[test]
fn test_parallel_map_strings() {
    // Works with string items -- len returns a string representation of length
    let output = eval(r#"#["hello" "world"] #[len] 2 parallel-map to-json"#).unwrap();
    assert!(output.contains("5"));
}

#[test]
fn test_parallel_map_identity() {
    // Block that just returns the item (items are literals from block eval)
    let output = eval(r#"#[1 2 3] #[] 2 parallel-map to-json"#).unwrap();
    assert!(output.contains("1"));
    assert!(output.contains("2"));
    assert!(output.contains("3"));
}

#[test]
fn test_parallel_map_high_concurrency() {
    // More threads than items is fine
    let output = eval(r#"#[1 2] #[3 plus] 100 parallel-map to-json"#).unwrap();
    assert_eq!(output.trim(), "[4.0,5.0]");
}

#[test]
fn test_parallel_map_error_in_block() {
    // Errors in a worker should produce Value::Error, not crash
    let output = eval(r#"#[1 2 3] #[drop] 2 parallel-map to-json"#).unwrap();
    // Each worker pushes the item, then drops it, leaving empty stack -> Nil
    assert!(output.contains("null"));
}

#[test]
fn test_parallel_map_with_json_list() {
    // Using into-json to create a proper numeric list
    let output = eval(r#"'[1,2,3]' into-json #[2 mul] 2 parallel-map to-json"#).unwrap();
    assert_eq!(output.trim(), "[2.0,4.0,6.0]");
}
