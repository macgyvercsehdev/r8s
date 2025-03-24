// r8s-persistence/src/lib.rs
//! Persistence layer for the r8s workflow orchestrator.
//!
//! This crate provides implementations of the repository interfaces
//! defined in r8s-core, using PostgreSQL as the backing store.

pub mod connection;
pub mod error;
pub mod migrations;
pub mod models;
pub mod postgres;

// Re-exports for convenience
pub use connection::ConnectionManager;
pub use error::PersistenceError;
pub use postgres::*;
