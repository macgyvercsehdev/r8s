// r8s-persistence/src/models/user.rs
//! Database model for users.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::uuid::Uuid;
use sqlx::types::JsonValue;

use r8s_core::common::Timestamp;
use r8s_core::entity::{User, UserRole};

use crate::error::{PersistenceError, Result};

/// Database model for users.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct UserModel {
    /// Primary key
    pub id: Uuid,

    /// Username for login
    pub username: String,

    /// Email address
    pub email: String,

    /// Full name
    pub full_name: String,

    /// Password hash
    pub password_hash: String,

    /// User role
    pub role: String,

    /// Whether the user is active
    pub active: bool,

    /// Last login timestamp
    pub last_login: Option<DateTime<Utc>>,

    /// User preferences as JSON
    pub preferences: JsonValue,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
}

/// Map a user role string from the database to the domain enum.
pub fn map_user_role_from_db(role: &str) -> Result<UserRole> {
    match role {
        "user" => Ok(UserRole::User),
        "power_user" => Ok(UserRole::PowerUser),
        "team_admin" => Ok(UserRole::TeamAdmin),
        "admin" => Ok(UserRole::Admin),
        _ => Err(PersistenceError::ConversionError(format!(
            "Invalid user role: {}",
            role
        ))),
    }
}

/// Map a domain enum user role to a database string.
pub fn map_user_role_to_db(role: &UserRole) -> &'static str {
    match role {
        UserRole::User => "user",
        UserRole::PowerUser => "power_user",
        UserRole::TeamAdmin => "team_admin",
        UserRole::Admin => "admin",
    }
}

impl UserModel {
    /// Convert a domain User entity to a database model.
    pub fn from_entity(entity: &User) -> Result<Self> {
        let preferences = serde_json::to_value(&entity.preferences)
            .map_err(|e| PersistenceError::SerializationError(e.to_string()))?;

        Ok(Self {
            id: Uuid::from_bytes(entity.id.as_bytes()),
            username: entity.username.clone(),
            email: entity.email.clone(),
            full_name: entity.full_name.clone(),
            password_hash: entity.password_hash.clone(),
            role: map_user_role_to_db(&entity.role).to_string(),
            active: entity.active,
            last_login: entity.last_login,
            preferences,
            created_at: entity.timestamps.created_at,
            updated_at: entity.timestamps.updated_at,
        })
    }

    /// Convert this database model to a domain User entity.
    pub fn to_entity(&self) -> Result<User> {
        let role = map_user_role_from_db(&self.role)?;

        let preferences = if self.preferences.is_object() {
            self.preferences.clone()
        } else {
            serde_json::Value::Object(serde_json::Map::new())
        };

        let timestamps = Timestamp {
            created_at: self.created_at,
            updated_at: self.updated_at,
        };

        Ok(User {
            id: r8s_core::common::EntityId::from_bytes(self.id.as_bytes()),
            username: self.username.clone(),
            email: self.email.clone(),
            full_name: self.full_name.clone(),
            password_hash: self.password_hash.clone(),
            role,
            active: self.active,
            last_login: self.last_login,
            preferences,
            timestamps,
        })
    }
}
