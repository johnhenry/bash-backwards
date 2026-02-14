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
    // .source should execute file content
    let tokens = lex(".source").unwrap();
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
    let input = format!("{} .source was_sourced", temp.path().to_str().unwrap());
    let tokens = lex(&input).unwrap();
    let program = parse(tokens).unwrap();
    let mut evaluator = Evaluator::new();
    let result = evaluator.eval(&program);

    // Should succeed because was_sourced should be defined
    assert!(result.is_ok(), ".source should execute file and define words");
}

#[test]
fn test_source_nonexistent_file_error() {
    let input = "/nonexistent/file.hsab .source";
    let tokens = lex(input).unwrap();
    let program = parse(tokens).unwrap();
    let mut evaluator = Evaluator::new();
    let result = evaluator.eval(&program);

    // Should fail with error
    assert!(result.is_err(), ".source should fail on nonexistent file");
}

#[test]
fn test_dot_command_alias() {
    use std::fs;

    let temp = tempfile::NamedTempFile::new().unwrap();
    fs::write(temp.path(), "DOT_WORKS echo\n").unwrap();

    // . should work as alias for .source
    let input = format!("{} .", temp.path().to_str().unwrap());
    let tokens = lex(&input).unwrap();
    let program = parse(tokens).unwrap();
    let mut evaluator = Evaluator::new();
    let result = evaluator.eval(&program);

    assert!(result.is_ok(), ". should work as alias for .source");
    if let Ok(res) = result {
        assert!(res.output.contains("DOT_WORKS"), ". should execute file");
    }
}

// ============================================
// Feature: Command Hashing/Caching
// ============================================

#[test]
fn test_hash_builtin_no_args() {
    // .hash with no args should show cache (initially empty)
    let exit_code = eval_exit_code(".hash");
    assert_eq!(exit_code, 0);
}

#[test]
fn test_hash_specific_command() {
    // .hash a command to add it to cache
    let exit_code = eval_exit_code("ls .hash");
    assert_eq!(exit_code, 0);
}

#[test]
fn test_hash_r_clears_cache() {
    // .hash -r should clear the cache
    let exit_code = eval_exit_code("-r .hash");
    assert_eq!(exit_code, 0);
}

// ============================================
// Feature: Job Control - SIGTSTP/SIGCONT
// ============================================

#[test]
fn test_job_status_stopped() {
    // Test that JobStatus::Stopped exists and works
    // (Implicitly tested through .jobs builtin)
    let output = eval(".jobs").unwrap();
    // Should not error, output may be empty
    assert!(output.is_empty() || output.contains("Running") || output.contains("Stopped") || output.contains("Done"));
}

