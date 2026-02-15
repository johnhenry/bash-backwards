//! Display formatting for structured data types
//!
//! Provides pretty-printing for Tables, Records, Lists, and Media
//! when displayed in the terminal.
//!
//! ## Terminal Graphics Support
//!
//! Media values can be rendered using one of three protocols:
//! - **iTerm2**: macOS-specific inline images via OSC 1337
//! - **Kitty**: Advanced graphics protocol with APC sequences
//! - **Sixel**: Wide terminal support, DEC-style bitmap graphics
//!
//! Protocol detection is automatic based on TERM_PROGRAM and capability queries.

use crate::ast::Value;
use std::collections::HashMap;
use std::sync::OnceLock;

/// Terminal graphics protocol supported by the current terminal
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GraphicsProtocol {
    /// No graphics support - show text placeholder
    None,
    /// iTerm2 inline images (macOS)
    ITerm2,
    /// Kitty graphics protocol
    Kitty,
    /// Sixel bitmap graphics
    Sixel,
}

/// Cached graphics protocol detection result
static GRAPHICS_PROTOCOL: OnceLock<GraphicsProtocol> = OnceLock::new();

/// Detect which graphics protocol the terminal supports
pub fn detect_graphics_protocol() -> GraphicsProtocol {
    *GRAPHICS_PROTOCOL.get_or_init(|| {
        // Check TERM_PROGRAM for iTerm2
        if let Ok(term_prog) = std::env::var("TERM_PROGRAM") {
            if term_prog.to_lowercase().contains("iterm") {
                return GraphicsProtocol::ITerm2;
            }
        }

        // Check for Kitty
        if std::env::var("KITTY_WINDOW_ID").is_ok() {
            return GraphicsProtocol::Kitty;
        }

        // Check TERM for sixel support hints
        if let Ok(term) = std::env::var("TERM") {
            // Some terminals advertise sixel support in TERM
            if term.contains("sixel") || term.contains("mlterm") || term.contains("mintty") {
                return GraphicsProtocol::Sixel;
            }
        }

        // Check COLORTERM for additional hints
        if let Ok(colorterm) = std::env::var("COLORTERM") {
            if colorterm == "truecolor" || colorterm == "24bit" {
                // Modern terminal, might support Kitty protocol
                // Could query with ESC_G but for now default to none
            }
        }

        GraphicsProtocol::None
    })
}

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
        Value::Media { mime_type, data, width, height, alt, source } => {
            format_media(mime_type, data, *width, *height, alt.as_deref(), source.as_deref(), max_width)
        }
        Value::Link { url, text } => format_link(url, text.as_deref()),
        Value::Bytes(data) => format_bytes(data, max_width),
        _ => val.as_arg().unwrap_or_default(),
    }
}

/// Format bytes for terminal display
/// Shows: [Bytes: 32B abc123...] with hex preview
fn format_bytes(data: &[u8], max_width: usize) -> String {
    let size = data.len();
    let size_str = if size == 1 { "1B".to_string() } else { format!("{}B", size) };

    // Calculate space for hex preview
    let prefix = format!("[Bytes: {} ", size_str);
    let suffix = "]";
    let overhead = prefix.len() + suffix.len();

    if max_width <= overhead + 3 {
        // Not enough space, just show size
        return format!("[Bytes: {}]", size_str);
    }

    let available = max_width - overhead;
    let hex = hex::encode(data);

    if hex.len() <= available {
        format!("{}{}{}", prefix, hex, suffix)
    } else {
        // Truncate with ellipsis
        let truncated = &hex[..available.saturating_sub(3)];
        format!("{}{}...{}", prefix, truncated, suffix)
    }
}

/// Format a hyperlink using OSC 8 escape sequence
/// Protocol: ESC ] 8 ; params ; URI BEL text ESC ] 8 ; ; BEL
fn format_link(url: &str, text: Option<&str>) -> String {
    let display_text = text.unwrap_or(url);
    // OSC 8: \x1b]8;;URL\x07 TEXT \x1b]8;;\x07
    format!("\x1b]8;;{}\x07{}\x1b]8;;\x07", url, display_text)
}

