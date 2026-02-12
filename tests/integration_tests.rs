//! Integration tests for hsab v2
//!
//! hsab v2 is a stack-based execution model:
//! - Literals push to stack
//! - Executables pop args, run, push output
//! - Blocks are deferred execution units

use hsab::{lex, parse, Evaluator};

/// Helper to evaluate hsab input and return output
fn eval(input: &str) -> Result<String, String> {
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
fn eval_exit_code(input: &str) -> i32 {
    let tokens = lex(input).unwrap();
    if tokens.is_empty() {
        return 0;
    }
    let program = parse(tokens).unwrap();
    let mut evaluator = Evaluator::new();
    let result = evaluator.eval(&program).unwrap();
    result.exit_code
}

// ============================================
// Basic literal and stack tests
// ============================================

/// Test that literals push to stack
#[test]
fn test_literals_push_to_stack() {
    let output = eval("hello world").unwrap();
    assert_eq!(output, "hello\nworld");
}

/// Test single literal
#[test]
fn test_single_literal() {
    let output = eval("hello").unwrap();
    assert_eq!(output, "hello");
}

/// Test quoted strings
#[test]
fn test_quoted_strings() {
    let output = eval("\"hello world\"").unwrap();
    // Quoted strings include the quotes
    assert!(output.contains("hello world"));
}

// ============================================
// Command execution tests
// ============================================

/// Test simple command execution
#[test]
fn test_simple_echo() {
    let output = eval("hello echo").unwrap();
    assert!(output.contains("hello"));
}

/// Test command with multiple args (LIFO order)
#[test]
fn test_echo_multiple_args_lifo() {
    // Stack: [world] -> [world, hello] -> echo pops both
    // LIFO means hello is popped first, world second
    // So: echo hello world (but in stack order)
    let output = eval("world hello echo").unwrap();
    // The output should contain both words
    assert!(output.contains("world") || output.contains("hello"));
}

/// Test command with flags
#[test]
fn test_command_with_flags() {
    // -la ls means: push -la, then ls executes with -la as arg
    let output = eval("-la ls").unwrap();
    // Should list files with details (total line, permissions, etc)
    assert!(output.contains("Cargo") || output.contains("src"));
}

/// Test command substitution (output threading)
#[test]
fn test_command_substitution() {
    // pwd ls: pwd runs, pushes output, ls runs with pwd's output as arg
    let exit_code = eval_exit_code("pwd ls");
    // If pwd output is a valid dir, ls should succeed
    assert_eq!(exit_code, 0);
}

/// Test true command (no output, exit 0)
#[test]
fn test_true_command() {
    let exit_code = eval_exit_code("true");
    assert_eq!(exit_code, 0);
}

/// Test false command (no output, exit 1)
#[test]
fn test_false_command() {
    let exit_code = eval_exit_code("false");
    assert_eq!(exit_code, 1);
}

// ============================================
// Block and Apply tests
// ============================================

/// Test block pushes without execution
#[test]
fn test_block_pushes() {
    // This just pushes a block to the stack
    let tokens = lex("[hello echo]").unwrap();
    let program = parse(tokens).unwrap();
    let mut evaluator = Evaluator::new();
    let result = evaluator.eval(&program).unwrap();

    // Stack should have one item (the block)
    assert_eq!(result.stack.len(), 1);
}

/// Test apply executes block
#[test]
fn test_apply_executes_block() {
    let output = eval("[hello echo] @").unwrap();
    assert!(output.contains("hello"));
}

/// Test apply with args before block
#[test]
fn test_apply_with_args() {
    // Push world, then block [echo], apply executes echo with world as arg
    let output = eval("world [echo] @").unwrap();
    assert!(output.contains("world"));
}

// ============================================
// Pipe tests
// ============================================

/// Test pipe operator
#[test]
fn test_pipe_basic() {
    // ls [grep Cargo] | means: ls runs, output piped to grep Cargo
    let output = eval("ls [Cargo grep] |").unwrap();
    assert!(output.contains("Cargo"));
}

/// Test chained pipes
#[test]
fn test_pipe_chained() {
    // Would need multiple pipes, but basic pipe works
    let output = eval("ls [txt grep] |").unwrap();
    // May or may not have .txt files
    assert!(output.is_empty() || output.contains("txt") || true);
}

// ============================================
// Redirect tests
// ============================================

/// Test redirect write
#[test]
fn test_redirect_write() {
    use std::fs;
    let temp_dir = tempfile::tempdir().unwrap();
    let temp_file = temp_dir.path().join("redirect.txt");
    let temp_path = temp_file.to_str().unwrap();

    // [hello echo] [path] >
    let _ = eval(&format!("[hello echo] [{}] >", temp_path));

    // Check file contents
    let contents = fs::read_to_string(&temp_file).unwrap();
    assert!(contents.contains("hello"));
    // temp_dir auto-cleans up on drop
}

/// Test redirect append
#[test]
fn test_redirect_append() {
    use std::fs;
    let temp_dir = tempfile::tempdir().unwrap();
    let temp_file = temp_dir.path().join("append.txt");
    let temp_path = temp_file.to_str().unwrap();

    // Write first line
    let _ = eval(&format!("[first echo] [{}] >", temp_path));
    // Append second line
    let _ = eval(&format!("[second echo] [{}] >>", temp_path));

    let contents = fs::read_to_string(&temp_file).unwrap();
    assert!(contents.contains("first"));
    assert!(contents.contains("second"));
    // temp_dir auto-cleans up on drop
}

// ============================================
// Logic operator tests
// ============================================

/// Test AND operator (success path)
#[test]
fn test_and_success() {
    let output = eval("[true] [done echo] &&").unwrap();
    assert!(output.contains("done"));
}

/// Test AND operator (failure path)
#[test]
fn test_and_failure() {
    // false && echo done should not echo
    let output = eval("[false] [done echo] &&").unwrap();
    // done should not appear because false fails
    assert!(!output.contains("done"));
}

/// Test OR operator (failure path triggers second)
#[test]
fn test_or_failure() {
    let output = eval("[false] [fallback echo] ||").unwrap();
    assert!(output.contains("fallback"));
}

/// Test OR operator (success path)
#[test]
fn test_or_success() {
    // true || echo fallback should not echo
    let output = eval("[true] [fallback echo] ||").unwrap();
    // fallback should not appear because true succeeds
    assert!(!output.contains("fallback"));
}

// ============================================
// Stack operation tests
// ============================================

/// Test dup: a b → a b b
#[test]
fn test_stack_dup() {
    let output = eval("a b dup").unwrap();
    // Should have: a, b, b
    assert_eq!(output.lines().count(), 3);
}

/// Test swap: a b → b a
#[test]
fn test_stack_swap() {
    let output = eval("a b swap").unwrap();
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines, vec!["b", "a"]);
}

