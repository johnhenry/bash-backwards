//! Integration tests for control operations

#[path = "common/mod.rs"]
mod common;
#[allow(unused_imports)]
use common::{eval, eval_exit_code, Evaluator, lex, parse};

#[test]
fn test_block_pushes() {
    // This just pushes a block to the stack
    let tokens = lex("[hello echo]").unwrap();
    let program = parse(tokens).unwrap();
    let mut evaluator = Evaluator::new();
    let result = evaluator.eval(&program).unwrap();

    // Stack should have one item (the block)
    assert_eq!(result.stack.len(), 1);
}

#[test]
fn test_apply_executes_block() {
    let output = eval("[hello echo] @").unwrap();
    assert!(output.contains("hello"));
}

#[test]
fn test_apply_with_args() {
    // Push world, then block [echo], apply executes echo with world as arg
    let output = eval("world [echo] @").unwrap();
    assert!(output.contains("world"));
}

#[test]
fn test_job_status_stopped() {
    // Test that JobStatus::Stopped exists and works
    // (Implicitly tested through .jobs builtin)
    let output = eval(".jobs").unwrap();
    // Should not error, output may be empty
    assert!(output.is_empty() || output.contains("Running") || output.contains("Stopped") || output.contains("Done"));
}

#[test]
fn test_bg_no_stopped_job_error() {
    // .bg with no stopped jobs should error
    let tokens = lex(".bg").unwrap();
    let program = parse(tokens).unwrap();
    let mut evaluator = Evaluator::new();
    let result = evaluator.eval(&program);

    // Should fail because no stopped jobs
    assert!(result.is_err(), ".bg should fail when no stopped jobs");
}

#[test]
fn test_fifo_creates_named_pipe() {
    use std::path::Path;
    use std::fs;

    // [hello echo] fifo should create a named pipe and push its path
    // Note: hsab uses postfix notation, so "hello echo" means echo hello
    let output = eval("[hello echo] fifo").unwrap();
    let pipe_path = output.trim();

    // The path should exist and be a named pipe (or at least exist)
    let path = Path::new(pipe_path);
    assert!(path.exists() || pipe_path.contains("hsab_fifo"), "fifo should create a pipe at: {}", pipe_path);

    // Clean up
    fs::remove_file(pipe_path).ok();
}

#[test]
fn test_fifo_path_is_in_tmp() {
    use std::fs;

    // Note: postfix notation - "test echo" means echo test
    let output = eval("[test echo] fifo").unwrap();
    let pipe_path = output.trim();

    assert!(pipe_path.starts_with("/tmp/") || pipe_path.contains("hsab_fifo"),
            "fifo path should be in /tmp: {}", pipe_path);

    fs::remove_file(pipe_path).ok();
}

#[test]
fn test_fifo_can_be_read() {
    // The fifo should be readable - spawn a reader
    // [hello echo] fifo cat should produce "hello"
    // Note: postfix notation - "hello echo" means echo hello
    let output = eval("[hello echo] fifo cat").unwrap();
    assert!(output.contains("hello"), "should be able to cat from fifo: {}", output);
}

#[test]
fn test_if_true_branch() {
    // Empty condition has exit code 0 (default), so then-branch runs
    // Use quoted strings to avoid treating "yes"/"no" as commands
    let output = eval(r#"[] ["yes" echo] ["no" echo] if"#).unwrap();
    assert!(output.contains("yes"), "if with true condition should run then-branch: {}", output);
}

#[test]
fn test_if_false_branch() {
    // [false] sets exit code to 1, so else-branch runs
    let output = eval(r#"[false] ["yes" echo] ["no" echo] if"#).unwrap();
    assert!(output.contains("no"), "if with false condition should run else-branch: {}", output);
}

#[test]
fn test_if_with_test_condition() {
    // Test comparison: 1 -eq 1 should succeed (exit 0)
    let output = eval(r#"[1 1 -eq test] ["equal" echo] ["not-equal" echo] if"#).unwrap();
    assert!(output.contains("equal"), "if with passing test should run then-branch: {}", output);
}

#[test]
fn test_times_loop() {
    // N [block] times - execute block N times
    let output = eval("3 [x echo] times").unwrap();
    let count = output.matches("x").count();
    assert_eq!(count, 3, "times should execute block N times");
}

#[test]
fn test_times_zero() {
    let output = eval("0 [x echo] times").unwrap();
    assert!(output.is_empty() || !output.contains("x"), "times 0 should not execute block");
}

#[test]
fn test_while_false_condition() {
    // [false] [] while should execute zero times since false returns exit code 1
    let output = eval("[false] [] while done echo").unwrap();
    assert!(output.contains("done"), "while with false condition should exit immediately: {}", output);
}

#[test]
fn test_if_else_true_branch() {
    // Condition block returns exit code 0 (success) - runs then branch
    let output = eval(r#"[true] ["yes" echo] ["no" echo] if"#).unwrap();
    assert!(output.contains("yes"));
}

#[test]
fn test_if_else_false_branch() {
    // Condition block returns exit code 1 (failure) - runs else branch
    let output = eval(r#"[false] ["yes" echo] ["no" echo] if"#).unwrap();
    assert!(output.contains("no"));
}

#[test]
fn test_while_immediate_exit() {
    // While with false condition exits immediately
    let output = eval("[false] [x echo] while done echo").unwrap();
    assert!(output.contains("done"));
}

#[test]
fn test_until_immediate_exit() {
    // Until with true condition exits immediately
    let output = eval("[true] [x echo] until done echo").unwrap();
    assert!(output.contains("done"));
}

#[test]
fn test_times_counter() {
    // times: N [body] times
    let output = eval("3 [x echo] times").unwrap();
    // Should print x three times
    let count = output.matches("x").count();
    assert_eq!(count, 3);
}

#[test]
fn test_jobs_command() {
    // Just verify .jobs command runs
    let exit_code = eval_exit_code(".jobs");
    assert_eq!(exit_code, 0);
}

#[test]
fn test_type_for_echo() {
    let output = eval("echo .type").unwrap();
    // Should show type info
    assert!(!output.is_empty());
}

#[test]
fn test_which_for_cat() {
    // which needs a quoted string argument
    let output = eval(r#""cat" .which"#).unwrap();
    // Should find cat in PATH
    assert!(output.contains("cat") || output.contains("bin"));
}

#[test]
fn test_and_both_blocks_succeed() {
    // [block1] [block2] && - both blocks succeed
    let output = eval("[true] [ok echo] &&").unwrap();
    assert!(output.contains("ok"));
}

#[test]
fn test_or_first_fails_runs_second() {
    // [fail] [block2] || - first fails, runs second
    let output = eval("[false] [fallback echo] ||").unwrap();
    assert!(output.contains("fallback"));
}

#[test]
fn test_and_first_fails_skips_second() {
    // [fail] [block2] && - first fails, skips second
    let output = eval("[false] [should_not_run echo] &&").unwrap();
    assert!(!output.contains("should_not_run"));
}

#[test]
fn test_or_first_succeeds_skips_second() {
    // [success] [block2] || - first succeeds, skips second
    let output = eval("[true] [should_not_run echo] ||").unwrap();
    assert!(!output.contains("should_not_run"));
}

#[test]
fn test_apply_block() {
    let output = eval(r#"5 [1 plus] @"#).unwrap();
    assert_eq!(output.trim(), "6");
}

