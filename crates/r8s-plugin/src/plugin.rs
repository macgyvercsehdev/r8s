// crates/r8s-plugin/src/plugin.rs
//! Core plugin traits and interfaces.

use std::any::Any;
use std::sync::Arc;
use async_trait::async_trait;
use semver::Version;
use serde::{Serialize, Deserialize};

use r8s_core::entity::{Node, NodeType, NodeTypeCategory};
use r8s_core::common::EntityId;

use crate::error::Result;

/// Metadata about a plugin.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginMetadata {
    /// Unique identifier for the plugin
    pub id: String,
    
    /// Human-readable name of the plugin
    pub name: String,
    
    /// Plugin version
    pub version: String,
    
    /// Plugin author
    pub author: String,
    
    /// Plugin description
    pub description: String,
    
    /// Minimum compatible r8s version
    pub min_core_version: String,
    
    /// Plugin website URL
    pub website: Option<String>,
    
    /// Plugin license
    pub license: Option<String>,
    
    /// Plugin icon (URL or base64 encoded)
    pub icon: Option<String>,
}

/// Information about a node type provided by a plugin.
#[derive(Clone, Debug)]
pub struct NodeTypeInfo {
    /// Node type definition
    pub node_type: NodeType,
    
    /// Default configuration for this node type
    pub default_config: serde_json::Value,
    
    /// Validation schema for node parameters (JSON Schema)
    pub schema: Option<serde_json::Value>,
    
    /// Documentation for this node type
    pub documentation: Option<String>,
    
    /// Example configurations
    pub examples: Vec<NodeExample>,
}

/// Example configuration for a node type.
#[derive(Clone, Debug)]
pub struct NodeExample {
    /// Example name
    pub name: String,
    
    /// Example description
    pub description: String,
    
    /// Example configuration
    pub config: serde_json::Value,
}

/// Common trait for all plugins in the r8s platform.
#[async_trait]
pub trait Plugin: Send + Sync {
    /// Get the plugin metadata.
    fn metadata(&self) -> &PluginMetadata;
    
    /// Initialize the plugin.
    async fn initialize(&self) -> Result<()>;
    
    /// Shut down the plugin and free any resources.
    async fn shutdown(&self) -> Result<()>;
    
    /// Convert the plugin to a trait object reference.
    fn as_any(&self) -> &dyn Any;
}

/// Trait for plugins that provide node types.
#[async_trait]
pub trait NodeProvider: Plugin {
    /// Get the list of node types provided by this plugin.
    fn get_node_types(&self) -> Vec<NodeTypeInfo>;
    
    /// Create a new instance of a node with the given type.
    fn create_node(&self, node_type: &str, node_name: &str, position: (f32, f32)) -> Result<Node>;
    
    /// Execute a node during workflow execution.
    async fn execute_node(
        &self,
        node: &Node,
        inputs: serde_json::Value,
        context: Arc<dyn ExecutionContext>,
    ) -> Result<serde_json::Value>;
}

/// Execution context provided to nodes during execution.
#[async_trait]
pub trait ExecutionContext: Send + Sync {
    /// Get the ID of the current execution.
    fn execution_id(&self) -> EntityId;
    
    /// Get the ID of the workflow being executed.
    fn workflow_id(&self) -> EntityId;
    
    /// Get a credential by ID.
    async fn get_credential(&self, credential_id: EntityId) -> Result<serde_json::Value>;
    
    /// Get a variable by key and scope.
    async fn get_variable(&self, key: &str) -> Result<serde_json::Value>;
    
    /// Log a message to the execution log.
    async fn log(&self, level: &str, message: &str, node_id: Option<EntityId>) -> Result<()>;
    
    /// Get binary data (for file processing).
    async fn get_binary_data(&self, key: &str) -> Result<Vec<u8>>;
    
    /// Store binary data (for file processing).
    async fn store_binary_data(&self, key: &str, data: Vec<u8>) -> Result<()>;
}

/// Plugin builder function signature.
pub type PluginBuilder = fn() -> Box<dyn Plugin>;