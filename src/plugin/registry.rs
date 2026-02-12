//! Plugin registry with command registration and dependency resolution
//!
//! This module manages loaded plugins, tracks which commands map to which plugins,
//! and handles dependency resolution to ensure plugins are loaded in the correct order.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use semver::{Version, VersionReq};
use wasmer::Store;

use crate::Value;
use super::loader::{LoadedPlugin, PluginLoader};
use super::manifest::PluginManifest;
use super::PluginError;

/// An entry in the registry for a loaded plugin
pub struct PluginEntry {
    /// The loaded plugin
    pub plugin: LoadedPlugin,
    /// The Wasmer store for this plugin
    pub store: Store,
    /// Last modified time of the WASM file (for hot reload detection)
    pub mtime: Option<std::time::SystemTime>,
}

/// Plugin registry managing all loaded plugins and their commands
pub struct PluginRegistry {
    /// Loaded plugins by name
    plugins: HashMap<String, PluginEntry>,

    /// Command -> plugin name mapping
    commands: HashMap<String, String>,

    /// Shared stack reference
    stack: Arc<Mutex<Vec<Value>>>,

    /// Plugin loader
    loader: PluginLoader,
}

impl PluginRegistry {
    pub fn new(stack: Arc<Mutex<Vec<Value>>>) -> Self {
        Self {
            plugins: HashMap::new(),
            commands: HashMap::new(),
            stack,
            loader: PluginLoader::new(),
        }
    }

    /// Check if a command exists in any loaded plugin
    pub fn has_command(&self, cmd: &str) -> bool {
        self.commands.contains_key(cmd)
    }

    /// Get the plugin name that provides a command
    pub fn get_plugin_for_command(&self, cmd: &str) -> Option<&str> {
        self.commands.get(cmd).map(|s| s.as_str())
    }

    /// Get all loaded plugin names
    pub fn plugin_names(&self) -> Vec<&str> {
        self.plugins.keys().map(|s| s.as_str()).collect()
    }

    /// Get all registered commands
    pub fn command_names(&self) -> Vec<&str> {
        self.commands.keys().map(|s| s.as_str()).collect()
    }

    /// Get plugin info
    pub fn get_plugin_info(&self, name: &str) -> Option<PluginInfo> {
        self.plugins.get(name).map(|entry| PluginInfo {
            name: entry.plugin.manifest.plugin.name.clone(),
            version: entry.plugin.manifest.plugin.version.clone(),
            description: entry.plugin.manifest.plugin.description.clone(),
            commands: entry.plugin.manifest.commands.keys().cloned().collect(),
            path: entry.plugin.path.clone(),
        })
    }

    /// Load all plugins from a directory, respecting dependency order
    pub fn load_all(&mut self, plugin_dir: &Path) -> Result<Vec<String>, PluginError> {
        if !plugin_dir.exists() {
            return Ok(Vec::new());
        }

        // Scan for plugins
        let manifests = self.scan_plugins(plugin_dir)?;

        if manifests.is_empty() {
            return Ok(Vec::new());
        }

        // Build dependency graph and get load order
        let load_order = self.resolve_dependencies(&manifests)?;

        // Load in order
        let mut loaded = Vec::new();
        for name in load_order {
            if let Some((path, manifest)) = manifests.get(&name) {
                match self.load_plugin(path, manifest) {
                    Ok(_) => loaded.push(name),
                    Err(e) => {
                        eprintln!("Warning: Failed to load plugin '{}': {}", name, e);
                    }
                }
            }
        }

        Ok(loaded)
    }

