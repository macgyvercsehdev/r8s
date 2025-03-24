// r8s-core/src/repository/execution_repository.rs
//! Interface de repositório para execuções de workflow.

use crate::common::{AsyncResult, EntityId, ExecutionState, Result};
use crate::entity::{Execution, ExecutionLog, NodeExecutionResult};
use crate::repository::Repository;
use async_trait::async_trait;

/// Critérios de busca para execuções
pub struct ExecutionFilter {
    /// Filtrar por ID do workflow
    pub workflow_id: Option<EntityId>,

    /// Filtrar por estado de execução
    pub state: Option<ExecutionState>,

    /// Filtrar por ID do iniciador
    pub initiated_by: Option<EntityId>,

    /// Filtrar por período de início (de)
    pub from_date: Option<chrono::DateTime<chrono::Utc>>,

    /// Filtrar por período de início (até)
    pub to_date: Option<chrono::DateTime<chrono::Utc>>,

    /// Filtrar por execuções que falharam
    pub failed_only: bool,

    /// Ordenar por campo
    pub sort_by: Option<ExecutionSortField>,

    /// Ordem ascendente ou descendente
    pub ascending: bool,

    /// Limite de resultados
    pub limit: Option<usize>,

    /// Offset para paginação
    pub offset: Option<usize>,
}

/// Campos para ordenação de execuções
pub enum ExecutionSortField {
    /// Ordenar por data de início
    StartedAt,

    /// Ordenar por data de finalização
    FinishedAt,

    /// Ordenar por duração
    Duration,
}

/// Interface para repositório de execuções
#[async_trait]
pub trait ExecutionRepository: Repository<Execution> {
    /// Busca execuções com filtros
    fn find_by_filter(&self, filter: ExecutionFilter) -> AsyncResult<Vec<Execution>>;

    /// Conta execuções com filtros
    fn count_by_filter(&self, filter: ExecutionFilter) -> AsyncResult<usize>;

    /// Busca execuções de um workflow específico
    fn find_by_workflow(&self, workflow_id: EntityId) -> AsyncResult<Vec<Execution>>;

    /// Busca execuções iniciadas por um usuário
    fn find_by_initiator(&self, user_id: EntityId) -> AsyncResult<Vec<Execution>>;

    /// Busca execuções pendentes para um worker processar
    fn find_pending_executions(&self, limit: usize) -> AsyncResult<Vec<Execution>>;

    /// Busca execuções com timeout (rodando há mais tempo que o configurado)
    fn find_timed_out_executions(&self) -> AsyncResult<Vec<Execution>>;

    /// Adiciona um resultado de nó a uma execução
    fn add_node_result(
        &self,
        execution_id: EntityId,
        result: NodeExecutionResult,
    ) -> AsyncResult<()>;

    /// Adiciona logs a uma execução
    fn add_logs(&self, execution_id: EntityId, logs: Vec<ExecutionLog>) -> AsyncResult<()>;

    /// Atualiza o estado de uma execução
    fn update_state(&self, execution_id: EntityId, state: ExecutionState) -> AsyncResult<()>;

    /// Busca execuções filhas de uma execução pai
    fn find_child_executions(&self, parent_execution_id: EntityId) -> AsyncResult<Vec<Execution>>;
}
