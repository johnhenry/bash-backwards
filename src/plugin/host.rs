//! Plugin host - main coordinator for the plugin system
//!
//! The PluginHost manages the plugin lifecycle, including loading, unloading,
//! hot reloading, and command dispatch.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::Value;
use super::hot_reload::{try_create_hot_reloader, HotReloader};
use super::manifest::PluginManifest;
use super::registry::{PluginInfo, PluginRegistry};
use super::PluginError;

/// The plugin host manages all plugin operations
pub struct PluginHost {
    /// The plugin registry
    registry: PluginRegistry,

    /// Hot reloader (optional, may fail to initialize)
    hot_reloader: Option<HotReloader>,

    /// Default plugin directory
    plugin_dir: PathBuf,

    /// Shared stack with the evaluator
    stack: Arc<Mutex<Vec<Value>>>,
}

impl PluginHost {
    /// Create a new plugin host with a shared stack
    pub fn new(stack: Arc<Mutex<Vec<Value>>>) -> Result<Self, PluginError> {
        let plugin_dir = default_plugin_dir();

        // Try to create hot reloader (non-fatal if it fails)
        let hot_reloader = try_create_hot_reloader(plugin_dir.clone());

        Ok(Self {
            registry: PluginRegistry::new(Arc::clone(&stack)),
            hot_reloader,
            plugin_dir,
            stack,
        })
    }

    /// Create a plugin host without hot reloading
    pub fn new_without_hot_reload(stack: Arc<Mutex<Vec<Value>>>) -> Self {
        let plugin_dir = default_plugin_dir();

        Self {
            registry: PluginRegistry::new(Arc::clone(&stack)),
            hot_reloader: None,
            plugin_dir,
            stack,
        }
    }

    /// Load all plugins from the default plugin directory
    pub fn load_plugins_dir(&mut self) -> Result<Vec<String>, PluginError> {
        // Create plugin directory if it doesn't exist
        if !self.plugin_dir.exists() {
            if let Err(e) = std::fs::create_dir_all(&self.plugin_dir) {
                eprintln!("Warning: Could not create plugin directory: {}", e);
                return Ok(Vec::new());
            }
        }

        self.registry.load_all(&self.plugin_dir)
    }

    /// Load a specific plugin by path
    pub fn load_plugin(&mut self, path: &Path) -> Result<(), PluginError> {
        let manifest = if path.is_dir() {
            let manifest_path = path.join("plugin.toml");
            if manifest_path.exists() {
                PluginManifest::load(&manifest_path)?
            } else {
                // Look for a .wasm file in the directory
                let wasm_file = std::fs::read_dir(path)?
                    .filter_map(|e| e.ok())
                    .find(|e| {
                        e.path()
                            .extension()
                            .map_or(false, |ext| ext == "wasm")
                    })
                    .map(|e| e.path())
                    .ok_or_else(|| {
                        PluginError::NotFound(format!(
                            "No plugin.toml or .wasm file found in {}",
                            path.display()
                        ))
                    })?;
                PluginManifest::from_wasm_file(&wasm_file)
            }
        } else if path.extension().map_or(false, |ext| ext == "wasm") {
            PluginManifest::from_wasm_file(path)
        } else {
            return Err(PluginError::NotFound(format!(
                "Cannot determine plugin type for {}",
                path.display()
            )));
        };

        self.registry.load_plugin(path, &manifest)
    }

    /// Unload a plugin by name
    pub fn unload_plugin(&mut self, name: &str) -> Result<(), PluginError> {
        self.registry.unload_plugin(name)
    }

    /// Reload a plugin by name
    pub fn reload_plugin(&mut self, name: &str) -> Result<(), PluginError> {
        self.registry.reload_plugin(name)
    }

    /// Check if a command is provided by a plugin
    pub fn has_command(&self, cmd: &str) -> bool {
        self.registry.has_command(cmd)
    }

    /// Call a plugin command
    pub fn call(&mut self, cmd: &str, args: &[String]) -> Result<i32, PluginError> {
        self.registry.call(cmd, args)
    }

    /// Get information about a plugin
    pub fn get_plugin_info(&self, name: &str) -> Option<PluginInfo> {
        self.registry.get_plugin_info(name)
    }

    /// List all loaded plugins
    pub fn list_plugins(&self) -> Vec<PluginInfo> {
        self.registry
            .plugin_names()
            .into_iter()
            .filter_map(|name| self.registry.get_plugin_info(name))
            .collect()
    }

    /// List all registered commands
    pub fn list_commands(&self) -> Vec<(&str, &str)> {
        self.registry
            .command_names()
            .into_iter()
            .filter_map(|cmd| {
                self.registry
                    .get_plugin_for_command(cmd)
                    .map(|plugin| (cmd, plugin))
            })
            .collect()
    }

    /// Check for hot reload and return names of reloaded plugins
    pub fn check_hot_reload(&mut self) -> Result<Vec<String>, PluginError> {
        // First check the registry's built-in change detection
        let registry_changes = self.registry.check_for_changes();

        // Then check the file watcher
        let watcher_changes = if let Some(ref mut reloader) = self.hot_reloader {
            reloader
                .poll_changes()
                .iter()
                .filter_map(|path| {
                    // Extract plugin name from path
                    if path.is_file() {
                        path.file_stem().and_then(|s| s.to_str()).map(String::from)
                    } else {
                        path.file_name().and_then(|s| s.to_str()).map(String::from)
                    }
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };

        // Combine and deduplicate
        let mut to_reload: Vec<String> = registry_changes;
        for name in watcher_changes {
            if !to_reload.contains(&name) {
                to_reload.push(name);
            }
        }

        // Reload changed plugins
        let mut reloaded = Vec::new();
        for name in to_reload {
            match self.reload_plugin(&name) {
                Ok(_) => {
                    reloaded.push(name);
                }
                Err(e) => {
                    eprintln!("Warning: Failed to reload plugin '{}': {}", name, e);
                }
            }
        }

        Ok(reloaded)
    }

    /// Get the default plugin directory path
    pub fn plugin_dir(&self) -> &Path {
        &self.plugin_dir
    }

    /// Get a reference to the shared stack
    pub fn stack(&self) -> &Arc<Mutex<Vec<Value>>> {
        &self.stack
    }
}

/// Get the default plugin directory (~/.hsab/plugins/)
fn default_plugin_dir() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".hsab").join("plugins")
    } else {
        PathBuf::from(".hsab").join("plugins")
    }
}
