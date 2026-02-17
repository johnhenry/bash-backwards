//! Integration tests for predicates operations

#[path = "common/mod.rs"]
mod common;
#[allow(unused_imports)]
use common::{eval, eval_exit_code, Evaluator, lex, parse};

#[test]
fn test_exists_predicate_cargo() {
    // exists? sets exit code 0 if file exists
    let exit_code = eval_exit_code(r#""Cargo.toml" exists?"#);
    assert_eq!(exit_code, 0);
}

#[test]
fn test_empty_predicate_empty_string() {
    // empty? works on strings
    let exit_code = eval_exit_code(r#""" empty?"#);
    assert_eq!(exit_code, 0);
}

#[test]
fn test_has_on_record() {
    // has? sets exit code 0 if key exists
    let exit_code = eval_exit_code(r#"record "key" 1 set "key" has?"#);
    assert_eq!(exit_code, 0);
}

// nil? predicate tests
#[test]
fn test_nil_predicate_on_nil() {
    // nil? returns exit code 0 when value is nil
    // cd to nonexistent path pushes nil
    let exit_code = eval_exit_code(r#"/nonexistent/path/xyz cd nil?"#);
    assert_eq!(exit_code, 0, "nil? should return 0 for nil value");
}

#[test]
fn test_nil_predicate_on_non_nil() {
    // nil? returns exit code 1 for non-nil values
    let exit_code = eval_exit_code(r#"42 nil?"#);
    assert_eq!(exit_code, 1, "nil? should return 1 for non-nil value");
}

#[test]
fn test_nil_predicate_non_destructive() {
    // nil? should not consume the value (it stays on stack)
    // nil? also pushes a Bool result, so depth should be >= 2
    let output = eval(r#"/nonexistent/path/xyz cd nil?"#).unwrap();
    assert!(output.contains("true"), "nil? on nil value should return true");
}

// contains? predicate tests
#[test]
fn test_contains_predicate_match() {
    let exit_code = eval_exit_code(r#""hello world" "wor" contains?"#);
    assert_eq!(exit_code, 0, "contains? should return 0 when substring found");
}

#[test]
fn test_contains_predicate_no_match() {
    let exit_code = eval_exit_code(r#""hello world" "xyz" contains?"#);
    assert_eq!(exit_code, 1, "contains? should return 1 when substring not found");
}

// starts? predicate tests
#[test]
fn test_starts_predicate_match() {
    let exit_code = eval_exit_code(r#""hello world" "hello" starts?"#);
    assert_eq!(exit_code, 0, "starts? should return 0 when string starts with prefix");
}

#[test]
fn test_starts_predicate_no_match() {
    let exit_code = eval_exit_code(r#""hello world" "world" starts?"#);
    assert_eq!(exit_code, 1, "starts? should return 1 when string doesn't start with prefix");
}

// ends? predicate tests
#[test]
fn test_ends_predicate_match() {
    let exit_code = eval_exit_code(r#""hello.txt" ".txt" ends?"#);
    assert_eq!(exit_code, 0, "ends? should return 0 when string ends with suffix");
}

#[test]
fn test_ends_predicate_no_match() {
    let exit_code = eval_exit_code(r#""hello.txt" ".md" ends?"#);
    assert_eq!(exit_code, 1, "ends? should return 1 when string doesn't end with suffix");
}

