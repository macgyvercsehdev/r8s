// r8s-core/src/entity/node.rs
//! Define a entidade Node, que representa um nó em um workflow.

use std::collections::HashMap;
use validator::Validate;

use crate::common::{EntityId, Identifiable, Result, Timestamp, Validatable};
use crate::error::Error;

/// Representa um nó no workflow.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Validate)]
pub struct Node {
    /// Identificador único do nó
    pub id: EntityId,

    /// Nome do nó para identificação
    #[validate(length(min = 1, max = 255))]
    pub name: String,

    /// Tipo do nó
    pub type_info: NodeType,

    /// Posição do nó no editor visual (x, y)
    pub position: Position,

    /// Parâmetros específicos do nó
    pub parameters: serde_json::Value,

    /// Entradas do nó
    pub inputs: Vec<NodePort>,

    /// Saídas do nó
    pub outputs: Vec<NodePort>,

    /// Credenciais necessárias para este nó
    #[serde(default)]
    pub credentials: Vec<EntityId>,

    /// Notas e documentação do nó
    #[serde(default)]
    pub notes: Option<String>,

    /// Se o nó é desabilitado na execução
    #[serde(default)]
    pub disabled: bool,

    /// Configurações específicas do nó (timeout, retry, etc)
    #[serde(default)]
    pub settings: NodeSettings,

    /// Metadados adicionais
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,

    /// Timestamps de criação e atualização
    #[serde(default)]
    pub timestamps: Timestamp,
}

/// Representa a posição visual de um nó no editor
#[derive(Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
pub struct Position {
    /// Coordenada X
    pub x: f32,

    /// Coordenada Y
    pub y: f32,
}

/// Porta de entrada ou saída de um nó
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Validate)]
pub struct NodePort {
    /// Nome da porta
    #[validate(length(min = 1, max = 255))]
    pub name: String,

    /// Tipo de dados da porta
    pub data_type: DataType,

    /// Descrição da porta
    #[serde(default)]
    pub description: Option<String>,

    /// Indica se a porta é necessária
    #[serde(default)]
    pub required: bool,

    /// Exemplos de dados válidos
    #[serde(default)]
    pub examples: Vec<serde_json::Value>,

    /// Esquema JSON para validação
    #[serde(default)]
    pub schema: Option<serde_json::Value>,
}

/// Configurações específicas para um nó
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct NodeSettings {
    /// Timeout em segundos para execução do nó
    pub timeout_seconds: u32,

    /// Número de retentativas em caso de falha
    pub retry_count: u32,

    /// Intervalo entre retentativas (segundos)
    pub retry_interval_seconds: u32,

    /// Ambiente de execução para código customizado
    pub execution_environment: Option<ExecutionEnvironment>,

    /// Limites de recursos para execução
    pub resource_limits: Option<ResourceLimits>,
}

/// Ambiente de execução para código customizado
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionEnvironment {
    /// JavaScript (V8)
    JavaScript,

    /// Lua
    Lua,

    /// WebAssembly
    Wasm,

    /// Python (via WASM)
    Python,
}

/// Limites de recursos para execução de nós
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ResourceLimits {
    /// Limite de memória em MB
    pub memory_mb: u32,

    /// Limite de tempo de CPU em segundos
    pub cpu_seconds: u32,

    /// Número máximo de operações de I/O
    pub io_operations: u32,

    /// Tamanho máximo de resposta em MB
    pub max_response_size_mb: u32,
}

/// Tipo de dados para as portas
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DataType {
    /// String de texto
    String,

    /// Número
    Number,

    /// Booleano
    Boolean,

    /// Data/hora
    DateTime,

    /// Array de qualquer tipo
    Array,

    /// Objeto/dicionário
    Object,

    /// Dados binários
    Binary,

    /// Tipo customizado
    Custom(String),

    /// Qualquer tipo (genérico)
    Any,
}

/// Informações do tipo de nó
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NodeType {
    /// Nome do tipo de nó
    pub name: String,

    /// Versão do tipo de nó
    pub version: String,

    /// Categoria do tipo
    pub category: NodeTypeCategory,

    /// ID do plugin que fornece este tipo (se aplicável)
    pub plugin_id: Option<EntityId>,

    /// Ícone do tipo de nó
    pub icon: Option<String>,

    /// Cor para representação visual
    pub color: Option<String>,

    /// Suporta execução de código customizado
    pub supports_custom_code: bool,
}

