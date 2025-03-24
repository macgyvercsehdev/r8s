// r8s-persistence/src/postgres/user_repository.rs
//! PostgreSQL implementation of the UserRepository interface.

use std::sync::Arc;
use async_trait::async_trait;
use chrono::Utc;
use sqlx::Row;
use uuid::Uuid;

use r8s_core::common::{EntityId, Result as CoreResult, AsyncResult};
use r8s_core::entity::{User, UserRole};
use r8s_core::repository::{Repository, UserRepository};
use r8s_core::error::Error as CoreError;

use crate::connection::ConnectionManager;
use crate::error::{PersistenceError, Result};
use crate::models::UserModel;

/// PostgreSQL implementation of the UserRepository.
pub struct PostgresUserRepository {
    /// Database connection manager
    connection_manager: Arc<ConnectionManager>,
}

impl PostgresUserRepository {
    /// Create a new PostgreSQL user repository.
    pub fn new(connection_manager: Arc<ConnectionManager>) -> Self {
        Self {
            connection_manager,
        }
    }
}

#[async_trait]
impl Repository<User> for PostgresUserRepository {
    fn find_by_id(&self, id: EntityId) -> AsyncResult<User> {
        let connection_manager = self.connection_manager.clone();
        let uuid = Uuid::from_bytes(id.as_bytes());
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            let user = sqlx::query_as::<_, UserModel>(
                "SELECT * FROM users WHERE id = $1",
            )
            .bind(uuid)
            .fetch_optional(&conn)
            .await
            .map_err(|e| CoreError::from(PersistenceError::from(e)))?
            .ok_or_else(|| CoreError::NotFound(format!("User with id {} not found", id)))?;
                
            // Convert to entity
            let user_entity = user.to_entity()
                .map_err(|e| CoreError::from(e))?;
                
            Ok(user_entity)
        })
    }
    
    fn save(&self, entity: User) -> AsyncResult<User> {
        let connection_manager = self.connection_manager.clone();
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            // Convert the entity to a model
            let user_model = UserModel::from_entity(&entity)
                .map_err(|e| CoreError::from(e))?;
                
            // Check if the user exists
            let exists = sqlx::query("SELECT 1 FROM users WHERE id = $1")
                .bind(user_model.id)
                .fetch_optional(&conn)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?
                .is_some();
                
            // Insert or update the user
            if exists {
                sqlx::query(
                    "UPDATE users 
                     SET username = $1, email = $2, full_name = $3, 
                     password_hash = $4, role = $5, active = $6, 
                     last_login = $7, preferences = $8, updated_at = $9
                     WHERE id = $10"
                )
                .bind(&user_model.username)
                .bind(&user_model.email)
                .bind(&user_model.full_name)
                .bind(&user_model.password_hash)
                .bind(&user_model.role)
                .bind(user_model.active)
                .bind(user_model.last_login)
                .bind(&user_model.preferences)
                .bind(user_model.updated_at)
                .bind(user_model.id)
                .execute(&conn)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
            } else {
                sqlx::query(
                    "INSERT INTO users 
                     (id, username, email, full_name, password_hash, role, active, 
                     last_login, preferences, created_at, updated_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)"
                )
                .bind(user_model.id)
                .bind(&user_model.username)
                .bind(&user_model.email)
                .bind(&user_model.full_name)
                .bind(&user_model.password_hash)
                .bind(&user_model.role)
                .bind(user_model.active)
                .bind(user_model.last_login)
                .bind(&user_model.preferences)
                .bind(user_model.created_at)
                .bind(user_model.updated_at)
                .execute(&conn)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
            }
            
            // Return the saved user
            PostgresUserRepository::new(connection_manager)
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
                
            // Check if the user exists
            let exists = sqlx::query("SELECT 1 FROM users WHERE id = $1")
                .bind(uuid)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?
                .is_some();
                
            if !exists {
                return Err(CoreError::NotFound(format!("User with id {} not found", id)));
            }
            
            // Delete user data (in proper order to respect foreign key constraints)
            
            // 1. Delete user variables
            sqlx::query("DELETE FROM variables WHERE scope_type = 'user' AND scope_id = $1")
                .bind(uuid)
                .execute(&mut *tx)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
                
            // 2. Delete user credentials
            sqlx::query("DELETE FROM credentials WHERE owner_id = $1")
                .bind(uuid)
                .execute(&mut *tx)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
                
            // 3. Delete workflow collaborators
            sqlx::query("DELETE FROM workflow_collaborators WHERE user_id = $1")
                .bind(uuid)
                .execute(&mut *tx)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
                
            // 4. Delete the user
            sqlx::query("DELETE FROM users WHERE id = $1")
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
                
            let exists = sqlx::query("SELECT 1 FROM users WHERE id = $1")
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
impl UserRepository for PostgresUserRepository {
    fn find_by_username(&self, username: &str) -> AsyncResult<Option<User>> {
        let connection_manager = self.connection_manager.clone();
        let username = username.to_string();
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            let user = sqlx::query_as::<_, UserModel>(
                "SELECT * FROM users WHERE username = $1",
            )
            .bind(&username)
            .fetch_optional(&conn)
            .await
            .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
            
            if let Some(user_model) = user {
                let user_entity = user_model.to_entity()
                    .map_err(|e| CoreError::from(e))?;
                    
                Ok(Some(user_entity))
            } else {
                Ok(None)
            }
        })
    }
    
    fn find_by_email(&self, email: &str) -> AsyncResult<Option<User>> {
        let connection_manager = self.connection_manager.clone();
        let email = email.to_string();
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            let user = sqlx::query_as::<_, UserModel>(
                "SELECT * FROM users WHERE email = $1",
            )
            .bind(&email)
            .fetch_optional(&conn)
            .await
            .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
            
            if let Some(user_model) = user {
                let user_entity = user_model.to_entity()
                    .map_err(|e| CoreError::from(e))?;
                    
                Ok(Some(user_entity))
            } else {
                Ok(None)
            }
        })
    }
    
    fn find_active_users(&self) -> AsyncResult<Vec<User>> {
        let connection_manager = self.connection_manager.clone();
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            let users = sqlx::query_as::<_, UserModel>(
                "SELECT * FROM users
                 WHERE active = true
                 ORDER BY username ASC"
            )
            .fetch_all(&conn)
            .await
            .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
            
            // Convert to entities
            let user_entities = users.into_iter()
                .map(|model| model.to_entity())
                .collect::<Result<Vec<_>>>()
                .map_err(|e| CoreError::from(e))?;
                
            Ok(user_entities)
        })
    }
    
    fn username_exists(&self, username: &str) -> AsyncResult<bool> {
        let connection_manager = self.connection_manager.clone();
        let username = username.to_string();
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            let exists = sqlx::query("SELECT 1 FROM users WHERE username = $1")
                .bind(&username)
                .fetch_optional(&conn)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?
                .is_some();
                
            Ok(exists)
        })
    }
    
    fn email_exists(&self, email: &str) -> AsyncResult<bool> {
        let connection_manager = self.connection_manager.clone();
        let email = email.to_string();
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            let exists = sqlx::query("SELECT 1 FROM users WHERE email = $1")
                .bind(&email)
                .fetch_optional(&conn)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?
                .is_some();
                
            Ok(exists)
        })
    }
    
    fn record_login(&self, user_id: EntityId) -> AsyncResult<()> {
        let connection_manager = self.connection_manager.clone();
        let uuid = Uuid::from_bytes(user_id.as_bytes());
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            let now = Utc::now();
            
            sqlx::query(
                "UPDATE users 
                 SET last_login = $1, updated_at = $1
                 WHERE id = $2"
            )
            .bind(now)
            .bind(uuid)
            .execute(&conn)
            .await
            .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
            
            Ok(())
        })
    }
    
    fn update_password_hash(&self, user_id: EntityId, password_hash: String) -> AsyncResult<()> {
        let connection_manager = self.connection_manager.clone();
        let uuid = Uuid::from_bytes(user_id.as_bytes());
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            sqlx::query(
                "UPDATE users 
                 SET password_hash = $1, updated_at = $2
                 WHERE id = $3"
            )
            .bind(&password_hash)
            .bind(Utc::now())
            .bind(uuid)
            .execute(&conn)
            .await
            .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
            
            Ok(())
        })
    }
    
    fn change_role(&self, user_id: EntityId, role: UserRole) -> AsyncResult<()> {
        let connection_manager = self.connection_manager.clone();
        let uuid = Uuid::from_bytes(user_id.as_bytes());
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            sqlx::query(
                "UPDATE users 
                 SET role = $1, updated_at = $2
                 WHERE id = $3"
            )
            .bind(&role.to_string())
            .bind(Utc::now())
            .bind(uuid)
            .execute(&conn)
            .await
            .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
            
            Ok(())
        })
    }
    
    fn activate_user(&self, user_id: EntityId) -> AsyncResult<()> {
        let connection_manager = self.connection_manager.clone();
        let uuid = Uuid::from_bytes(user_id.as_bytes());
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            sqlx::query(
                "UPDATE users 
                 SET active = true, updated_at = $1
                 WHERE id = $2"
            )
            .bind(Utc::now())
            .bind(uuid)
            .execute(&conn)
            .await
            .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
            
            Ok(())
        })
    }
    
    fn deactivate_user(&self, user_id: EntityId) -> AsyncResult<()> {
        let connection_manager = self.connection_manager.clone();
        let uuid = Uuid::from_bytes(user_id.as_bytes());
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            sqlx::query(
                "UPDATE users 
                 SET active = false, updated_at = $1
                 WHERE id = $2"
            )
            .bind(Utc::now())
            .bind(uuid)
            .execute(&conn)
            .await
            .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
            
            Ok(())
        })
    }
    
    fn find_by_role(&self, role: UserRole) -> AsyncResult<Vec<User>> {
        let connection_manager = self.connection_manager.clone();
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            let users = sqlx::query_as::<_, UserModel>(
                "SELECT * FROM users
                 WHERE role = $1
                 ORDER BY username ASC"
            )
            .bind(&role.to_string())
            .fetch_all(&conn)
            .await
            .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
            
            // Convert to entities
            let user_entities = users.into_iter()
                .map(|model| model.to_entity())
                .collect::<Result<Vec<_>>>()
                .map_err(|e| CoreError::from(e))?;
                
            Ok(user_entities)
        })
    }
}