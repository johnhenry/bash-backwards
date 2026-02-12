//! AST for hsab v2 - Stack-based model
//!
//! The AST represents the parsed structure before evaluation.
//! Evaluation happens on a stack where:
//! - Literals push themselves
//! - Executables pop args, run, push output
//! - Blocks are deferred execution units

use std::collections::HashMap;
use serde_json::Value as JsonValue;

/// Convert a Value to a JSON value for serialization
pub fn value_to_json(v: &Value) -> JsonValue {
    match v {
        Value::Literal(s) => JsonValue::String(s.clone()),
        Value::Output(s) => JsonValue::String(s.clone()),
        Value::Number(n) => serde_json::Number::from_f64(*n)
            .map(JsonValue::Number)
            .unwrap_or(JsonValue::Null),
        Value::Bool(b) => JsonValue::Bool(*b),
        Value::Nil => JsonValue::Null,
        Value::List(items) => JsonValue::Array(items.iter().map(value_to_json).collect()),
        Value::Map(map) => JsonValue::Object(
            map.iter()
                .map(|(k, v)| (k.clone(), value_to_json(v)))
                .collect(),
        ),
        Value::Table { columns, rows } => {
            // Convert table to array of objects
            let records: Vec<JsonValue> = rows.iter().map(|row| {
                let obj: serde_json::Map<String, JsonValue> = columns.iter()
                    .zip(row.iter())
                    .map(|(col, val)| (col.clone(), value_to_json(val)))
                    .collect();
                JsonValue::Object(obj)
            }).collect();
            JsonValue::Array(records)
        }
        Value::Error { kind, message, code, source, command } => {
            let mut obj = serde_json::Map::new();
            obj.insert("kind".into(), JsonValue::String(kind.clone()));
            obj.insert("message".into(), JsonValue::String(message.clone()));
            if let Some(c) = code {
                obj.insert("code".into(), JsonValue::Number((*c).into()));
            }
            if let Some(s) = source {
                obj.insert("source".into(), JsonValue::String(s.clone()));
            }
            if let Some(c) = command {
                obj.insert("command".into(), JsonValue::String(c.clone()));
            }
            JsonValue::Object(obj)
        }
        Value::Block(_) | Value::Marker => JsonValue::Null,
    }
}

/// Convert a JSON value to a stack Value
pub fn json_to_value(json: JsonValue) -> Value {
    match json {
        JsonValue::Null => Value::Nil,
        JsonValue::Bool(b) => Value::Bool(b),
        JsonValue::Number(n) => Value::Number(n.as_f64().unwrap_or(0.0)),
        JsonValue::String(s) => Value::Literal(s),
        JsonValue::Array(arr) => {
            Value::List(arr.into_iter().map(json_to_value).collect())
        }
        JsonValue::Object(obj) => {
            let map = obj
                .into_iter()
                .map(|(k, v)| (k, json_to_value(v)))
                .collect();
            Value::Map(map)
        }
    }
}

/// A value that can be on the stack
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// A literal string (argument, path, flag, etc.)
    Literal(String),
    /// Output from a command execution
    Output(String),
    /// A deferred block of expressions
    Block(Vec<Expr>),
    /// Nil - represents empty/no output
    Nil,
    /// Marker - boundary for spread/each/collect operations
    Marker,
    /// A list of values (for structured data)
    List(Vec<Value>),
    /// A map/object of key-value pairs (for structured data)
    Map(HashMap<String, Value>),
    /// A numeric value
    Number(f64),
    /// A boolean value
    Bool(bool),
    /// A table: list of records with consistent columns
    Table {
        columns: Vec<String>,
        rows: Vec<Vec<Value>>,
    },
    /// Structured error with metadata
    Error {
        kind: String,
        message: String,
        code: Option<i32>,
        source: Option<String>,
        command: Option<String>,
    },
}

impl Value {
    /// Convert value to string for use as command argument
    pub fn as_arg(&self) -> Option<String> {
        match self {
            Value::Literal(s) => Some(s.clone()),
            Value::Output(s) => {
                if s.is_empty() {
                    None // Treat empty output as nil
                } else {
                    Some(s.trim_end_matches('\n').to_string())
                }
            }
            Value::Block(_) => None, // Blocks can't be args directly
            Value::Nil => None,
            Value::Marker => None, // Markers can't be args
            Value::Number(n) => {
                // Format nicely - no trailing .0 for integers
                if n.fract() == 0.0 && n.abs() < i64::MAX as f64 {
                    Some(format!("{}", *n as i64))
                } else {
                    Some(n.to_string())
                }
            }
            Value::Bool(b) => Some(b.to_string()),
            Value::List(items) => {
                // Join list items with newlines for shell compatibility
                let parts: Vec<String> = items.iter()
                    .filter_map(|v| v.as_arg())
                    .collect();
                if parts.is_empty() {
                    None
                } else {
                    Some(parts.join("\n"))
                }
            }
            Value::Map(map) => {
                // Check if map is flat (no nested structures)
                let is_flat = map.values().all(|v| matches!(v,
                    Value::Literal(_) | Value::Output(_) | Value::Number(_) |
                    Value::Bool(_) | Value::Nil
                ));

                if is_flat {
                    // Flat map: use key=value format for shell compatibility
                    let mut pairs: Vec<_> = map.iter()
                        .map(|(k, v)| {
                            let val_str = v.as_arg().unwrap_or_default();
                            format!("{}={}", k, val_str)
                        })
                        .collect();
                    pairs.sort(); // Consistent ordering
                    if pairs.is_empty() {
                        None
                    } else {
                        Some(pairs.join("\n"))
                    }
                } else {
                    // Nested map: use JSON for shell compatibility
                    let json: serde_json::Map<String, serde_json::Value> = map.iter()
                        .map(|(k, v)| (k.clone(), value_to_json(v)))
                        .collect();
                    serde_json::to_string(&json).ok()
                }
            }
            Value::Table { columns, rows } => {
                // Convert table to TSV for shell compatibility
                let mut lines = vec![columns.join("\t")];
                for row in rows {
                    let line: Vec<String> = row.iter()
                        .map(|v| v.as_arg().unwrap_or_default())
                        .collect();
                    lines.push(line.join("\t"));
                }
                Some(lines.join("\n"))
            }
            Value::Error { message, .. } => Some(message.clone()),
        }
    }

