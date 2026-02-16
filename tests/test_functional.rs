//! Integration tests for functional operations (reduce, fold, bend)

#[path = "common/mod.rs"]
mod common;
use common::eval;

#[test]
fn test_reduce_sum() {
    let output = eval("'[1,2,3,4,5]' json 0 [plus] reduce").unwrap();
    assert_eq!(output.trim(), "15");
}

#[test]
fn test_reduce_product() {
    let output = eval("'[1,2,3,4,5]' json 1 [mul] reduce").unwrap();
    assert_eq!(output.trim(), "120");
}

#[test]
fn test_fold_alias() {
    let output = eval("'[1,2,3]' json 0 [plus] fold").unwrap();
    assert_eq!(output.trim(), "6");
}

#[test]
fn test_bend_sequence() {
    let output = eval("1 [dup 5 le?] [dup 1 plus] bend").unwrap();
    assert!(output.contains("1") && output.contains("5"));
}
