// r8s-execution/src/executor/mod.rs
//! Node executors for different node types.
//!
//! This module provides the interfaces and implementations for executing
//! different types of nodes in a workflow.

mod js_executor;
mod lua_executor;
mod wasm_executor;
mod builtin_executor;
mod core_nodes;

use std::collections::HashMap;
use std::sync::Arc;
use async_trait::async_trait;
use r8s_core::common::EntityId;
use r8s_core::entity::{Node, NodeExecutionResult};
use crate::error::Result;
use crate::runtime::RuntimeFactory;

pub use js_executor::JavaScriptExecutor;
pub use lua_executor::LuaExecutor;
pub use wasm_executor::WasmExecutor;
pub use builtin_executor::BuiltinExecutor;

/// Interface for executing workflow nodes.
#[async_trait]
pub trait NodeExecutor: Send + Sync {
    /// Execute a node with the given inputs and return the result.
    async fn execute_node(
        &self,
        execution_id: EntityId,
        node: &Node,
        inputs: HashMap<String, serde_json::Value>,
    ) -> Result<NodeExecutionResult>;
}

/// Registry of node executors for different node types.
pub struct NodeExecutorRegistry {
    /// Map of node type to executor
    executors: HashMap<String, Arc<dyn NodeExecutor>>,
    
    /// Default executor used when no specific executor is found
    default_executor: Arc<dyn NodeExecutor>,
}

impl NodeExecutorRegistry {
    /// Create a new executor registry with a default executor.
    pub fn new(default_executor: Arc<dyn NodeExecutor>) -> Self {
        Self {
            executors: HashMap::new(),
            default_executor,
        }
    }
    
    /// Register an executor for a specific node type.
    pub fn register(&mut self, node_type: &str, executor: Arc<dyn NodeExecutor>) {
        self.executors.insert(node_type.to_string(), executor);
    }
    
    /// Get the appropriate executor for a node.
    pub fn get_executor(&self, node: &Node) -> Arc<dyn NodeExecutor> {
        self.executors
            .get(&node.type_info.name)
            .cloned()
            .unwrap_or_else(|| self.default_executor.clone())
    }
}

/// A composite node executor that delegates to specific executors based on node type.
pub struct CompositeNodeExecutor {
    /// Registry of node executors
    registry: NodeExecutorRegistry,
    
    /// Runtime factory for script execution
    runtime_factory: Arc<dyn RuntimeFactory>,
}

impl CompositeNodeExecutor {
    /// Create a new composite executor.
    pub fn new(registry: NodeExecutorRegistry, runtime_factory: Arc<dyn RuntimeFactory>) -> Self {
        Self {
            registry,
            runtime_factory,
        }
    }
    
    /// Get the runtime factory.
    pub fn runtime_factory(&self) -> Arc<dyn RuntimeFactory> {
        self.runtime_factory.clone()
    }
}

#[async_trait]
impl NodeExecutor for CompositeNodeExecutor {
    async fn execute_node(
        &self,
        execution_id: EntityId,
        node: &Node,
        inputs: HashMap<String, serde_json::Value>,
    ) -> Result<NodeExecutionResult> {
        // Skip execution if node is disabled
        if node.disabled {
            let mut result = NodeExecutionResult::new(node.id);
            result.state = r8s_core::common::ExecutionState::Success;
            result.output_data = inputs.iter()
                .filter(|(k, _)| node.has_output(k))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            return Ok(result);
        }
        
        // Get the appropriate executor for the node
        let executor = self.registry.get_executor(node);
        
        // Execute the node
        executor.execute_node(execution_id, node, inputs).await
    }
}

/// Build a default node executor registry.
pub fn build_default_registry(runtime_factory: Arc<dyn RuntimeFactory>) -> NodeExecutorRegistry {
    let builtin_executor = Arc::new(BuiltinExecutor::new());
    let js_executor = Arc::new(JavaScriptExecutor::new(runtime_factory.clone()));
    let lua_executor = Arc::new(LuaExecutor::new(runtime_factory.clone()));
    let wasm_executor = Arc::new(WasmExecutor::new(runtime_factory));

    let mut registry = NodeExecutorRegistry::new(builtin_executor.clone());
    
    // Register executors for different node types
    registry.register("js_code", js_executor);
    registry.register("lua_code", lua_executor);
    registry.register("wasm_code", wasm_executor);
    
    // Register core node executors
    for node_type in core_nodes::get_all_node_types() {
        registry.register(&node_type, builtin_executor.clone());
    }
    
    registry
}