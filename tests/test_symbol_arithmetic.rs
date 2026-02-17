#[path = "common/mod.rs"]
mod common;
use common::eval;

#[test]
fn test_plus_symbol() {
    let output = eval("5 3 +").unwrap();
    assert_eq!(output.trim(), "8");
}

#[test]
fn test_minus_symbol() {
    let output = eval("10 3 -").unwrap();
    assert_eq!(output.trim(), "7");
}

#[test]
fn test_multiply_symbol() {
    let output = eval("4 5 *").unwrap();
    assert_eq!(output.trim(), "20");
}

#[test]
fn test_divide_symbol() {
    let output = eval("20 4 /").unwrap();
    assert_eq!(output.trim(), "5");
}

#[test]
fn test_modulo_symbol() {
    let output = eval("17 5 %").unwrap();
    assert_eq!(output.trim(), "2");
}

#[test]
fn test_power_symbol() {
    let output = eval("2 8 **").unwrap();
    assert_eq!(output.trim(), "256");
}

#[test]
fn test_increment() {
    let output = eval("5 ++").unwrap();
    assert_eq!(output.trim(), "6");
}

#[test]
fn test_decrement() {
    let output = eval("5 --").unwrap();
    assert_eq!(output.trim(), "4");
}
