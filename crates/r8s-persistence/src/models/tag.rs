// r8s-persistence/src/models/tag.rs
//! Database model for tags.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::uuid::Uuid;

use r8s_core::common::Timestamp;
use r8s_core::entity::{Tag, TagColor};

use crate::error::{PersistenceError, Result};

/// Database model for tags.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TagModel {
    /// Primary key
    pub id: Uuid,

    /// Tag name
    pub name: String,

    /// Tag color
    pub color: String,

    /// Tag description
    pub description: Option<String>,

    /// User who created the tag
    pub created_by: Option<Uuid>,

    /// Whether this is a system tag
    pub system_tag: bool,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
}

/// Database model for workflow tags (junction table).
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct WorkflowTagModel {
    /// Workflow ID
    pub workflow_id: Uuid,

    /// Tag ID
    pub tag_id: Uuid,
}

/// Map a tag color string from the database to the domain enum.
pub fn map_tag_color_from_db(color: &str) -> Result<TagColor> {
    match color {
        "red" => Ok(TagColor::Red),
        "green" => Ok(TagColor::Green),
        "blue" => Ok(TagColor::Blue),
        "yellow" => Ok(TagColor::Yellow),
        "orange" => Ok(TagColor::Orange),
        "purple" => Ok(TagColor::Purple),
        "gray" => Ok(TagColor::Gray),
        "teal" => Ok(TagColor::Teal),
        "pink" => Ok(TagColor::Pink),
        "brown" => Ok(TagColor::Brown),
        _ => Err(PersistenceError::ConversionError(format!(
            "Invalid tag color: {}",
            color
        ))),
    }
}

/// Map a domain enum tag color to a database string.
pub fn map_tag_color_to_db(color: &TagColor) -> &'static str {
    match color {
        TagColor::Red => "red",
        TagColor::Green => "green",
        TagColor::Blue => "blue",
        TagColor::Yellow => "yellow",
        TagColor::Orange => "orange",
        TagColor::Purple => "purple",
        TagColor::Gray => "gray",
        TagColor::Teal => "teal",
        TagColor::Pink => "pink",
        TagColor::Brown => "brown",
    }
}

impl TagModel {
    /// Convert a domain Tag entity to a database model.
    pub fn from_entity(entity: &Tag) -> Result<Self> {
        Ok(Self {
            id: Uuid::from_bytes(entity.id.as_bytes()),
            name: entity.name.clone(),
            color: map_tag_color_to_db(&entity.color).to_string(),
            description: entity.description.clone(),
            created_by: entity.created_by.map(|id| Uuid::from_bytes(id.as_bytes())),
            system_tag: entity.system_tag,
            created_at: entity.timestamps.created_at,
            updated_at: entity.timestamps.updated_at,
        })
    }

    /// Convert this database model to a domain Tag entity.
    pub fn to_entity(&self) -> Result<Tag> {
        let color = map_tag_color_from_db(&self.color)?;

        let timestamps = Timestamp {
            created_at: self.created_at,
            updated_at: self.updated_at,
        };

        Ok(Tag {
            id: r8s_core::common::EntityId::from_bytes(self.id.as_bytes()),
            name: self.name.clone(),
            color,
            description: self.description.clone(),
            created_by: self
                .created_by
                .map(|id| r8s_core::common::EntityId::from_bytes(id.as_bytes())),
            system_tag: self.system_tag,
            timestamps,
        })
    }
}
