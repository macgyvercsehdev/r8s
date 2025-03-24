// r8s-persistence/src/models/variable_model.rs
//! Database model for variable entities.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use r8s_core::common::EntityId;
use r8s_core::entity::{Variable, VariableScope, VariableType};
use r8s_core::error::Error as CoreError;

use crate::error::{PersistenceError, Result};

/// Database model for the variable entity.
#[derive(Debug, sqlx::FromRow)]
pub struct VariableModel {
    pub id: Uuid,
    pub key: String,
    pub value: String,
    pub var_type: String,
    pub scope_type: String,
    pub scope_id: Option<Uuid>,
    pub scope_id_text: Option<String>,
    pub protected: bool,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl VariableModel {
    /// Convert the database model to a domain entity.
    pub fn to_entity(&self) -> Result<Variable> {
        let id = EntityId::from_bytes(self.id.into_bytes());

        // Parse the variable type
        let var_type = match self.var_type.as_str() {
            "string" => VariableType::String,
            "number" => VariableType::Number,
            "boolean" => VariableType::Boolean,
            "json" => VariableType::Json,
            "binary" => VariableType::Binary,
            _ => {
                return Err(PersistenceError::DataConversionError(format!(
                    "Invalid variable type: {}",
                    self.var_type
                )))
            }
        };

        // Parse the scope
        let scope = match self.scope_type.as_str() {
            "global" => VariableScope::Global,

            "workflow" => {
                if let Some(scope_id) = self.scope_id {
                    VariableScope::Workflow(EntityId::from_bytes(scope_id.into_bytes()))
                } else {
                    return Err(PersistenceError::DataConversionError(
                        "Workflow scope requires a scope_id".to_string(),
                    ));
                }
            }

            "user" => {
                if let Some(scope_id) = self.scope_id {
                    VariableScope::User(EntityId::from_bytes(scope_id.into_bytes()))
                } else {
                    return Err(PersistenceError::DataConversionError(
                        "User scope requires a scope_id".to_string(),
                    ));
                }
            }

            "environment" => {
                if let Some(env) = &self.scope_id_text {
                    VariableScope::Environment(env.clone())
                } else {
                    return Err(PersistenceError::DataConversionError(
                        "Environment scope requires a scope_id_text".to_string(),
                    ));
                }
            }

            _ => {
                return Err(PersistenceError::DataConversionError(format!(
                    "Invalid variable scope: {}",
                    self.scope_type
                )))
            }
        };

        let variable = Variable {
            id,
            key: self.key.clone(),
            value: self.value.clone(),
            var_type,
            scope,
            protected: self.protected,
            description: self.description.clone(),
            timestamps: r8s_core::common::Timestamp {
                created_at: self.created_at,
                updated_at: self.updated_at,
            },
        };

        Ok(variable)
    }

    /// Convert a domain entity to a database model.
    pub fn from_entity(entity: &Variable) -> Result<Self> {
        let id = Uuid::from_bytes(entity.id.as_bytes());

        // Convert the variable type to a string
        let var_type = match entity.var_type {
            VariableType::String => "string",
            VariableType::Number => "number",
            VariableType::Boolean => "boolean",
            VariableType::Json => "json",
            VariableType::Binary => "binary",
        }
        .to_string();

        // Convert the scope
        let (scope_type, scope_id, scope_id_text) = match &entity.scope {
            VariableScope::Global => ("global".to_string(), None, None),

            VariableScope::Workflow(workflow_id) => (
                "workflow".to_string(),
                Some(Uuid::from_bytes(workflow_id.as_bytes())),
                None,
            ),

            VariableScope::User(user_id) => (
                "user".to_string(),
                Some(Uuid::from_bytes(user_id.as_bytes())),
                None,
            ),

            VariableScope::Environment(env) => ("environment".to_string(), None, Some(env.clone())),
        };

        Ok(Self {
            id,
            key: entity.key.clone(),
            value: entity.value.clone(),
            var_type,
            scope_type,
            scope_id,
            scope_id_text,
            protected: entity.protected,
            description: entity.description.clone(),
            created_at: entity.timestamps.created_at,
            updated_at: entity.timestamps.updated_at,
        })
    }
}
