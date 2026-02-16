#[path = "common/mod.rs"]
mod common;
use common::eval;

#[test]
fn test_plus_symbol() { assert_eq!(eval("5 3 +").unwrap().trim(), "8"); }
#[test]
fn test_minus_symbol() { assert_eq!(eval("10 3 -").unwrap().trim(), "7"); }
#[test]
fn test_multiply_symbol() { assert_eq!(eval("4 5 *").unwrap().trim(), "20"); }
#[test]
fn test_divide_symbol() { assert_eq!(eval("20 4 /").unwrap().trim(), "5"); }
#[test]
fn test_modulo_symbol() { assert_eq!(eval("17 5 %").unwrap().trim(), "2"); }
#[test]
fn test_power_symbol() { assert_eq!(eval("2 8 **").unwrap().trim(), "256"); }
#[test]
fn test_increment() { assert_eq!(eval("5 ++").unwrap().trim(), "6"); }
#[test]
fn test_decrement() { assert_eq!(eval("5 --").unwrap().trim(), "4"); }
