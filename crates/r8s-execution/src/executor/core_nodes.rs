// r8s-execution/src/executor/core_nodes.rs
//! Core node type implementations.
//!
//! This module provides implementations for the built-in node types
//! that are part of the core system.

use std::collections::HashMap;
use std::sync::Arc;
use async_trait::async_trait;

use r8s_core::entity::Node;
use crate::error::{ExecutionError, Result};

/// A handler for a specific node type.
#[async_trait]
pub trait NodeHandler: Send + Sync {
    /// Execute the node with the given inputs and return the outputs.
    async fn execute(
        &self,
        node: &Node,
        inputs: HashMap<String, serde_json::Value>,
    ) -> Result<HashMap<String, serde_json::Value>>;
}

/// Registry of node handlers.
struct NodeHandlerRegistry {
    handlers: HashMap<String, Arc<dyn NodeHandler>>,
}

impl NodeHandlerRegistry {
    /// Create a new handler registry.
    fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }
    
    /// Register a handler for a node type.
    fn register(&mut self, node_type: &str, handler: Arc<dyn NodeHandler>) {
        self.handlers.insert(node_type.to_string(), handler);
    }
    
    /// Get a handler for a node type.
    fn get(&self, node_type: &str) -> Option<Arc<dyn NodeHandler>> {
        self.handlers.get(node_type).cloned()
    }
    
    /// Get all registered node types.
    fn get_all_types(&self) -> Vec<String> {
        self.handlers.keys().cloned().collect()
    }
}

// Global handler registry
lazy_static::lazy_static! {
    static ref HANDLER_REGISTRY: NodeHandlerRegistry = {
        let mut registry = NodeHandlerRegistry::new();
        
        // Register core handlers
        registry.register("start", Arc::new(StartNodeHandler));
        registry.register("end", Arc::new(EndNodeHandler));
        registry.register("if", Arc::new(IfNodeHandler));
        registry.register("switch", Arc::new(SwitchNodeHandler));
        registry.register("foreach", Arc::new(ForEachNodeHandler));
        registry.register("wait", Arc::new(WaitNodeHandler));
        registry.register("merge", Arc::new(MergeNodeHandler));
        registry.register("set_variable", Arc::new(SetVariableNodeHandler));
        registry.register("get_variable", Arc::new(GetVariableNodeHandler));
        registry.register("http_request", Arc::new(HttpRequestNodeHandler));
        registry.register("transform", Arc::new(TransformNodeHandler));
        
        registry
    };
}

/// Get a node handler by type name.
pub fn get_node_handler(node_type: &str) -> Option<Arc<dyn NodeHandler>> {
    HANDLER_REGISTRY.get(node_type)
}

/// Get all registered node types.
pub fn get_all_node_types() -> Vec<String> {
    HANDLER_REGISTRY.get_all_types()
}

//
// Core Node Handler Implementations
//

/// Handler for start nodes.
struct StartNodeHandler;

#[async_trait]
impl NodeHandler for StartNodeHandler {
    async fn execute(
        &self,
        node: &Node,
        inputs: HashMap<String, serde_json::Value>,
    ) -> Result<HashMap<String, serde_json::Value>> {
        // Start nodes just pass through their inputs as outputs
        let mut outputs = HashMap::new();
        
        // Copy all inputs to outputs that match output ports
        for out_port in &node.outputs {
            if let Some(value) = inputs.get(&out_port.name) {
                outputs.insert(out_port.name.clone(), value.clone());
            }
        }
        
        Ok(outputs)
    }
}

/// Handler for end nodes.
struct EndNodeHandler;

#[async_trait]
impl NodeHandler for EndNodeHandler {
    async fn execute(
        &self,
        _node: &Node,
        inputs: HashMap<String, serde_json::Value>,
    ) -> Result<HashMap<String, serde_json::Value>> {
        // End nodes just collect their inputs
        Ok(inputs)
    }
}

/// Handler for if nodes.
struct IfNodeHandler;

