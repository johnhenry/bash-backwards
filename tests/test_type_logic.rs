#[path = "common/mod.rs"]
mod common;
use common::{eval, eval_exit_code};

#[test]
fn test_number_pred() {
    assert_eq!(eval_exit_code("42 number?"), 0);
}
#[test]
fn test_string_pred() {
    assert_eq!(eval_exit_code("\"hi\" string?"), 0);
}
#[test]
fn test_typeof_number() {
    assert_eq!(eval("42 typeof").unwrap().trim(), "int");
}
#[test]
fn test_typeof_string() {
    assert_eq!(eval("\"hi\" typeof").unwrap().trim(), "string");
}
#[test]
fn test_not_true() {
    assert_ne!(eval_exit_code("true not"), 0);
}
#[test]
fn test_not_false() {
    assert_eq!(eval_exit_code("false not"), 0);
}
#[test]
fn test_empty_string() {
    assert_eq!(eval_exit_code("\"\" empty?"), 0);
}

// ============================================
// Issue #33: source positions on runtime errors
// ============================================

#[test]
fn test_type_error_reports_position() {
    use hsab::{lex_spanned, parse_with_spans, Evaluator};
    let tokens = lex_spanned(r#""abc" 3 plus"#).unwrap();
    let (program, spans) = parse_with_spans(tokens).unwrap();
    let mut evaluator = Evaluator::new();
    let err = evaluator.eval_with_spans(&program, &spans).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("expected number"), "msg: {}", msg);
    assert!(msg.contains("got string"), "msg: {}", msg);
    assert!(msg.contains("at line 1 col 9"), "msg: {}", msg);
}

#[test]
fn test_stack_underflow_reports_position() {
    use hsab::{lex_spanned, parse_with_spans, Evaluator};
    let tokens = lex_spanned("drop").unwrap();
    let (program, spans) = parse_with_spans(tokens).unwrap();
    let mut evaluator = Evaluator::new();
    let err = evaluator.eval_with_spans(&program, &spans).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("at line 1 col 1"), "msg: {}", msg);
}

#[test]
fn test_error_position_on_second_line() {
    use hsab::{lex_spanned, parse_with_spans, Evaluator};
    let tokens = lex_spanned("1 2 plus\n\"abc\" 3 plus").unwrap();
    let (program, spans) = parse_with_spans(tokens).unwrap();
    let mut evaluator = Evaluator::new();
    let err = evaluator.eval_with_spans(&program, &spans).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("at line 2"), "msg: {}", msg);
}

#[test]
fn test_eval_without_spans_has_no_position() {
    // The plain eval() API is unchanged: no position annotation
    let err = eval(r#""abc" 3 plus"#).unwrap_err();
    assert!(!err.contains("at line"), "msg: {}", err);
}
