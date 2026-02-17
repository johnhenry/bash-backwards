#[path = "common/mod.rs"]
mod common;
use common::{eval, eval_exit_code};

#[test]
fn test_number_pred() { assert_eq!(eval_exit_code("42 number?"), 0); }
#[test]
fn test_string_pred() { assert_eq!(eval_exit_code("\"hi\" string?"), 0); }
#[test]
fn test_typeof_number() { assert_eq!(eval("42 typeof").unwrap().trim(), "number"); }
#[test]
fn test_typeof_string() { assert_eq!(eval("\"hi\" typeof").unwrap().trim(), "string"); }
#[test]
fn test_not_true() { assert_ne!(eval_exit_code("true not"), 0); }
#[test]
fn test_not_false() { assert_eq!(eval_exit_code("false not"), 0); }
#[test]
fn test_empty_string() { assert_eq!(eval_exit_code("\"\" empty?"), 0); }