#[async_trait]
impl NodeHandler for IfNodeHandler {
    async fn execute(
        &self,
        node: &Node,
        inputs: HashMap<String, serde_json::Value>,
    ) -> Result<HashMap<String, serde_json::Value>> {
        // Get the condition value
        let condition = inputs.get("condition").ok_or_else(|| {
            ExecutionError::MissingInput {
                node_id: node.id,
                input_name: "condition".to_string(),
            }
        })?;
        
        // Get the input data
        let input_data = inputs.get("data").cloned().unwrap_or(serde_json::Value::Null);
        
        // Evaluate the condition
        let condition_result = Self::evaluate_condition(condition)?;
        
        // Create outputs based on condition
        let mut outputs = HashMap::new();
        
        if condition_result {
            outputs.insert("true".to_string(), input_data);
        } else {
            outputs.insert("false".to_string(), input_data);
        }
        
        Ok(outputs)
    }
}

impl IfNodeHandler {
    /// Evaluate a condition value as a boolean.
    fn evaluate_condition(value: &serde_json::Value) -> Result<bool> {
        match value {
            serde_json::Value::Bool(b) => Ok(*b),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(i != 0)
                } else if let Some(f) = n.as_f64() {
                    Ok(f != 0.0)
                } else {
                    Ok(false)
                }
            },
            serde_json::Value::String(s) => {
                // Try to parse as boolean
                if let Ok(b) = s.parse::<bool>() {
                    return Ok(b);
                }
                
                // Check for truthy strings
                let s_lower = s.to_lowercase();
                if ["true", "yes", "y", "1"].contains(&s_lower.as_str()) {
                    return Ok(true);
                }
                
                // Check for non-empty string
                Ok(!s.is_empty())
            },
            serde_json::Value::Array(a) => Ok(!a.is_empty()),
            serde_json::Value::Object(o) => Ok(!o.is_empty()),
            serde_json::Value::Null => Ok(false),
        }
    }
}

/// Handler for switch nodes.
struct SwitchNodeHandler;

#[async_trait]
impl NodeHandler for SwitchNodeHandler {
    async fn execute(
        &self,
        node: &Node,
        inputs: HashMap<String, serde_json::Value>,
    ) -> Result<HashMap<String, serde_json::Value>> {
        // Get the value to switch on
        let value = inputs.get("value").ok_or_else(|| {
            ExecutionError::MissingInput {
                node_id: node.id,
                input_name: "value".to_string(),
            }
        })?;
        
        // Get the data to pass through
        let data = inputs.get("data").cloned().unwrap_or(serde_json::Value::Null);
        
        // Get the cases parameter
        let cases = node.get_parameter::<HashMap<String, String>>("cases")?
            .unwrap_or_default();
        
        // Convert the value to a string for comparison
        let value_str = match value {
            serde_json::Value::String(s) => s.clone(),
            _ => value.to_string(),
        };
        
        // Create outputs
        let mut outputs = HashMap::new();
        
        // Check if the value matches any case
        let mut matched = false;
        for (case_value, output_name) in cases {
            if case_value == value_str {
                outputs.insert(output_name, data.clone());
                matched = true;
                break;
            }
        }
        
        // If no match, use default output
        if !matched {
            outputs.insert("default".to_string(), data);
        }
        
        Ok(outputs)
    }
}

/// Handler for forEach nodes.
struct ForEachNodeHandler;

