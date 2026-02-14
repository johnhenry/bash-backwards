use hsab::{Evaluator, Value};
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use rustyline::completion::{Completer, Pair};
use rustyline::{Cmd, ConditionalEventHandler, Editor, Event, EventContext, KeyCode, KeyEvent, Modifiers, Movement, RepeatCount};
use rustyline::{Helper, Result as RlResult};
use std::borrow::Cow;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use crate::prompt::{set_prompt_context, eval_prompt_definition, extract_hint_format};
use crate::rcfile::{load_hsabrc, load_hsab_profile, load_stdlib, dirs_home};
use crate::terminal::{execute_line, is_triple_quotes_balanced};
use crate::cli::print_help;

const VERSION: &str = env!("CARGO_PKG_VERSION");

// ============================================
// Shared state between the REPL and key handlers
// ============================================

struct SharedState {
    /// Stack values that can be popped to input (Ctrl+O)
    stack: Vec<Value>,
    /// Words to be pushed to stack after key handler returns (Alt+O)
    pending_push: Vec<String>,
    /// Pending prepend: value waiting to be prepended after cursor moves to end
    pending_prepend: Option<String>,
    /// Number of pops to apply to the real evaluator stack after readline returns
    pops_to_apply: usize,
    /// Hint format: (prefix, separator, suffix) for formatting stack items
    /// e.g., (" [", ", ", "]") produces " [a, b, c]"
    hint_format: (String, String, String),
    /// Whether to show the stack hint (Alt+h toggles)
    hint_visible: bool,
    /// Whether to show type annotations in hint (Alt+t toggles)
    show_types: bool,
    /// Limbo storage for popped values awaiting resolution
    limbo: std::collections::HashMap<String, Value>,
    /// Counter for generating unique limbo IDs
    limbo_counter: u32,
}

impl SharedState {
    fn new() -> Self {
        SharedState {
            stack: Vec::new(),
            pending_push: Vec::new(),
            pending_prepend: None,
            pops_to_apply: 0,
            hint_format: ("".to_string(), " ".to_string(), "".to_string()), // Default: space-separated
            hint_visible: true,
            show_types: false,
            limbo: std::collections::HashMap::new(),
            limbo_counter: 0,
        }
    }

    /// Generate a unique limbo ID
    fn generate_limbo_id(&mut self) -> String {
        self.limbo_counter += 1;
        format!("{:04x}", self.limbo_counter)
    }

    /// Check if a value is simple enough to pop directly (no limbo ref needed)
    /// Simple values: numbers, bools, short strings without special chars
    fn is_simple_value(&self, value: &Value) -> bool {
        match value {
            Value::Number(_) => true,
            Value::Bool(_) => true,
            Value::BigInt(n) => n.to_string().len() <= 20, // Reasonable length bigints
            Value::Literal(s) | Value::Output(s) => {
                // Simple if short and no special characters that would need quoting
                s.len() <= 30
                    && !s.contains(' ')
                    && !s.contains('\n')
                    && !s.contains('\t')
                    && !s.contains('"')
                    && !s.contains('\'')
                    && !s.contains('`')
                    && !s.contains('$')
                    && !s.contains('[')
                    && !s.contains(']')
                    && !s.is_empty()
            }
            _ => false, // Everything else (maps, lists, tables, media, blocks, etc.) needs limbo
        }
    }

    /// Check if a value is "huge" and should be auto-converted to limbo on stack sync
    /// Huge: large strings, large collections, media, etc.
    fn is_huge_value(&self, value: &Value) -> bool {
        match value {
            Value::Literal(s) | Value::Output(s) => s.len() > 100,
            Value::List(items) => items.len() > 10,
            Value::Map(m) => m.len() > 5,
            Value::Table { rows, .. } => rows.len() > 5,
            Value::Media { .. } => true, // Always huge
            Value::Bytes(b) => b.len() > 100,
            Value::Block(exprs) => exprs.len() > 10,
            Value::BigInt(n) => n.to_string().len() > 30,
            _ => false,
        }
    }

    /// Sync stack from evaluator, auto-converting huge values to limbo refs
    /// Returns the synced stack with huge values replaced by limbo ref literals
    fn sync_stack_with_auto_limbo(&mut self, eval_stack: &[Value]) -> Vec<Value> {
        eval_stack.iter().map(|value| {
            if self.is_huge_value(value) {
                // Convert huge value to limbo ref immediately
                let id = self.generate_limbo_id();
                let limbo_ref = self.format_limbo_ref(&id, value);
                self.limbo.insert(id, value.clone());
                // Return the limbo ref as a Literal so it displays in hint
                // and pops directly as the ref string
                Value::Literal(limbo_ref)
            } else {
                value.clone()
            }
        }).collect()
    }

    /// Extract the first token from input, handling quoted strings and limbo refs
    /// Returns (token, rest_of_line) or None if empty
    fn extract_first_token(line: &str) -> Option<(String, String)> {
        let trimmed = line.trim_start();
        if trimmed.is_empty() {
            return None;
        }

        let chars: Vec<char> = trimmed.chars().collect();
        let first = chars[0];

        // Handle quoted strings
        if first == '"' || first == '\'' {
            let quote = first;
            let mut i = 1;
            let mut escaped = false;
            while i < chars.len() {
                if escaped {
                    escaped = false;
                } else if chars[i] == '\\' {
                    escaped = true;
                } else if chars[i] == quote {
                    // Found closing quote
                    let token: String = chars[..=i].iter().collect();
                    let rest: String = chars[i + 1..].iter().collect();
                    return Some((token, rest.trim_start().to_string()));
                }
                i += 1;
            }
            // Unclosed quote - take the whole thing
            return Some((trimmed.to_string(), String::new()));
        }

        // Handle limbo refs
        if first == '`' {
            if let Some(end) = trimmed[1..].find('`') {
                let token = &trimmed[..end + 2];
                let rest = &trimmed[end + 2..];
                return Some((token.to_string(), rest.trim_start().to_string()));
            }
        }

        // Regular word - ends at whitespace
        let end = trimmed.find(char::is_whitespace).unwrap_or(trimmed.len());
        let token = &trimmed[..end];
        let rest = &trimmed[end..];
        Some((token.to_string(), rest.trim_start().to_string()))
    }

