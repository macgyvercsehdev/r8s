// r8s-core/src/entity/execution.rs
//! Define a entidade Execution, que representa a execução de um workflow.

use chrono::{DateTime, Utc};
use std::collections::HashMap;

use crate::common::{
    EntityId, ExecutionState, Identifiable, Priority, Result, Timestamp, Validatable,
};
use crate::error::Error;

/// Representa a execução de um workflow.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Execution {
    /// Identificador único da execução
    pub id: EntityId,

    /// ID do workflow sendo executado
    pub workflow_id: EntityId,

    /// Versão do workflow sendo executado
    pub workflow_version: u32,

    /// Estado atual da execução
    pub state: ExecutionState,

    /// Resultados da execução (por nó)
    pub results: HashMap<EntityId, NodeExecutionResult>,

    /// Dados iniciais para a execução
    pub initial_data: Option<serde_json::Value>,

    /// Hora de início da execução
    pub started_at: DateTime<Utc>,

    /// Hora de finalização da execução (se finalizada)
    pub finished_at: Option<DateTime<Utc>>,

    /// Tempo total de execução em milissegundos
    pub duration_ms: Option<u64>,

    /// ID do usuário que iniciou a execução
    pub initiated_by: Option<EntityId>,

    /// Prioridade da execução
    pub priority: Priority,

    /// Número da tentativa (para retries)
    pub attempt: u32,

    /// Motivo de falha (se falhou)
    pub failure_reason: Option<String>,

    /// ID do nó que falhou (se falhou)
    pub failed_node_id: Option<EntityId>,

    /// ID da execução pai (se esta for parte de um workflow pai)
    pub parent_execution_id: Option<EntityId>,

    /// IDs de execuções filhas (para sub-workflows)
    pub child_execution_ids: Vec<EntityId>,

    /// ID do worker que está processando esta execução
    pub worker_id: Option<String>,

    /// Tags para categorização
    pub tags: Vec<String>,

    /// Logs da execução
    pub logs: Vec<ExecutionLog>,

    /// Timestamps de criação e atualização
    pub timestamps: Timestamp,
}

/// Representa o resultado da execução de um nó individual.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NodeExecutionResult {
    /// ID do nó
    pub node_id: EntityId,

    /// Estado da execução deste nó
    pub state: ExecutionState,

    /// Hora de início da execução
    pub started_at: DateTime<Utc>,

    /// Hora de finalização da execução (se finalizada)
    pub finished_at: Option<DateTime<Utc>>,

    /// Tempo de execução em milissegundos
    pub duration_ms: Option<u64>,

    /// Dados de entrada para o nó
    pub input_data: HashMap<String, serde_json::Value>,

    /// Dados de saída do nó
    pub output_data: HashMap<String, serde_json::Value>,

    /// Motivo de falha (se falhou)
    pub failure_reason: Option<String>,

    /// Número da tentativa (para retries no nível do nó)
    pub attempt: u32,

    /// ID do worker que processou este nó
    pub worker_id: Option<String>,
}

/// Nível de severidade para logs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    /// Informação detalhada (debug)
    Debug,

    /// Informação normal
    Info,

    /// Aviso (não crítico)
    Warning,

    /// Erro (falha)
    Error,
}

/// Log gerado durante a execução
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExecutionLog {
    /// Hora do log
    pub timestamp: DateTime<Utc>,

    /// Nível de severidade
    pub level: LogLevel,

    /// Mensagem de log
    pub message: String,

    /// ID do nó relacionado (se aplicável)
    pub node_id: Option<EntityId>,

    /// Dados adicionais do log
    pub metadata: Option<serde_json::Value>,
}

/// Resultados gerais da execução
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExecutionResult {
    /// ID da execução
    pub execution_id: EntityId,

    /// ID do workflow
    pub workflow_id: EntityId,

    /// Estado final
    pub state: ExecutionState,

    /// Duração total em milissegundos
    pub duration_ms: u64,

    /// Timestamp de início
    pub started_at: DateTime<Utc>,

    /// Timestamp de fim
    pub finished_at: DateTime<Utc>,

    /// Dados de saída finais
    pub output_data: Option<serde_json::Value>,

    /// Mensagem de erro (se falhou)
    pub error_message: Option<String>,

    /// Dados de erro (se falhou)
    pub error_data: Option<serde_json::Value>,
}

