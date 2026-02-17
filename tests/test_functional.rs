//! Integration tests for functional operations: reduce/fold (catamorphism) and bend (anamorphism)

#[path = "common/mod.rs"]
mod common;
#[allow(unused_imports)]
use common::{eval, eval_exit_code, Evaluator, lex, parse};

// === reduce tests ===

#[test]
fn test_reduce_sum() {
    // [1 2 3 4 5] with init=0, block adds accumulator + element
    let output = eval("'[1,2,3,4,5]' json 0 [plus] reduce").unwrap();
    assert_eq!(output.trim(), "15");
}

#[test]
fn test_reduce_product() {
    let output = eval("'[1,2,3,4,5]' json 1 [mul] reduce").unwrap();
    assert_eq!(output.trim(), "120");
}

#[test]
fn test_reduce_single_element() {
    let output = eval("'[42]' json 0 [plus] reduce").unwrap();
    assert_eq!(output.trim(), "42");
}

#[test]
fn test_reduce_empty_list() {
    // Reducing an empty list should return the initial value
    let output = eval("'[]' json 99 [plus] reduce").unwrap();
    assert_eq!(output.trim(), "99");
}

// === fold tests (alias for reduce) ===

#[test]
fn test_fold_is_reduce_alias() {
    let output = eval("'[1,2,3]' json 0 [plus] fold").unwrap();
    assert_eq!(output.trim(), "6");
}

#[test]
fn test_fold_product() {
    let output = eval("'[2,3,4]' json 1 [mul] fold").unwrap();
    assert_eq!(output.trim(), "24");
}

#[test]
fn test_fold_empty_list() {
    let output = eval("'[]' json 0 [plus] fold").unwrap();
    assert_eq!(output.trim(), "0");
}

// === bend tests (anamorphism / unfold) ===

#[test]
fn test_bend_generate_sequence() {
    let output = eval("1 [dup 5 le?] [dup 1 plus] bend").unwrap();
    assert!(output.contains("1") && output.contains("2") && output.contains("3")
            && output.contains("4") && output.contains("5"),
            "bend should generate sequence 1..5: {}", output);
}

#[test]
fn test_bend_powers_of_two() {
    let output = eval("1 [dup 16 le?] [dup 2 mul] bend").unwrap();
    assert!(output.contains("1") && output.contains("2") && output.contains("4")
            && output.contains("8") && output.contains("16"),
            "bend should generate powers of 2 up to 16: {}", output);
}

#[test]
fn test_bend_immediate_false() {
    let output = eval("10 [dup 5 le?] [dup 1 plus] bend").unwrap();
    assert!(!output.contains("10"),
            "bend with immediately false predicate should not emit seed: {}", output);
}

#[test]
fn test_bend_single_element() {
    let output = eval("1 [dup 1 le?] [dup 1 plus] bend").unwrap();
    assert!(output.contains("1"),
            "bend should generate at least the seed when predicate passes once: {}", output);
}

// === roundtrip: bend then reduce ===

#[test]
fn test_bend_then_reduce_sum() {
    let output = eval("1 [dup 5 le?] [dup 1 plus] bend 0 [plus] reduce").unwrap();
    assert_eq!(output.trim(), "15");
}