    /// Split input into tokens, handling quoted strings and limbo refs
    fn tokenize_input(line: &str) -> Vec<String> {
        let mut tokens = Vec::new();
        let mut remaining = line.to_string();
        while let Some((token, rest)) = Self::extract_first_token(&remaining) {
            tokens.push(token);
            remaining = rest;
        }
        tokens
    }

    /// Get the direct representation of a simple value (for inserting into input)
    fn simple_value_repr(&self, value: &Value) -> Option<String> {
        match value {
            Value::Number(n) => {
                if n.fract() == 0.0 && n.abs() < i64::MAX as f64 {
                    Some(format!("{}", *n as i64))
                } else {
                    Some(format!("{}", n))
                }
            }
            Value::Bool(b) => Some(format!("{}", b)),
            Value::BigInt(n) => Some(n.to_string()),
            Value::Literal(s) | Value::Output(s) => Some(s.clone()),
            _ => None,
        }
    }

    /// Format a limbo reference with type and preview annotations
    fn format_limbo_ref(&self, id: &str, value: &Value) -> String {
        let preview_len = 8;
        let formatted = match value {
            Value::Literal(s) | Value::Output(s) => {
                let len = s.chars().count();
                let preview: String = s.chars().take(preview_len).collect();
                if len > preview_len {
                    format!("string[{}]:\"{}...\"", len, preview)
                } else {
                    format!("string:\"{}\"", s)
                }
            }
            Value::Number(n) => {
                if n.fract() == 0.0 && n.abs() < i64::MAX as f64 {
                    format!("i64:{}", *n as i64)
                } else {
                    format!("f64:{}", n)
                }
            }
            Value::Bool(b) => format!("bool:{}", b),
            Value::Map(m) => {
                let fields: Vec<_> = m.keys().take(3).cloned().collect();
                let suffix = if m.len() > 3 { ", ..." } else { "" };
                format!("record:{{{}{}}}", fields.join(", "), suffix)
            }
            Value::List(items) => {
                format!("vector[{}]", items.len())
            }
            Value::Table { columns, rows } => {
                format!("table[{}x{}]", columns.len(), rows.len())
            }
            Value::Media { mime_type, width, height, .. } => {
                match (width, height) {
                    (Some(w), Some(h)) => format!("{}[{}x{}]", mime_type, w, h),
                    _ => mime_type.clone(),
                }
            }
            Value::Block(_) => "block:[...]".to_string(),
            Value::BigInt(n) => {
                let s = n.to_string();
                if s.len() > preview_len {
                    format!("bigint:{}...", &s[..preview_len])
                } else {
                    format!("bigint:{}", s)
                }
            }
            Value::Bytes(b) => format!("bytes[{}]", b.len()),
            Value::Nil => "nil".to_string(),
            Value::Marker => "marker".to_string(),
            Value::Link { url, .. } => {
                if url.len() > preview_len {
                    format!("link:\"{}...\"", &url[..preview_len])
                } else {
                    format!("link:\"{}\"", url)
                }
            }
            Value::Error { kind, .. } => format!("error:{}", kind),
        };
        format!("`&{}:{}`", id, formatted)
    }

    /// Clear pending operations (e.g., after .clear)
    fn clear(&mut self) {
        self.pending_prepend = None;
        self.pops_to_apply = 0;
        self.limbo.clear();
        self.limbo_counter = 0;
    }

    /// Compute stack hint from current stack state
    fn compute_hint(&self) -> Option<String> {
        if !self.hint_visible {
            return None;
        }

        let items: Vec<String> = self.stack.iter().filter_map(|v| {
            if self.show_types {
                // Show type annotations
                match v {
                    Value::Literal(s) if s.len() > 15 => Some(format!("{}...(str)", &s[..12])),
                    Value::Literal(s) => Some(format!("{}(str)", s)),
                    Value::Output(s) if s.len() > 15 => Some(format!("{}...(out)", &s[..12])),
                    Value::Output(s) => Some(format!("{}(out)", s)),
                    Value::Block(_) => Some("[...](blk)".to_string()),
                    Value::Map(_) => Some("{...}(map)".to_string()),
                    Value::Table { .. } => Some("[table](tbl)".to_string()),
                    Value::List(_) => Some("[list](lst)".to_string()),
                    Value::Number(n) => Some(format!("{}(num)", n)),
                    Value::Bool(b) => Some(format!("{}(bool)", b)),
                    Value::Error { message, .. } => Some(format!("ERR:{}", message)),
                    Value::Media { data, .. } => Some(format!("<img:{}B>(media)", data.len())),
                    Value::Link { url, .. } => Some(format!("<link:{}>(link)", if url.len() > 10 { &url[..10] } else { url })),
                    Value::Bytes(data) => Some(format!("<{}B>(bytes)", data.len())),
                    Value::BigInt(n) => {
                        let s = n.to_string();
                        if s.len() > 12 {
                            Some(format!("{}...(bigint)", &s[..9]))
                        } else {
                            Some(format!("{}(bigint)", s))
                        }
                    }
                    Value::Marker => None,
                    Value::Nil => None,
                }
            } else {
                // Simple display
                match v.as_arg() {
                    Some(s) if s.len() > 20 => Some(format!("{}...", &s[..17])),
                    Some(s) => Some(s),
                    None => None,
                }
            }
        }).collect();

        if items.is_empty() {
            return None;
        }

        let (prefix, separator, suffix) = &self.hint_format;
        Some(format!("\n{}{}{}", prefix, items.join(separator), suffix))
    }
}

