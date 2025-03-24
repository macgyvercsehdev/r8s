// r8s-execution/src/lib.rs
//! Execution engine for the r8s workflow orchestrator.
//!
//! This crate provides the runtime for executing workflows, including
//! node execution, data passing, and control flow logic.

pub mod engine;
pub mod error;
pub mod executor;
pub mod runtime;
pub mod scheduler;
pub mod worker;

// Re-export key types for easier use
pub use engine::WorkflowEngine;
pub use error::ExecutionError;
pub use executor::NodeExecutor;
pub use scheduler::ExecutionScheduler;
pub use worker::Worker;
