//! Display formatting for structured data types
//!
//! Provides pretty-printing for Tables, Records, and Lists
//! when displayed in the terminal.

use crate::ast::Value;
use std::collections::HashMap;

/// Format a value for terminal display
pub fn format_value(val: &Value, max_width: usize) -> String {
    match val {
        Value::Table { columns, rows } => format_table(columns, rows, max_width),
        Value::Map(map) => format_record(map, max_width),
        Value::List(items) => format_list(items, max_width),
        Value::Error { kind, message, code, .. } => {
            let code_str = code.map(|c| format!(" (exit {})", c)).unwrap_or_default();
            format!("\x1b[31mError[{}]\x1b[0m: {}{}", kind, message, code_str)
        }
        _ => val.as_arg().unwrap_or_default(),
    }
}

/// Format a table with box-drawing characters
fn format_table(columns: &[String], rows: &[Vec<Value>], max_width: usize) -> String {
    if columns.is_empty() {
        return "(empty table)".to_string();
    }

    // Calculate column widths
    let mut widths: Vec<usize> = columns.iter().map(|c| c.len()).collect();
    for row in rows {
        for (i, val) in row.iter().enumerate() {
            if let Some(w) = widths.get_mut(i) {
                let val_width = val.as_arg().unwrap_or_default().len();
                *w = (*w).max(val_width);
            }
        }
    }

    // Cap individual column widths
    let max_col_width = 40;
    for w in &mut widths {
        *w = (*w).min(max_col_width);
    }

    // Scale down if total exceeds max_width
    let total: usize = widths.iter().sum::<usize>() + (widths.len() * 3) + 1;
    if total > max_width && max_width > 0 {
        let scale = (max_width - widths.len() * 3 - 1) as f64 / widths.iter().sum::<usize>() as f64;
        for w in &mut widths {
            *w = ((*w as f64 * scale) as usize).max(3);
        }
    }

    let mut out = String::new();

    // Top border
    out.push_str("\x1b[90m┌");
    for (i, w) in widths.iter().enumerate() {
        out.push_str(&"─".repeat(*w + 2));
        if i < widths.len() - 1 {
            out.push('┬');
        }
    }
    out.push_str("┐\x1b[0m\n");

    // Header row
    out.push_str("\x1b[90m│\x1b[0m");
    for (i, col) in columns.iter().enumerate() {
        let w = widths.get(i).copied().unwrap_or(10);
        let truncated = truncate_str(col, w);
        out.push_str(&format!(" \x1b[1m{:width$}\x1b[0m \x1b[90m│\x1b[0m", truncated, width = w));
    }
    out.push('\n');

    // Header separator
    out.push_str("\x1b[90m├");
    for (i, w) in widths.iter().enumerate() {
        out.push_str(&"─".repeat(*w + 2));
        if i < widths.len() - 1 {
            out.push('┼');
        }
    }
    out.push_str("┤\x1b[0m\n");

    // Data rows
    for row in rows {
        out.push_str("\x1b[90m│\x1b[0m");
        for (i, val) in row.iter().enumerate() {
            let w = widths.get(i).copied().unwrap_or(10);
            let s = val.as_arg().unwrap_or_default();
            let truncated = truncate_str(&s, w);
            out.push_str(&format!(" {:width$} \x1b[90m│\x1b[0m", truncated, width = w));
        }
        out.push('\n');
    }

    // Bottom border
    out.push_str("\x1b[90m└");
    for (i, w) in widths.iter().enumerate() {
        out.push_str(&"─".repeat(*w + 2));
        if i < widths.len() - 1 {
            out.push('┴');
        }
    }
    out.push_str("┘\x1b[0m");

    // Row count
    out.push_str(&format!("\n\x1b[90m({} rows)\x1b[0m", rows.len()));

    out
}

/// Format a record (map) with aligned key-value pairs
fn format_record(map: &HashMap<String, Value>, _max_width: usize) -> String {
    if map.is_empty() {
        return "{}".to_string();
    }

    let max_key_len = map.keys().map(|k| k.len()).max().unwrap_or(0);
    let mut out = String::from("\x1b[90m{\x1b[0m\n");

    let mut keys: Vec<_> = map.keys().collect();
    keys.sort();

    for key in keys {
        let val = map.get(key).unwrap();
        out.push_str(&format!(
            "  \x1b[36m{:width$}\x1b[0m: {}\n",
            key,
            format_value_inline(val),
            width = max_key_len
        ));
    }
    out.push_str("\x1b[90m}\x1b[0m");
    out
}

