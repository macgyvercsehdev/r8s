// r8s-core/src/use_case/execution.rs
//! Casos de uso relacionados a execuções de workflows.

use std::sync::Arc;

use crate::common::{EntityId, ExecutionState, Result, Priority};
use crate::entity::{Execution, ExecutionLog, NodeExecutionResult, LogLevel};
use crate::error::Error;
use crate::repository::{WorkflowRepository, ExecutionRepository};

/// Caso de uso para iniciar uma execução de workflow
pub struct StartWorkflowUseCase<W: WorkflowRepository, E: ExecutionRepository> {
    workflow_repository: Arc<W>,
    execution_repository: Arc<E>,
}

/// Entrada para o caso de uso de início de execução
pub struct StartWorkflowInput {
    /// ID do workflow a ser executado
    pub workflow_id: EntityId,
    
    /// Dados iniciais para a execução
    pub initial_data: Option<serde_json::Value>,
    
    /// ID do usuário que está iniciando a execução
    pub initiated_by: Option<EntityId>,
    
    /// Prioridade da execução
    pub priority: Option<Priority>,
    
    /// Tags para a execução
    pub tags: Vec<String>,
}

impl<W: WorkflowRepository, E: ExecutionRepository> StartWorkflowUseCase<W, E> {
    /// Cria uma nova instância do caso de uso
    pub fn new(workflow_repository: Arc<W>, execution_repository: Arc<E>) -> Self {
        Self {
            workflow_repository,
            execution_repository,
        }
    }
    
    /// Executa o caso de uso
    pub async fn execute(&self, input: StartWorkflowInput) -> Result<Execution> {
        // Busca o workflow
        let workflow = self.workflow_repository.find_by_id(input.workflow_id).await?;
        
        // Verifica se o workflow está ativo
        if !workflow.active {
            return Err(Error::WorkflowExecution(
                format!("Workflow {} não está ativo", workflow.id)
            ));
        }
        
        // Cria a nova execução
        let mut execution = Execution::new(
            workflow.id,
            workflow.version,
            input.initial_data,
        );
        
        // Define campos adicionais
        execution.initiated_by = input.initiated_by;
        
        if let Some(priority) = input.priority {
            execution.priority = priority;
        }
        
        execution.tags = input.tags;
        
        // Adiciona log inicial
        execution.add_log(
            LogLevel::Info,
            &format!("Execução iniciada para o workflow '{}' (v{})", workflow.name, workflow.version),
            None,
            None,
        );
        
        // Salva a execução
        let execution = self.execution_repository.save(execution).await?;
        
        Ok(execution)
    }
}

/// Caso de uso para registrar resultado de nó em uma execução
pub struct RecordNodeResultUseCase<E: ExecutionRepository> {
    execution_repository: Arc<E>,
}

impl<E: ExecutionRepository> RecordNodeResultUseCase<E> {
    /// Cria uma nova instância do caso de uso
    pub fn new(execution_repository: Arc<E>) -> Self {
        Self {
            execution_repository,
        }
    }
    
    /// Executa o caso de uso
    pub async fn execute(&self, execution_id: EntityId, result: NodeExecutionResult) -> Result<()> {
        // Adiciona o resultado do nó à execução
        self.execution_repository.add_node_result(execution_id, result).await?;
        
        Ok(())
    }
}

/// Caso de uso para completar uma execução
pub struct CompleteExecutionUseCase<E: ExecutionRepository> {
    execution_repository: Arc<E>,
}

impl<E: ExecutionRepository> CompleteExecutionUseCase<E> {
    /// Cria uma nova instância do caso de uso
    pub fn new(execution_repository: Arc<E>) -> Self {
        Self {
            execution_repository,
        }
    }
    
    /// Executa o caso de uso
    pub async fn execute(&self, execution_id: EntityId) -> Result<Execution> {
        // Busca a execução
        let mut execution = self.execution_repository.find_by_id(execution_id).await?;
        
        // Verifica se a execução está em andamento
        if execution.state != ExecutionState::Running {
            return Err(Error::WorkflowExecution(
                format!("Execução {} não está em andamento", execution_id)
            ));
        }
        
        // Marca a execução como completa
        execution.complete();
        
        // Salva a execução
        let execution = self.execution_repository.save(execution).await?;
        
        Ok(execution)
    }
}

