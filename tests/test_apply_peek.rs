#[path = "common/mod.rs"]
mod common;
use common::eval;

#[test]
fn test_apply_executes_block() {
    let output = eval("#[5 3 plus] apply").unwrap();
    assert_eq!(output.trim(), "8");
}

#[test]
fn test_exec_alias() {
    let output = eval("#[\"hello\" echo] exec").unwrap();
    assert_eq!(output.trim(), "hello");
}

#[test]
fn test_peek_shows_top() {
    let output = eval("42 peek").unwrap();
    assert!(output.contains("42"));
}

#[test]
fn test_peek_non_destructive() {
    let output = eval("42 peek dup plus").unwrap();
    assert!(output.contains("84"));
}

#[test]
fn test_peek_all() {
    let output = eval("1 2 3 peek-all").unwrap();
    assert!(output.contains("1") && output.contains("2") && output.contains("3"));
}

#[test]
fn test_at_is_now_a_literal() {
    // @ is no longer a special operator, it's just a word character
    let result = eval("#[\"hello\" echo] @");
    assert!(result.is_ok(), "@ should be treated as a literal word, not an error");
}