    /// Scan a directory for plugins (returns manifests by name)
    fn scan_plugins(&self, plugin_dir: &Path) -> Result<HashMap<String, (PathBuf, PluginManifest)>, PluginError> {
        let mut manifests = HashMap::new();

        for entry in std::fs::read_dir(plugin_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                // Directory with plugin.toml
                let manifest_path = path.join("plugin.toml");
                if manifest_path.exists() {
                    match PluginManifest::load(&manifest_path) {
                        Ok(mut manifest) => {
                            // Load user config overrides
                            let _ = manifest.load_user_config(&path);
                            manifests.insert(manifest.plugin.name.clone(), (path, manifest));
                        }
                        Err(e) => {
                            eprintln!("Warning: Failed to parse {}: {}", manifest_path.display(), e);
                        }
                    }
                }
            } else if path.extension().map_or(false, |ext| ext == "wasm") {
                // Standalone WASM file
                let manifest = PluginManifest::from_wasm_file(&path);
                manifests.insert(manifest.plugin.name.clone(), (path, manifest));
            }
        }

        Ok(manifests)
    }

    /// Resolve dependencies and return load order (topological sort)
    fn resolve_dependencies(
        &self,
        manifests: &HashMap<String, (PathBuf, PluginManifest)>,
    ) -> Result<Vec<String>, PluginError> {
        // Build adjacency list for dependency graph
        let mut graph: HashMap<&str, Vec<&str>> = HashMap::new();
        let mut in_degree: HashMap<&str, usize> = HashMap::new();

        // Initialize
        for name in manifests.keys() {
            graph.insert(name.as_str(), Vec::new());
            in_degree.insert(name.as_str(), 0);
        }

        // Build edges
        for (name, (_, manifest)) in manifests {
            for (dep_name, version_req) in &manifest.dependencies {
                // Check dependency exists
                let (_, dep_manifest) = manifests.get(dep_name).ok_or_else(|| {
                    PluginError::MissingDependency(format!(
                        "Plugin '{}' requires '{}' which is not installed",
                        name, dep_name
                    ))
                })?;

                // Check version compatibility
                let req = VersionReq::parse(version_req).map_err(|e| {
                    PluginError::Manifest(format!(
                        "Invalid version requirement '{}' in {}: {}",
                        version_req, name, e
                    ))
                })?;

                let ver = Version::parse(&dep_manifest.plugin.version).map_err(|e| {
                    PluginError::Manifest(format!(
                        "Invalid version '{}' in {}: {}",
                        dep_manifest.plugin.version, dep_name, e
                    ))
                })?;

                if !req.matches(&ver) {
                    return Err(PluginError::VersionMismatch {
                        plugin: name.clone(),
                        required: version_req.clone(),
                        found: dep_manifest.plugin.version.clone(),
                    });
                }

                // Add edge: dep_name -> name (dep must load before name)
                graph.get_mut(dep_name.as_str()).unwrap().push(name.as_str());
                *in_degree.get_mut(name.as_str()).unwrap() += 1;
            }
        }

        // Kahn's algorithm for topological sort
        let mut queue: Vec<&str> = in_degree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(&name, _)| name)
            .collect();
        queue.sort(); // Deterministic order

        let mut order = Vec::new();

        while let Some(name) = queue.pop() {
            order.push(name.to_string());

            if let Some(neighbors) = graph.get(name) {
                for &neighbor in neighbors {
                    let deg = in_degree.get_mut(neighbor).unwrap();
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push(neighbor);
                        queue.sort(); // Keep deterministic
                    }
                }
            }
        }

        // Check for cycles
        if order.len() != manifests.len() {
            return Err(PluginError::CircularDependency);
        }

        Ok(order)
    }

    /// Load a single plugin
    pub fn load_plugin(
        &mut self,
        path: &Path,
        manifest: &PluginManifest,
    ) -> Result<(), PluginError> {
        let plugin_dir = if path.is_file() {
            path.parent().unwrap_or(Path::new("."))
        } else {
            path
        };

        let (plugin, store) = self.loader.load(plugin_dir, manifest, Arc::clone(&self.stack))?;

        // Get modification time for hot reload
        let wasm_path = plugin_dir.join(&manifest.plugin.wasm);
        let mtime = std::fs::metadata(&wasm_path).ok().and_then(|m| m.modified().ok());

        // Register commands
        for (cmd, _handler) in &manifest.commands {
            if self.commands.contains_key(cmd) {
                eprintln!(
                    "Warning: Plugin '{}' shadows command '{}' from another plugin",
                    manifest.plugin.name, cmd
                );
            }
            self.commands.insert(cmd.clone(), manifest.plugin.name.clone());
        }

        // Call init
        let mut entry = PluginEntry {
            plugin,
            store,
            mtime,
        };
        entry.plugin.call_init(&mut entry.store)?;

        // Store plugin
        self.plugins.insert(manifest.plugin.name.clone(), entry);

        Ok(())
    }

    /// Unload a plugin
    pub fn unload_plugin(&mut self, name: &str) -> Result<(), PluginError> {
        // Get plugin entry
        let mut entry = self.plugins.remove(name).ok_or_else(|| {
            PluginError::NotFound(name.to_string())
        })?;

        // Call cleanup
        let _ = entry.plugin.call_cleanup(&mut entry.store);

        // Remove commands
        self.commands.retain(|_, plugin_name| plugin_name != name);

        Ok(())
    }

    /// Reload a plugin
    pub fn reload_plugin(&mut self, name: &str) -> Result<(), PluginError> {
        // Get current plugin info
        let entry = self.plugins.get(name).ok_or_else(|| {
            PluginError::NotFound(name.to_string())
        })?;

        let path = entry.plugin.path.clone();
        let manifest = entry.plugin.manifest.clone();

        // Unload
        self.unload_plugin(name)?;

        // Reload
        self.load_plugin(&path, &manifest)
    }

    /// Call a plugin command
    pub fn call(
        &mut self,
        cmd: &str,
        args: &[String],
    ) -> Result<i32, PluginError> {
        // Find which plugin handles this command
        let plugin_name = self.commands.get(cmd).ok_or_else(|| {
            PluginError::CommandNotFound(cmd.to_string())
        })?.clone();

        // Get the handler function name
        let entry = self.plugins.get_mut(&plugin_name).ok_or_else(|| {
            PluginError::NotFound(plugin_name.clone())
        })?;

        let handler = entry
            .plugin
            .manifest
            .commands
            .get(cmd)
            .ok_or_else(|| PluginError::CommandNotFound(cmd.to_string()))?
            .clone();

        // Convert args to JSON
        let args_json = serde_json::to_string(args).unwrap_or_else(|_| "[]".to_string());

        // Call the handler
        entry.plugin.call_function(&mut entry.store, &handler, cmd, &args_json)
    }

    /// Check for plugins that need reloading based on file modification time
    pub fn check_for_changes(&self) -> Vec<String> {
        let mut changed = Vec::new();

        for (name, entry) in &self.plugins {
            let wasm_path = entry.plugin.path.join(&entry.plugin.manifest.plugin.wasm);
            if let Ok(metadata) = std::fs::metadata(&wasm_path) {
                if let Ok(current_mtime) = metadata.modified() {
                    if let Some(cached_mtime) = entry.mtime {
                        if current_mtime > cached_mtime {
                            changed.push(name.clone());
                        }
                    }
                }
            }

            // Also check manifest
            let manifest_path = entry.plugin.path.join("plugin.toml");
            if manifest_path.exists() {
                if let Ok(metadata) = std::fs::metadata(&manifest_path) {
                    if let Ok(current_mtime) = metadata.modified() {
                        if let Some(cached_mtime) = entry.mtime {
                            if current_mtime > cached_mtime {
                                if !changed.contains(name) {
                                    changed.push(name.clone());
                                }
                            }
                        }
                    }
                }
            }
        }

        changed
    }
}

/// Information about a loaded plugin
#[derive(Debug, Clone)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub commands: Vec<String>,
    pub path: PathBuf,
}
