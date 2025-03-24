// r8s-persistence/src/models/variable.rs
//! Database model for variables.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::uuid::Uuid;

use r8s_core::common::Timestamp;
use r8s_core::entity::{Variable, VariableScope, VariableType};

use crate::error::{PersistenceError, Result};

/// Database model for variables.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct VariableModel {
    /// Primary key
    pub id: Uuid,

    /// Variable key (name)
    pub key: String,

    /// Variable value (as string)
    pub value: String,

    /// Variable type
    pub var_type: String,

    /// Scope type
    pub scope_type: String,

    /// Scope ID (for workflow and user scopes)
    pub scope_id: Option<Uuid>,

    /// Environment name (for environment scope)
    pub environment_name: Option<String>,

    /// Whether the variable is protected
    pub protected: bool,

    /// Description
    pub description: Option<String>,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
}

/// Map a variable type string from the database to the domain enum.
pub fn map_variable_type_from_db(var_type: &str) -> Result<VariableType> {
    match var_type {
        "string" => Ok(VariableType::String),
        "number" => Ok(VariableType::Number),
        "boolean" => Ok(VariableType::Boolean),
        "json" => Ok(VariableType::Json),
        "binary" => Ok(VariableType::Binary),
        _ => Err(PersistenceError::ConversionError(format!(
            "Invalid variable type: {}",
            var_type
        ))),
    }
}

/// Map a domain enum variable type to a database string.
pub fn map_variable_type_to_db(var_type: &VariableType) -> &'static str {
    match var_type {
        VariableType::String => "string",
        VariableType::Number => "number",
        VariableType::Boolean => "boolean",
        VariableType::Json => "json",
        VariableType::Binary => "binary",
    }
}

/// Map a variable scope from the database to the domain enum.
pub fn map_variable_scope_from_db(
    scope_type: &str,
    scope_id: Option<Uuid>,
    environment_name: Option<&str>,
) -> Result<VariableScope> {
    match scope_type {
        "global" => Ok(VariableScope::Global),
        "workflow" => {
            let id = scope_id.ok_or_else(|| {
                PersistenceError::ConversionError("Workflow scope requires a scope ID".to_string())
            })?;
            Ok(VariableScope::Workflow(
                r8s_core::common::EntityId::from_bytes(id.as_bytes()),
            ))
        }
        "user" => {
            let id = scope_id.ok_or_else(|| {
                PersistenceError::ConversionError("User scope requires a scope ID".to_string())
            })?;
            Ok(VariableScope::User(r8s_core::common::EntityId::from_bytes(
                id.as_bytes(),
            )))
        }
        "environment" => {
            let env_name = environment_name.ok_or_else(|| {
                PersistenceError::ConversionError(
                    "Environment scope requires an environment name".to_string(),
                )
            })?;
            Ok(VariableScope::Environment(env_name.to_string()))
        }
        _ => Err(PersistenceError::ConversionError(format!(
            "Invalid variable scope: {}",
            scope_type
        ))),
    }
}

/// Map a domain enum variable scope to database fields.
pub fn map_variable_scope_to_db(scope: &VariableScope) -> (String, Option<Uuid>, Option<String>) {
    match scope {
        VariableScope::Global => ("global".to_string(), None, None),
        VariableScope::Workflow(id) => (
            "workflow".to_string(),
            Some(Uuid::from_bytes(id.as_bytes())),
            None,
        ),
        VariableScope::User(id) => (
            "user".to_string(),
            Some(Uuid::from_bytes(id.as_bytes())),
            None,
        ),
        VariableScope::Environment(name) => ("environment".to_string(), None, Some(name.clone())),
    }
}

impl VariableModel {
    /// Convert a domain Variable entity to a database model.
    pub fn from_entity(entity: &Variable) -> Result<Self> {
        let (scope_type, scope_id, environment_name) = map_variable_scope_to_db(&entity.scope);

        Ok(Self {
            id: Uuid::from_bytes(entity.id.as_bytes()),
            key: entity.key.clone(),
            value: entity.value.clone(),
            var_type: map_variable_type_to_db(&entity.var_type).to_string(),
            scope_type,
            scope_id,
            environment_name,
            protected: entity.protected,
            description: entity.description.clone(),
            created_at: entity.timestamps.created_at,
            updated_at: entity.timestamps.updated_at,
        })
    }

    /// Convert this database model to a domain Variable entity.
    pub fn to_entity(&self) -> Result<Variable> {
        let var_type = map_variable_type_from_db(&self.var_type)?;
        let scope = map_variable_scope_from_db(
            &self.scope_type,
            self.scope_id,
            self.environment_name.as_deref(),
        )?;

        let timestamps = Timestamp {
            created_at: self.created_at,
            updated_at: self.updated_at,
        };

        Ok(Variable {
            id: r8s_core::common::EntityId::from_bytes(self.id.as_bytes()),
            key: self.key.clone(),
            value: self.value.clone(),
            var_type,
            scope,
            protected: self.protected,
            description: self.description.clone(),
            timestamps,
        })
    }
}
