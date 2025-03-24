// r8s-execution/src/error.rs
//! Error types for the execution engine.

use r8s_core::common::EntityId;
use std::fmt;
use thiserror::Error;

/// Errors that can occur during workflow execution.
#[derive(Error, Debug)]
pub enum ExecutionError {
    /// Error from the core domain layer.
    #[error(transparent)]
    CoreError(#[from] r8s_core::error::Error),

    /// Error when a node fails to execute.
    #[error("Node execution failed: {message} [node_id={node_id}]")]
    NodeExecutionFailed { node_id: EntityId, message: String },

    /// Error when a required input is missing.
    #[error("Missing required input '{input_name}' for node {node_id}")]
    MissingInput {
        node_id: EntityId,
        input_name: String,
    },

    /// Error when connection data fails to transform.
    #[error("Failed to transform data for connection: {0}")]
    DataTransformError(String),

    /// Error in JavaScript execution environment.
    #[error("JavaScript execution error: {0}")]
    JavaScriptError(String),

    /// Error in Lua execution environment.
    #[error("Lua execution error: {0}")]
    LuaError(String),

    /// Error in WebAssembly execution environment.
    #[error("WebAssembly execution error: {0}")]
    WasmError(String),

    /// Error when a timeout occurs during execution.
    #[error("Execution timeout after {duration_secs} seconds")]
    TimeoutError { duration_secs: u32 },

    /// Error when resources are exceeded.
    #[error("Resource limit exceeded: {0}")]
    ResourceExceededError(String),

    /// Error when a worker fails.
    #[error("Worker failed: {0}")]
    WorkerFailure(String),

    /// Error when a scheduler operation fails.
    #[error("Scheduler error: {0}")]
    SchedulerError(String),

    /// Error when execution is canceled.
    #[error("Execution canceled: {0}")]
    ExecutionCanceled(String),

    /// General execution error.
    #[error("Execution error: {0}")]
    ExecutionError(String),

    /// IO Error
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
}

/// Shorthand type for results with ExecutionError.
pub type Result<T> = std::result::Result<T, ExecutionError>;
