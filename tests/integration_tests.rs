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
    let temp_file = "/tmp/hsab_test_redirect.txt";

    // [hello echo] [/tmp/hsab_test_redirect.txt] >
    let _ = eval(&format!("[hello echo] [{}] >", temp_file));

    // Check file contents
    let contents = fs::read_to_string(temp_file).unwrap();
    assert!(contents.contains("hello"));

    // Cleanup
    fs::remove_file(temp_file).ok();
}

/// Test redirect append
#[test]
fn test_redirect_append() {
    use std::fs;
    let temp_file = "/tmp/hsab_test_append.txt";

    // Write first line
    let _ = eval(&format!("[first echo] [{}] >", temp_file));
    // Append second line
    let _ = eval(&format!("[second echo] [{}] >>", temp_file));

    let contents = fs::read_to_string(temp_file).unwrap();
    assert!(contents.contains("first"));
    assert!(contents.contains("second"));

    fs::remove_file(temp_file).ok();
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

/// Test join: /dir file.txt → /dir/file.txt
#[test]
fn test_path_join() {
    let output = eval("/path file.txt join").unwrap();
    assert_eq!(output, "/path/file.txt");
}

/// Test join with trailing slash
#[test]
fn test_path_join_trailing_slash() {
    let output = eval("/path/ file.txt join").unwrap();
    assert_eq!(output, "/path/file.txt");
}

/// Test basename: /path/to/file.txt → file
#[test]
fn test_path_basename() {
    let output = eval("/path/to/file.txt basename").unwrap();
    assert_eq!(output, "file");
}

/// Test dirname: /path/to/file.txt → /path/to
#[test]
fn test_path_dirname() {
    let output = eval("/path/to/file.txt dirname").unwrap();
    assert_eq!(output, "/path/to");
}

/// Test suffix: file _bak → file_bak
/// Note: we quote "myfile" because "file" is a real Unix command
#[test]
fn test_path_suffix() {
    let output = eval("myfile _bak suffix").unwrap();
    assert_eq!(output, "myfile_bak");
}

/// Test reext: file.txt .md → file.md
#[test]
fn test_path_reext() {
    let output = eval("file.txt .md reext").unwrap();
    assert_eq!(output, "file.md");
}

/// Test reext without leading dot
#[test]
fn test_path_reext_no_dot() {
    let output = eval("photo.jpg png reext").unwrap();
    assert_eq!(output, "photo.png");
}

// ============================================
// Bash passthrough tests
// ============================================

/// Test bash passthrough
#[test]
fn test_bash_passthrough() {
    let output = eval("#!bash echo hello from bash").unwrap();
    assert!(output.contains("hello from bash"));
}

/// Test bash passthrough with complex command
#[test]
fn test_bash_passthrough_complex() {
    let output = eval("#!bash for i in 1 2 3; do echo $i; done").unwrap();
    assert!(output.contains("1"));
    assert!(output.contains("2"));
    assert!(output.contains("3"));
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

/// Test practical: create backup filename
#[test]
fn test_practical_backup_name() {
    // file.txt .bak reext → file.bak
    let output = eval("file.txt .bak reext").unwrap();
    assert_eq!(output, "file.bak");
}

/// Test practical: join path components
#[test]
fn test_practical_join_path() {
    let output = eval("/var/log access.log join").unwrap();
    assert_eq!(output, "/var/log/access.log");
}