impl Execution {
    /// Cria uma nova execução de workflow
    pub fn new(
        workflow_id: EntityId,
        workflow_version: u32,
        initial_data: Option<serde_json::Value>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: EntityId::new_v4(),
            workflow_id,
            workflow_version,
            state: ExecutionState::Pending,
            results: HashMap::new(),
            initial_data,
            started_at: now,
            finished_at: None,
            duration_ms: None,
            initiated_by: None,
            priority: Priority::default(),
            attempt: 1,
            failure_reason: None,
            failed_node_id: None,
            parent_execution_id: None,
            child_execution_ids: Vec::new(),
            worker_id: None,
            tags: Vec::new(),
            logs: Vec::new(),
            timestamps: Timestamp::new(),
        }
    }

    /// Inicia a execução
    pub fn start(&mut self, worker_id: Option<String>) {
        self.state = ExecutionState::Running;
        self.worker_id = worker_id;
        self.started_at = Utc::now();
        self.add_log(LogLevel::Info, "Execução iniciada", None, None);
    }

    /// Marca a execução como completa com sucesso
    pub fn complete(&mut self) {
        let now = Utc::now();
        self.state = ExecutionState::Success;
        self.finished_at = Some(now);
        self.duration_ms = Some(
            (now - self.started_at)
                .num_milliseconds()
                .try_into()
                .unwrap_or(0),
        );
        self.add_log(
            LogLevel::Info,
            "Execução completada com sucesso",
            None,
            None,
        );
    }

    /// Marca a execução como falha
    pub fn fail(&mut self, reason: String, failed_node_id: Option<EntityId>) {
        let now = Utc::now();
        self.state = ExecutionState::Failed;
        self.failure_reason = Some(reason.clone());
        self.failed_node_id = failed_node_id;
        self.finished_at = Some(now);
        self.duration_ms = Some(
            (now - self.started_at)
                .num_milliseconds()
                .try_into()
                .unwrap_or(0),
        );
        self.add_log(
            LogLevel::Error,
            &format!("Execução falhou: {}", reason),
            failed_node_id,
            None,
        );
    }

    /// Cancela a execução
    pub fn cancel(&mut self, reason: Option<String>) {
        let now = Utc::now();
        self.state = ExecutionState::Cancelled;
        self.failure_reason = reason.clone();
        self.finished_at = Some(now);
        self.duration_ms = Some(
            (now - self.started_at)
                .num_milliseconds()
                .try_into()
                .unwrap_or(0),
        );

        let message = if let Some(reason) = reason {
            format!("Execução cancelada: {}", reason)
        } else {
            "Execução cancelada".to_string()
        };

        self.add_log(LogLevel::Warning, &message, None, None);
    }

    /// Pausa a execução (estado de espera)
    pub fn wait(&mut self, reason: String) {
        self.state = ExecutionState::Waiting;
        self.add_log(
            LogLevel::Info,
            &format!("Execução aguardando: {}", reason),
            None,
            None,
        );
    }

    /// Resume uma execução pausada
    pub fn resume(&mut self) {
        self.state = ExecutionState::Running;
        self.add_log(LogLevel::Info, "Execução retomada", None, None);
    }

    /// Adiciona um resultado de execução de nó
    pub fn add_node_result(&mut self, result: NodeExecutionResult) {
        self.results.insert(result.node_id, result);
    }

    /// Adiciona um log à execução
    pub fn add_log(
        &mut self,
        level: LogLevel,
        message: &str,
        node_id: Option<EntityId>,
        metadata: Option<serde_json::Value>,
    ) {
        let log = ExecutionLog {
            timestamp: Utc::now(),
            level,
            message: message.to_string(),
            node_id,
            metadata,
        };
        self.logs.push(log);
    }

    /// Adiciona uma execução filha (sub-workflow)
    pub fn add_child_execution(&mut self, child_id: EntityId) {
        if !self.child_execution_ids.contains(&child_id) {
            self.child_execution_ids.push(child_id);
        }
    }

    /// Cria um resultado final da execução
    pub fn create_result(&self) -> ExecutionResult {
        let finished_at = self.finished_at.unwrap_or_else(Utc::now);
        let duration_ms = self.duration_ms.unwrap_or_else(|| {
            (finished_at - self.started_at)
                .num_milliseconds()
                .try_into()
                .unwrap_or(0)
        });

        // Encontra o output final (nós sem saída ou fim do workflow)
        let output_data = self.get_final_output();

        ExecutionResult {
            execution_id: self.id,
            workflow_id: self.workflow_id,
            state: self.state,
            duration_ms,
            started_at: self.started_at,
            finished_at,
            output_data,
            error_message: self.failure_reason.clone(),
            error_data: None,
        }
    }

    /// Obtém a saída final agregada do workflow
    fn get_final_output(&self) -> Option<serde_json::Value> {
        if self.results.is_empty() {
            return None;
        }

        // Agregamos todos os resultados em um objeto
        let mut output = serde_json::Map::new();

        for (node_id, result) in &self.results {
            if !result.output_data.is_empty() && result.state == ExecutionState::Success {
                let node_output = serde_json::Value::Object(
                    result
                        .output_data
                        .iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect(),
                );

                output.insert(node_id.to_string(), node_output);
            }
        }

        Some(serde_json::Value::Object(output))
    }

    /// Verifica se todos os nós foram executados com sucesso
    pub fn all_nodes_successful(&self) -> bool {
        !self
            .results
            .values()
            .any(|r| r.state != ExecutionState::Success)
    }

    /// Verifica se a execução já foi finalizada (sucesso, falha ou cancelada)
    pub fn is_finished(&self) -> bool {
        matches!(
            self.state,
            ExecutionState::Success | ExecutionState::Failed | ExecutionState::Cancelled
        )
    }
}

