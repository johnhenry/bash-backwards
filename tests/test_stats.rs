//! Integration tests for statistical functions

#[path = "common/mod.rs"]
mod common;
#[allow(unused_imports)]
use common::{eval, eval_exit_code, Evaluator, lex, parse};

#[test]
fn test_product_basic() {
    let output = eval("'[1,2,3,4,5]' into-json product").unwrap();
    assert_eq!(output.trim(), "120");
}

#[test]
fn test_product_with_zero() {
    let output = eval("'[1,2,0,4]' into-json product").unwrap();
    assert_eq!(output.trim(), "0");
}

#[test]
fn test_product_single() {
    let output = eval("'[42]' into-json product").unwrap();
    assert_eq!(output.trim(), "42");
}

#[test]
fn test_product_empty() {
    let output = eval("'[]' into-json product").unwrap();
    assert_eq!(output.trim(), "1");
}

#[test]
fn test_median_odd() {
    let output = eval("'[1,2,3,4,5]' into-json median").unwrap();
    assert_eq!(output.trim(), "3");
}

#[test]
fn test_median_even() {
    let output = eval("'[1,2,3,4]' into-json median").unwrap();
    assert_eq!(output.trim(), "2.5");
}

#[test]
fn test_median_unsorted() {
    let output = eval("'[5,1,3,2,4]' into-json median").unwrap();
    assert_eq!(output.trim(), "3");
}

#[test]
fn test_median_single() {
    let output = eval("'[42]' into-json median").unwrap();
    assert_eq!(output.trim(), "42");
}

#[test]
fn test_mode_basic() {
    let output = eval("'[1,2,2,3,3,3]' into-json mode").unwrap();
    assert_eq!(output.trim(), "3");
}

#[test]
fn test_modes_single_mode() {
    let output = eval("'[1,2,2,3]' into-json modes count").unwrap();
    assert_eq!(output.trim(), "1");
}

#[test]
fn test_modes_multiple() {
    let output = eval("'[1,1,2,2,3]' into-json modes count").unwrap();
    // Both 1 and 2 appear twice
    assert_eq!(output.trim(), "2");
}

#[test]
fn test_variance_basic() {
    // [2, 4, 4, 4, 5, 5, 7, 9] -> mean=5, variance=4
    let output = eval("'[2,4,4,4,5,5,7,9]' into-json variance").unwrap();
    let val: f64 = output.trim().parse().unwrap();
    assert!((val - 4.0).abs() < 0.0001, "Expected variance ~4.0, got {}", val);
}

#[test]
fn test_sample_variance_basic() {
    // Same data, sample variance = 4 * 8/7 = 4.571...
    let output = eval("'[2,4,4,4,5,5,7,9]' into-json sample-variance").unwrap();
    let val: f64 = output.trim().parse().unwrap();
    assert!((val - 4.571428).abs() < 0.001, "Expected sample-variance ~4.571, got {}", val);
}

#[test]
fn test_stdev_basic() {
    // stdev = sqrt(4) = 2
    let output = eval("'[2,4,4,4,5,5,7,9]' into-json stdev").unwrap();
    let val: f64 = output.trim().parse().unwrap();
    assert!((val - 2.0).abs() < 0.0001, "Expected stdev ~2.0, got {}", val);
}

#[test]
fn test_sample_stdev_basic() {
    let output = eval("'[2,4,4,4,5,5,7,9]' into-json sample-stdev").unwrap();
    let val: f64 = output.trim().parse().unwrap();
    assert!((val - 2.13809).abs() < 0.001, "Expected sample-stdev ~2.138, got {}", val);
}

#[test]
fn test_percentile_median() {
    // 50th percentile = median
    let output = eval("'[1,2,3,4,5]' into-json 0.5 percentile").unwrap();
    assert_eq!(output.trim(), "3");
}

#[test]
fn test_percentile_min() {
    let output = eval("'[1,2,3,4,5]' into-json 0.0 percentile").unwrap();
    assert_eq!(output.trim(), "1");
}

#[test]
fn test_percentile_max() {
    let output = eval("'[1,2,3,4,5]' into-json 1.0 percentile").unwrap();
    assert_eq!(output.trim(), "5");
}

#[test]
fn test_percentile_interpolation() {
    // 25th percentile of [1,2,3,4,5]: position 0.25*4 = 1.0, so value = 2
    let output = eval("'[1,2,3,4,5]' into-json 0.25 percentile").unwrap();
    assert_eq!(output.trim(), "2");
}

#[test]
fn test_five_num_basic() {
    let output = eval("'[1,2,3,4,5]' into-json five-num to-json").unwrap();
    // [min=1, Q1=2, median=3, Q3=4, max=5]
    assert!(output.contains("1") && output.contains("3") && output.contains("5"));
}

#[test]
fn test_five_num_count() {
    let output = eval("'[1,2,3,4,5,6,7,8,9,10]' into-json five-num count").unwrap();
    assert_eq!(output.trim(), "5");
}