/// Format a list
fn format_list(items: &[Value], _max_width: usize) -> String {
    if items.is_empty() {
        return "[]".to_string();
    }

    if items.len() <= 10 {
        let parts: Vec<String> = items.iter()
            .map(format_value_inline)
            .collect();
        format!("\x1b[90m[\x1b[0m{}...\x1b[90m]\x1b[0m", parts.join(", "))
    } else {
        let first: Vec<String> = items.iter().take(5)
            .map(format_value_inline)
            .collect();
        format!(
            "\x1b[90m[\x1b[0m{}, \x1b[90m... ({} more)]\x1b[0m",
            first.join(", "),
            items.len() - 5
        )
    }
}

/// Format a value for inline display (compact)
pub fn format_value_inline(val: &Value) -> String {
    match val {
        Value::Literal(s) => format!("\x1b[33m\"{}\"\x1b[0m", s),
        Value::Output(s) => s.trim().to_string(),
        Value::Number(n) => {
            if n.fract() == 0.0 && n.abs() < i64::MAX as f64 {
                format!("\x1b[35m{}\x1b[0m", *n as i64)
            } else {
                format!("\x1b[35m{:.2}\x1b[0m", n)
            }
        }
        Value::Bool(b) => format!("\x1b[34m{}\x1b[0m", b),
        Value::Nil => "\x1b[90mnil\x1b[0m".to_string(),
        Value::List(items) => format!("\x1b[90m[...{}]\x1b[0m", items.len()),
        Value::Map(_) => "\x1b[90m{...}\x1b[0m".to_string(),
        Value::Table { rows, .. } => format!("\x1b[90m<table:{} rows>\x1b[0m", rows.len()),
        Value::Block(_) => "\x1b[32m[...]\x1b[0m".to_string(),
        Value::Marker => "\x1b[90m|marker|\x1b[0m".to_string(),
        Value::Error { message, .. } => format!("\x1b[31mError: {}\x1b[0m", message),
    }
}

/// Format a value for the stack hint (very compact)
pub fn format_value_hint(val: &Value) -> String {
    match val {
        Value::Literal(s) => {
            if s.len() > 20 {
                format!("\"{}...\"", &s[..17])
            } else {
                format!("\"{}\"", s)
            }
        }
        Value::Output(s) => {
            let trimmed = s.trim();
            if trimmed.len() > 20 {
                format!("{}...", &trimmed[..17])
            } else {
                trimmed.to_string()
            }
        }
        Value::Number(n) => {
            if n.fract() == 0.0 && n.abs() < i64::MAX as f64 {
                format!("{}", *n as i64)
            } else {
                format!("{:.2}", n)
            }
        }
        Value::Bool(b) => b.to_string(),
        Value::Nil => "nil".to_string(),
        Value::List(items) => format!("[{}]", items.len()),
        Value::Map(m) => format!("{{{}}}", m.len()),
        Value::Table { rows, .. } => format!("<{}>", rows.len()),
        Value::Block(exprs) => format!("[{}]", exprs.len()),
        Value::Marker => "|".to_string(),
        Value::Error { .. } => "err".to_string(),
    }
}

/// Truncate a string to max width, adding ellipsis if needed
fn truncate_str(s: &str, max_width: usize) -> String {
    if s.len() <= max_width {
        s.to_string()
    } else if max_width <= 1 {
        ".".to_string()
    } else {
        format!("{}…", &s[..max_width - 1])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_empty_table() {
        let result = format_table(&[], &[], 80);
        assert_eq!(result, "(empty table)");
    }

    #[test]
    fn test_format_simple_table() {
        let columns = vec!["name".to_string(), "age".to_string()];
        let rows = vec![
            vec![Value::Literal("alice".to_string()), Value::Number(30.0)],
            vec![Value::Literal("bob".to_string()), Value::Number(25.0)],
        ];
        let result = format_table(&columns, &rows, 80);
        assert!(result.contains("name"));
        assert!(result.contains("age"));
        assert!(result.contains("alice"));
        assert!(result.contains("bob"));
        assert!(result.contains("2 rows"));
    }

    #[test]
    fn test_format_empty_record() {
        let result = format_record(&HashMap::new(), 80);
        assert_eq!(result, "{}");
    }

    #[test]
    fn test_format_simple_record() {
        let mut map = HashMap::new();
        map.insert("name".to_string(), Value::Literal("hsab".to_string()));
        map.insert("version".to_string(), Value::Literal("0.2".to_string()));
        let result = format_record(&map, 80);
        assert!(result.contains("name"));
        assert!(result.contains("version"));
    }

    #[test]
    fn test_truncate_str() {
        assert_eq!(truncate_str("hello", 10), "hello");
        assert_eq!(truncate_str("hello world", 5), "hell…");
        assert_eq!(truncate_str("hi", 2), "hi");
    }
}
