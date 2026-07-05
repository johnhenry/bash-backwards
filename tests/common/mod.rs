//! Common test utilities for hsab integration tests

pub use hsab::{lex, parse, Evaluator};

/// Helper to evaluate hsab input and return output
#[allow(dead_code)] // not every test target uses every helper
pub fn eval(input: &str) -> Result<String, String> {
    let tokens = lex(input).map_err(|e| e.to_string())?;
    if tokens.is_empty() {
        return Ok(String::new());
    }
    let program = parse(tokens).map_err(|e| e.to_string())?;
    let mut evaluator = Evaluator::new();
    let result = evaluator.eval(&program).map_err(|e| e.to_string())?;
    Ok(result.output)
}

/// Helper to evaluate and get exit code
#[allow(dead_code)]
pub fn eval_exit_code(input: &str) -> i32 {
    let tokens = lex(input).unwrap_or_else(|e| panic!("lex failed for {:?}: {}", input, e));
    if tokens.is_empty() {
        return 0;
    }
    let program = parse(tokens).unwrap_or_else(|e| panic!("parse failed for {:?}: {}", input, e));
    let mut evaluator = Evaluator::new();
    let result = evaluator
        .eval(&program)
        .unwrap_or_else(|e| panic!("eval failed for {:?}: {}", input, e));
    result.exit_code
}
