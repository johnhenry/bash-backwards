//! Host function implementations for plugins
//!
//! This module provides the host functions that WASM plugins can import
//! to interact with hsab's stack, environment, and other features.

#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use wasmer::{Function, FunctionEnv, FunctionEnvMut, Imports, Memory, Store};

#[allow(unused_imports)]
use crate::Value;
use super::abi::{read_string, write_string, value_to_json, json_to_value};

/// Shared state between host functions and the plugin host
pub struct PluginEnv {
    /// Reference to WASM memory (set after instantiation)
    pub memory: Arc<RwLock<Option<Memory>>>,

    /// The hsab stack (shared with evaluator)
    pub stack: Arc<Mutex<Vec<Value>>>,

    /// Plugin configuration
    pub config: Arc<RwLock<HashMap<String, toml::Value>>>,

    /// Output buffer for hsab_print
    pub stdout_buffer: Arc<Mutex<String>>,

    /// Error buffer for hsab_eprint
    pub stderr_buffer: Arc<Mutex<String>>,

    /// Current working directory
    pub cwd: Arc<RwLock<std::path::PathBuf>>,

    /// Plugin name (for error messages)
    pub plugin_name: String,
}

impl PluginEnv {
    pub fn new(plugin_name: String, stack: Arc<Mutex<Vec<Value>>>) -> Self {
        Self {
            memory: Arc::new(RwLock::new(None)),
            stack,
            config: Arc::new(RwLock::new(HashMap::new())),
            stdout_buffer: Arc::new(Mutex::new(String::new())),
            stderr_buffer: Arc::new(Mutex::new(String::new())),
            cwd: Arc::new(RwLock::new(
                std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/")),
            )),
            plugin_name,
        }
    }

    pub fn set_memory(&mut self, memory: Memory) {
        if let Ok(mut mem) = self.memory.write() {
            *mem = Some(memory);
        }
    }

    pub fn set_config(&mut self, config: HashMap<String, toml::Value>) {
        if let Ok(mut cfg) = self.config.write() {
            *cfg = config;
        }
    }
}

/// Create the imports object for a plugin
pub fn create_imports(
    store: &mut Store,
    env: &FunctionEnv<PluginEnv>,
) -> Imports {
    let mut imports = Imports::new();

    // Stack operations
    imports.define(
        "env",
        "hsab_stack_push_string",
        Function::new_typed_with_env(store, env, hsab_stack_push_string),
    );
    imports.define(
        "env",
        "hsab_stack_push_number",
        Function::new_typed_with_env(store, env, hsab_stack_push_number),
    );
    imports.define(
        "env",
        "hsab_stack_push_bool",
        Function::new_typed_with_env(store, env, hsab_stack_push_bool),
    );
    imports.define(
        "env",
        "hsab_stack_push_null",
        Function::new_typed_with_env(store, env, hsab_stack_push_null),
    );
    imports.define(
        "env",
        "hsab_stack_push_json",
        Function::new_typed_with_env(store, env, hsab_stack_push_json),
    );
    imports.define(
        "env",
        "hsab_stack_pop_string",
        Function::new_typed_with_env(store, env, hsab_stack_pop_string),
    );
    imports.define(
        "env",
        "hsab_stack_pop_number",
        Function::new_typed_with_env(store, env, hsab_stack_pop_number),
    );
    imports.define(
        "env",
        "hsab_stack_pop_bool",
        Function::new_typed_with_env(store, env, hsab_stack_pop_bool),
    );
    imports.define(
        "env",
        "hsab_stack_pop_json",
        Function::new_typed_with_env(store, env, hsab_stack_pop_json),
    );
    imports.define(
        "env",
        "hsab_stack_len",
        Function::new_typed_with_env(store, env, hsab_stack_len),
    );
    imports.define(
        "env",
        "hsab_stack_peek_json",
        Function::new_typed_with_env(store, env, hsab_stack_peek_json),
    );

    // Environment operations
    imports.define(
        "env",
        "hsab_env_get",
        Function::new_typed_with_env(store, env, hsab_env_get),
    );
    imports.define(
        "env",
        "hsab_env_set",
        Function::new_typed_with_env(store, env, hsab_env_set),
    );
    imports.define(
        "env",
        "hsab_cwd",
        Function::new_typed_with_env(store, env, hsab_cwd),
    );
    imports.define(
        "env",
        "hsab_chdir",
        Function::new_typed_with_env(store, env, hsab_chdir),
    );

    // Output operations
    imports.define(
        "env",
        "hsab_print",
        Function::new_typed_with_env(store, env, hsab_print),
    );
    imports.define(
        "env",
        "hsab_eprint",
        Function::new_typed_with_env(store, env, hsab_eprint),
    );

    // Config operations
    imports.define(
        "env",
        "hsab_config_get",
        Function::new_typed_with_env(store, env, hsab_config_get),
    );

    imports
}

