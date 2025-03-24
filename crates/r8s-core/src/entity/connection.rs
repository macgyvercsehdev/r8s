// r8s-core/src/entity/connection.rs
//! Define a entidade Connection, que representa uma conexão entre nós em um workflow.

use crate::common::{EntityId, Identifiable, Result, Validatable};
use validator::Validate;

/// Representa uma conexão entre dois nós em um workflow.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Validate)]
pub struct Connection {
    /// Identificador único da conexão
    pub id: EntityId,

    /// ID do nó de origem
    pub source_node: EntityId,

    /// Nome da porta de saída no nó de origem
    #[validate(length(min = 1, max = 255))]
    pub source_output: String,

    /// ID do nó de destino
    pub target_node: EntityId,

    /// Nome da porta de entrada no nó de destino
    #[validate(length(min = 1, max = 255))]
    pub target_input: String,

    /// Condição opcional para ativação da conexão
    #[serde(default)]
    pub condition: Option<ConnectionCondition>,

    /// Transformação opcional a ser aplicada aos dados
    #[serde(default)]
    pub transform: Option<String>,

    /// Metadados da conexão (estilo visual, etc)
    #[serde(default)]
    pub metadata: serde_json::Value,
}

/// Condição para ativação de uma conexão
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConnectionCondition {
    /// Tipo de condição
    pub condition_type: ConditionType,

    /// Expressão de condição (JavaScript)
    pub expression: String,
}

/// Tipos de condição para conexões
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConditionType {
    /// Expressão JavaScript
    JavaScript,

    /// Condição baseada em JSONPath
    JSONPath,

    /// Regra de negócio
    BusinessRule,
}

impl Connection {
    /// Cria uma nova conexão entre dois nós
    pub fn new(
        source_node: EntityId,
        source_output: String,
        target_node: EntityId,
        target_input: String,
    ) -> Self {
        Self {
            id: EntityId::new_v4(),
            source_node,
            source_output,
            target_node,
            target_input,
            condition: None,
            transform: None,
            metadata: serde_json::Value::Object(serde_json::Map::new()),
        }
    }

    /// Define uma condição para a conexão
    pub fn with_condition(mut self, condition_type: ConditionType, expression: String) -> Self {
        self.condition = Some(ConnectionCondition {
            condition_type,
            expression,
        });
        self
    }

    /// Define uma transformação para a conexão
    pub fn with_transform(mut self, transform: String) -> Self {
        self.transform = Some(transform);
        self
    }

    /// Adiciona metadados à conexão
    pub fn with_metadata(mut self, key: &str, value: impl serde::Serialize) -> Result<Self> {
        let value = serde_json::to_value(value)
            .map_err(|e| crate::error::Error::Serialization(e.to_string()))?;

        if let serde_json::Value::Object(metadata) = &mut self.metadata {
            metadata.insert(key.to_string(), value);
        } else {
            let mut map = serde_json::Map::new();
            map.insert(key.to_string(), value);
            self.metadata = serde_json::Value::Object(map);
        }

        Ok(self)
    }
}

impl Identifiable for Connection {
    fn id(&self) -> EntityId {
        self.id
    }
}

impl Validatable for Connection {
    fn validate(&self) -> Result<()> {
        // Usa o validator para validar os campos
        validator::Validate::validate(self)?;

        // Verificar se a origem e o destino não são o mesmo nó
        if self.source_node == self.target_node {
            return Err(crate::error::Error::Validation(
                "Conexão não pode ter o mesmo nó como origem e destino".to_string(),
            ));
        }

        Ok(())
    }
}