// ============================================
// Keyboard shortcut handlers for stack operations
// ============================================

/// Handler for Alt+Up: Pop from stack and insert into input
/// Simple values (numbers, short strings) are inserted directly
/// Complex values (records, lists, long strings) use limbo references
struct PopToInputHandler {
    state: Arc<Mutex<SharedState>>,
}

impl ConditionalEventHandler for PopToInputHandler {
    fn handle(&self, _evt: &Event, _n: RepeatCount, _positive: bool, ctx: &EventContext) -> Option<Cmd> {
        let mut state = self.state.lock().ok()?;

        // First check if we have a pending prepend to complete
        if let Some(text) = state.pending_prepend.take() {
            let current_line = ctx.line().to_string();
            let new_line = if current_line.is_empty() {
                format!("{} ", text)
            } else {
                format!("{} {}", text, current_line)
            };
            return Some(Cmd::Replace(Movement::BeginningOfLine, Some(new_line)));
        }

        // Pop from stack
        if let Some(value) = state.stack.pop() {
            // Track that we need to pop from the real evaluator stack too
            state.pops_to_apply += 1;

            // Simple values are inserted directly, complex values use limbo refs
            let insert_text = if state.is_simple_value(&value) {
                // Direct representation - no limbo needed
                state.simple_value_repr(&value).unwrap_or_else(|| "nil".to_string())
            } else {
                // Complex value - generate limbo reference
                let id = state.generate_limbo_id();
                let limbo_ref = state.format_limbo_ref(&id, &value);
                state.limbo.insert(id, value);
                limbo_ref
            };

            let current_line = ctx.line().to_string();
            let pos = ctx.pos();
            let len = current_line.len();

            if pos >= len {
                // Cursor at end (common case): do the replace now
                let new_line = if current_line.is_empty() {
                    format!("{} ", insert_text)
                } else {
                    format!("{} {}", insert_text, current_line)
                };
                return Some(Cmd::Replace(Movement::BeginningOfLine, Some(new_line)));
            } else {
                // Cursor not at end: move to end first, complete on next keypress
                state.pending_prepend = Some(insert_text);
                return Some(Cmd::Move(Movement::EndOfLine));
            }
        }
        Some(Cmd::Noop)
    }
}

/// Handler for Alt+Down: Push first token from input to stack
/// Handles quoted strings and limbo refs as single units
struct PushToStackHandler {
    state: Arc<Mutex<SharedState>>,
}

impl ConditionalEventHandler for PushToStackHandler {
    fn handle(&self, _evt: &Event, _n: RepeatCount, _positive: bool, ctx: &EventContext) -> Option<Cmd> {
        let line = ctx.line().to_string();

        // Extract first token (handles quoted strings, limbo refs, regular words)
        let (first_token, rest) = SharedState::extract_first_token(&line)?;

        if first_token.is_empty() {
            return Some(Cmd::Noop);
        }

        // Store the token to be pushed to stack when Enter is pressed
        // Also add to state.stack for immediate visual feedback in the hint
        if let Ok(mut state) = self.state.lock() {
            state.pending_push.push(first_token.clone());
            state.stack.push(Value::Literal(first_token));
        }

        Some(Cmd::Replace(Movement::WholeLine, Some(rest)))
    }
}

/// Handler for Alt+A (Shift): Push ALL tokens from input to stack
/// Handles quoted strings and limbo refs as single units
struct PushAllToStackHandler {
    state: Arc<Mutex<SharedState>>,
}

impl ConditionalEventHandler for PushAllToStackHandler {
    fn handle(&self, _evt: &Event, _n: RepeatCount, _positive: bool, ctx: &EventContext) -> Option<Cmd> {
        let line = ctx.line().to_string();

        // Tokenize input (handles quoted strings, limbo refs, regular words)
        let tokens = SharedState::tokenize_input(&line);
        if tokens.is_empty() {
            return Some(Cmd::Noop);
        }

        if let Ok(mut state) = self.state.lock() {
            for token in &tokens {
                state.pending_push.push(token.clone());
                state.stack.push(Value::Literal(token.clone()));
            }
        }

        // Clear the input line
        Some(Cmd::Replace(Movement::WholeLine, Some(String::new())))
    }
}

/// Handler for Ctrl+,: Clear the stack
struct ClearStackHandler {
    state: Arc<Mutex<SharedState>>,
}

impl ConditionalEventHandler for ClearStackHandler {
    fn handle(&self, _evt: &Event, _n: RepeatCount, _positive: bool, _ctx: &EventContext) -> Option<Cmd> {
        if let Ok(mut state) = self.state.lock() {
            // Mark all items in stack copy as needing to be popped from real stack
            let count = state.stack.len();
            state.stack.clear();
            state.clear();
            state.pops_to_apply = count;  // Set after clearing so it's not overwritten
        }
        // No change to the input line
        Some(Cmd::Noop)
    }
}

/// Handler for Alt+t: Toggle type annotations in hint
struct ToggleTypesHandler {
    state: Arc<Mutex<SharedState>>,
}

impl ConditionalEventHandler for ToggleTypesHandler {
    fn handle(&self, _evt: &Event, _n: RepeatCount, _positive: bool, ctx: &EventContext) -> Option<Cmd> {
        if let Ok(mut state) = self.state.lock() {
            state.show_types = !state.show_types;
        }
        // Replace line with itself to trigger a redraw (including hint refresh)
        let line = ctx.line().to_string();
        Some(Cmd::Replace(Movement::WholeLine, Some(line)))
    }
}

/// Handler for Alt+h: Toggle hint visibility
struct ToggleHintHandler {
    state: Arc<Mutex<SharedState>>,
}

