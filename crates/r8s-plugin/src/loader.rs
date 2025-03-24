// crates/r8s-plugin/src/loader.rs
//! Plugin loader for dynamically loading plugin libraries.

use libloading::{Library, Symbol};
use semver::Version;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{error, info, warn};

use crate::error::{PluginError, Result};
use crate::plugin::{Plugin, PluginBuilder, PluginMetadata};
use crate::registry::PluginRegistry;

/// Symbol name for the plugin builder function.
const PLUGIN_BUILDER_SYMBOL: &[u8] = b"create_plugin";

/// Minimum compatible core version for plugins.
const MIN_CORE_VERSION: &str = "0.1.0";

/// Loader for dynamically loading plugin libraries.
pub struct PluginLoader {
    /// Registry to register loaded plugins
    registry: PluginRegistry,

    /// Directories to search for plugins
    plugin_dirs: Vec<PathBuf>,

    /// Loaded libraries (must keep them alive while plugins are used)
    #[allow(dead_code)]
    libraries: Vec<Library>,
}

impl PluginLoader {
    /// Create a new plugin loader with the given registry.
    pub fn new(registry: PluginRegistry) -> Self {
        Self {
            registry,
            plugin_dirs: Vec::new(),
            libraries: Vec::new(),
        }
    }

    /// Add a directory to search for plugins.
    pub fn add_plugin_directory<P: AsRef<Path>>(&mut self, dir: P) -> Result<()> {
        let dir_path = dir.as_ref().to_path_buf();

        if !dir_path.exists() || !dir_path.is_dir() {
            return Err(PluginError::LoadError(format!(
                "Plugin directory does not exist or is not a directory: {:?}",
                dir_path
            )));
        }

        self.plugin_dirs.push(dir_path);
        Ok(())
    }

    /// Load a plugin from a shared library file.
    pub unsafe fn load_plugin<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        let path = path.as_ref();

        info!("Loading plugin from {:?}", path);

        // Load the library
        let lib = Library::new(path).map_err(|e| PluginError::LoadError(e.to_string()))?;

        // Get the plugin builder function
        let builder: Symbol<PluginBuilder> = lib.get(PLUGIN_BUILDER_SYMBOL).map_err(|e| {
            PluginError::LoadError(format!("Could not find plugin builder symbol: {}", e))
        })?;

        // Create the plugin instance
        let plugin = builder();

        // Check version compatibility
        let metadata = plugin.metadata();
        self.check_version_compatibility(metadata)?;

        // Register the plugin
        self.registry.register_plugin(plugin)?;

        // Keep the library alive
        self.libraries.push(lib);

        Ok(())
    }

    /// Load all plugins from the configured plugin directories.
    pub fn load_all_plugins(&mut self) -> Result<()> {
        for dir in &self.plugin_dirs {
            self.load_plugins_from_directory(dir)?;
        }

        Ok(())
    }

    /// Load all plugins from a directory.
    fn load_plugins_from_directory<P: AsRef<Path>>(&mut self, dir: P) -> Result<()> {
        let dir = dir.as_ref();

        info!("Scanning for plugins in {:?}", dir);

        let entries = fs::read_dir(dir).map_err(|e| {
            PluginError::LoadError(format!("Failed to read plugin directory {:?}: {}", dir, e))
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| {
                PluginError::LoadError(format!("Failed to read directory entry: {}", e))
            })?;

            let path = entry.path();

            // Skip directories and non-library files
            if path.is_dir() || !Self::is_plugin_library(&path) {
                continue;
            }

            // Try to load the plugin
            unsafe {
                match self.load_plugin(&path) {
                    Ok(_) => {}
                    Err(e) => {
                        warn!("Failed to load plugin from {:?}: {}", path, e);
                        // Continue loading other plugins
                    }
                }
            }
        }

        Ok(())
    }

    /// Check if a path points to a potential plugin library.
    fn is_plugin_library(path: &Path) -> bool {
        if let Some(ext) = path.extension() {
            #[cfg(target_os = "linux")]
            return ext == "so";

            #[cfg(target_os = "macos")]
            return ext == "dylib";

            #[cfg(target_os = "windows")]
            return ext == "dll";
        }

        false
    }

    /// Check if a plugin is compatible with this version of r8s.
    fn check_version_compatibility(&self, metadata: &PluginMetadata) -> Result<()> {
        let min_core_version = Version::parse(&metadata.min_core_version).map_err(|e| {
            PluginError::IncompatibleVersion(format!(
                "Invalid min_core_version in plugin metadata: {}",
                e
            ))
        })?;

        let current_version =
            Version::parse(MIN_CORE_VERSION).expect("Invalid core version constant");

        if min_core_version > current_version {
            return Err(PluginError::IncompatibleVersion(format!(
                "Plugin {} requires r8s version {} or later, but current version is {}",
                metadata.id, min_core_version, current_version
            )));
        }

        Ok(())
    }

    /// Get the plugin registry.
    pub fn registry(&self) -> &PluginRegistry {
        &self.registry
    }
}
