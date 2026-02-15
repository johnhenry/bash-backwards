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

    // === Snapshot tests ===

    #[test]
    fn test_snapshot_named() {
        // Test: a b c "name" snapshot -> saves [a,b,c], restores them
        let tokens = lex("alpha bravo charlie \"checkpoint\" snapshot").expect("lex");
        let program = parse(tokens).expect("parse");
        let mut eval = Evaluator::new();
        eval.eval(&program).expect("eval");

        // Stack should be restored (3 values)
        assert_eq!(eval.stack.len(), 3);
        // Snapshot should be saved
        assert!(eval.snapshots.contains_key("checkpoint"));
        assert_eq!(eval.snapshots["checkpoint"].len(), 3);
    }

    #[test]
    fn test_snapshot_anonymous() {
        // Test: a b c snapshot -> saves with auto-name, restores, pushes name
        // Note: in current impl, first arg is treated as name, so this saves [b,c]
        // To truly auto-name, we'd need special handling
        let tokens = lex("snapshot").expect("lex");
        let program = parse(tokens).expect("parse");
        let mut eval = Evaluator::new();
        eval.eval(&program).expect("eval");

        // With empty stack, auto-name should be pushed
        assert_eq!(eval.stack.len(), 1);
        if let Value::Literal(name) = &eval.stack[0] {
            assert!(name.starts_with("snap-"));
        } else {
            panic!("Expected snapshot name on stack");
        }
    }

    #[test]
    fn test_snapshot_restore() {
        // Test: save snapshot, modify stack, restore
        let tokens = lex("alpha bravo charlie \"test\" snapshot").expect("lex");
        let program = parse(tokens).expect("parse");
        let mut eval = Evaluator::new();
        eval.eval(&program).expect("eval snapshot");
        assert_eq!(eval.stack.len(), 3);

        // Modify stack
        let tokens2 = lex("drop drop").expect("lex");
        let program2 = parse(tokens2).expect("parse");
        eval.eval(&program2).expect("eval drop");
        assert_eq!(eval.stack.len(), 1);

        // Restore
        let tokens3 = lex("\"test\" snapshot-restore").expect("lex");
        let program3 = parse(tokens3).expect("parse");
        eval.eval(&program3).expect("eval restore");
        assert_eq!(eval.stack.len(), 3);
    }

    #[test]
    fn test_snapshot_list() {
        let tokens = lex("\"a\" snapshot \"b\" snapshot snapshot-list").expect("lex");
        let program = parse(tokens).expect("parse");
        let mut eval = Evaluator::new();
        eval.eval(&program).expect("eval");

        // Should have a list on top
        if let Some(Value::List(names)) = eval.stack.last() {
            assert_eq!(names.len(), 2);
        } else {
            panic!("Expected list of snapshot names");
        }
    }

    #[test]
    fn test_snapshot_delete() {
        let tokens = lex("\"test\" snapshot \"test\" snapshot-delete snapshot-list").expect("lex");
        let program = parse(tokens).expect("parse");
        let mut eval = Evaluator::new();
        eval.eval(&program).expect("eval");

        // Should have empty list
        if let Some(Value::List(names)) = eval.stack.last() {
            assert!(names.is_empty());
        } else {
            panic!("Expected empty list");
        }
    }

    #[test]
    fn test_snapshot_clear() {
        let tokens = lex("\"a\" snapshot \"b\" snapshot snapshot-clear snapshot-list").expect("lex");
        let program = parse(tokens).expect("parse");
        let mut eval = Evaluator::new();
        eval.eval(&program).expect("eval");

        // Should have empty list
        if let Some(Value::List(names)) = eval.stack.last() {
            assert!(names.is_empty());
        } else {
            panic!("Expected empty list");
        }
    }

    // === Async operation tests ===

    #[test]
    fn test_async_basic() {
        let mut eval = Evaluator::new();
        // Run a simple block asynchronously
        let tokens = lex("[42] async").expect("lex");
        let program = parse(tokens).expect("parse");
        eval.eval(&program).expect("eval");

        // Should have a Future on the stack
        assert_eq!(eval.stack.len(), 1);
        if let Value::Future { id, .. } = &eval.stack[0] {
            assert!(!id.is_empty());
        } else {
            panic!("Expected Future value");
        }
    }

    #[test]
    fn test_async_await() {
        let mut eval = Evaluator::new();
        // Run a block and await the result
        let tokens = lex("[42] async await").expect("lex");
        let program = parse(tokens).expect("parse");
        eval.eval(&program).expect("eval");

        // Should have the result on the stack
        assert_eq!(eval.stack.len(), 1);
        // 42 is pushed as a Literal since it's not recognized as a command
        if let Some(s) = eval.stack[0].as_arg() {
            assert_eq!(s, "42");
        } else {
            panic!("Expected value with string representation, got {:?}", eval.stack[0]);
        }
    }

    #[test]
    fn test_future_status() {
        let mut eval = Evaluator::new();
        // Check status of a completed future
        let tokens = lex("[true] async").expect("lex");
        let program = parse(tokens).expect("parse");
        eval.eval(&program).expect("eval");

        // Wait a bit for the future to complete
        std::thread::sleep(std::time::Duration::from_millis(50));

        let tokens2 = lex("future-status").expect("lex");
        let program2 = parse(tokens2).expect("parse");
        eval.eval(&program2).expect("eval");

        // Should have status and future on stack
        assert!(eval.stack.len() >= 2);
        if let Value::Literal(status) = eval.stack.last().unwrap() {
            // Status could be "pending" or "completed" depending on timing
            assert!(status == "pending" || status == "completed");
        } else {
            panic!("Expected status string");
        }
    }

    #[test]
    fn test_delay() {
        use std::time::Instant;

        let mut eval = Evaluator::new();
        let start = Instant::now();

        // Delay for 50ms
        let tokens = lex("50 delay").expect("lex");
        let program = parse(tokens).expect("parse");
        eval.eval(&program).expect("eval");

        // Should have taken at least 50ms
        assert!(start.elapsed().as_millis() >= 50);
    }

    #[test]
    fn test_parallel_n() {
        let mut eval = Evaluator::new();
        // Run 3 blocks with concurrency 2
        let tokens = lex("[[1] [2] [3]] 2 parallel-n").expect("lex");
        let program = parse(tokens).expect("parse");
        eval.eval(&program).expect("eval");

        // Should have a list of 3 results
        assert_eq!(eval.stack.len(), 1);
        if let Value::List(results) = &eval.stack[0] {
            assert_eq!(results.len(), 3);
        } else {
            panic!("Expected List");
        }
    }

    #[test]
    fn test_race() {
        let mut eval = Evaluator::new();
        // Race two blocks - first one should win
        let tokens = lex("[[1] [2]] race").expect("lex");
        let program = parse(tokens).expect("parse");
        eval.eval(&program).expect("eval");

        // Should have one result (either "1" or "2" as Literal)
        assert_eq!(eval.stack.len(), 1);
        let result = format!("{:?}", eval.stack[0]);
        assert!(result.contains("1") || result.contains("2"), "Expected 1 or 2, got: {}", result);
    }

    #[test]
    fn test_future_cancel() {
        let mut eval = Evaluator::new();
        // Create a future that would take a while
        let tokens = lex("[100 delay] async").expect("lex");
        let program = parse(tokens).expect("parse");
        eval.eval(&program).expect("eval");

        // Cancel it
        let tokens2 = lex("future-cancel").expect("lex");
        let program2 = parse(tokens2).expect("parse");
        eval.eval(&program2).expect("eval");

        // Stack should be empty (future consumed)
        assert!(eval.stack.is_empty());
    }

    #[test]
    fn test_future_map() {
        let mut eval = Evaluator::new();
        // Create a future that returns "hello", then map it with dup (stack op)
        let tokens = lex("[\"hello\"] async [dup] future-map await").expect("lex");
        let program = parse(tokens).expect("parse");
        eval.eval(&program).expect("eval");

        // Should have "hello" on stack (dup pushes a copy, we get the top)
        assert_eq!(eval.stack.len(), 1);
        let result = format!("{:?}", eval.stack[0]);
        assert!(result.contains("hello"), "Expected hello, got: {}", result);
    }

    #[test]
    fn test_retry_delay() {
        let mut eval = Evaluator::new();
        // Simple retry-delay that succeeds on first try
        let tokens = lex("[42] 3 10 retry-delay").expect("lex");
        let program = parse(tokens).expect("parse");
        eval.eval(&program).expect("eval");

        // Should have result on stack
        assert_eq!(eval.stack.len(), 1);
    }

    #[test]
    fn test_future_await_n() {
        let mut eval = Evaluator::new();
        // Create 3 futures and await them all with future-await-n
        let tokens = lex("[\"a\"] async [\"b\"] async [\"c\"] async 3 future-await-n").expect("lex");
        let program = parse(tokens).expect("parse");
        eval.eval(&program).expect("eval");

        // Should have 3 results on stack (not in a list, just on stack)
        assert_eq!(eval.stack.len(), 3);
        // Results should be "a", "b", "c" (in order they were created)
        let results: Vec<String> = eval.stack.iter()
            .filter_map(|v| v.as_arg())
            .collect();
        assert!(results.contains(&"a".to_string()));
        assert!(results.contains(&"b".to_string()));
        assert!(results.contains(&"c".to_string()));
    }

    // === Extended Spread Tests ===

    #[test]
    fn test_spread_list() {
        let mut eval = Evaluator::new();
        // Push a list onto the stack programmatically
        eval.push_value(Value::List(vec![
            Value::Literal("1".to_string()),
            Value::Literal("2".to_string()),
            Value::Literal("3".to_string()),
        ]));
        // Spread it
        let tokens = lex("spread").expect("lex");
        let program = parse(tokens).expect("parse");
        eval.eval(&program).expect("eval");

        // Should have marker + 3 items
        assert_eq!(eval.stack.len(), 4);
        assert!(eval.stack[0].is_marker());
    }

    #[test]
    fn test_spread_record() {
        let mut eval = Evaluator::new();
        // Create a record using the correct syntax and spread it
        let tokens = lex("\"a\" 1 \"b\" 2 record spread").expect("lex");
        let program = parse(tokens).expect("parse");
        eval.eval(&program).expect("eval");

        // Should have marker + 2 values (order undefined but both present)
        assert_eq!(eval.stack.len(), 3);
        assert!(eval.stack[0].is_marker());
    }

    #[test]
    fn test_fields() {
        let mut eval = Evaluator::new();
        // Create a record using the correct syntax
        let tokens = lex("\"name\" \"Alice\" \"age\" 30 \"email\" \"a@b.com\" record").expect("lex");
        let program = parse(tokens).expect("parse");
        eval.eval(&program).expect("eval");
        // Push the list of keys
        eval.push_value(Value::List(vec![
            Value::Literal("name".to_string()),
            Value::Literal("age".to_string()),
        ]));
        // Run fields
        let tokens = lex("fields").expect("lex");
        let program = parse(tokens).expect("parse");
        eval.eval(&program).expect("eval");

        // Should have 2 values on stack (no marker)
        assert_eq!(eval.stack.len(), 2);
        let results: Vec<String> = eval.stack.iter()
            .filter_map(|v| v.as_arg())
            .collect();
        assert!(results.contains(&"Alice".to_string()));
        assert!(results.contains(&"30".to_string()));
    }

    #[test]
    fn test_fields_keys() {
        let mut eval = Evaluator::new();
        // Create a record using the correct syntax and extract key-value pairs
        let tokens = lex("\"a\" 1 \"b\" 2 record fields-keys").expect("lex");
        let program = parse(tokens).expect("parse");
        eval.eval(&program).expect("eval");

        // Should have marker + 4 items (k v k v)
        assert_eq!(eval.stack.len(), 5);
        assert!(eval.stack[0].is_marker());
    }

    #[test]
    fn test_spread_head() {
        let mut eval = Evaluator::new();
        // Push a list onto the stack programmatically
        eval.push_value(Value::List(vec![
            Value::Literal("1".to_string()),
            Value::Literal("2".to_string()),
            Value::Literal("3".to_string()),
            Value::Literal("4".to_string()),
            Value::Literal("5".to_string()),
        ]));
        // Split first element from rest
        let tokens = lex("spread-head").expect("lex");
        let program = parse(tokens).expect("parse");
        eval.eval(&program).expect("eval");

        // Should have head + tail (2 items)
        assert_eq!(eval.stack.len(), 2);
        // First item should be "1", second should be list [2,3,4,5]
        let head = eval.stack[0].as_arg().unwrap();
        assert_eq!(head, "1");
        if let Value::List(tail) = &eval.stack[1] {
            assert_eq!(tail.len(), 4);
        } else {
            panic!("Expected List for tail");
        }
    }

    #[test]
    fn test_spread_tail() {
        let mut eval = Evaluator::new();
        // Push a list onto the stack programmatically
        eval.push_value(Value::List(vec![
            Value::Literal("1".to_string()),
            Value::Literal("2".to_string()),
            Value::Literal("3".to_string()),
            Value::Literal("4".to_string()),
            Value::Literal("5".to_string()),
        ]));
        // Split last element from init
        let tokens = lex("spread-tail").expect("lex");
        let program = parse(tokens).expect("parse");
        eval.eval(&program).expect("eval");

        // Should have init + last (2 items)
        assert_eq!(eval.stack.len(), 2);
        // First item should be list [1,2,3,4], second should be "5"
        if let Value::List(init) = &eval.stack[0] {
            assert_eq!(init.len(), 4);
        } else {
            panic!("Expected List for init");
        }
        let last = eval.stack[1].as_arg().unwrap();
        assert_eq!(last, "5");
    }

    #[test]
    fn test_spread_n() {
        let mut eval = Evaluator::new();
        // Push a list onto the stack programmatically
        eval.push_value(Value::List(vec![
            Value::Literal("1".to_string()),
            Value::Literal("2".to_string()),
            Value::Literal("3".to_string()),
            Value::Literal("4".to_string()),
            Value::Literal("5".to_string()),
        ]));
        // Take first 2 elements, leave rest as list
        let tokens = lex("2 spread-n").expect("lex");
        let program = parse(tokens).expect("parse");
        eval.eval(&program).expect("eval");

        // Should have 2 items + rest list (3 items total)
        assert_eq!(eval.stack.len(), 3);
        // First two should be "1" and "2", third should be list [3,4,5]
        assert_eq!(eval.stack[0].as_arg().unwrap(), "1");
        assert_eq!(eval.stack[1].as_arg().unwrap(), "2");
        if let Value::List(rest) = &eval.stack[2] {
            assert_eq!(rest.len(), 3);
        } else {
            panic!("Expected List for rest");
        }
    }

    #[test]
    fn test_spread_to() {
        let mut eval = Evaluator::new();
        // Push the list of values
        eval.push_value(Value::List(vec![
            Value::Literal("1".to_string()),
            Value::Literal("2".to_string()),
            Value::Literal("3".to_string()),
        ]));
        // Push the list of names
        eval.push_value(Value::List(vec![
            Value::Literal("a".to_string()),
            Value::Literal("b".to_string()),
            Value::Literal("c".to_string()),
        ]));
        // Bind values to locals
        let tokens = lex("spread-to").expect("lex");
        let program = parse(tokens).expect("parse");
        eval.eval(&program).expect("eval");

        // Verify variables were bound by checking local_values
        assert!(eval.local_values.last().unwrap().contains_key("a"));
        assert!(eval.local_values.last().unwrap().contains_key("b"));
        assert!(eval.local_values.last().unwrap().contains_key("c"));

        // Verify values
        assert_eq!(eval.local_values.last().unwrap().get("a").unwrap().as_arg().unwrap(), "1");
        assert_eq!(eval.local_values.last().unwrap().get("b").unwrap().as_arg().unwrap(), "2");
        assert_eq!(eval.local_values.last().unwrap().get("c").unwrap().as_arg().unwrap(), "3");
    }

    // === HTTP Client Tests ===
    // Note: These tests use httpbin.org as a test endpoint

    #[test]
    fn test_fetch_get_basic() {
        let mut eval = Evaluator::new();
        // Simple GET request
        let tokens = lex("\"https://httpbin.org/get\" fetch").expect("lex");
        let program = parse(tokens).expect("parse");
        let result = eval.eval(&program);

        // Should succeed and return response body
        assert!(result.is_ok(), "fetch should succeed");
        assert_eq!(eval.stack.len(), 1);
        // Response should contain the URL we requested
        let response = eval.stack[0].as_arg().unwrap();
        assert!(response.contains("httpbin.org"), "response should contain the URL");
    }

    #[test]
    fn test_fetch_get_json_response() {
        let mut eval = Evaluator::new();
        // GET request that returns JSON
        let tokens = lex("\"https://httpbin.org/json\" fetch").expect("lex");
        let program = parse(tokens).expect("parse");
        let result = eval.eval(&program);

        assert!(result.is_ok(), "fetch should succeed");
        assert_eq!(eval.stack.len(), 1);
        // Response should be parsed as a Map (JSON object)
        assert!(matches!(eval.stack[0], Value::Map(_)), "JSON response should be parsed as Map");
    }

    #[test]
    fn test_fetch_post_with_body() {
        let mut eval = Evaluator::new();
        // POST request with JSON body
        let tokens = lex("\"{\\\"name\\\":\\\"test\\\"}\" \"https://httpbin.org/post\" \"POST\" fetch").expect("lex");
        let program = parse(tokens).expect("parse");
        let result = eval.eval(&program);

        assert!(result.is_ok(), "fetch POST should succeed");
        assert_eq!(eval.stack.len(), 1);
        // httpbin echoes back what you send
        let response = eval.stack[0].as_arg().unwrap_or_default();
        assert!(response.contains("test"), "response should contain echoed data");
    }

    #[test]
    fn test_fetch_status() {
        let mut eval = Evaluator::new();
        // Get status code
        let tokens = lex("\"https://httpbin.org/status/200\" fetch-status").expect("lex");
        let program = parse(tokens).expect("parse");
        let result = eval.eval(&program);

        assert!(result.is_ok(), "fetch-status should succeed");
        assert_eq!(eval.stack.len(), 1);
        // Should return numeric status code
        if let Value::Number(n) = &eval.stack[0] {
            assert_eq!(*n, 200.0);
        } else {
            panic!("Expected Number for status code");
        }
    }

    #[test]
    fn test_fetch_headers() {
        let mut eval = Evaluator::new();
        // Get response headers
        let tokens = lex("\"https://httpbin.org/headers\" fetch-headers").expect("lex");
        let program = parse(tokens).expect("parse");
        let result = eval.eval(&program);

        assert!(result.is_ok(), "fetch-headers should succeed");
        assert_eq!(eval.stack.len(), 1);
        // Should return a Map of headers
        assert!(matches!(eval.stack[0], Value::Map(_)), "headers should be a Map");
    }

    #[test]
    fn test_fetch_put_method() {
        let mut eval = Evaluator::new();
        // Test PUT method
        let tokens = lex("\"{\\\"data\\\":\\\"test\\\"}\" \"https://httpbin.org/put\" \"PUT\" fetch").expect("lex");
        let program = parse(tokens).expect("parse");
        let result = eval.eval(&program);

        assert!(result.is_ok(), "fetch PUT should succeed");
        assert_eq!(eval.stack.len(), 1);
        // httpbin echoes back the request
        let response = eval.stack[0].as_arg().unwrap_or_default();
        assert!(response.contains("test"), "response should contain echoed data");
    }

    #[test]
    fn test_fetch_error_handling() {
        let mut eval = Evaluator::new();
        // Request to non-existent domain
        let tokens = lex("\"https://this-domain-does-not-exist-xyz.invalid/\" fetch").expect("lex");
        let program = parse(tokens).expect("parse");
        let result = eval.eval(&program);

        // Should fail gracefully
        assert!(result.is_err() || eval.last_exit_code != 0,
            "fetch to invalid domain should fail");
    }
}
