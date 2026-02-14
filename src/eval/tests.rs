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
    fn eval_path_resolve() {
        // Test resolving current directory
        let result = eval_str(". path-resolve").unwrap();
        // Should return an absolute path (starts with /)
        assert!(result.output.starts_with('/'));

        // Test resolving parent directory
        let result = eval_str(".. path-resolve").unwrap();
        assert!(result.output.starts_with('/'));

        // Test resolving with .. components
        let result = eval_str("/usr/local/bin/.. path-resolve").unwrap();
        assert_eq!(result.output, "/usr/local");
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

    // === Limbo reference tests ===

    #[test]
    fn test_limbo_ref_resolves_value() {
        let mut eval = Evaluator::new();
        // Insert a value in limbo
        eval.limbo.insert("0001".to_string(), Value::Literal("hello".to_string()));

        // Parse and eval a limbo reference (note: `&` prefix, ID extracted)
        let tokens = lex("`&0001`").expect("lex");
        let program = parse(tokens).expect("parse");
        eval.eval(&program).expect("eval");

        // Value should be pushed to stack
        assert_eq!(eval.stack.len(), 1);
        assert_eq!(eval.stack[0].as_arg().unwrap(), "hello");
        // Limbo should be empty (value consumed)
        assert!(eval.limbo.is_empty());
    }

    #[test]
    fn test_limbo_ref_with_annotations_extracts_id() {
        let mut eval = Evaluator::new();
        // Insert a value with a specific ID
        eval.limbo.insert("a1b2".to_string(), Value::Number(42.0));

        // Parse limbo ref with type/preview annotations - ID should be extracted
        let tokens = lex("`&a1b2:i64:42`").expect("lex");
        let program = parse(tokens).expect("parse");
        eval.eval(&program).expect("eval");

        // Value should be on stack
        assert_eq!(eval.stack.len(), 1);
        if let Value::Number(n) = &eval.stack[0] {
            assert_eq!(*n, 42.0);
        } else {
            panic!("Expected Number value");
        }
    }

    #[test]
    fn test_limbo_ref_unknown_id_pushes_nil() {
        let mut eval = Evaluator::new();
        // No value in limbo for this ID

        // Parse limbo ref with unknown ID
        let tokens = lex("`&unknown_id`").expect("lex");
        let program = parse(tokens).expect("parse");
        eval.eval(&program).expect("eval");

        // Should push Nil (graceful degradation)
        assert_eq!(eval.stack.len(), 1);
        assert!(matches!(eval.stack[0], Value::Nil));
    }

    #[test]
    fn test_limbo_ref_double_use_second_is_nil() {
        let mut eval = Evaluator::new();
        eval.limbo.insert("once".to_string(), Value::Literal("value".to_string()));

        // Use the same ref twice
        let tokens = lex("`&once` `&once`").expect("lex");
        let program = parse(tokens).expect("parse");
        eval.eval(&program).expect("eval");

        // First use should have the value, second should be Nil
        assert_eq!(eval.stack.len(), 2);
        assert_eq!(eval.stack[0].as_arg().unwrap(), "value");
        assert!(matches!(eval.stack[1], Value::Nil));
    }

    #[test]
    fn test_format_limbo_ref_string() {
        let eval = Evaluator::new();
        let ref_str = eval.format_limbo_ref("0001", &Value::Literal("hello".to_string()));
        assert_eq!(ref_str, "`&0001:string:\"hello\"`");
    }

    #[test]
    fn test_format_limbo_ref_long_string() {
        let eval = Evaluator::new();
        let long_str = "This is a very long string that exceeds the preview length";
        let ref_str = eval.format_limbo_ref("0001", &Value::Literal(long_str.to_string()));
        // Should truncate and show length
        assert!(ref_str.contains("`&0001:string["));
        assert!(ref_str.contains("..."));
    }

    #[test]
    fn test_format_limbo_ref_number() {
        let eval = Evaluator::new();
        let ref_str = eval.format_limbo_ref("0002", &Value::Number(42.0));
        assert_eq!(ref_str, "`&0002:i64:42`");
    }

    #[test]
    fn test_format_limbo_ref_bool() {
        let eval = Evaluator::new();
        let ref_str = eval.format_limbo_ref("0003", &Value::Bool(true));
        assert_eq!(ref_str, "`&0003:bool:true`");
    }

    #[test]
    fn test_limbo_count() {
        let mut eval = Evaluator::new();
        assert_eq!(eval.limbo_count(), 0);

        eval.limbo.insert("a".to_string(), Value::Nil);
        eval.limbo.insert("b".to_string(), Value::Nil);
        assert_eq!(eval.limbo_count(), 2);

        eval.clear_limbo();
        assert_eq!(eval.limbo_count(), 0);
    }
}
