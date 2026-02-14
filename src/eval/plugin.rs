use super::Evaluator;
#[cfg(feature = "plugins")]
use super::EvalError;

impl Evaluator {
    /// Load a plugin: "path/to/plugin" plugin-load
    #[cfg(feature = "plugins")]
    pub(crate) fn builtin_plugin_load(&mut self, args: &[String]) -> Result<(), EvalError> {
        let path = args.first().ok_or_else(|| {
            EvalError::ExecError("plugin-load requires a path argument".to_string())
        })?;

        let path = self.expand_tilde(path);
        let plugin_path = std::path::Path::new(&path);

        if let Some(ref mut host) = self.plugin_host {
            host.load_plugin(plugin_path).map_err(|e| {
                EvalError::ExecError(format!("Failed to load plugin: {}", e))
            })?;
            self.last_exit_code = 0;
        } else {
            return Err(EvalError::ExecError("Plugin system not initialized".to_string()));
        }

        Ok(())
    }

    /// Unload a plugin: "plugin-name" plugin-unload
    #[cfg(feature = "plugins")]
    pub(crate) fn builtin_plugin_unload(&mut self, args: &[String]) -> Result<(), EvalError> {
        let name = args.first().ok_or_else(|| {
            EvalError::ExecError("plugin-unload requires a plugin name".to_string())
        })?;

        if let Some(ref mut host) = self.plugin_host {
            host.unload_plugin(name).map_err(|e| {
                EvalError::ExecError(format!("Failed to unload plugin: {}", e))
            })?;
            self.last_exit_code = 0;
        } else {
            return Err(EvalError::ExecError("Plugin system not initialized".to_string()));
        }

        Ok(())
    }

    /// Force reload a plugin: "plugin-name" plugin-reload
    #[cfg(feature = "plugins")]
    pub(crate) fn builtin_plugin_reload(&mut self, args: &[String]) -> Result<(), EvalError> {
        let name = args.first().ok_or_else(|| {
            EvalError::ExecError("plugin-reload requires a plugin name".to_string())
        })?;

        if let Some(ref mut host) = self.plugin_host {
            host.reload_plugin(name).map_err(|e| {
                EvalError::ExecError(format!("Failed to reload plugin: {}", e))
            })?;
            println!("Plugin reloaded: {}", name);
            self.last_exit_code = 0;
        } else {
            return Err(EvalError::ExecError("Plugin system not initialized".to_string()));
        }

        Ok(())
    }

    /// List all loaded plugins
    #[cfg(feature = "plugins")]
    pub(crate) fn builtin_plugin_list(&mut self) -> Result<(), EvalError> {
        if let Some(ref host) = self.plugin_host {
            let plugins = host.list_plugins();
            if plugins.is_empty() {
                println!("No plugins loaded");
                println!("Plugin directory: {}", host.plugin_dir().display());
            } else {
                println!("Loaded plugins:");
                for info in plugins {
                    println!("  {} v{} - {}", info.name, info.version, info.description);
                    println!("    Commands: {}", info.commands.join(", "));
                    println!("    Path: {}", info.path.display());
                }
            }
            self.last_exit_code = 0;
        } else {
            println!("Plugin system not initialized");
            self.last_exit_code = 1;
        }

        Ok(())
    }

    /// Show details about a specific plugin: "plugin-name" plugin-info
    #[cfg(feature = "plugins")]
    pub(crate) fn builtin_plugin_info(&mut self, args: &[String]) -> Result<(), EvalError> {
        let name = args.first().ok_or_else(|| {
            EvalError::ExecError("plugin-info requires a plugin name".to_string())
        })?;

        if let Some(ref host) = self.plugin_host {
            if let Some(info) = host.get_plugin_info(name) {
                println!("Plugin: {}", info.name);
                println!("Version: {}", info.version);
                println!("Description: {}", info.description);
                println!("Commands: {}", info.commands.join(", "));
                println!("Path: {}", info.path.display());
                self.last_exit_code = 0;
            } else {
                println!("Plugin not found: {}", name);
                self.last_exit_code = 1;
            }
        } else {
            return Err(EvalError::ExecError("Plugin system not initialized".to_string()));
        }

        Ok(())
    }
}
