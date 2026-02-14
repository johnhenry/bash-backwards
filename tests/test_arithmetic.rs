//! Integration tests for arithmetic operations

#[path = "common/mod.rs"]
mod common;
#[allow(unused_imports)]
use common::{eval, eval_exit_code, Evaluator, lex, parse};

#[test]
fn test_plus() {
    let output = eval("5 3 plus").unwrap();
    assert_eq!(output.trim(), "8");
}

#[test]
fn test_plus_negative() {
    let output = eval("5 -3 plus").unwrap();
    assert_eq!(output.trim(), "2");
}

#[test]
fn test_minus() {
    let output = eval("10 3 minus").unwrap();
    assert_eq!(output.trim(), "7");
}

#[test]
fn test_mul() {
    let output = eval("4 5 mul").unwrap();
    assert_eq!(output.trim(), "20");
}

#[test]
fn test_div() {
    // div now returns float division
    let output = eval("10 2 div").unwrap();
    assert_eq!(output.trim(), "5");
    // Non-integer division
    let output = eval("10 4 div").unwrap();
    assert_eq!(output.trim(), "2.5");
}

#[test]
fn test_mod() {
    let output = eval("10 3 mod").unwrap();
    assert_eq!(output.trim(), "1");
}

#[test]
fn test_arithmetic_chain() {
    // (5 + 3) * 2 = 16
    let output = eval("5 3 plus 2 mul").unwrap();
    assert_eq!(output.trim(), "16");
}

#[test]
fn test_pow_integers() {
    let output = eval(r#"2 3 pow"#).unwrap();
    assert_eq!(output.trim(), "8");
}

#[test]
fn test_pow_float_exponent() {
    let output = eval(r#"4 0.5 pow"#).unwrap();
    assert_eq!(output.trim(), "2"); // sqrt(4) = 2
}

#[test]
fn test_pow_negative_exponent() {
    let output = eval(r#"2 -1 pow"#).unwrap();
    assert_eq!(output.trim(), "0.5");
}

#[test]
fn test_sqrt_perfect_square() {
    let output = eval(r#"16 sqrt"#).unwrap();
    assert_eq!(output.trim(), "4");
}

#[test]
fn test_sqrt_non_perfect() {
    let output = eval(r#"2 sqrt"#).unwrap();
    let val: f64 = output.trim().parse().unwrap();
    assert!((val - 1.4142135).abs() < 0.0001);
}

#[test]
fn test_sqrt_zero() {
    let output = eval(r#"0 sqrt"#).unwrap();
    assert_eq!(output.trim(), "0");
}

#[test]
fn test_sort_nums_ascending() {
    let output = eval(r#"'[3,1,4,1,5,9,2,6]' into-json sort-nums to-json"#).unwrap();
    assert_eq!(output.trim(), "[1.0,1.0,2.0,3.0,4.0,5.0,6.0,9.0]");
}

#[test]
fn test_sort_nums_with_floats() {
    let output = eval(r#"'[3.14,2.71,1.41]' into-json sort-nums to-json"#).unwrap();
    assert_eq!(output.trim(), "[1.41,2.71,3.14]");
}

#[test]
fn test_sort_nums_negative() {
    let output = eval(r#"'[-5,3,-2,0,1]' into-json sort-nums to-json"#).unwrap();
    assert_eq!(output.trim(), "[-5.0,-2.0,0.0,1.0,3.0]");
}

#[test]
fn test_sort_nums_empty() {
    let output = eval(r#"'[]' into-json sort-nums to-json"#).unwrap();
    assert_eq!(output.trim(), "[]");
}

#[test]
fn test_sort_nums_single() {
    let output = eval(r#"'[42]' into-json sort-nums to-json"#).unwrap();
    assert_eq!(output.trim(), "[42.0]");
}

