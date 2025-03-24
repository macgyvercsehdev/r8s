// r8s-execution/src/executor/wasm_executor.rs
//! WebAssembly executor for nodes.
//!
//! This module provides an executor for WebAssembly code in workflow nodes.

use std::collections::HashMap;
use std::sync::Arc;
use async_trait::async_trait;

use r8s_core::common::EntityId;
use r8s_core::entity::{Node, NodeExecutionResult};

use crate::error::{ExecutionError, Result};
use crate::executor::NodeExecutor;
use crate::runtime::RuntimeFactory;

/// Executor for WebAssembly code nodes.
pub struct WasmExecutor {
    /// Runtime factory for creating WebAssembly environments
    runtime_factory: Arc<dyn RuntimeFactory>,
}

impl WasmExecutor {
    /// Create a new WebAssembly executor.
    pub fn new(runtime_factory: Arc<dyn RuntimeFactory>) -> Self {
        Self {
            runtime_factory,
        }
    }
}

#[async_trait]
impl NodeExecutor for WasmExecutor {
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
        
        // Get the WebAssembly binary or text to execute
        let wasm_binary = if let Some(binary) = node.get_parameter::<Vec<u8>>("binary")? {
            binary
        } else if let Some(base64) = node.get_parameter::<String>("base64")? {
            // Decode base64 to binary
            match base64::decode(&base64) {
                Ok(bytes) => bytes,
                Err(err) => {
                    return Err(ExecutionError::NodeExecutionFailed {
                        node_id: node.id,
                        message: format!("Invalid base64 WebAssembly: {}", err),
                    });
                }
            }
        } else {
            return Err(ExecutionError::NodeExecutionFailed {
                node_id: node.id,
                message: "No WebAssembly code found in node parameters".to_string(),
            });
        };
        
        // Create a WebAssembly runtime
        let mut runtime = self.runtime_factory.create_wasm_runtime().await?;
        
        // Set up the runtime with inputs and execution context
        let inputs_json = serde_json::to_string(&inputs)?;
        
        // Execute the WebAssembly module
        match runtime.execute_wasm(&wasm_binary, "process", &[inputs_json]).await {
            Ok(output_json) => {
                // Parse the output
                let result_value: serde_json::Value = serde_json::from_str(&output_json)?;
                
                let outputs = if let serde_json::Value::Object(map) = result_value {
                    map.into_iter()
                        .map(|(k, v)| (k, v))
                        .collect()
                } else {
                    // If output is not an object, create a default output
                    let mut out = HashMap::new();
                    out.insert("result".to_string(), result_value);
                    out
                };
                
                // Complete with success
                result.complete(outputs);
                Ok(result)
            },
            Err(err) => {
                // Mark as failed
                let error_message = format!("WebAssembly execution error: {}", err);
                result.fail(error_message.clone());
                
                Err(ExecutionError::WasmError(error_message))
            }
        }
    }
}