//! Integration tests for local operations

#[path = "common/mod.rs"]
mod common;
#[allow(unused_imports)]
use common::{eval, eval_exit_code, Evaluator, lex, parse};

#[test]
fn test_variable_passthrough() {
    let output = eval("$HOME echo").unwrap();
    // Should contain the home directory path
    assert!(output.contains("/"));
}

#[test]
fn test_empty_input() {
    let tokens = lex("").unwrap();
    assert!(tokens.is_empty());
}

#[test]
fn test_whitespace_only() {
    let tokens = lex("   ").unwrap();
    assert!(tokens.is_empty());
}

#[test]
fn test_nested_blocks() {
    // #[#[inner] outer] should parse correctly
    let tokens = lex("#[#[hello echo] apply] apply").unwrap();
    let program = parse(tokens).unwrap();
    assert!(!program.expressions.is_empty());
}

#[test]
fn test_list_files() {
    let output = eval("ls").unwrap();
    assert!(output.contains("Cargo") || output.contains("src"));
}

#[test]
fn test_list_files_with_flags() {
    let output = eval("-la ls").unwrap();
    // Should have detailed listing
    assert!(output.len() > 10);
}

#[test]
fn test_practical_backup_name() {
    // file.txt .bak -> swap, split on ".", drop ext, swap, suffix
    let output = eval("file.txt .bak swap \".\" split1 drop swap suffix").unwrap();
    assert_eq!(output, "file.bak");
}

#[test]
fn test_practical_join_path() {
    let output = eval("/var/log access.log path-join").unwrap();
    assert_eq!(output, "/var/log/access.log");
}

#[test]
fn test_semicolon_basic_assignment() {
    // ABC=5; $ABC echo should print 5
    let output = eval("ABC=5; $ABC echo").unwrap();
    assert_eq!(output.trim(), "5", "basic assignment should work: {}", output);
}

#[test]
fn test_semicolon_multiple_assignments() {
    // Multiple assignments before semicolon
    // Note: In postfix stack semantics, $A pushes "hello", $B pushes "world"
    // echo pops in LIFO order: world then hello -> "world hello"
    let output = eval("A=hello B=world; $A $B echo").unwrap();
    assert_eq!(output.trim(), "world hello",
            "multiple assignments with LIFO order: {}", output);
}

#[test]
fn test_semicolon_shadowing() {
    // Variable should be restored after semicolon scope
    // First set a value, then shadow it, then check it's restored
    std::env::set_var("HSAB_TEST_VAR", "original");
    let output = eval("HSAB_TEST_VAR=shadowed; $HSAB_TEST_VAR echo").unwrap();
    // Output should be exactly "shadowed", not "HSAB_TEST_VAR=shadowed"
    assert_eq!(output.trim(), "shadowed", "shadowed value should be used: {}", output);
    // After the scoped expression, original should be restored
    assert_eq!(std::env::var("HSAB_TEST_VAR").unwrap(), "original",
               "original value should be restored after scope");
    std::env::remove_var("HSAB_TEST_VAR");
}

#[test]
fn test_semicolon_unset_after_scope() {
    // Variable that didn't exist should be unset after scope
    std::env::remove_var("HSAB_NEW_VAR");
    let output = eval("HSAB_NEW_VAR=temporary; $HSAB_NEW_VAR echo").unwrap();
    // Output should be exactly "temporary"
    assert_eq!(output.trim(), "temporary", "new var should work: {}", output);
    assert!(std::env::var("HSAB_NEW_VAR").is_err(),
            "new var should be unset after scope");
}

#[test]
fn test_without_semicolon_is_literal() {
    // Without semicolon, ABC=5 should be treated as a literal
    let output = eval("ABC=5 echo").unwrap();
    assert_eq!(output.trim(), "ABC=5", "without semicolon should be literal: {}", output);
}

#[test]
fn test_flags_still_work() {
    // Flags should not be affected by assignment parsing
    let output = eval("-la ls").unwrap();
    // Just check it doesn't error - output depends on directory
    assert!(output.len() > 0 || output.is_empty(), "flags should still work");
}

#[test]
fn test_assignment_with_special_chars_in_value() {
    // Values can contain special characters
    let output = eval("PATH=/usr/bin:/bin; $PATH echo").unwrap();
    assert!(output.contains("/usr/bin:/bin"), "special chars in value: {}", output);
}

#[test]
fn test_empty_value_assignment() {
    // Empty value assignment
    let output = eval("EMPTY=; $EMPTY echo").unwrap();
    // Empty value means $EMPTY expands to empty string
    assert!(output.trim().is_empty() || output == "\n", "empty value should work: '{}'", output);
}