/// Format media content for terminal display using the appropriate graphics protocol
fn format_media(
    mime_type: &str,
    data: &[u8],
    width: Option<u32>,
    height: Option<u32>,
    alt: Option<&str>,
    source: Option<&str>,
    _max_width: usize,
) -> String {
    let protocol = detect_graphics_protocol();

    match protocol {
        GraphicsProtocol::ITerm2 => format_media_iterm2(data, width, height),
        GraphicsProtocol::Kitty => format_media_kitty(data, mime_type),
        GraphicsProtocol::Sixel => format_media_placeholder(mime_type, data, width, height, alt, source, "sixel"),
        GraphicsProtocol::None => format_media_placeholder(mime_type, data, width, height, alt, source, "none"),
    }
}

/// Format media using iTerm2 inline image protocol
/// Protocol: ESC ] 1337 ; File = [args] : base64data BEL
fn format_media_iterm2(data: &[u8], width: Option<u32>, height: Option<u32>) -> String {
    use base64::{Engine as _, engine::general_purpose::STANDARD};

    let mut args = vec!["inline=1".to_string()];

    // Size in bytes helps iTerm2 allocate memory
    args.push(format!("size={}", data.len()));

    // Optional dimensions (in cells or pixels with px suffix)
    if let Some(w) = width {
        // Scale down for terminal display (assume ~10px per cell width)
        let cells = (w / 10).max(10).min(80);
        args.push(format!("width={}", cells));
    }
    if let Some(h) = height {
        // Scale down for terminal display (assume ~20px per cell height)
        let cells = (h / 20).max(5).min(40);
        args.push(format!("height={}", cells));
    }

    // preserveAspectRatio=1 maintains proportions
    args.push("preserveAspectRatio=1".to_string());

    let args_str = args.join(";");
    let b64_data = STANDARD.encode(data);

    // OSC 1337 ; File = args : base64data BEL
    format!("\x1b]1337;File={}:{}\x07", args_str, b64_data)
}

/// Format media using Kitty graphics protocol
/// Protocol: ESC _ G a=T,f=100,... ; base64data ESC \
fn format_media_kitty(data: &[u8], mime_type: &str) -> String {
    use base64::{Engine as _, engine::general_purpose::STANDARD};

    // Kitty format codes: 100=PNG, 24=RGB, 32=RGBA
    let format = if mime_type.contains("png") {
        "100" // PNG
    } else {
        "100" // Default to PNG format (Kitty can auto-detect)
    };

    let b64_data = STANDARD.encode(data);

    // For large images, Kitty supports chunked transmission
    // For simplicity, we'll send as single chunk if small enough
    if b64_data.len() <= 4096 {
        // Single chunk: a=T (transmit and display), f=format, m=0 (no more chunks)
        format!("\x1b_Ga=T,f={},m=0;{}\x1b\\", format, b64_data)
    } else {
        // Multi-chunk transmission
        let mut output = String::new();
        let chunks: Vec<&str> = b64_data.as_bytes()
            .chunks(4096)
            .map(|c| std::str::from_utf8(c).unwrap_or(""))
            .collect();

        for (i, chunk) in chunks.iter().enumerate() {
            let is_last = i == chunks.len() - 1;
            let m = if is_last { "0" } else { "1" };

            if i == 0 {
                // First chunk: include transmission parameters
                output.push_str(&format!("\x1b_Ga=T,f={},m={};{}\x1b\\", format, m, chunk));
            } else {
                // Continuation chunks: just m parameter
                output.push_str(&format!("\x1b_Gm={};{}\x1b\\", m, chunk));
            }
        }
        output
    }
}

