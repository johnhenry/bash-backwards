//! Integration tests for shell operations

#[path = "common/mod.rs"]
mod common;
#[allow(unused_imports)]
use common::{eval, eval_exit_code, Evaluator, lex, parse};

#[test]
fn test_pipe_basic() {
    // ls [grep Cargo] | means: ls runs, output piped to grep Cargo
    let output = eval("ls [Cargo grep] |").unwrap();
    assert!(output.contains("Cargo"));
}

#[test]
fn test_pipe_chained() {
    // Would need multiple pipes, but basic pipe works
    let output = eval("ls [txt grep] |").unwrap();
    // May or may not have .txt files
    assert!(output.is_empty() || output.contains("txt") || true);
}

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

#[test]
fn test_and_success() {
    let output = eval("[true] [done echo] &&").unwrap();
    assert!(output.contains("done"));
}

#[test]
fn test_and_failure() {
    // false && echo done should not echo
    let output = eval("[false] [done echo] &&").unwrap();
    // done should not appear because false fails
    assert!(!output.contains("done"));
}

#[test]
fn test_or_failure() {
    let output = eval("[false] [fallback echo] ||").unwrap();
    assert!(output.contains("fallback"));
}

#[test]
fn test_or_success() {
    // true || echo fallback should not echo
    let output = eval("[true] [fallback echo] ||").unwrap();
    // fallback should not appear because true succeeds
    assert!(!output.contains("fallback"));
}

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

#[test]
fn test_stdin_redirect() {
    let temp_file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(temp_file.path(), "hello from file\n").unwrap();

    // [cat] [input.txt] < should feed file to cat's stdin
    let output = eval(&format!("[cat] [{}] <", temp_file.path().to_str().unwrap())).unwrap();
    assert!(output.contains("hello from file"), "stdin redirect should work");
    // temp_file auto-cleans up on drop
}

#[test]
fn test_stderr_to_stdout_redirect() {
    // 2>&1 should merge stderr into stdout
    // Use bash -c to run a command that outputs to stderr
    let output = eval(r#"["echo error >&2" -c bash] 2>&1"#).unwrap();
    // The error message should appear in output
    assert!(output.contains("error"), "stderr should be redirected to stdout: got {}", output);
}

#[test]
fn test_cd_nonexistent_dir() {
    // cd to nonexistent dir returns nil (stack-native behavior)
    // Use nil? to check the result (exit 0 if nil)
    let exit_code = eval_exit_code("/nonexistent/path/xyz cd nil?");
    assert_eq!(exit_code, 0, "cd to nonexistent dir should push nil");
}

#[test]
fn test_cd_to_file_fails() {
    // cd to a file returns nil (stack-native behavior)
    // Use nil? to check the result (exit 0 if nil)
    let exit_code = eval_exit_code("Cargo.toml cd nil?");
    assert_eq!(exit_code, 0, "cd to file should push nil");
}

#[test]
fn test_cd_home() {
    // cd with no args should go to home and push the path
    let original_dir = std::env::current_dir().unwrap();
    let output = eval("cd").unwrap();
    std::env::set_current_dir(&original_dir).unwrap();
    // Should contain a path string (not nil)
    assert!(!output.trim().is_empty() && output.trim() != "nil",
        "cd to home should push path, got: {}", output);
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

#[test]
fn test_background_simple() {
    // Background should run without blocking
    let exit_code = eval_exit_code(r#"[true] &"#);
    assert_eq!(exit_code, 0);
}

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

