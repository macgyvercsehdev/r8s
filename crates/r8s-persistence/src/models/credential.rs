// r8s-persistence/src/models/credential.rs
//! Database model for credentials.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::uuid::Uuid;
use sqlx::types::JsonValue;

use r8s_core::common::Timestamp;
use r8s_core::entity::Credential;

use crate::error::{PersistenceError, Result};

/// Database model for credentials.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CredentialModel {
    /// Primary key
    pub id: Uuid,

    /// Name of the credential
    pub name: String,

    /// Type of credential
    pub type_name: String,

    /// Encrypted credential data
    pub data: JsonValue,

    /// Owner user ID
    pub owner_id: Option<Uuid>,

    /// Whether the credential is shared
    pub shared: bool,

    /// Notes about the credential
    pub notes: Option<String>,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
}

/// Database model for node credentials (junction table).
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct NodeCredentialModel {
    /// Node ID
    pub node_id: Uuid,

    /// Credential ID
    pub credential_id: Uuid,
}

impl CredentialModel {
    /// Convert a domain Credential entity to a database model.
    pub fn from_entity(entity: &Credential) -> Result<Self> {
        Ok(Self {
            id: Uuid::from_bytes(entity.id.as_bytes()),
            name: entity.name.clone(),
            type_name: entity.type_name.clone(),
            data: entity.data.clone(),
            owner_id: entity.owner_id.map(|id| Uuid::from_bytes(id.as_bytes())),
            shared: entity.shared,
            notes: entity.notes.clone(),
            created_at: entity.timestamps.created_at,
            updated_at: entity.timestamps.updated_at,
        })
    }

    /// Convert this database model to a domain Credential entity.
    pub fn to_entity(&self) -> Result<Credential> {
        let timestamps = Timestamp {
            created_at: self.created_at,
            updated_at: self.updated_at,
        };

        Ok(Credential {
            id: r8s_core::common::EntityId::from_bytes(self.id.as_bytes()),
            name: self.name.clone(),
            type_name: self.type_name.clone(),
            data: self.data.clone(),
            owner_id: self
                .owner_id
                .map(|id| r8s_core::common::EntityId::from_bytes(id.as_bytes())),
            shared: self.shared,
            notes: self.notes.clone(),
            timestamps,
        })
    }
}