/// Format media as a text placeholder (when graphics not supported)
fn format_media_placeholder(
    mime_type: &str,
    data: &[u8],
    width: Option<u32>,
    height: Option<u32>,
    alt: Option<&str>,
    source: Option<&str>,
    protocol_hint: &str,
) -> String {
    let size_str = if data.len() < 1024 {
        format!("{} bytes", data.len())
    } else if data.len() < 1024 * 1024 {
        format!("{:.1} KB", data.len() as f64 / 1024.0)
    } else {
        format!("{:.1} MB", data.len() as f64 / (1024.0 * 1024.0))
    };

    let dims = match (width, height) {
        (Some(w), Some(h)) => format!(" {}x{}", w, h),
        _ => String::new(),
    };

    let src = source.map(|s| format!(" ({})", s)).unwrap_or_default();
    let alt_text = alt.map(|a| format!(" \"{}\"", a)).unwrap_or_default();

    let hint = if protocol_hint == "sixel" {
        " [sixel: not yet implemented]"
    } else {
        ""
    };

    format!(
        "\x1b[36m[Image: {}{} {}{}{}{}]\x1b[0m",
        mime_type, dims, size_str, src, alt_text, hint
    )
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

/// Display mode for compact value formatting
#[derive(Debug, Clone, Copy, PartialEq)]
enum CompactMode {
    /// Colored inline display for record fields and list items
    Inline,
    /// Minimal plain-text display for stack hints
    Hint,
}

/// Format a value for inline display (compact)
pub fn format_value_inline(val: &Value) -> String {
    format_value_compact(val, CompactMode::Inline)
}

/// Format a value for the stack hint (very compact)
pub fn format_value_hint(val: &Value) -> String {
    format_value_compact(val, CompactMode::Hint)
}

/// Wrap text in an ANSI color code, or return it plain for hint mode
fn color(mode: CompactMode, code: &str, text: &str) -> String {
    match mode {
        CompactMode::Inline => format!("\x1b[{}m{}\x1b[0m", code, text),
        CompactMode::Hint => text.to_string(),
    }
}

/// Format a number with int detection, optionally colored
fn format_number_compact(n: f64, mode: CompactMode) -> String {
    let text = if n.fract() == 0.0 && n.abs() < i64::MAX as f64 {
        format!("{}", n as i64)
    } else {
        format!("{:.2}", n)
    };
    color(mode, "35", &text)
}

/// Truncate a string to max_len, appending "..." if truncated.
/// If `quote` is true, wraps result in double quotes.
fn truncate_compact(s: &str, max_len: usize, quote: bool) -> String {
    if s.len() > max_len {
        if quote {
            format!("\"{}...\"", &s[..max_len - 3])
        } else {
            format!("{}...", &s[..max_len - 3])
        }
    } else if quote {
        format!("\"{}\"", s)
    } else {
        s.to_string()
    }
}

/// Unified compact formatter shared by `format_value_inline` and `format_value_hint`
fn format_value_compact(val: &Value, mode: CompactMode) -> String {
    match val {
        Value::Literal(s) => {
            let quoted = match mode {
                CompactMode::Inline => format!("\"{}\"", s),
                CompactMode::Hint => truncate_compact(s, 20, true),
            };
            color(mode, "33", &quoted)
        }
        Value::Output(s) => {
            let trimmed = s.trim();
            match mode {
                CompactMode::Inline => trimmed.to_string(),
                CompactMode::Hint => truncate_compact(trimmed, 20, false),
            }
        }
        Value::Number(n) => format_number_compact(*n, mode),
        Value::Bool(b) => color(mode, "34", &b.to_string()),
        Value::Nil => color(mode, "90", "nil"),
        Value::List(items) => match mode {
            CompactMode::Inline => color(mode, "90", &format!("[...{}]", items.len())),
            CompactMode::Hint => format!("[{}]", items.len()),
        },
        Value::Map(m) => match mode {
            CompactMode::Inline => color(mode, "90", "{...}"),
            CompactMode::Hint => format!("{{{}}}", m.len()),
        },
        Value::Table { rows, .. } => match mode {
            CompactMode::Inline => color(mode, "90", &format!("<table:{} rows>", rows.len())),
            CompactMode::Hint => format!("<{}>", rows.len()),
        },
        Value::Block(exprs) => match mode {
            CompactMode::Inline => color(mode, "32", "[...]"),
            CompactMode::Hint => format!("[{}]", exprs.len()),
        },
        Value::Marker => match mode {
            CompactMode::Inline => color(mode, "90", "|marker|"),
            CompactMode::Hint => "|".to_string(),
        },
        Value::Error { message, .. } => match mode {
            CompactMode::Inline => color(mode, "31", &format!("Error: {}", message)),
            CompactMode::Hint => "err".to_string(),
        },
        Value::Media { mime_type, data, width, height, .. } => {
            let size_kb = data.len() as f64 / 1024.0;
            match mode {
                CompactMode::Inline => {
                    let dims = match (width, height) {
                        (Some(w), Some(h)) => format!(" {}x{}", w, h),
                        _ => String::new(),
                    };
                    color(mode, "36", &format!("<img:{}{} {:.1}KB>",
                        mime_type.split('/').last().unwrap_or("?"), dims, size_kb))
                }
                CompactMode::Hint => format!("<img:{:.0}K>", size_kb),
            }
        }
        Value::Link { url, text } => match mode {
            CompactMode::Inline => {
                let display = text.as_deref().unwrap_or(url);
                if display.len() > 30 {
                    color(mode, "34", &format!("<link:{}...>", &display[..27]))
                } else {
                    color(mode, "34", &format!("<link:{}>", display))
                }
            }
            CompactMode::Hint => "<link>".to_string(),
        },
        Value::Bytes(data) => match mode {
            CompactMode::Inline => {
                let hex = hex::encode(data);
                if hex.len() > 16 {
                    color(mode, "36", &format!("<bytes:{}B {}...>", data.len(), &hex[..12]))
                } else {
                    color(mode, "36", &format!("<bytes:{}B {}>", data.len(), hex))
                }
            }
            CompactMode::Hint => format!("<{}B>", data.len()),
        },
        Value::BigInt(n) => {
            let s = n.to_string();
            match mode {
                CompactMode::Inline => {
                    if s.len() > 20 {
                        color(mode, "35", &format!("<bigint:{}...>", &s[..17]))
                    } else {
                        color(mode, "35", &s)
                    }
                }
                CompactMode::Hint => {
                    if s.len() > 10 {
                        format!("{}...", &s[..7])
                    } else {
                        s
                    }
                }
            }
        }
        Value::Future { id, state } => {
            use crate::ast::FutureState;
            let guard = state.lock().unwrap();
            let status = match &*guard {
                FutureState::Pending => "pending",
                FutureState::Completed(_) => "completed",
                FutureState::Failed(_) => "failed",
                FutureState::Cancelled => "cancelled",
            };
            drop(guard);
            match mode {
                CompactMode::Inline => color(mode, "36", &format!("Future<{}:{}>", status, id)),
                CompactMode::Hint => format!("Future<{}>", status),
            }
        }
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
    use crate::ast::Expr;

    // ============================================================
    // format_value() tests
    // ============================================================

    #[test]
    fn test_format_value_error_with_exit_code() {
        let err = Value::Error {
            kind: "Command".to_string(),
            message: "command failed".to_string(),
            code: Some(127),
            source: None,
            command: Some("ls".to_string()),
        };
        let result = format_value(&err, 80);
        assert!(result.contains("Error[Command]"));
        assert!(result.contains("command failed"));
        assert!(result.contains("(exit 127)"));
    }

    #[test]
    fn test_format_value_error_without_exit_code() {
        let err = Value::Error {
            kind: "Parse".to_string(),
            message: "syntax error".to_string(),
            code: None,
            source: None,
            command: None,
        };
        let result = format_value(&err, 80);
        assert!(result.contains("Error[Parse]"));
        assert!(result.contains("syntax error"));
        assert!(!result.contains("exit"));
    }

    #[test]
    fn test_format_value_table() {
        let table = Value::Table {
            columns: vec!["col1".to_string()],
            rows: vec![vec![Value::Literal("val1".to_string())]],
        };
        let result = format_value(&table, 80);
        assert!(result.contains("col1"));
        assert!(result.contains("val1"));
    }

    #[test]
    fn test_format_value_map() {
        let mut map = HashMap::new();
        map.insert("key".to_string(), Value::Literal("value".to_string()));
        let result = format_value(&Value::Map(map), 80);
        assert!(result.contains("key"));
        assert!(result.contains("value"));
    }

    #[test]
    fn test_format_value_list() {
        let list = Value::List(vec![Value::Number(1.0), Value::Number(2.0)]);
        let result = format_value(&list, 80);
        assert!(result.contains("1"));
        assert!(result.contains("2"));
    }

    #[test]
    fn test_format_value_literal() {
        let lit = Value::Literal("hello world".to_string());
        let result = format_value(&lit, 80);
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_format_value_number() {
        let num = Value::Number(42.0);
        let result = format_value(&num, 80);
        assert_eq!(result, "42");
    }

    #[test]
    fn test_format_value_bool() {
        let val = Value::Bool(true);
        let result = format_value(&val, 80);
        assert_eq!(result, "true");
    }

    #[test]
    fn test_format_value_nil() {
        // Nil returns empty string via as_arg().unwrap_or_default()
        let result = format_value(&Value::Nil, 80);
        assert_eq!(result, "");
    }

    // ============================================================
    // format_table() tests - column width scaling
    // ============================================================

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
    fn test_format_table_column_width_scaling() {
        // Create a table with columns that would exceed max_width
        let columns = vec![
            "very_long_column_name_one".to_string(),
            "very_long_column_name_two".to_string(),
            "very_long_column_name_three".to_string(),
        ];
        let rows = vec![
            vec![
                Value::Literal("some_long_value_here".to_string()),
                Value::Literal("another_long_value".to_string()),
                Value::Literal("third_long_value".to_string()),
            ],
        ];
        // Set max_width to trigger scaling
        let result = format_table(&columns, &rows, 50);
        // Should still contain truncated content
        assert!(result.contains("rows"));
        // The table should be rendered (has box-drawing chars)
        assert!(result.contains("┌"));
        assert!(result.contains("┘"));
    }

    #[test]
    fn test_format_table_very_narrow_width() {
        // Test with very narrow max_width to force aggressive scaling
        let columns = vec!["col1".to_string(), "col2".to_string()];
        let rows = vec![
            vec![Value::Literal("value1".to_string()), Value::Literal("value2".to_string())],
        ];
        let result = format_table(&columns, &rows, 20);
        // Table should still render with truncation
        assert!(result.contains("┌"));
        assert!(result.contains("1 rows"));
    }

    // ============================================================
    // format_record() tests
    // ============================================================

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

    // ============================================================
    // format_list() tests
    // ============================================================

    #[test]
    fn test_format_list_empty() {
        let result = format_list(&[], 80);
        assert_eq!(result, "[]");
    }

    #[test]
    fn test_format_list_small() {
        // Small list (<=10 items) shows all items with "..."
        let items = vec![
            Value::Number(1.0),
            Value::Number(2.0),
            Value::Number(3.0),
        ];
        let result = format_list(&items, 80);
        assert!(result.contains("["));
        assert!(result.contains("]"));
        assert!(result.contains("1"));
        assert!(result.contains("2"));
        assert!(result.contains("3"));
        assert!(result.contains("..."));
    }

    #[test]
    fn test_format_list_exactly_10_items() {
        // Exactly 10 items should use the small list format
        let items: Vec<Value> = (1..=10).map(|i| Value::Number(i as f64)).collect();
        let result = format_list(&items, 80);
        assert!(result.contains("..."));
        assert!(!result.contains("more"));
    }

    #[test]
    fn test_format_list_large() {
        // Large list (>10 items) shows first 5 items with "... (X more)"
        let items: Vec<Value> = (1..=15).map(|i| Value::Number(i as f64)).collect();
        let result = format_list(&items, 80);
        assert!(result.contains("["));
        assert!(result.contains("1"));
        assert!(result.contains("2"));
        assert!(result.contains("3"));
        assert!(result.contains("4"));
        assert!(result.contains("5"));
        assert!(result.contains("10 more")); // 15 - 5 = 10 more
    }

    #[test]
    fn test_format_list_with_mixed_types() {
        let items = vec![
            Value::Literal("hello".to_string()),
            Value::Number(42.0),
            Value::Bool(true),
        ];
        let result = format_list(&items, 80);
        assert!(result.contains("hello"));
        assert!(result.contains("42"));
        assert!(result.contains("true"));
    }

    // ============================================================
    // format_value_inline() tests
    // ============================================================

    #[test]
    fn test_format_value_inline_literal() {
        let result = format_value_inline(&Value::Literal("test".to_string()));
        assert!(result.contains("\"test\""));
    }

    #[test]
    fn test_format_value_inline_output() {
        let result = format_value_inline(&Value::Output("output text\n".to_string()));
        assert_eq!(result, "output text");
    }

    #[test]
    fn test_format_value_inline_number_integer() {
        let result = format_value_inline(&Value::Number(42.0));
        assert!(result.contains("42"));
        assert!(!result.contains(".")); // No decimal for whole numbers
    }

    #[test]
    fn test_format_value_inline_number_float() {
        let result = format_value_inline(&Value::Number(3.14159));
        assert!(result.contains("3.14"));
    }

    #[test]
    fn test_format_value_inline_bool() {
        let result = format_value_inline(&Value::Bool(false));
        assert!(result.contains("false"));
    }

    #[test]
    fn test_format_value_inline_nil() {
        let result = format_value_inline(&Value::Nil);
        assert!(result.contains("nil"));
    }

    #[test]
    fn test_format_value_inline_list() {
        let list = Value::List(vec![Value::Number(1.0), Value::Number(2.0), Value::Number(3.0)]);
        let result = format_value_inline(&list);
        assert!(result.contains("[...3]"));
    }

    #[test]
    fn test_format_value_inline_map() {
        let mut map = HashMap::new();
        map.insert("key".to_string(), Value::Literal("value".to_string()));
        let result = format_value_inline(&Value::Map(map));
        assert!(result.contains("{...}"));
    }

    #[test]
    fn test_format_value_inline_table() {
        let table = Value::Table {
            columns: vec!["col".to_string()],
            rows: vec![
                vec![Value::Literal("a".to_string())],
                vec![Value::Literal("b".to_string())],
            ],
        };
        let result = format_value_inline(&table);
        assert!(result.contains("<table:2 rows>"));
    }

    #[test]
    fn test_format_value_inline_block() {
        let block = Value::Block(vec![Expr::Literal("echo".to_string())]);
        let result = format_value_inline(&block);
        assert!(result.contains("[...]"));
    }

    #[test]
    fn test_format_value_inline_marker() {
        let result = format_value_inline(&Value::Marker);
        assert!(result.contains("|marker|"));
    }

    #[test]
    fn test_format_value_inline_error() {
        let err = Value::Error {
            kind: "Test".to_string(),
            message: "test error".to_string(),
            code: Some(1),
            source: None,
            command: None,
        };
        let result = format_value_inline(&err);
        assert!(result.contains("Error: test error"));
    }

    // ============================================================
    // format_value_hint() tests
    // ============================================================

    #[test]
    fn test_format_value_hint_literal_short() {
        let result = format_value_hint(&Value::Literal("short".to_string()));
        assert_eq!(result, "\"short\"");
    }

    #[test]
    fn test_format_value_hint_literal_long() {
        let long_string = "this is a very long string that exceeds 20 characters";
        let result = format_value_hint(&Value::Literal(long_string.to_string()));
        assert!(result.starts_with("\"this is a very lo"));
        assert!(result.ends_with("...\""));
        assert!(result.len() < long_string.len() + 3); // Should be truncated
    }

    #[test]
    fn test_format_value_hint_output_short() {
        let result = format_value_hint(&Value::Output("output\n".to_string()));
        assert_eq!(result, "output");
    }

    #[test]
    fn test_format_value_hint_output_long() {
        let long_output = "this is a very long output string that exceeds limit";
        let result = format_value_hint(&Value::Output(long_output.to_string()));
        assert!(result.contains("..."));
        assert!(result.len() < long_output.len());
    }

    #[test]
    fn test_format_value_hint_number_integer() {
        let result = format_value_hint(&Value::Number(100.0));
        assert_eq!(result, "100");
    }

    #[test]
    fn test_format_value_hint_number_float() {
        let result = format_value_hint(&Value::Number(3.14159));
        assert_eq!(result, "3.14");
    }

    #[test]
    fn test_format_value_hint_bool() {
        assert_eq!(format_value_hint(&Value::Bool(true)), "true");
        assert_eq!(format_value_hint(&Value::Bool(false)), "false");
    }

    #[test]
    fn test_format_value_hint_nil() {
        let result = format_value_hint(&Value::Nil);
        assert_eq!(result, "nil");
    }

    #[test]
    fn test_format_value_hint_list() {
        let list = Value::List(vec![
            Value::Number(1.0),
            Value::Number(2.0),
            Value::Number(3.0),
        ]);
        let result = format_value_hint(&list);
        assert_eq!(result, "[3]");
    }

    #[test]
    fn test_format_value_hint_map() {
        let mut map = HashMap::new();
        map.insert("a".to_string(), Value::Number(1.0));
        map.insert("b".to_string(), Value::Number(2.0));
        let result = format_value_hint(&Value::Map(map));
        assert_eq!(result, "{2}");
    }

    #[test]
    fn test_format_value_hint_table() {
        let table = Value::Table {
            columns: vec!["col1".to_string(), "col2".to_string()],
            rows: vec![
                vec![Value::Number(1.0), Value::Number(2.0)],
                vec![Value::Number(3.0), Value::Number(4.0)],
                vec![Value::Number(5.0), Value::Number(6.0)],
            ],
        };
        let result = format_value_hint(&table);
        assert_eq!(result, "<3>");
    }

    #[test]
    fn test_format_value_hint_block() {
        let block = Value::Block(vec![
            Expr::Literal("echo".to_string()),
            Expr::Literal("hello".to_string()),
        ]);
        let result = format_value_hint(&block);
        assert_eq!(result, "[2]");
    }

    #[test]
    fn test_format_value_hint_marker() {
        let result = format_value_hint(&Value::Marker);
        assert_eq!(result, "|");
    }

    #[test]
    fn test_format_value_hint_error() {
        let err = Value::Error {
            kind: "Test".to_string(),
            message: "error message".to_string(),
            code: Some(1),
            source: None,
            command: None,
        };
        let result = format_value_hint(&err);
        assert_eq!(result, "err");
    }

    // ============================================================
    // truncate_str() tests
    // ============================================================

    #[test]
    fn test_truncate_str() {
        assert_eq!(truncate_str("hello", 10), "hello");
        assert_eq!(truncate_str("hello world", 5), "hell…");
        assert_eq!(truncate_str("hi", 2), "hi");
    }

    #[test]
    fn test_truncate_str_max_width_1() {
        // Edge case: max_width=1 should return "."
        assert_eq!(truncate_str("hello", 1), ".");
    }

    #[test]
    fn test_truncate_str_max_width_0() {
        // Edge case: max_width=0 should return "."
        assert_eq!(truncate_str("hello", 0), ".");
    }

    #[test]
    fn test_truncate_str_exact_length() {
        // String exactly at max_width should not be truncated
        assert_eq!(truncate_str("hello", 5), "hello");
    }

    #[test]
    fn test_truncate_str_one_under() {
        // String one under max_width should not be truncated
        assert_eq!(truncate_str("hello", 6), "hello");
    }

    #[test]
    fn test_truncate_str_empty() {
        assert_eq!(truncate_str("", 5), "");
    }
}
