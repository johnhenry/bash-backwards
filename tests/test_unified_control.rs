#[path = "common/mod.rs"]
mod common;
#[allow(unused_imports)]
use common::{eval, eval_exit_code};

// ===== New if order: #[else-block] #[then-block] condition if =====

#[test]
fn test_if_new_order_true() {
    let output = eval(r#"#["no" echo] #["yes" echo] true if"#).unwrap();
    assert_eq!(output.trim(), "yes");
}

#[test]
fn test_if_new_order_false() {
    let output = eval(r#"#["no" echo] #["yes" echo] false if"#).unwrap();
    assert_eq!(output.trim(), "no");
}

#[test]
fn test_if_two_arg_form_true() {
    // Two-arg if: #[then-block] condition if (no else block)
    let output = eval(r#"#["yes" echo] true if"#).unwrap();
    assert_eq!(output.trim(), "yes");
}

#[test]
fn test_if_two_arg_false_no_output() {
    // Two-arg if with false condition: nothing should happen
    let output = eval(r#"#["yes" echo] false if"#).unwrap();
    assert!(output.trim().is_empty(), "false condition with no else should produce no output, got: {}", output);
}

// ===== New times order: #[block] N times =====

#[test]
fn test_times_new_order() {
    let output = eval(r#"#["hi" echo] 3 times"#).unwrap();
    assert_eq!(output.trim(), "hi\nhi\nhi");
}

#[test]
fn test_times_new_order_zero() {
    let output = eval(r#"#["hi" echo] 0 times"#).unwrap();
    assert!(output.trim().is_empty(), "times 0 should produce no output");
}

// ===== Chained elseif/else =====

#[test]
fn test_elseif_chain() {
    // value condition #[block] swap if
    // condition #[block] swap elseif
    // #[block] else
    //
    // 10: not equal to 15 (if skipped), 10%3!=0 (elseif skipped), 10%5==0 (elseif fires)
    // Use drop in blocks that don't need the value, so it doesn't remain on stack
    let program = r#"10 dup 15 eq? #[drop "fizzbuzz" echo] swap if dup 3 mod 0 eq? #[drop "fizz" echo] swap elseif dup 5 mod 0 eq? #[drop "buzz" echo] swap elseif #[echo] else"#;
    let output = eval(program).unwrap();
    assert_eq!(output.trim(), "buzz");
}

#[test]
fn test_else_fallback() {
    let output = eval(r#"#["yes" echo] false if #["no" echo] else"#).unwrap();
    assert_eq!(output.trim(), "no");
}

#[test]
fn test_if_true_no_else_needed() {
    // When if-branch taken, else should be skipped
    let output = eval(r#"#["yes" echo] true if #["no" echo] else"#).unwrap();
    assert_eq!(output.trim(), "yes");
}

#[test]
fn test_elseif_first_branch_taken() {
    // First if is true, so elseif should be skipped
    let program = r#"#["first" echo] true if #["second" echo] true elseif"#;
    let output = eval(program).unwrap();
    assert_eq!(output.trim(), "first");
}

#[test]
fn test_elseif_second_branch_taken() {
    // First if is false, second elseif is true
    let program = r#"#["first" echo] false if #["second" echo] true elseif"#;
    let output = eval(program).unwrap();
    assert_eq!(output.trim(), "second");
}

#[test]
fn test_else_after_all_false() {
    // All conditions false, else runs
    let program = r#"#["first" echo] false if #["second" echo] false elseif #["fallback" echo] else"#;
    let output = eval(program).unwrap();
    assert_eq!(output.trim(), "fallback");
}

#[test]
fn test_elseif_chain_fizzbuzz_15() {
    // Value 15 should match the first condition (15 eq? 15)
    let program = r#"15 dup 15 eq? #[drop "fizzbuzz" echo] swap if dup 5 mod 0 eq? #[drop "buzz" echo] swap elseif #[echo] else"#;
    let output = eval(program).unwrap();
    assert_eq!(output.trim(), "fizzbuzz");
}

#[test]
fn test_elseif_chain_else_with_value() {
    // Value 7 matches none of the conditions, falls through to else
    // 7 dup 15 eq? => false, 7 mod 3 => 1 != 0, 7 mod 5 => 2 != 0
    let program = r#"7 dup 15 eq? #["fizzbuzz" echo] swap if dup 3 mod 0 eq? #["fizz" echo] swap elseif dup 5 mod 0 eq? #["buzz" echo] swap elseif #[echo] else"#;
    let output = eval(program).unwrap();
    assert_eq!(output.trim(), "7");
}
