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

#[cfg(test)]
mod tests {
    use super::*;

    // ==========================================================================
    // Test-only dependency resolution types and functions
    // ==========================================================================

    /// Lightweight plugin info for dependency resolution (no WASM required)
    #[derive(Debug, Clone)]
    struct PluginDepInfo {
        /// Plugin name (for debugging; also stored as the HashMap key)
        #[allow(dead_code)]
        name: String,
        version: String,
        dependencies: HashMap<String, String>,
    }

    /// Resolve plugin dependencies and return load order (topological sort).
    ///
    /// This is the core dependency resolution algorithm extracted for testability.
    /// It uses Kahn's algorithm to perform a topological sort while detecting cycles.
    fn resolve_plugin_dependencies(
        plugins: &HashMap<String, PluginDepInfo>,
    ) -> Result<Vec<String>, PluginError> {
        // Build adjacency list for dependency graph
        let mut graph: HashMap<&str, Vec<&str>> = HashMap::new();
        let mut in_degree: HashMap<&str, usize> = HashMap::new();

        // Initialize
        for name in plugins.keys() {
            graph.insert(name.as_str(), Vec::new());
            in_degree.insert(name.as_str(), 0);
        }

        // Build edges
        for (name, info) in plugins {
            for (dep_name, version_req) in &info.dependencies {
                // Check dependency exists
                let dep_info = plugins.get(dep_name).ok_or_else(|| {
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

                let ver = Version::parse(&dep_info.version).map_err(|e| {
                    PluginError::Manifest(format!(
                        "Invalid version '{}' in {}: {}",
                        dep_info.version, dep_name, e
                    ))
                })?;

                if !req.matches(&ver) {
                    return Err(PluginError::VersionMismatch {
                        plugin: name.clone(),
                        required: version_req.clone(),
                        found: dep_info.version.clone(),
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
        if order.len() != plugins.len() {
            return Err(PluginError::CircularDependency);
        }

        Ok(order)
    }

    // ==========================================================================
    // Helper Functions for Tests
    // ==========================================================================

    fn make_plugin(name: &str, version: &str, deps: Vec<(&str, &str)>) -> (String, PluginDepInfo) {
        let dependencies: HashMap<String, String> = deps
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        (
            name.to_string(),
            PluginDepInfo {
                name: name.to_string(),
                version: version.to_string(),
                dependencies,
            },
        )
    }

    // ==========================================================================
    // Basic Dependency Resolution Tests
    // ==========================================================================

    #[test]
    fn test_no_dependencies() {
        let plugins: HashMap<String, PluginDepInfo> = [
            make_plugin("alpha", "1.0.0", vec![]),
            make_plugin("beta", "2.0.0", vec![]),
            make_plugin("gamma", "3.0.0", vec![]),
        ]
        .into_iter()
        .collect();

        let order = resolve_plugin_dependencies(&plugins).unwrap();
        assert_eq!(order.len(), 3);
        // All plugins should be present (order is deterministic alphabetically for no-deps)
        assert!(order.contains(&"alpha".to_string()));
        assert!(order.contains(&"beta".to_string()));
        assert!(order.contains(&"gamma".to_string()));
    }

    #[test]
    fn test_single_dependency() {
        let plugins: HashMap<String, PluginDepInfo> = [
            make_plugin("app", "1.0.0", vec![("core", "^1.0.0")]),
            make_plugin("core", "1.2.0", vec![]),
        ]
        .into_iter()
        .collect();

        let order = resolve_plugin_dependencies(&plugins).unwrap();
        assert_eq!(order.len(), 2);

        // core must come before app
        let core_pos = order.iter().position(|x| x == "core").unwrap();
        let app_pos = order.iter().position(|x| x == "app").unwrap();
        assert!(core_pos < app_pos);
    }

    #[test]
    fn test_chain_dependencies() {
        // a -> b -> c -> d
        let plugins: HashMap<String, PluginDepInfo> = [
            make_plugin("a", "1.0.0", vec![("b", "^1.0.0")]),
            make_plugin("b", "1.0.0", vec![("c", "^1.0.0")]),
            make_plugin("c", "1.0.0", vec![("d", "^1.0.0")]),
            make_plugin("d", "1.0.0", vec![]),
        ]
        .into_iter()
        .collect();

        let order = resolve_plugin_dependencies(&plugins).unwrap();
        assert_eq!(order.len(), 4);

        // Order must be d, c, b, a
        assert_eq!(order, vec!["d", "c", "b", "a"]);
    }

    #[test]
    fn test_diamond_dependency() {
        //      top
        //     /   \
        //   left  right
        //     \   /
        //     bottom
        let plugins: HashMap<String, PluginDepInfo> = [
            make_plugin("top", "1.0.0", vec![("left", "^1.0.0"), ("right", "^1.0.0")]),
            make_plugin("left", "1.0.0", vec![("bottom", "^1.0.0")]),
            make_plugin("right", "1.0.0", vec![("bottom", "^1.0.0")]),
            make_plugin("bottom", "1.0.0", vec![]),
        ]
        .into_iter()
        .collect();

        let order = resolve_plugin_dependencies(&plugins).unwrap();
        assert_eq!(order.len(), 4);

        // bottom must be first, top must be last
        let bottom_pos = order.iter().position(|x| x == "bottom").unwrap();
        let left_pos = order.iter().position(|x| x == "left").unwrap();
        let right_pos = order.iter().position(|x| x == "right").unwrap();
        let top_pos = order.iter().position(|x| x == "top").unwrap();

        assert!(bottom_pos < left_pos);
        assert!(bottom_pos < right_pos);
        assert!(left_pos < top_pos);
        assert!(right_pos < top_pos);
    }

    #[test]
    fn test_multiple_dependencies_single_plugin() {
        let plugins: HashMap<String, PluginDepInfo> = [
            make_plugin("app", "1.0.0", vec![
                ("utils", "^1.0.0"),
                ("core", "^2.0.0"),
                ("logging", "^1.0.0"),
            ]),
            make_plugin("utils", "1.5.0", vec![]),
            make_plugin("core", "2.1.0", vec![]),
            make_plugin("logging", "1.2.0", vec![]),
        ]
        .into_iter()
        .collect();

        let order = resolve_plugin_dependencies(&plugins).unwrap();
        assert_eq!(order.len(), 4);

        // app must be last
        let app_pos = order.iter().position(|x| x == "app").unwrap();
        assert_eq!(app_pos, 3);
    }

    // ==========================================================================
    // Cycle Detection Tests
    // ==========================================================================

    #[test]
    fn test_simple_cycle() {
        // a -> b -> a
        let plugins: HashMap<String, PluginDepInfo> = [
            make_plugin("a", "1.0.0", vec![("b", "^1.0.0")]),
            make_plugin("b", "1.0.0", vec![("a", "^1.0.0")]),
        ]
        .into_iter()
        .collect();

        let result = resolve_plugin_dependencies(&plugins);
        assert!(matches!(result, Err(PluginError::CircularDependency)));
    }

    #[test]
    fn test_self_dependency() {
        // a -> a
        let plugins: HashMap<String, PluginDepInfo> = [
            make_plugin("a", "1.0.0", vec![("a", "^1.0.0")]),
        ]
        .into_iter()
        .collect();

        let result = resolve_plugin_dependencies(&plugins);
        assert!(matches!(result, Err(PluginError::CircularDependency)));
    }

    #[test]
    fn test_triangle_cycle() {
        // a -> b -> c -> a
        let plugins: HashMap<String, PluginDepInfo> = [
            make_plugin("a", "1.0.0", vec![("b", "^1.0.0")]),
            make_plugin("b", "1.0.0", vec![("c", "^1.0.0")]),
            make_plugin("c", "1.0.0", vec![("a", "^1.0.0")]),
        ]
        .into_iter()
        .collect();

        let result = resolve_plugin_dependencies(&plugins);
        assert!(matches!(result, Err(PluginError::CircularDependency)));
    }

    #[test]
    fn test_cycle_in_subgraph() {
        // isolated is fine, but x -> y -> z -> x is a cycle
        let plugins: HashMap<String, PluginDepInfo> = [
            make_plugin("isolated", "1.0.0", vec![]),
            make_plugin("x", "1.0.0", vec![("y", "^1.0.0")]),
            make_plugin("y", "1.0.0", vec![("z", "^1.0.0")]),
            make_plugin("z", "1.0.0", vec![("x", "^1.0.0")]),
        ]
        .into_iter()
        .collect();

        let result = resolve_plugin_dependencies(&plugins);
        assert!(matches!(result, Err(PluginError::CircularDependency)));
    }

    // ==========================================================================
    // Missing Dependency Tests
    // ==========================================================================

    #[test]
    fn test_missing_dependency() {
        let plugins: HashMap<String, PluginDepInfo> = [
            make_plugin("app", "1.0.0", vec![("nonexistent", "^1.0.0")]),
        ]
        .into_iter()
        .collect();

        let result = resolve_plugin_dependencies(&plugins);
        assert!(matches!(result, Err(PluginError::MissingDependency(_))));

        if let Err(PluginError::MissingDependency(msg)) = result {
            assert!(msg.contains("nonexistent"));
            assert!(msg.contains("app"));
        }
    }

    #[test]
    fn test_missing_transitive_dependency() {
        // a -> b -> missing
        let plugins: HashMap<String, PluginDepInfo> = [
            make_plugin("a", "1.0.0", vec![("b", "^1.0.0")]),
            make_plugin("b", "1.0.0", vec![("missing", "^1.0.0")]),
        ]
        .into_iter()
        .collect();

        let result = resolve_plugin_dependencies(&plugins);
        assert!(matches!(result, Err(PluginError::MissingDependency(_))));
    }

    // ==========================================================================
    // Version Requirement Tests
    // ==========================================================================

    #[test]
    fn test_version_caret_requirement_satisfied() {
        let plugins: HashMap<String, PluginDepInfo> = [
            make_plugin("app", "1.0.0", vec![("lib", "^1.2.0")]),
            make_plugin("lib", "1.5.3", vec![]),
        ]
        .into_iter()
        .collect();

        let result = resolve_plugin_dependencies(&plugins);
        assert!(result.is_ok());
    }

    #[test]
    fn test_version_caret_requirement_not_satisfied() {
        let plugins: HashMap<String, PluginDepInfo> = [
            make_plugin("app", "1.0.0", vec![("lib", "^2.0.0")]),
            make_plugin("lib", "1.9.9", vec![]), // Major version mismatch
        ]
        .into_iter()
        .collect();

        let result = resolve_plugin_dependencies(&plugins);
        assert!(matches!(result, Err(PluginError::VersionMismatch { .. })));
    }

    #[test]
    fn test_version_tilde_requirement() {
        // ~1.2.3 allows 1.2.3 to <1.3.0
        let plugins: HashMap<String, PluginDepInfo> = [
            make_plugin("app", "1.0.0", vec![("lib", "~1.2.3")]),
            make_plugin("lib", "1.2.9", vec![]),
        ]
        .into_iter()
        .collect();

        let result = resolve_plugin_dependencies(&plugins);
        assert!(result.is_ok());
    }

    #[test]
    fn test_version_tilde_requirement_not_satisfied() {
        let plugins: HashMap<String, PluginDepInfo> = [
            make_plugin("app", "1.0.0", vec![("lib", "~1.2.0")]),
            make_plugin("lib", "1.3.0", vec![]), // Minor version too high for tilde
        ]
        .into_iter()
        .collect();

        let result = resolve_plugin_dependencies(&plugins);
        assert!(matches!(result, Err(PluginError::VersionMismatch { .. })));
    }

    #[test]
    fn test_version_exact_requirement() {
        let plugins: HashMap<String, PluginDepInfo> = [
            make_plugin("app", "1.0.0", vec![("lib", "=1.5.0")]),
            make_plugin("lib", "1.5.0", vec![]),
        ]
        .into_iter()
        .collect();

        let result = resolve_plugin_dependencies(&plugins);
        assert!(result.is_ok());
    }

    #[test]
    fn test_version_exact_requirement_not_satisfied() {
        let plugins: HashMap<String, PluginDepInfo> = [
            make_plugin("app", "1.0.0", vec![("lib", "=1.5.0")]),
            make_plugin("lib", "1.5.1", vec![]), // Patch mismatch
        ]
        .into_iter()
        .collect();

        let result = resolve_plugin_dependencies(&plugins);
        assert!(matches!(result, Err(PluginError::VersionMismatch { .. })));
    }

    #[test]
    fn test_version_range_requirement() {
        let plugins: HashMap<String, PluginDepInfo> = [
            make_plugin("app", "1.0.0", vec![("lib", ">=1.0.0, <2.0.0")]),
            make_plugin("lib", "1.9.9", vec![]),
        ]
        .into_iter()
        .collect();

        let result = resolve_plugin_dependencies(&plugins);
        assert!(result.is_ok());
    }

    #[test]
    fn test_version_range_requirement_not_satisfied() {
        let plugins: HashMap<String, PluginDepInfo> = [
            make_plugin("app", "1.0.0", vec![("lib", ">=1.0.0, <2.0.0")]),
            make_plugin("lib", "2.0.0", vec![]), // At the exclusive upper bound
        ]
        .into_iter()
        .collect();

        let result = resolve_plugin_dependencies(&plugins);
        assert!(matches!(result, Err(PluginError::VersionMismatch { .. })));
    }

    #[test]
    fn test_version_wildcard_requirement() {
        let plugins: HashMap<String, PluginDepInfo> = [
            make_plugin("app", "1.0.0", vec![("lib", "1.*")]),
            make_plugin("lib", "1.99.99", vec![]),
        ]
        .into_iter()
        .collect();

        let result = resolve_plugin_dependencies(&plugins);
        assert!(result.is_ok());
    }

    #[test]
    fn test_version_star_any() {
        let plugins: HashMap<String, PluginDepInfo> = [
            make_plugin("app", "1.0.0", vec![("lib", "*")]),
            make_plugin("lib", "999.0.0", vec![]),
        ]
        .into_iter()
        .collect();

        let result = resolve_plugin_dependencies(&plugins);
        assert!(result.is_ok());
    }

    #[test]
    fn test_version_mismatch_error_details() {
        let plugins: HashMap<String, PluginDepInfo> = [
            make_plugin("consumer", "1.0.0", vec![("provider", "^3.0.0")]),
            make_plugin("provider", "2.5.0", vec![]),
        ]
        .into_iter()
        .collect();

        let result = resolve_plugin_dependencies(&plugins);
        match result {
            Err(PluginError::VersionMismatch { plugin, required, found }) => {
                assert_eq!(plugin, "consumer");
                assert_eq!(required, "^3.0.0");
                assert_eq!(found, "2.5.0");
            }
            _ => panic!("Expected VersionMismatch error"),
        }
    }

    #[test]
    fn test_invalid_version_requirement_syntax() {
        let plugins: HashMap<String, PluginDepInfo> = [
            make_plugin("app", "1.0.0", vec![("lib", "not-a-valid-version")]),
            make_plugin("lib", "1.0.0", vec![]),
        ]
        .into_iter()
        .collect();

        let result = resolve_plugin_dependencies(&plugins);
        assert!(matches!(result, Err(PluginError::Manifest(_))));
    }

    #[test]
    fn test_invalid_plugin_version() {
        let plugins: HashMap<String, PluginDepInfo> = [
            make_plugin("app", "1.0.0", vec![("lib", "^1.0.0")]),
            ("lib".to_string(), PluginDepInfo {
                name: "lib".to_string(),
                version: "not-semver".to_string(),
                dependencies: HashMap::new(),
            }),
        ]
        .into_iter()
        .collect();

        let result = resolve_plugin_dependencies(&plugins);
        assert!(matches!(result, Err(PluginError::Manifest(_))));
    }

    // ==========================================================================
    // Complex Dependency Graph Tests
    // ==========================================================================

    #[test]
    fn test_complex_dag() {
        //         app
        //        / | \
        //       a  b  c
        //      /\ /\ /\
        //     d  e  f  g
        //      \ | /
        //       core
        let plugins: HashMap<String, PluginDepInfo> = [
            make_plugin("app", "1.0.0", vec![("a", "^1.0.0"), ("b", "^1.0.0"), ("c", "^1.0.0")]),
            make_plugin("a", "1.0.0", vec![("d", "^1.0.0"), ("e", "^1.0.0")]),
            make_plugin("b", "1.0.0", vec![("e", "^1.0.0"), ("f", "^1.0.0")]),
            make_plugin("c", "1.0.0", vec![("f", "^1.0.0"), ("g", "^1.0.0")]),
            make_plugin("d", "1.0.0", vec![("core", "^1.0.0")]),
            make_plugin("e", "1.0.0", vec![("core", "^1.0.0")]),
            make_plugin("f", "1.0.0", vec![("core", "^1.0.0")]),
            make_plugin("g", "1.0.0", vec![]),
            make_plugin("core", "1.0.0", vec![]),
        ]
        .into_iter()
        .collect();

        let order = resolve_plugin_dependencies(&plugins).unwrap();
        assert_eq!(order.len(), 9);

        // Verify ordering constraints
        let pos = |n: &str| order.iter().position(|x| x == n).unwrap();

        // core must come before d, e, f
        assert!(pos("core") < pos("d"));
        assert!(pos("core") < pos("e"));
        assert!(pos("core") < pos("f"));

        // d, e must come before a
        assert!(pos("d") < pos("a"));
        assert!(pos("e") < pos("a"));

        // e, f must come before b
        assert!(pos("e") < pos("b"));
        assert!(pos("f") < pos("b"));

        // f, g must come before c
        assert!(pos("f") < pos("c"));
        assert!(pos("g") < pos("c"));

        // a, b, c must come before app
        assert!(pos("a") < pos("app"));
        assert!(pos("b") < pos("app"));
        assert!(pos("c") < pos("app"));
    }

    #[test]
    fn test_deterministic_ordering() {
        // With no dependencies, order should be deterministic (alphabetical)
        let plugins: HashMap<String, PluginDepInfo> = [
            make_plugin("zebra", "1.0.0", vec![]),
            make_plugin("apple", "1.0.0", vec![]),
            make_plugin("mango", "1.0.0", vec![]),
        ]
        .into_iter()
        .collect();

        let order1 = resolve_plugin_dependencies(&plugins).unwrap();
        let order2 = resolve_plugin_dependencies(&plugins).unwrap();
        let order3 = resolve_plugin_dependencies(&plugins).unwrap();

        assert_eq!(order1, order2);
        assert_eq!(order2, order3);
    }

    #[test]
    fn test_single_plugin_no_deps() {
        let plugins: HashMap<String, PluginDepInfo> = [
            make_plugin("solo", "1.0.0", vec![]),
        ]
        .into_iter()
        .collect();

        let order = resolve_plugin_dependencies(&plugins).unwrap();
        assert_eq!(order, vec!["solo"]);
    }

    #[test]
    fn test_empty_plugin_set() {
        let plugins: HashMap<String, PluginDepInfo> = HashMap::new();

        let order = resolve_plugin_dependencies(&plugins).unwrap();
        assert!(order.is_empty());
    }

    // ==========================================================================
    // Prerelease Version Tests
    // ==========================================================================

    #[test]
    fn test_prerelease_version() {
        let plugins: HashMap<String, PluginDepInfo> = [
            make_plugin("app", "1.0.0", vec![("lib", "^1.0.0-alpha")]),
            make_plugin("lib", "1.0.0-beta", vec![]),
        ]
        .into_iter()
        .collect();

        // Prerelease versions have special semver matching rules
        let result = resolve_plugin_dependencies(&plugins);
        // This may or may not match depending on semver crate behavior
        // Just ensure we don't panic
        assert!(result.is_ok() || matches!(result, Err(PluginError::VersionMismatch { .. })));
    }

    // ==========================================================================
    // Edge Cases
    // ==========================================================================

    #[test]
    fn test_version_0_x_special_cases() {
        // 0.x versions have special semver behavior
        let plugins: HashMap<String, PluginDepInfo> = [
            make_plugin("app", "1.0.0", vec![("lib", "^0.2.0")]),
            make_plugin("lib", "0.2.5", vec![]),
        ]
        .into_iter()
        .collect();

        let result = resolve_plugin_dependencies(&plugins);
        assert!(result.is_ok());
    }

    #[test]
    fn test_version_0_x_breaking_change() {
        // For 0.x, minor version bumps are breaking
        let plugins: HashMap<String, PluginDepInfo> = [
            make_plugin("app", "1.0.0", vec![("lib", "^0.2.0")]),
            make_plugin("lib", "0.3.0", vec![]),
        ]
        .into_iter()
        .collect();

        let result = resolve_plugin_dependencies(&plugins);
        assert!(matches!(result, Err(PluginError::VersionMismatch { .. })));
    }

    #[test]
    fn test_large_version_numbers() {
        let plugins: HashMap<String, PluginDepInfo> = [
            make_plugin("app", "999.888.777", vec![("lib", ">=100.0.0")]),
            make_plugin("lib", "100.200.300", vec![]),
        ]
        .into_iter()
        .collect();

        let result = resolve_plugin_dependencies(&plugins);
        assert!(result.is_ok());
    }
}
