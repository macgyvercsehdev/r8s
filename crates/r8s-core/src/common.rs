// r8s-core/src/common.rs
//! Tipos e traits comuns usados em todo o domínio core.

use std::future::Future;
use std::pin::Pin;

use crate::error::Error;

/// Tipo de resultado para operações do domínio
pub type Result<T> = std::result::Result<T, Error>;

/// Define um resultado assíncrono para operações do domínio
pub type AsyncResult<T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'static>>;

/// ID de um objeto de domínio
pub type EntityId = uuid::Uuid;

/// Estado de execução para workflows e nós
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionState {
    /// Aguardando execução
    Pending,

    /// Em execução no momento
    Running,

    /// Aguardando alguma condição externa (ex: webhook)
    Waiting,

    /// Completado com sucesso
    Success,

    /// Falhou durante a execução
    Failed,

    /// Cancelado pelo usuário ou sistema
    Cancelled,
}

/// Prioridade de execução
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    Low,
    Normal,
    High,
    Critical,
}

impl Default for Priority {
    fn default() -> Self {
        Self::Normal
    }
}

/// Trait para entidades que possuem ID
pub trait Identifiable {
    /// Retorna o ID da entidade
    fn id(&self) -> EntityId;
}

/// Trait para entidades que podem ser validadas
pub trait Validatable {
    /// Valida a entidade e retorna erro caso inválida
    fn validate(&self) -> Result<()>;
}

/// Timestamp para controle de criação/atualização
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Timestamp {
    /// Momento da criação
    pub created_at: chrono::DateTime<chrono::Utc>,

    /// Momento da última atualização
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl Timestamp {
    /// Cria um novo timestamp com o momento atual
    pub fn new() -> Self {
        let now = chrono::Utc::now();
        Self {
            created_at: now,
            updated_at: now,
        }
    }

    /// Atualiza o timestamp de atualização para agora
    pub fn update(&mut self) {
        self.updated_at = chrono::Utc::now();
    }
}

impl Default for Timestamp {
    fn default() -> Self {
        Self::new()
    }
}