impl NodeExecutionResult {
    /// Cria um novo resultado de execução de nó
    pub fn new(node_id: EntityId) -> Self {
        let now = Utc::now();
        Self {
            node_id,
            state: ExecutionState::Pending,
            started_at: now,
            finished_at: None,
            duration_ms: None,
            input_data: HashMap::new(),
            output_data: HashMap::new(),
            failure_reason: None,
            attempt: 1,
            worker_id: None,
        }
    }

    /// Inicia a execução do nó
    pub fn start(
        &mut self,
        worker_id: Option<String>,
        input_data: HashMap<String, serde_json::Value>,
    ) {
        self.state = ExecutionState::Running;
        self.worker_id = worker_id;
        self.started_at = Utc::now();
        self.input_data = input_data;
    }

    /// Completa a execução do nó com sucesso
    pub fn complete(&mut self, output_data: HashMap<String, serde_json::Value>) {
        let now = Utc::now();
        self.state = ExecutionState::Success;
        self.output_data = output_data;
        self.finished_at = Some(now);
        self.duration_ms = Some(
            (now - self.started_at)
                .num_milliseconds()
                .try_into()
                .unwrap_or(0),
        );
    }

    /// Marca a execução do nó como falha
    pub fn fail(&mut self, reason: String) {
        let now = Utc::now();
        self.state = ExecutionState::Failed;
        self.failure_reason = Some(reason);
        self.finished_at = Some(now);
        self.duration_ms = Some(
            (now - self.started_at)
                .num_milliseconds()
                .try_into()
                .unwrap_or(0),
        );
    }
}

impl Identifiable for Execution {
    fn id(&self) -> EntityId {
        self.id
    }
}

impl Validatable for Execution {
    fn validate(&self) -> Result<()> {
        // Validações específicas para Execution
        if self.workflow_id == EntityId::nil() {
            return Err(Error::Validation("ID de workflow inválido".to_string()));
        }

        if self.workflow_version == 0 {
            return Err(Error::Validation("Versão de workflow inválida".to_string()));
        }

        Ok(())
    }
}
