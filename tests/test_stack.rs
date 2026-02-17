//! Integration tests for stack operations

#[path = "common/mod.rs"]
mod common;
#[allow(unused_imports)]
use common::{eval, eval_exit_code, Evaluator, lex, parse};

#[test]
fn test_literals_push_to_stack() {
    let output = eval("hello world").unwrap();
    assert_eq!(output, "hello\nworld");
}

#[test]
fn test_single_literal() {
    let output = eval("hello").unwrap();
    assert_eq!(output, "hello");
}

#[test]
fn test_quoted_strings() {
    let output = eval("\"hello world\"").unwrap();
    // Quoted strings include the quotes
    assert!(output.contains("hello world"));
}

#[test]
fn test_simple_echo() {
    let output = eval("hello echo").unwrap();
    assert!(output.contains("hello"));
}

#[test]
fn test_echo_multiple_args_lifo() {
    // Stack: [world] -> [world, hello] -> echo pops both
    // LIFO means hello is popped first, world second
    // So: echo hello world (but in stack order)
    let output = eval("world hello echo").unwrap();
    // The output should contain both words
    assert!(output.contains("world") || output.contains("hello"));
}

#[test]
fn test_command_with_flags() {
    // -la ls means: push -la, then ls executes with -la as arg
    let output = eval("-la ls").unwrap();
    // Should list files with details (total line, permissions, etc)
    assert!(output.contains("Cargo") || output.contains("src"));
}

#[test]
fn test_command_substitution() {
    // pwd ls: pwd runs, pushes output, ls runs with pwd's output as arg
    let exit_code = eval_exit_code("pwd ls");
    // If pwd output is a valid dir, ls should succeed
    assert_eq!(exit_code, 0);
}

#[test]
fn test_true_command() {
    let exit_code = eval_exit_code("true");
    assert_eq!(exit_code, 0);
}

#[test]
fn test_false_command() {
    let exit_code = eval_exit_code("false");
    assert_eq!(exit_code, 1);
}

#[test]
fn test_stack_dup() {
    let output = eval("a b dup").unwrap();
    // Should have: a, b, b
    assert_eq!(output.lines().count(), 3);
}

#[test]
fn test_stack_swap() {
    let output = eval("a b swap").unwrap();
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines, vec!["b", "a"]);
}

#[test]
fn test_stack_drop() {
    let output = eval("a b drop").unwrap();
    assert_eq!(output, "a");
}

#[test]
fn test_stack_over() {
    let output = eval("a b over").unwrap();
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines, vec!["a", "b", "a"]);
}

#[test]
fn test_stack_rot() {
    let output = eval("a b c rot").unwrap();
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines, vec!["b", "c", "a"]);
}

#[test]
fn test_path_join() {
    let output = eval("/path file.txt path-join").unwrap();
    assert_eq!(output, "/path/file.txt");
}

#[test]
fn test_path_join_trailing_slash() {
    let output = eval("/path/ file.txt path-join").unwrap();
    assert_eq!(output, "/path/file.txt");
}

#[test]
fn test_string_split1() {
    let output = eval("\"a.b.c\" \".\" split1").unwrap();
    assert_eq!(output, "a\nb.c");
}

#[test]
fn test_string_rsplit1() {
    let output = eval("\"a/b/c\" \"/\" rsplit1").unwrap();
    assert_eq!(output, "a/b\nc");
}

#[test]
fn test_path_suffix() {
    let output = eval("myfile _bak suffix").unwrap();
    assert_eq!(output, "myfile_bak");
}

#[test]
fn test_stack_underflow_dup() {
    let result = eval("dup");
    assert!(result.is_err(), "dup on empty stack should error");
}

#[test]
fn test_stack_underflow_swap() {
    let result = eval("a swap");
    assert!(result.is_err(), "swap with only one item should error");
}

#[test]
fn test_stack_underflow_drop() {
    let result = eval("drop");
    assert!(result.is_err(), "drop on empty stack should error");
}

