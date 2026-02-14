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

