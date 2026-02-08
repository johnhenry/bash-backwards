//! Integration tests for hsab

use hsab::{compile_transformed, execute, execute_bash, Shell};

/// Test that hsab compiles simple pipes correctly
#[test]
fn test_simple_pipe_compilation() {
    let bash = compile_transformed("%(hello grep) ls").unwrap();
    assert_eq!(bash, "ls | grep hello");
}

/// Test chained pipes
#[test]
fn test_chained_pipes() {
    let bash = compile_transformed("%(-5 head) %(hello grep) ls").unwrap();
    assert_eq!(bash, "ls | grep hello | head -5");
}

/// Test AND operator
#[test]
fn test_and_operator() {
    let bash = compile_transformed("ls %(done echo) &&").unwrap();
    assert_eq!(bash, "ls && echo done");
}

/// Test OR operator
#[test]
fn test_or_operator() {
    let bash = compile_transformed("ls %(failed echo) ||").unwrap();
    assert_eq!(bash, "ls || echo failed");
}

/// Test redirect write (postfix: "hello echo" then redirect)
#[test]
fn test_redirect_write() {
    let bash = compile_transformed("hello echo %(out.txt) >").unwrap();
    assert_eq!(bash, "echo hello > out.txt");
}

/// Test redirect append
#[test]
fn test_redirect_append() {
    let bash = compile_transformed("hello echo %(out.txt) >>").unwrap();
    assert_eq!(bash, "echo hello >> out.txt");
}

/// Test redirect read
#[test]
fn test_redirect_read() {
    let bash = compile_transformed("cat %(input.txt) <").unwrap();
    assert_eq!(bash, "cat < input.txt");
}

/// Test background execution (postfix: "10 sleep" → "sleep 10")
#[test]
fn test_background() {
    let bash = compile_transformed("10 sleep &").unwrap();
    assert_eq!(bash, "sleep 10 &");
}

/// Test quoted strings
#[test]
fn test_quoted_strings() {
    let bash = compile_transformed("%(\"hello world\" grep) ls").unwrap();
    assert!(bash.contains("grep"));
    assert!(bash.contains("hello world"));
}

/// Test variable passthrough (postfix: "$HOME echo" → "echo $HOME")
#[test]
fn test_variable_passthrough() {
    let bash = compile_transformed("$HOME echo").unwrap();
    assert_eq!(bash, "echo $HOME");
}

/// Execution test: simple echo (postfix: "hello echo")
#[test]
fn test_execute_echo() {
    let result = execute("hello echo").unwrap();
    assert!(result.success());
    assert_eq!(result.stdout.trim(), "hello");
}

/// Execution test: compare with bash
#[test]
fn test_execution_matches_bash() {
    // Run the same logic in both hsab and bash, compare outputs
    let hsab_result = execute_bash("ls /tmp | head -3").unwrap();
    let bash_result = execute_bash("ls /tmp | head -3").unwrap();

    assert_eq!(hsab_result.stdout, bash_result.stdout);
}

/// Test command with flags in pipes
#[test]
fn test_command_with_flags() {
    let bash = compile_transformed("%(-la grep) ls").unwrap();
    assert_eq!(bash, "ls | grep -la");
}

/// Test complex chain
#[test]
fn test_complex_chain() {
    // ls | grep txt | head -5 && echo done
    let bash = compile_transformed("%(-5 head) %(txt grep) ls %(done echo) &&").unwrap();
    // This is a more complex case - the && applies to the whole pipe chain
    assert!(bash.contains("&&"));
    assert!(bash.contains("echo"));
}

