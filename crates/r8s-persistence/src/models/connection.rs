// r8s-persistence/src/models/connection.rs
//! Database model for workflow connections.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::uuid::Uuid;
use sqlx::types::JsonValue;

use r8s_core::entity::{ConditionType, Connection, ConnectionCondition};

use crate::error::{PersistenceError, Result};

/// Database model for workflow connections.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ConnectionModel {
    /// Primary key
    pub id: Uuid,

    /// Foreign key to workflow
    pub workflow_id: Uuid,

    /// Source node ID
    pub source_node: Uuid,

    /// Source node output port name
    pub source_output: String,

    /// Target node ID
    pub target_node: Uuid,

    /// Target node input port name
    pub target_input: String,

    /// Condition type (if any)
    pub condition_type: Option<String>,

    /// Condition expression
    pub condition_expression: Option<String>,

    /// Transform expression (if any)
    pub transform: Option<String>,

    /// Connection metadata as JSON
    pub metadata: Option<JsonValue>,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
}

/// Map a condition type string from the database to the domain enum.
pub fn map_condition_type_from_db(condition_type: &str) -> Result<ConditionType> {
    match condition_type {
        "javascript" => Ok(ConditionType::JavaScript),
        "jsonpath" => Ok(ConditionType::JSONPath),
        "business_rule" => Ok(ConditionType::BusinessRule),
        _ => Err(PersistenceError::ConversionError(format!(
            "Invalid condition type: {}",
            condition_type
        ))),
    }
}

/// Map a domain enum condition type to a database string.
pub fn map_condition_type_to_db(condition_type: &ConditionType) -> &'static str {
    match condition_type {
        ConditionType::JavaScript => "javascript",
        ConditionType::JSONPath => "jsonpath",
        ConditionType::BusinessRule => "business_rule",
    }
}

impl ConnectionModel {
    /// Convert a domain Connection entity to a database model.
    pub fn from_entity(entity: &Connection, workflow_id: Uuid) -> Result<Self> {
        let (condition_type, condition_expression) = if let Some(condition) = &entity.condition {
            (
                Some(map_condition_type_to_db(&condition.condition_type).to_string()),
                Some(condition.expression.clone()),
            )
        } else {
            (None, None)
        };

        let metadata =
            if entity.metadata.is_object() && !entity.metadata.as_object().unwrap().is_empty() {
                Some(entity.metadata.clone())
            } else {
                None
            };

        Ok(Self {
            id: Uuid::from_bytes(entity.id.as_bytes()),
            workflow_id,
            source_node: Uuid::from_bytes(entity.source_node.as_bytes()),
            source_output: entity.source_output.clone(),
            target_node: Uuid::from_bytes(entity.target_node.as_bytes()),
            target_input: entity.target_input.clone(),
            condition_type,
            condition_expression,
            transform: entity.transform.clone(),
            metadata,
            created_at: chrono::Utc::now(), // Not stored in domain entity
            updated_at: chrono::Utc::now(), // Not stored in domain entity
        })
    }

    /// Convert this database model to a domain Connection entity.
    pub fn to_entity(&self) -> Result<Connection> {
        let mut connection = Connection {
            id: r8s_core::common::EntityId::from_bytes(self.id.as_bytes()),
            source_node: r8s_core::common::EntityId::from_bytes(self.source_node.as_bytes()),
            source_output: self.source_output.clone(),
            target_node: r8s_core::common::EntityId::from_bytes(self.target_node.as_bytes()),
            target_input: self.target_input.clone(),
            condition: None, // Will be set below if present
            transform: self.transform.clone(),
            metadata: if let Some(meta) = &self.metadata {
                meta.clone()
            } else {
                serde_json::Value::Object(serde_json::Map::new())
            },
        };

        // Set condition if present
        if let (Some(condition_type), Some(condition_expression)) =
            (&self.condition_type, &self.condition_expression)
        {
            let cond_type = map_condition_type_from_db(condition_type)?;

            connection.condition = Some(ConnectionCondition {
                condition_type: cond_type,
                expression: condition_expression.clone(),
            });
        }

        Ok(connection)
    }
}
