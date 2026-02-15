# Plugin System

hsab supports WASM-based plugins that extend the shell with custom commands. Plugins have full system access via WASI and support hot reloading for rapid development.

## Overview

- **WASM-based**: Plugins compile to WebAssembly, enabling any language that targets WASM (Rust, C, Go, AssemblyScript, etc.)
- **Full system access**: WASI provides filesystem, environment variables, stdin/stdout/stderr access
- **Hot reload**: Changes to `.wasm` files are detected automatically and plugins reload without restarting hsab
- **Dependency resolution**: Plugins can depend on other plugins with semver version requirements
- **Configuration**: TOML-based configuration with user overrides

## Plugin Structure

Plugins live in `~/.hsab/plugins/`. Each plugin can be either a directory with a manifest or a standalone `.wasm` file.

```
~/.hsab/plugins/
├── http-client/           # Directory-based plugin
│   ├── plugin.toml        # Manifest (required metadata)
│   ├── http_client.wasm   # WASM binary
│   └── config.toml        # User config overrides (optional)
├── json-utils/
│   ├── plugin.toml
│   └── json_utils.wasm
└── my-tool.wasm           # Standalone plugin (no manifest)
```

### Standalone Plugins

A single `.wasm` file can be placed directly in the plugins directory. hsab will:
- Use the filename (without extension) as the plugin name
- Create a default command mapping the filename to `hsab_call`
- Apply default WASI configuration (full access)

## Manifest Format (plugin.toml)

The manifest describes the plugin's metadata, commands, dependencies, and runtime configuration.

### Complete Example

```toml
[plugin]
name = "http-client"
version = "1.0.0"
description = "HTTP client for hsab with GET, POST, PUT, DELETE"
author = "Your Name"
wasm = "http_client.wasm"

[commands]
# Map command names to exported WASM function names
http-get = "cmd_get"
http-post = "cmd_post"
http-put = "cmd_put"
http-delete = "cmd_delete"

[dependencies]
# Other plugins this plugin depends on (optional)
json-utils = ">=1.0.0"

[config]
# Default configuration values
timeout = 30
user_agent = "hsab-http/1.0"
follow_redirects = true

[wasi]
# WASI runtime configuration (all default to true)
inherit_env = true
inherit_args = true
inherit_stdin = true
inherit_stdout = true
inherit_stderr = true

# Filesystem preopens (optional)
preopens = [
    { host = ".", guest = "/" },
    { host = "/tmp", guest = "/sandbox" }
]
```

### Required Fields

| Field | Description |
|-------|-------------|
| `plugin.name` | Plugin name (used for dependency resolution) |
| `plugin.version` | Semantic version (e.g., "1.0.0") |
| `plugin.wasm` | WASM binary filename (relative to plugin directory) |

### Optional Fields

| Field | Description |
|-------|-------------|
| `plugin.description` | Human-readable description |
| `plugin.author` | Plugin author |
| `commands` | Map of command names to WASM function names |
| `dependencies` | Map of plugin names to semver requirements |
| `config` | Default configuration values |
| `wasi` | WASI runtime configuration |

### Version Requirements

Dependencies use semver version requirements:

| Syntax | Meaning |
|--------|---------|
| `^1.2.3` | Compatible updates (>=1.2.3, <2.0.0) |
| `~1.2.3` | Patch updates only (>=1.2.3, <1.3.0) |
| `=1.2.3` | Exact version |
| `>=1.0.0, <2.0.0` | Range |
| `1.*` | Any 1.x version |
| `*` | Any version |

## Creating a Plugin

### Rust Setup for WASM

1. Create a new Rust library project:

```bash
cargo new --lib my-plugin
cd my-plugin
```

2. Configure `Cargo.toml`:

```toml
[package]
name = "my-plugin"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
# Add any dependencies you need
```

3. Add the WASM target:

```bash
rustup target add wasm32-wasip1
```

### Required Exports

Your plugin must export functions that hsab can call. Each exported function receives:
- `cmd_ptr: i32` - Pointer to command name string
- `cmd_len: i32` - Length of command name
- `args_ptr: i32` - Pointer to JSON array of arguments
- `args_len: i32` - Length of JSON arguments

Returns an `i32` status code (0 = success).

