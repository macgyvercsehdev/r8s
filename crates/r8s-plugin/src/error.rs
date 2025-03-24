// crates/r8s-plugin/src/error.rs
//! Error types for the plugin system.

use thiserror::Error;

/// Errors that can occur in the plugin system.
#[derive(Error, Debug)]
pub enum PluginError {
    /// Error loading a plugin from disk
    #[error("Failed to load plugin: {0}")]
    LoadError(String),

    /// Error initializing a plugin
    #[error("Failed to initialize plugin: {0}")]
    InitializationError(String),

    /// Plugin with the specified ID was not found
    #[error("Plugin not found: {0}")]
    NotFound(String),

    /// Plugin with the same ID already exists
    #[error("Plugin with ID {0} already exists")]
    AlreadyExists(String),

    /// Plugin version is incompatible
    #[error("Incompatible plugin version: {0}")]
    IncompatibleVersion(String),

    /// Generic plugin error
    #[error("Plugin error: {0}")]
    Other(String),

    /// Error during plugin execution
    #[error("Plugin execution error: {0}")]
    ExecutionError(String),

    /// Invalid plugin manifest
    #[error("Invalid plugin manifest: {0}")]
    InvalidManifest(String),
}

/// Result type for plugin operations
pub type Result<T> = std::result::Result<T, PluginError>;
