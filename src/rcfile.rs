use hsab::Evaluator;
use crate::terminal::execute_line;
use std::env;
use std::fs;

/// Embedded stdlib content (compiled into binary)
pub(crate) const STDLIB_CONTENT: &str = include_str!("../examples/stdlib.hsabrc");

/// Get home directory
pub(crate) fn dirs_home() -> Option<std::path::PathBuf> {
    env::var_os("HOME").map(std::path::PathBuf::from)
}

/// Get the stdlib path (~/.hsab/lib/stdlib.hsabrc)
fn stdlib_path() -> Option<std::path::PathBuf> {
    dirs_home().map(|h| h.join(".hsab").join("lib").join("stdlib.hsabrc"))
}

/// Load and execute ~/.hsabrc if it exists
pub(crate) fn load_hsabrc(eval: &mut Evaluator) {
    let rc_path = match dirs_home() {
        Some(home) => home.join(".hsabrc"),
        None => return,
    };

    let content = match fs::read_to_string(&rc_path) {
        Ok(c) => c,
        Err(_) => return,
    };

    load_rc_content(eval, &content, "~/.hsabrc");
}

/// Load and execute ~/.hsab_profile if it exists (for login shells)
pub(crate) fn load_hsab_profile(eval: &mut Evaluator) {
    // Profile search paths in order of priority
    let profile_paths = [
        dirs_home().map(|h| h.join(".hsab_profile")),
        dirs_home().map(|h| h.join(".profile")),
    ];

    for path in profile_paths.iter().flatten() {
        if path.exists() {
            if let Ok(content) = fs::read_to_string(path) {
                for (line_num, line) in content.lines().enumerate() {
                    let trimmed = line.trim();

                    // Skip empty lines and comments
                    if trimmed.is_empty() || trimmed.starts_with('#') {
                        continue;
                    }

                    if let Err(e) = execute_line(eval, trimmed, false) {
                        eprintln!("Warning: {} line {}: {}", path.display(), line_num + 1, e);
                    }

                    // Clear the stack after each line in profile
                    eval.clear_stack();
                }
            }
            break; // Only source first found profile
        }
    }

    // Set LOGIN_SHELL environment variable
    std::env::set_var("LOGIN_SHELL", "1");
}

/// Load stdlib from ~/.hsab/lib/stdlib.hsabrc if it exists
pub(crate) fn load_stdlib(eval: &mut Evaluator) {
    let path = match stdlib_path() {
        Some(p) => p,
        None => return,
    };

    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return, // Silently skip if not installed
    };

    load_rc_content(eval, &content, "stdlib");
}

/// Load RC file content, handling multiline blocks
fn load_rc_content(eval: &mut Evaluator, content: &str, source: &str) {
    let mut buffer = String::new();
    let mut bracket_depth: i32 = 0;
    let mut start_line = 1;

    for (line_num, line) in content.lines().enumerate() {
        let trimmed = line.trim();

        // Skip empty lines and comment-only lines when not in a multiline block
        if bracket_depth == 0 && (trimmed.is_empty() || trimmed.starts_with('#')) {
            continue;
        }

        // Strip inline comments (but not inside quotes - simplified check)
        let code = if let Some(pos) = trimmed.find('#') {
            // Only strip if # is not inside quotes (very simplified)
            let before_hash = &trimmed[..pos];
            let quote_count = before_hash.matches('"').count() + before_hash.matches('\'').count();
            if quote_count % 2 == 0 {
                before_hash.trim()
            } else {
                trimmed
            }
        } else {
            trimmed
        };

        if code.is_empty() {
            continue;
        }

        // Track bracket depth
        for ch in code.chars() {
            match ch {
                '[' => bracket_depth += 1,
                ']' => bracket_depth = bracket_depth.saturating_sub(1),
                _ => {}
            }
        }

        // Accumulate into buffer
        if buffer.is_empty() {
            start_line = line_num + 1;
            buffer = code.to_string();
        } else {
            buffer.push(' ');
            buffer.push_str(code);
        }

        // Execute when brackets are balanced
        if bracket_depth == 0 && !buffer.is_empty() {
            if let Err(e) = execute_line(eval, &buffer, true) {
                eprintln!("Warning: {} line {}: {}", source, start_line, e);
            }
            eval.clear_stack();
            buffer.clear();
        }
    }

    // Handle any remaining content (shouldn't happen with valid files)
    if !buffer.is_empty() {
        if let Err(e) = execute_line(eval, &buffer, true) {
            eprintln!("Warning: {} line {}: {}", source, start_line, e);
        }
        eval.clear_stack();
    }
}