#[test]
fn test_undefined_var_empty() {
    let output = eval(r#"$UNDEFINED_VAR_XYZ_123 echo"#).unwrap();
    // Undefined vars expand to empty
    assert!(output.is_empty() || output.trim().is_empty());
}

#[test]
fn test_tilde_expansion() {
    let output = eval("~ echo").unwrap();
    // Tilde should expand to home dir
    assert!(output.contains("/") || output.contains("Users") || output.contains("home"));
}

#[test]
fn test_escape_dollar() {
    let output = eval(r#""\$HOME" echo"#).unwrap();
    // Should print literal $HOME
    assert!(output.contains("$HOME"));
}


// === Recovered tests ===

#[test]
fn test_local_structured_map_get() {
    // Test that local Map can use get to access fields
    let output = eval(r#"
        #[
            '{"name":"bob","score":95}' into-json _DATA local
            $_DATA "score" get
        ] :get_map_field
        get_map_field
    "#).unwrap();
    assert_eq!(output.trim(), "95", "local Map should support get: {}", output);
}

#[test]
fn test_local_structured_table() {
    // Test that local preserves Table values
    let output = eval(r#"
        #[
            marker "name" "alice" record "name" "bob" record table _TBL local
            $_TBL typeof
        ] :table_local
        table_local
    "#).unwrap();
    assert_eq!(output.trim(), "Table", "local should preserve Table structure: {}", output);
}

#[test]
fn test_local_number() {
    // Test local with Number value (should use env vars for primitives)
    let output = eval(r#"
        #[
            42 _NUM local
            $_NUM 8 plus
        ] :num_local
        num_local
    "#).unwrap();
    assert_eq!(output.trim(), "50", "local Number should work: {}", output);
}

#[test]
fn test_local_nested_function_scopes() {
    // Test that nested function calls have independent local scopes
    let output = eval(r#"
        #[
            '[10,20,30]' into-json _INNER_LIST local
            $_INNER_LIST sum
        ] :inner_func

        #[
            '[1,2,3]' into-json _OUTER_LIST local
            inner_func
            $_OUTER_LIST sum
            plus
        ] :outer_func

        outer_func
    "#).unwrap();
    // inner_func returns 60, outer sum is 6, total is 66
    assert_eq!(output.trim(), "66", "nested scopes should be independent: {}", output);
}

#[test]
fn test_local_variable_shadowing() {
    // Test that inner scope shadows outer scope variables
    let output = eval(r#"
        #[
            100 _SHADOW local
            $_SHADOW
        ] :inner_shadow

        #[
            5 _SHADOW local
            inner_shadow
        ] :outer_shadow

        outer_shadow
    "#).unwrap();
    // inner_shadow shadows _SHADOW with 100, should return 100
    assert_eq!(output.trim(), "100", "inner scope should shadow outer: {}", output);
}

#[test]
fn test_local_variable_shadowing_structured() {
    // Test shadowing with structured types (List)
    let output = eval(r#"
        #[
            '[100,200]' into-json _DATA local
            $_DATA sum
        ] :inner_struct

        #[
            '[1,2]' into-json _DATA local
            inner_struct
            $_DATA sum
            plus
        ] :outer_struct

        outer_struct
    "#).unwrap();
    // inner returns 300, outer sum is 3, total is 303
    assert_eq!(output.trim(), "303", "structured shadowing should work: {}", output);
}

#[test]
fn test_local_restoration_after_function_exit() {
    // Test that env vars are restored after function exits
    std::env::set_var("HSAB_RESTORE_TEST", "original_value");
    let output = eval(r#"
        #[
            modified HSAB_RESTORE_TEST local
            $HSAB_RESTORE_TEST echo
        ] :modify_var
        modify_var
    "#).unwrap();
    assert!(output.contains("modified"), "local should use new value inside: {}", output);
    assert_eq!(std::env::var("HSAB_RESTORE_TEST").unwrap(), "original_value",
               "original value should be restored after function exits");
    std::env::remove_var("HSAB_RESTORE_TEST");
}

#[test]
fn test_local_restoration_unset_var() {
    // Test that previously unset vars are removed after function exits
    std::env::remove_var("HSAB_UNSET_TEST");
    let output = eval(r#"
        #[
            newvalue HSAB_UNSET_TEST local
            $HSAB_UNSET_TEST echo
        ] :set_new_var
        set_new_var
    "#).unwrap();
    assert!(output.contains("newvalue"), "local should set new value: {}", output);
    assert!(std::env::var("HSAB_UNSET_TEST").is_err(),
            "previously unset var should be unset after function exits");
}

#[test]
fn test_local_deeply_nested_scopes() {
    // Test 3-level deep nesting
    let output = eval(r#"
        #[
            1000 _LEVEL local
            $_LEVEL
        ] :level3

        #[
            100 _LEVEL local
            level3 $_LEVEL plus
        ] :level2

        #[
            10 _LEVEL local
            level2 $_LEVEL plus
        ] :level1

        level1
    "#).unwrap();
    // level3 returns 1000, level2 returns 1000+100=1100, level1 returns 1100+10=1110
    assert_eq!(output.trim(), "1110", "deeply nested scopes should work: {}", output);
}

