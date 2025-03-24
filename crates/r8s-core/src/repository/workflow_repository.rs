// r8s-core/src/repository/workflow_repository.rs
//! Interface de repositório para workflows.

use crate::common::{AsyncResult, EntityId, Result};
use crate::entity::{Tag, Workflow};
use crate::repository::Repository;
use async_trait::async_trait;

/// Critérios de busca para workflows
pub struct WorkflowFilter {
    /// Filtrar por nome (parcial)
    pub name: Option<String>,

    /// Filtrar por tags
    pub tags: Option<Vec<EntityId>>,

    /// Filtrar por status ativo
    pub active: Option<bool>,

    /// Filtrar por ID do criador
    pub created_by: Option<EntityId>,

    /// Ordenar por campo
    pub sort_by: Option<WorkflowSortField>,

    /// Ordem ascendente ou descendente
    pub ascending: bool,

    /// Limite de resultados
    pub limit: Option<usize>,

    /// Offset para paginação
    pub offset: Option<usize>,
}

/// Campos para ordenação de workflows
pub enum WorkflowSortField {
    /// Ordenar por nome
    Name,

    /// Ordenar por data de criação
    CreatedAt,

    /// Ordenar por data de atualização
    UpdatedAt,
}

/// Interface para repositório de workflows
#[async_trait]
pub trait WorkflowRepository: Repository<Workflow> {
    /// Busca workflows com filtros
    fn find_by_filter(&self, filter: WorkflowFilter) -> AsyncResult<Vec<Workflow>>;

    /// Conta workflows com filtros
    fn count_by_filter(&self, filter: WorkflowFilter) -> AsyncResult<usize>;

    /// Busca workflows por tag
    fn find_by_tag(&self, tag_id: EntityId) -> AsyncResult<Vec<Workflow>>;

    /// Busca workflows criados por um usuário
    fn find_by_creator(&self, user_id: EntityId) -> AsyncResult<Vec<Workflow>>;

    /// Busca uma versão específica de um workflow
    fn find_version(&self, id: EntityId, version: u32) -> AsyncResult<Option<Workflow>>;

    /// Lista todas as versões de um workflow
    fn list_versions(&self, id: EntityId) -> AsyncResult<Vec<u32>>;

    /// Adiciona uma tag a um workflow
    fn add_tag(&self, workflow_id: EntityId, tag: &Tag) -> AsyncResult<()>;

    /// Remove uma tag de um workflow
    fn remove_tag(&self, workflow_id: EntityId, tag_id: EntityId) -> AsyncResult<()>;
}