impl ConditionalEventHandler for ToggleHintHandler {
    fn handle(&self, _evt: &Event, _n: RepeatCount, _positive: bool, ctx: &EventContext) -> Option<Cmd> {
        if let Ok(mut state) = self.state.lock() {
            state.hint_visible = !state.hint_visible;
        }
        // Replace line with itself to trigger a redraw (including hint refresh)
        let line = ctx.line().to_string();
        Some(Cmd::Replace(Movement::WholeLine, Some(line)))
    }
}

/// Handler for Alt+c: Copy top of stack to system clipboard (OSC 52)
struct ClipCopyHandler {
    state: Arc<Mutex<SharedState>>,
}

impl ConditionalEventHandler for ClipCopyHandler {
    fn handle(&self, _evt: &Event, _n: RepeatCount, _positive: bool, ctx: &EventContext) -> Option<Cmd> {
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        use std::io::Write;

        let state = self.state.lock().ok()?;
        if let Some(top) = state.stack.last() {
            if let Some(text) = top.as_arg() {
                let b64 = STANDARD.encode(text.as_bytes());
                print!("\x1b]52;c;{}\x07", b64);
                std::io::stdout().flush().ok();
            }
        }
        // Trigger redraw to refresh hint
        let line = ctx.line().to_string();
        Some(Cmd::Replace(Movement::WholeLine, Some(line)))
    }
}

/// Handler for Alt+x: Cut top of stack to system clipboard (OSC 52)
struct ClipCutHandler {
    state: Arc<Mutex<SharedState>>,
}

impl ConditionalEventHandler for ClipCutHandler {
    fn handle(&self, _evt: &Event, _n: RepeatCount, _positive: bool, ctx: &EventContext) -> Option<Cmd> {
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        use std::io::Write;

        let mut state = self.state.lock().ok()?;
        if let Some(top) = state.stack.pop() {
            if let Some(text) = top.as_arg() {
                let b64 = STANDARD.encode(text.as_bytes());
                print!("\x1b]52;c;{}\x07", b64);
                std::io::stdout().flush().ok();
            }
            state.pops_to_apply += 1;
        }
        // Trigger redraw to refresh hint
        let line = ctx.line().to_string();
        Some(Cmd::Replace(Movement::WholeLine, Some(line)))
    }
}

/// Handler for Alt+a: Pop ALL from stack and insert into input
/// Simple values are inserted directly, complex values use limbo references
struct PopAllToInputHandler {
    state: Arc<Mutex<SharedState>>,
}

impl ConditionalEventHandler for PopAllToInputHandler {
    fn handle(&self, _evt: &Event, _n: RepeatCount, _positive: bool, ctx: &EventContext) -> Option<Cmd> {
        let mut state = self.state.lock().ok()?;

        if state.stack.is_empty() {
            return Some(Cmd::Noop);
        }

        // Collect all stack items (in stack order, will be reversed for prepending)
        let mut items: Vec<String> = Vec::new();
        while let Some(value) = state.stack.pop() {
            state.pops_to_apply += 1;
            // Simple values are inserted directly, complex values use limbo refs
            let text = if state.is_simple_value(&value) {
                state.simple_value_repr(&value).unwrap_or_else(|| "nil".to_string())
            } else {
                let id = state.generate_limbo_id();
                let limbo_ref = state.format_limbo_ref(&id, &value);
                state.limbo.insert(id, value);
                limbo_ref
            };
            items.push(text);
        }

        if items.is_empty() {
            return Some(Cmd::Noop);
        }

        // Items are popped in LIFO order, so reverse to get original push order
        items.reverse();
        let insert_text = items.join(" ");

        let current_line = ctx.line().to_string();
        let new_line = if current_line.is_empty() {
            format!("{} ", insert_text)
        } else {
            format!("{} {}", insert_text, current_line)
        };

        Some(Cmd::Replace(Movement::BeginningOfLine, Some(new_line)))
    }
}

// ============================================
// HsabHelper: rustyline helper with tab completion and hints
// ============================================

/// Helper struct for rustyline with live stack display and tab completion
struct HsabHelper {
    state: Arc<Mutex<SharedState>>,
    builtins: HashSet<&'static str>,
    definitions: HashSet<String>,
}

impl Helper for HsabHelper {}

impl Completer for HsabHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &rustyline::Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        // Find the word being completed
        let start = line[..pos]
            .rfind(char::is_whitespace)
            .map(|i| i + 1)
            .unwrap_or(0);
        let prefix = &line[start..pos];

        if prefix.is_empty() {
            return Ok((start, Vec::new()));
        }

        // Check stack state for postfix-aware completion
        let stack_has_items = self.state.lock()
            .map(|s| !s.stack.is_empty())
            .unwrap_or(false);

        // Check if line already has content before cursor (indicates values already entered)
        let line_has_values = start > 0;

        let completions = if prefix.contains('/') || prefix.starts_with('.') || prefix.starts_with('~') {
            // Explicit path completion
            self.complete_path(prefix)
        } else if stack_has_items || line_has_values {
            // Stack has values OR line has previous words -> prioritize operations (commands)
            // This is postfix: values are on stack, user is typing the operation
            let mut cmds = self.complete_command(prefix);
            // Still include files, but after commands
            cmds.extend(self.complete_current_dir(prefix));
            cmds.sort();
            cmds.dedup();
            cmds
        } else {
            // Empty stack, first word -> prioritize files (values in postfix)
            // User is likely entering values first before applying operations
            let mut files = self.complete_current_dir(prefix);
            files.extend(self.complete_command(prefix));
            files.sort();
            files.dedup();
            files
        };

        let pairs: Vec<Pair> = completions
            .into_iter()
            .map(|c| Pair {
                display: c.clone(),
                replacement: c,
            })
            .collect();

        Ok((start, pairs))
    }
}