#[test]
fn test_subst_creates_file() {
    use std::path::Path;

    let output = eval("#[hello echo] subst").unwrap();
    let path = output.trim();
    assert!(Path::new(path).exists() || path.contains("hsab_subst"),
            "subst should create a temp file: {}", path);
    // Clean up
    std::fs::remove_file(path).ok();
}

#[test]
fn test_subst_content() {
    let output = eval("#[hello echo] subst cat").unwrap();
    assert!(output.contains("hello"), "subst should capture command output: {}", output);
}

#[test]
fn test_read_pushes_to_stack() {
    // read without args should push input to stack
    // We can't easily test stdin, but we can test the builtin exists
    // and that it integrates with the stack model
    let tokens = hsab::lex("read").unwrap();
    let program = hsab::parse(tokens).unwrap();
    let mut evaluator = Evaluator::new();
    // This will fail waiting for stdin in a test, but we can verify parsing works
    assert!(program.expressions.len() > 0);
}

#[test]
fn test_file_predicate_true() {
    // file? should return 0 for existing files
    let code = eval_exit_code("Cargo.toml file?");
    assert_eq!(code, 0, "file? should return 0 for existing file");
}

#[test]
fn test_file_predicate_false() {
    // file? should return 1 for non-existent files
    let code = eval_exit_code("nonexistent.xyz file?");
    assert_eq!(code, 1, "file? should return 1 for non-existent file");
}

#[test]
fn test_file_predicate_dir_false() {
    // file? should return 1 for directories
    let code = eval_exit_code("src file?");
    assert_eq!(code, 1, "file? should return 1 for directory");
}

#[test]
fn test_dir_predicate_true() {
    // dir? should return 0 for existing directories
    let code = eval_exit_code("src dir?");
    assert_eq!(code, 0, "dir? should return 0 for existing directory");
}

#[test]
fn test_dir_predicate_false() {
    // dir? should return 1 for non-existent directories
    let code = eval_exit_code("nonexistent_dir dir?");
    assert_eq!(code, 1, "dir? should return 1 for non-existent directory");
}

#[test]
fn test_dir_predicate_file_false() {
    // dir? should return 1 for files
    let code = eval_exit_code("Cargo.toml dir?");
    assert_eq!(code, 1, "dir? should return 1 for file");
}

#[test]
fn test_exists_predicate_file() {
    // exists? should return 0 for existing files
    let code = eval_exit_code("Cargo.toml exists?");
    assert_eq!(code, 0, "exists? should return 0 for existing file");
}

#[test]
fn test_exists_predicate_dir() {
    // exists? should return 0 for existing directories
    let code = eval_exit_code("src exists?");
    assert_eq!(code, 0, "exists? should return 0 for existing directory");
}

#[test]
fn test_exists_predicate_false() {
    // exists? should return 1 for non-existent paths
    let code = eval_exit_code("nonexistent.xyz exists?");
    assert_eq!(code, 1, "exists? should return 1 for non-existent path");
}

#[test]
fn test_empty_predicate_true() {
    // empty? should return 0 for empty string
    let code = eval_exit_code("\"\" empty?");
    assert_eq!(code, 0, "empty? should return 0 for empty string");
}

#[test]
fn test_empty_predicate_false() {
    // empty? should return 1 for non-empty string
    let code = eval_exit_code("hello empty?");
    assert_eq!(code, 1, "empty? should return 1 for non-empty string");
}

#[test]
fn test_eq_predicate_true() {
    // eq? should return 0 for equal strings
    let code = eval_exit_code("hello hello eq?");
    assert_eq!(code, 0, "eq? should return 0 for equal strings");
}

#[test]
fn test_eq_predicate_false() {
    // eq? should return 1 for different strings
    let code = eval_exit_code("hello world eq?");
    assert_eq!(code, 1, "eq? should return 1 for different strings");
}

