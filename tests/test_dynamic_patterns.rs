//! Integration tests for dynamic operator patterns

#[path = "common/mod.rs"]
mod common;
#[allow(unused_imports)]
use common::{eval, eval_exit_code, Evaluator, lex, parse};

#[test]
fn test_dynamic_plus() {
    // 5 3+ should be equivalent to 5 3 plus
    let output = eval("5 3+").unwrap();
    assert_eq!(output.trim(), "8");
}

#[test]
fn test_dynamic_minus() {
    let output = eval("10 3-").unwrap();
    assert_eq!(output.trim(), "7");
}

#[test]
fn test_dynamic_mul() {
    let output = eval("4 5*").unwrap();
    assert_eq!(output.trim(), "20");
}

#[test]
fn test_dynamic_div() {
    let output = eval("10 2/").unwrap();
    assert_eq!(output.trim(), "5");
}

#[test]
fn test_dynamic_mod() {
    let output = eval("10 3%").unwrap();
    assert_eq!(output.trim(), "1");
}

#[test]
fn test_dynamic_float() {
    let output = eval("10 2.5+").unwrap();
    assert_eq!(output.trim(), "12.5");
}

#[test]
fn test_dynamic_negative() {
    // -3+ should parse as push -3, then plus
    let output = eval("5 -3+").unwrap();
    assert_eq!(output.trim(), "2");
}

#[test]
fn test_dynamic_log_base_10() {
    // 100 10log -> log base 10 of 100 = 2
    let output = eval("100 10log").unwrap();
    assert_eq!(output.trim(), "2");
}

#[test]
fn test_dynamic_log_base_2() {
    // 8 2log -> log base 2 of 8 = 3
    let output = eval("8 2log").unwrap();
    assert_eq!(output.trim(), "3");
}

#[test]
fn test_dynamic_pow() {
    // 2 3pow -> 2^3 = 8
    let output = eval("2 3pow").unwrap();
    assert_eq!(output.trim(), "8");
}

#[test]
fn test_dynamic_pow_float() {
    // 4 0.5pow -> 4^0.5 = 2
    let output = eval("4 0.5pow").unwrap();
    assert_eq!(output.trim(), "2");
}

#[test]
fn test_dynamic_chain() {
    // (5 + 3) * 2 = 16
    let output = eval("5 3+ 2*").unwrap();
    assert_eq!(output.trim(), "16");
}

#[test]
fn test_non_dynamic_word() {
    // "hello" should not trigger dynamic parsing
    let output = eval("hello").unwrap();
    assert_eq!(output.trim(), "hello");
}

#[test]
fn test_log_base_explicit() {
    // Test explicit log-base builtin
    let output = eval("100 10 log-base").unwrap();
    assert_eq!(output.trim(), "2");
}

#[test]
fn test_dynamic_in_block() {
    // Dynamic patterns should work inside blocks too
    let output = eval("5 #[3+] apply").unwrap();
    assert_eq!(output.trim(), "8");
}
