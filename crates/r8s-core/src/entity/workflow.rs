// r8s-core/src/entity/workflow.rs
//! Define a entidade Workflow, que é a unidade central do sistema.

use std::collections::HashMap;
use validator::Validate;

use crate::common::{EntityId, Identifiable, Result, Timestamp, Validatable};
use crate::entity::{Connection, Node, Tag};
use crate::error::Error;

/// Representa um workflow completo no sistema r8s.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Validate)]
pub struct Workflow {
    /// Identificador único do workflow
    #[validate(length(min = 1))]
    pub id: EntityId,

    /// Nome descritivo do workflow
    #[validate(length(min = 1, max = 255))]
    pub name: String,

    /// Descrição opcional do workflow
    #[validate(length(max = 2000))]
    pub description: Option<String>,

    /// Versão do workflow para controle de concorrência
    pub version: u32,

    /// Indica se o workflow está ativo ou não
    pub active: bool,

    /// Nós que compõem o workflow
    #[validate]
    pub nodes: Vec<Node>,

    /// Conexões entre os nós
    #[validate]
    pub connections: Vec<Connection>,

    /// Flag de execução estática (todos os nós são executados) ou dinâmica (baseada em condições)
    pub static_data: bool,

    /// Configurações de pipeline (paralelismo, retry, etc)
    #[serde(default)]
    pub settings: WorkflowSettings,

    /// Metadados do workflow (posição dos nós no editor, etc)
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,

    /// Tags para categorização
    #[serde(default)]
    pub tags: Vec<Tag>,

    /// Timestamps de criação e atualização
    #[serde(default)]
    pub timestamps: Timestamp,
}

/// Configurações específicas para execução do workflow
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct WorkflowSettings {
    /// Nível máximo de execução paralela (0 = ilimitado)
    pub max_parallel: u32,

    /// Timeout em segundos para execução completa (0 = sem timeout)
    pub timeout_seconds: u32,

    /// Política de retry automático
    pub retry_policy: RetryPolicy,

    /// Modo de execução (padrão, debug, etc)
    pub execution_mode: ExecutionMode,

    /// Configurações de agendamento (cron, interval)
    pub schedule: Option<ScheduleSettings>,

    /// Guardar histórico de execuções por quantos dias (0 = para sempre)
    pub history_retention_days: u32,

    /// Variáveis definidas no escopo do workflow
    pub variables: HashMap<String, String>,
}

/// Política de retry para execuções que falham
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct RetryPolicy {
    /// Número máximo de tentativas (0 = sem retry)
    pub max_attempts: u32,

    /// Intervalo inicial entre tentativas (segundos)
    pub initial_interval_seconds: u32,

    /// Fator de backoff (multiplicador para cada tentativa subsequente)
    pub backoff_factor: f32,

    /// Intervalo máximo entre tentativas (segundos)
    pub max_interval_seconds: u32,
}

/// Modo de execução do workflow
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    /// Modo normal de produção
    Normal,

    /// Modo de debug com mais logs
    Debug,

    /// Modo manual (somente executa quando explicitamente solicitado)
    Manual,
}

impl Default for ExecutionMode {
    fn default() -> Self {
        Self::Normal
    }
}

/// Configurações de agendamento para execução periódica
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScheduleSettings {
    /// Expressão cron (ex: "0 0 * * *" para diariamente à meia-noite)
    Cron(String),

    /// Intervalo fixo
    Interval {
        /// Intervalo em segundos
        seconds: u32,

        /// Número máximo de execuções (0 = infinito)
        max_runs: u32,
    },
}

impl Workflow {
    /// Cria um novo workflow com valores padrão
    pub fn new(name: String, description: Option<String>) -> Self {
        Self {
            id: EntityId::new_v4(),
            name,
            description,
            version: 1,
            active: false,
            nodes: Vec::new(),
            connections: Vec::new(),
            static_data: true,
            settings: WorkflowSettings::default(),
            metadata: HashMap::new(),
            tags: Vec::new(),
            timestamps: Timestamp::new(),
        }
    }

    /// Adiciona um nó ao workflow
    pub fn add_node(&mut self, node: Node) -> Result<()> {
        // Verifica se já existe um nó com o mesmo ID
        if self.nodes.iter().any(|n| n.id == node.id) {
            return Err(Error::Conflict(format!("Nó com ID {} já existe", node.id)));
        }

        self.nodes.push(node);
        self.timestamps.update();
        Ok(())
    }

