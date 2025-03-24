// r8s-persistence/src/models/node.rs
//! Database model for workflow nodes.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::uuid::Uuid;
use sqlx::types::JsonValue;

use r8s_core::common::Timestamp;
use r8s_core::entity::{
    DataType, Node, NodePort, NodeSettings, NodeType, NodeTypeCategory, Position,
};

use crate::error::{PersistenceError, Result};

/// Database model for workflow nodes.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct NodeModel {
    /// Primary key
    pub id: Uuid,

    /// Foreign key to workflow
    pub workflow_id: Uuid,

    /// Node name
    pub name: String,

    /// Node type name
    pub type_name: String,

    /// Node type version
    pub type_version: String,

    /// Node type category
    pub type_category: String,

    /// ID of the plugin providing this node type (if any)
    pub plugin_id: Option<Uuid>,

    /// X position in the editor
    pub position_x: f32,

    /// Y position in the editor
    pub position_y: f32,

    /// Node parameters as JSON
    pub parameters: JsonValue,

    /// Whether the node is disabled
    pub disabled: bool,

    /// Node settings as JSON
    pub settings: JsonValue,

    /// Node metadata as JSON
    pub metadata: Option<JsonValue>,

    /// Node notes
    pub notes: Option<String>,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
}

/// Database model for node ports (inputs/outputs).
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct NodePortModel {
    /// Primary key
    pub id: Uuid,

    /// Foreign key to node
    pub node_id: Uuid,

    /// Port name
    pub name: String,

    /// Whether this is an input port (true) or output port (false)
    pub is_input: bool,

    /// Data type
    pub data_type: String,

    /// Description
    pub description: Option<String>,

    /// Whether the port is required
    pub required: bool,

    /// Example values as JSON
    pub examples: Option<JsonValue>,

    /// JSON schema for validation
    pub schema: Option<JsonValue>,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
}

/// Map a type category string from the database to the domain enum.
pub fn map_type_category_from_db(category: &str) -> Result<NodeTypeCategory> {
    match category {
        "trigger" => Ok(NodeTypeCategory::Trigger),
        "core" => Ok(NodeTypeCategory::Core),
        "integration" => Ok(NodeTypeCategory::Integration),
        "transformation" => Ok(NodeTypeCategory::Transformation),
        "flow" => Ok(NodeTypeCategory::Flow),
        "generator" => Ok(NodeTypeCategory::Generator),
        "ai" => Ok(NodeTypeCategory::AI),
        "io" => Ok(NodeTypeCategory::IO),
        "code" => Ok(NodeTypeCategory::Code),
        "utility" => Ok(NodeTypeCategory::Utility),
        _ => Err(PersistenceError::ConversionError(format!(
            "Invalid node type category: {}",
            category
        ))),
    }
}

/// Map a domain enum type category to a database string.
pub fn map_type_category_to_db(category: &NodeTypeCategory) -> &'static str {
    match category {
        NodeTypeCategory::Trigger => "trigger",
        NodeTypeCategory::Core => "core",
        NodeTypeCategory::Integration => "integration",
        NodeTypeCategory::Transformation => "transformation",
        NodeTypeCategory::Flow => "flow",
        NodeTypeCategory::Generator => "generator",
        NodeTypeCategory::AI => "ai",
        NodeTypeCategory::IO => "io",
        NodeTypeCategory::Code => "code",
        NodeTypeCategory::Utility => "utility",
    }
}

/// Map a data type string from the database to the domain enum.
pub fn map_data_type_from_db(data_type: &str) -> Result<DataType> {
    if data_type.starts_with("custom:") {
        let custom_type = data_type.trim_start_matches("custom:").to_string();
        return Ok(DataType::Custom(custom_type));
    }

    match data_type {
        "string" => Ok(DataType::String),
        "number" => Ok(DataType::Number),
        "boolean" => Ok(DataType::Boolean),
        "datetime" => Ok(DataType::DateTime),
        "array" => Ok(DataType::Array),
        "object" => Ok(DataType::Object),
        "binary" => Ok(DataType::Binary),
        "any" => Ok(DataType::Any),
        _ => Err(PersistenceError::ConversionError(format!(
            "Invalid data type: {}",
            data_type
        ))),
    }
}

/// Map a domain enum data type to a database string.
pub fn map_data_type_to_db(data_type: &DataType) -> String {
    match data_type {
        DataType::String => "string".to_string(),
        DataType::Number => "number".to_string(),
        DataType::Boolean => "boolean".to_string(),
        DataType::DateTime => "datetime".to_string(),
        DataType::Array => "array".to_string(),
        DataType::Object => "object".to_string(),
        DataType::Binary => "binary".to_string(),
        DataType::Custom(custom_type) => format!("custom:{}", custom_type),
        DataType::Any => "any".to_string(),
    }
}

