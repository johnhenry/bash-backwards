//! Tests for block syntax change: #[...] for blocks, [...] for array literals

#[path = "common/mod.rs"]
mod common;
#[allow(unused_imports)]
use common::{eval, eval_exit_code, Evaluator, lex, parse};

// === #[...] block syntax (required by task) ===

#[test]
fn test_hash_block_executes() {
    let output = eval("#[5 3 plus] apply").unwrap();
    assert_eq!(output.trim(), "8");
}

#[test]
fn test_hash_block_definition() {
    let output = eval("#[dup mul] :square\n5 square").unwrap();
    assert_eq!(output.trim(), "25");
}

#[test]
fn test_hash_block_times() {
    let output = eval("#[\"hi\" echo] 3 times").unwrap();
    let count = output.matches("hi").count();
    assert_eq!(count, 3, "times should execute block 3 times");
}

#[test]
fn test_hash_block_if() {
    let output = eval(r#"#["no" echo] #["yes" echo] true if"#).unwrap();
    assert!(output.contains("yes"), "if with true condition should run then-branch: {}", output);
}

#[test]
fn test_nested_hash_blocks() {
    let output = eval("#[#[\"inner\" echo] apply] apply").unwrap();
    assert_eq!(output.trim(), "inner");
}

// === Additional #[...] block syntax tests ===

#[test]
fn test_hash_block_basic() {
    let output = eval("#[hello echo] apply").unwrap();
    assert!(output.contains("hello"));
}

#[test]
fn test_hash_block_definition_suffix() {
    let output = eval("#[.bak suffix] :backup file.txt backup").unwrap();
    assert_eq!(output.trim(), "file.txt.bak");
}

#[test]
fn test_hash_block_if_else() {
    let output = eval(r#"#["no" echo] #["yes" echo] true if"#).unwrap();
    assert!(output.contains("yes"));
}

#[test]
fn test_hash_block_times_count() {
    let output = eval("#[x echo] 3 times").unwrap();
    let count = output.matches("x").count();
    assert_eq!(count, 3);
}

// === [...] array literals ===

#[test]
fn test_array_literal() {
    let output = eval("[1 2 3] typeof").unwrap();
    assert_eq!(output.trim(), "list");
}

#[test]
fn test_array_literal_to_json() {
    let output = eval("[1 2 3] to-json").unwrap();
    assert!(output.contains("1") && output.contains("2") && output.contains("3"));
}

#[test]
fn test_array_literal_sum() {
    let output = eval("[1 2 3 4 5] sum").unwrap();
    assert_eq!(output.trim(), "15");
}

#[test]
fn test_array_literal_spaces() {
    let output = eval("[  1   2   3  ] sum").unwrap();
    assert_eq!(output.trim(), "6");
}

#[test]
fn test_empty_array() {
    let output = eval("[] to-json").unwrap();
    assert_eq!(output.trim(), "[]");
}

#[test]
fn test_empty_block() {
    // Empty block should still push a block to the stack
    let output = eval("#[] typeof").unwrap();
    assert_eq!(output.trim(), "block");
}

// === Nested blocks ===

#[test]
fn test_nested_blocks() {
    let output = eval("#[#[hello echo] apply] apply").unwrap();
    assert!(output.contains("hello"));
}

// === Array inside block ===

#[test]
fn test_array_nested_in_block() {
    let output = eval("#[[1 2 3] sum] apply").unwrap();
    assert_eq!(output.trim(), "6");
}

// === Block operators ===

#[test]
fn test_hash_block_pipe() {
    let output = eval("ls #[Cargo grep] |").unwrap();
    assert!(output.contains("Cargo"));
}

#[test]
fn test_hash_block_and() {
    let output = eval("#[true] #[ok echo] &&").unwrap();
    assert!(output.contains("ok"));
}

#[test]
fn test_hash_block_or() {
    let output = eval("#[false] #[fallback echo] ||").unwrap();
    assert!(output.contains("fallback"));
}

// === Comments still work ===

#[test]
fn test_hash_comment_still_works() {
    let output = eval("hello echo # this is a comment").unwrap();
    assert!(output.contains("hello"));
}

#[test]
fn test_hash_block_in_comment_context() {
    // #[ at start of token should be block, # followed by space should be comment
    let output = eval("#[hello echo] apply # comment after block").unwrap();
    assert!(output.contains("hello"));
}
