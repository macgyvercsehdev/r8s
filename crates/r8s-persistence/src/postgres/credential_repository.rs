// r8s-persistence/src/postgres/credential_repository.rs
//! PostgreSQL implementation of the CredentialRepository interface.

use std::sync::Arc;
use async_trait::async_trait;
use sqlx::Row;
use uuid::Uuid;

use r8s_core::common::{EntityId, Result as CoreResult, AsyncResult};
use r8s_core::entity::Credential;
use r8s_core::repository::{Repository, CredentialRepository};
use r8s_core::error::Error as CoreError;

use crate::connection::ConnectionManager;
use crate::error::{PersistenceError, Result};
use crate::models::CredentialModel;

/// PostgreSQL implementation of the CredentialRepository.
pub struct PostgresCredentialRepository {
    /// Database connection manager
    connection_manager: Arc<ConnectionManager>,
}

impl PostgresCredentialRepository {
    /// Create a new PostgreSQL credential repository.
    pub fn new(connection_manager: Arc<ConnectionManager>) -> Self {
        Self {
            connection_manager,
        }
    }
}

#[async_trait]
impl Repository<Credential> for PostgresCredentialRepository {
    fn find_by_id(&self, id: EntityId) -> AsyncResult<Credential> {
        let connection_manager = self.connection_manager.clone();
        let uuid = Uuid::from_bytes(id.as_bytes());
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            let credential = sqlx::query_as::<_, CredentialModel>(
                "SELECT * FROM credentials WHERE id = $1",
            )
            .bind(uuid)
            .fetch_optional(&conn)
            .await
            .map_err(|e| CoreError::from(PersistenceError::from(e)))?
            .ok_or_else(|| CoreError::NotFound(format!("Credential with id {} not found", id)))?;
                
            // Convert to entity
            let credential_entity = credential.to_entity()
                .map_err(|e| CoreError::from(e))?;
                
            Ok(credential_entity)
        })
    }
    
    fn save(&self, entity: Credential) -> AsyncResult<Credential> {
        let connection_manager = self.connection_manager.clone();
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            // Convert the entity to a model
            let credential_model = CredentialModel::from_entity(&entity)
                .map_err(|e| CoreError::from(e))?;
                
            // Check if the credential exists
            let exists = sqlx::query("SELECT 1 FROM credentials WHERE id = $1")
                .bind(credential_model.id)
                .fetch_optional(&conn)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?
                .is_some();
                
            // Insert or update the credential
            if exists {
                sqlx::query(
                    "UPDATE credentials 
                     SET name = $1, type_name = $2, data = $3, owner_id = $4,
                     shared = $5, notes = $6, updated_at = $7
                     WHERE id = $8"
                )
                .bind(&credential_model.name)
                .bind(&credential_model.type_name)
                .bind(&credential_model.data)
                .bind(credential_model.owner_id)
                .bind(credential_model.shared)
                .bind(&credential_model.notes)
                .bind(credential_model.updated_at)
                .bind(credential_model.id)
                .execute(&conn)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
            } else {
                sqlx::query(
                    "INSERT INTO credentials 
                     (id, name, type_name, data, owner_id, shared, notes, created_at, updated_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"
                )
                .bind(credential_model.id)
                .bind(&credential_model.name)
                .bind(&credential_model.type_name)
                .bind(&credential_model.data)
                .bind(credential_model.owner_id)
                .bind(credential_model.shared)
                .bind(&credential_model.notes)
                .bind(credential_model.created_at)
                .bind(credential_model.updated_at)
                .execute(&conn)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
            }
            
            // Return the saved credential
            PostgresCredentialRepository::new(connection_manager)
                .find_by_id(entity.id)
                .await
        })
    }
    
    fn delete(&self, id: EntityId) -> AsyncResult<()> {
        let connection_manager = self.connection_manager.clone();
        let uuid = Uuid::from_bytes(id.as_bytes());
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            // Start a transaction
            let mut tx = conn.begin().await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
                
            // Check if the credential exists
            let exists = sqlx::query("SELECT 1 FROM credentials WHERE id = $1")
                .bind(uuid)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?
                .is_some();
                
            if !exists {
                return Err(CoreError::NotFound(format!("Credential with id {} not found", id)));
            }
            
            // Delete node credentials using this credential
            sqlx::query("DELETE FROM node_credentials WHERE credential_id = $1")
                .bind(uuid)
                .execute(&mut *tx)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
                
            // Delete the credential
            sqlx::query("DELETE FROM credentials WHERE id = $1")
                .bind(uuid)
                .execute(&mut *tx)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
                
            // Commit the transaction
            tx.commit()
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
                
            Ok(())
        })
    }
    
    fn exists(&self, id: EntityId) -> AsyncResult<bool> {
        let connection_manager = self.connection_manager.clone();
        let uuid = Uuid::from_bytes(id.as_bytes());
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            let exists = sqlx::query("SELECT 1 FROM credentials WHERE id = $1")
                .bind(uuid)
                .fetch_optional(&conn)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?
                .is_some();
                
            Ok(exists)
        })
    }
}

