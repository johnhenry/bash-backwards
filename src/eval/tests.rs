#[cfg(test)]
mod tests {
    use crate::eval::*;
    use crate::ast::{Expr, Value};
    use crate::lexer::lex;
    use crate::parser::parse;

    fn eval_str(input: &str) -> Result<EvalResult, EvalError> {
        let tokens = lex(input).expect("lex failed");
        let program = parse(tokens).expect("parse failed");
        let mut eval = Evaluator::new();
        eval.eval(&program)
    }

    #[test]
    fn eval_literal() {
        let result = eval_str("hello world").unwrap();
        assert_eq!(result.output, "hello\nworld");
    }

    #[test]
    fn eval_command() {
        let result = eval_str("hello echo").unwrap();
        assert!(result.output.contains("hello"));
    }

    #[test]
    fn eval_command_substitution() {
        let result = eval_str("pwd ls").unwrap();
        // ls $(pwd) should list current directory
        assert!(result.exit_code == 0);
    }

    #[test]
    fn eval_stack_dup() {
        let result = eval_str("hello dup").unwrap();
        assert_eq!(result.stack.len(), 2);
    }

    #[test]
    fn eval_stack_swap() {
        let result = eval_str("a b swap").unwrap();
        assert_eq!(result.output, "b\na");
    }

    #[test]
    fn eval_path_join() {
        let result = eval_str("/path file.txt path-join").unwrap();
        assert_eq!(result.output, "/path/file.txt");
    }

    #[test]
    fn eval_string_split1() {
        let result = eval_str("\"a.b.c\" \".\" split1").unwrap();
        assert_eq!(result.output, "a\nb.c");
    }

    #[test]
    fn eval_string_rsplit1() {
        let result = eval_str("\"a.b.c\" \".\" rsplit1").unwrap();
        assert_eq!(result.output, "a.b\nc");
    }

    #[test]
    fn eval_define_and_use() {
        // Define a word, then use it
        let tokens = lex("[dup swap] :test").expect("lex");
        let program = parse(tokens).expect("parse");
        let mut eval = Evaluator::new();
        eval.eval(&program).expect("eval define");

        // Now use the defined word
        let tokens2 = lex("a b test").expect("lex");
        let program2 = parse(tokens2).expect("parse");
        let result = eval.eval(&program2).expect("eval use");

        assert_eq!(result.output, "a\nb\nb");
    }

    #[test]
    fn eval_variable_expansion() {
        std::env::set_var("HSAB_TEST_VAR", "test_value");
        let result = eval_str("$HSAB_TEST_VAR echo").unwrap();
        assert!(result.output.contains("test_value"));
        std::env::remove_var("HSAB_TEST_VAR");
    }

    #[test]
    fn eval_builtin_true_false() {
        let result = eval_str("true").unwrap();
        assert_eq!(result.exit_code, 0);

        let result = eval_str("false").unwrap();
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn eval_builtin_test() {
        // Test file existence
        let result = eval_str("Cargo.toml -f test").unwrap();
        assert_eq!(result.exit_code, 0);

        // Test string comparison
        let result = eval_str("a a = test").unwrap();
        assert_eq!(result.exit_code, 0);

        let result = eval_str("a b = test").unwrap();
        assert_eq!(result.exit_code, 1);
    }

    // === Debugger tests ===

    #[test]
    fn test_debugger_mode_toggle() {
        let mut eval = Evaluator::new();
        assert!(!eval.is_debug_mode());

        eval.set_debug_mode(true);
        assert!(eval.is_debug_mode());

        eval.set_debug_mode(false);
        assert!(!eval.is_debug_mode());
    }

    #[test]
    fn test_debugger_step_mode() {
        let mut eval = Evaluator::new();
        assert!(!eval.is_step_mode());

        eval.set_debug_mode(true);
        eval.set_step_mode(true);
        assert!(eval.is_step_mode());

        // Turning off debug mode should also turn off step mode
        eval.set_debug_mode(false);
        assert!(!eval.is_step_mode());
    }

    #[test]
    fn test_debugger_breakpoints() {
        let mut eval = Evaluator::new();

        // Add breakpoints
        eval.add_breakpoint("echo".to_string());
        eval.add_breakpoint("dup".to_string());
        assert_eq!(eval.breakpoints().len(), 2);

        // Remove a breakpoint
        assert!(eval.remove_breakpoint("echo"));
        assert_eq!(eval.breakpoints().len(), 1);

        // Clear all breakpoints
        eval.clear_breakpoints();
        assert!(eval.breakpoints().is_empty());
    }

    #[test]
    fn test_debugger_breakpoint_matching() {
        let mut eval = Evaluator::new();
        eval.add_breakpoint("echo".to_string());

        // Test matching
        let echo_expr = Expr::Literal("echo".to_string());
        let ls_expr = Expr::Literal("ls".to_string());

        assert!(eval.matches_breakpoint(&echo_expr));
        assert!(!eval.matches_breakpoint(&ls_expr));
    }

    #[test]
    fn test_debugger_expr_to_string() {
        let eval = Evaluator::new();

        // Test various expression types
        assert_eq!(eval.expr_to_string(&Expr::Literal("test".to_string())), "test");
        assert_eq!(eval.expr_to_string(&Expr::Dup), "dup");
        assert_eq!(eval.expr_to_string(&Expr::Swap), "swap");
        assert_eq!(eval.expr_to_string(&Expr::Pipe), "|");
        assert_eq!(eval.expr_to_string(&Expr::Apply), "@");
        assert_eq!(eval.expr_to_string(&Expr::If), "if");
    }

    #[test]
    fn test_debugger_format_state() {
        let mut eval = Evaluator::new();
        eval.stack.push(Value::Literal("test".to_string()));
        eval.stack.push(Value::Number(42.0));

        let expr = Expr::Literal("echo".to_string());
        let state = eval.format_debug_state(&expr);

        // Verify the debug state contains expected elements
        assert!(state.contains("echo"));
        assert!(state.contains("Stack (2 items)"));
        assert!(state.contains("\"test\""));
        assert!(state.contains("42"));
    }
}
