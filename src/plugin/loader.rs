//! Plugin loader with WASM support
//!
//! This module handles loading WASM plugins and instantiating them
//! with the appropriate imports.

#![allow(dead_code)]

use std::path::Path;
use std::sync::{Arc, Mutex};
use wasmer::{Engine, FunctionEnv, Instance, Module, Store};

use crate::Value;
use super::imports::{create_imports, PluginEnv};
use super::manifest::PluginManifest;
use super::PluginError;

/// A loaded and instantiated plugin
pub struct LoadedPlugin {
    /// The WASM instance
    pub instance: Instance,

    /// The plugin environment for hsab imports
    pub plugin_env: FunctionEnv<PluginEnv>,

    /// The plugin manifest
    pub manifest: PluginManifest,

    /// Path to the plugin directory
    pub path: std::path::PathBuf,
}

impl LoadedPlugin {
    /// Call a function exported by the plugin
    pub fn call_function(
        &self,
        store: &mut Store,
        function_name: &str,
        cmd: &str,
        args_json: &str,
    ) -> Result<i32, PluginError> {
        let func = self
            .instance
            .exports
            .get_function(function_name)
            .map_err(|e| PluginError::CallFailed(format!("Function '{}' not found: {}", function_name, e)))?;

        // Get the memory to write command and args
        let memory = self
            .instance
            .exports
            .get_memory("memory")
            .map_err(|e| PluginError::CallFailed(format!("Memory not found: {}", e)))?;

        // Allocate space for command and args in WASM memory
        // We'll use a simple strategy: write at fixed offsets
        // In a real implementation, we'd call a malloc-like function in WASM
        let cmd_offset: u32 = 1024;
        let args_offset: u32 = cmd_offset + 1024;

        let view = memory.view(store);

        // Write command string
        let cmd_bytes = cmd.as_bytes();
        view.write(cmd_offset as u64, cmd_bytes)
            .map_err(|e| PluginError::CallFailed(format!("Memory write failed: {}", e)))?;

        // Write args JSON string
        let args_bytes = args_json.as_bytes();
        view.write(args_offset as u64, args_bytes)
            .map_err(|e| PluginError::CallFailed(format!("Memory write failed: {}", e)))?;

        // Call the function with (cmd_ptr, cmd_len, args_ptr, args_len)
        let result = func
            .call(store, &[
                wasmer::Value::I32(cmd_offset as i32),
                wasmer::Value::I32(cmd_bytes.len() as i32),
                wasmer::Value::I32(args_offset as i32),
                wasmer::Value::I32(args_bytes.len() as i32),
            ])
            .map_err(|e| PluginError::CallFailed(format!("Function call failed: {}", e)))?;

        // Get return code
        if let Some(wasmer::Value::I32(code)) = result.first() {
            Ok(*code)
        } else {
            Ok(0)
        }
    }

    /// Call the plugin's init function if it exists
    pub fn call_init(&self, store: &mut Store) -> Result<(), PluginError> {
        if let Ok(func) = self.instance.exports.get_function("hsab_plugin_init") {
            func.call(store, &[])
                .map_err(|e| PluginError::CallFailed(format!("Init failed: {}", e)))?;
        }
        Ok(())
    }

    /// Call the plugin's cleanup function if it exists
    pub fn call_cleanup(&self, store: &mut Store) -> Result<(), PluginError> {
        if let Ok(func) = self.instance.exports.get_function("hsab_plugin_cleanup") {
            func.call(store, &[])
                .map_err(|e| PluginError::CallFailed(format!("Cleanup failed: {}", e)))?;
        }
        Ok(())
    }
}

/// Plugin loader responsible for compiling and instantiating WASM plugins
pub struct PluginLoader {
    /// Wasmer engine (shared across all plugins for efficiency)
    engine: Engine,
}

impl PluginLoader {
    pub fn new() -> Self {
        Self {
            engine: Engine::default(),
        }
    }

    /// Load a plugin from a directory with a manifest
    pub fn load(
        &self,
        plugin_dir: &Path,
        manifest: &PluginManifest,
        stack: Arc<Mutex<Vec<Value>>>,
    ) -> Result<(LoadedPlugin, Store), PluginError> {
        // Read the WASM file
        let wasm_path = plugin_dir.join(&manifest.plugin.wasm);
        let wasm_bytes = std::fs::read(&wasm_path).map_err(|e| {
            PluginError::Io(std::io::Error::new(
                e.kind(),
                format!("Failed to read {}: {}", wasm_path.display(), e),
            ))
        })?;

        // Create a new store for this plugin
        let mut store = Store::new(self.engine.clone());

        // Compile the module
        let module = Module::new(&store, &wasm_bytes)
            .map_err(|e| PluginError::Compilation(e.to_string()))?;

        // Create the plugin environment
        let mut plugin_env = PluginEnv::new(manifest.plugin.name.clone(), stack);

        // Set plugin config before creating FunctionEnv
        plugin_env.set_config(manifest.config.clone());

        let env = FunctionEnv::new(&mut store, plugin_env);

        // Create hsab imports
        let imports = create_imports(&mut store, &env);

        // Instantiate the module
        let instance = Instance::new(&mut store, &module, &imports)
            .map_err(|e| PluginError::Instantiation(e.to_string()))?;

        // Set the memory reference in the plugin env
        if let Ok(memory) = instance.exports.get_memory("memory") {
            env.as_mut(&mut store).set_memory(memory.clone());
        }

        let loaded = LoadedPlugin {
            instance,
            plugin_env: env,
            manifest: manifest.clone(),
            path: plugin_dir.to_path_buf(),
        };

        Ok((loaded, store))
    }

    /// Load a standalone WASM file (no manifest)
    pub fn load_standalone(
        &self,
        wasm_path: &Path,
        stack: Arc<Mutex<Vec<Value>>>,
    ) -> Result<(LoadedPlugin, Store), PluginError> {
        let manifest = PluginManifest::from_wasm_file(wasm_path);
        let plugin_dir = wasm_path.parent().unwrap_or(Path::new("."));
        self.load(plugin_dir, &manifest, stack)
    }
}

impl Default for PluginLoader {
    fn default() -> Self {
        Self::new()
    }
}