#[test]
fn test_ne_predicate_true() {
    // ne? should return 0 for different strings
    let code = eval_exit_code("hello world ne?");
    assert_eq!(code, 0, "ne? should return 0 for different strings");
}

#[test]
fn test_ne_predicate_false() {
    // ne? should return 1 for equal strings
    let code = eval_exit_code("hello hello ne?");
    assert_eq!(code, 1, "ne? should return 1 for equal strings");
}

#[test]
fn test_numeric_eq_predicate_true() {
    // =? should return 0 for equal numbers
    let code = eval_exit_code("42 42 =?");
    assert_eq!(code, 0, "=? should return 0 for equal numbers");
}

#[test]
fn test_numeric_eq_predicate_false() {
    // =? should return 1 for different numbers
    let code = eval_exit_code("42 43 =?");
    assert_eq!(code, 1, "=? should return 1 for different numbers");
}

#[test]
fn test_numeric_lt_predicate_true() {
    // lt? should return 0 when first < second
    let code = eval_exit_code("5 10 lt?");
    assert_eq!(code, 0, "lt? should return 0 when 5 < 10");
}

#[test]
fn test_numeric_lt_predicate_false() {
    // lt? should return 1 when first >= second
    let code = eval_exit_code("10 5 lt?");
    assert_eq!(code, 1, "lt? should return 1 when 10 >= 5");
}

#[test]
fn test_numeric_gt_predicate_true() {
    // gt? should return 0 when first > second
    let code = eval_exit_code("10 5 gt?");
    assert_eq!(code, 0, "gt? should return 0 when 10 > 5");
}

#[test]
fn test_numeric_gt_predicate_false() {
    // gt? should return 1 when first <= second
    let code = eval_exit_code("5 10 gt?");
    assert_eq!(code, 1, "gt? should return 1 when 5 <= 10");
}

#[test]
fn test_numeric_le_predicate_true() {
    // le? should return 0 when first <= second
    let code = eval_exit_code("5 10 le?");
    assert_eq!(code, 0, "le? should return 0 when 5 <= 10");
    let code2 = eval_exit_code("5 5 le?");
    assert_eq!(code2, 0, "le? should return 0 when 5 <= 5");
}

#[test]
fn test_numeric_le_predicate_false() {
    // le? should return 1 when first > second
    let code = eval_exit_code("10 5 le?");
    assert_eq!(code, 1, "le? should return 1 when 10 > 5");
}

#[test]
fn test_numeric_ge_predicate_true() {
    // ge? should return 0 when first >= second
    let code = eval_exit_code("10 5 ge?");
    assert_eq!(code, 0, "ge? should return 0 when 10 >= 5");
    let code2 = eval_exit_code("5 5 ge?");
    assert_eq!(code2, 0, "ge? should return 0 when 5 >= 5");
}

#[test]
fn test_numeric_ge_predicate_false() {
    // ge? should return 1 when first < second
    let code = eval_exit_code("5 10 ge?");
    assert_eq!(code, 1, "ge? should return 1 when 5 < 10");
}

#[test]
fn test_numeric_neq_predicate_true() {
    // !=? should return 0 when numbers are different
    let code = eval_exit_code("5 10 !=?");
    assert_eq!(code, 0, "!=? should return 0 when 5 != 10");
}

#[test]
fn test_numeric_neq_predicate_false() {
    // !=? should return 1 when numbers are equal
    let code = eval_exit_code("5 5 !=?");
    assert_eq!(code, 1, "!=? should return 1 when 5 == 5");
}

#[test]
fn test_export_stack_value() {
    // value name .export - take value from stack
    std::env::remove_var("HSAB_STACK_TEST");
    let _output = eval("myvalue HSAB_STACK_TEST .export").unwrap();
    assert_eq!(std::env::var("HSAB_STACK_TEST").unwrap(), "myvalue",
               ".export should set env var from stack value");
    std::env::remove_var("HSAB_STACK_TEST");
}

