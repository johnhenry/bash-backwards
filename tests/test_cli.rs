//! CLI integration tests (issue #34): drive the hsab binary with assert_cmd.

use assert_cmd::Command;
use predicates::prelude::*;

fn hsab() -> Command {
    // cargo_bin is deprecated in favor of a macro that requires newer
    // assert_cmd; it works fine for the standard target dir used in CI.
    #[allow(deprecated)]
    Command::cargo_bin("hsab").expect("hsab binary should build")
}

// === hsab -c '<program>' ===

#[test]
fn test_dash_c_arithmetic() {
    hsab()
        .args(["-c", "5 3 plus"])
        .assert()
        .success()
        .stdout(predicate::str::contains("8"));
}

#[test]
fn test_dash_c_echo() {
    hsab()
        .args(["-c", "hello echo"])
        .assert()
        .success()
        .stdout(predicate::str::contains("hello"));
}

#[test]
fn test_dash_c_error_exit_code() {
    // A type error must produce a failure exit code and an error message
    hsab()
        .args(["-c", "\"not-a-record\" \"key\" get"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Error"));
}

// === --version / --help ===

#[test]
fn test_version_flag() {
    hsab()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("hsab-"));
}

#[test]
fn test_help_flag() {
    hsab()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("USAGE"))
        .stdout(predicate::str::contains("hsab -c <command>"));
}

// === script files ===

#[test]
fn test_script_file_basic() {
    let dir = tempfile::tempdir().expect("tempdir");
    let script = dir.path().join("test.hsab");
    std::fs::write(&script, "hello echo\n").expect("write script");

    hsab()
        .arg(script.to_str().expect("utf8 path"))
        .assert()
        .success()
        .stdout(predicate::str::contains("hello"));
}

#[test]
fn test_script_skips_comments_but_runs_blocks() {
    // `#` starts a comment line, but `#[` is block syntax and must execute
    let dir = tempfile::tempdir().expect("tempdir");
    let script = dir.path().join("test.hsab");
    std::fs::write(
        &script,
        "# this is a comment\nhello echo\n#[world echo] apply\n",
    )
    .expect("write script");

    hsab()
        .arg(script.to_str().expect("utf8 path"))
        .assert()
        .success()
        .stdout(predicate::str::contains("hello"))
        .stdout(predicate::str::contains("world"))
        .stdout(predicate::str::contains("comment").not());
}

#[test]
fn test_script_is_line_oriented_multiline_blocks_error() {
    // Scripts execute line by line: a block spanning lines is a lex error
    // reported with the line number, not silently skipped.
    let dir = tempfile::tempdir().expect("tempdir");
    let script = dir.path().join("test.hsab");
    std::fs::write(&script, "#[\nmultiline echo\n] apply\n").expect("write script");

    hsab()
        .arg(script.to_str().expect("utf8 path"))
        .assert()
        .failure()
        .stderr(predicate::str::contains("line 1"));
}

#[test]
fn test_script_missing_file() {
    hsab()
        .arg("/nonexistent/script.hsab")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Error reading"));
}

#[test]
fn test_script_stops_on_failing_line() {
    let dir = tempfile::tempdir().expect("tempdir");
    let script = dir.path().join("test.hsab");
    // Line 2 raises a type error; line 3 must not run
    std::fs::write(&script, "one echo\n\"x\" \"k\" get\nnever echo\n").expect("write script");

    hsab()
        .arg(script.to_str().expect("utf8 path"))
        .assert()
        .failure()
        .stdout(predicate::str::contains("one"))
        .stdout(predicate::str::contains("never").not())
        .stderr(predicate::str::contains("line 2"));
}

// === hsab init ===

#[test]
fn test_init_installs_stdlib_into_home() {
    let home = tempfile::tempdir().expect("tempdir");

    hsab()
        .env("HOME", home.path())
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::contains("Installed stdlib"));

    let stdlib = home.path().join(".hsab/lib/stdlib.hsabrc");
    assert!(stdlib.is_file(), "init should write {}", stdlib.display());
    let content = std::fs::read_to_string(&stdlib).expect("read stdlib");
    assert!(!content.is_empty(), "stdlib should not be empty");
}

#[test]
fn test_init_refuses_to_overwrite() {
    let home = tempfile::tempdir().expect("tempdir");
    let lib_dir = home.path().join(".hsab/lib");
    std::fs::create_dir_all(&lib_dir).expect("mkdir");
    let stdlib = lib_dir.join("stdlib.hsabrc");
    std::fs::write(&stdlib, "# my customized stdlib\n").expect("write");

    hsab()
        .env("HOME", home.path())
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::contains("already installed"));

    let content = std::fs::read_to_string(&stdlib).expect("read stdlib");
    assert_eq!(
        content, "# my customized stdlib\n",
        "init must not overwrite an existing stdlib"
    );
}

// === REPL smoke tests (piped stdin) ===

#[test]
fn test_repl_smoke_echo_and_exit() {
    hsab()
        .write_stdin("hello echo\n.exit\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("hello"));
}

#[test]
fn test_repl_smoke_stack_and_clear() {
    hsab()
        .write_stdin("5 3 plus peek\n.clear\n.exit\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("8"));
}

#[test]
fn test_repl_smoke_eof_exits_cleanly() {
    // Ctrl-D / EOF on stdin should exit without error
    hsab().write_stdin("hello echo\n").assert().success();
}
