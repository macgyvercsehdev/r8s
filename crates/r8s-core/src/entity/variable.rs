// r8s-core/src/entity/variable.rs
//! Define a entidade Variable, que representa variáveis de ambiente ou configuração.

use validator::Validate;

use crate::common::{EntityId, Identifiable, Result, Timestamp, Validatable};
use crate::error::Error;

/// Representa uma variável de ambiente ou configuração.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Validate)]
pub struct Variable {
    /// Identificador único da variável
    pub id: EntityId,

    /// Chave da variável
    #[validate(length(min = 1, max = 255))]
    pub key: String,

    /// Valor da variável
    pub value: String,

    /// Tipo da variável
    pub var_type: VariableType,

    /// Escopo da variável
    pub scope: VariableScope,

    /// Se a variável está protegida (sensível)
    pub protected: bool,

    /// Descrição da variável
    #[serde(default)]
    pub description: Option<String>,

    /// Timestamps de criação e atualização
    #[serde(default)]
    pub timestamps: Timestamp,
}

/// Tipo de variável
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VariableType {
    /// String simples
    String,

    /// Número
    Number,

    /// Booleano
    Boolean,

    /// JSON
    Json,

    /// Binário (base64)
    Binary,
}

/// Escopo da variável
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VariableScope {
    /// Global para todo o sistema
    Global,

    /// Específica para um workflow
    Workflow(EntityId),

    /// Específica para um usuário
    User(EntityId),

    /// Específica para um ambiente
    Environment(String),
}

impl Variable {
    /// Cria uma nova variável
    pub fn new(
        key: String,
        value: String,
        var_type: VariableType,
        scope: VariableScope,
        protected: bool,
    ) -> Self {
        Self {
            id: EntityId::new_v4(),
            key,
            value,
            var_type,
            scope,
            protected,
            description: None,
            timestamps: Timestamp::new(),
        }
    }

    /// Cria uma nova variável global
    pub fn new_global(key: String, value: String, var_type: VariableType, protected: bool) -> Self {
        Self::new(key, value, var_type, VariableScope::Global, protected)
    }

    /// Cria uma nova variável para um workflow específico
    pub fn new_for_workflow(
        key: String,
        value: String,
        var_type: VariableType,
        workflow_id: EntityId,
        protected: bool,
    ) -> Self {
        Self::new(
            key,
            value,
            var_type,
            VariableScope::Workflow(workflow_id),
            protected,
        )
    }

    /// Cria uma nova variável para um usuário específico
    pub fn new_for_user(
        key: String,
        value: String,
        var_type: VariableType,
        user_id: EntityId,
        protected: bool,
    ) -> Self {
        Self::new(
            key,
            value,
            var_type,
            VariableScope::User(user_id),
            protected,
        )
    }

    /// Cria uma nova variável para um ambiente específico
    pub fn new_for_environment(
        key: String,
        value: String,
        var_type: VariableType,
        environment: String,
        protected: bool,
    ) -> Self {
        Self::new(
            key,
            value,
            var_type,
            VariableScope::Environment(environment),
            protected,
        )
    }

    /// Atualiza o valor da variável
    pub fn update_value(&mut self, value: String) {
        self.value = value;
        self.timestamps.update();
    }

    /// Define a proteção da variável
    pub fn set_protected(&mut self, protected: bool) {
        self.protected = protected;
        self.timestamps.update();
    }

    /// Obtém um identificador composto para a variável (escopo + chave)
    pub fn full_key(&self) -> String {
        match &self.scope {
            VariableScope::Global => format!("global:{}", self.key),
            VariableScope::Workflow(id) => format!("workflow:{}:{}", id, self.key),
            VariableScope::User(id) => format!("user:{}:{}", id, self.key),
            VariableScope::Environment(env) => format!("env:{}:{}", env, self.key),
        }
    }

    /// Converte a variável para um valor tipado conforme seu tipo
    pub fn typed_value(&self) -> Result<serde_json::Value> {
        match self.var_type {
            VariableType::String => Ok(serde_json::Value::String(self.value.clone())),

            VariableType::Number => {
                if let Ok(num) = self.value.parse::<i64>() {
                    Ok(serde_json::Value::Number(num.into()))
                } else if let Ok(num) = self.value.parse::<f64>() {
                    match serde_json::Number::from_f64(num) {
                        Some(n) => Ok(serde_json::Value::Number(n)),
                        None => Err(Error::Validation(format!(
                            "Valor '{}' não é um número válido",
                            self.value
                        ))),
                    }
                } else {
                    Err(Error::Validation(format!(
                        "Valor '{}' não é um número válido",
                        self.value
                    )))
                }
            }

            VariableType::Boolean => {
                let lower = self.value.to_lowercase();
                match lower.as_str() {
                    "true" | "1" | "yes" | "y" => Ok(serde_json::Value::Bool(true)),
                    "false" | "0" | "no" | "n" => Ok(serde_json::Value::Bool(false)),
                    _ => Err(Error::Validation(format!(
                        "Valor '{}' não é um booleano válido",
                        self.value
                    ))),
                }
            }

            VariableType::Json => serde_json::from_str(&self.value).map_err(|e| {
                Error::Validation(format!(
                    "Valor '{}' não é um JSON válido: {}",
                    self.value, e
                ))
            }),

            VariableType::Binary => Ok(serde_json::Value::String(self.value.clone())),
        }
    }
}

impl Identifiable for Variable {
    fn id(&self) -> EntityId {
        self.id
    }
}

impl Validatable for Variable {
    fn validate(&self) -> Result<()> {
        // Usa o validator para validar os campos
        validator::Validate::validate(self)?;

        // Validações adicionais específicas para cada tipo
        match self.var_type {
            VariableType::Number => {
                if self.value.parse::<f64>().is_err() {
                    return Err(Error::Validation(format!(
                        "Valor '{}' não é um número válido",
                        self.value
                    )));
                }
            }

            VariableType::Boolean => {
                let lower = self.value.to_lowercase();
                if !["true", "false", "1", "0", "yes", "no", "y", "n"].contains(&lower.as_str()) {
                    return Err(Error::Validation(format!(
                        "Valor '{}' não é um booleano válido",
                        self.value
                    )));
                }
            }

            VariableType::Json => {
                if serde_json::from_str::<serde_json::Value>(&self.value).is_err() {
                    return Err(Error::Validation(format!(
                        "Valor '{}' não é um JSON válido",
                        self.value
                    )));
                }
            }

            _ => {}
        }

        Ok(())
    }
}
