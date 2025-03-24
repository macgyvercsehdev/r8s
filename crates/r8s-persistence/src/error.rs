// r8s-persistence/src/error.rs
//! Error types for the persistence layer.

use std::fmt;
use thiserror::Error;

/// Errors that can occur in the persistence layer.
#[derive(Error, Debug)]
pub enum PersistenceError {
    /// Error connecting to the database
    #[error("Database connection error: {0}")]
    ConnectionError(String),

    /// Error executing a database query
    #[error("Database query error: {0}")]
    QueryError(String),

    /// Error with database migrations
    #[error("Database migration error: {0}")]
    MigrationError(String),

    /// Error converting between types
    #[error("Type conversion error: {0}")]
    ConversionError(String),

    /// Error with serialization or deserialization
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Entity not found
    #[error("Entity not found: {entity} with ID {id}")]
    NotFoundError { entity: String, id: String },

    /// Error from the SQLx library
    #[error("SQLx error: {0}")]
    SqlxError(#[from] sqlx::Error),

    /// Error from the core domain
    #[error("Domain error: {0}")]
    DomainError(#[from] r8s_core::error::Error),

    /// Generic error
    #[error("Persistence error: {0}")]
    Other(String),
}

/// Type alias for persistence results
pub type Result<T> = std::result::Result<T, PersistenceError>;

impl From<serde_json::Error> for PersistenceError {
    fn from(err: serde_json::Error) -> Self {
        PersistenceError::SerializationError(err.to_string())
    }
}