/// Categoria para tipos de nós
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeTypeCategory {
    /// Nós de trigger/início
    Trigger,

    /// Operações core do sistema
    Core,

    /// Interações com serviços externos
    Integration,

    /// Transformação de dados
    Transformation,

    /// Fluxo de controle (if, switch, loop)
    Flow,

    /// Geração de conteúdo
    Generator,

    /// AI/ML
    AI,

    /// Entrada/saída de dados
    IO,

    /// Programação e código
    Code,

    /// Utilitários diversos
    Utility,
}

impl Node {
    /// Cria um novo nó com valores padrão
    pub fn new(name: String, type_info: NodeType, position: Position) -> Self {
        Self {
            id: EntityId::new_v4(),
            name,
            type_info,
            position,
            parameters: serde_json::Value::Object(serde_json::Map::new()),
            inputs: Vec::new(),
            outputs: Vec::new(),
            credentials: Vec::new(),
            notes: None,
            disabled: false,
            settings: NodeSettings::default(),
            metadata: HashMap::new(),
            timestamps: Timestamp::new(),
        }
    }

    /// Adiciona uma porta de entrada ao nó
    pub fn add_input(&mut self, input: NodePort) -> Result<()> {
        // Verifica se já existe uma entrada com o mesmo nome
        if self.inputs.iter().any(|i| i.name == input.name) {
            return Err(Error::Conflict(format!(
                "Entrada com nome '{}' já existe",
                input.name
            )));
        }

        self.inputs.push(input);
        Ok(())
    }

    /// Adiciona uma porta de saída ao nó
    pub fn add_output(&mut self, output: NodePort) -> Result<()> {
        // Verifica se já existe uma saída com o mesmo nome
        if self.outputs.iter().any(|o| o.name == output.name) {
            return Err(Error::Conflict(format!(
                "Saída com nome '{}' já existe",
                output.name
            )));
        }

        self.outputs.push(output);
        Ok(())
    }

    /// Adiciona uma credencial necessária para este nó
    pub fn add_credential(&mut self, credential_id: EntityId) -> Result<()> {
        if self.credentials.contains(&credential_id) {
            return Err(Error::Conflict(
                "Credencial já adicionada ao nó".to_string(),
            ));
        }

        self.credentials.push(credential_id);
        Ok(())
    }

    /// Define um parâmetro do nó
    pub fn set_parameter<T: serde::Serialize>(&mut self, key: &str, value: T) -> Result<()> {
        let value = serde_json::to_value(value).map_err(|e| Error::Serialization(e.to_string()))?;

        if let serde_json::Value::Object(params) = &mut self.parameters {
            params.insert(key.to_string(), value);
            Ok(())
        } else {
            self.parameters = serde_json::Value::Object(serde_json::Map::new());
            if let serde_json::Value::Object(params) = &mut self.parameters {
                params.insert(key.to_string(), value);
                Ok(())
            } else {
                Err(Error::Internal(
                    "Falha ao criar parâmetros do nó".to_string(),
                ))
            }
        }
    }

    /// Obtém um parâmetro do nó
    pub fn get_parameter<T: serde::de::DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        if let serde_json::Value::Object(params) = &self.parameters {
            if let Some(value) = params.get(key) {
                let result = serde_json::from_value(value.clone())
                    .map_err(|e| Error::Serialization(e.to_string()))?;
                Ok(Some(result))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    /// Verifica se a porta de entrada com o nome dado existe
    pub fn has_input(&self, name: &str) -> bool {
        self.inputs.iter().any(|i| i.name == name)
    }

    /// Verifica se a porta de saída com o nome dado existe
    pub fn has_output(&self, name: &str) -> bool {
        self.outputs.iter().any(|o| o.name == name)
    }
}

impl Identifiable for Node {
    fn id(&self) -> EntityId {
        self.id
    }
}

impl Validatable for Node {
    fn validate(&self) -> Result<()> {
        // Usa o validator para validar os campos
        validator::Validate::validate(self)?;

        // Validações adicionais específicas
        let inputs_unique = self
            .inputs
            .iter()
            .map(|i| &i.name)
            .collect::<std::collections::HashSet<_>>()
            .len()
            == self.inputs.len();

        if !inputs_unique {
            return Err(Error::Validation(
                "Nomes de entradas devem ser únicos".to_string(),
            ));
        }

        let outputs_unique = self
            .outputs
            .iter()
            .map(|o| &o.name)
            .collect::<std::collections::HashSet<_>>()
            .len()
            == self.outputs.len();

        if !outputs_unique {
            return Err(Error::Validation(
                "Nomes de saídas devem ser únicos".to_string(),
            ));
        }

        Ok(())
    }
}
