// r8s-persistence/src/postgres/mod.rs
//! PostgreSQL implementations of repository interfaces.

pub mod credential_repository;
pub mod execution_repository;
pub mod user_repository;
pub mod variable_repository;
pub mod workflow_repository;

// Re-export for convenience
pub use credential_repository::PostgresCredentialRepository;
pub use execution_repository::PostgresExecutionRepository;
pub use user_repository::PostgresUserRepository;
pub use variable_repository::PostgresVariableRepository;
pub use workflow_repository::PostgresWorkflowRepository;

use crate::connection::ConnectionManager;
use std::sync::Arc;

/// Factory for creating PostgreSQL repositories.
pub struct PostgresRepositoryFactory {
    /// Database connection manager
    connection_manager: Arc<ConnectionManager>,
}

impl PostgresRepositoryFactory {
    /// Create a new repository factory.
    pub fn new(connection_manager: Arc<ConnectionManager>) -> Self {
        Self { connection_manager }
    }

    /// Create a workflow repository.
    pub fn create_workflow_repository(&self) -> PostgresWorkflowRepository {
        PostgresWorkflowRepository::new(self.connection_manager.clone())
    }

    /// Create an execution repository.
    pub fn create_execution_repository(&self) -> PostgresExecutionRepository {
        PostgresExecutionRepository::new(self.connection_manager.clone())
    }

    /// Create a credential repository.
    pub fn create_credential_repository(&self) -> PostgresCredentialRepository {
        PostgresCredentialRepository::new(self.connection_manager.clone())
    }

    /// Create a variable repository.
    pub fn create_variable_repository(&self) -> PostgresVariableRepository {
        PostgresVariableRepository::new(self.connection_manager.clone())
    }

    /// Create a user repository.
    pub fn create_user_repository(&self) -> PostgresUserRepository {
        PostgresUserRepository::new(self.connection_manager.clone())
    }
}
