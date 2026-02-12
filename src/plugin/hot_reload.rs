//! Hot reload functionality using file watching
//!
//! This module provides file watching capabilities to detect when plugin files
//! change and trigger automatic reloading.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::time::Duration;

use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use super::PluginError;

/// Hot reloader that watches plugin directories for changes
pub struct HotReloader {
    /// The file watcher
    _watcher: RecommendedWatcher,

    /// Channel receiver for file system events
    rx: Receiver<Result<Event, notify::Error>>,

    /// The plugin directory being watched
    plugin_dir: PathBuf,

    /// Debounce buffer - paths that have changed recently
    pending_changes: HashSet<PathBuf>,
}

impl HotReloader {
    /// Create a new hot reloader watching the given directory
    pub fn new(plugin_dir: PathBuf) -> Result<Self, PluginError> {
        let (tx, rx) = channel();

        let mut watcher = RecommendedWatcher::new(
            move |res| {
                let _ = tx.send(res);
            },
            Config::default().with_poll_interval(Duration::from_secs(2)),
        )
        .map_err(|e| PluginError::HotReload(format!("Failed to create watcher: {}", e)))?;

        // Only watch if the directory exists
        if plugin_dir.exists() {
            watcher
                .watch(&plugin_dir, RecursiveMode::Recursive)
                .map_err(|e| PluginError::HotReload(format!("Failed to watch directory: {}", e)))?;
        }

        Ok(Self {
            _watcher: watcher,
            rx,
            plugin_dir,
            pending_changes: HashSet::new(),
        })
    }

    /// Poll for changes and return plugin directories that have changed
    ///
    /// Returns a list of plugin directory paths that need to be reloaded.
    pub fn poll_changes(&mut self) -> Vec<PathBuf> {
        // Drain all pending events
        loop {
            match self.rx.try_recv() {
                Ok(Ok(event)) => {
                    self.process_event(event);
                }
                Ok(Err(e)) => {
                    eprintln!("Warning: File watcher error: {}", e);
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    eprintln!("Warning: File watcher disconnected");
                    break;
                }
            }
        }

        // Collect and clear pending changes
        let changed: Vec<PathBuf> = self.pending_changes.drain().collect();
        changed
    }

    /// Process a file system event
    fn process_event(&mut self, event: Event) {
        // Only care about modifications and creations
        match event.kind {
            EventKind::Modify(_) | EventKind::Create(_) => {}
            _ => return,
        }

        for path in event.paths {
            // Only care about .wasm files and plugin.toml
            let dominated_by_relevant = match path.file_name().and_then(|n| n.to_str()) {
                Some(name) if name.ends_with(".wasm") => true,
                Some("plugin.toml") => true,
                Some("config.toml") => true,
                _ => false,
            };

            if !dominated_by_relevant {
                continue;
            }

            // Find the plugin directory
            if let Some(plugin_dir) = self.find_plugin_dir(&path) {
                self.pending_changes.insert(plugin_dir);
            }
        }
    }

    /// Find the plugin directory for a changed file
    fn find_plugin_dir(&self, path: &Path) -> Option<PathBuf> {
        // Walk up from the changed file to find the plugin directory
        let mut current = path.parent()?;

        // If the file is directly in the plugins directory, it's a standalone plugin
        if current == self.plugin_dir {
            return Some(path.to_path_buf());
        }

        // Otherwise, find the directory that's a direct child of plugins dir
        while let Some(parent) = current.parent() {
            if parent == self.plugin_dir {
                return Some(current.to_path_buf());
            }
            current = parent;
        }

        None
    }

    /// Get the plugin directory path
    pub fn plugin_dir(&self) -> &Path {
        &self.plugin_dir
    }
}

/// Create a hot reloader, returning None if it fails (non-fatal)
pub fn try_create_hot_reloader(plugin_dir: PathBuf) -> Option<HotReloader> {
    match HotReloader::new(plugin_dir) {
        Ok(reloader) => Some(reloader),
        Err(e) => {
            eprintln!("Warning: Hot reload disabled: {}", e);
            None
        }
    }
}
