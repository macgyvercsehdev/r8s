// r8s-persistence/src/models/workflow.rs
//! Database model for workflows.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::uuid::Uuid;
use sqlx::types::JsonValue;

use r8s_core::common::Timestamp;
use r8s_core::entity::{Workflow, WorkflowSettings};

use crate::error::{PersistenceError, Result};
use crate::models::{ConnectionModel, NodeModel, TagModel};

/// Database model for workflows.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct WorkflowModel {
    /// Primary key
    pub id: Uuid,

    /// Name of the workflow
    pub name: String,

    /// Description (optional)
    pub description: Option<String>,

    /// Version number
    pub version: i32,

    /// Whether the workflow is active
    pub active: bool,

    /// Whether the workflow uses static data
    pub static_data: bool,

    /// Settings as JSON
    pub settings: JsonValue,

    /// Metadata as JSON
    pub metadata: Option<JsonValue>,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    pub updated_at: DateTime<Utc>,

    /// Creator's user ID
    pub created_by: Option<Uuid>,
}

impl WorkflowModel {
    /// Convert a domain Workflow entity to a database model.
    pub fn from_entity(entity: &Workflow) -> Result<Self> {
        let settings = serde_json::to_value(&entity.settings)
            .map_err(|e| PersistenceError::SerializationError(e.to_string()))?;

        let metadata = if entity.metadata.is_empty() {
            None
        } else {
            Some(
                serde_json::to_value(&entity.metadata)
                    .map_err(|e| PersistenceError::SerializationError(e.to_string()))?,
            )
        };

        Ok(Self {
            id: Uuid::from_bytes(entity.id.as_bytes()),
            name: entity.name.clone(),
            description: entity.description.clone(),
            version: entity.version as i32,
            active: entity.active,
            static_data: entity.static_data,
            settings,
            metadata,
            created_at: entity.timestamps.created_at,
            updated_at: entity.timestamps.updated_at,
            created_by: entity.created_by.map(|id| Uuid::from_bytes(id.as_bytes())),
        })
    }

    /// Convert this database model to a domain Workflow entity.
    ///
    /// Note: This only converts the workflow itself, not its nodes or connections.
    /// Use the `to_entity_with_relations` method for a complete conversion.
    pub fn to_entity(&self) -> Result<Workflow> {
        let settings: WorkflowSettings = serde_json::from_value(self.settings.clone())
            .map_err(|e| PersistenceError::SerializationError(e.to_string()))?;

        let metadata = if let Some(meta) = &self.metadata {
            serde_json::from_value(meta.clone())
                .map_err(|e| PersistenceError::SerializationError(e.to_string()))?
        } else {
            std::collections::HashMap::new()
        };

        let timestamps = Timestamp {
            created_at: self.created_at,
            updated_at: self.updated_at,
        };

        let created_by = self
            .created_by
            .map(|id| r8s_core::common::EntityId::from_bytes(id.as_bytes()));

        let workflow = Workflow {
            id: r8s_core::common::EntityId::from_bytes(self.id.as_bytes()),
            name: self.name.clone(),
            description: self.description.clone(),
            version: self.version as u32,
            active: self.active,
            static_data: self.static_data,
            nodes: Vec::new(), // Will be populated by to_entity_with_relations
            connections: Vec::new(), // Will be populated by to_entity_with_relations
            settings,
            metadata,
            tags: Vec::new(), // Will be populated by to_entity_with_relations
            timestamps,
            created_by,
        };

        Ok(workflow)
    }

    /// Convert to a complete Workflow entity with nodes, connections, and tags.
    pub fn to_entity_with_relations(
        &self,
        nodes: Vec<NodeModel>,
        connections: Vec<ConnectionModel>,
        tags: Vec<TagModel>,
    ) -> Result<Workflow> {
        let mut workflow = self.to_entity()?;

        // Convert nodes
        workflow.nodes = nodes
            .into_iter()
            .map(|n| n.to_entity())
            .collect::<Result<Vec<_>>>()?;

        // Convert connections
        workflow.connections = connections
            .into_iter()
            .map(|c| c.to_entity())
            .collect::<Result<Vec<_>>>()?;

        // Convert tags
        workflow.tags = tags
            .into_iter()
            .map(|t| t.to_entity())
            .collect::<Result<Vec<_>>>()?;

        Ok(workflow)
    }
}