impl HsabHelper {
    /// Complete files in the current directory (for postfix value-first completion)
    fn complete_current_dir(&self, prefix: &str) -> Vec<String> {
        let mut completions = Vec::new();
        if let Ok(entries) = std::fs::read_dir(".") {
            for entry in entries.filter_map(|e| e.ok()) {
                if let Some(name) = entry.file_name().to_str() {
                    if name.starts_with(prefix) && !name.starts_with('.') {
                        let is_dir = entry.path().is_dir();
                        completions.push(if is_dir {
                            format!("{}/", name)
                        } else {
                            name.to_string()
                        });
                    }
                }
            }
        }
        completions.sort();
        completions
    }

    fn complete_command(&self, prefix: &str) -> Vec<String> {
        let mut completions = Vec::new();

        // Check builtins
        for &b in &self.builtins {
            if b.starts_with(prefix) {
                completions.push(b.to_string());
            }
        }

        // Check user definitions
        for d in &self.definitions {
            if d.starts_with(prefix) {
                completions.push(d.clone());
            }
        }

        // Check PATH for executables (limit to avoid slowness)
        if let Ok(path) = std::env::var("PATH") {
            let mut found = 0;
            'outer: for dir in path.split(':') {
                if let Ok(entries) = std::fs::read_dir(dir) {
                    for entry in entries.filter_map(|e| e.ok()) {
                        if let Some(name) = entry.file_name().to_str() {
                            if name.starts_with(prefix) && !completions.contains(&name.to_string()) {
                                completions.push(name.to_string());
                                found += 1;
                                if found >= 50 {
                                    break 'outer;
                                }
                            }
                        }
                    }
                }
            }
        }

        completions.sort();
        completions.dedup();
        completions
    }

    fn complete_path(&self, prefix: &str) -> Vec<String> {
        let expanded = if prefix.starts_with('~') {
            if let Ok(home) = std::env::var("HOME") {
                if prefix == "~" {
                    home.clone()
                } else {
                    prefix.replacen('~', &home, 1)
                }
            } else {
                prefix.to_string()
            }
        } else {
            prefix.to_string()
        };

        let (dir, file_prefix) = if expanded.contains('/') {
            let idx = expanded.rfind('/').unwrap();
            (&expanded[..=idx], &expanded[idx + 1..])
        } else {
            ("./", expanded.as_str())
        };

        let mut completions = Vec::new();
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                if let Some(name) = entry.file_name().to_str() {
                    if name.starts_with(file_prefix) {
                        let full = if prefix.starts_with('~') {
                            // Keep tilde prefix in output
                            let home = std::env::var("HOME").unwrap_or_default();
                            let full_path = format!("{}{}", dir, name);
                            full_path.replacen(&home, "~", 1)
                        } else {
                            format!("{}{}", dir, name)
                        };
                        // Add trailing slash for directories
                        let is_dir = entry.path().is_dir();
                        completions.push(if is_dir { format!("{}/", full) } else { full });
                    }
                }
            }
        }
        completions.sort();
        completions
    }
}

/// Get default builtins for tab completion
fn default_builtins() -> HashSet<&'static str> {
    [
        // Shell builtins
        "cd", "pwd", "echo", "true", "false", "test", "[",
        "export", "unset", "env", "exit", "jobs", "fg", "bg",
        "tty", "which", "source", ".", "hash", "type",
        "read", "printf", "wait", "kill", "local", "return",
        "pushd", "popd", "dirs", "alias", "unalias", "trap",
        // Stack operations
        "dup", "swap", "drop", "over", "rot", "depth",
        // Path operations
        "path-join", "suffix", "basename", "dirname", "path-resolve",
        // String operations
        "split1", "rsplit1", "len", "slice", "indexof", "str-replace", "format",
        // List operations
        "marker", "spread", "each", "keep", "collect", "map", "filter",
        // Control flow
        "if", "times", "while", "until", "break",
        // Parallel
        "parallel", "fork",
        // Process substitution
        "subst", "fifo",
        // JSON / Structured data
        "json", "unjson", "typeof",
        // Record operations
        "record", "get", "set", "del", "has?", "keys", "values", "merge",
        // Table operations
        "table", "where", "sort-by", "select", "first", "last", "nth",
        "group-by", "unique", "reverse", "flatten",
        // Error handling
        "try", "error?", "throw",
        // Serialization
        "into-json", "into-csv", "into-lines", "into-kv", "into-tsv", "into-delimited",
        "to-json", "to-csv", "to-lines", "to-kv",
        // Stack utilities
        "tap", "dip",
        // Aggregations
        "sum", "avg", "min", "max", "count",
        // Predicates
        "file?", "dir?", "exists?", "empty?", "eq?", "ne?",
        "=?", "!=?", "lt?", "gt?", "le?", "ge?",
        // Arithmetic
        "plus", "minus", "mul", "div", "mod",
        // Other
        "timeout", "pipestatus", ".import",
        // Plugins
        "plugin-load", "plugin-unload", "plugin-reload", "plugin-list", "plugin-info",
        // Media / Image operations
        "image-load", "image-show", "image-info", "to-base64", "from-base64",
        // Common external commands
        "ls", "cat", "grep", "find", "rm", "mv", "cp", "mkdir",
        "touch", "chmod", "head", "tail", "wc", "sort", "uniq",
        "git", "cargo", "make", "vim", "nano",
    ]
    .into_iter()
    .collect()
}

impl Hinter for HsabHelper {
    type Hint = String;

    fn hint(&self, _line: &str, _pos: usize, _ctx: &rustyline::Context<'_>) -> Option<String> {
        // Compute stack hint in real-time from shared state
        // This allows the hint to update as keyboard shortcuts modify the stack
        if let Ok(state) = self.state.lock() {
            state.compute_hint()
        } else {
            None
        }
    }
}

impl Highlighter for HsabHelper {
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        Cow::Borrowed(line)
    }

    fn highlight_char(&self, _line: &str, _pos: usize) -> bool {
        false
    }

    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        // Dim the stack hint
        Cow::Owned(format!("\x1b[90m{}\x1b[0m", hint))
    }
}

