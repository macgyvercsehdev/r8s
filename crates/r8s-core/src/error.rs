// r8s-core/src/error.rs
//! Definições de erros para o domínio core do r8s.

use thiserror::Error;

/// Erros que podem ocorrer no domínio core do r8s.
#[derive(Error, Debug)]
pub enum Error {
    /// Erro ao validar entidade ou operação
    #[error("Erro de validação: {0}")]
    Validation(String),

    /// Entidade não encontrada
    #[error("Entidade não encontrada: {entity} com ID {id}")]
    NotFound { entity: String, id: String },

    /// Conflito de entidade (ex: duplicata)
    #[error("Conflito de entidade: {0}")]
    Conflict(String),

    /// Operação não autorizada
    #[error("Operação não autorizada: {0}")]
    Unauthorized(String),

    /// Erro de execução de workflow
    #[error("Erro de execução de workflow: {0}")]
    WorkflowExecution(String),

    /// Erro de execução de nó
    #[error("Erro de execução de nó: {0}")]
    NodeExecution(String),

    /// Erro ao carregar plugin
    #[error("Erro ao carregar plugin: {0}")]
    PluginLoad(String),

    /// Erro ao serializar/deserializar
    #[error("Erro de serialização: {0}")]
    Serialization(String),

    /// Erro inesperado ou interno
    #[error("Erro interno: {0}")]
    Internal(String),
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::Serialization(err.to_string())
    }
}

impl From<validator::ValidationErrors> for Error {
    fn from(err: validator::ValidationErrors) -> Self {
        Error::Validation(err.to_string())
    }
}