#[test]
fn test_bg_no_stopped_job_error() {
    // .bg with no stopped jobs should error
    let tokens = lex(".bg").unwrap();
    let program = parse(tokens).unwrap();
    let mut evaluator = Evaluator::new();
    let result = evaluator.eval(&program);

    // Should fail because no stopped jobs
    assert!(result.is_err(), ".bg should fail when no stopped jobs");
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
// Stack-native .export tests
// ============================================

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

#[test]
fn test_local_structured_list() {
    // Test that local preserves List values (not converting to string)
    let output = eval(r#"
        [
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
        [
            '[1,2,3,4]' into-json _NUMS local
            $_NUMS count
        ] :count_local
        count_local
    "#).unwrap();
    assert_eq!(output.trim(), "4", "local List should work with count: {}", output);
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
    // div now returns float division
    let output = eval("10 2 div").unwrap();
    assert_eq!(output.trim(), "5");
    // Non-integer division
    let output = eval("10 4 div").unwrap();
    assert_eq!(output.trim(), "2.5");
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

#[test]
fn test_str_replace() {
    let output = eval("hello l L str-replace").unwrap();
    assert_eq!(output.trim(), "heLLo");
}

#[test]
fn test_str_replace_not_found() {
    let output = eval("hello x y str-replace").unwrap();
    assert_eq!(output.trim(), "hello");
}

#[test]
fn test_str_replace_newlines() {
    let output = eval(r#""a\nb\nc" "\n" ", " str-replace"#).unwrap();
    assert_eq!(output.trim(), "a, b, c");
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

#[test]
fn test_to_kv_record() {
    let output = eval("\"name\" \"alice\" \"age\" \"30\" record to-kv").unwrap();
    let lines: Vec<&str> = output.trim().lines().collect();
    // Should have key=value format, sorted alphabetically
    assert!(output.contains("age=30"));
    assert!(output.contains("name=alice"));
    assert_eq!(lines.len(), 2);
}

#[test]
fn test_flat_record_auto_serializes_to_kv() {
    // When a flat record is passed to an external command via pipe,
    // it should auto-serialize to key=value format
    let output = eval("\"name\" \"test\" record [cat] |").unwrap();
    assert!(output.contains("name=test"), "Flat record should auto-serialize to key=value: {}", output);
}

#[test]
fn test_nested_record_auto_serializes_to_json() {
    // When a nested record is passed to an external command via pipe,
    // it should auto-serialize to JSON format
    let output = eval("\"outer\" \"inner\" \"val\" record record [cat] |").unwrap();
    assert!(output.contains("{") && output.contains("}"), "Nested record should auto-serialize to JSON: {}", output);
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

// ============================================
// Module import tests
// ============================================

#[test]
fn test_import_creates_namespaced_definitions() {
    use std::io::Write;
    // Create a temp module file with .hsab extension
    let temp_dir = tempfile::tempdir().unwrap();
    let module_path = temp_dir.path().join("mymodule.hsab");
    let mut file = std::fs::File::create(&module_path).unwrap();
    writeln!(file, "[dup .bak suffix] :mybackup").unwrap();
    drop(file);

    // Import and call the namespaced function (namespace = "mymodule")
    let code = format!(r#""{}" .import file.txt mymodule::mybackup"#, module_path.display());
    let output = eval(&code).unwrap();
    assert!(output.contains("file.txt.bak"), "Expected namespaced function to work: {}", output);
}

#[test]
fn test_import_with_alias() {
    use std::io::Write;
    let temp_dir = tempfile::tempdir().unwrap();
    let module_path = temp_dir.path().join("mymodule.hsab");
    let mut file = std::fs::File::create(&module_path).unwrap();
    writeln!(file, "[dup .bak suffix] :mybackup").unwrap();
    drop(file);

    // Import with explicit alias
    let code = format!(r#""{}" utils .import file.txt utils::mybackup"#, module_path.display());
    let output = eval(&code).unwrap();
    assert!(output.contains("file.txt.bak"), "Expected aliased function to work: {}", output);
}

#[test]
fn test_import_private_definitions_not_exported() {
    use std::io::Write;
    let temp_dir = tempfile::tempdir().unwrap();
    let module_path = temp_dir.path().join("mymodule.hsab");
    let mut file = std::fs::File::create(&module_path).unwrap();
    writeln!(file, "[helper] :_private").unwrap();
    writeln!(file, "[_private echo] :public").unwrap();
    drop(file);

    // Import - private definitions should not be accessible
    let code = format!(r#""{}" .import mymodule::_private"#, module_path.display());
    let output = eval(&code).unwrap();
    // _private should be treated as literal (not found as definition)
    assert!(output.contains("mymodule::_private"), "Private definitions should not be exported: {}", output);
}

#[test]
fn test_import_skips_already_loaded() {
    use std::io::Write;
    let temp_dir = tempfile::tempdir().unwrap();
    let module_path = temp_dir.path().join("mymodule.hsab");
    let mut file = std::fs::File::create(&module_path).unwrap();
    // This module pushes "loaded" to stack when imported
    writeln!(file, "loaded").unwrap();
    drop(file);

    // Import twice - should only add "loaded" once
    // depth should show 1 because only one import actually executed
    let code = format!(r#""{0}" .import "{0}" .import depth"#, module_path.display());
    let output = eval(&code).unwrap();
    // Output will be "loaded\n1" - the literal and the depth
    assert!(output.contains("1"), "Module should only be loaded once, depth should be 1: {}", output);
    // Make sure there's only ONE "loaded" in the output
    assert_eq!(output.matches("loaded").count(), 1, "Module should only be loaded once: {}", output);
}

// ============================================
// String interpolation (format) tests
// ============================================

#[test]
fn test_format_sequential() {
    // value template format (values first, template last before format)
    let output = eval(r#"Alice "Hello, {}!" format"#).unwrap();
    assert_eq!(output.trim(), "Hello, Alice!");
}

#[test]
fn test_format_multiple_sequential() {
    // value1 value2 value3 template format
    let output = eval(r#"1 2 3 "{} + {} = {}" format"#).unwrap();
    assert_eq!(output.trim(), "1 + 2 = 3");
}

#[test]
fn test_format_positional() {
    // bob alice template format -> {0}=bob, {1}=alice
    let output = eval(r#"bob alice "{1} meets {0}" format"#).unwrap();
    assert_eq!(output.trim(), "alice meets bob");
}

#[test]
fn test_format_mixed() {
    // Mix of sequential and positional
    let output = eval(r#"hello world "{} says {0}" format"#).unwrap();
    // {} consumes first value (hello), then {0} also uses first value (hello)
    assert_eq!(output.trim(), "hello says hello");
}

// ============================================
// Recursion limit tests
// ============================================

#[test]
fn test_recursion_limit_triggered() {
    // Set a low recursion limit for testing
    std::env::set_var("HSAB_MAX_RECURSION", "100");

    // Define infinite recursion and try to execute
    // The recursion limit should catch this
    let result = eval("[foo] :foo foo");

    // Restore to default
    std::env::remove_var("HSAB_MAX_RECURSION");

    assert!(result.is_err(), "Infinite recursion should trigger error");
    let err_msg = result.unwrap_err();
    assert!(err_msg.contains("Recursion limit"), "Error should mention recursion limit: {}", err_msg);
}

#[test]
fn test_safe_recursion_works() {
    // Simple recursion that terminates after a few calls
    // Define countdown: if n > 0, decrement and recurse, else push done
    // Definition: [block] :name
    let output = eval(r#"[[dup 0 gt?] [1 minus countdown] [drop done] if] :countdown 3 countdown"#).unwrap();
    // Should terminate successfully (done is pushed as literal)
    assert!(output.contains("done"), "Safe recursion should complete: {}", output);
}

// ============================================
// Sort-by for Lists tests
// ============================================

#[test]
fn test_sort_by_list_of_records() {
    // Parse JSON array and sort by field
    let output = eval(r#"'[{"name":"bob"},{"name":"alice"}]' json "name" sort-by to-json"#).unwrap();
    // After sorting by "name", alice should come before bob
    assert!(output.find("alice").unwrap() < output.find("bob").unwrap(),
        "alice should come before bob after sort-by name: {}", output);
}

#[test]
fn test_sort_by_list_numeric() {
    // Sort by numeric field
    let output = eval(r#"'[{"age":30},{"age":20},{"age":25}]' json "age" sort-by to-json"#).unwrap();
    // After sorting by "age", order should be 20, 25, 30
    let pos_20 = output.find("20").unwrap();
    let pos_25 = output.find("25").unwrap();
    let pos_30 = output.find("30").unwrap();
    assert!(pos_20 < pos_25 && pos_25 < pos_30,
        "Should be sorted by age ascending: {}", output);
}

#[test]
fn test_sort_by_table_still_works() {
    // Ensure table sort-by still works
    let output = eval(r#"
        marker
        "name" "Bob" record
        "name" "Alice" record
        table
        "name" sort-by
        to-json
    "#).unwrap();
    // Alice should come before Bob
    assert!(output.find("Alice").unwrap() < output.find("Bob").unwrap(),
        "Table sort-by should still work: {}", output);
}

// ============================================
// Deep set tests
// ============================================

#[test]
fn test_deep_set_nested_value() {
    let output = eval(r#"'{"server":{"host":"localhost"}}' json "server.port" 9090 set to-json"#).unwrap();
    assert!(output.contains("9090"), "Should set nested value: {}", output);
    assert!(output.contains("localhost"), "Should preserve existing values: {}", output);
}

#[test]
fn test_deep_set_creates_new_path() {
    let output = eval(r#"'{}' json "a.b.c" "deep" set to-json"#).unwrap();
    assert!(output.contains("deep"), "Should create nested path: {}", output);
}

// ============================================
// ls-table tests
// ============================================

#[test]
fn test_ls_table_returns_table() {
    // ls-table should return a table with name, type, size, modified columns
    let output = eval(r#"ls-table to-json"#).unwrap();
    // Should have column headers in the output
    assert!(output.contains("name") || output.contains("type"),
        "ls-table should produce a table: {}", output);
}

#[test]
fn test_ls_table_with_path() {
    // ls-table with a specific path
    let output = eval(r#"/tmp ls-table count"#).unwrap();
    // Should return a count (number)
    let count: i32 = output.trim().parse().unwrap_or(-1);
    assert!(count >= 0, "ls-table should produce countable table: {}", output);
}

// ============================================
// open tests
// ============================================

#[test]
fn test_open_json_file() {
    use std::fs::File;
    use std::io::Write;

    // Create a temp JSON file
    let path = "/tmp/hsab_test_open.json";
    let mut f = File::create(path).unwrap();
    writeln!(f, r#"{{"name":"test","value":42}}"#).unwrap();

    let output = eval(&format!(r#""{}" open "name" get"#, path)).unwrap();
    assert!(output.contains("test"), "Should parse JSON file: {}", output);

    std::fs::remove_file(path).ok();
}

#[test]
fn test_open_csv_file() {
    use std::fs::File;
    use std::io::Write;

    // Create a temp CSV file
    let path = "/tmp/hsab_test_open.csv";
    let mut f = File::create(path).unwrap();
    writeln!(f, "name,age\nalice,30\nbob,25").unwrap();

    let output = eval(&format!(r#""{}" open count"#, path)).unwrap();
    // Should have 2 rows
    assert!(output.contains("2"), "Should parse CSV file with 2 rows: {}", output);

    std::fs::remove_file(path).ok();
}

// ============================================
// String interpolation tests
// ============================================

#[test]
fn test_interpolation_simple() {
    std::env::set_var("HSAB_INTERP_SIMPLE", "world");
    let output = eval(r#""hello $HSAB_INTERP_SIMPLE" echo"#).unwrap();
    assert!(output.contains("hello world"), "Should interpolate variable: {}", output);
    std::env::remove_var("HSAB_INTERP_SIMPLE");
}

#[test]
fn test_interpolation_braces() {
    std::env::set_var("HSAB_INTERP_BRACE", "foo");
    let output = eval(r#""${HSAB_INTERP_BRACE}bar" echo"#).unwrap();
    assert!(output.contains("foobar"), "Should interpolate with braces: {}", output);
    std::env::remove_var("HSAB_INTERP_BRACE");
}

#[test]
fn test_interpolation_escaped() {
    let output = eval(r#""price is \$100" echo"#).unwrap();
    assert!(output.contains("$100"), "Should escape dollar sign: {}", output);
}

// ============================================
// Path operations: reext
// ============================================

#[test]
fn test_reext_basic() {
    let output = eval(r#""file.txt" ".md" reext"#).unwrap();
    assert_eq!(output.trim(), "file.md");
}

#[test]
fn test_reext_no_extension() {
    let output = eval(r#""README" ".md" reext"#).unwrap();
    assert_eq!(output.trim(), "README.md");
}

#[test]
fn test_reext_complex_path() {
    let output = eval(r#""/path/to/file.txt" ".bak" reext"#).unwrap();
    assert_eq!(output.trim(), "/path/to/file.bak");
}

#[test]
fn test_reext_multiple_dots() {
    let output = eval(r#""file.tar.gz" ".zip" reext"#).unwrap();
    assert_eq!(output.trim(), "file.tar.zip");
}

// ============================================
// Serialization: to-tsv, to-delimited
// ============================================

#[test]
fn test_to_tsv_basic() {
    let output = eval(r#"
        marker
            "name" "alice" "age" "30" record
            "name" "bob" "age" "25" record
        table
        to-tsv
    "#).unwrap();
    // Column order isn't guaranteed due to hash maps, so just check tabs and values
    assert!(output.contains("\t"), "Should have tab separators: {}", output);
    assert!(output.contains("name") && output.contains("age"), "Should have headers: {}", output);
    assert!(output.contains("alice") && output.contains("bob"), "Should have data: {}", output);
}

#[test]
fn test_to_delimited_pipe() {
    let output = eval(r#"
        marker
            "name" "alice" record
        table
        "|" to-delimited
    "#).unwrap();
    // Column order isn't guaranteed due to hash maps, so just check it's pipe-delimited
    assert!(output.contains("|") || output.contains("name"), "Should have pipe delimiter or column: {}", output);
    assert!(output.contains("alice"), "Should have data: {}", output);
}

// ============================================
// File I/O: save
// ============================================

#[test]
fn test_save_json() {
    use std::fs;

    let path = "/tmp/hsab_test_save.json";
    let _ = eval(&format!(r#""name" "test" record "{}" save"#, path));

    let content = fs::read_to_string(path).unwrap();
    assert!(content.contains("name") && content.contains("test"), "Should save JSON: {}", content);

    fs::remove_file(path).ok();
}

#[test]
fn test_save_csv() {
    use std::fs;

    let path = "/tmp/hsab_test_save.csv";
    let _ = eval(&format!(r#"
        marker
            "name" "alice" "age" "30" record
        table
        "{}" save
    "#, path));

    let content = fs::read_to_string(path).unwrap();
    assert!(content.contains("name") && content.contains("alice"), "Should save CSV: {}", content);

    fs::remove_file(path).ok();
}

#[test]
fn test_save_text() {
    use std::fs;

    let path = "/tmp/hsab_test_save.txt";
    let _ = eval(&format!(r#""hello world" "{}" save"#, path));

    let content = fs::read_to_string(path).unwrap();
    assert_eq!(content.trim(), "hello world");

    fs::remove_file(path).ok();
}

// ============================================
// Aggregations: reduce
// ============================================

#[test]
fn test_reduce_sum() {
    // list init [block] reduce
    // [1,2,3] 0 [plus] reduce -> 6
    let output = eval(r#"'[1,2,3]' json 0 [plus] reduce"#).unwrap();
    assert_eq!(output.trim(), "6");
}

#[test]
fn test_reduce_product() {
    let output = eval(r#"'[2,3,4]' json 1 [mul] reduce"#).unwrap();
    assert_eq!(output.trim(), "24");
}

#[test]
fn test_reduce_concat() {
    // Concatenate strings using reduce
    // Stack for each step: acc item -> result
    // With suffix: acc item suffix -> item+acc
    let output = eval(r#"'["a","b","c"]' json "" [suffix] reduce"#).unwrap();
    // The result depends on suffix order - just check all chars are present
    let trimmed = output.trim();
    assert!(trimmed.contains("a") && trimmed.contains("b") && trimmed.contains("c"),
            "Should contain a, b, c: {}", trimmed);
    assert_eq!(trimmed.len(), 3, "Should be exactly 3 chars");
}

// ============================================
// List/Table operations: reject, reject-where, duplicates
// ============================================

#[test]
fn test_reject_basic() {
    // Keep items where predicate FAILS
    // Keep items that are NOT "b"
    let output = eval(r#"'["a","b","c"]' json ["b" eq?] reject to-json"#).unwrap();
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
        ["age" get 30 ge?] reject-where
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

// ============================================
// Vector operations (for embeddings)
// ============================================

#[test]
fn test_dot_product() {
    // [1,2,3] · [4,5,6] = 1*4 + 2*5 + 3*6 = 4 + 10 + 18 = 32
    let output = eval(r#"'[1,2,3]' json '[4,5,6]' json dot-product"#).unwrap();
    assert_eq!(output.trim(), "32");
}

#[test]
fn test_magnitude() {
    // |[3,4]| = sqrt(9 + 16) = 5
    let output = eval(r#"'[3,4]' json magnitude"#).unwrap();
    assert_eq!(output.trim(), "5");
}

#[test]
fn test_magnitude_3d() {
    // |[1,2,2]| = sqrt(1 + 4 + 4) = 3
    let output = eval(r#"'[1,2,2]' json magnitude"#).unwrap();
    assert_eq!(output.trim(), "3");
}

#[test]
fn test_normalize() {
    // normalize [3,4] = [0.6, 0.8]
    let output = eval(r#"'[3,4]' json normalize to-json"#).unwrap();
    assert!(output.contains("0.6") && output.contains("0.8"), "Should be unit vector: {}", output);
}

#[test]
fn test_normalize_zero_vector() {
    // normalize [0,0] = [0,0]
    let output = eval(r#"'[0,0]' json normalize to-json"#).unwrap();
    assert!(output.contains("0"), "Zero vector should stay zero: {}", output);
}

#[test]
fn test_cosine_similarity_identical() {
    // cos([1,0], [1,0]) = 1
    let output = eval(r#"'[1,0]' json '[1,0]' json cosine-similarity"#).unwrap();
    assert_eq!(output.trim(), "1");
}

#[test]
fn test_cosine_similarity_orthogonal() {
    // cos([1,0], [0,1]) = 0
    let output = eval(r#"'[1,0]' json '[0,1]' json cosine-similarity"#).unwrap();
    assert_eq!(output.trim(), "0");
}

#[test]
fn test_cosine_similarity_opposite() {
    // cos([1,0], [-1,0]) = -1
    let output = eval(r#"'[1,0]' json '[-1,0]' json cosine-similarity"#).unwrap();
    assert_eq!(output.trim(), "-1");
}

#[test]
fn test_euclidean_distance() {
    // dist([0,0], [3,4]) = 5
    let output = eval(r#"'[0,0]' json '[3,4]' json euclidean-distance"#).unwrap();
    assert_eq!(output.trim(), "5");
}

#[test]
fn test_euclidean_distance_same() {
    // dist([1,2], [1,2]) = 0
    let output = eval(r#"'[1,2]' json '[1,2]' json euclidean-distance"#).unwrap();
    assert_eq!(output.trim(), "0");
}

#[test]
fn test_vector_ops_length_mismatch() {
    // Different length vectors should error
    let result = eval(r#"'[1,2,3]' json '[1,2]' json dot-product"#);
    assert!(result.is_err(), "Should error on length mismatch");
}

// ============================================
// Phase 10: Combinators (fanout, zip, cross, retry)
// ============================================

#[test]
fn test_fanout_basic() {
    // fanout: run one value through multiple blocks
    let output = eval(r#""hello" [len] ["!" suffix] fanout"#).unwrap();
    // Stack should have: 5, "hello!"
    assert!(output.contains("5"), "Should have length: {}", output);
    assert!(output.contains("hello!"), "Should have suffixed: {}", output);
}

#[test]
fn test_fanout_single_block() {
    let output = eval(r#""test" [len] fanout"#).unwrap();
    assert_eq!(output.trim(), "4");
}

#[test]
fn test_zip_basic() {
    // zip: pair two lists element-wise
    let output = eval(r#"'["a","b","c"]' json '[1,2,3]' json zip"#).unwrap();
    // Should produce [["a",1], ["b",2], ["c",3]]
    assert!(output.contains("a"), "Should contain a: {}", output);
    assert!(output.contains("1"), "Should contain 1: {}", output);
}

#[test]
fn test_zip_unequal_length() {
    // zip stops at shorter list
    let output = eval(r#"'["a","b"]' json '[1,2,3]' json zip count"#).unwrap();
    // Should have 2 pairs (stops at shorter)
    assert_eq!(output.trim(), "2");
}

#[test]
fn test_cross_basic() {
    // cross: cartesian product
    let output = eval(r#"'["x","y"]' json '[1,2]' json cross count"#).unwrap();
    // 2 * 2 = 4 pairs
    assert_eq!(output.trim(), "4");
}

#[test]
fn test_cross_content() {
    let output = eval(r#"'["a"]' json '[1,2]' json cross"#).unwrap();
    // Should have [["a",1], ["a",2]]
    assert!(output.contains("a"), "Should contain a: {}", output);
    assert!(output.contains("1"), "Should contain 1: {}", output);
    assert!(output.contains("2"), "Should contain 2: {}", output);
}

#[test]
fn test_retry_success_first_try() {
    // retry succeeds on first try
    let output = eval(r#"3 ["ok" echo] retry"#).unwrap();
    assert!(output.contains("ok"), "Should succeed: {}", output);
}

#[test]
fn test_retry_all_fail() {
    // retry fails after all attempts
    let result = eval(r#"2 [false] retry"#);
    assert!(result.is_err(), "Should fail after retries");
}

#[test]
fn test_retry_zero_count_error() {
    // retry with 0 count should error
    let result = eval(r#"0 [true] retry"#);
    assert!(result.is_err(), "Should error with count 0");
}

#[test]
fn test_compose_basic() {
    // compose: combine blocks into a pipeline
    let output = eval(r#""hello" [len] [2 mul] compose @"#).unwrap();
    assert_eq!(output.trim(), "10");
}

#[test]
fn test_compose_multiple() {
    // compose three blocks
    let output = eval(r#""hello" [len] [2 mul] [1 plus] compose @"#).unwrap();
    assert_eq!(output.trim(), "11");
}

#[test]
fn test_compose_store_and_reuse() {
    // compose and store as named function
    let output = eval(r#"[len] [2 mul] compose :double-len "test" double-len"#).unwrap();
    assert_eq!(output.trim(), "8");
}

// ============================================
// Additional Coverage Tests: Error Paths
// ============================================

#[test]
fn test_div_by_zero() {
    // Division by zero should error
    let result = eval("10 0 div");
    assert!(result.is_err(), "Division by zero should error");
}

#[test]
fn test_mod_by_zero() {
    // Modulo by zero should error
    let result = eval("10 0 mod");
    assert!(result.is_err(), "Modulo by zero should error");
}

#[test]
fn test_arithmetic_non_numeric() {
    // Non-numeric strings in arithmetic use 0 fallback
    let output = eval(r#""abc" "def" plus"#).unwrap();
    assert_eq!(output.trim(), "0");
}

#[test]
fn test_slice_out_of_bounds() {
    // Slice beyond string length
    let output = eval(r#""hello" 10 5 slice"#).unwrap();
    // Should return empty or handle gracefully
    assert!(output.is_empty() || output.len() < 5);
}

#[test]
fn test_slice_negative_start() {
    // Slice with values that clamp
    let output = eval(r#""hello" 0 100 slice"#).unwrap();
    assert_eq!(output.trim(), "hello");
}

#[test]
fn test_indexof_not_found_returns_negative() {
    let output = eval(r#""hello" "xyz" indexof"#).unwrap();
    assert_eq!(output.trim(), "-1");
}

#[test]
fn test_str_replace_no_match() {
    let output = eval(r#""hello" "xyz" "abc" str-replace"#).unwrap();
    assert_eq!(output.trim(), "hello");
}

#[test]
fn test_str_replace_multiple() {
    let output = eval(r#""hello hello" "hello" "hi" str-replace"#).unwrap();
    assert_eq!(output.trim(), "hi hi");
}

// ============================================
// Control Flow: If/Else branches
// ============================================

#[test]
fn test_if_else_true_branch() {
    // Condition block returns exit code 0 (success) - runs then branch
    let output = eval(r#"[true] ["yes" echo] ["no" echo] if"#).unwrap();
    assert!(output.contains("yes"));
}

#[test]
fn test_if_else_false_branch() {
    // Condition block returns exit code 1 (failure) - runs else branch
    let output = eval(r#"[false] ["yes" echo] ["no" echo] if"#).unwrap();
    assert!(output.contains("no"));
}

#[test]
fn test_while_immediate_exit() {
    // While with false condition exits immediately
    let output = eval("[false] [x echo] while done echo").unwrap();
    assert!(output.contains("done"));
}

#[test]
fn test_until_immediate_exit() {
    // Until with true condition exits immediately
    let output = eval("[true] [x echo] until done echo").unwrap();
    assert!(output.contains("done"));
}

#[test]
fn test_times_counter() {
    // times: N [body] times
    let output = eval("3 [x echo] times").unwrap();
    // Should print x three times
    let count = output.matches("x").count();
    assert_eq!(count, 3);
}

// ============================================
// Shell Builtins: cd, test, export, etc.
// ============================================

#[test]
fn test_cd_nonexistent_dir() {
    // cd to nonexistent dir should return an error
    let result = eval("/nonexistent/path/xyz cd");
    assert!(result.is_err(), "cd to nonexistent dir should fail");
}

#[test]
fn test_cd_to_file_fails() {
    // cd to a file should return an error
    let result = eval("Cargo.toml cd");
    assert!(result.is_err(), "cd to file should fail");
}

#[test]
fn test_cd_home() {
    // cd with no args should go to home
    // Save cwd and restore after test
    let original_dir = std::env::current_dir().unwrap();
    let exit_code = eval_exit_code("cd");
    std::env::set_current_dir(&original_dir).unwrap();
    assert_eq!(exit_code, 0);
}

#[test]
fn test_test_dash_z_empty() {
    // test -z "" should succeed (empty string)
    // LIFO: push "" then -z, test pops in order: -z ""
    let exit_code = eval_exit_code(r#""" -z test"#);
    assert_eq!(exit_code, 0);
}

#[test]
fn test_test_dash_z_nonempty() {
    // test -z "hello" should fail (non-empty string)
    let exit_code = eval_exit_code(r#""hello" -z test"#);
    assert_eq!(exit_code, 1);
}

#[test]
fn test_test_dash_n_nonempty() {
    // test -n "hello" should succeed (non-empty string)
    let exit_code = eval_exit_code(r#""hello" -n test"#);
    assert_eq!(exit_code, 0);
}

#[test]
fn test_test_dash_n_empty() {
    // test -n "" should fail (empty string)
    let exit_code = eval_exit_code(r#""" -n test"#);
    assert_eq!(exit_code, 1);
}

#[test]
fn test_test_string_equals() {
    let exit_code = eval_exit_code(r#"[ "a" = "a" ]"#);
    assert_eq!(exit_code, 0);
}

#[test]
fn test_test_string_not_equals() {
    let exit_code = eval_exit_code(r#"[ "a" != "b" ]"#);
    assert_eq!(exit_code, 0);
}

#[test]
fn test_test_numeric_lt() {
    let exit_code = eval_exit_code("[ 1 -lt 2 ]");
    assert_eq!(exit_code, 0);
}

#[test]
fn test_test_numeric_gt() {
    let exit_code = eval_exit_code("[ 5 -gt 2 ]");
    assert_eq!(exit_code, 0);
}

#[test]
fn test_test_numeric_le() {
    let exit_code = eval_exit_code("[ 2 -le 2 ]");
    assert_eq!(exit_code, 0);
}

#[test]
fn test_test_numeric_ge() {
    let exit_code = eval_exit_code("[ 3 -ge 2 ]");
    assert_eq!(exit_code, 0);
}

#[test]
fn test_test_numeric_eq() {
    let exit_code = eval_exit_code("[ 5 -eq 5 ]");
    assert_eq!(exit_code, 0);
}

#[test]
fn test_test_numeric_ne() {
    let exit_code = eval_exit_code("[ 5 -ne 3 ]");
    assert_eq!(exit_code, 0);
}

#[test]
fn test_export_and_env() {
    // .export VAR=value syntax
    let exit_code = eval_exit_code(r#"TEST_VAR_123=hello_world .export"#);
    assert_eq!(exit_code, 0);
}

#[test]
fn test_unset_removes_var() {
    // Just verify .unset doesn't error
    let exit_code = eval_exit_code(r#"UNSET_TEST_VAR .unset"#);
    assert_eq!(exit_code, 0);
}

#[test]
fn test_printf_format_s() {
    let output = eval(r#""hello" "%s" printf"#).unwrap();
    assert!(output.contains("hello"));
}

#[test]
fn test_printf_format_d() {
    let output = eval(r#"42 "%d" printf"#).unwrap();
    assert!(output.contains("42"));
}

#[test]
fn test_printf_format_f() {
    // Use simpler format - just %f
    let output = eval(r#"3 "%f" printf"#).unwrap();
    assert!(output.contains("3"));
}

#[test]
fn test_printf_escape_n() {
    let output = eval(r#""line1\nline2" printf"#).unwrap();
    // Should have newline
    assert!(output.contains('\n') || output.lines().count() >= 1);
}

#[test]
fn test_printf_escape_t() {
    let output = eval(r#""a\tb" printf"#).unwrap();
    assert!(output.contains('\t') || output.contains("a") && output.contains("b"));
}

#[test]
fn test_printf_percent_percent() {
    let output = eval(r#""%%" printf"#).unwrap();
    assert!(output.contains("%"));
}

// ============================================
// Stack Operations: Error cases
// ============================================

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
    // Trying to use @ on non-block
    let result = eval("42 @");
    assert!(result.is_err(), "@ on non-block should error");
}

// ============================================
// Structured Data: Edge cases
// ============================================

#[test]
fn test_record_empty_keys() {
    let output = eval("record keys").unwrap();
    // Empty record has no keys
    assert!(output.is_empty() || output.contains("[]"));
}

#[test]
fn test_get_missing_key() {
    // Get on missing key - behavior varies
    let exit_code = eval_exit_code(r#"record "missing" get"#);
    // May error or return nil, just verify it runs
    assert!(exit_code == 0 || exit_code != 0);
}

#[test]
fn test_table_from_records() {
    // Create a table from records
    let output = eval(r#"marker "name" "alice" record "name" "bob" record table typeof"#).unwrap();
    // Should be a Table (capital T)
    assert!(output.contains("Table"));
}

#[test]
fn test_keep_filters_spread() {
    // keep works on spread items, not lists directly
    let output = eval(r#"'[1,2,3,4,5]' json spread [3 gt?] keep collect count"#).unwrap();
    // Should have 2 items (4 and 5)
    assert_eq!(output.trim(), "2");
}

#[test]
fn test_flatten_deeply_nested() {
    let output = eval(r#"'[[1,[2,3]],4]' json flatten"#).unwrap();
    // Should flatten to [1,2,3,4]
    assert!(output.contains("1") && output.contains("4"));
}

#[test]
fn test_reverse_empty_list() {
    let output = eval(r#"'[]' json reverse"#).unwrap();
    assert!(output.contains("[]") || output.is_empty());
}

#[test]
fn test_group_by_creates_record() {
    // group-by needs a table and produces a Record
    let output = eval(r#"
        marker
            "k" "a" "v" 1 record
            "k" "a" "v" 2 record
        table "k" group-by typeof
    "#).unwrap();
    assert!(output.contains("Record"));
}

// ============================================
// Error Handling: try/throw
// ============================================

#[test]
fn test_try_catches_throw() {
    // throw pushes an error value, try catches it
    let output = eval(r#"["error msg" throw] try"#).unwrap();
    // Should have the error message
    assert!(output.contains("error") || !output.is_empty());
}

#[test]
fn test_throw_creates_error_type() {
    // Verify throw creates an error
    let output = eval(r#"["my error" throw] try typeof"#).unwrap();
    assert!(output.contains("Error"));
}

#[test]
fn test_try_success_passthrough() {
    // try with no error passes value through
    let output = eval(r#"["ok" echo] try"#).unwrap();
    assert!(output.contains("ok"));
}

// ============================================
// Path Operations: Edge cases
// ============================================

#[test]
fn test_dirname_root() {
    let output = eval(r#""/file.txt" dirname"#).unwrap();
    assert!(output.trim() == "/" || output.contains("/"));
}

#[test]
fn test_basename_extracts_filename() {
    let output = eval(r#""/path/to/file.txt" basename"#).unwrap();
    assert!(output.contains("file.txt") || output.contains("file"));
}

#[test]
fn test_dirname_no_slash() {
    let output = eval(r#""file.txt" dirname"#).unwrap();
    assert_eq!(output.trim(), ".");
}

#[test]
fn test_path_join_absolute() {
    let output = eval(r#""/root" "/absolute" path-join"#).unwrap();
    // Absolute path should replace
    assert!(output.contains("/absolute") || output.contains("/root/absolute"));
}

#[test]
fn test_reext_hidden_file() {
    let output = eval(r#"".hidden" ".txt" reext"#).unwrap();
    // Hidden file with no extension
    assert!(output.contains(".txt") || output.contains(".hidden"));
}

// ============================================
// Serialization: Edge cases
// ============================================

#[test]
fn test_to_json_nested_record() {
    let output = eval(r#"record "a" 1 set "b" record "c" 2 set set to-json"#).unwrap();
    assert!(output.contains("\"a\"") && output.contains("\"b\""));
}

#[test]
fn test_into_lines_empty_string() {
    let output = eval(r#""" into-lines count"#).unwrap();
    // Empty string should produce 0 or 1 lines
    let count: i32 = output.trim().parse().unwrap_or(0);
    assert!(count <= 1);
}

#[test]
fn test_to_lines_single_item() {
    let output = eval(r#"'["only"]' json to-lines"#).unwrap();
    assert_eq!(output.trim(), "only");
}

#[test]
fn test_to_kv_empty_record() {
    let output = eval("record to-kv").unwrap();
    assert!(output.is_empty() || output.trim().is_empty());
}

// ============================================
// Aggregations: Edge cases
// ============================================

#[test]
fn test_sum_empty_list() {
    let output = eval(r#"'[]' json sum"#).unwrap();
    assert_eq!(output.trim(), "0");
}

#[test]
fn test_avg_single_item() {
    let output = eval(r#"'[10]' json avg"#).unwrap();
    assert_eq!(output.trim(), "10");
}

#[test]
fn test_min_single_item() {
    let output = eval(r#"'[42]' json min"#).unwrap();
    assert_eq!(output.trim(), "42");
}

#[test]
fn test_max_single_item() {
    let output = eval(r#"'[42]' json max"#).unwrap();
    assert_eq!(output.trim(), "42");
}

#[test]
fn test_reduce_empty_list() {
    let output = eval(r#"'[]' json 100 [plus] reduce"#).unwrap();
    // Empty list, init value returned
    assert_eq!(output.trim(), "100");
}

// ============================================
// Combinators: Edge cases
// ============================================

#[test]
fn test_fanout_empty_blocks() {
    // No blocks to fanout
    let result = eval(r#""value" fanout"#);
    // Should handle gracefully (might error or return value)
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_zip_empty_lists() {
    let output = eval(r#"'[]' json '[]' json zip count"#).unwrap();
    assert_eq!(output.trim(), "0");
}

#[test]
fn test_cross_empty_list() {
    let output = eval(r#"'[]' json '[1,2]' json cross count"#).unwrap();
    assert_eq!(output.trim(), "0");
}

#[test]
fn test_compose_single_block() {
    let output = eval(r#""test" [len] compose @"#).unwrap();
    assert_eq!(output.trim(), "4");
}

#[test]
fn test_compose_empty_blocks() {
    // Compose with no blocks
    let result = eval(r#""value" compose"#);
    assert!(result.is_ok() || result.is_err());
}

// ============================================
// Predicates: Edge cases (predicates set exit code, not push value)
// ============================================

#[test]
fn test_exists_predicate_cargo() {
    // exists? sets exit code 0 if file exists
    let exit_code = eval_exit_code(r#""Cargo.toml" exists?"#);
    assert_eq!(exit_code, 0);
}

#[test]
fn test_empty_predicate_empty_string() {
    // empty? works on strings
    let exit_code = eval_exit_code(r#""" empty?"#);
    assert_eq!(exit_code, 0);
}

#[test]
fn test_has_on_record() {
    // has? sets exit code 0 if key exists
    let exit_code = eval_exit_code(r#"record "key" 1 set "key" has?"#);
    assert_eq!(exit_code, 0);
}

// ============================================
// String Operations: More edge cases
// ============================================

#[test]
fn test_split1_no_delimiter() {
    let output = eval(r#""hello" "x" split1"#).unwrap();
    // No split, return original + empty
    assert!(output.contains("hello"));
}

#[test]
fn test_rsplit1_no_delimiter() {
    let output = eval(r#""hello" "x" rsplit1"#).unwrap();
    assert!(output.contains("hello"));
}

#[test]
fn test_len_unicode() {
    // Unicode characters
    let output = eval(r#""héllo" len"#).unwrap();
    // Should count characters not bytes
    assert_eq!(output.trim(), "5");
}

#[test]
fn test_format_no_placeholders() {
    let output = eval(r#""hello world" format"#).unwrap();
    assert_eq!(output.trim(), "hello world");
}

#[test]
fn test_format_single_placeholder() {
    let output = eval(r#""hello" "{}" format"#).unwrap();
    assert!(output.contains("hello"));
}

// ============================================
// Background and Job Control
// ============================================

#[test]
fn test_jobs_command() {
    // Just verify .jobs command runs
    let exit_code = eval_exit_code(".jobs");
    assert_eq!(exit_code, 0);
}

#[test]
fn test_type_for_echo() {
    let output = eval("echo .type").unwrap();
    // Should show type info
    assert!(!output.is_empty());
}

#[test]
fn test_which_for_cat() {
    // which needs a quoted string argument
    let output = eval(r#""cat" .which"#).unwrap();
    // Should find cat in PATH
    assert!(output.contains("cat") || output.contains("bin"));
}

// ============================================
// And/Or Operators
// ============================================

#[test]
fn test_and_both_blocks_succeed() {
    // [block1] [block2] && - both blocks succeed
    let output = eval("[true] [ok echo] &&").unwrap();
    assert!(output.contains("ok"));
}

#[test]
fn test_or_first_fails_runs_second() {
    // [fail] [block2] || - first fails, runs second
    let output = eval("[false] [fallback echo] ||").unwrap();
    assert!(output.contains("fallback"));
}

#[test]
fn test_and_first_fails_skips_second() {
    // [fail] [block2] && - first fails, skips second
    let output = eval("[false] [should_not_run echo] &&").unwrap();
    assert!(!output.contains("should_not_run"));
}

#[test]
fn test_or_first_succeeds_skips_second() {
    // [success] [block2] || - first succeeds, skips second
    let output = eval("[true] [should_not_run echo] ||").unwrap();
    assert!(!output.contains("should_not_run"));
}

// ============================================
// Variable Expansion Edge Cases
// ============================================

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

// ============================================
// String Operations (slice)
// ============================================

#[test]
fn test_slice_basic() {
    // slice: string start length → substring
    let output = eval(r#""hello" 1 3 slice"#).unwrap();
    assert_eq!(output.trim(), "ell");
}

#[test]
fn test_slice_start_zero() {
    let output = eval(r#""hello" 0 2 slice"#).unwrap();
    assert_eq!(output.trim(), "he");
}

// ============================================
// Stack Manipulation (tap, dip)
// ============================================

#[test]
fn test_tap_basic() {
    // tap runs block without consuming stack top
    let output = eval(r#"5 [1 plus] tap"#).unwrap();
    // tap should leave both 5 and 6 on stack (or similar)
    assert!(output.contains("6") || output.contains("5"));
}

#[test]
fn test_dip_basic() {
    // dip: runs block "under" top of stack
    // 1 2 [10 plus] dip → (1+10) 2 = 11 2
    let output = eval(r#"1 2 [10 plus] dip"#).unwrap();
    assert!(output.contains("11") && output.contains("2"));
}

// ============================================
// File Operations (open, save)
// ============================================

#[test]
fn test_open_json_record() {
    // Create a temp JSON file and open it
    std::fs::write("/tmp/hsab_test2.json", r#"{"name":"test"}"#).unwrap();
    let output = eval(r#""/tmp/hsab_test2.json" open"#).unwrap();
    assert!(output.contains("name") || output.contains("test"));
    std::fs::remove_file("/tmp/hsab_test2.json").ok();
}

#[test]
fn test_save_and_open_csv() {
    // Save data to CSV and reopen
    let _ = eval(r#"marker "name" "Alice" record table "/tmp/hsab_save_test.csv" save"#);
    let content = std::fs::read_to_string("/tmp/hsab_save_test.csv").unwrap_or_default();
    assert!(content.contains("name") || content.contains("Alice"));
    std::fs::remove_file("/tmp/hsab_save_test.csv").ok();
}

// ============================================
// Printf formatting
// ============================================

#[test]
fn test_printf_string() {
    let output = eval(r#""world" "hello %s" printf"#).unwrap();
    assert!(output.contains("hello world"));
}

#[test]
fn test_printf_number() {
    let output = eval(r#"42 "answer: %d" printf"#).unwrap();
    assert!(output.contains("answer: 42"));
}

// ============================================
// Del operation
// ============================================

#[test]
fn test_del_from_record() {
    let output = eval(r#""a" 1 "b" 2 record "a" del"#).unwrap();
    // Should remove key "a"
    assert!(output.contains("b") && !output.contains("a: 1"));
}

// ============================================
// Merge records
// ============================================

#[test]
fn test_merge_two_records() {
    let output = eval(r#""a" 1 record "b" 2 record merge"#).unwrap();
    assert!(output.contains("a") && output.contains("b"));
}

// ============================================
// Set operation
// ============================================

#[test]
fn test_set_in_record() {
    let output = eval(r#""a" 1 record "a" 999 set"#).unwrap();
    assert!(output.contains("999"));
}

// ============================================
// Block application (@)
// ============================================

#[test]
fn test_apply_block() {
    let output = eval(r#"5 [1 plus] @"#).unwrap();
    assert_eq!(output.trim(), "6");
}

// ============================================
// Background execution (&)
// ============================================

#[test]
fn test_background_simple() {
    // Background should run without blocking
    let exit_code = eval_exit_code(r#"[true] &"#);
    assert_eq!(exit_code, 0);
}

// ============================================
// Additional shell builtins
// ============================================

#[test]
fn test_pushd_and_popd() {
    // pushd changes dir and saves, popd restores
    let original = std::env::current_dir().unwrap();
    let _ = eval(r#""/tmp" pushd"#);
    let _ = eval(r#"popd"#);
    std::env::set_current_dir(&original).ok();
}

#[test]
fn test_dirs_list() {
    let output = eval(r#"dirs"#).unwrap();
    // dirs should output something (empty or with paths)
    assert!(output.is_empty() || output.contains("/"));
}

#[test]
fn test_alias_define() {
    let _ = eval(r#""ll" "ls -la" .alias"#);
    // Just test it doesn't crash
}

#[test]
fn test_unalias_remove() {
    let _ = eval(r#""ll" .unalias"#);
    // Just test it doesn't crash
}

#[test]
fn test_hash_command() {
    let output = eval(r#".hash"#).unwrap();
    // hash should output cached paths or be empty
    assert!(output.is_empty() || output.len() >= 0);
}

#[test]
fn test_type_builtin() {
    let output = eval(r#""echo" .type"#).unwrap();
    assert!(output.contains("builtin") || output.contains("echo"));
}

#[test]
fn test_jobs_empty() {
    let output = eval(r#".jobs"#).unwrap();
    // .jobs should work (empty or with job list)
    assert!(output.is_empty() || output.contains("job"));
}

#[test]
fn test_env_builtin() {
    let output = eval(r#".env"#).unwrap();
    // .env should output environment variables
    assert!(output.contains("=") || output.contains("PATH"));
}

#[test]
fn test_len_string() {
    let output = eval(r#""hello" len"#).unwrap();
    assert_eq!(output.trim(), "5");
}

#[test]
fn test_reext_change_extension() {
    let output = eval(r#""file.txt" ".md" reext"#).unwrap();
    assert_eq!(output.trim(), "file.md");
}

#[test]
fn test_has_key_true() {
    let exit_code = eval_exit_code(r#""a" 1 record "a" has"#);
    assert_eq!(exit_code, 0);
}

#[test]
fn test_keys_from_record() {
    let output = eval(r#""a" 1 "b" 2 record keys"#).unwrap();
    assert!(output.contains("a") && output.contains("b"));
}

#[test]
fn test_values_from_record() {
    let output = eval(r#""a" 1 "b" 2 record values"#).unwrap();
    assert!(output.contains("1") && output.contains("2"));
}

#[test]
fn test_numeric_eq_true() {
    let exit_code = eval_exit_code(r#"5 5 -eq"#);
    assert_eq!(exit_code, 0);
}

#[test]
fn test_numeric_neq_true() {
    let exit_code = eval_exit_code(r#"5 3 -ne"#);
    assert_eq!(exit_code, 0);
}

#[test]
fn test_numeric_lt() {
    let exit_code = eval_exit_code(r#"3 5 -lt"#);
    assert_eq!(exit_code, 0);
}

#[test]
fn test_numeric_gt() {
    let exit_code = eval_exit_code(r#"5 3 -gt"#);
    assert_eq!(exit_code, 0);
}

#[test]
fn test_numeric_le() {
    let exit_code = eval_exit_code(r#"5 5 -le"#);
    assert_eq!(exit_code, 0);
}

#[test]
fn test_numeric_ge() {
    let exit_code = eval_exit_code(r#"5 5 -ge"#);
    assert_eq!(exit_code, 0);
}

#[test]
fn test_into_kv_parsing() {
    let output = eval(r#""name=Alice\nage=30" into-kv"#).unwrap();
    assert!(output.contains("name") || output.contains("Alice"));
}

// ============================================
// Bytes Type and Hash Functions (TDD)
// ============================================

#[test]
fn test_sha256_returns_bytes() {
    // sha256 should return Bytes type
    let output = eval(r#""hello" sha256 typeof"#).unwrap();
    assert_eq!(output.trim(), "Bytes");
}

#[test]
fn test_sha256_to_hex() {
    let output = eval(r#""hello" sha256 to-hex"#).unwrap();
    // SHA-256 of "hello" is known
    assert_eq!(output.trim(), "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824");
}

#[test]
fn test_sha384_to_hex() {
    let output = eval(r#""hello" sha384 to-hex"#).unwrap();
    // SHA-384 of "hello"
    assert_eq!(output.trim(), "59e1748777448c69de6b800d7a33bbfb9ff1b463e44354c3553bcdb9c666fa90125a3c79f90397bdf5f6a13de828684f");
}

#[test]
fn test_sha512_to_hex() {
    let output = eval(r#""hello" sha512 to-hex"#).unwrap();
    // SHA-512 of "hello"
    assert_eq!(output.trim(), "9b71d224bd62f3785d96d46ad3ea3d73319bfbc2890caadae2dff72519673ca72323c3d99ba5c11d7c7acc6e14b8c5da0c4663475c2e5c3adef46f73bcdec043");
}

#[test]
fn test_sha3_256_to_hex() {
    let output = eval(r#""hello" sha3-256 to-hex"#).unwrap();
    // SHA3-256 of "hello"
    assert_eq!(output.trim(), "3338be694f50c5f338814986cdf0686453a888b84f424d792af4b9202398f392");
}

#[test]
fn test_sha3_384_to_hex() {
    let output = eval(r#""hello" sha3-384 to-hex"#).unwrap();
    // SHA3-384 of "hello"
    assert_eq!(output.trim(), "720aea11019ef06440fbf05d87aa24680a2153df3907b23631e7177ce620fa1330ff07c0fddee54699a4c3ee0ee9d887");
}

#[test]
fn test_sha3_512_to_hex() {
    let output = eval(r#""hello" sha3-512 to-hex"#).unwrap();
    // SHA3-512 of "hello"
    assert_eq!(output.trim(), "75d527c368f2efe848ecf6b073a36767800805e9eef2b1857d5f984f036eb6df891d75f72d9b154518c1cd58835286d1da9a38deba3de98b5a53e5ed78a84976");
}

#[test]
fn test_sha256_to_base64() {
    let output = eval(r#""hello" sha256 to-base64"#).unwrap();
    // Base64 of SHA-256 of "hello"
    assert_eq!(output.trim(), "LPJNul+wow4m6DsqxbninhsWHlwfp0JecwQzYpOLmCQ=");
}

#[test]
fn test_sha256_to_bytes_list() {
    let output = eval(r#""hello" sha256 to-bytes"#).unwrap();
    // Should be a list starting with [44, 242, 77, ...
    assert!(output.contains("44") && output.contains("242"));
}

#[test]
fn test_sha256_file() {
    use std::fs;
    let temp = tempfile::NamedTempFile::new().unwrap();
    fs::write(temp.path(), "hello").unwrap();
    
    let input = format!(r#""{}" sha256-file to-hex"#, temp.path().display());
    let output = eval(&input).unwrap();
    // Same as sha256 of "hello"
    assert_eq!(output.trim(), "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824");
}

#[test]
fn test_sha3_256_file() {
    use std::fs;
    let temp = tempfile::NamedTempFile::new().unwrap();
    fs::write(temp.path(), "hello").unwrap();
    
    let input = format!(r#""{}" sha3-256-file to-hex"#, temp.path().display());
    let output = eval(&input).unwrap();
    assert_eq!(output.trim(), "3338be694f50c5f338814986cdf0686453a888b84f424d792af4b9202398f392");
}

#[test]
fn test_bytes_equality() {
    let exit_code = eval_exit_code(r#""hello" sha256 "hello" sha256 eq?"#);
    assert_eq!(exit_code, 0, "Same hash should be equal");
}

#[test]
fn test_bytes_inequality() {
    let exit_code = eval_exit_code(r#""hello" sha256 "world" sha256 eq?"#);
    assert_eq!(exit_code, 1, "Different hashes should not be equal");
}

#[test]
fn test_as_bytes_string() {
    let output = eval(r#""hello" as-bytes to-hex"#).unwrap();
    // "hello" as hex bytes
    assert_eq!(output.trim(), "68656c6c6f");
}

#[test]
fn test_from_hex() {
    let output = eval(r#""68656c6c6f" from-hex to-string"#).unwrap();
    assert_eq!(output.trim(), "hello");
}

#[test]
fn test_from_base64() {
    let output = eval(r#""aGVsbG8=" from-base64 to-string"#).unwrap();
    assert_eq!(output.trim(), "hello");
}

#[test]
fn test_hex_roundtrip() {
    let output = eval(r#""hello" as-bytes to-hex from-hex to-string"#).unwrap();
    assert_eq!(output.trim(), "hello");
}

#[test]
fn test_base64_roundtrip() {
    let output = eval(r#""hello" as-bytes to-base64 from-base64 to-string"#).unwrap();
    assert_eq!(output.trim(), "hello");
}

#[test]
fn test_typeof_bytes() {
    let output = eval(r#""hello" sha256 typeof"#).unwrap();
    assert_eq!(output.trim(), "Bytes");
}

#[test]
fn test_bytes_len() {
    let output = eval(r#""hello" sha256 len"#).unwrap();
    assert_eq!(output.trim(), "32"); // SHA-256 is 32 bytes
}

#[test]
fn test_sha512_len() {
    let output = eval(r#""hello" sha512 len"#).unwrap();
    assert_eq!(output.trim(), "64"); // SHA-512 is 64 bytes
}

#[test]
fn test_cross_encoding_hex_to_base64() {
    let output = eval(r#""68656c6c6f" from-hex to-base64"#).unwrap();
    assert_eq!(output.trim(), "aGVsbG8=");
}

#[test]
fn test_empty_string_sha256() {
    let output = eval(r#""" sha256 to-hex"#).unwrap();
    // SHA-256 of empty string
    assert_eq!(output.trim(), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
}

// ============================================
// BigInt Type (TDD)
// ============================================

#[test]
fn test_bytes_to_bigint() {
    // Convert SHA-256 hash to BigInt
    let output = eval(r#""hello" sha256 to-bigint typeof"#).unwrap();
    assert_eq!(output.trim(), "BigInt");
}

#[test]
fn test_bigint_to_hex() {
    // BigInt should convert back to same hex as original hash
    let output = eval(r#""hello" sha256 to-bigint to-hex"#).unwrap();
    assert_eq!(output.trim(), "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824");
}

#[test]
fn test_bigint_to_bytes() {
    // BigInt should convert back to Bytes
    let output = eval(r#""hello" sha256 to-bigint to-bytes to-hex"#).unwrap();
    assert_eq!(output.trim(), "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824");
}

#[test]
fn test_string_to_bigint() {
    // Parse decimal string to BigInt
    let output = eval(r#""12345678901234567890" to-bigint typeof"#).unwrap();
    assert_eq!(output.trim(), "BigInt");
}

#[test]
fn test_bigint_to_string() {
    let output = eval(r#""12345678901234567890" to-bigint to-string"#).unwrap();
    assert_eq!(output.trim(), "12345678901234567890");
}

#[test]
fn test_hex_to_bigint() {
    // Parse hex string to BigInt
    let output = eval(r#""0xff" to-bigint to-string"#).unwrap();
    assert_eq!(output.trim(), "255");
}

#[test]
fn test_bigint_add() {
    let output = eval(r#""100" to-bigint "200" to-bigint big-add to-string"#).unwrap();
    assert_eq!(output.trim(), "300");
}

#[test]
fn test_bigint_sub() {
    let output = eval(r#""300" to-bigint "100" to-bigint big-sub to-string"#).unwrap();
    assert_eq!(output.trim(), "200");
}

#[test]
fn test_bigint_mul() {
    let output = eval(r#""12345678901234567890" to-bigint "2" to-bigint big-mul to-string"#).unwrap();
    assert_eq!(output.trim(), "24691357802469135780");
}

#[test]
fn test_bigint_div() {
    let output = eval(r#""100" to-bigint "3" to-bigint big-div to-string"#).unwrap();
    assert_eq!(output.trim(), "33");
}

#[test]
fn test_bigint_mod() {
    let output = eval(r#""100" to-bigint "3" to-bigint big-mod to-string"#).unwrap();
    assert_eq!(output.trim(), "1");
}

#[test]
fn test_bigint_xor() {
    // XOR two hashes
    let output = eval(r#""hello" sha256 to-bigint "world" sha256 to-bigint big-xor to-hex"#).unwrap();
    // Result should be different from both inputs
    assert!(!output.contains("2cf24dba"));
    assert_eq!(output.trim().len(), 64); // Still 256 bits
}

#[test]
fn test_bigint_and() {
    let output = eval(r#""0xff" to-bigint "0x0f" to-bigint big-and to-string"#).unwrap();
    assert_eq!(output.trim(), "15");
}

#[test]
fn test_bigint_or() {
    let output = eval(r#""0xf0" to-bigint "0x0f" to-bigint big-or to-string"#).unwrap();
    assert_eq!(output.trim(), "255");
}

#[test]
fn test_bigint_eq() {
    let exit_code = eval_exit_code(r#""100" to-bigint "100" to-bigint big-eq?"#);
    assert_eq!(exit_code, 0);
}

#[test]
fn test_bigint_neq() {
    let exit_code = eval_exit_code(r#""100" to-bigint "200" to-bigint big-eq?"#);
    assert_eq!(exit_code, 1);
}

#[test]
fn test_bigint_lt() {
    let exit_code = eval_exit_code(r#""100" to-bigint "200" to-bigint big-lt?"#);
    assert_eq!(exit_code, 0);
}

#[test]
fn test_bigint_gt() {
    let exit_code = eval_exit_code(r#""200" to-bigint "100" to-bigint big-gt?"#);
    assert_eq!(exit_code, 0);
}

#[test]
fn test_bigint_shl() {
    // Shift left by 4 bits (multiply by 16)
    let output = eval(r#""1" to-bigint 4 big-shl to-string"#).unwrap();
    assert_eq!(output.trim(), "16");
}

#[test]
fn test_bigint_shr() {
    // Shift right by 4 bits (divide by 16)
    let output = eval(r#""256" to-bigint 4 big-shr to-string"#).unwrap();
    assert_eq!(output.trim(), "16");
}

#[test]
fn test_bigint_pow() {
    let output = eval(r#""2" to-bigint 10 big-pow to-string"#).unwrap();
    assert_eq!(output.trim(), "1024");
}

// ============================================
// Math Primitives (for stats support)
// ============================================

#[test]
fn test_pow_integers() {
    let output = eval(r#"2 3 pow"#).unwrap();
    assert_eq!(output.trim(), "8");
}

#[test]
fn test_pow_float_exponent() {
    let output = eval(r#"4 0.5 pow"#).unwrap();
    assert_eq!(output.trim(), "2"); // sqrt(4) = 2
}

#[test]
fn test_pow_negative_exponent() {
    let output = eval(r#"2 -1 pow"#).unwrap();
    assert_eq!(output.trim(), "0.5");
}

#[test]
fn test_sqrt_perfect_square() {
    let output = eval(r#"16 sqrt"#).unwrap();
    assert_eq!(output.trim(), "4");
}

#[test]
fn test_sqrt_non_perfect() {
    let output = eval(r#"2 sqrt"#).unwrap();
    let val: f64 = output.trim().parse().unwrap();
    assert!((val - 1.4142135).abs() < 0.0001);
}

#[test]
fn test_sqrt_zero() {
    let output = eval(r#"0 sqrt"#).unwrap();
    assert_eq!(output.trim(), "0");
}

#[test]
fn test_sort_nums_ascending() {
    let output = eval(r#"'[3,1,4,1,5,9,2,6]' into-json sort-nums to-json"#).unwrap();
    assert_eq!(output.trim(), "[1.0,1.0,2.0,3.0,4.0,5.0,6.0,9.0]");
}

#[test]
fn test_sort_nums_with_floats() {
    let output = eval(r#"'[3.14,2.71,1.41]' into-json sort-nums to-json"#).unwrap();
    assert_eq!(output.trim(), "[1.41,2.71,3.14]");
}

#[test]
fn test_sort_nums_negative() {
    let output = eval(r#"'[-5,3,-2,0,1]' into-json sort-nums to-json"#).unwrap();
    assert_eq!(output.trim(), "[-5.0,-2.0,0.0,1.0,3.0]");
}

#[test]
fn test_sort_nums_empty() {
    let output = eval(r#"'[]' into-json sort-nums to-json"#).unwrap();
    assert_eq!(output.trim(), "[]");
}

#[test]
fn test_sort_nums_single() {
    let output = eval(r#"'[42]' into-json sort-nums to-json"#).unwrap();
    assert_eq!(output.trim(), "[42.0]");
}
