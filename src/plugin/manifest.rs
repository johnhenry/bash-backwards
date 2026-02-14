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
    use tempfile::TempDir;

    // ==========================================================================
    // Manifest Parsing Tests
    // ==========================================================================

    #[test]
    fn test_parse_manifest_complete() {
        let toml_content = r#"
[plugin]
name = "test-plugin"
version = "1.0.0"
description = "A test plugin"
author = "Test Author"
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
inherit_args = false
inherit_stdin = true
inherit_stdout = true
inherit_stderr = false
preopens = [
    { host = ".", guest = "/" },
    { host = "/tmp", guest = "/sandbox" }
]
"#;

        let manifest: PluginManifest = toml::from_str(toml_content).unwrap();
        assert_eq!(manifest.plugin.name, "test-plugin");
        assert_eq!(manifest.plugin.version, "1.0.0");
        assert_eq!(manifest.plugin.description, "A test plugin");
        assert_eq!(manifest.plugin.author, "Test Author");
        assert_eq!(manifest.plugin.wasm, "test.wasm");
        assert_eq!(manifest.commands.len(), 2);
        assert_eq!(manifest.commands.get("test-cmd"), Some(&"cmd_test".to_string()));
        assert_eq!(manifest.commands.get("another"), Some(&"cmd_another".to_string()));
        assert_eq!(manifest.dependencies.len(), 1);
        assert_eq!(manifest.dependencies.get("other-plugin"), Some(&">=1.0.0".to_string()));
        assert!(manifest.wasi.inherit_env);
        assert!(!manifest.wasi.inherit_args);
        assert!(manifest.wasi.inherit_stdin);
        assert!(manifest.wasi.inherit_stdout);
        assert!(!manifest.wasi.inherit_stderr);
        assert_eq!(manifest.wasi.preopens.len(), 2);
        assert_eq!(manifest.wasi.preopens[0].host, ".");
        assert_eq!(manifest.wasi.preopens[0].guest, "/");
        assert_eq!(manifest.wasi.preopens[1].host, "/tmp");
        assert_eq!(manifest.wasi.preopens[1].guest, "/sandbox");
    }

    #[test]
    fn test_parse_manifest_minimal() {
        let toml_content = r#"
[plugin]
name = "minimal"
version = "0.1.0"
wasm = "minimal.wasm"
"#;

        let manifest: PluginManifest = toml::from_str(toml_content).unwrap();
        assert_eq!(manifest.plugin.name, "minimal");
        assert_eq!(manifest.plugin.version, "0.1.0");
        assert_eq!(manifest.plugin.wasm, "minimal.wasm");
        assert_eq!(manifest.plugin.description, "");
        assert_eq!(manifest.plugin.author, "");
        assert!(manifest.commands.is_empty());
        assert!(manifest.dependencies.is_empty());
        assert!(manifest.config.is_empty());
        // Check WASI defaults
        assert!(manifest.wasi.inherit_env);
        assert!(manifest.wasi.inherit_args);
        assert!(manifest.wasi.inherit_stdin);
        assert!(manifest.wasi.inherit_stdout);
        assert!(manifest.wasi.inherit_stderr);
        assert!(manifest.wasi.preopens.is_empty());
    }

    #[test]
    fn test_parse_manifest_with_config_types() {
        let toml_content = r#"
[plugin]
name = "config-test"
version = "1.0.0"
wasm = "test.wasm"

[config]
string_val = "hello"
int_val = 42
float_val = 3.14
bool_val = true
array_val = [1, 2, 3]
"#;

        let manifest: PluginManifest = toml::from_str(toml_content).unwrap();
        assert_eq!(manifest.config.len(), 5);

        assert_eq!(
            manifest.config.get("string_val").unwrap().as_str(),
            Some("hello")
        );
        assert_eq!(
            manifest.config.get("int_val").unwrap().as_integer(),
            Some(42)
        );
        assert!(
            (manifest.config.get("float_val").unwrap().as_float().unwrap() - 3.14).abs() < 0.001
        );
        assert_eq!(
            manifest.config.get("bool_val").unwrap().as_bool(),
            Some(true)
        );
        assert!(manifest.config.get("array_val").unwrap().as_array().is_some());
    }

    #[test]
    fn test_parse_manifest_multiple_dependencies() {
        let toml_content = r#"
[plugin]
name = "multi-dep"
version = "2.0.0"
wasm = "multi.wasm"

[dependencies]
core = "^1.0.0"
utils = ">=0.5.0, <2.0.0"
optional = "~1.2.3"
exact = "=3.0.0"
"#;

        let manifest: PluginManifest = toml::from_str(toml_content).unwrap();
        assert_eq!(manifest.dependencies.len(), 4);
        assert_eq!(manifest.dependencies.get("core"), Some(&"^1.0.0".to_string()));
        assert_eq!(manifest.dependencies.get("utils"), Some(&">=0.5.0, <2.0.0".to_string()));
        assert_eq!(manifest.dependencies.get("optional"), Some(&"~1.2.3".to_string()));
        assert_eq!(manifest.dependencies.get("exact"), Some(&"=3.0.0".to_string()));
    }

    #[test]
    fn test_parse_manifest_invalid_missing_plugin() {
        let toml_content = r#"
[commands]
test = "cmd_test"
"#;

        let result: Result<PluginManifest, _> = toml::from_str(toml_content);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_manifest_invalid_missing_name() {
        let toml_content = r#"
[plugin]
version = "1.0.0"
wasm = "test.wasm"
"#;

        let result: Result<PluginManifest, _> = toml::from_str(toml_content);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_manifest_invalid_missing_version() {
        let toml_content = r#"
[plugin]
name = "test"
wasm = "test.wasm"
"#;

        let result: Result<PluginManifest, _> = toml::from_str(toml_content);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_manifest_invalid_missing_wasm() {
        let toml_content = r#"
[plugin]
name = "test"
version = "1.0.0"
"#;

        let result: Result<PluginManifest, _> = toml::from_str(toml_content);
        assert!(result.is_err());
    }

    // ==========================================================================
    // Default Manifest from WASM File Tests
    // ==========================================================================

    #[test]
    fn test_default_manifest_from_wasm() {
        let path = Path::new("/tmp/my-cool-plugin.wasm");
        let manifest = PluginManifest::from_wasm_file(path);
        assert_eq!(manifest.plugin.name, "my-cool-plugin");
        assert_eq!(manifest.plugin.wasm, "my-cool-plugin.wasm");
        assert_eq!(manifest.plugin.version, "0.0.0");
        assert_eq!(manifest.plugin.description, "");
        assert_eq!(manifest.plugin.author, "");
        assert!(manifest.commands.contains_key("my-cool-plugin"));
        assert_eq!(manifest.commands.get("my-cool-plugin"), Some(&"hsab_call".to_string()));
        assert!(manifest.dependencies.is_empty());
        assert!(manifest.config.is_empty());
    }

    #[test]
    fn test_default_manifest_from_wasm_with_underscores() {
        let path = Path::new("/plugins/my_plugin_name.wasm");
        let manifest = PluginManifest::from_wasm_file(path);
        // Name keeps underscores
        assert_eq!(manifest.plugin.name, "my_plugin_name");
        assert_eq!(manifest.plugin.wasm, "my_plugin_name.wasm");
        // Command converts underscores to dashes
        assert!(manifest.commands.contains_key("my-plugin-name"));
        assert_eq!(manifest.commands.get("my-plugin-name"), Some(&"hsab_call".to_string()));
    }

    #[test]
    fn test_default_manifest_from_wasm_simple_name() {
        let path = Path::new("plugin.wasm");
        let manifest = PluginManifest::from_wasm_file(path);
        assert_eq!(manifest.plugin.name, "plugin");
        assert_eq!(manifest.plugin.wasm, "plugin.wasm");
        assert!(manifest.commands.contains_key("plugin"));
    }

    #[test]
    fn test_default_manifest_from_wasm_nested_path() {
        let path = Path::new("/home/user/.hsab/plugins/subdir/complex-name.wasm");
        let manifest = PluginManifest::from_wasm_file(path);
        assert_eq!(manifest.plugin.name, "complex-name");
        assert_eq!(manifest.plugin.wasm, "complex-name.wasm");
        assert!(manifest.commands.contains_key("complex-name"));
    }

    #[test]
    fn test_default_manifest_wasi_defaults() {
        let path = Path::new("test.wasm");
        let manifest = PluginManifest::from_wasm_file(path);
        assert!(manifest.wasi.inherit_env);
        assert!(manifest.wasi.inherit_args);
        assert!(manifest.wasi.inherit_stdin);
        assert!(manifest.wasi.inherit_stdout);
        assert!(manifest.wasi.inherit_stderr);
        assert!(manifest.wasi.preopens.is_empty());
    }

    // ==========================================================================
    // WasiConfig Default Tests
    // ==========================================================================

    #[test]
    fn test_wasi_config_default() {
        let wasi = WasiConfig::default();
        assert!(wasi.inherit_env);
        assert!(wasi.inherit_args);
        assert!(wasi.inherit_stdin);
        assert!(wasi.inherit_stdout);
        assert!(wasi.inherit_stderr);
        assert!(wasi.preopens.is_empty());
    }

    #[test]
    fn test_wasi_config_parse_all_false() {
        let toml_content = r#"
[plugin]
name = "test"
version = "1.0.0"
wasm = "test.wasm"

[wasi]
inherit_env = false
inherit_args = false
inherit_stdin = false
inherit_stdout = false
inherit_stderr = false
"#;

        let manifest: PluginManifest = toml::from_str(toml_content).unwrap();
        assert!(!manifest.wasi.inherit_env);
        assert!(!manifest.wasi.inherit_args);
        assert!(!manifest.wasi.inherit_stdin);
        assert!(!manifest.wasi.inherit_stdout);
        assert!(!manifest.wasi.inherit_stderr);
    }

    // ==========================================================================
    // File Loading Tests
    // ==========================================================================

    #[test]
    fn test_load_manifest_from_file() {
        let dir = TempDir::new().unwrap();
        let manifest_path = dir.path().join("plugin.toml");

        let toml_content = r#"
[plugin]
name = "file-test"
version = "1.2.3"
description = "Loaded from file"
wasm = "file-test.wasm"

[commands]
run = "cmd_run"
"#;

        std::fs::write(&manifest_path, toml_content).unwrap();

        let manifest = PluginManifest::load(&manifest_path).unwrap();
        assert_eq!(manifest.plugin.name, "file-test");
        assert_eq!(manifest.plugin.version, "1.2.3");
        assert_eq!(manifest.plugin.description, "Loaded from file");
        assert_eq!(manifest.commands.len(), 1);
    }

    #[test]
    fn test_load_manifest_file_not_found() {
        let result = PluginManifest::load(Path::new("/nonexistent/path/plugin.toml"));
        assert!(result.is_err());
    }

    #[test]
    fn test_load_manifest_invalid_toml() {
        let dir = TempDir::new().unwrap();
        let manifest_path = dir.path().join("plugin.toml");

        std::fs::write(&manifest_path, "this is not valid toml {{{").unwrap();

        let result = PluginManifest::load(&manifest_path);
        assert!(result.is_err());
    }

    // ==========================================================================
    // Config Loading and Merging Tests
    // ==========================================================================

    #[test]
    fn test_load_user_config_merges_values() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("config.toml");

        // Create user config
        let config_content = r#"
timeout = 60
new_key = "user value"
"#;
        std::fs::write(&config_path, config_content).unwrap();

        // Create manifest with default config
        let mut manifest = PluginManifest {
            plugin: PluginMeta {
                name: "test".to_string(),
                version: "1.0.0".to_string(),
                description: String::new(),
                author: String::new(),
                wasm: "test.wasm".to_string(),
            },
            commands: HashMap::new(),
            dependencies: HashMap::new(),
            config: {
                let mut c = HashMap::new();
                c.insert("timeout".to_string(), toml::Value::Integer(30));
                c.insert("existing".to_string(), toml::Value::String("original".to_string()));
                c
            },
            wasi: WasiConfig::default(),
        };

        manifest.load_user_config(dir.path()).unwrap();

        // User value overrides default
        assert_eq!(manifest.config.get("timeout").unwrap().as_integer(), Some(60));
        // New key added
        assert_eq!(manifest.config.get("new_key").unwrap().as_str(), Some("user value"));
        // Existing key preserved
        assert_eq!(manifest.config.get("existing").unwrap().as_str(), Some("original"));
    }

    #[test]
    fn test_load_user_config_no_config_file() {
        let dir = TempDir::new().unwrap();

        let mut manifest = PluginManifest {
            plugin: PluginMeta {
                name: "test".to_string(),
                version: "1.0.0".to_string(),
                description: String::new(),
                author: String::new(),
                wasm: "test.wasm".to_string(),
            },
            commands: HashMap::new(),
            dependencies: HashMap::new(),
            config: {
                let mut c = HashMap::new();
                c.insert("key".to_string(), toml::Value::String("value".to_string()));
                c
            },
            wasi: WasiConfig::default(),
        };

        // Should succeed even if no config.toml exists
        let result = manifest.load_user_config(dir.path());
        assert!(result.is_ok());
        // Original config preserved
        assert_eq!(manifest.config.get("key").unwrap().as_str(), Some("value"));
    }

    #[test]
    fn test_load_user_config_invalid_toml() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("config.toml");

        std::fs::write(&config_path, "invalid { toml").unwrap();

        let mut manifest = PluginManifest::from_wasm_file(Path::new("test.wasm"));
        let result = manifest.load_user_config(dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_load_user_config_complex_values() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("config.toml");

        // Note: In flat config TOML, nested tables become separate keys
        let config_content = r#"
simple_key = "value"
numbers = [1, 2, 3]
"#;
        std::fs::write(&config_path, config_content).unwrap();

        let mut manifest = PluginManifest::from_wasm_file(Path::new("test.wasm"));
        manifest.load_user_config(dir.path()).unwrap();

        assert!(manifest.config.contains_key("simple_key"));
        assert!(manifest.config.contains_key("numbers"));
        assert!(manifest.config.get("numbers").unwrap().as_array().is_some());
    }

    // ==========================================================================
    // Edge Cases
    // ==========================================================================

    #[test]
    fn test_parse_manifest_empty_commands() {
        let toml_content = r#"
[plugin]
name = "no-commands"
version = "1.0.0"
wasm = "test.wasm"

[commands]
"#;

        let manifest: PluginManifest = toml::from_str(toml_content).unwrap();
        assert!(manifest.commands.is_empty());
    }

    #[test]
    fn test_parse_manifest_unicode_values() {
        let toml_content = r#"
[plugin]
name = "unicode-test"
version = "1.0.0"
description = "Description with unicode: "
author = "Author Name"
wasm = "test.wasm"
"#;

        let manifest: PluginManifest = toml::from_str(toml_content).unwrap();
        assert!(manifest.plugin.description.contains(""));
        assert!(manifest.plugin.author.contains(""));
    }

    #[test]
    fn test_preopen_mapping_struct() {
        let toml_content = r#"
[plugin]
name = "preopen-test"
version = "1.0.0"
wasm = "test.wasm"

[[wasi.preopens]]
host = "/home/user/data"
guest = "/data"

[[wasi.preopens]]
host = "/var/log"
guest = "/logs"
"#;

        let manifest: PluginManifest = toml::from_str(toml_content).unwrap();
        assert_eq!(manifest.wasi.preopens.len(), 2);
        assert_eq!(manifest.wasi.preopens[0].host, "/home/user/data");
        assert_eq!(manifest.wasi.preopens[0].guest, "/data");
        assert_eq!(manifest.wasi.preopens[1].host, "/var/log");
        assert_eq!(manifest.wasi.preopens[1].guest, "/logs");
    }
}
