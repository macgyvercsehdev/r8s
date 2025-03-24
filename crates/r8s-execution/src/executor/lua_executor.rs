// r8s-execution/src/executor/lua_executor.rs
//! Lua executor for nodes.
//!
//! This module provides an executor for Lua code in workflow nodes,
//! using the Lua programming language.

use std::collections::HashMap;
use std::sync::Arc;
use async_trait::async_trait;

use r8s_core::common::EntityId;
use r8s_core::entity::{Node, NodeExecutionResult};

use crate::error::{ExecutionError, Result};
use crate::executor::NodeExecutor;
use crate::runtime::RuntimeFactory;

/// Executor for Lua code nodes.
pub struct LuaExecutor {
    /// Runtime factory for creating Lua environments
    runtime_factory: Arc<dyn RuntimeFactory>,
}

impl LuaExecutor {
    /// Create a new Lua executor.
    pub fn new(runtime_factory: Arc<dyn RuntimeFactory>) -> Self {
        Self {
            runtime_factory,
        }
    }
}

#[async_trait]
impl NodeExecutor for LuaExecutor {
    async fn execute_node(
        &self,
        execution_id: EntityId,
        node: &Node,
        inputs: HashMap<String, serde_json::Value>,
    ) -> Result<NodeExecutionResult> {
        // Create new execution result
        let mut result = NodeExecutionResult::new(node.id);
        
        // Start execution
        result.start(None, inputs.clone());
        
        // Get the code to execute
        let code = if let Some(code) = node.get_parameter::<String>("code")? {
            code
        } else {
            return Err(ExecutionError::NodeExecutionFailed {
                node_id: node.id,
                message: "No Lua code found in node parameters".to_string(),
            });
        };
        
        // Create a Lua runtime
        let mut runtime = self.runtime_factory.create_lua_runtime().await?;
        
        // Set up the runtime with inputs and execution context
        runtime.set_global("inputs", &serde_json::to_value(&inputs)?)?;
        runtime.set_global("nodeId", &serde_json::to_value(&node.id.to_string())?)?;
        runtime.set_global("executionId", &serde_json::to_value(&execution_id.to_string())?)?;
        
        // Create default outputs table
        runtime.execute_lua("outputs = {}").await?;
        
        // Execute the code
        let execution_result = runtime.execute_lua(&code).await;
        
        match execution_result {
            Ok(lua_result) => {
                // Parse the output
                let outputs = if let Ok(outputs) = runtime.get_global::<serde_json::Value>("outputs") {
                    // If the code set an 'outputs' table, use that
                    if let serde_json::Value::Object(map) = outputs {
                        map.into_iter()
                            .map(|(k, v)| (k, v))
                            .collect()
                    } else {
                        // If outputs is not an object, create a default output
                        let mut out = HashMap::new();
                        out.insert("result".to_string(), outputs);
                        out
                    }
                } else {
                    // If no outputs variable was set, use the return value
                    let mut outputs = HashMap::new();
                    outputs.insert("result".to_string(), lua_result);
                    outputs
                };
                
                // Complete with success
                result.complete(outputs);
                Ok(result)
            },
            Err(err) => {
                // Mark as failed
                let error_message = format!("Lua execution error: {}", err);
                result.fail(error_message.clone());
                
                Err(ExecutionError::LuaError(error_message))
            }
        }
    }
}