#[async_trait]
impl NodeHandler for ForEachNodeHandler {
    async fn execute(
        &self,
        node: &Node,
        inputs: HashMap<String, serde_json::Value>,
    ) -> Result<HashMap<String, serde_json::Value>> {
        // Get the array to iterate over
        let array = inputs.get("array").ok_or_else(|| {
            ExecutionError::MissingInput {
                node_id: node.id,
                input_name: "array".to_string(),
            }
        })?;
        
        // Ensure it's an array
        let array = match array {
            serde_json::Value::Array(arr) => arr,
            _ => return Err(ExecutionError::NodeExecutionFailed {
                node_id: node.id,
                message: "Input 'array' must be an array".to_string(),
            }),
        };
        
        // Create output with the array and the current item (for the first iteration)
        let mut outputs = HashMap::new();
        outputs.insert("array".to_string(), serde_json::Value::Array(array.clone()));
        
        if !array.is_empty() {
            outputs.insert("item".to_string(), array[0].clone());
            outputs.insert("index".to_string(), serde_json::Value::Number(0.into()));
            outputs.insert("hasNext".to_string(), serde_json::Value::Bool(array.len() > 1));
        } else {
            // Empty array case
            outputs.insert("complete".to_string(), serde_json::Value::Bool(true));
        }
        
        Ok(outputs)
    }
}

/// Handler for wait nodes.
struct WaitNodeHandler;

#[async_trait]
impl NodeHandler for WaitNodeHandler {
    async fn execute(
        &self,
        node: &Node,
        inputs: HashMap<String, serde_json::Value>,
    ) -> Result<HashMap<String, serde_json::Value>> {
        // Get the wait duration
        let seconds = if let Some(duration) = inputs.get("duration") {
            match duration {
                serde_json::Value::Number(n) => {
                    n.as_f64().unwrap_or(0.0) as u64
                },
                serde_json::Value::String(s) => {
                    s.parse::<u64>().unwrap_or(0)
                },
                _ => 0,
            }
        } else if let Some(seconds) = node.get_parameter::<u64>("seconds")? {
            seconds
        } else {
            0
        };
        
        // In a real implementation, we would set up a timer here
        // For now, just pass through the inputs
        let data = inputs.get("data").cloned().unwrap_or(serde_json::Value::Null);
        
        let mut outputs = HashMap::new();
        outputs.insert("data".to_string(), data);
        
        Ok(outputs)
    }
}

/// Handler for merge nodes.
struct MergeNodeHandler;

#[async_trait]
impl NodeHandler for MergeNodeHandler {
    async fn execute(
        &self,
        _node: &Node,
        inputs: HashMap<String, serde_json::Value>,
    ) -> Result<HashMap<String, serde_json::Value>> {
        // Merge all inputs into a single output
        let mut result = serde_json::Map::new();
        
        for (key, value) in inputs {
            if let serde_json::Value::Object(obj) = value {
                // Merge objects
                for (inner_key, inner_value) in obj {
                    result.insert(inner_key, inner_value);
                }
            } else {
                // Add non-objects as-is
                result.insert(key, value);
            }
        }
        
        let mut outputs = HashMap::new();
        outputs.insert("result".to_string(), serde_json::Value::Object(result));
        
        Ok(outputs)
    }
}

/// Handler for set variable nodes.
struct SetVariableNodeHandler;

#[async_trait]
impl NodeHandler for SetVariableNodeHandler {
    async fn execute(
        &self,
        node: &Node,
        inputs: HashMap<String, serde_json::Value>,
    ) -> Result<HashMap<String, serde_json::Value>> {
        // Get the variable name
        let name = if let Some(name) = inputs.get("name") {
            match name {
                serde_json::Value::String(s) => s.clone(),
                _ => name.to_string(),
            }
        } else if let Some(name) = node.get_parameter::<String>("variableName")? {
            name
        } else {
            return Err(ExecutionError::NodeExecutionFailed {
                node_id: node.id,
                message: "Variable name not specified".to_string(),
            });
        };
        
        // Get the value to set
        let value = inputs.get("value").cloned().unwrap_or(serde_json::Value::Null);
        
        // In a real implementation, we would store the variable in the execution context
        // For now, just pass it through
        let mut outputs = HashMap::new();
        outputs.insert("result".to_string(), value);
        
        Ok(outputs)
    }
}

/// Handler for get variable nodes.
struct GetVariableNodeHandler;