#[async_trait]
impl CredentialRepository for PostgresCredentialRepository {
    fn find_by_owner(&self, owner_id: EntityId) -> AsyncResult<Vec<Credential>> {
        let connection_manager = self.connection_manager.clone();
        let uuid = Uuid::from_bytes(owner_id.as_bytes());
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            let credentials = sqlx::query_as::<_, CredentialModel>(
                "SELECT * FROM credentials
                 WHERE owner_id = $1
                 ORDER BY name ASC"
            )
            .bind(uuid)
            .fetch_all(&conn)
            .await
            .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
            
            // Convert to entities
            let credential_entities = credentials.into_iter()
                .map(|model| model.to_entity())
                .collect::<Result<Vec<_>>>()
                .map_err(|e| CoreError::from(e))?;
                
            Ok(credential_entities)
        })
    }
    
    fn find_shared_credentials(&self) -> AsyncResult<Vec<Credential>> {
        let connection_manager = self.connection_manager.clone();
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            let credentials = sqlx::query_as::<_, CredentialModel>(
                "SELECT * FROM credentials
                 WHERE shared = true
                 ORDER BY name ASC"
            )
            .fetch_all(&conn)
            .await
            .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
            
            // Convert to entities
            let credential_entities = credentials.into_iter()
                .map(|model| model.to_entity())
                .collect::<Result<Vec<_>>>()
                .map_err(|e| CoreError::from(e))?;
                
            Ok(credential_entities)
        })
    }
    
    fn find_by_type(&self, type_name: &str) -> AsyncResult<Vec<Credential>> {
        let connection_manager = self.connection_manager.clone();
        let type_name = type_name.to_string();
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            let credentials = sqlx::query_as::<_, CredentialModel>(
                "SELECT * FROM credentials
                 WHERE type_name = $1
                 ORDER BY name ASC"
            )
            .bind(&type_name)
            .fetch_all(&conn)
            .await
            .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
            
            // Convert to entities
            let credential_entities = credentials.into_iter()
                .map(|model| model.to_entity())
                .collect::<Result<Vec<_>>>()
                .map_err(|e| CoreError::from(e))?;
                
            Ok(credential_entities)
        })
    }
    
    fn has_access(&self, credential_id: EntityId, user_id: EntityId) -> AsyncResult<bool> {
        let connection_manager = self.connection_manager.clone();
        let credential_uuid = Uuid::from_bytes(credential_id.as_bytes());
        let user_uuid = Uuid::from_bytes(user_id.as_bytes());
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            let access = sqlx::query(
                "SELECT 1 FROM credentials
                 WHERE id = $1 AND (shared = true OR owner_id = $2)"
            )
            .bind(credential_uuid)
            .bind(user_uuid)
            .fetch_optional(&conn)
            .await
            .map_err(|e| CoreError::from(PersistenceError::from(e)))?
            .is_some();
            
            Ok(access)
        })
    }
    
    fn find_by_node(&self, node_id: EntityId) -> AsyncResult<Vec<Credential>> {
        let connection_manager = self.connection_manager.clone();
        let node_uuid = Uuid::from_bytes(node_id.as_bytes());
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            let credentials = sqlx::query_as::<_, CredentialModel>(
                "SELECT c.* FROM credentials c
                 JOIN node_credentials nc ON c.id = nc.credential_id
                 WHERE nc.node_id = $1
                 ORDER BY c.name ASC"
            )
            .bind(node_uuid)
            .fetch_all(&conn)
            .await
            .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
            
            // Convert to entities
            let credential_entities = credentials.into_iter()
                .map(|model| model.to_entity())
                .collect::<Result<Vec<_>>>()
                .map_err(|e| CoreError::from(e))?;
                
            Ok(credential_entities)
        })
    }
    
    fn update_data(&self, credential_id: EntityId, encrypted_data: serde_json::Value) -> AsyncResult<()> {
        let connection_manager = self.connection_manager.clone();
        let credential_uuid = Uuid::from_bytes(credential_id.as_bytes());
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            sqlx::query(
                "UPDATE credentials 
                 SET data = $1, updated_at = $2
                 WHERE id = $3"
            )
            .bind(&encrypted_data)
            .bind(chrono::Utc::now())
            .bind(credential_uuid)
            .execute(&conn)
            .await
            .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
            
            Ok(())
        })
    }
    
    fn share_credential(&self, credential_id: EntityId) -> AsyncResult<()> {
        let connection_manager = self.connection_manager.clone();
        let credential_uuid = Uuid::from_bytes(credential_id.as_bytes());
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            sqlx::query(
                "UPDATE credentials 
                 SET shared = true, updated_at = $1
                 WHERE id = $2"
            )
            .bind(chrono::Utc::now())
            .bind(credential_uuid)
            .execute(&conn)
            .await
            .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
            
            Ok(())
        })
    }
    
    fn unshare_credential(&self, credential_id: EntityId) -> AsyncResult<()> {
        let connection_manager = self.connection_manager.clone();
        let credential_uuid = Uuid::from_bytes(credential_id.as_bytes());
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            sqlx::query(
                "UPDATE credentials 
                 SET shared = false, updated_at = $1
                 WHERE id = $2"
            )
            .bind(chrono::Utc::now())
            .bind(credential_uuid)
            .execute(&conn)
            .await
            .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
            
            Ok(())
        })
    }
}