// === Stack Operations ===

fn hsab_stack_push_string(mut env: FunctionEnvMut<PluginEnv>, ptr: u32, len: u32) {
    let (data, store) = env.data_and_store_mut();
    if let Ok(memory_guard) = data.memory.read() {
        if let Some(ref memory) = *memory_guard {
            if let Some(s) = read_string(memory, &store, ptr, len) {
                if let Ok(mut stack) = data.stack.lock() {
                    stack.push(Value::Literal(s));
                }
            }
        }
    }
}

fn hsab_stack_push_number(env: FunctionEnvMut<PluginEnv>, value: f64) {
    if let Ok(mut stack) = env.data().stack.lock() {
        stack.push(Value::Number(value));
    }
}

fn hsab_stack_push_bool(env: FunctionEnvMut<PluginEnv>, value: i32) {
    if let Ok(mut stack) = env.data().stack.lock() {
        stack.push(Value::Bool(value != 0));
    }
}

fn hsab_stack_push_null(env: FunctionEnvMut<PluginEnv>) {
    if let Ok(mut stack) = env.data().stack.lock() {
        stack.push(Value::Nil);
    }
}

fn hsab_stack_push_json(mut env: FunctionEnvMut<PluginEnv>, ptr: u32, len: u32) {
    let (data, store) = env.data_and_store_mut();
    if let Ok(memory_guard) = data.memory.read() {
        if let Some(ref memory) = *memory_guard {
            if let Some(json_str) = read_string(memory, &store, ptr, len) {
                if let Some(value) = json_to_value(&json_str) {
                    if let Ok(mut stack) = data.stack.lock() {
                        stack.push(value);
                    }
                }
            }
        }
    }
}

fn hsab_stack_pop_string(mut env: FunctionEnvMut<PluginEnv>, out_ptr: u32, max_len: u32) -> u32 {
    let (data, store) = env.data_and_store_mut();
    if let Ok(memory_guard) = data.memory.read() {
        if let Some(ref memory) = *memory_guard {
            if let Ok(mut stack) = data.stack.lock() {
                if let Some(value) = stack.pop() {
                    let s = value.as_arg().unwrap_or_default();
                    return write_string(memory, &store, out_ptr, max_len, &s);
                }
            }
        }
    }
    0
}

fn hsab_stack_pop_number(env: FunctionEnvMut<PluginEnv>) -> f64 {
    if let Ok(mut stack) = env.data().stack.lock() {
        if let Some(value) = stack.pop() {
            match value {
                Value::Number(n) => return n,
                Value::Literal(s) | Value::Output(s) => {
                    if let Ok(n) = s.parse::<f64>() {
                        return n;
                    }
                }
                _ => {}
            }
        }
    }
    f64::NAN
}

fn hsab_stack_pop_bool(env: FunctionEnvMut<PluginEnv>) -> i32 {
    if let Ok(mut stack) = env.data().stack.lock() {
        if let Some(value) = stack.pop() {
            match value {
                Value::Bool(b) => return if b { 1 } else { 0 },
                Value::Literal(s) | Value::Output(s) => {
                    return if s == "true" || s == "1" { 1 } else { 0 };
                }
                Value::Number(n) => return if n != 0.0 { 1 } else { 0 },
                _ => {}
            }
        }
    }
    0
}

fn hsab_stack_pop_json(mut env: FunctionEnvMut<PluginEnv>, out_ptr: u32, max_len: u32) -> u32 {
    let (data, store) = env.data_and_store_mut();
    if let Ok(memory_guard) = data.memory.read() {
        if let Some(ref memory) = *memory_guard {
            if let Ok(mut stack) = data.stack.lock() {
                if let Some(value) = stack.pop() {
                    let json_str = value_to_json(&value);
                    return write_string(memory, &store, out_ptr, max_len, &json_str);
                }
            }
        }
    }
    0
}

fn hsab_stack_len(env: FunctionEnvMut<PluginEnv>) -> u32 {
    if let Ok(stack) = env.data().stack.lock() {
        stack.len() as u32
    } else {
        0
    }
}

fn hsab_stack_peek_json(
    mut env: FunctionEnvMut<PluginEnv>,
    index: u32,
    out_ptr: u32,
    max_len: u32,
) -> u32 {
    let (data, store) = env.data_and_store_mut();
    if let Ok(memory_guard) = data.memory.read() {
        if let Some(ref memory) = *memory_guard {
            if let Ok(stack) = data.stack.lock() {
                let len = stack.len();
                if (index as usize) < len {
                    // Index from top of stack (0 = top)
                    let actual_index = len - 1 - (index as usize);
                    let json_str = value_to_json(&stack[actual_index]);
                    return write_string(memory, &store, out_ptr, max_len, &json_str);
                }
            }
        }
    }
    0
}