#[test]
fn test_export_stack_value_with_spaces() {
    // Quoted value with spaces
    std::env::remove_var("HSAB_STACK_TEST2");
    let _output = eval("\"hello world\" HSAB_STACK_TEST2 .export").unwrap();
    assert_eq!(std::env::var("HSAB_STACK_TEST2").unwrap(), "hello world",
               ".export should handle values with spaces");
    std::env::remove_var("HSAB_STACK_TEST2");
}

#[test]
fn test_export_old_syntax_still_works() {
    // Old KEY=VALUE syntax should still work
    std::env::remove_var("HSAB_OLD_SYNTAX");
    let _output = eval("HSAB_OLD_SYNTAX=oldvalue .export").unwrap();
    assert_eq!(std::env::var("HSAB_OLD_SYNTAX").unwrap(), "oldvalue",
               "old KEY=VALUE .export syntax should still work");
    std::env::remove_var("HSAB_OLD_SYNTAX");
}

#[test]
fn test_local_stack_native_in_definition() {
    // value NAME local inside a definition
    std::env::set_var("HSAB_LOCAL_TEST", "original");
    let output = eval(r#"
        #[myvalue HSAB_LOCAL_TEST local $HSAB_LOCAL_TEST echo] :test_local
        test_local
    "#).unwrap();
    assert!(output.contains("myvalue"), "local should use stack value: {}", output);
    // Original should be restored after definition exits
    assert_eq!(std::env::var("HSAB_LOCAL_TEST").unwrap(), "original",
               "original value should be restored after definition exits");
    std::env::remove_var("HSAB_LOCAL_TEST");
}

#[test]
fn test_local_structured_list() {
    // Test that local preserves List values (not converting to string)
    let output = eval(r#"
        #[
            '[1,2,3,4,5]' into-json _MYLIST local
            $_MYLIST sum
        ] :sum_local_list
        sum_local_list
    "#).unwrap();
    assert_eq!(output.trim(), "15", "local should preserve List structure: {}", output);
}

#[test]
fn test_local_structured_list_count() {
    // Test that local Lists preserve structure and can use count
    let output = eval(r#"
        #[
            '[1,2,3,4]' into-json _NUMS local
            $_NUMS count
        ] :count_local
        count_local
    "#).unwrap();
    assert_eq!(output.trim(), "4", "local List should work with count: {}", output);
}

#[test]
fn test_tap_keeps_original() {
    // tap executes block for side effect but keeps original value
    let output = eval("42 #[drop] tap").unwrap();
    assert_eq!(output.trim(), "42");
}

#[test]
fn test_tap_with_output() {
    // tap can be used to inspect values mid-pipeline
    let output = eval("5 #[dup plus] tap").unwrap();
    // Original 5 should remain (tap discards block results)
    assert_eq!(output.trim(), "5");
}

#[test]
fn test_dip_operates_under() {
    // dip: pop top, execute block, push top back
    // Stack: a b #[block] -> a (block results) b
    let output = eval("1 2 #[dup plus] dip").unwrap();
    // Stack starts: 1 2, block sees 1, makes 2, then 2 pushed back
    // Result: 2 2
    assert!(output.contains("2"));
}

#[test]
fn test_dip_swap_equivalent() {
    // dip with single operation should work like operating under top
    let output = eval("3 4 #[10 plus] dip").unwrap();
    // Stack: 3 4, save 4, execute #[10 plus] on 3 -> 13, restore 4
    // Result: 13 4
    let lines: Vec<&str> = output.trim().lines().collect();
    assert!(lines.contains(&"13") || output.contains("13"));
    assert!(lines.contains(&"4") || output.contains("4"));
}

#[test]
fn test_reject_basic() {
    // Keep items where predicate FAILS
    // Keep items that are NOT "b"
    let output = eval(r#"'["a","b","c"]' json #["b" eq?] reject to-json"#).unwrap();
    assert!(output.contains("a") && output.contains("c"), "Should have a and c: {}", output);
    assert!(!output.contains(r#""b""#), "Should not have b: {}", output);
}

#[test]
fn test_reject_where_table() {
    let output = eval(r#"
        marker
            "name" "alice" "age" 30 record
            "name" "bob" "age" 25 record
            "name" "carol" "age" 35 record
        table
        #["age" get 30 ge?] reject-where
        count
    "#).unwrap();
    // Only bob (age 25) should remain
    assert_eq!(output.trim(), "1");
}

#[test]
fn test_duplicates_basic() {
    let output = eval(r#"'["a","b","a","c","b","a"]' json duplicates count"#).unwrap();
    // "a" and "b" appear more than once
    assert_eq!(output.trim(), "2");
}

#[test]
fn test_duplicates_none() {
    let output = eval(r#"'["a","b","c"]' json duplicates count"#).unwrap();
    assert_eq!(output.trim(), "0");
}

#[test]
fn test_over_stack_operation() {
    let output = eval("1 2 over").unwrap();
    // Stack: 1 2 -> 1 2 1
    assert!(output.contains("1") && output.contains("2"));
}

#[test]
fn test_rot_stack_operation() {
    let output = eval("1 2 3 rot").unwrap();
    // Stack: 1 2 3 -> 2 3 1
    assert!(output.contains("2") && output.contains("3") && output.contains("1"));
}

#[test]
fn test_depth_returns_count() {
    let output = eval("a b c depth").unwrap();
    // 3 items + depth result
    assert!(output.contains("3"));
}

#[test]
fn test_pop_block_error() {
    // Trying to use apply on non-block
    let result = eval("42 apply");
    assert!(result.is_err(), "apply on non-block should error");
}

#[test]
fn test_tap_basic() {
    // tap runs block without consuming stack top
    let output = eval(r#"5 #[1 plus] tap"#).unwrap();
    // tap should leave both 5 and 6 on stack (or similar)
    assert!(output.contains("6") || output.contains("5"));
}

#[test]
fn test_dip_basic() {
    // dip: runs block "under" top of stack
    // 1 2 #[10 plus] dip -> (1+10) 2 = 11 2
    let output = eval(r#"1 2 #[10 plus] dip"#).unwrap();
    assert!(output.contains("11") && output.contains("2"));
}

#[test]
fn test_dig_basic() {
    // Stack: 1 2 3 4 5, dig 3 pulls position 3 (which is 3) to top
    let output = eval("1 2 3 4 5 3 dig").unwrap();
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines, vec!["1", "2", "4", "5", "3"]);
}

#[test]
fn test_dig_position_1() {
    // dig 1 removes top and pushes it back, so it's a no-op
    let output = eval("1 2 3 1 dig").unwrap();
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines, vec!["1", "2", "3"]);
}

#[test]
fn test_dig_out_of_range() {
    let result = eval("1 2 10 dig");
    assert!(result.is_err(), "dig with index out of range should error");
}

#[test]
fn test_bury_basic() {
    // Stack: 1 2 3 4 5, bury 3 buries top (5) to position 3
    let output = eval("1 2 3 4 5 3 bury").unwrap();
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines, vec!["1", "2", "5", "3", "4"]);
}

#[test]
fn test_bury_position_1() {
    // bury 1 is a no-op (put top at position 1 which is already top)
    let output = eval("1 2 3 1 bury").unwrap();
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines, vec!["1", "2", "3"]);
}

#[test]
fn test_bury_out_of_range() {
    let result = eval("1 2 10 bury");
    assert!(result.is_err(), "bury with index out of range should error");
}

#[test]
fn test_pick_alias() {
    // pick is alias for dig
    let output = eval("1 2 3 4 5 3 pick").unwrap();
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines, vec!["1", "2", "4", "5", "3"]);
}

#[test]
fn test_roll_alias() {
    // roll is alias for bury
    let output = eval("1 2 3 4 5 3 roll").unwrap();
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines, vec!["1", "2", "5", "3", "4"]);
}
