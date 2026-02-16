//! Integration tests for macro-generated builtins

#[path = "common/mod.rs"]
mod common;
#[allow(unused_imports)]
use common::{eval, eval_exit_code, Evaluator, lex, parse};

#[test]
fn test_abs_positive() {
    let output = eval("5 abs").unwrap();
    assert_eq!(output.trim(), "5");
}

#[test]
fn test_abs_negative() {
    let output = eval("-5 abs").unwrap();
    assert_eq!(output.trim(), "5");
}

#[test]
fn test_abs_zero() {
    let output = eval("0 abs").unwrap();
    assert_eq!(output.trim(), "0");
}

#[test]
fn test_negate_positive() {
    let output = eval("5 negate").unwrap();
    assert_eq!(output.trim(), "-5");
}

#[test]
fn test_negate_negative() {
    let output = eval("-5 negate").unwrap();
    assert_eq!(output.trim(), "5");
}

#[test]
fn test_max_of() {
    let output = eval("3 7 max-of").unwrap();
    assert_eq!(output.trim(), "7");
}

#[test]
fn test_min_of() {
    let output = eval("3 7 min-of").unwrap();
    assert_eq!(output.trim(), "3");
}

#[test]
fn test_max_of_equal() {
    let output = eval("5 5 max-of").unwrap();
    assert_eq!(output.trim(), "5");
}

#[test]
fn test_min_of_negative() {
    let output = eval("-10 -3 min-of").unwrap();
    assert_eq!(output.trim(), "-10");
}