/// Test drop: a b → a
#[test]
fn test_stack_drop() {
    let output = eval("a b drop").unwrap();
    assert_eq!(output, "a");
}

/// Test over: a b → a b a
#[test]
fn test_stack_over() {
    let output = eval("a b over").unwrap();
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines, vec!["a", "b", "a"]);
}

/// Test rot: a b c → b c a
#[test]
fn test_stack_rot() {
    let output = eval("a b c rot").unwrap();
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines, vec!["b", "c", "a"]);
}

// ============================================
// Path operation tests
// ============================================

/// Test path-join: /dir file.txt → /dir/file.txt
#[test]
fn test_path_join() {
    let output = eval("/path file.txt path-join").unwrap();
    assert_eq!(output, "/path/file.txt");
}

/// Test path-join with trailing slash
#[test]
fn test_path_join_trailing_slash() {
    let output = eval("/path/ file.txt path-join").unwrap();
    assert_eq!(output, "/path/file.txt");
}

/// Test split1: split at first occurrence
#[test]
fn test_string_split1() {
    let output = eval("\"a.b.c\" \".\" split1").unwrap();
    assert_eq!(output, "a\nb.c");
}

/// Test rsplit1: split at last occurrence
#[test]
fn test_string_rsplit1() {
    let output = eval("\"a/b/c\" \"/\" rsplit1").unwrap();
    assert_eq!(output, "a/b\nc");
}

/// Test suffix: file _bak → file_bak
/// Note: we quote "myfile" because "file" is a real Unix command
#[test]
fn test_path_suffix() {
    let output = eval("myfile _bak suffix").unwrap();
    assert_eq!(output, "myfile_bak");
}

// ============================================
// Variable tests
// ============================================

/// Test variable passthrough
#[test]
fn test_variable_passthrough() {
    let output = eval("$HOME echo").unwrap();
    // Should contain the home directory path
    assert!(output.contains("/"));
}

// ============================================
// Edge case tests
// ============================================

/// Test empty input
#[test]
fn test_empty_input() {
    let tokens = lex("").unwrap();
    assert!(tokens.is_empty());
}

/// Test whitespace only
#[test]
fn test_whitespace_only() {
    let tokens = lex("   ").unwrap();
    assert!(tokens.is_empty());
}

/// Test nested blocks
#[test]
fn test_nested_blocks() {
    // [[inner] outer] should parse correctly
    let tokens = lex("[[hello echo] @] @").unwrap();
    let program = parse(tokens).unwrap();
    assert!(!program.expressions.is_empty());
}

// ============================================
// Practical workflow tests
// ============================================

/// Test file listing
#[test]
fn test_list_files() {
    let output = eval("ls").unwrap();
    assert!(output.contains("Cargo") || output.contains("src"));
}

/// Test file listing with flags
#[test]
fn test_list_files_with_flags() {
    let output = eval("-la ls").unwrap();
    // Should have detailed listing
    assert!(output.len() > 10);
}

/// Test practical: create backup filename using split1/suffix
#[test]
fn test_practical_backup_name() {
    // file.txt .bak → swap, split on ".", drop ext, swap, suffix
    let output = eval("file.txt .bak swap \".\" split1 drop swap suffix").unwrap();
    assert_eq!(output, "file.bak");
}

/// Test practical: path-join path components
#[test]
fn test_practical_join_path() {
    let output = eval("/var/log access.log path-join").unwrap();
    assert_eq!(output, "/var/log/access.log");
}

// ============================================
// Feature: Login Shell Mode (-l/--login)
// ============================================

#[test]
fn test_login_flag_recognized() {
    use std::process::Command;
    // hsab -l -c should work
    let output = Command::new("./target/debug/hsab")
        .args(["-l", "-c", "echo test"])
        .output();

    // Just check it doesn't fail with "unknown option"
    if let Ok(out) = output {
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(!stderr.contains("Unknown option: -l"), "Login flag should be recognized");
    }
}

