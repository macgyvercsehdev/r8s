// r8s-execution/src/executor/builtin_executor.rs
//! Executor for built-in node types.
//!
//! This module provides an executor for the core node types that
//! are built into the system.

use std::collections::HashMap;
use async_trait::async_trait;

use r8s_core::common::EntityId;
use r8s_core::entity::{Node, NodeExecutionResult};

use crate::error::{ExecutionError, Result};
use crate::executor::NodeExecutor;
use crate::executor::core_nodes::{get_node_handler, NodeHandler};

/// Executor for built-in node types.
pub struct BuiltinExecutor {}

impl BuiltinExecutor {
    /// Create a new built-in executor.
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl NodeExecutor for BuiltinExecutor {
    async fn execute_node(
        &self,
        execution_id: EntityId,
        node: &Node,
        inputs: HashMap<String, serde_json::Value>,
    ) -> Result<NodeExecutionResult> {
        // Create new execution result
        let mut result = NodeExecutionResult::new(node.id);
        
        // Get the handler for this node type
        let handler = get_node_handler(&node.type_info.name).ok_or_else(|| {
            ExecutionError::NodeExecutionFailed {
                node_id: node.id,
                message: format!("No handler found for node type: {}", node.type_info.name),
            }
        })?;
        
        // Start execution
        result.start(None, inputs.clone());
        
        // Execute the handler
        match handler.execute(node, inputs).await {
            Ok(outputs) => {
                // Complete with success
                result.complete(outputs);
                Ok(result)
            }
            Err(err) => {
                // Mark as failed
                result.fail(err.to_string());
                Err(ExecutionError::NodeExecutionFailed {
                    node_id: node.id,
                    message: err.to_string(),
                })
            }
        }
    }
}