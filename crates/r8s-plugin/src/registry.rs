// crates/r8s-plugin/src/registry.rs
//! Plugin registry for managing loaded plugins.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tracing::{info, error, warn};

use crate::error::{PluginError, Result};
use crate::plugin::{Plugin, NodeProvider, PluginMetadata, NodeTypeInfo};

/// Registry for managing all loaded plugins.
pub struct PluginRegistry {
    /// Map of plugin ID to plugin instance
    plugins: RwLock<HashMap<String, Arc<dyn Plugin>>>,
    
    /// Map of node type name to the plugin that provides it
    node_types: RwLock<HashMap<String, String>>,
}

impl PluginRegistry {
    /// Create a new empty plugin registry.
    pub fn new() -> Self {
        Self {
            plugins: RwLock::new(HashMap::new()),
            node_types: RwLock::new(HashMap::new()),
        }
    }
    
    /// Register a plugin in the registry.
    pub fn register_plugin(&self, plugin: Box<dyn Plugin>) -> Result<()> {
        let metadata = plugin.metadata();
        let plugin_id = metadata.id.clone();
        
        // Check if a plugin with this ID already exists
        {
            let plugins = self.plugins.read().unwrap();
            if plugins.contains_key(&plugin_id) {
                return Err(PluginError::AlreadyExists(plugin_id));
            }
        }
        
        // Register the plugin
        let plugin = Arc::new(plugin as Box<dyn Plugin>);
        
        // If the plugin is a node provider, register its node types
        if let Some(node_provider) = plugin.as_any().downcast_ref::<Box<dyn NodeProvider>>() {
            let node_types = node_provider.get_node_types();
            let mut node_types_map = self.node_types.write().unwrap();
            
            for node_type in node_types {
                let node_type_name = format!("{}.{}", node_type.node_type.name, node_type.node_type.version);
                node_types_map.insert(node_type_name, plugin_id.clone());
            }
        }
        
        // Add the plugin to the registry
        let mut plugins = self.plugins.write().unwrap();
        plugins.insert(plugin_id.clone(), plugin);
        
        info!("Registered plugin: {}", plugin_id);
        Ok(())
    }
    
    /// Unregister a plugin from the registry.
    pub fn unregister_plugin(&self, plugin_id: &str) -> Result<()> {
        // Remove the plugin's node types
        {
            let mut node_types_map = self.node_types.write().unwrap();
            node_types_map.retain(|_, pid| pid != plugin_id);
        }
        
        // Remove the plugin
        let mut plugins = self.plugins.write().unwrap();
        if plugins.remove(plugin_id).is_some() {
            info!("Unregistered plugin: {}", plugin_id);
            Ok(())
        } else {
            Err(PluginError::NotFound(plugin_id.to_string()))
        }
    }
    
    /// Get a plugin by ID.
    pub fn get_plugin(&self, plugin_id: &str) -> Result<Arc<dyn Plugin>> {
        let plugins = self.plugins.read().unwrap();
        
        plugins.get(plugin_id)
            .cloned()
            .ok_or_else(|| PluginError::NotFound(plugin_id.to_string()))
    }
    
    /// Get a node provider by node type.
    pub fn get_node_provider(&self, node_type: &str) -> Result<Arc<dyn NodeProvider>> {
        let node_types_map = self.node_types.read().unwrap();
        let plugins = self.plugins.read().unwrap();
        
        let plugin_id = node_types_map.get(node_type)
            .ok_or_else(|| PluginError::NotFound(format!("Node type not found: {}", node_type)))?;
            
        let plugin = plugins.get(plugin_id)
            .ok_or_else(|| PluginError::NotFound(format!("Plugin not found: {}", plugin_id)))?;
            
        plugin.as_any()
            .downcast_ref::<Arc<dyn NodeProvider>>()
            .cloned()
            .ok_or_else(|| PluginError::Other(format!("Plugin {} is not a NodeProvider", plugin_id)))
    }
    
    /// Get all registered plugins.
    pub fn get_all_plugins(&self) -> Vec<Arc<dyn Plugin>> {
        let plugins = self.plugins.read().unwrap();
        plugins.values().cloned().collect()
    }
    
    /// Get all node types from all registered plugins.
    pub fn get_all_node_types(&self) -> Vec<NodeTypeInfo> {
        let plugins = self.plugins.read().unwrap();
        let mut node_types = Vec::new();
        
        for plugin in plugins.values() {
            if let Some(node_provider) = plugin.as_any().downcast_ref::<Box<dyn NodeProvider>>() {
                node_types.extend(node_provider.get_node_types());
            }
        }
        
        node_types
    }
    
    /// Initialize all plugins.
    pub async fn initialize_all_plugins(&self) -> Result<()> {
        let plugins = self.plugins.read().unwrap();
        
        for (plugin_id, plugin) in plugins.iter() {
            match plugin.initialize().await {
                Ok(_) => {
                    info!("Initialized plugin: {}", plugin_id);
                }
                Err(e) => {
                    error!("Failed to initialize plugin {}: {}", plugin_id, e);
                    return Err(PluginError::InitializationError(
                        format!("Failed to initialize plugin {}: {}", plugin_id, e)
                    ));
                }
            }
        }
        
        Ok(())
    }
    
    /// Shutdown all plugins.
    pub async fn shutdown_all_plugins(&self) -> Result<()> {
        let plugins = self.plugins.read().unwrap();
        
        for (plugin_id, plugin) in plugins.iter() {
            match plugin.shutdown().await {
                Ok(_) => {
                    info!("Shut down plugin: {}", plugin_id);
                }
                Err(e) => {
                    warn!("Error shutting down plugin {}: {}", plugin_id, e);
                    // Continue shutting down other plugins even if one fails
                }
            }
        }
        
        Ok(())
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}