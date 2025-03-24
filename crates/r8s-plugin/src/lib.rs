// crates/r8s-plugin/src/lib.rs
//! Plugin system for the r8s workflow orchestrator.
//!
//! This crate provides the infrastructure for loading, registering, and
//! managing plugins that extend the functionality of the r8s platform.

pub mod error;
pub mod loader;
pub mod plugin;
pub mod registry;

pub use error::PluginError;
pub use loader::PluginLoader;
pub use plugin::{NodeProvider, Plugin, PluginBuilder, PluginMetadata};
pub use registry::PluginRegistry;
