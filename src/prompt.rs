use hsab::{Evaluator, Value};
use crate::terminal::execute_line;

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Set prompt context variables for PS1/PS2/STACK_HINT functions
pub(crate) fn set_prompt_context(eval: &Evaluator, cmd_num: usize) {
    // Version info
    let version_parts: Vec<&str> = VERSION.split('.').collect();
    std::env::set_var("_VERSION", VERSION);
    std::env::set_var("_VERSION_MAJOR", version_parts.get(0).unwrap_or(&"0"));
    std::env::set_var("_VERSION_MINOR", version_parts.get(1).unwrap_or(&"0"));
    std::env::set_var("_VERSION_PATCH", version_parts.get(2).unwrap_or(&"0"));

    // Shell state
    let depth = eval.stack().iter().filter(|v| v.as_arg().is_some()).count();
    std::env::set_var("_DEPTH", depth.to_string());
    std::env::set_var("_EXIT", eval.last_exit_code().to_string());
    std::env::set_var("_JOBS", eval.job_count().to_string());
    std::env::set_var("_LIMBO", eval.limbo_count().to_string());
    std::env::set_var("_CMD_NUM", cmd_num.to_string());
    std::env::set_var("_SHLVL", std::env::var("SHLVL").unwrap_or_else(|_| "1".to_string()));

    // Environment
    std::env::set_var("_CWD", eval.cwd().display().to_string());
    std::env::set_var("_USER", std::env::var("USER").unwrap_or_else(|_| "".to_string()));
    std::env::set_var("_HOST", hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "".to_string()));

    // Time
    let now = chrono::Local::now();
    std::env::set_var("_TIME", now.format("%H:%M:%S").to_string());
    std::env::set_var("_DATE", now.format("%Y-%m-%d").to_string());

    // Git info (only if in a git repo)
    let git_branch = std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    std::env::set_var("_GIT_BRANCH", &git_branch);

    if !git_branch.is_empty() {
        let git_dirty = std::process::Command::new("git")
            .args(["status", "--porcelain"])
            .output()
            .ok()
            .map(|o| if o.stdout.is_empty() { "0" } else { "1" })
            .unwrap_or("0");
        std::env::set_var("_GIT_DIRTY", git_dirty);

        let git_repo = std::process::Command::new("git")
            .args(["rev-parse", "--show-toplevel"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| {
                let path = String::from_utf8_lossy(&o.stdout).trim().to_string();
                std::path::Path::new(&path)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default()
            })
            .unwrap_or_default();
        std::env::set_var("_GIT_REPO", git_repo);
    } else {
        std::env::set_var("_GIT_DIRTY", "0");
        std::env::set_var("_GIT_REPO", "");
    }
}

/// Evaluate a prompt definition (PS1, PS2, STACK_HINT) and return the output string
pub(crate) fn eval_prompt_definition(eval: &mut Evaluator, name: &str) -> Option<String> {
    if !eval.has_definition(name) {
        return None;
    }

    // Save current stack and exit code (predicates in PS1 shouldn't affect user's exit code)
    let saved_stack = eval.stack().to_vec();
    let saved_exit_code = eval.last_exit_code();

    // Clear stack for prompt evaluation
    eval.clear_stack();

    // Execute the definition
    let result = execute_line(eval, name, false);

    // Get the output from stack
    let prompt = if result.is_ok() {
        eval.stack()
            .iter()
            .filter_map(|v| v.as_arg())
            .collect::<Vec<_>>()
            .join("")
    } else {
        // On error, return None to use default
        eval.restore_stack(saved_stack);
        eval.set_last_exit_code(saved_exit_code);
        return None;
    };

    // Restore stack and exit code
    eval.restore_stack(saved_stack);
    eval.set_last_exit_code(saved_exit_code);

    if prompt.is_empty() {
        None
    } else {
        Some(prompt)
    }
}

/// Extract hint format (prefix, separator, suffix) from STACK_HINT definition
/// Calls STACK_HINT with test input "A\nB" (two items) and parses the result
pub(crate) fn extract_hint_format(eval: &mut Evaluator) -> (String, String, String) {
    let default = ("".to_string(), " ".to_string(), "".to_string());

    if !eval.has_definition("STACK_HINT") {
        return default;
    }

    // Save current stack and exit code
    let saved_stack = eval.stack().to_vec();
    let saved_exit_code = eval.last_exit_code();

    // Clear and push test input: two items separated by newline
    eval.clear_stack();
    eval.push_value(Value::Literal("A\nB".to_string()));

    // Execute STACK_HINT
    if execute_line(eval, "STACK_HINT", false).is_ok() {
        if let Some(result) = eval.stack().last().and_then(|v| v.as_arg()) {
            // Parse result to extract prefix, separator, and suffix
            // Expected output like " [A, B]" -> prefix=" [", sep=", ", suffix="]"
            if let Some(pos_a) = result.find('A') {
                if let Some(pos_b) = result.find('B') {
                    // Trim leading newlines from prefix and trailing newlines from suffix
                    // to avoid double newlines in hint display
                    let prefix = result[..pos_a].trim_start_matches('\n').to_string();
                    let separator = result[pos_a + 1..pos_b].to_string();
                    let suffix = result[pos_b + 1..].trim_end_matches('\n').to_string();
                    eval.restore_stack(saved_stack);
                    eval.set_last_exit_code(saved_exit_code);
                    return (prefix, separator, suffix);
                }
            }
        }
    }

    // Restore stack and exit code on failure
    eval.restore_stack(saved_stack);
    eval.set_last_exit_code(saved_exit_code);
    default
}