```rust
use std::slice;
use std::str;

// Read a string from WASM memory
unsafe fn read_str(ptr: i32, len: i32) -> &'static str {
    let slice = slice::from_raw_parts(ptr as *const u8, len as usize);
    str::from_utf8_unchecked(slice)
}

#[no_mangle]
pub extern "C" fn cmd_hello(
    cmd_ptr: i32,
    cmd_len: i32,
    args_ptr: i32,
    args_len: i32,
) -> i32 {
    let _cmd = unsafe { read_str(cmd_ptr, cmd_len) };
    let args_json = unsafe { read_str(args_ptr, args_len) };

    // Parse args as JSON array
    // args_json will be something like: ["arg1", "arg2"]

    // Use host functions to interact with hsab
    unsafe {
        let msg = "Hello from plugin!";
        hsab_print(msg.as_ptr() as i32, msg.len() as i32);
    }

    0 // Success
}

// Optional: initialization when plugin loads
#[no_mangle]
pub extern "C" fn hsab_plugin_init() {
    // Setup code here
}

// Optional: cleanup when plugin unloads
#[no_mangle]
pub extern "C" fn hsab_plugin_cleanup() {
    // Cleanup code here
}

// Declare host functions (imported from hsab)
extern "C" {
    fn hsab_print(ptr: i32, len: i32);
    fn hsab_stack_push_string(ptr: i32, len: i32);
    fn hsab_stack_pop_string(out_ptr: i32, max_len: i32) -> i32;
    // ... see Host Functions section for complete list
}
```

### Building with Cargo

```bash
cargo build --target wasm32-wasip1 --release
```

The compiled WASM binary will be at `target/wasm32-wasip1/release/my_plugin.wasm`.

Copy to your plugin directory:

```bash
mkdir -p ~/.hsab/plugins/my-plugin
cp target/wasm32-wasip1/release/my_plugin.wasm ~/.hsab/plugins/my-plugin/
```

## Host Functions

Plugins import host functions from the `env` namespace to interact with hsab. These are defined in `src/plugin/imports.rs`.

### Stack Operations

| Function | Signature | Description |
|----------|-----------|-------------|
| `hsab_stack_push_string` | `(ptr: i32, len: i32)` | Push a string onto the stack |
| `hsab_stack_push_number` | `(value: f64)` | Push a number onto the stack |
| `hsab_stack_push_bool` | `(value: i32)` | Push a boolean (0 = false, non-0 = true) |
| `hsab_stack_push_null` | `()` | Push nil onto the stack |
| `hsab_stack_push_json` | `(ptr: i32, len: i32)` | Push a JSON value (parsed into hsab types) |
| `hsab_stack_pop_string` | `(out_ptr: i32, max_len: i32) -> i32` | Pop string, returns bytes written |
| `hsab_stack_pop_number` | `() -> f64` | Pop a number (NaN if not a number) |
| `hsab_stack_pop_bool` | `() -> i32` | Pop a boolean |
| `hsab_stack_pop_json` | `(out_ptr: i32, max_len: i32) -> i32` | Pop any value as JSON |
| `hsab_stack_len` | `() -> i32` | Get current stack depth |
| `hsab_stack_peek_json` | `(index: i32, out_ptr: i32, max_len: i32) -> i32` | Peek at stack position (0 = top) |

### Environment Operations

| Function | Signature | Description |
|----------|-----------|-------------|
| `hsab_env_get` | `(name_ptr: i32, name_len: i32, out_ptr: i32, max_len: i32) -> i32` | Get environment variable |
| `hsab_env_set` | `(name_ptr: i32, name_len: i32, val_ptr: i32, val_len: i32)` | Set environment variable |
| `hsab_cwd` | `(out_ptr: i32, max_len: i32) -> i32` | Get current working directory |
| `hsab_chdir` | `(path_ptr: i32, path_len: i32)` | Change working directory |

### Output Operations

| Function | Signature | Description |
|----------|-----------|-------------|
| `hsab_print` | `(ptr: i32, len: i32)` | Print to stdout |
| `hsab_eprint` | `(ptr: i32, len: i32)` | Print to stderr |

### Configuration Operations

| Function | Signature | Description |
|----------|-----------|-------------|
| `hsab_config_get` | `(key_ptr: i32, key_len: i32, out_ptr: i32, max_len: i32) -> i32` | Get plugin config value |

### Return Codes

Use these standard return codes from your command functions:

| Code | Constant | Meaning |
|------|----------|---------|
| 0 | `SUCCESS` | Success |
| 1 | `ERROR` | General error |
| 2 | `CMD_NOT_FOUND` | Command not found |
| 3 | `INVALID_ARGS` | Invalid arguments |
| 4 | `STACK_UNDERFLOW` | Not enough values on stack |
| 5 | `TYPE_ERROR` | Wrong type on stack |
| 6 | `IO_ERROR` | I/O error |