    /// Adiciona uma conexão ao workflow
    pub fn add_connection(&mut self, connection: Connection) -> Result<()> {
        // Verifica se os nós da conexão existem
        let source_exists = self.nodes.iter().any(|n| n.id == connection.source_node);
        let target_exists = self.nodes.iter().any(|n| n.id == connection.target_node);

        if !source_exists {
            return Err(Error::NotFound {
                entity: "Node".to_string(),
                id: connection.source_node.to_string(),
            });
        }

        if !target_exists {
            return Err(Error::NotFound {
                entity: "Node".to_string(),
                id: connection.target_node.to_string(),
            });
        }

        // Verifica se já existe uma conexão idêntica
        if self.connections.iter().any(|c| {
            c.source_node == connection.source_node
                && c.target_node == connection.target_node
                && c.source_output == connection.source_output
                && c.target_input == connection.target_input
        }) {
            return Err(Error::Conflict("Conexão duplicada".to_string()));
        }

        self.connections.push(connection);
        self.timestamps.update();
        Ok(())
    }

    /// Remove um nó e todas as suas conexões
    pub fn remove_node(&mut self, node_id: EntityId) -> Result<()> {
        if !self.nodes.iter().any(|n| n.id == node_id) {
            return Err(Error::NotFound {
                entity: "Node".to_string(),
                id: node_id.to_string(),
            });
        }

        // Remove o nó
        self.nodes.retain(|n| n.id != node_id);

        // Remove conexões relacionadas ao nó
        self.connections
            .retain(|c| c.source_node != node_id && c.target_node != node_id);

        self.timestamps.update();
        Ok(())
    }

    /// Incrementa a versão do workflow
    pub fn increment_version(&mut self) {
        self.version += 1;
        self.timestamps.update();
    }
}

impl Identifiable for Workflow {
    fn id(&self) -> EntityId {
        self.id
    }
}

impl Validatable for Workflow {
    fn validate(&self) -> Result<()> {
        // Usa o validator para validar os campos
        validator::Validate::validate(self)?;

        // Validações adicionais específicas
        // 1. Verifica existência de ao menos um nó
        if self.nodes.is_empty() {
            return Err(Error::Validation(
                "Workflow deve ter ao menos um nó".to_string(),
            ));
        }

        // 2. Verifica conexões com nós que não existem
        for connection in &self.connections {
            let source_exists = self.nodes.iter().any(|n| n.id == connection.source_node);
            let target_exists = self.nodes.iter().any(|n| n.id == connection.target_node);

            if !source_exists {
                return Err(Error::Validation(format!(
                    "Conexão referencia nó de origem inexistente: {}",
                    connection.source_node
                )));
            }

            if !target_exists {
                return Err(Error::Validation(format!(
                    "Conexão referencia nó de destino inexistente: {}",
                    connection.target_node
                )));
            }
        }

        // 3. Verifica a presença de ciclos no grafo
        if self.has_cycles() {
            return Err(Error::Validation("Workflow contém ciclos".to_string()));
        }

        Ok(())
    }
}

impl Workflow {
    /// Verifica se o grafo de workflow contém ciclos
    fn has_cycles(&self) -> bool {
        let mut visited = std::collections::HashSet::new();
        let mut stack = std::collections::HashSet::new();

        // Para cada nó, tenta encontrar um ciclo
        for node in &self.nodes {
            if self.has_cycle_dfs(node.id, &mut visited, &mut stack) {
                return true;
            }
        }

        false
    }

    // Implementação de busca em profundidade para detectar ciclos
    fn has_cycle_dfs(
        &self,
        node_id: EntityId,
        visited: &mut std::collections::HashSet<EntityId>,
        stack: &mut std::collections::HashSet<EntityId>,
    ) -> bool {
        // Se já visitamos este nó nesta busca, encontramos um ciclo
        if stack.contains(&node_id) {
            return true;
        }

        // Se já visitamos este nó em outra busca, não precisamos revisitá-lo
        if visited.contains(&node_id) {
            return false;
        }

        // Marca o nó como visitado
        visited.insert(node_id);
        stack.insert(node_id);

        // Visita todos os nós vizinhos (conectados)
        for connection in &self.connections {
            if connection.source_node == node_id {
                if self.has_cycle_dfs(connection.target_node, visited, stack) {
                    return true;
                }
            }
        }

        // Remove o nó do stack (retorno da recursão)
        stack.remove(&node_id);

        false
    }
}