    /// Check if this is nil or empty
    pub fn is_nil(&self) -> bool {
        match self {
            Value::Nil => true,
            Value::Output(s) if s.is_empty() => true,
            _ => false,
        }
    }

    /// Check if this is a marker
    pub fn is_marker(&self) -> bool {
        matches!(self, Value::Marker)
    }
}

/// An expression in the hsab language
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// A literal value (string, path, flag, etc.)
    Literal(String),

    /// A quoted string (preserves quotes in output)
    Quoted { content: String, double: bool },

    /// A variable reference ($VAR or ${VAR})
    Variable(String),

    /// A block/quotation [...] - deferred execution
    Block(Vec<Expr>),

    /// Execute/apply: @ operator
    Apply,

    /// Pipe operator: |
    Pipe,

    /// Redirect operators
    RedirectOut,       // >
    RedirectAppend,    // >>
    RedirectIn,        // <
    RedirectErr,       // 2>
    RedirectErrAppend, // 2>>
    RedirectBoth,      // &>
    RedirectErrToOut,  // 2>&1

    /// Background operator: &
    Background,

    /// Logical operators
    And, // &&
    Or,  // ||

    /// Stack operations
    Dup,
    Swap,
    Drop,
    Over,
    Rot,
    Depth, // Push stack size

    /// Path operations
    Join,
    Suffix,
    Dirname,  // /path/to/file.txt → /path/to
    Basename, // /path/to/file.txt → file (without extension)

    /// String operations
    Split1,  // Split at first occurrence: "a.b.c" "." split1 → "a", "b.c"
    Rsplit1, // Split at last occurrence: "a.b.c" "." rsplit1 → "a.b", "c"

    /// List operations
    Marker,  // Push a marker onto the stack (boundary for each/keep/collect)
    Spread,  // Split multi-line value into separate stack items (pushes marker first)
    Each,    // Apply block to each item on stack (until marker)
    Collect, // Gather stack items back into single value
    Keep,    // Filter: keep items where predicate passes (exit code 0)
    Map,     // [block] map - each + collect (transform items)
    Filter,  // [predicate] filter - keep + collect (filter items)

    /// Control flow
    If,    // [condition] [then] [else] if
    Times, // N [block] times - repeat block N times
    While, // [condition] [body] while - repeat while condition passes
    Until, // [condition] [body] until - repeat until condition passes
    Break, // Exit current loop early

    /// Parallel execution
    Parallel, // [[cmd1] [cmd2] ...] parallel - run blocks in parallel, wait for all
    Fork,     // [cmd1] [cmd2] ... fork - background multiple blocks

    /// Process substitution
    Subst, // [cmd] subst - run cmd, push temp file path (like <(cmd))
    Fifo,  // [cmd] fifo - run cmd, push named pipe path (faster than subst)

    /// JSON / Structured data
    Json,   // Parse JSON string to structured data
    Unjson, // Convert structured data to JSON string

    /// Resource limits
    Timeout, // seconds [cmd] timeout - kill after timeout

    /// Pipeline status
    Pipestatus, // Get exit codes from last pipeline

    /// Module import: "path.hsab" .import or "path.hsab" :alias .import
    Import,

    /// Define a named word: :name (pops block from stack, stores it)
    Define(String),

    /// Scoped variable assignments: ABC=5 DEF=10; body
    /// Assignments are applied before body, then restored/unset after
    ScopedBlock {
        assignments: Vec<(String, String)>,
        body: Vec<Expr>,
    },
}

/// A parsed hsab program is a sequence of expressions
#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub expressions: Vec<Expr>,
}

impl Program {
    pub fn new(expressions: Vec<Expr>) -> Self {
        Program { expressions }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_as_arg() {
        assert_eq!(Value::Literal("hello".into()).as_arg(), Some("hello".into()));
        assert_eq!(Value::Output("world\n".into()).as_arg(), Some("world".into()));
        assert_eq!(Value::Nil.as_arg(), None);
        assert_eq!(Value::Output("".into()).as_arg(), None);
    }

    #[test]
    fn test_value_is_nil() {
        assert!(Value::Nil.is_nil());
        assert!(Value::Output("".into()).is_nil());
        assert!(!Value::Literal("x".into()).is_nil());
        assert!(!Value::Output("x".into()).is_nil());
    }
}