// === Environment Operations ===

fn hsab_env_get(
    mut env: FunctionEnvMut<PluginEnv>,
    name_ptr: u32,
    name_len: u32,
    out_ptr: u32,
    max_len: u32,
) -> u32 {
    let (data, store) = env.data_and_store_mut();
    if let Ok(memory_guard) = data.memory.read() {
        if let Some(ref memory) = *memory_guard {
            if let Some(name) = read_string(memory, &store, name_ptr, name_len) {
                if let Ok(value) = std::env::var(&name) {
                    return write_string(memory, &store, out_ptr, max_len, &value);
                }
            }
        }
    }
    0
}

fn hsab_env_set(
    mut env: FunctionEnvMut<PluginEnv>,
    name_ptr: u32,
    name_len: u32,
    val_ptr: u32,
    val_len: u32,
) {
    let (data, store) = env.data_and_store_mut();
    if let Ok(memory_guard) = data.memory.read() {
        if let Some(ref memory) = *memory_guard {
            if let (Some(name), Some(value)) = (
                read_string(memory, &store, name_ptr, name_len),
                read_string(memory, &store, val_ptr, val_len),
            ) {
                std::env::set_var(&name, &value);
            }
        }
    }
}

fn hsab_cwd(mut env: FunctionEnvMut<PluginEnv>, out_ptr: u32, max_len: u32) -> u32 {
    let (data, store) = env.data_and_store_mut();
    if let Ok(memory_guard) = data.memory.read() {
        if let Some(ref memory) = *memory_guard {
            if let Ok(cwd) = data.cwd.read() {
                let cwd_str = cwd.display().to_string();
                return write_string(memory, &store, out_ptr, max_len, &cwd_str);
            }
        }
    }
    0
}

fn hsab_chdir(mut env: FunctionEnvMut<PluginEnv>, path_ptr: u32, path_len: u32) {
    let (data, store) = env.data_and_store_mut();
    if let Ok(memory_guard) = data.memory.read() {
        if let Some(ref memory) = *memory_guard {
            if let Some(path_str) = read_string(memory, &store, path_ptr, path_len) {
                let path = std::path::PathBuf::from(&path_str);
                if path.is_dir() {
                    if let Ok(mut cwd) = data.cwd.write() {
                        *cwd = path;
                    }
                }
            }
        }
    }
}

// === Output Operations ===

fn hsab_print(mut env: FunctionEnvMut<PluginEnv>, ptr: u32, len: u32) {
    let (data, store) = env.data_and_store_mut();
    if let Ok(memory_guard) = data.memory.read() {
        if let Some(ref memory) = *memory_guard {
            if let Some(s) = read_string(memory, &store, ptr, len) {
                // Write to stdout immediately
                print!("{}", s);
                // Also buffer for capture mode
                if let Ok(mut buf) = data.stdout_buffer.lock() {
                    buf.push_str(&s);
                }
            }
        }
    }
}

fn hsab_eprint(mut env: FunctionEnvMut<PluginEnv>, ptr: u32, len: u32) {
    let (data, store) = env.data_and_store_mut();
    if let Ok(memory_guard) = data.memory.read() {
        if let Some(ref memory) = *memory_guard {
            if let Some(s) = read_string(memory, &store, ptr, len) {
                // Write to stderr immediately
                eprint!("{}", s);
                // Also buffer
                if let Ok(mut buf) = data.stderr_buffer.lock() {
                    buf.push_str(&s);
                }
            }
        }
    }
}

// === Config Operations ===

fn hsab_config_get(
    mut env: FunctionEnvMut<PluginEnv>,
    key_ptr: u32,
    key_len: u32,
    out_ptr: u32,
    max_len: u32,
) -> u32 {
    let (data, store) = env.data_and_store_mut();
    if let Ok(memory_guard) = data.memory.read() {
        if let Some(ref memory) = *memory_guard {
            if let Some(key) = read_string(memory, &store, key_ptr, key_len) {
                if let Ok(config) = data.config.read() {
                    if let Some(value) = config.get(&key) {
                        let value_str = match value {
                            toml::Value::String(s) => s.clone(),
                            toml::Value::Integer(i) => i.to_string(),
                            toml::Value::Float(f) => f.to_string(),
                            toml::Value::Boolean(b) => b.to_string(),
                            _ => serde_json::to_string(value).unwrap_or_default(),
                        };
                        return write_string(memory, &store, out_ptr, max_len, &value_str);
                    }
                }
            }
        }
    }
    0
}
