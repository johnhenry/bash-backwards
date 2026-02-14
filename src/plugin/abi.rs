//! Plugin ABI constants and memory helpers
//!
//! This module defines the ABI (Application Binary Interface) for communication
//! between hsab and WASM plugins. It provides utilities for memory management
//! and data exchange.
//!
//! Note: Many items are intentionally public for use by external WASM plugins,
//! even if not used internally by hsab.

#![allow(dead_code)]

use wasmer::{Memory, MemoryView, StoreMut, WasmPtr};

/// Maximum size for string buffers in plugin communication
pub const MAX_STRING_LEN: u32 = 65536; // 64KB

/// Maximum size for JSON data in plugin communication
pub const MAX_JSON_LEN: u32 = 1048576; // 1MB

/// Plugin return codes - used by WASM plugins to communicate status
pub mod return_codes {
    /// Success
    pub const SUCCESS: i32 = 0;
    /// General error
    pub const ERROR: i32 = 1;
    /// Command not found
    pub const CMD_NOT_FOUND: i32 = 2;
    /// Invalid arguments
    pub const INVALID_ARGS: i32 = 3;
    /// Stack underflow
    pub const STACK_UNDERFLOW: i32 = 4;
    /// Type error
    pub const TYPE_ERROR: i32 = 5;
    /// IO error
    pub const IO_ERROR: i32 = 6;
}

/// Read a string from WASM memory
pub fn read_string(memory: &Memory, store: &StoreMut, ptr: u32, len: u32) -> Option<String> {
    if len == 0 {
        return Some(String::new());
    }
    if len > MAX_STRING_LEN {
        return None;
    }

    let view: MemoryView = memory.view(store);
    let mut buffer = vec![0u8; len as usize];

    let wasm_ptr: WasmPtr<u8> = WasmPtr::new(ptr);
    let slice = wasm_ptr.slice(&view, len).ok()?;
    slice.read_slice(&mut buffer).ok()?;

    String::from_utf8(buffer).ok()
}

/// Write a string to WASM memory, returning the number of bytes written
pub fn write_string(
    memory: &Memory,
    store: &StoreMut,
    ptr: u32,
    max_len: u32,
    data: &str,
) -> u32 {
    let bytes = data.as_bytes();
    let write_len = std::cmp::min(bytes.len(), max_len as usize);

    if write_len == 0 {
        return 0;
    }

    let view: MemoryView = memory.view(store);
    let wasm_ptr: WasmPtr<u8> = WasmPtr::new(ptr);

    if let Ok(slice) = wasm_ptr.slice(&view, write_len as u32) {
        if slice.write_slice(&bytes[..write_len]).is_ok() {
            return write_len as u32;
        }
    }

    0
}

/// Read bytes from WASM memory
pub fn read_bytes(memory: &Memory, store: &StoreMut, ptr: u32, len: u32) -> Option<Vec<u8>> {
    if len == 0 {
        return Some(Vec::new());
    }
    if len > MAX_JSON_LEN {
        return None;
    }

    let view: MemoryView = memory.view(store);
    let mut buffer = vec![0u8; len as usize];

    let wasm_ptr: WasmPtr<u8> = WasmPtr::new(ptr);
    let slice = wasm_ptr.slice(&view, len).ok()?;
    slice.read_slice(&mut buffer).ok()?;

    Some(buffer)
}

/// Write bytes to WASM memory, returning the number of bytes written
pub fn write_bytes(
    memory: &Memory,
    store: &StoreMut,
    ptr: u32,
    max_len: u32,
    data: &[u8],
) -> u32 {
    let write_len = std::cmp::min(data.len(), max_len as usize);

    if write_len == 0 {
        return 0;
    }

    let view: MemoryView = memory.view(store);
    let wasm_ptr: WasmPtr<u8> = WasmPtr::new(ptr);

    if let Ok(slice) = wasm_ptr.slice(&view, write_len as u32) {
        if slice.write_slice(&data[..write_len]).is_ok() {
            return write_len as u32;
        }
    }

    0
}