impl Validator for HsabHelper {}

// ============================================
// REPL main loop
// ============================================

/// Run the REPL with optional login shell mode
pub(crate) fn run_repl_with_login(is_login: bool, trace: bool) -> RlResult<()> {
    // Set up signal handlers for job control
    hsab::signals::setup_signal_handlers();

    let mut rl = Editor::new()?;

    // Set up shared state for keyboard handlers and stack display
    let shared_state = Arc::new(Mutex::new(SharedState::new()));

    // Set helper with shared state for live stack display and tab completion
    rl.set_helper(Some(HsabHelper {
        state: Arc::clone(&shared_state),
        builtins: default_builtins(),
        definitions: HashSet::new(),
    }));

    // Stack manipulation shortcuts:
    // - Alt+Up: Pop from stack and insert limbo reference into input
    // - Alt+Down: Push first word from input to stack
    // - Ctrl+Alt+Up: Push ALL words from input to stack
    // - Ctrl+Alt+Down: Pop ALL from stack to input
    // - Ctrl+,: Clear stack (discard)
    // Note: Some terminals (iTerm2, Terminal.app) may need configuration:
    // - iTerm2: Preferences > Profiles > Keys > Option key acts as: Esc+
    // - Terminal.app: Preferences > Profiles > Keyboard > Use Option as Meta key

    // Bind Alt+Up to pop from stack and insert limbo reference into input
    rl.bind_sequence(
        KeyEvent(KeyCode::Up, Modifiers::ALT),
        rustyline::EventHandler::Conditional(Box::new(PopToInputHandler {
            state: Arc::clone(&shared_state),
        })),
    );

    // Bind Alt+Down to push first word from input to stack
    rl.bind_sequence(
        KeyEvent(KeyCode::Down, Modifiers::ALT),
        rustyline::EventHandler::Conditional(Box::new(PushToStackHandler {
            state: Arc::clone(&shared_state),
        })),
    );

    // Bind Alt+a to pop ALL from stack to input
    // (Letter-based shortcuts are more reliable than modifier+arrow)
    rl.bind_sequence(
        KeyEvent(KeyCode::Char('a'), Modifiers::ALT),
        rustyline::EventHandler::Conditional(Box::new(PopAllToInputHandler {
            state: Arc::clone(&shared_state),
        })),
    );

    // Bind Alt+A (Alt+Shift+a) to push ALL words from input to stack
    rl.bind_sequence(
        KeyEvent(KeyCode::Char('A'), Modifiers::ALT),
        rustyline::EventHandler::Conditional(Box::new(PushAllToStackHandler {
            state: Arc::clone(&shared_state),
        })),
    );

    // Keep Ctrl+O as alternative for pop (compatibility with all terminals)
    rl.bind_sequence(
        KeyEvent(KeyCode::Char('O'), Modifiers::CTRL),
        rustyline::EventHandler::Conditional(Box::new(PopToInputHandler {
            state: Arc::clone(&shared_state),
        })),
    );

    // Bind Alt+k to clear/discard the stack (k = kill, like Ctrl+K in readline)
    rl.bind_sequence(
        KeyEvent(KeyCode::Char('k'), Modifiers::ALT),
        rustyline::EventHandler::Conditional(Box::new(ClearStackHandler {
            state: Arc::clone(&shared_state),
        })),
    );

    // Bind Alt+t to toggle type annotations in stack hint
    rl.bind_sequence(
        KeyEvent(KeyCode::Char('t'), Modifiers::ALT),
        rustyline::EventHandler::Conditional(Box::new(ToggleTypesHandler {
            state: Arc::clone(&shared_state),
        })),
    );

    // Bind Alt+h to toggle stack hint visibility
    rl.bind_sequence(
        KeyEvent(KeyCode::Char('h'), Modifiers::ALT),
        rustyline::EventHandler::Conditional(Box::new(ToggleHintHandler {
            state: Arc::clone(&shared_state),
        })),
    );

    // Bind Alt+c to copy top of stack to clipboard (OSC 52)
    rl.bind_sequence(
        KeyEvent(KeyCode::Char('c'), Modifiers::ALT),
        rustyline::EventHandler::Conditional(Box::new(ClipCopyHandler {
            state: Arc::clone(&shared_state),
        })),
    );

    // Bind Alt+x to cut top of stack to clipboard (OSC 52)
    rl.bind_sequence(
        KeyEvent(KeyCode::Char('x'), Modifiers::ALT),
        rustyline::EventHandler::Conditional(Box::new(ClipCutHandler {
            state: Arc::clone(&shared_state),
        })),
    );

    let mut eval = Evaluator::new();
    eval.set_trace_mode(trace);

    // Load profile if login shell
    if is_login {
        load_hsab_profile(&mut eval);
    }

    // Load stdlib first (provides defaults)
    load_stdlib(&mut eval);

    // Load ~/.hsabrc (user customizations override stdlib)
    load_hsabrc(&mut eval);

    // Extract hint format from STACK_HINT definition (for real-time stack display)
    {
        let format = extract_hint_format(&mut eval);
        let mut state = shared_state.lock().unwrap();
        state.hint_format = format;
    }

    // Try to load history
    let history_path = dirs_home().map(|h| h.join(".hsab_history"));
    if let Some(ref path) = history_path {
        let _ = rl.load_history(path);
    }

    // Show banner only if HSAB_BANNER is set
    if std::env::var("HSAB_BANNER").is_ok() {
        println!("hsab-{}£ Hash Backwards - stack-based postfix shell", VERSION);
        println!("  Type 'exit' or Ctrl-D to quit, '.help' for usage");
    }

    // Track items to pre-fill the next prompt (from .use command or Ctrl+Alt+Right)
    let mut prefill = String::new();
    // Track multiline input (for triple-quoted strings)
    let mut multiline_buffer = String::new();
    // Command counter for $_CMD_NUM
    let mut cmd_num: usize = 0;
    // Fallback prompts if PS1/PS2 not defined
    let fallback_normal = format!("hsab-{}£ ", VERSION);
    let fallback_stack = format!("hsab-{}¢ ", VERSION);
    let fallback_multiline = format!("hsab-{}… ", VERSION);

    loop {
        // Sync evaluator stack with shared state, auto-converting huge values to limbo refs
        {
            let mut state = shared_state.lock().unwrap();
            let eval_stack = eval.stack();
            state.stack = state.sync_stack_with_auto_limbo(eval_stack);
        }

        // Update definitions in helper for tab completion
        if let Some(helper) = rl.helper_mut() {
            helper.definitions = eval.definition_names();
        }

        // Set prompt context variables before generating prompt
        set_prompt_context(&eval, cmd_num);

        // Determine which prompt to use
        let prompt: String = if !multiline_buffer.is_empty() {
            // Multiline: try PS2, fallback to default
            eval_prompt_definition(&mut eval, "PS2")
                .unwrap_or_else(|| fallback_multiline.clone())
        } else {
            // Normal: try PS1, fallback to default
            eval_prompt_definition(&mut eval, "PS1")
                .unwrap_or_else(|| {
                    // Use fallback with pound/cent based on stack
                    let has_stack = eval.stack().iter().any(|v| v.as_arg().is_some());
                    if !prefill.is_empty() || has_stack {
                        fallback_stack.clone()
                    } else {
                        fallback_normal.clone()
                    }
                })
        };

        // Use readline_with_initial if we have prefill from .use command
        let readline = if prefill.is_empty() || !multiline_buffer.is_empty() {
            rl.readline(&prompt)
        } else {
            let initial = format!("{} ", prefill); // Add space after prefill
            prefill.clear();
            rl.readline_with_initial(&prompt, (&initial, ""))
        };

        match readline {
            Ok(line) => {
                // Process any pending pushes from Ctrl+\ (before executing the line)
                // and apply pending pops from Ctrl+] to the real evaluator stack
                {
                    let mut state = shared_state.lock().unwrap();

                    // Push words from input to stack
                    for word in state.pending_push.drain(..) {
                        eval.push_value(Value::Literal(word));
                    }

                    // Pop items from real stack that were popped from the copy during Ctrl+]
                    for _ in 0..state.pops_to_apply {
                        eval.pop_value();
                    }
                    state.pops_to_apply = 0;
                }
                // If we're in multiline mode, accumulate
                if !multiline_buffer.is_empty() {
                    multiline_buffer.push('\n');
                    multiline_buffer.push_str(&line);

                    // Check if we now have balanced triple quotes
                    if is_triple_quotes_balanced(&multiline_buffer) {
                        let complete_input = std::mem::take(&mut multiline_buffer);
                        let _ = rl.add_history_entry(&complete_input);

                        // Transfer limbo from SharedState to evaluator before execution
                        {
                            let mut state = shared_state.lock().unwrap();
                            for (id, value) in state.limbo.drain() {
                                eval.limbo.insert(id, value);
                            }
                        }

                        let result = execute_line(&mut eval, &complete_input, true);

                        // Clear limbo and pending state after execution
                        {
                            let mut state = shared_state.lock().unwrap();
                            state.clear();
                        }
                        eval.clear_limbo();

                        match result {
                            Ok(exit_code) => {
                                if exit_code != 0 {
                                    eprintln!("Exit code: {}", exit_code);
                                }
                            }
                            Err(e) => eprintln!("Error: {}", e),
                        }
                    }
                    continue;
                }

                let trimmed = line.trim();

                if trimmed.is_empty() {
                    continue;
                }

                // Check for unclosed triple quotes
                if !is_triple_quotes_balanced(trimmed) {
                    multiline_buffer = line.to_string();
                    continue;
                }

                // Add to history
                let _ = rl.add_history_entry(trimmed);

                // Handle built-in REPL commands (dot-prefix)
                match trimmed {
                    "exit" | "quit" => break,
                    ".help" | ".h" => {
                        print_help();
                        continue;
                    }
                    ".stack" | ".s" => {
                        // Debug command to show current stack
                        println!("Stack: {:?}", eval.stack());
                        continue;
                    }
                    ".clear" | ".c" | "clear" => {
                        // Clear the stack and the screen
                        eval.clear_stack();
                        {
                            let mut state = shared_state.lock().unwrap();
                            state.clear();
                        }
                        // Clear screen using ANSI escape codes
                        print!("\x1B[2J\x1B[1;1H");
                        std::io::Write::flush(&mut std::io::stdout()).ok();
                        continue;
                    }
                    ".clear-stack" | "clear-stack" => {
                        // Clear just the stack
                        eval.clear_stack();
                        {
                            let mut state = shared_state.lock().unwrap();
                            state.clear();
                        }
                        continue;
                    }
                    ".clear-screen" | "clear-screen" => {
                        // Clear just the screen
                        print!("\x1B[2J\x1B[1;1H");
                        std::io::Write::flush(&mut std::io::stdout()).ok();
                        continue;
                    }
                    ".pop" | ".p" => {
                        // Pop and display top of stack
                        if let Some(value) = eval.pop_value() {
                            println!("{:?}", value);
                        } else {
                            println!("Stack empty");
                        }
                        continue;
                    }
                    ".peek" | ".k" => {
                        // Show top of stack without popping
                        if let Some(value) = eval.stack().last() {
                            if let Some(s) = value.as_arg() {
                                println!("{}", s);
                            } else {
                                println!("{:?}", value);
                            }
                        } else {
                            println!("Stack empty");
                        }
                        continue;
                    }
                    ".use" | ".u" => {
                        // Move top stack item to input
                        let items = eval.pop_n_as_string(1);
                        if !items.is_empty() {
                            prefill = items;
                        } else {
                            println!("Stack empty");
                        }
                        continue;
                    }
                    ".types" | ".t" => {
                        // Toggle type annotations in hint
                        let mut state = shared_state.lock().unwrap();
                        state.show_types = !state.show_types;
                        println!("Type annotations: {}", if state.show_types { "ON" } else { "OFF" });
                        continue;
                    }
                    ".hint" => {
                        // Toggle hint visibility
                        let mut state = shared_state.lock().unwrap();
                        state.hint_visible = !state.hint_visible;
                        println!("Hint: {}", if state.hint_visible { "ON" } else { "OFF" });
                        continue;
                    }
                    // === Debugger commands ===
                    ".debug" | ".d" => {
                        // Toggle debug mode
                        let new_state = !eval.is_debug_mode();
                        eval.set_debug_mode(new_state);
                        if new_state {
                            eval.set_step_mode(true); // Start in step mode
                            println!("Debug mode: ON (step mode enabled)");
                            println!("  Use .break <pattern> to set breakpoints");
                            println!("  When paused: (n)ext, (c)ontinue, (s)tack, (q)uit");
                        } else {
                            println!("Debug mode: OFF");
                        }
                        continue;
                    }
                    ".step" => {
                        // Enable step mode (only useful if debug mode is on)
                        if eval.is_debug_mode() {
                            eval.set_step_mode(true);
                            println!("Step mode: ON (will pause on next expression)");
                        } else {
                            println!("Enable debug mode first with .debug");
                        }
                        continue;
                    }
                    ".breakpoints" | ".bl" => {
                        // List all breakpoints
                        let bps = eval.breakpoints();
                        if bps.is_empty() {
                            println!("No breakpoints set");
                        } else {
                            println!("Breakpoints ({}):", bps.len());
                            for bp in bps {
                                println!("  - {}", bp);
                            }
                        }
                        continue;
                    }
                    ".clearbreaks" | ".cb" => {
                        // Clear all breakpoints
                        eval.clear_breakpoints();
                        println!("All breakpoints cleared");
                        continue;
                    }
                    _ if trimmed.starts_with(".break ") || trimmed.starts_with(".b ") => {
                        // Add a breakpoint
                        let pattern = trimmed.strip_prefix(".break ")
                            .or_else(|| trimmed.strip_prefix(".b "))
                            .unwrap_or("")
                            .to_string();
                        if pattern.is_empty() {
                            println!("Usage: .break <pattern>");
                            println!("  Pattern matches against expression names (e.g., 'echo', 'dup', 'if')");
                        } else {
                            eval.add_breakpoint(pattern.clone());
                            // Auto-enable debug mode if not already on
                            if !eval.is_debug_mode() {
                                eval.set_debug_mode(true);
                                println!("Debug mode auto-enabled");
                            }
                            println!("Breakpoint set: {}", pattern);
                        }
                        continue;
                    }
                    _ if trimmed.starts_with(".delbreak ") || trimmed.starts_with(".db ") => {
                        // Remove a breakpoint
                        let pattern = trimmed.strip_prefix(".delbreak ")
                            .or_else(|| trimmed.strip_prefix(".db "))
                            .unwrap_or("");
                        if eval.remove_breakpoint(pattern) {
                            println!("Breakpoint removed: {}", pattern);
                        } else {
                            println!("Breakpoint not found: {}", pattern);
                        }
                        continue;
                    }
                    _ if trimmed.starts_with(".use=") || trimmed.starts_with(".u=") => {
                        // Move N stack items to input
                        let n_str = trimmed.strip_prefix(".use=")
                            .or_else(|| trimmed.strip_prefix(".u="))
                            .unwrap_or("");
                        match n_str.parse::<usize>() {
                            Ok(n) => {
                                let items = eval.pop_n_as_string(n);
                                if !items.is_empty() {
                                    prefill = items;
                                } else {
                                    println!("Stack empty");
                                }
                            }
                            Err(_) => {
                                eprintln!("Invalid number: {}", n_str);
                            }
                        }
                        continue;
                    }
                    _ => {}
                }

                // Transfer limbo from SharedState to evaluator before execution
                {
                    let mut state = shared_state.lock().unwrap();
                    for (id, value) in state.limbo.drain() {
                        eval.limbo.insert(id, value);
                    }
                }

                // Execute the line
                let result = execute_line(&mut eval, trimmed, true);

                // Clear limbo and pending state after execution (refs are consumed or lost)
                {
                    let mut state = shared_state.lock().unwrap();
                    state.clear();
                }
                eval.clear_limbo();

                match result {
                    Ok(exit_code) => {
                        // Increment command counter
                        cmd_num += 1;
                        // Stack persists between lines - use .use to move items to input
                        if exit_code != 0 {
                            eprintln!("Exit code: {}", exit_code);
                        }
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                // Ctrl-C - return limbo values to stack, clear pending state, continue
                prefill.clear();
                {
                    let mut state = shared_state.lock().unwrap();
                    // Return limbo values to the real stack in reverse order
                    let mut limbo_items: Vec<_> = state.limbo.drain().collect();
                    // Sort by ID to get deterministic order (IDs are hex counters)
                    limbo_items.sort_by_key(|(id, _)| id.clone());
                    // Push in reverse so lowest ID ends up on bottom of stack
                    for (_, value) in limbo_items.into_iter().rev() {
                        eval.push_value(value);
                    }
                    state.limbo_counter = 0;
                    state.clear();
                }
                continue;
            }
            Err(ReadlineError::Eof) => {
                // Ctrl-D - exit
                break;
            }
            Err(err) => {
                eprintln!("Error: {:?}", err);
                break;
            }
        }
    }

    // Save history
    if let Some(ref path) = history_path {
        let _ = rl.save_history(path);
    }

    Ok(())
}