#[async_trait]
impl NodeHandler for GetVariableNodeHandler {
    async fn execute(
        &self,
        node: &Node,
        inputs: HashMap<String, serde_json::Value>,
    ) -> Result<HashMap<String, serde_json::Value>> {
        // Get the variable name
        let name = if let Some(name) = inputs.get("name") {
            match name {
                serde_json::Value::String(s) => s.clone(),
                _ => name.to_string(),
            }
        } else if let Some(name) = node.get_parameter::<String>("variableName")? {
            name
        } else {
            return Err(ExecutionError::NodeExecutionFailed {
                node_id: node.id,
                message: "Variable name not specified".to_string(),
            });
        };
        
        // In a real implementation, we would get the variable from the execution context
        // For now, just return a placeholder
        let value = serde_json::Value::Null;
        
        let mut outputs = HashMap::new();
        outputs.insert("value".to_string(), value);
        
        Ok(outputs)
    }
}

/// Handler for HTTP request nodes.
struct HttpRequestNodeHandler;

#[async_trait]
impl NodeHandler for HttpRequestNodeHandler {
    async fn execute(
        &self,
        node: &Node,
        inputs: HashMap<String, serde_json::Value>,
    ) -> Result<HashMap<String, serde_json::Value>> {
        // Get the URL
        let url = if let Some(url) = inputs.get("url") {
            match url {
                serde_json::Value::String(s) => s.clone(),
                _ => return Err(ExecutionError::NodeExecutionFailed {
                    node_id: node.id,
                    message: "URL must be a string".to_string(),
                }),
            }
        } else if let Some(url) = node.get_parameter::<String>("url")? {
            url
        } else {
            return Err(ExecutionError::NodeExecutionFailed {
                node_id: node.id,
                message: "URL not specified".to_string(),
            });
        };
        
        // Get the method
        let method = if let Some(method) = inputs.get("method") {
            match method {
                serde_json::Value::String(s) => s.clone(),
                _ => "GET".to_string(),
            }
        } else if let Some(method) = node.get_parameter::<String>("method")? {
            method
        } else {
            "GET".to_string()
        };
        
        // Get the body
        let body = inputs.get("body").cloned();
        
        // Get headers
        let headers = if let Some(headers) = inputs.get("headers") {
            headers.clone()
        } else if let Some(headers) = node.get_parameter::<serde_json::Value>("headers")? {
            headers
        } else {
            serde_json::Value::Object(serde_json::Map::new())
        };
        
        // In a real implementation, we would make an actual HTTP request
        // For now, return a mock response
        let response_body = serde_json::json!({
            "success": true,
            "message": "This is a mock response",
            "request": {
                "url": url,
                "method": method,
                "headers": headers,
                "body": body
            }
        });
        
        let mut outputs = HashMap::new();
        outputs.insert("response".to_string(), response_body);
        outputs.insert("statusCode".to_string(), serde_json::Value::Number(200.into()));
        
        Ok(outputs)
    }
}

/// Handler for transform nodes.
struct TransformNodeHandler;

#[async_trait]
impl NodeHandler for TransformNodeHandler {
    async fn execute(
        &self,
        node: &Node,
        inputs: HashMap<String, serde_json::Value>,
    ) -> Result<HashMap<String, serde_json::Value>> {
        // Get the data to transform
        let data = inputs.get("data").ok_or_else(|| {
            ExecutionError::MissingInput {
                node_id: node.id,
                input_name: "data".to_string(),
            }
        })?;
        
        // Get the transformation expression
        let expression = if let Some(expr) = node.get_parameter::<String>("expression")? {
            expr
        } else {
            return Err(ExecutionError::NodeExecutionFailed {
                node_id: node.id,
                message: "Transformation expression not specified".to_string(),
            });
        };
        
        // In a real implementation, we would apply the transformation
        // For now, just pass through the data
        let mut outputs = HashMap::new();
        outputs.insert("result".to_string(), data.clone());
        
        Ok(outputs)
    }
}