/// Convert an hsab Value to JSON string for passing to plugins
pub fn value_to_json(value: &crate::Value) -> String {
    use crate::Value;

    match value {
        Value::Literal(s) => serde_json::to_string(s).unwrap_or_else(|_| "null".to_string()),
        Value::Output(s) => serde_json::to_string(s).unwrap_or_else(|_| "null".to_string()),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Nil => "null".to_string(),
        Value::Block(exprs) => {
            // Blocks are represented as a special object
            format!(r#"{{"__type":"block","exprs":{}}}"#, exprs.len())
        }
        Value::Marker => r#"{"__type":"marker"}"#.to_string(),
        Value::Map(map) => {
            // Convert map to JSON object
            let mut obj = serde_json::Map::new();
            for (k, v) in map {
                if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&value_to_json(v)) {
                    obj.insert(k.clone(), json_val);
                }
            }
            serde_json::to_string(&obj).unwrap_or_else(|_| "{}".to_string())
        }
        Value::List(items) => {
            let json_items: Vec<String> = items.iter().map(value_to_json).collect();
            format!("[{}]", json_items.join(","))
        }
        Value::Table { columns, rows } => {
            let json_obj = serde_json::json!({
                "__type": "table",
                "columns": columns,
                "rows": rows.iter().map(|row| {
                    row.iter().map(value_to_json).collect::<Vec<_>>()
                }).collect::<Vec<_>>()
            });
            serde_json::to_string(&json_obj).unwrap_or_else(|_| "null".to_string())
        }
        Value::Error { kind, message, code, source, command } => {
            let mut json_obj = serde_json::Map::new();
            json_obj.insert("__type".to_string(), serde_json::json!("error"));
            json_obj.insert("kind".to_string(), serde_json::json!(kind));
            json_obj.insert("message".to_string(), serde_json::json!(message));
            if let Some(c) = code {
                json_obj.insert("code".to_string(), serde_json::json!(c));
            }
            if let Some(s) = source {
                json_obj.insert("source".to_string(), serde_json::json!(s));
            }
            if let Some(c) = command {
                json_obj.insert("command".to_string(), serde_json::json!(c));
            }
            serde_json::to_string(&json_obj).unwrap_or_else(|_| "null".to_string())
        }
        Value::Media { mime_type, data, width, height, alt, source } => {
            use base64::{Engine as _, engine::general_purpose::STANDARD};
            let mut json_obj = serde_json::Map::new();
            json_obj.insert("__type".to_string(), serde_json::json!("media"));
            json_obj.insert("mime_type".to_string(), serde_json::json!(mime_type));
            json_obj.insert("data".to_string(), serde_json::json!(STANDARD.encode(data)));
            json_obj.insert("size".to_string(), serde_json::json!(data.len()));
            if let Some(w) = width {
                json_obj.insert("width".to_string(), serde_json::json!(w));
            }
            if let Some(h) = height {
                json_obj.insert("height".to_string(), serde_json::json!(h));
            }
            if let Some(a) = alt {
                json_obj.insert("alt".to_string(), serde_json::json!(a));
            }
            if let Some(s) = source {
                json_obj.insert("source".to_string(), serde_json::json!(s));
            }
            serde_json::to_string(&json_obj).unwrap_or_else(|_| "null".to_string())
        }
        Value::Link { url, text } => {
            let mut json_obj = serde_json::Map::new();
            json_obj.insert("__type".to_string(), serde_json::json!("link"));
            json_obj.insert("url".to_string(), serde_json::json!(url));
            if let Some(t) = text {
                json_obj.insert("text".to_string(), serde_json::json!(t));
            }
            serde_json::to_string(&json_obj).unwrap_or_else(|_| "null".to_string())
        }
        Value::Bytes(data) => {
            use base64::{Engine as _, engine::general_purpose::STANDARD};
            let mut json_obj = serde_json::Map::new();
            json_obj.insert("__type".to_string(), serde_json::json!("bytes"));
            json_obj.insert("data".to_string(), serde_json::json!(STANDARD.encode(data)));
            json_obj.insert("size".to_string(), serde_json::json!(data.len()));
            json_obj.insert("hex".to_string(), serde_json::json!(hex::encode(data)));
            serde_json::to_string(&json_obj).unwrap_or_else(|_| "null".to_string())
        }
        Value::BigInt(n) => {
            let mut json_obj = serde_json::Map::new();
            json_obj.insert("__type".to_string(), serde_json::json!("bigint"));
            json_obj.insert("decimal".to_string(), serde_json::json!(n.to_string()));
            json_obj.insert("hex".to_string(), serde_json::json!(format!("{:x}", n)));
            serde_json::to_string(&json_obj).unwrap_or_else(|_| "null".to_string())
        }
    }
}

