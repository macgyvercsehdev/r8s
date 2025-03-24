// r8s-execution/src/engine/execution_context.rs
//! Execution context for workflow runs.
//!
//! This module defines the context in which a workflow is executed,
//! including the workflow definition, execution state, and input data.

use r8s_core::common::EntityId;
use r8s_core::entity::Workflow;

/// Context for a workflow execution.
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    /// ID of the execution
    pub execution_id: EntityId,

    /// The workflow definition being executed
    pub workflow: Workflow,

    /// Initial data provided for the execution
    pub initial_data: Option<serde_json::Value>,

    /// Current execution variables
    pub variables: serde_json::Map<String, serde_json::Value>,
}

impl ExecutionContext {
    /// Create a new execution context.
    pub fn new(
        execution_id: EntityId,
        workflow: Workflow,
        initial_data: Option<serde_json::Value>,
    ) -> Self {
        Self {
            execution_id,
            workflow,
            initial_data,
            variables: serde_json::Map::new(),
        }
    }

    /// Get a variable from the context.
    pub fn get_variable(&self, name: &str) -> Option<&serde_json::Value> {
        self.variables.get(name)
    }

    /// Set a variable in the context.
    pub fn set_variable(&mut self, name: String, value: serde_json::Value) {
        self.variables.insert(name, value);
    }

    /// Check if a variable exists in the context.
    pub fn has_variable(&self, name: &str) -> bool {
        self.variables.contains_key(name)
    }

    /// Remove a variable from the context.
    pub fn remove_variable(&mut self, name: &str) -> Option<serde_json::Value> {
        self.variables.remove(name)
    }

    /// Get all variables as a JSON object.
    pub fn get_all_variables(&self) -> serde_json::Value {
        serde_json::Value::Object(self.variables.clone())
    }

    /// Merge variables from a JSON object.
    pub fn merge_variables(&mut self, vars: serde_json::Value) {
        if let serde_json::Value::Object(map) = vars {
            for (key, value) in map {
                self.variables.insert(key, value);
            }
        }
    }
}
