//! WASM Plugin System for hsab
//!
//! This module provides a full-featured WASM plugin system using Wasmer runtime
//! with WASI support. Plugins have full system access, support configuration files,
//! dependency resolution, and hot reloading.
//!
//! # Features
//!
//! - **Wasmer Runtime:** Uses Wasmer 4.2 with WASIX support
//! - **Plugin Languages:** Any WASM language via C-style ABI
//! - **Auto-loading:** Plugins auto-load from `~/.hsab/plugins/`
//! - **Hot Reload:** Watches plugin files and reloads on change
//! - **Dependencies:** Plugins can depend on other plugins
//! - **Configuration:** TOML plugin manifest files
//!
//! # Plugin Directory Structure
//!
//! ```text
//! ~/.hsab/plugins/
//! ├── http-client/
//! │   ├── plugin.toml           # Manifest
//! │   ├── http_client.wasm      # WASM binary
//! │   └── config.toml           # User overrides (optional)
//! ├── json-utils/
//! │   ├── plugin.toml
//! │   └── json_utils.wasm
//! └── my-local-plugin.wasm      # Simple single-file plugin (no manifest)
//! ```

mod abi;
mod host;
mod hot_reload;
mod imports;
mod loader;
mod manifest;
mod registry;

pub use host::PluginHost;
pub use manifest::PluginManifest;

/// Error types for the plugin system
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("WASM compilation error: {0}")]
    Compilation(String),

    #[error("WASM instantiation error: {0}")]
    Instantiation(String),

    #[error("WASM runtime error: {0}")]
    Runtime(String),

    #[error("Plugin manifest error: {0}")]
    Manifest(String),

    #[error("Missing dependency: {0}")]
    MissingDependency(String),

    #[error("Version mismatch for {plugin}: requires {required}, found {found}")]
    VersionMismatch {
        plugin: String,
        required: String,
        found: String,
    },

    #[error("Circular dependency detected")]
    CircularDependency,

    #[error("Plugin not found: {0}")]
    NotFound(String),

    #[error("Command not found: {0}")]
    CommandNotFound(String),

    #[error("Plugin call failed: {0}")]
    CallFailed(String),

    #[error("Hot reload error: {0}")]
    HotReload(String),
}

impl From<toml::de::Error> for PluginError {
    fn from(e: toml::de::Error) -> Self {
        PluginError::Manifest(e.to_string())
    }
}
