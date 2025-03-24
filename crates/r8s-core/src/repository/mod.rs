// r8s-core/src/repository/mod.rs
//! Interfaces de repositório para acesso a dados.
//!
//! Este módulo define as interfaces abstratas para acesso a dados,
//! seguindo os princípios de inversão de dependência da Clean Architecture.

pub mod credential_repository;
pub mod execution_repository;
pub mod user_repository;
pub mod variable_repository;
pub mod workflow_repository;

// Re-exportações para facilitar o uso
pub use credential_repository::CredentialRepository;
pub use execution_repository::ExecutionRepository;
pub use user_repository::UserRepository;
pub use variable_repository::VariableRepository;
pub use workflow_repository::WorkflowRepository;

use crate::common::{AsyncResult, EntityId, Result};

/// Trait genérico para operações CRUD em repositórios
#[async_trait::async_trait]
pub trait Repository<T> {
    /// Busca uma entidade pelo ID
    fn find_by_id(&self, id: EntityId) -> AsyncResult<T>;

    /// Salva uma entidade (insere ou atualiza)
    fn save(&self, entity: T) -> AsyncResult<T>;

    /// Remove uma entidade pelo ID
    fn delete(&self, id: EntityId) -> AsyncResult<()>;

    /// Verifica se uma entidade existe pelo ID
    fn exists(&self, id: EntityId) -> AsyncResult<bool>;
}
