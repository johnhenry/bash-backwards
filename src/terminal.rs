use hsab::{display, lex, parse, Evaluator, Value};

/// Execute a single line of hsab code
pub(crate) fn execute_line(eval: &mut Evaluator, input: &str, print_output: bool) -> Result<i32, String> {
    execute_line_with_options(eval, input, print_output, true)
}

/// Execute a single line with display options
pub(crate) fn execute_line_with_options(
    eval: &mut Evaluator,
    input: &str,
    print_output: bool,
    use_format: bool,
) -> Result<i32, String> {
    let tokens = lex(input).map_err(|e| e.to_string())?;

    // Empty input is OK
    if tokens.is_empty() {
        return Ok(0);
    }

    let program = parse(tokens).map_err(|e| e.to_string())?;
    let result = eval.eval(&program).map_err(|e| e.to_string())?;

    if print_output {
        // Get terminal width for formatting
        let term_width = terminal_width();

        // Format and print each stack item
        for val in &result.stack {
            if val.as_arg().is_none() {
                continue; // Skip nil/marker
            }

            // Use pretty formatting for Tables, Records, and Errors when in REPL
            if use_format && is_structured(val) {
                println!("{}", display::format_value(val, term_width));
            } else if let Some(s) = val.as_arg() {
                println!("{}", s);
            }
        }
    }

    Ok(result.exit_code)
}

/// Check if a value is a structured type that benefits from formatting
pub(crate) fn is_structured(val: &Value) -> bool {
    matches!(
        val,
        Value::Table { .. } | Value::Map(_) | Value::Error { .. } | Value::Media { .. } | Value::Link { .. } | Value::Bytes(_) | Value::BigInt(_)
    )
}

/// Get terminal width, defaulting to 80
pub(crate) fn terminal_width() -> usize {
    terminal_size::terminal_size()
        .map(|(w, _)| w.0 as usize)
        .unwrap_or(80)
}

/// Check if triple quotes are balanced in the input
pub(crate) fn is_triple_quotes_balanced(input: &str) -> bool {
    let mut in_triple_double = false;
    let mut in_triple_single = false;
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if i + 2 < chars.len() {
            let triple: String = chars[i..i+3].iter().collect();
            if triple == "\"\"\"" && !in_triple_single {
                in_triple_double = !in_triple_double;
                i += 3;
                continue;
            }
            if triple == "'''" && !in_triple_double {
                in_triple_single = !in_triple_single;
                i += 3;
                continue;
            }
        }
        i += 1;
    }

    !in_triple_double && !in_triple_single
}
