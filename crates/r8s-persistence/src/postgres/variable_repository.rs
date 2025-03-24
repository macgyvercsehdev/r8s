// r8s-persistence/src/postgres/variable_repository.rs
//! PostgreSQL implementation of the VariableRepository interface.

use std::sync::Arc;
use async_trait::async_trait;
use sqlx::Row;
use uuid::Uuid;

use r8s_core::common::{EntityId, Result as CoreResult, AsyncResult};
use r8s_core::entity::{Variable, VariableScope, VariableType};
use r8s_core::repository::{Repository, VariableRepository};
use r8s_core::error::Error as CoreError;

use crate::connection::ConnectionManager;
use crate::error::{PersistenceError, Result};
use crate::models::VariableModel;

/// PostgreSQL implementation of the VariableRepository.
pub struct PostgresVariableRepository {
    /// Database connection manager
    connection_manager: Arc<ConnectionManager>,
}

impl PostgresVariableRepository {
    /// Create a new PostgreSQL variable repository.
    pub fn new(connection_manager: Arc<ConnectionManager>) -> Self {
        Self {
            connection_manager,
        }
    }
}

#[async_trait]
impl Repository<Variable> for PostgresVariableRepository {
    fn find_by_id(&self, id: EntityId) -> AsyncResult<Variable> {
        let connection_manager = self.connection_manager.clone();
        let uuid = Uuid::from_bytes(id.as_bytes());
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            let variable = sqlx::query_as::<_, VariableModel>(
                "SELECT * FROM variables WHERE id = $1",
            )
            .bind(uuid)
            .fetch_optional(&conn)
            .await
            .map_err(|e| CoreError::from(PersistenceError::from(e)))?
            .ok_or_else(|| CoreError::NotFound(format!("Variable with id {} not found", id)))?;
                
            // Convert to entity
            let variable_entity = variable.to_entity()
                .map_err(|e| CoreError::from(e))?;
                
            Ok(variable_entity)
        })
    }
    
    fn save(&self, entity: Variable) -> AsyncResult<Variable> {
        let connection_manager = self.connection_manager.clone();
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            // Convert the entity to a model
            let variable_model = VariableModel::from_entity(&entity)
                .map_err(|e| CoreError::from(e))?;
                
            // Check if the variable exists
            let exists = sqlx::query("SELECT 1 FROM variables WHERE id = $1")
                .bind(variable_model.id)
                .fetch_optional(&conn)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?
                .is_some();
                
            // Insert or update the variable
            if exists {
                sqlx::query(
                    "UPDATE variables 
                     SET key = $1, value = $2, var_type = $3, scope_type = $4,
                     scope_id = $5, protected = $6, description = $7, updated_at = $8
                     WHERE id = $9"
                )
                .bind(&variable_model.key)
                .bind(&variable_model.value)
                .bind(&variable_model.var_type)
                .bind(&variable_model.scope_type)
                .bind(variable_model.scope_id)
                .bind(variable_model.protected)
                .bind(&variable_model.description)
                .bind(variable_model.updated_at)
                .bind(variable_model.id)
                .execute(&conn)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
            } else {
                sqlx::query(
                    "INSERT INTO variables 
                     (id, key, value, var_type, scope_type, scope_id, protected, 
                     description, created_at, updated_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)"
                )
                .bind(variable_model.id)
                .bind(&variable_model.key)
                .bind(&variable_model.value)
                .bind(&variable_model.var_type)
                .bind(&variable_model.scope_type)
                .bind(variable_model.scope_id)
                .bind(variable_model.protected)
                .bind(&variable_model.description)
                .bind(variable_model.created_at)
                .bind(variable_model.updated_at)
                .execute(&conn)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
            }
            
            // Return the saved variable
            PostgresVariableRepository::new(connection_manager)
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
                
            // Check if the variable exists
            let exists = sqlx::query("SELECT 1 FROM variables WHERE id = $1")
                .bind(uuid)
                .fetch_optional(&conn)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?
                .is_some();
                
            if !exists {
                return Err(CoreError::NotFound(format!("Variable with id {} not found", id)));
            }
            
            // Delete the variable
            sqlx::query("DELETE FROM variables WHERE id = $1")
                .bind(uuid)
                .execute(&conn)
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
                
            let exists = sqlx::query("SELECT 1 FROM variables WHERE id = $1")
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
impl VariableRepository for PostgresVariableRepository {
    fn find_global_variables(&self) -> AsyncResult<Vec<Variable>> {
        let connection_manager = self.connection_manager.clone();
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            let variables = sqlx::query_as::<_, VariableModel>(
                "SELECT * FROM variables
                 WHERE scope_type = 'global'
                 ORDER BY key ASC"
            )
            .fetch_all(&conn)
            .await
            .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
            
            // Convert to entities
            let variable_entities = variables.into_iter()
                .map(|model| model.to_entity())
                .collect::<Result<Vec<_>>>()
                .map_err(|e| CoreError::from(e))?;
                
            Ok(variable_entities)
        })
    }
    
    fn find_by_workflow(&self, workflow_id: EntityId) -> AsyncResult<Vec<Variable>> {
        let connection_manager = self.connection_manager.clone();
        let uuid = Uuid::from_bytes(workflow_id.as_bytes());
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            let variables = sqlx::query_as::<_, VariableModel>(
                "SELECT * FROM variables
                 WHERE scope_type = 'workflow' AND scope_id = $1
                 ORDER BY key ASC"
            )
            .bind(uuid)
            .fetch_all(&conn)
            .await
            .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
            
            // Convert to entities
            let variable_entities = variables.into_iter()
                .map(|model| model.to_entity())
                .collect::<Result<Vec<_>>>()
                .map_err(|e| CoreError::from(e))?;
                
            Ok(variable_entities)
        })
    }
    
    fn find_by_user(&self, user_id: EntityId) -> AsyncResult<Vec<Variable>> {
        let connection_manager = self.connection_manager.clone();
        let uuid = Uuid::from_bytes(user_id.as_bytes());
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            let variables = sqlx::query_as::<_, VariableModel>(
                "SELECT * FROM variables
                 WHERE scope_type = 'user' AND scope_id = $1
                 ORDER BY key ASC"
            )
            .bind(uuid)
            .fetch_all(&conn)
            .await
            .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
            
            // Convert to entities
            let variable_entities = variables.into_iter()
                .map(|model| model.to_entity())
                .collect::<Result<Vec<_>>>()
                .map_err(|e| CoreError::from(e))?;
                
            Ok(variable_entities)
        })
    }
    
    fn find_by_environment(&self, environment: &str) -> AsyncResult<Vec<Variable>> {
        let connection_manager = self.connection_manager.clone();
        let environment = environment.to_string();
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            // For environment variables, we store the environment name in the scope_id text field
            let variables = sqlx::query_as::<_, VariableModel>(
                "SELECT * FROM variables
                 WHERE scope_type = 'environment' AND scope_id_text = $1
                 ORDER BY key ASC"
            )
            .bind(environment)
            .fetch_all(&conn)
            .await
            .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
            
            // Convert to entities
            let variable_entities = variables.into_iter()
                .map(|model| model.to_entity())
                .collect::<Result<Vec<_>>>()
                .map_err(|e| CoreError::from(e))?;
                
            Ok(variable_entities)
        })
    }
    
    fn find_by_scope_and_key(&self, scope: &VariableScope, key: &str) -> AsyncResult<Option<Variable>> {
        let connection_manager = self.connection_manager.clone();
        let key = key.to_string();
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            // Build query based on scope type
            let variable = match scope {
                VariableScope::Global => {
                    sqlx::query_as::<_, VariableModel>(
                        "SELECT * FROM variables
                         WHERE scope_type = 'global' AND key = $1"
                    )
                    .bind(&key)
                    .fetch_optional(&conn)
                    .await
                    .map_err(|e| CoreError::from(PersistenceError::from(e)))?
                },
                
                VariableScope::Workflow(workflow_id) => {
                    let uuid = Uuid::from_bytes(workflow_id.as_bytes());
                    
                    sqlx::query_as::<_, VariableModel>(
                        "SELECT * FROM variables
                         WHERE scope_type = 'workflow' AND scope_id = $1 AND key = $2"
                    )
                    .bind(uuid)
                    .bind(&key)
                    .fetch_optional(&conn)
                    .await
                    .map_err(|e| CoreError::from(PersistenceError::from(e)))?
                },
                
                VariableScope::User(user_id) => {
                    let uuid = Uuid::from_bytes(user_id.as_bytes());
                    
                    sqlx::query_as::<_, VariableModel>(
                        "SELECT * FROM variables
                         WHERE scope_type = 'user' AND scope_id = $1 AND key = $2"
                    )
                    .bind(uuid)
                    .bind(&key)
                    .fetch_optional(&conn)
                    .await
                    .map_err(|e| CoreError::from(PersistenceError::from(e)))?
                },
                
                VariableScope::Environment(env) => {
                    sqlx::query_as::<_, VariableModel>(
                        "SELECT * FROM variables
                         WHERE scope_type = 'environment' AND scope_id_text = $1 AND key = $2"
                    )
                    .bind(env)
                    .bind(&key)
                    .fetch_optional(&conn)
                    .await
                    .map_err(|e| CoreError::from(PersistenceError::from(e)))?
                },
            };
            
            // Convert to entity if found
            if let Some(model) = variable {
                let variable_entity = model.to_entity()
                    .map_err(|e| CoreError::from(e))?;
                    
                Ok(Some(variable_entity))
            } else {
                Ok(None)
            }
        })
    }
    
    fn find_by_key(&self, key: &str) -> AsyncResult<Vec<Variable>> {
        let connection_manager = self.connection_manager.clone();
        let key = key.to_string();
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            let variables = sqlx::query_as::<_, VariableModel>(
                "SELECT * FROM variables
                 WHERE key = $1
                 ORDER BY scope_type ASC"
            )
            .bind(&key)
            .fetch_all(&conn)
            .await
            .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
            
            // Convert to entities
            let variable_entities = variables.into_iter()
                .map(|model| model.to_entity())
                .collect::<Result<Vec<_>>>()
                .map_err(|e| CoreError::from(e))?;
                
            Ok(variable_entities)
        })
    }
    
    fn update_value(&self, variable_id: EntityId, value: String) -> AsyncResult<()> {
        let connection_manager = self.connection_manager.clone();
        let uuid = Uuid::from_bytes(variable_id.as_bytes());
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            sqlx::query(
                "UPDATE variables 
                 SET value = $1, updated_at = $2
                 WHERE id = $3"
            )
            .bind(&value)
            .bind(chrono::Utc::now())
            .bind(uuid)
            .execute(&conn)
            .await
            .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
            
            Ok(())
        })
    }
    
    fn set_protected(&self, variable_id: EntityId, protected: bool) -> AsyncResult<()> {
        let connection_manager = self.connection_manager.clone();
        let uuid = Uuid::from_bytes(variable_id.as_bytes());
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            sqlx::query(
                "UPDATE variables 
                 SET protected = $1, updated_at = $2
                 WHERE id = $3"
            )
            .bind(protected)
            .bind(chrono::Utc::now())
            .bind(uuid)
            .execute(&conn)
            .await
            .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
            
            Ok(())
        })
    }
    
    fn has_access(&self, variable_id: EntityId, user_id: EntityId) -> AsyncResult<bool> {
        let connection_manager = self.connection_manager.clone();
        let variable_uuid = Uuid::from_bytes(variable_id.as_bytes());
        let user_uuid = Uuid::from_bytes(user_id.as_bytes());
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            // First get the variable to check its scope
            let variable = sqlx::query_as::<_, VariableModel>(
                "SELECT * FROM variables WHERE id = $1"
            )
            .bind(variable_uuid)
            .fetch_optional(&conn)
            .await
            .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
            
            if variable.is_none() {
                return Err(CoreError::NotFound(format!("Variable with id {} not found", variable_id)));
            }
            
            let variable = variable.unwrap();
            
            // Check access based on scope and user
            let has_access = match variable.scope_type.as_str() {
                // Global variables are accessible to all
                "global" => true,
                
                // User variables are only accessible to the user or admins
                "user" => {
                    if let Some(scope_id) = variable.scope_id {
                        scope_id == user_uuid || is_admin_user(user_uuid, &conn).await
                            .map_err(|e| CoreError::from(e))?
                    } else {
                        false
                    }
                },
                
                // Workflow variables are accessible to workflow owners or admins
                "workflow" => {
                    if let Some(workflow_id) = variable.scope_id {
                        has_workflow_access(workflow_id, user_uuid, &conn).await
                            .map_err(|e| CoreError::from(e))?
                    } else {
                        false
                    }
                },
                
                // Environment variables are accessible to admins only
                "environment" => {
                    is_admin_user(user_uuid, &conn).await
                        .map_err(|e| CoreError::from(e))?
                },
                
                // Unknown scope type
                _ => false,
            };
            
            Ok(has_access)
        })
    }
}

/// Check if a user is an admin
async fn is_admin_user(user_id: Uuid, conn: &sqlx::PgPool) -> Result<bool> {
    let is_admin = sqlx::query(
        "SELECT 1 FROM users
         WHERE id = $1 AND role IN ('admin', 'team_admin')"
    )
    .bind(user_id)
    .fetch_optional(conn)
    .await
    .map_err(|e| PersistenceError::from(e))?
    .is_some();
    
    Ok(is_admin)
}

/// Check if a user has access to a workflow
async fn has_workflow_access(workflow_id: Uuid, user_id: Uuid, conn: &sqlx::PgPool) -> Result<bool> {
    // Check if user is admin or owner of the workflow
    let has_access = sqlx::query(
        "SELECT 1 FROM workflows
         WHERE id = $1 AND (
             created_by = $2 OR
             $2 IN (SELECT user_id FROM workflow_collaborators WHERE workflow_id = $1) OR
             EXISTS (SELECT 1 FROM users WHERE id = $2 AND role IN ('admin', 'team_admin'))
         )"
    )
    .bind(workflow_id)
    .bind(user_id)
    .fetch_optional(conn)
    .await
    .map_err(|e| PersistenceError::from(e))?
    .is_some();
    
    Ok(has_access)
}