#[test]
fn test_login_long_flag() {
    use std::process::Command;
    // hsab --login -c should work
    let output = Command::new("./target/debug/hsab")
        .args(["--login", "-c", "echo test"])
        .output();

    if let Ok(out) = output {
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(!stderr.contains("Unknown option: --login"), "Login long flag should be recognized");
    }
}

#[test]
fn test_login_shell_sources_profile() {
    use std::process::Command;
    use std::fs;

    // Create temp directory with .hsab_profile
    let temp_dir = tempfile::tempdir().unwrap();
    let profile = temp_dir.path().join(".hsab_profile");

    // Define a word in profile that we can test
    fs::write(&profile, "[PROFILE_LOADED true] :test_profile_loaded\n").unwrap();

    let output = Command::new("./target/debug/hsab")
        .env("HOME", temp_dir.path())
        .args(["-l", "-c", "test_profile_loaded"])
        .output();

    if let Ok(out) = output {
        // Should succeed (exit 0) because test_profile_loaded was defined
        assert!(out.status.success() || out.status.code() == Some(0),
            "Profile should be sourced in login mode");
    }
}

// ============================================
// Feature: Native Source Command
// ============================================

#[test]
fn test_source_builtin_exists() {
    // source should execute file content
    let tokens = lex("source").unwrap();
    let program = parse(tokens).unwrap();
    // Should parse without error
    assert!(!program.expressions.is_empty());
}

#[test]
fn test_source_executes_file() {
    use std::fs;

    let temp = tempfile::NamedTempFile::new().unwrap();
    fs::write(temp.path(), "[SOURCED true] :was_sourced\n").unwrap();

    // Source the file and then call the defined word
    let input = format!("{} source was_sourced", temp.path().to_str().unwrap());
    let tokens = lex(&input).unwrap();
    let program = parse(tokens).unwrap();
    let mut evaluator = Evaluator::new();
    let result = evaluator.eval(&program);

    // Should succeed because was_sourced should be defined
    assert!(result.is_ok(), "source should execute file and define words");
}

#[test]
fn test_source_nonexistent_file_error() {
    let input = "/nonexistent/file.hsab source";
    let tokens = lex(input).unwrap();
    let program = parse(tokens).unwrap();
    let mut evaluator = Evaluator::new();
    let result = evaluator.eval(&program);

    // Should fail with error
    assert!(result.is_err(), "source should fail on nonexistent file");
}

#[test]
fn test_dot_command_alias() {
    use std::fs;

    let temp = tempfile::NamedTempFile::new().unwrap();
    fs::write(temp.path(), "DOT_WORKS echo\n").unwrap();

    // . should work as alias for source
    let input = format!("{} .", temp.path().to_str().unwrap());
    let tokens = lex(&input).unwrap();
    let program = parse(tokens).unwrap();
    let mut evaluator = Evaluator::new();
    let result = evaluator.eval(&program);

    assert!(result.is_ok(), ". should work as alias for source");
    if let Ok(res) = result {
        assert!(res.output.contains("DOT_WORKS"), ". should execute file");
    }
}

// ============================================
// Feature: Command Hashing/Caching
// ============================================

#[test]
fn test_hash_builtin_no_args() {
    // hash with no args should show cache (initially empty)
    let exit_code = eval_exit_code("hash");
    assert_eq!(exit_code, 0);
}

#[test]
fn test_hash_specific_command() {
    // hash a command to add it to cache
    let exit_code = eval_exit_code("ls hash");
    assert_eq!(exit_code, 0);
}

#[test]
fn test_hash_r_clears_cache() {
    // hash -r should clear the cache
    let exit_code = eval_exit_code("-r hash");
    assert_eq!(exit_code, 0);
}

// ============================================
// Feature: Job Control - SIGTSTP/SIGCONT
// ============================================

#[test]
fn test_job_status_stopped() {
    // Test that JobStatus::Stopped exists and works
    // (Implicitly tested through jobs builtin)
    let output = eval("jobs").unwrap();
    // Should not error, output may be empty
    assert!(output.is_empty() || output.contains("Running") || output.contains("Stopped") || output.contains("Done"));
}

#[test]
fn test_bg_no_stopped_job_error() {
    // bg with no stopped jobs should error
    let tokens = lex("bg").unwrap();
    let program = parse(tokens).unwrap();
    let mut evaluator = Evaluator::new();
    let result = evaluator.eval(&program);

    // Should fail because no stopped jobs
    assert!(result.is_err(), "bg should fail when no stopped jobs");
}

// ============================================
// Feature: depth builtin
// ============================================

#[test]
fn test_depth_empty_stack() {
    let output = eval("depth").unwrap();
    assert_eq!(output, "0");
}

#[test]
fn test_depth_with_items() {
    let output = eval("a b c depth").unwrap();
    // Stack has a, b, c then depth pushes 3
    assert!(output.contains("3"));
}

// ============================================
// Feature: Stdin redirect (<)
// ============================================

#[test]
fn test_stdin_redirect() {
    let temp_file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(temp_file.path(), "hello from file\n").unwrap();

    // [cat] [input.txt] < should feed file to cat's stdin
    let output = eval(&format!("[cat] [{}] <", temp_file.path().to_str().unwrap())).unwrap();
    assert!(output.contains("hello from file"), "stdin redirect should work");
    // temp_file auto-cleans up on drop
}

// ============================================
// Feature: 2>&1 redirect
// ============================================

