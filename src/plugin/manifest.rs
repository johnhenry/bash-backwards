//! Plugin manifest parsing (plugin.toml)
//!
//! Each plugin directory can contain a `plugin.toml` manifest file that describes
//! the plugin's metadata, commands, dependencies, and WASI configuration.

use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

use super::PluginError;

/// Plugin manifest structure (plugin.toml)
#[derive(Debug, Clone, Deserialize)]
pub struct PluginManifest {
    /// Plugin metadata
    pub plugin: PluginMeta,

    /// Command mappings: command_name -> exported_function_name
    #[serde(default)]
    pub commands: HashMap<String, String>,

    /// Plugin dependencies: plugin_name -> version_requirement
    #[serde(default)]
    pub dependencies: HashMap<String, String>,

    /// Default configuration values
    #[serde(default)]
    pub config: HashMap<String, toml::Value>,

    /// WASI configuration
    #[serde(default)]
    pub wasi: WasiConfig,
}

/// Plugin metadata
#[derive(Debug, Clone, Deserialize)]
pub struct PluginMeta {
    /// Plugin name (used for dependency resolution)
    pub name: String,

    /// Plugin version (semver)
    pub version: String,

    /// Human-readable description
    #[serde(default)]
    pub description: String,

    /// Plugin author
    #[serde(default)]
    pub author: String,

    /// WASM binary filename (relative to plugin directory)
    pub wasm: String,
}

/// WASI configuration
#[derive(Debug, Clone, Deserialize)]
pub struct WasiConfig {
    /// Inherit environment variables from host
    #[serde(default = "default_true")]
    pub inherit_env: bool,

    /// Inherit command-line arguments
    #[serde(default = "default_true")]
    pub inherit_args: bool,

    /// Inherit stdin from host
    #[serde(default = "default_true")]
    pub inherit_stdin: bool,

    /// Inherit stdout from host
    #[serde(default = "default_true")]
    pub inherit_stdout: bool,

    /// Inherit stderr from host
    #[serde(default = "default_true")]
    pub inherit_stderr: bool,

    /// Filesystem preopens (directory mappings)
    #[serde(default)]
    pub preopens: Vec<PreopenMapping>,
}

impl Default for WasiConfig {
    fn default() -> Self {
        Self {
            inherit_env: true,
            inherit_args: true,
            inherit_stdin: true,
            inherit_stdout: true,
            inherit_stderr: true,
            preopens: Vec::new(),
        }
    }
}

/// Filesystem preopen mapping
#[derive(Debug, Clone, Deserialize)]
pub struct PreopenMapping {
    /// Host filesystem path
    pub host: String,

    /// Guest (WASM) path
    pub guest: String,
}

fn default_true() -> bool {
    true
}

impl PluginManifest {
    /// Load a manifest from a plugin.toml file
    pub fn load(path: &Path) -> Result<Self, PluginError> {
        let content = std::fs::read_to_string(path)?;
        let manifest: PluginManifest = toml::from_str(&content)?;
        Ok(manifest)
    }

    /// Create a default manifest for a standalone WASM file (no plugin.toml)
    pub fn from_wasm_file(wasm_path: &Path) -> Self {
        let name = wasm_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        // Try to infer command name from filename (e.g., "my-plugin.wasm" -> "my-plugin")
        let cmd_name = name.replace('_', "-");

        let mut commands = HashMap::new();
        // Default command handler
        commands.insert(cmd_name, "hsab_call".to_string());

        PluginManifest {
            plugin: PluginMeta {
                name: name.clone(),
                version: "0.0.0".to_string(),
                description: String::new(),
                author: String::new(),
                wasm: wasm_path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("plugin.wasm")
                    .to_string(),
            },
            commands,
            dependencies: HashMap::new(),
            config: HashMap::new(),
            wasi: WasiConfig::default(),
        }
    }

    /// Get user config overrides from config.toml in plugin directory
    pub fn load_user_config(&mut self, plugin_dir: &Path) -> Result<(), PluginError> {
        let config_path = plugin_dir.join("config.toml");
        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            let user_config: HashMap<String, toml::Value> = toml::from_str(&content)?;
            // Merge user config (user values override defaults)
            for (key, value) in user_config {
                self.config.insert(key, value);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_manifest() {
        let toml_content = r#"
[plugin]
name = "test-plugin"
version = "1.0.0"
description = "A test plugin"
wasm = "test.wasm"

[commands]
test-cmd = "cmd_test"
another = "cmd_another"

[dependencies]
other-plugin = ">=1.0.0"

[config]
timeout = 30
name = "test"

[wasi]
inherit_env = true
preopens = [
    { host = ".", guest = "/" }
]
"#;

        let manifest: PluginManifest = toml::from_str(toml_content).unwrap();
        assert_eq!(manifest.plugin.name, "test-plugin");
        assert_eq!(manifest.plugin.version, "1.0.0");
        assert_eq!(manifest.commands.len(), 2);
        assert_eq!(manifest.commands.get("test-cmd"), Some(&"cmd_test".to_string()));
        assert_eq!(manifest.dependencies.len(), 1);
        assert!(manifest.wasi.inherit_env);
        assert_eq!(manifest.wasi.preopens.len(), 1);
    }

    #[test]
    fn test_default_manifest_from_wasm() {
        let path = Path::new("/tmp/my-cool-plugin.wasm");
        let manifest = PluginManifest::from_wasm_file(path);
        assert_eq!(manifest.plugin.name, "my-cool-plugin");
        assert_eq!(manifest.plugin.wasm, "my-cool-plugin.wasm");
        assert!(manifest.commands.contains_key("my-cool-plugin"));
    }
}