impl NodeModel {
    /// Convert a domain Node entity to a database model.
    pub fn from_entity(entity: &Node, workflow_id: Uuid) -> Result<(Self, Vec<NodePortModel>)> {
        let parameters = serde_json::to_value(&entity.parameters)
            .map_err(|e| PersistenceError::SerializationError(e.to_string()))?;

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

        let node_model = Self {
            id: Uuid::from_bytes(entity.id.as_bytes()),
            workflow_id,
            name: entity.name.clone(),
            type_name: entity.type_info.name.clone(),
            type_version: entity.type_info.version.clone(),
            type_category: map_type_category_to_db(&entity.type_info.category).to_string(),
            plugin_id: entity
                .type_info
                .plugin_id
                .map(|id| Uuid::from_bytes(id.as_bytes())),
            position_x: entity.position.x,
            position_y: entity.position.y,
            parameters,
            disabled: entity.disabled,
            settings,
            metadata,
            notes: entity.notes.clone(),
            created_at: entity.timestamps.created_at,
            updated_at: entity.timestamps.updated_at,
        };

        // Convert input ports
        let mut port_models = Vec::new();
        for input in &entity.inputs {
            let examples = if input.examples.is_empty() {
                None
            } else {
                Some(
                    serde_json::to_value(&input.examples)
                        .map_err(|e| PersistenceError::SerializationError(e.to_string()))?,
                )
            };

            port_models.push(NodePortModel {
                id: Uuid::new_v4(),
                node_id: node_model.id,
                name: input.name.clone(),
                is_input: true,
                data_type: map_data_type_to_db(&input.data_type),
                description: input.description.clone(),
                required: input.required,
                examples,
                schema: input.schema.clone(),
                created_at: entity.timestamps.created_at,
                updated_at: entity.timestamps.updated_at,
            });
        }

        // Convert output ports
        for output in &entity.outputs {
            let examples = if output.examples.is_empty() {
                None
            } else {
                Some(
                    serde_json::to_value(&output.examples)
                        .map_err(|e| PersistenceError::SerializationError(e.to_string()))?,
                )
            };

            port_models.push(NodePortModel {
                id: Uuid::new_v4(),
                node_id: node_model.id,
                name: output.name.clone(),
                is_input: false,
                data_type: map_data_type_to_db(&output.data_type),
                description: output.description.clone(),
                required: output.required,
                examples,
                schema: output.schema.clone(),
                created_at: entity.timestamps.created_at,
                updated_at: entity.timestamps.updated_at,
            });
        }

        Ok((node_model, port_models))
    }

    /// Convert this database model to a domain Node entity.
    pub fn to_entity(&self) -> Result<Node> {
        let parameters = self.parameters.clone();

        let settings: NodeSettings = serde_json::from_value(self.settings.clone())
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

        let node_type = NodeType {
            name: self.type_name.clone(),
            version: self.type_version.clone(),
            category: map_type_category_from_db(&self.type_category)?,
            plugin_id: self
                .plugin_id
                .map(|id| r8s_core::common::EntityId::from_bytes(id.as_bytes())),
            icon: None,
            color: None,
            supports_custom_code: false,
        };

        let position = Position {
            x: self.position_x,
            y: self.position_y,
        };

        let node = Node {
            id: r8s_core::common::EntityId::from_bytes(self.id.as_bytes()),
            name: self.name.clone(),
            type_info: node_type,
            position,
            parameters,
            inputs: Vec::new(),      // Will be populated by to_entity_with_ports
            outputs: Vec::new(),     // Will be populated by to_entity_with_ports
            credentials: Vec::new(), // Will be populated separately
            notes: self.notes.clone(),
            disabled: self.disabled,
            settings,
            metadata,
            timestamps,
        };

        Ok(node)
    }

    /// Convert to a complete Node entity with ports.
    pub fn to_entity_with_ports(
        &self,
        ports: Vec<NodePortModel>,
        credential_ids: Vec<Uuid>,
    ) -> Result<Node> {
        let mut node = self.to_entity()?;

        // Process ports
        for port in ports {
            if port.node_id != self.id {
                continue;
            }

            let data_type = map_data_type_from_db(&port.data_type)?;

            let examples = if let Some(ex) = port.examples {
                serde_json::from_value(ex)
                    .map_err(|e| PersistenceError::SerializationError(e.to_string()))?
            } else {
                Vec::new()
            };

            let node_port = NodePort {
                name: port.name,
                data_type,
                description: port.description,
                required: port.required,
                examples,
                schema: port.schema,
            };

            if port.is_input {
                node.inputs.push(node_port);
            } else {
                node.outputs.push(node_port);
            }
        }

        // Add credential IDs
        node.credentials = credential_ids
            .into_iter()
            .map(|id| r8s_core::common::EntityId::from_bytes(id.as_bytes()))
            .collect();

        Ok(node)
    }
}