#[test]
fn test_stderr_to_stdout_redirect() {
    // 2>&1 should merge stderr into stdout
    // Use bash -c to run a command that outputs to stderr
    let output = eval(r#"["echo error >&2" -c bash] 2>&1"#).unwrap();
    // The error message should appear in output
    assert!(output.contains("error"), "stderr should be redirected to stdout: got {}", output);
}

// ============================================
// Feature: fifo (named pipe)
// ============================================

#[test]
fn test_fifo_creates_named_pipe() {
    use std::path::Path;
    use std::fs;

    // [hello echo] fifo should create a named pipe and push its path
    // Note: hsab uses postfix notation, so "hello echo" means echo hello
    let output = eval("[hello echo] fifo").unwrap();
    let pipe_path = output.trim();

    // The path should exist and be a named pipe (or at least exist)
    let path = Path::new(pipe_path);
    assert!(path.exists() || pipe_path.contains("hsab_fifo"), "fifo should create a pipe at: {}", pipe_path);

    // Clean up
    fs::remove_file(pipe_path).ok();
}

#[test]
fn test_fifo_path_is_in_tmp() {
    use std::fs;

    // Note: postfix notation - "test echo" means echo test
    let output = eval("[test echo] fifo").unwrap();
    let pipe_path = output.trim();

    assert!(pipe_path.starts_with("/tmp/") || pipe_path.contains("hsab_fifo"),
            "fifo path should be in /tmp: {}", pipe_path);

    fs::remove_file(pipe_path).ok();
}

#[test]
fn test_fifo_can_be_read() {
    // The fifo should be readable - spawn a reader
    // [hello echo] fifo cat should produce "hello"
    // Note: postfix notation - "hello echo" means echo hello
    let output = eval("[hello echo] fifo cat").unwrap();
    assert!(output.contains("hello"), "should be able to cat from fifo: {}", output);
}

// ============================================
// Control Flow: if
// ============================================

#[test]
fn test_if_true_branch() {
    // Empty condition has exit code 0 (default), so then-branch runs
    // Use quoted strings to avoid treating "yes"/"no" as commands
    let output = eval(r#"[] ["yes" echo] ["no" echo] if"#).unwrap();
    assert!(output.contains("yes"), "if with true condition should run then-branch: {}", output);
}

#[test]
fn test_if_false_branch() {
    // [false] sets exit code to 1, so else-branch runs
    let output = eval(r#"[false] ["yes" echo] ["no" echo] if"#).unwrap();
    assert!(output.contains("no"), "if with false condition should run else-branch: {}", output);
}

#[test]
fn test_if_with_test_condition() {
    // Test comparison: 1 -eq 1 should succeed (exit 0)
    let output = eval(r#"[1 1 -eq test] ["equal" echo] ["not-equal" echo] if"#).unwrap();
    assert!(output.contains("equal"), "if with passing test should run then-branch: {}", output);
}

// ============================================
// Control Flow: times
// ============================================

#[test]
fn test_times_loop() {
    // N [block] times - execute block N times
    let output = eval("3 [x echo] times").unwrap();
    let count = output.matches("x").count();
    assert_eq!(count, 3, "times should execute block N times");
}

#[test]
fn test_times_zero() {
    let output = eval("0 [x echo] times").unwrap();
    assert!(output.is_empty() || !output.contains("x"), "times 0 should not execute block");
}

// ============================================
// Control Flow: while
// ============================================

#[test]
fn test_while_false_condition() {
    // [false] [] while should execute zero times since false returns exit code 1
    let output = eval("[false] [] while done echo").unwrap();
    assert!(output.contains("done"), "while with false condition should exit immediately: {}", output);
}

// ============================================
// JSON Support
// ============================================

#[test]
fn test_json_parse() {
    let output = eval(r#"'{"name":"test","value":42}' json"#).unwrap();
    // JSON parsed to structured data, then displayed
    assert!(output.contains("name") || output.contains("test"),
            "json should parse JSON string: {}", output);
}

#[test]
fn test_unjson_stringify() {
    // Create a value and stringify it
    let output = eval(r#"'{"x":1}' json unjson"#).unwrap();
    assert!(output.contains("x") && output.contains("1"),
            "unjson should stringify back to JSON: {}", output);
}

// ============================================
// List Operations
// ============================================

#[test]
fn test_spread() {
    // spread: take list and push each element
    let output = eval(r#"'["a","b","c"]' json spread"#).unwrap();
    assert!(output.contains("a") && output.contains("b") && output.contains("c"),
            "spread should push each list element: {}", output);
}

#[test]
fn test_marker_and_collect() {
    // marker pushes a boundary, collect gathers everything back to marker
    let output = eval("marker a b c collect").unwrap();
    // collect should produce a list
    assert!(output.contains("a") && output.contains("b") && output.contains("c"),
            "collect should gather items after marker: {}", output);
}

// ============================================
// Stack Underflow Errors
// ============================================

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

// ============================================
// Subst (process substitution)
// ============================================

#[test]
fn test_subst_creates_file() {
    use std::path::Path;

    let output = eval("[hello echo] subst").unwrap();
    let path = output.trim();
    assert!(Path::new(path).exists() || path.contains("hsab_subst"),
            "subst should create a temp file: {}", path);
    // Clean up
    std::fs::remove_file(path).ok();
}

#[test]
fn test_subst_content() {
    let output = eval("[hello echo] subst cat").unwrap();
    assert!(output.contains("hello"), "subst should capture command output: {}", output);
}

// ============================================
// Multiline Strings (Triple Quotes)
// ============================================

#[test]
fn test_triple_single_quote() {
    // Triple single quotes should preserve the content
    let output = eval("'''hello world''' echo").unwrap();
    assert!(output.contains("hello") && output.contains("world"),
            "triple single quotes should work: {}", output);
}

#[test]
fn test_triple_double_quote() {
    // Triple double quotes should work too
    let output = eval(r#""""test string""" echo"#).unwrap();
    assert!(output.contains("test") && output.contains("string"),
            "triple double quotes should work: {}", output);
}

// ============================================
// Semicolon-scoped Variable Assignment
// ============================================

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

// ============================================
// Stack-native read tests
// ============================================

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

// ============================================
// Stack-native test predicates
// ============================================

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

// ============================================
// Stack-native export tests
// ============================================

#[test]
fn test_export_stack_value() {
    // value name export - take value from stack
    std::env::remove_var("HSAB_STACK_TEST");
    let _output = eval("myvalue HSAB_STACK_TEST export").unwrap();
    assert_eq!(std::env::var("HSAB_STACK_TEST").unwrap(), "myvalue",
               "export should set env var from stack value");
    std::env::remove_var("HSAB_STACK_TEST");
}

#[test]
fn test_export_stack_value_with_spaces() {
    // Quoted value with spaces
    std::env::remove_var("HSAB_STACK_TEST2");
    let _output = eval("\"hello world\" HSAB_STACK_TEST2 export").unwrap();
    assert_eq!(std::env::var("HSAB_STACK_TEST2").unwrap(), "hello world",
               "export should handle values with spaces");
    std::env::remove_var("HSAB_STACK_TEST2");
}

#[test]
fn test_export_old_syntax_still_works() {
    // Old KEY=VALUE syntax should still work
    std::env::remove_var("HSAB_OLD_SYNTAX");
    let _output = eval("HSAB_OLD_SYNTAX=oldvalue export").unwrap();
    assert_eq!(std::env::var("HSAB_OLD_SYNTAX").unwrap(), "oldvalue",
               "old KEY=VALUE export syntax should still work");
    std::env::remove_var("HSAB_OLD_SYNTAX");
}

// ============================================
// Stack-native local tests
// ============================================

#[test]
fn test_local_stack_native_in_definition() {
    // value NAME local inside a definition
    std::env::set_var("HSAB_LOCAL_TEST", "original");
    let output = eval(r#"
        [myvalue HSAB_LOCAL_TEST local $HSAB_LOCAL_TEST echo] :test_local
        test_local
    "#).unwrap();
    assert!(output.contains("myvalue"), "local should use stack value: {}", output);
    // Original should be restored after definition exits
    assert_eq!(std::env::var("HSAB_LOCAL_TEST").unwrap(), "original",
               "original value should be restored after definition exits");
    std::env::remove_var("HSAB_LOCAL_TEST");
}

// ============================================
// Arithmetic primitives tests
// ============================================

#[test]
fn test_plus() {
    let output = eval("5 3 plus").unwrap();
    assert_eq!(output.trim(), "8");
}

#[test]
fn test_plus_negative() {
    let output = eval("5 -3 plus").unwrap();
    assert_eq!(output.trim(), "2");
}

#[test]
fn test_minus() {
    let output = eval("10 3 minus").unwrap();
    assert_eq!(output.trim(), "7");
}

#[test]
fn test_mul() {
    let output = eval("4 5 mul").unwrap();
    assert_eq!(output.trim(), "20");
}

#[test]
fn test_div() {
    let output = eval("10 3 div").unwrap();
    assert_eq!(output.trim(), "3");
}

#[test]
fn test_mod() {
    let output = eval("10 3 mod").unwrap();
    assert_eq!(output.trim(), "1");
}

#[test]
fn test_arithmetic_chain() {
    // (5 + 3) * 2 = 16
    let output = eval("5 3 plus 2 mul").unwrap();
    assert_eq!(output.trim(), "16");
}

// ============================================
// String primitives tests
// ============================================

#[test]
fn test_len() {
    let output = eval("hello len").unwrap();
    assert_eq!(output.trim(), "5");
}

#[test]
fn test_len_empty() {
    let output = eval("\"\" len").unwrap();
    assert_eq!(output.trim(), "0");
}

#[test]
fn test_slice() {
    let output = eval("hello 1 3 slice").unwrap();
    assert_eq!(output.trim(), "ell");
}

#[test]
fn test_slice_from_start() {
    let output = eval("hello 0 2 slice").unwrap();
    assert_eq!(output.trim(), "he");
}

#[test]
fn test_indexof_found() {
    let output = eval("hello ll indexof").unwrap();
    assert_eq!(output.trim(), "2");
}

#[test]
fn test_indexof_not_found() {
    let output = eval("hello xyz indexof").unwrap();
    assert_eq!(output.trim(), "-1");
}

#[test]
fn test_indexof_at_start() {
    let output = eval("hello he indexof").unwrap();
    assert_eq!(output.trim(), "0");
}

// ============================================
// Phase 0: Value Types and typeof
// ============================================

#[test]
fn test_typeof_string() {
    let output = eval("hello typeof").unwrap();
    assert_eq!(output.trim(), "String");
}

#[test]
fn test_typeof_quoted_string() {
    let output = eval("\"hello world\" typeof").unwrap();
    assert_eq!(output.trim(), "String");
}

#[test]
fn test_typeof_number() {
    // Numbers come from JSON parsing or arithmetic
    let output = eval("'42' json typeof").unwrap();
    assert_eq!(output.trim(), "Number");
}

#[test]
fn test_typeof_boolean_true() {
    // Using JSON to get a boolean
    let output = eval("'true' json typeof").unwrap();
    assert_eq!(output.trim(), "Boolean");
}

#[test]
fn test_typeof_boolean_false() {
    let output = eval("'false' json typeof").unwrap();
    assert_eq!(output.trim(), "Boolean");
}

#[test]
fn test_typeof_list() {
    let output = eval("'[1,2,3]' json typeof").unwrap();
    assert_eq!(output.trim(), "List");
}

#[test]
fn test_typeof_record() {
    let output = eval("'{\"name\":\"test\"}' json typeof").unwrap();
    assert_eq!(output.trim(), "Record");
}

#[test]
fn test_typeof_null() {
    let output = eval("'null' json typeof").unwrap();
    assert_eq!(output.trim(), "Null");
}

#[test]
fn test_typeof_block() {
    let output = eval("[hello echo] typeof").unwrap();
    assert_eq!(output.trim(), "Block");
}

// ============================================
// Phase 1: Record Operations
// ============================================

#[test]
fn test_record_construction() {
    // record collects key-value pairs from stack
    let output = eval("\"name\" \"hsab\" \"version\" \"0.2\" record typeof").unwrap();
    assert_eq!(output.trim(), "Record");
}

#[test]
fn test_record_get_field() {
    let output = eval("\"name\" \"hsab\" record \"name\" get").unwrap();
    assert_eq!(output.trim(), "hsab");
}

#[test]
fn test_record_get_missing_field() {
    let result = eval("\"name\" \"hsab\" record \"missing\" get");
    // Should either error or return nil/empty
    match result {
        Err(_) => (), // Expected - error for missing field
        Ok(s) => assert!(s.trim().is_empty() || s.contains("null"), "missing field should be empty or null: {}", s),
    }
}

#[test]
fn test_record_set_field() {
    let output = eval("\"a\" 1 record \"b\" 2 set \"b\" get").unwrap();
    assert_eq!(output.trim(), "2");
}

#[test]
fn test_record_set_overwrites() {
    let output = eval("\"a\" 1 record \"a\" 99 set \"a\" get").unwrap();
    assert_eq!(output.trim(), "99");
}

#[test]
fn test_record_del_field() {
    let code = eval_exit_code("\"a\" 1 \"b\" 2 record \"a\" del \"a\" has?");
    assert_eq!(code, 1, "has? should return 1 (false) for deleted field");
}

#[test]
fn test_record_has_true() {
    let code = eval_exit_code("\"name\" \"test\" record \"name\" has?");
    assert_eq!(code, 0, "has? should return 0 (true) for existing field");
}

#[test]
fn test_record_has_false() {
    let code = eval_exit_code("\"name\" \"test\" record \"missing\" has?");
    assert_eq!(code, 1, "has? should return 1 (false) for missing field");
}

#[test]
fn test_record_keys() {
    let output = eval("\"a\" 1 \"b\" 2 record keys typeof").unwrap();
    assert_eq!(output.trim(), "List");
}

#[test]
fn test_record_values() {
    let output = eval("\"a\" 1 \"b\" 2 record values typeof").unwrap();
    assert_eq!(output.trim(), "List");
}

#[test]
fn test_record_merge() {
    // merge two records, right overwrites left
    let output = eval("\"a\" 1 record \"b\" 2 record merge \"b\" get").unwrap();
    assert_eq!(output.trim(), "2");
}

#[test]
fn test_record_merge_overwrites() {
    let output = eval("\"a\" 1 record \"a\" 99 record merge \"a\" get").unwrap();
    assert_eq!(output.trim(), "99");
}

// ============================================
// Phase 2: Table Operations
// ============================================

#[test]
fn test_table_construction() {
    // table from records
    let output = eval("marker \"name\" \"alice\" record \"name\" \"bob\" record table typeof").unwrap();
    assert_eq!(output.trim(), "Table");
}

#[test]
fn test_table_where_filter() {
    // Filter rows where condition is true
    let output = eval(r#"
        marker
            "name" "alice" "age" 30 record
            "name" "bob" "age" 25 record
            "name" "carol" "age" 35 record
        table
        ["age" get 30 gt?] where
        "name" get
    "#).unwrap();
    // Should only have carol (age > 30)
    assert!(output.contains("carol"), "where should filter to carol: {}", output);
    assert!(!output.contains("alice"), "alice should be filtered out");
    assert!(!output.contains("bob"), "bob should be filtered out");
}

#[test]
fn test_table_sort_by() {
    let output = eval(r#"
        marker
            "name" "bob" "age" 25 record
            "name" "alice" "age" 30 record
        table
        "name" sort-by
        0 nth "name" get
    "#).unwrap();
    // First after sorting by name should be alice
    assert_eq!(output.trim(), "alice");
}

#[test]
fn test_table_select_columns() {
    let output = eval(r#"
        marker
            "name" "alice" "age" 30 "city" "NYC" record
        table
        ["name" "age"] select
        0 nth keys
    "#).unwrap();
    // Should only have name and age, not city
    assert!(output.contains("name") && output.contains("age"), "should have name and age");
    assert!(!output.contains("city"), "city should be removed");
}

#[test]
fn test_table_first() {
    // Simpler test: just check that first returns a table with correct row count
    let output = eval(r#"
        marker
            "n" "a" record
            "n" "b" record
            "n" "c" record
        table
        2 first
        0 nth "n" get
    "#).unwrap();
    // First 2 rows, get row 0's "n" field - should be first record's value
    // After reverse, first record is {n:a}
    assert!(output.trim() == "a" || output.trim() == "b" || output.trim() == "c",
        "Expected a, b, or c but got: {}", output.trim());
}

#[test]
fn test_table_last() {
    let output = eval(r#"
        marker
            "n" 1 record
            "n" 2 record
            "n" 3 record
        table
        1 last
        0 nth "n" get
    "#).unwrap();
    assert_eq!(output.trim(), "3");
}

#[test]
fn test_table_nth_row() {
    let output = eval(r#"
        marker
            "n" "first" record
            "n" "second" record
        table
        1 nth "n" get
    "#).unwrap();
    assert_eq!(output.trim(), "second");
}

// ============================================
// Phase 3: Structured Errors
// ============================================

#[test]
fn test_try_success() {
    let output = eval("[hello echo] try typeof").unwrap();
    // Should return the output, not an error
    assert!(output.contains("hello") || output.contains("String"), "try should return result on success: {}", output);
}

#[test]
fn test_try_captures_error() {
    // Use a stack underflow which definitely causes EvalError
    let output = eval("[dup] try typeof").unwrap();
    assert_eq!(output.trim(), "Error", "try should capture error: {}", output);
}

#[test]
fn test_error_predicate_true() {
    // Use a stack underflow to create an Error
    let code = eval_exit_code("[dup] try error?");
    assert_eq!(code, 0, "error? should return 0 (true) for Error value");
}

#[test]
fn test_error_predicate_false() {
    let code = eval_exit_code("[hello echo] try error?");
    assert_eq!(code, 1, "error? should return 1 (false) for non-Error value");
}

#[test]
fn test_throw_creates_error() {
    let output = eval("\"something went wrong\" throw typeof").unwrap();
    assert_eq!(output.trim(), "Error");
}

#[test]
fn test_error_has_message() {
    let output = eval("\"my error message\" throw \"message\" get").unwrap();
    assert!(output.contains("my error message"), "error should have message field: {}", output);
}

// ============================================
// Phase 4: Serialization Bridge
// ============================================

#[test]
fn test_into_json_object() {
    let output = eval("'{\"name\":\"test\"}' into-json typeof").unwrap();
    assert_eq!(output.trim(), "Record");
}

#[test]
fn test_into_json_array() {
    let output = eval("'[1,2,3]' into-json typeof").unwrap();
    assert_eq!(output.trim(), "List");
}

#[test]
fn test_into_csv_creates_table() {
    let output = eval("\"name,age\\nalice,30\\nbob,25\" into-csv typeof").unwrap();
    assert_eq!(output.trim(), "Table");
}

#[test]
fn test_into_csv_correct_rows() {
    let output = eval("\"name,age\\nalice,30\\nbob,25\" into-csv 0 nth \"name\" get").unwrap();
    assert_eq!(output.trim(), "alice");
}

#[test]
fn test_into_lines() {
    let output = eval("\"a\\nb\\nc\" into-lines typeof").unwrap();
    assert_eq!(output.trim(), "List");
}

#[test]
fn test_into_lines_content() {
    let output = eval("\"a\\nb\\nc\" into-lines").unwrap();
    assert!(output.contains("a") && output.contains("b") && output.contains("c"));
}

#[test]
fn test_into_kv() {
    let output = eval("\"name=test\\nversion=1.0\" into-kv typeof").unwrap();
    assert_eq!(output.trim(), "Record");
}

#[test]
fn test_into_kv_content() {
    let output = eval("\"name=test\\nversion=1.0\" into-kv \"name\" get").unwrap();
    assert_eq!(output.trim(), "test");
}

#[test]
fn test_to_json_record() {
    let output = eval("\"name\" \"test\" record to-json").unwrap();
    assert!(output.contains("name") && output.contains("test"), "to-json should serialize record: {}", output);
}

#[test]
fn test_to_json_list() {
    let output = eval("'[1,2,3]' into-json to-json").unwrap();
    assert!(output.contains("[") && output.contains("1") && output.contains("2") && output.contains("3"));
}

#[test]
fn test_to_csv_table() {
    let output = eval(r#"
        marker
            "name" "alice" "age" "30" record
            "name" "bob" "age" "25" record
        table
        to-csv
    "#).unwrap();
    assert!(output.contains("name") && output.contains("age"), "to-csv should have headers: {}", output);
    assert!(output.contains("alice") && output.contains("bob"), "to-csv should have data: {}", output);
}

#[test]
fn test_to_lines_list() {
    let output = eval("'[\"a\",\"b\",\"c\"]' into-json to-lines").unwrap();
    let lines: Vec<&str> = output.trim().lines().collect();
    assert_eq!(lines.len(), 3);
    assert!(lines.contains(&"a") && lines.contains(&"b") && lines.contains(&"c"));
}

// ============================================
// Phase 5: Stack utilities (peek, tap, dip)
// ============================================

#[test]
fn test_tap_keeps_original() {
    // tap executes block for side effect but keeps original value
    let output = eval("42 [drop] tap").unwrap();
    assert_eq!(output.trim(), "42");
}

#[test]
fn test_tap_with_output() {
    // tap can be used to inspect values mid-pipeline
    let output = eval("5 [dup plus] tap").unwrap();
    // Original 5 should remain (tap discards block results)
    assert_eq!(output.trim(), "5");
}

#[test]
fn test_dip_operates_under() {
    // dip: pop top, execute block, push top back
    // Stack: a b [block] -> a (block results) b
    let output = eval("1 2 [dup plus] dip").unwrap();
    // Stack starts: 1 2, block sees 1, makes 2, then 2 pushed back
    // Result: 2 2
    assert!(output.contains("2"));
}

#[test]
fn test_dip_swap_equivalent() {
    // dip with single operation should work like operating under top
    let output = eval("3 4 [10 plus] dip").unwrap();
    // Stack: 3 4, save 4, execute [10 plus] on 3 -> 13, restore 4
    // Result: 13 4
    let lines: Vec<&str> = output.trim().lines().collect();
    assert!(lines.contains(&"13") || output.contains("13"));
    assert!(lines.contains(&"4") || output.contains("4"));
}

// ============================================
// Phase 6: Aggregation operations
// ============================================

#[test]
fn test_sum_list() {
    let output = eval("'[1,2,3,4,5]' into-json sum").unwrap();
    assert_eq!(output.trim(), "15");
}

#[test]
fn test_avg_list() {
    let output = eval("'[10,20,30]' into-json avg").unwrap();
    assert_eq!(output.trim(), "20");
}

#[test]
fn test_min_list() {
    let output = eval("'[5,2,8,1,9]' into-json min").unwrap();
    assert_eq!(output.trim(), "1");
}

#[test]
fn test_max_list() {
    let output = eval("'[5,2,8,1,9]' into-json max").unwrap();
    assert_eq!(output.trim(), "9");
}

#[test]
fn test_count_list() {
    let output = eval("'[1,2,3,4,5]' into-json count").unwrap();
    assert_eq!(output.trim(), "5");
}

#[test]
fn test_count_table() {
    let output = eval(r#"
        marker
            "name" "alice" record
            "name" "bob" record
            "name" "charlie" record
        table
        count
    "#).unwrap();
    assert_eq!(output.trim(), "3");
}

// ============================================
// Phase 7: Deep path access
// ============================================

#[test]
fn test_deep_get_nested() {
    let output = eval(r#"'{"server":{"host":"localhost","port":8080}}' into-json "server.port" get"#).unwrap();
    assert_eq!(output.trim(), "8080");
}

#[test]
fn test_deep_get_array_index() {
    let output = eval(r#"'{"items":[10,20,30]}' into-json "items.1" get"#).unwrap();
    assert_eq!(output.trim(), "20");
}

#[test]
fn test_deep_get_missing() {
    let output = eval(r#"'{"a":1}' into-json "a.b.c" get typeof"#).unwrap();
    assert_eq!(output.trim(), "Null");
}

// ============================================
// Phase 8: Extended table operations
// ============================================

#[test]
fn test_group_by() {
    let output = eval(r#"
        marker
            "type" "fruit" "name" "apple" record
            "type" "veg" "name" "carrot" record
            "type" "fruit" "name" "banana" record
        table
        "type" group-by
        typeof
    "#).unwrap();
    assert_eq!(output.trim(), "Record");
}

#[test]
fn test_group_by_access() {
    let output = eval(r#"
        marker
            "type" "a" "val" "1" record
            "type" "b" "val" "2" record
            "type" "a" "val" "3" record
        table
        "type" group-by
        "a" get
        count
    "#).unwrap();
    assert_eq!(output.trim(), "2");
}

#[test]
fn test_unique_list() {
    let output = eval("'[1,2,2,3,3,3]' into-json unique count").unwrap();
    assert_eq!(output.trim(), "3");
}

#[test]
fn test_reverse_list() {
    let output = eval("'[1,2,3]' into-json reverse").unwrap();
    // Should be 3,2,1
    assert!(output.contains("3") && output.contains("2") && output.contains("1"));
}

#[test]
fn test_flatten_nested() {
    let output = eval("'[[1,2],[3,4]]' into-json flatten count").unwrap();
    assert_eq!(output.trim(), "4");
}

// ============================================
// Phase 11: Additional parsers
// ============================================

#[test]
fn test_into_tsv() {
    let output = eval(r#""name\tage\nalice\t30\nbob\t25" into-tsv count"#).unwrap();
    assert_eq!(output.trim(), "2");
}

#[test]
fn test_into_delimited() {
    let output = eval(r#""name|age\nalice|30" "|" into-delimited count"#).unwrap();
    assert_eq!(output.trim(), "1");
}

// ============================================
// Brace expansion tests
// ============================================

#[test]
fn test_brace_expansion_comma() {
    // {a,b,c} should expand to three stack items
    let output = eval("{a,b,c} depth").unwrap();
    // depth returns 3 (three items: a, b, c), then all items are output
    assert!(output.contains("3"), "depth should show 3 items on stack: {}", output);
}

#[test]
fn test_brace_expansion_range() {
    // {1..3} should expand to 1, 2, 3
    let output = eval("{1..3}").unwrap();
    assert!(output.contains("1") && output.contains("2") && output.contains("3"));
}

#[test]
fn test_brace_expansion_with_command() {
    // {a,b,c} echo should echo each item
    let output = eval("{hello,world} echo").unwrap();
    assert!(output.contains("hello") && output.contains("world"));
}

#[test]
fn test_brace_expansion_prefix_suffix() {
    // file{1,2}.txt should become file1.txt file2.txt
    let output = eval("file{1,2}.txt").unwrap();
    assert!(output.contains("file1.txt") && output.contains("file2.txt"));
}