### Buffer Sizes

| Constant | Value | Description |
|----------|-------|-------------|
| `MAX_STRING_LEN` | 65536 (64KB) | Maximum string buffer size |
| `MAX_JSON_LEN` | 1048576 (1MB) | Maximum JSON buffer size |

## Hot Reload

hsab automatically watches the `~/.hsab/plugins/` directory for changes.

### Automatic Detection

Changes to these files trigger automatic reload:
- `.wasm` files (the plugin binary)
- `plugin.toml` (the manifest)
- `config.toml` (user configuration)

The file watcher polls every 2 seconds by default.

### Manual Reload

Reload a specific plugin:

```bash
> .plugin-reload http-client
```

List loaded plugins:

```bash
> .plugins
```

### Reload Lifecycle

When a plugin reloads:
1. `hsab_plugin_cleanup()` is called on the old instance (if exported)
2. The old plugin is unloaded, commands are unregistered
3. The new WASM binary is loaded and instantiated
4. Commands are re-registered
5. `hsab_plugin_init()` is called on the new instance (if exported)

### Limitations

- **State is lost**: Plugin memory is reset on reload. Use files or hsab's stack for persistence.
- **In-flight operations**: Commands executing during reload may fail.
- **Dependencies**: Reloading a plugin that others depend on may cause issues.

## Configuration

Plugins access configuration through the `hsab_config_get` host function.

### Default Configuration

Define defaults in `plugin.toml`:

```toml
[config]
timeout = 30
base_url = "https://api.example.com"
debug = false
retry_count = 3
headers = ["Content-Type: application/json"]
```

### User Overrides

Users can override defaults by creating `config.toml` in the plugin directory:

```toml
# ~/.hsab/plugins/http-client/config.toml
timeout = 60
debug = true
```

User values override plugin defaults. Missing keys fall back to defaults.

### Accessing Configuration from Plugin

```rust
extern "C" {
    fn hsab_config_get(key_ptr: i32, key_len: i32, out_ptr: i32, max_len: i32) -> i32;
}

fn get_config(key: &str) -> Option<String> {
    let mut buffer = [0u8; 1024];
    let len = unsafe {
        hsab_config_get(
            key.as_ptr() as i32,
            key.len() as i32,
            buffer.as_mut_ptr() as i32,
            buffer.len() as i32,
        )
    };
    if len > 0 {
        Some(String::from_utf8_lossy(&buffer[..len as usize]).to_string())
    } else {
        None
    }
}

// Usage
let timeout: u32 = get_config("timeout")
    .and_then(|s| s.parse().ok())
    .unwrap_or(30);
```

### Supported Config Types

| TOML Type | Returned As |
|-----------|-------------|
| String | String |
| Integer | String (parseable) |
| Float | String (parseable) |
| Boolean | "true" or "false" |
| Array/Table | JSON string |

## Best Practices

### Error Handling

1. **Always check return values** from host functions
2. **Return appropriate error codes** from command functions
3. **Use `hsab_eprint`** for error messages
4. **Validate stack depth** before popping values

```rust
#[no_mangle]
pub extern "C" fn cmd_example(
    _cmd_ptr: i32, _cmd_len: i32,
    _args_ptr: i32, _args_len: i32,
) -> i32 {
    // Check stack has enough values
    let depth = unsafe { hsab_stack_len() };
    if depth < 2 {
        let msg = "Error: requires 2 values on stack\n";
        unsafe { hsab_eprint(msg.as_ptr() as i32, msg.len() as i32); }
        return 4; // STACK_UNDERFLOW
    }

    // Pop values
    let mut buf = [0u8; 1024];
    let len = unsafe { hsab_stack_pop_string(buf.as_mut_ptr() as i32, buf.len() as i32) };
    if len == 0 {
        let msg = "Error: failed to pop value\n";
        unsafe { hsab_eprint(msg.as_ptr() as i32, msg.len() as i32); }
        return 1; // ERROR
    }

    // ... process ...

    0 // SUCCESS
}
```

### Documentation

1. **Write a clear description** in `plugin.toml`
2. **Document each command** - what it expects on the stack, what it produces
3. **Include examples** in a README if distributing the plugin

### Testing Plugins

1. **Unit test Rust code** before compiling to WASM
2. **Test with hsab** using `.plugin-load` for manual testing
3. **Use `--trace` mode** to see stack changes: `hsab --trace`
4. **Test hot reload** by modifying and rebuilding during a session