/// Test empty input handling
#[test]
fn test_empty_input() {
    let result = hsab::lexer::lex("");
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

/// Test various flag formats
#[test]
fn test_flag_formats() {
    // Short flags in pipes
    let bash = compile_transformed("%(-l grep) ls").unwrap();
    assert!(bash.contains("grep -l"));

    // Commands with flags (postfix: "--color=auto ls" → "ls --color=auto")
    let bash = compile_transformed("--color=auto ls").unwrap();
    assert_eq!(bash, "ls --color=auto");
}

/// Test single command (no args)
#[test]
fn test_single_command() {
    let bash = compile_transformed("ls").unwrap();
    assert_eq!(bash, "ls");
}

/// Test multiple args (postfix order)
#[test]
fn test_multiple_args() {
    // "world hello echo" → "echo world hello"  (NOT "echo hello world")
    // Because in postfix, we read left-to-right as args, last is command
    let bash = compile_transformed("world hello echo").unwrap();
    assert_eq!(bash, "echo world hello");
}

// ============================================
// Executable-aware parsing tests (new syntax)
// ============================================

/// Test single executable with preceding args
#[test]
fn test_executable_aware_single() {
    // "-la ls" → "ls -la" (args accumulate until executable found)
    let bash = compile_transformed("-la ls").unwrap();
    assert_eq!(bash, "ls -la");
}

/// Test that multiple executables result in only the first being parsed
/// (remaining tokens are leftovers for the shell to put back on input)
#[test]
fn test_executable_aware_leftovers() {
    // "-la ls hello grep" → "ls -la" (hello grep are leftovers)
    let bash = compile_transformed("-la ls hello grep").unwrap();
    assert_eq!(bash, "ls -la");
}

/// Test that for multi-command pipes, use explicit groups
#[test]
fn test_explicit_pipe_groups() {
    // To get "cat file | sort -r | head -5", use groups:
    let bash = compile_transformed("%(-5 head) %(-r sort) file.txt cat").unwrap();
    assert_eq!(bash, "cat file.txt | sort -r | head -5");
}

/// Test group-based syntax still works (backward compat)
#[test]
fn test_group_still_works() {
    // "%(hello grep) ls" → "ls | grep hello" (same as before)
    let bash = compile_transformed("%(hello grep) ls").unwrap();
    assert_eq!(bash, "ls | grep hello");
}

/// Test fallback when no executable found
#[test]
fn test_fallback_no_executable() {
    // "foo bar baz" - none are executables, treat last as command
    let bash = compile_transformed("foo bar baz").unwrap();
    assert_eq!(bash, "baz foo bar");
}

/// Test mixed: groups work with executable detection
#[test]
fn test_mixed_group_and_executable() {
    // Groups still work for explicit postfix operators like &&
    // "ls %(done echo) &&" → "ls && echo done"
    let bash = compile_transformed("ls %(done echo) &&").unwrap();
    assert_eq!(bash, "ls && echo done");

    // Pipe groups combined with executable detection
    // "%(hello grep) -la ls" → "ls -la | grep hello"
    let bash = compile_transformed("%(hello grep) -la ls").unwrap();
    assert_eq!(bash, "ls -la | grep hello");
}

/// Test executable detection - trailing args become leftovers
#[test]
fn test_executable_trailing_args_leftovers() {
    // "ls -la /tmp" - ls detected, -la and /tmp are LEFTOVERS (not args)
    let bash = compile_transformed("ls -la /tmp").unwrap();
    assert_eq!(bash, "ls");

    // To get "ls -la /tmp", use postfix: args before command
    let bash = compile_transformed("-la /tmp ls").unwrap();
    assert_eq!(bash, "ls -la /tmp");
}

/// Test common workflow: git commands
#[test]
fn test_git_workflow() {
    // "status git" → "git status"
    let bash = compile_transformed("status git").unwrap();
    assert_eq!(bash, "git status");
}

/// Test execution of executable-aware syntax
#[test]
fn test_execute_executable_aware() {
    // "-la ls" should execute and produce output
    let result = execute("-la ls").unwrap();
    assert!(result.success());
    // Should show file listing with details
    assert!(result.stdout.contains("Cargo"));
}

/// Test execution with explicit pipe groups
#[test]
fn test_execute_piped_with_groups() {
    // Use explicit groups for pipes: "%(Cargo grep) ls" → "ls | grep Cargo"
    let result = execute("%(Cargo grep) ls").unwrap();
    assert!(result.success());
    assert!(result.stdout.contains("Cargo"));
}

// ============================================
// Edge case tests for executable-aware parsing
// ============================================

/// Edge case: Two consecutive executables - only first is parsed
#[test]
fn test_edge_consecutive_executables() {
    // Two executables - only first is parsed, second is leftover
    let bash = compile_transformed("echo cat").unwrap();
    assert_eq!(bash, "echo");

    let bash = compile_transformed("ls cat").unwrap();
    assert_eq!(bash, "ls");

    // For pipes, use explicit groups
    let bash = compile_transformed("%(cat) echo").unwrap();
    assert_eq!(bash, "echo | cat");
}

/// Edge case: To echo a literal executable name, must quote it
#[test]
fn test_edge_quote_executable_names() {
    // Without quotes: only first exec parsed (ls), echo is leftover
    let bash = compile_transformed("ls echo").unwrap();
    assert_eq!(bash, "ls");

    // With quotes: "ls" is just a string argument, echo is the command
    let bash = compile_transformed("\"ls\" echo").unwrap();
    assert_eq!(bash, "echo \"ls\"");

    // Single quotes work too
    let bash = compile_transformed("'grep' echo").unwrap();
    assert_eq!(bash, "echo 'grep'");
}

/// Edge case: Postfix args vs traditional order
#[test]
fn test_edge_arg_order_difference() {
    // Postfix: args before command → args are consumed
    let bash1 = compile_transformed("-la ls").unwrap();
    assert_eq!(bash1, "ls -la");

    // Traditional order: command first → parsing stops, trailing becomes leftovers
    let bash2 = compile_transformed("ls -la").unwrap();
    assert_eq!(bash2, "ls"); // -la is leftover, not consumed

    // With multiple args in postfix
    let bash = compile_transformed("-la /tmp ls").unwrap();
    assert_eq!(bash, "ls -la /tmp");
}

/// Edge case: Only first executable is parsed, rest are leftovers
#[test]
fn test_edge_first_executable_only() {
    // "cat myfile grep" - cat is detected, parsing stops
    // 'myfile' and 'grep' are leftovers
    let bash = compile_transformed("cat myfile grep").unwrap();
    assert_eq!(bash, "cat");

    // To get piped commands, use explicit groups:
    let bash = compile_transformed("%(myfile grep) cat").unwrap();
    assert_eq!(bash, "cat | grep myfile");
}

/// Edge case: Trailing tokens after executable are leftovers
#[test]
fn test_edge_trailing_tokens_leftovers() {
    // "cat grep pattern" - cat is detected, parsing stops
    // 'grep' and 'pattern' are leftovers
    let bash = compile_transformed("cat grep pattern").unwrap();
    assert_eq!(bash, "cat");

    // To get cat | grep pattern, use groups:
    let bash = compile_transformed("%(pattern grep) cat").unwrap();
    assert_eq!(bash, "cat | grep pattern");
}

/// Edge case: Complex pipes require explicit groups
#[test]
fn test_edge_complex_requires_groups() {
    // For complex pipes, use explicit groups:
    // "file.txt cat" → "cat file.txt"
    let bash = compile_transformed("file.txt cat").unwrap();
    assert_eq!(bash, "cat file.txt");

    // Full pipe chain with groups:
    let bash = compile_transformed("%(-5 head) %(pattern grep) file.txt cat").unwrap();
    assert_eq!(bash, "cat file.txt | grep pattern | head -5");
}

/// Edge case: Common user mistake - tokens after executable are leftovers
#[test]
fn test_edge_common_mistake_leftovers() {
    // User writes "cat myfile grep pattern"
    // Only "cat" is parsed, rest are leftovers
    let bash = compile_transformed("cat myfile grep pattern").unwrap();
    assert_eq!(bash, "cat");

    // The correct way to write "cat myfile | grep pattern":
    let bash = compile_transformed("%(pattern grep) myfile cat").unwrap();
    assert_eq!(bash, "cat myfile | grep pattern");
}

/// Edge case: Flags are never treated as executables
#[test]
fn test_edge_flags_not_executables() {
    // Even if a flag matches an executable name pattern, it's still a flag
    let bash = compile_transformed("-ls echo").unwrap();
    assert_eq!(bash, "echo -ls");

    let bash = compile_transformed("--grep ls").unwrap();
    assert_eq!(bash, "ls --grep");
}

/// Edge case: Paths are never treated as executables
#[test]
fn test_edge_paths_not_executables() {
    // Absolute path
    let bash = compile_transformed("/bin/ls cat").unwrap();
    assert_eq!(bash, "cat /bin/ls");

    // Relative path
    let bash = compile_transformed("./script.sh cat").unwrap();
    assert_eq!(bash, "cat ./script.sh");

    // Path with directory
    let bash = compile_transformed("src/main.rs cat").unwrap();
    assert_eq!(bash, "cat src/main.rs");
}

/// Edge case: Variables are never treated as executables
#[test]
fn test_edge_variables_not_executables() {
    let bash = compile_transformed("$FILE cat").unwrap();
    assert_eq!(bash, "cat $FILE");

    let bash = compile_transformed("${HOME} echo").unwrap();
    assert_eq!(bash, "echo ${HOME}");
}

/// Edge case: Unknown words trigger fallback (last word = command)
#[test]
fn test_edge_fallback_unknown_words() {
    // None of these are executables, so fallback: last word is command
    let bash = compile_transformed("arg1 arg2 mycommand").unwrap();
    assert_eq!(bash, "mycommand arg1 arg2");

    // Single unknown word
    let bash = compile_transformed("unknowncmd").unwrap();
    assert_eq!(bash, "unknowncmd");
}

/// Edge case: Mixed known and unknown - known is detected, rest are leftovers
#[test]
fn test_edge_mixed_known_unknown() {
    // 'ls' is known, 'foo' is unknown
    // foo accumulates, ls takes it
    let bash = compile_transformed("foo ls").unwrap();
    assert_eq!(bash, "ls foo");

    // 'ls' is detected first, 'unknowncmd' becomes leftover
    let bash = compile_transformed("ls unknowncmd").unwrap();
    assert_eq!(bash, "ls");
}

/// Edge case: Groups create explicit pipes
#[test]
fn test_edge_groups_create_pipes() {
    // Groups always create postfix command structure
    let bash = compile_transformed("%(cat echo) ls").unwrap();
    assert_eq!(bash, "ls | echo cat");

    // Groups can come after executable args
    let bash = compile_transformed("%(pattern grep) -la ls").unwrap();
    assert_eq!(bash, "ls -la | grep pattern");
}

/// Execution test: only first executable runs, rest are leftovers
#[test]
fn test_execute_first_executable_only() {
    // "hello echo cat" - echo is first executable, "hello" is its arg
    // "cat" is leftover (not executed)
    let result = execute("hello echo cat").unwrap();
    assert!(result.success());
    assert_eq!(result.stdout.trim(), "hello");

    // To get piped execution, use explicit groups:
    let result = execute("%(cat) hello echo").unwrap();
    assert!(result.success());
    assert_eq!(result.stdout.trim(), "hello");
}

/// Execution test: quoted executable name is treated as argument
#[test]
fn test_execute_quoted_executable() {
    let result = execute("\"ls\" echo").unwrap();
    assert!(result.success());
    assert_eq!(result.stdout.trim(), "ls");
}

// ============================================
// Shell state and %variable tests
// ============================================

/// Test Shell state tracks last argument
#[test]
fn test_shell_state_last_arg() {
    let mut shell = Shell::new();

    // Execute a command with args
    let _ = shell.execute("hello world echo").unwrap();

    // %_ should be the last arg (world, since hello comes before it)
    assert_eq!(shell.state.last_arg, "world");
}

/// Test Shell state tracks exit code
#[test]
fn test_shell_state_exit_code() {
    let mut shell = Shell::new();

    // Successful command
    let _ = shell.execute("true").unwrap();
    assert_eq!(shell.state.last_exit_code, 0);

    // Failed command
    let _ = shell.execute("false").unwrap();
    assert_eq!(shell.state.last_exit_code, 1);
}

/// Test Shell state tracks stdout
#[test]
fn test_shell_state_stdout() {
    let mut shell = Shell::new();

    let _ = shell.execute("hello echo").unwrap();
    assert_eq!(shell.state.last_stdout.trim(), "hello");
}

/// Test Shell state line indexing
#[test]
fn test_shell_state_line_indexing() {
    let mut shell = Shell::new();

    // Multi-line output
    let _ = shell.execute("#!bash echo -e 'first\\nsecond\\nthird'").unwrap();

    assert_eq!(shell.state.get_line(0), "first");
    assert_eq!(shell.state.get_line(1), "second");
    assert_eq!(shell.state.get_line(2), "third");
    assert_eq!(shell.state.get_line(99), ""); // Out of bounds
}

/// Test %_ variable expansion
#[test]
fn test_percent_underscore_expansion() {
    let mut shell = Shell::new();

    // First command sets up state
    let _ = shell.execute("myfile.txt echo").unwrap();

    // Second command uses %_
    let bash = shell.compile("%_ cat").unwrap();
    assert_eq!(bash, "cat myfile.txt");
}

/// Test %! variable expansion (stdout)
#[test]
fn test_percent_bang_expansion() {
    let mut shell = Shell::new();

    // First command produces output
    let _ = shell.execute("hello echo").unwrap();

    // Second command uses %!
    let bash = shell.compile("%! cat").unwrap();
    assert_eq!(bash, "cat hello");
}

/// Test %? variable expansion (exit code)
#[test]
fn test_percent_question_expansion() {
    let mut shell = Shell::new();

    let _ = shell.execute("false").unwrap();

    let bash = shell.compile("%? echo").unwrap();
    assert_eq!(bash, "echo 1");
}

/// Test %N variable expansion (line indexing)
#[test]
fn test_percent_number_expansion() {
    let mut shell = Shell::new();

    let _ = shell.execute("#!bash echo -e 'alpha\\nbeta\\ngamma'").unwrap();

    let bash = shell.compile("%0 echo").unwrap();
    assert_eq!(bash, "echo alpha");

    let bash = shell.compile("%1 echo").unwrap();
    assert_eq!(bash, "echo beta");

    let bash = shell.compile("%2 echo").unwrap();
    assert_eq!(bash, "echo gamma");
}

/// Test %cmd variable expansion
#[test]
fn test_percent_cmd_expansion() {
    let mut shell = Shell::new();

    let _ = shell.execute("-la ls").unwrap();

    // %cmd should contain the generated bash
    assert!(shell.state.last_bash_cmd.contains("ls"));
    assert!(shell.state.last_bash_cmd.contains("-la"));
}

/// Test chained %variable usage
#[test]
fn test_chained_percent_vars() {
    let mut shell = Shell::new();

    // List files
    let _ = shell.execute("ls").unwrap();

    // Get first line (should be a file/directory name)
    let first_file = shell.state.get_line(0).to_string();
    assert!(!first_file.is_empty());

    // Use %0 in next command
    let bash = shell.compile("%0 echo").unwrap();
    assert_eq!(bash, format!("echo {}", first_file));
}
