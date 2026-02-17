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
    // Start from 1, keep going while value <= 5, each step: dup current, then increment seed
    // seed=1, predicate=[dup 5 le?], step=[dup 1 plus]
    // Iteration: seed=1, pred true, emit 1, seed=2
    //            seed=2, pred true, emit 2, seed=3
    //            ...
    //            seed=5, pred true, emit 5, seed=6
    //            seed=6, pred false, stop
    // Result: list [1, 2, 3, 4, 5]
    let output = eval("1 [dup 5 le?] [dup 1 plus] bend").unwrap();
    // The output should contain 1 through 5
    assert!(output.contains("1") && output.contains("2") && output.contains("3")
            && output.contains("4") && output.contains("5"),
            "bend should generate sequence 1..5: {}", output);
}

#[test]
fn test_bend_powers_of_two() {
    // Start from 1, while <= 16, step: dup then double
    let output = eval("1 [dup 16 le?] [dup 2 mul] bend").unwrap();
    assert!(output.contains("1") && output.contains("2") && output.contains("4")
            && output.contains("8") && output.contains("16"),
            "bend should generate powers of 2 up to 16: {}", output);
}

#[test]
fn test_bend_immediate_false() {
    // Predicate is false immediately - should produce empty list
    let output = eval("10 [dup 5 le?] [dup 1 plus] bend").unwrap();
    // Should be an empty list or nil
    assert!(!output.contains("10"),
            "bend with immediately false predicate should not emit seed: {}", output);
}

#[test]
fn test_bend_single_element() {
    // Start from 1, only true once (1 <= 1), step produces 2 which fails (2 <= 1)
    let output = eval("1 [dup 1 le?] [dup 1 plus] bend").unwrap();
    assert!(output.contains("1"),
            "bend should generate at least the seed when predicate passes once: {}", output);
}

// === roundtrip: bend then reduce ===

#[test]
fn test_bend_then_reduce_sum() {
    // Generate [1,2,3,4,5] with bend, then sum with reduce
    let output = eval("1 [dup 5 le?] [dup 1 plus] bend 0 [plus] reduce").unwrap();
    assert_eq!(output.trim(), "15");
}