### Performance

1. **Minimize host function calls** - batch operations when possible
2. **Use JSON for complex data** rather than multiple string operations
3. **Allocate buffers appropriately** - too small causes truncation, too large wastes memory

### Security Considerations

1. **Plugins have full WASI access** - they can read/write files, access environment variables
2. **Only load trusted plugins** - WASM sandboxing does not restrict WASI capabilities
3. **Review filesystem preopens** - limit access when possible in `plugin.toml`

## Plugin Commands

hsab provides built-in commands for managing plugins:

| Command | Description |
|---------|-------------|
| `.plugins` | List all loaded plugins |
| `.plugin-load <path>` | Load a plugin from path |
| `.plugin-unload <name>` | Unload a plugin by name |
| `.plugin-reload <name>` | Reload a plugin by name |
| `.plugin-info <name>` | Show plugin details |

## Example: Complete Plugin

Here's a complete example of a plugin that provides a `reverse` command:

### plugin.toml

```toml
[plugin]
name = "string-utils"
version = "1.0.0"
description = "String manipulation utilities"
author = "Example Author"
wasm = "string_utils.wasm"

[commands]
reverse = "cmd_reverse"
uppercase = "cmd_uppercase"
```

### src/lib.rs

```rust
use std::slice;
use std::str;

extern "C" {
    fn hsab_stack_pop_string(out_ptr: i32, max_len: i32) -> i32;
    fn hsab_stack_push_string(ptr: i32, len: i32);
    fn hsab_eprint(ptr: i32, len: i32);
    fn hsab_stack_len() -> i32;
}

const BUFFER_SIZE: usize = 65536;

unsafe fn read_str(ptr: i32, len: i32) -> &'static str {
    let slice = slice::from_raw_parts(ptr as *const u8, len as usize);
    str::from_utf8_unchecked(slice)
}

fn pop_string() -> Option<String> {
    let mut buffer = vec![0u8; BUFFER_SIZE];
    let len = unsafe {
        hsab_stack_pop_string(buffer.as_mut_ptr() as i32, BUFFER_SIZE as i32)
    };
    if len > 0 {
        buffer.truncate(len as usize);
        String::from_utf8(buffer).ok()
    } else {
        None
    }
}

fn push_string(s: &str) {
    unsafe {
        hsab_stack_push_string(s.as_ptr() as i32, s.len() as i32);
    }
}

fn print_error(msg: &str) {
    unsafe {
        hsab_eprint(msg.as_ptr() as i32, msg.len() as i32);
    }
}

fn require_stack(n: i32) -> bool {
    let depth = unsafe { hsab_stack_len() };
    if depth < n {
        print_error(&format!("Error: requires {} value(s) on stack\n", n));
        false
    } else {
        true
    }
}

#[no_mangle]
pub extern "C" fn cmd_reverse(
    _cmd_ptr: i32, _cmd_len: i32,
    _args_ptr: i32, _args_len: i32,
) -> i32 {
    if !require_stack(1) {
        return 4; // STACK_UNDERFLOW
    }

    match pop_string() {
        Some(s) => {
            let reversed: String = s.chars().rev().collect();
            push_string(&reversed);
            0 // SUCCESS
        }
        None => {
            print_error("Error: failed to pop string\n");
            1 // ERROR
        }
    }
}

#[no_mangle]
pub extern "C" fn cmd_uppercase(
    _cmd_ptr: i32, _cmd_len: i32,
    _args_ptr: i32, _args_len: i32,
) -> i32 {
    if !require_stack(1) {
        return 4;
    }

    match pop_string() {
        Some(s) => {
            push_string(&s.to_uppercase());
            0
        }
        None => {
            print_error("Error: failed to pop string\n");
            1
        }
    }
}

#[no_mangle]
pub extern "C" fn hsab_plugin_init() {
    // Plugin loaded
}

#[no_mangle]
pub extern "C" fn hsab_plugin_cleanup() {
    // Plugin unloading
}
```

### Building and Installing

```bash
cargo build --target wasm32-wasip1 --release
mkdir -p ~/.hsab/plugins/string-utils
cp target/wasm32-wasip1/release/string_utils.wasm ~/.hsab/plugins/string-utils/
cp plugin.toml ~/.hsab/plugins/string-utils/
```

### Using the Plugin

```bash
> hello reverse
olleh

> hello uppercase
HELLO

> .plugin-info string-utils
Name: string-utils
Version: 1.0.0
Description: String manipulation utilities
Commands: reverse, uppercase
```
