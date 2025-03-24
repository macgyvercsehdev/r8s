// crates/r8s-api/src/lib.rs
//! RESTful API for the r8s workflow orchestrator.
//!
//! This crate provides a HTTP API for clients to interact with the r8s platform,
//! including managing workflows, monitoring executions, and handling authentication.

pub mod error;
pub mod middleware;
pub mod routes;
pub mod server;

pub use error::{ApiError, Result};
pub use server::ApiServer;