/// Caso de uso para falhar uma execução
pub struct FailExecutionUseCase<E: ExecutionRepository> {
    execution_repository: Arc<E>,
}

impl<E: ExecutionRepository> FailExecutionUseCase<E> {
    /// Cria uma nova instância do caso de uso
    pub fn new(execution_repository: Arc<E>) -> Self {
        Self {
            execution_repository,
        }
    }
    
    /// Executa o caso de uso
    pub async fn execute(
        &self, 
        execution_id: EntityId,
        reason: String,
        failed_node_id: Option<EntityId>
    ) -> Result<Execution> {
        // Busca a execução
        let mut execution = self.execution_repository.find_by_id(execution_id).await?;
        
        // Verifica se a execução está em andamento
        if execution.state != ExecutionState::Running && 
           execution.state != ExecutionState::Waiting {
            return Err(Error::WorkflowExecution(
                format!("Execução {} não está em andamento ou em espera", execution_id)
            ));
        }
        
        // Marca a execução como falha
        execution.fail(reason, failed_node_id);
        
        // Salva a execução
        let execution = self.execution_repository.save(execution).await?;
        
        Ok(execution)
    }
}

/// Caso de uso para cancelar uma execução
pub struct CancelExecutionUseCase<E: ExecutionRepository> {
    execution_repository: Arc<E>,
}

impl<E: ExecutionRepository> CancelExecutionUseCase<E> {
    /// Cria uma nova instância do caso de uso
    pub fn new(execution_repository: Arc<E>) -> Self {
        Self {
            execution_repository,
        }
    }
    
    /// Executa o caso de uso
    pub async fn execute(
        &self, 
        execution_id: EntityId,
        reason: Option<String>
    ) -> Result<Execution> {
        // Busca a execução
        let mut execution = self.execution_repository.find_by_id(execution_id).await?;
        
        // Verifica se a execução pode ser cancelada
        if execution.is_finished() {
            return Err(Error::WorkflowExecution(
                format!("Execução {} já está finalizada", execution_id)
            ));
        }
        
        // Cancela a execução
        execution.cancel(reason);
        
        // Salva a execução
        let execution = self.execution_repository.save(execution).await?;
        
        Ok(execution)
    }
}

/// Caso de uso para listar execuções com filtros
pub struct ListExecutionsUseCase<E: ExecutionRepository> {
    execution_repository: Arc<E>,
}

impl<E: ExecutionRepository> ListExecutionsUseCase<E> {
    /// Cria uma nova instância do caso de uso
    pub fn new(execution_repository: Arc<E>) -> Self {
        Self {
            execution_repository,
        }
    }
    
    /// Executa o caso de uso
    pub async fn execute(&self, filter: crate::repository::execution_repository::ExecutionFilter) -> Result<Vec<Execution>> {
        let executions = self.execution_repository.find_by_filter(filter).await?;
        Ok(executions)
    }
}

/// Caso de uso para obter execuções pendentes para processamento
pub struct GetPendingExecutionsUseCase<E: ExecutionRepository> {
    execution_repository: Arc<E>,
}

impl<E: ExecutionRepository> GetPendingExecutionsUseCase<E> {
    /// Cria uma nova instância do caso de uso
    pub fn new(execution_repository: Arc<E>) -> Self {
        Self {
            execution_repository,
        }
    }
    
    /// Executa o caso de uso
    pub async fn execute(&self, limit: usize) -> Result<Vec<Execution>> {
        let executions = self.execution_repository.find_pending_executions(limit).await?;
        Ok(executions)
    }
}

/// Caso de uso para adicionar logs a uma execução
pub struct AddExecutionLogsUseCase<E: ExecutionRepository> {
    execution_repository: Arc<E>,
}

impl<E: ExecutionRepository> AddExecutionLogsUseCase<E> {
    /// Cria uma nova instância do caso de uso
    pub fn new(execution_repository: Arc<E>) -> Self {
        Self {
            execution_repository,
        }
    }
    
    /// Executa o caso de uso
    pub async fn execute(&self, execution_id: EntityId, logs: Vec<ExecutionLog>) -> Result<()> {
        self.execution_repository.add_logs(execution_id, logs).await?;
        Ok(())
    }
}