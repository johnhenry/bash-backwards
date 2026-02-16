//! Builtins generated via the stack_builtin! macro (proof of concept)

use super::{Evaluator, EvalError};
use crate::ast::Value;

impl Evaluator {
    // abs: absolute value of a number
    stack_builtin!(builtin_abs, "abs", (n: Number) -> Number, n.abs());

    // negate: negate a number
    stack_builtin!(builtin_negate, "negate", (n: Number) -> Number, -n);

    // max-of: maximum of two numbers
    stack_builtin!(builtin_max_of, "max-of", (a: Number, b: Number) -> Number, a.max(b));

    // min-of: minimum of two numbers
    stack_builtin!(builtin_min_of, "min-of", (a: Number, b: Number) -> Number, a.min(b));
}