#[test]
fn test_local_list_operations_in_nested_scope() {
    // Test that List operations work correctly in nested scopes
    let output = eval(r#"
        #[
            '[5,6,7,8]' into-json _NUMS local
            $_NUMS count $_NUMS sum plus
        ] :inner_list_ops

        #[
            '[1,2,3]' into-json _NUMS local
            inner_list_ops
            $_NUMS count $_NUMS sum plus
            plus
        ] :outer_list_ops

        outer_list_ops
    "#).unwrap();
    // inner: count=4, sum=26, inner_total=30
    // outer: count=3, sum=6, outer_total=9
    // final: 30+9=39
    assert_eq!(output.trim(), "39", "list operations in nested scopes: {}", output);
}

#[test]
fn test_local_map_in_nested_scope() {
    // Test Map operations in nested scopes
    let output = eval(r#"
        #[
            '{"value":100}' into-json _OBJ local
            $_OBJ "value" get
        ] :get_inner_value

        #[
            '{"value":10}' into-json _OBJ local
            get_inner_value
            $_OBJ "value" get
            plus
        ] :get_outer_value

        get_outer_value
    "#).unwrap();
    // inner returns 100, outer returns 10, total is 110
    assert_eq!(output.trim(), "110", "map in nested scopes: {}", output);
}

#[test]
fn test_local_table_count_in_scope() {
    // Test Table operations with local variables
    let output = eval(r#"
        #[
            marker "id" 1 record "id" 2 record "id" 3 record table _TBL local
            $_TBL count
        ] :count_table
        count_table
    "#).unwrap();
    assert_eq!(output.trim(), "3", "table count in local scope: {}", output);
}

#[test]
fn test_local_multiple_vars_same_scope() {
    // Test multiple local variables in the same function scope
    let output = eval(r#"
        #[
            10 _A local
            20 _B local
            30 _C local
            $_A $_B plus $_C plus
        ] :multi_local
        multi_local
    "#).unwrap();
    assert_eq!(output.trim(), "60", "multiple local vars should work: {}", output);
}

#[test]
fn test_local_error_outside_function() {
    // Test that local fails when used outside a function
    let result = eval("42 _VAR local");
    assert!(result.is_err(), "local should fail outside function");
    let err = result.unwrap_err();
    assert!(err.contains("inside a function"), "error should mention function: {}", err);
}

#[test]
fn test_local_scope_isolation_between_calls() {
    // Test that separate function calls have isolated scopes
    let output = eval(r#"
        #[
            '[1,2,3]' into-json _ISOLATED local
            $_ISOLATED sum
        ] :isolated_func

        isolated_func isolated_func plus
    "#).unwrap();
    // Each call returns 6, total is 12
    assert_eq!(output.trim(), "12", "separate calls should have isolated scopes: {}", output);
}

#[test]
fn test_local_with_literal_string() {
    // Test local with a literal string value
    let output = eval(r#"
        #[
            "hello world" _MSG local
            $_MSG echo
        ] :string_local
        string_local
    "#).unwrap();
    assert!(output.contains("hello world"), "local should work with strings: {}", output);
}

#[test]
fn test_local_preserves_list_after_operations() {
    // Test that operations on local List don't affect the stored value
    let output = eval(r#"
        #[
            '[1,2,3,4,5]' into-json _NUMS local
            $_NUMS sum drop
            $_NUMS count drop
            $_NUMS sum
        ] :preserve_list
        preserve_list
    "#).unwrap();
    assert_eq!(output.trim(), "15", "local List should be preserved after operations: {}", output);
}

#[test]
fn test_local_env_var_overwrite() {
    // Test that local correctly overwrites existing env var within scope
    std::env::set_var("HSAB_OVERWRITE_TEST", "outer");
    let output = eval(r#"
        #[
            inner1 HSAB_OVERWRITE_TEST local
            #[
                inner2 HSAB_OVERWRITE_TEST local
                $HSAB_OVERWRITE_TEST echo
            ] :nested_overwrite
            nested_overwrite
            $HSAB_OVERWRITE_TEST echo
        ] :outer_overwrite
        outer_overwrite
    "#).unwrap();
    assert!(output.contains("inner2"), "nested should see inner2: {}", output);
    assert!(output.contains("inner1"), "outer should see inner1: {}", output);
    assert_eq!(std::env::var("HSAB_OVERWRITE_TEST").unwrap(), "outer",
               "original should be restored");
    std::env::remove_var("HSAB_OVERWRITE_TEST");
}