/// Parse a JSON string into an hsab Value
pub fn json_to_value(json: &str) -> Option<crate::Value> {
    let parsed: serde_json::Value = serde_json::from_str(json).ok()?;

    Some(json_value_to_hsab_value(&parsed))
}

fn json_value_to_hsab_value(json: &serde_json::Value) -> crate::Value {
    use crate::Value;

    match json {
        serde_json::Value::Null => Value::Nil,
        serde_json::Value::Bool(b) => Value::Bool(*b),
        serde_json::Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                Value::Number(f)
            } else {
                Value::Literal(n.to_string())
            }
        }
        serde_json::Value::String(s) => Value::Literal(s.clone()),
        serde_json::Value::Array(arr) => {
            let items: Vec<Value> = arr.iter().map(json_value_to_hsab_value).collect();
            Value::List(items)
        }
        serde_json::Value::Object(obj) => {
            // Check for special types
            if let Some(type_str) = obj.get("__type").and_then(|v| v.as_str()) {
                match type_str {
                    "marker" => return Value::Marker,
                    "error" => {
                        let kind = obj.get("kind")
                            .and_then(|v| v.as_str())
                            .unwrap_or("error")
                            .to_string();
                        let message = obj.get("message")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Unknown error")
                            .to_string();
                        let code = obj.get("code")
                            .and_then(|v| v.as_i64())
                            .map(|c| c as i32);
                        let source = obj.get("source")
                            .and_then(|v| v.as_str())
                            .map(String::from);
                        let command = obj.get("command")
                            .and_then(|v| v.as_str())
                            .map(String::from);
                        return Value::Error {
                            kind,
                            message,
                            code,
                            source,
                            command,
                        };
                    }
                    "table" => {
                        if let (Some(columns), Some(rows)) =
                            (obj.get("columns"), obj.get("rows"))
                        {
                            let columns: Vec<String> = columns
                                .as_array()
                                .map(|arr| {
                                    arr.iter()
                                        .filter_map(|v| v.as_str().map(String::from))
                                        .collect()
                                })
                                .unwrap_or_default();
                            let rows: Vec<Vec<Value>> = rows
                                .as_array()
                                .map(|arr| {
                                    arr.iter()
                                        .map(|row| {
                                            row.as_array()
                                                .map(|r| {
                                                    r.iter()
                                                        .map(json_value_to_hsab_value)
                                                        .collect()
                                                })
                                                .unwrap_or_default()
                                        })
                                        .collect()
                                })
                                .unwrap_or_default();
                            return Value::Table { columns, rows };
                        }
                    }
                    _ => {}
                }
            }

            // Regular object -> Map with Value entries
            let map: std::collections::HashMap<String, Value> =
                obj.iter()
                    .map(|(k, v)| (k.clone(), json_value_to_hsab_value(v)))
                    .collect();
            Value::Map(map)
        }
    }
}
