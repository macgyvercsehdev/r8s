// r8s-persistence/src/postgres/execution_repository.rs
//! PostgreSQL implementation of the ExecutionRepository interface.

use std::sync::Arc;
use async_trait::async_trait;
use sqlx::{postgres::PgRow, Row};
use uuid::Uuid;
use chrono::{DateTime, Utc};

use r8s_core::common::{EntityId, ExecutionState, Priority, Result as CoreResult, AsyncResult};
use r8s_core::entity::{Execution, ExecutionLog, NodeExecutionResult};
use r8s_core::repository::{Repository, ExecutionRepository, ExecutionFilter, ExecutionSortField};
use r8s_core::error::Error as CoreError;

use crate::connection::ConnectionManager;
use crate::error::{PersistenceError, Result};
use crate::models::{ExecutionModel, NodeExecutionResultModel, ExecutionLogModel};

/// PostgreSQL implementation of the ExecutionRepository.
pub struct PostgresExecutionRepository {
    /// Database connection manager
    connection_manager: Arc<ConnectionManager>,
}

impl PostgresExecutionRepository {
    /// Create a new PostgreSQL execution repository.
    pub fn new(connection_manager: Arc<ConnectionManager>) -> Self {
        Self {
            connection_manager,
        }
    }
    
    /// Load an execution with all its related data from the database.
    async fn load_execution_with_relations(&self, id: Uuid) -> Result<Execution> {
        let conn = self.connection_manager.get_connection().await?;
        
        // Load the execution
        let execution = sqlx::query_as::<_, ExecutionModel>(
            "SELECT * FROM executions WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&conn)
        .await?
        .ok_or_else(|| PersistenceError::NotFoundError {
            entity: "Execution".to_string(),
            id: id.to_string(),
        })?;
        
        // Load node execution results
        let node_results = sqlx::query_as::<_, NodeExecutionResultModel>(
            "SELECT * FROM node_execution_results WHERE execution_id = $1",
        )
        .bind(id)
        .fetch_all(&conn)
        .await?;
        
        // Load execution logs
        let logs = sqlx::query_as::<_, ExecutionLogModel>(
            "SELECT * FROM execution_logs WHERE execution_id = $1 ORDER BY timestamp ASC",
        )
        .bind(id)
        .fetch_all(&conn)
        .await?;
        
        // Convert models to entities
        let mut execution_entity = execution.to_entity()?;
        
        // Add node results
        for node_result in node_results {
            let node_result_entity = node_result.to_entity()?;
            execution_entity.results.insert(node_result_entity.node_id, node_result_entity);
        }
        
        // Add logs
        execution_entity.logs = logs.into_iter()
            .map(|log| log.to_entity())
            .collect::<Result<Vec<_>>>()?;
        
        Ok(execution_entity)
    }
}

#[async_trait]
impl Repository<Execution> for PostgresExecutionRepository {
    fn find_by_id(&self, id: EntityId) -> AsyncResult<Execution> {
        let connection_manager = self.connection_manager.clone();
        let uuid = Uuid::from_bytes(id.as_bytes());
        
        Box::pin(async move {
            let execution = PostgresExecutionRepository::new(connection_manager)
                .load_execution_with_relations(uuid)
                .await
                .map_err(|e| e.into())?;
                
            Ok(execution)
        })
    }
    
    fn save(&self, entity: Execution) -> AsyncResult<Execution> {
        let connection_manager = self.connection_manager.clone();
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            // Start a transaction
            let mut tx = conn.begin().await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
                
            // Convert the entity to a model
            let execution_model = ExecutionModel::from_entity(&entity)
                .map_err(|e| CoreError::from(e))?;
                
            // Check if the execution exists
            let exists = sqlx::query("SELECT 1 FROM executions WHERE id = $1")
                .bind(execution_model.id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?
                .is_some();
                
            // Insert or update the execution
            if exists {
                sqlx::query(
                    "UPDATE executions 
                     SET workflow_id = $1, workflow_version = $2, state = $3,
                     initial_data = $4, started_at = $5, finished_at = $6,
                     duration_ms = $7, initiated_by = $8, priority = $9,
                     attempt = $10, failure_reason = $11, failed_node_id = $12,
                     parent_execution_id = $13, worker_id = $14, tags = $15, 
                     updated_at = $16
                     WHERE id = $17"
                )
                .bind(execution_model.workflow_id)
                .bind(execution_model.workflow_version)
                .bind(&execution_model.state)
                .bind(&execution_model.initial_data)
                .bind(execution_model.started_at)
                .bind(execution_model.finished_at)
                .bind(execution_model.duration_ms)
                .bind(execution_model.initiated_by)
                .bind(&execution_model.priority)
                .bind(execution_model.attempt)
                .bind(&execution_model.failure_reason)
                .bind(execution_model.failed_node_id)
                .bind(execution_model.parent_execution_id)
                .bind(&execution_model.worker_id)
                .bind(&execution_model.tags)
                .bind(execution_model.updated_at)
                .bind(execution_model.id)
                .execute(&mut *tx)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
            } else {
                sqlx::query(
                    "INSERT INTO executions 
                     (id, workflow_id, workflow_version, state, initial_data,
                     started_at, finished_at, duration_ms, initiated_by, priority,
                     attempt, failure_reason, failed_node_id, parent_execution_id,
                     worker_id, tags, created_at, updated_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)"
                )
                .bind(execution_model.id)
                .bind(execution_model.workflow_id)
                .bind(execution_model.workflow_version)
                .bind(&execution_model.state)
                .bind(&execution_model.initial_data)
                .bind(execution_model.started_at)
                .bind(execution_model.finished_at)
                .bind(execution_model.duration_ms)
                .bind(execution_model.initiated_by)
                .bind(&execution_model.priority)
                .bind(execution_model.attempt)
                .bind(&execution_model.failure_reason)
                .bind(execution_model.failed_node_id)
                .bind(execution_model.parent_execution_id)
                .bind(&execution_model.worker_id)
                .bind(&execution_model.tags)
                .bind(execution_model.created_at)
                .bind(execution_model.updated_at)
                .execute(&mut *tx)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
            }
            
            // Update child execution IDs
            if !entity.child_execution_ids.is_empty() {
                // Delete existing relationships first
                sqlx::query("DELETE FROM execution_children WHERE parent_id = $1")
                    .bind(execution_model.id)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
                
                // Insert new relationships
                for child_id in &entity.child_execution_ids {
                    let child_uuid = Uuid::from_bytes(child_id.as_bytes());
                    
                    sqlx::query(
                        "INSERT INTO execution_children (parent_id, child_id)
                         VALUES ($1, $2)"
                    )
                    .bind(execution_model.id)
                    .bind(child_uuid)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
                }
            }
            
            // Handle node execution results
            for (node_id, result) in &entity.results {
                // Convert to model
                let node_uuid = Uuid::from_bytes(node_id.as_bytes());
                let result_model = NodeExecutionResultModel::from_entity(result, execution_model.id)
                    .map_err(|e| CoreError::from(e))?;
                
                // Check if the result exists
                let result_exists = sqlx::query(
                    "SELECT 1 FROM node_execution_results 
                     WHERE execution_id = $1 AND node_id = $2"
                )
                .bind(execution_model.id)
                .bind(node_uuid)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?
                .is_some();
                
                // Insert or update the result
                if result_exists {
                    sqlx::query(
                        "UPDATE node_execution_results 
                         SET state = $1, started_at = $2, finished_at = $3,
                         duration_ms = $4, input_data = $5, output_data = $6,
                         failure_reason = $7, attempt = $8, worker_id = $9
                         WHERE execution_id = $10 AND node_id = $11"
                    )
                    .bind(&result_model.state)
                    .bind(result_model.started_at)
                    .bind(result_model.finished_at)
                    .bind(result_model.duration_ms)
                    .bind(&result_model.input_data)
                    .bind(&result_model.output_data)
                    .bind(&result_model.failure_reason)
                    .bind(result_model.attempt)
                    .bind(&result_model.worker_id)
                    .bind(execution_model.id)
                    .bind(node_uuid)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
                } else {
                    sqlx::query(
                        "INSERT INTO node_execution_results 
                         (execution_id, node_id, state, started_at, finished_at,
                         duration_ms, input_data, output_data, failure_reason, 
                         attempt, worker_id)
                         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)"
                    )
                    .bind(execution_model.id)
                    .bind(node_uuid)
                    .bind(&result_model.state)
                    .bind(result_model.started_at)
                    .bind(result_model.finished_at)
                    .bind(result_model.duration_ms)
                    .bind(&result_model.input_data)
                    .bind(&result_model.output_data)
                    .bind(&result_model.failure_reason)
                    .bind(result_model.attempt)
                    .bind(&result_model.worker_id)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
                }
            }
            
            // Save execution logs
            for log in &entity.logs {
                let log_model = ExecutionLogModel::from_entity(log, execution_model.id)
                    .map_err(|e| CoreError::from(e))?;
                
                // Check if this log already exists (using timestamp and message as composite key)
                let log_exists = sqlx::query(
                    "SELECT 1 FROM execution_logs 
                     WHERE execution_id = $1 AND timestamp = $2 AND message = $3"
                )
                .bind(execution_model.id)
                .bind(log_model.timestamp)
                .bind(&log_model.message)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?
                .is_some();
                
                // Only insert if the log doesn't exist
                if !log_exists {
                    sqlx::query(
                        "INSERT INTO execution_logs 
                         (execution_id, timestamp, level, message, node_id, metadata)
                         VALUES ($1, $2, $3, $4, $5, $6)"
                    )
                    .bind(execution_model.id)
                    .bind(log_model.timestamp)
                    .bind(&log_model.level)
                    .bind(&log_model.message)
                    .bind(log_model.node_id)
                    .bind(&log_model.metadata)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
                }
            }
            
            // Commit the transaction
            tx.commit()
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
                
            // Load the saved execution
            PostgresExecutionRepository::new(connection_manager)
                .load_execution_with_relations(execution_model.id)
                .await
                .map_err(|e| e.into())
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
                
            // Check if the execution exists
            let exists = sqlx::query("SELECT 1 FROM executions WHERE id = $1")
                .bind(uuid)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?
                .is_some();
                
            if !exists {
                return Err(CoreError::NotFound(format!("Execution with id {} not found", id)));
            }
            
            // Delete in proper order to respect foreign key constraints
            
            // Delete execution logs
            sqlx::query("DELETE FROM execution_logs WHERE execution_id = $1")
                .bind(uuid)
                .execute(&mut *tx)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
                
            // Delete node execution results
            sqlx::query("DELETE FROM node_execution_results WHERE execution_id = $1")
                .bind(uuid)
                .execute(&mut *tx)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
                
            // Delete execution children relationships
            sqlx::query("DELETE FROM execution_children WHERE parent_id = $1 OR child_id = $1")
                .bind(uuid)
                .execute(&mut *tx)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
                
            // Finally delete the execution
            sqlx::query("DELETE FROM executions WHERE id = $1")
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
                
            let exists = sqlx::query("SELECT 1 FROM executions WHERE id = $1")
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
impl ExecutionRepository for PostgresExecutionRepository {
    fn find_by_filter(&self, filter: ExecutionFilter) -> AsyncResult<Vec<Execution>> {
        let connection_manager = self.connection_manager.clone();
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            // Build the query
            let mut query = "SELECT DISTINCT e.* FROM executions e".to_string();
            let mut conditions = Vec::new();
            let mut params: Vec<sqlx::types::Any> = Vec::new();
            let mut param_count = 1;
            
            // Add workflow_id filter
            if let Some(workflow_id) = filter.workflow_id {
                conditions.push(format!("e.workflow_id = ${}", param_count));
                params.push(sqlx::types::Any::from(Uuid::from_bytes(workflow_id.as_bytes())));
                param_count += 1;
            }
            
            // Add state filter
            if let Some(state) = filter.state {
                conditions.push(format!("e.state = ${}", param_count));
                params.push(sqlx::types::Any::from(state.to_string()));
                param_count += 1;
            }
            
            // Add initiated_by filter
            if let Some(initiated_by) = filter.initiated_by {
                conditions.push(format!("e.initiated_by = ${}", param_count));
                params.push(sqlx::types::Any::from(Uuid::from_bytes(initiated_by.as_bytes())));
                param_count += 1;
            }
            
            // Add date range filters
            if let Some(from_date) = filter.from_date {
                conditions.push(format!("e.started_at >= ${}", param_count));
                params.push(sqlx::types::Any::from(from_date));
                param_count += 1;
            }
            
            if let Some(to_date) = filter.to_date {
                conditions.push(format!("e.started_at <= ${}", param_count));
                params.push(sqlx::types::Any::from(to_date));
                param_count += 1;
            }
            
            // Add failed_only filter
            if filter.failed_only {
                conditions.push("e.state = 'failed'".to_string());
            }
            
            // Add WHERE clause if there are conditions
            if !conditions.is_empty() {
                query.push_str(" WHERE ");
                query.push_str(&conditions.join(" AND "));
            }
            
            // Add ORDER BY clause
            if let Some(sort_by) = filter.sort_by {
                let sort_field = match sort_by {
                    ExecutionSortField::StartedAt => "e.started_at",
                    ExecutionSortField::FinishedAt => "e.finished_at",
                    ExecutionSortField::Duration => "e.duration_ms",
                };
                
                let direction = if filter.ascending { "ASC" } else { "DESC" };
                query.push_str(&format!(" ORDER BY {} {}", sort_field, direction));
            } else {
                // Default sorting
                query.push_str(" ORDER BY e.started_at DESC");
            }
            
            // Add LIMIT and OFFSET
            if let Some(limit) = filter.limit {
                query.push_str(&format!(" LIMIT {}", limit));
            }
            
            if let Some(offset) = filter.offset {
                query.push_str(&format!(" OFFSET {}", offset));
            }
            
            // Execute the query
            let mut q = sqlx::query_as::<_, ExecutionModel>(&query);
            
            // Bind parameters
            for param in params {
                q = q.bind(param);
            }
            
            let execution_models = q.fetch_all(&conn)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
                
            // Load full executions with relations
            let mut executions = Vec::new();
            for model in execution_models {
                let execution = PostgresExecutionRepository::new(connection_manager.clone())
                    .load_execution_with_relations(model.id)
                    .await
                    .map_err(|e| CoreError::from(e))?;
                    
                executions.push(execution);
            }
            
            Ok(executions)
        })
    }
    
    fn count_by_filter(&self, filter: ExecutionFilter) -> AsyncResult<usize> {
        let connection_manager = self.connection_manager.clone();
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            // Build the query
            let mut query = "SELECT COUNT(DISTINCT e.id) FROM executions e".to_string();
            let mut conditions = Vec::new();
            let mut params: Vec<sqlx::types::Any> = Vec::new();
            let mut param_count = 1;
            
            // Add workflow_id filter
            if let Some(workflow_id) = filter.workflow_id {
                conditions.push(format!("e.workflow_id = ${}", param_count));
                params.push(sqlx::types::Any::from(Uuid::from_bytes(workflow_id.as_bytes())));
                param_count += 1;
            }
            
            // Add state filter
            if let Some(state) = filter.state {
                conditions.push(format!("e.state = ${}", param_count));
                params.push(sqlx::types::Any::from(state.to_string()));
                param_count += 1;
            }
            
            // Add initiated_by filter
            if let Some(initiated_by) = filter.initiated_by {
                conditions.push(format!("e.initiated_by = ${}", param_count));
                params.push(sqlx::types::Any::from(Uuid::from_bytes(initiated_by.as_bytes())));
                param_count += 1;
            }
            
            // Add date range filters
            if let Some(from_date) = filter.from_date {
                conditions.push(format!("e.started_at >= ${}", param_count));
                params.push(sqlx::types::Any::from(from_date));
                param_count += 1;
            }
            
            if let Some(to_date) = filter.to_date {
                conditions.push(format!("e.started_at <= ${}", param_count));
                params.push(sqlx::types::Any::from(to_date));
                param_count += 1;
            }
            
            // Add failed_only filter
            if filter.failed_only {
                conditions.push("e.state = 'failed'".to_string());
            }
            
            // Add WHERE clause if there are conditions
            if !conditions.is_empty() {
                query.push_str(" WHERE ");
                query.push_str(&conditions.join(" AND "));
            }
            
            // Execute the query
            let mut q = sqlx::query(&query);
            
            // Bind parameters
            for param in params {
                q = q.bind(param);
            }
            
            let count: i64 = q.fetch_one(&conn)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?
                .get(0);
                
            Ok(count as usize)
        })
    }
    
    fn find_by_workflow(&self, workflow_id: EntityId) -> AsyncResult<Vec<Execution>> {
        let connection_manager = self.connection_manager.clone();
        let uuid = Uuid::from_bytes(workflow_id.as_bytes());
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            let execution_models = sqlx::query_as::<_, ExecutionModel>(
                "SELECT * FROM executions
                 WHERE workflow_id = $1
                 ORDER BY started_at DESC"
            )
            .bind(uuid)
            .fetch_all(&conn)
            .await
            .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
            
            // Load full executions with relations
            let mut executions = Vec::new();
            for model in execution_models {
                let execution = PostgresExecutionRepository::new(connection_manager.clone())
                    .load_execution_with_relations(model.id)
                    .await
                    .map_err(|e| CoreError::from(e))?;
                    
                executions.push(execution);
            }
            
            Ok(executions)
        })
    }
    
    fn find_by_initiator(&self, user_id: EntityId) -> AsyncResult<Vec<Execution>> {
        let connection_manager = self.connection_manager.clone();
        let uuid = Uuid::from_bytes(user_id.as_bytes());
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            let execution_models = sqlx::query_as::<_, ExecutionModel>(
                "SELECT * FROM executions
                 WHERE initiated_by = $1
                 ORDER BY started_at DESC"
            )
            .bind(uuid)
            .fetch_all(&conn)
            .await
            .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
            
            // Load full executions with relations
            let mut executions = Vec::new();
            for model in execution_models {
                let execution = PostgresExecutionRepository::new(connection_manager.clone())
                    .load_execution_with_relations(model.id)
                    .await
                    .map_err(|e| CoreError::from(e))?;
                    
                executions.push(execution);
            }
            
            Ok(executions)
        })
    }
    
    fn find_pending_executions(&self, limit: usize) -> AsyncResult<Vec<Execution>> {
        let connection_manager = self.connection_manager.clone();
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            let execution_models = sqlx::query_as::<_, ExecutionModel>(
                "SELECT * FROM executions
                 WHERE state = 'pending'
                 ORDER BY priority ASC, started_at ASC
                 LIMIT $1"
            )
            .bind(limit as i64)
            .fetch_all(&conn)
            .await
            .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
            
            // Load full executions with relations
            let mut executions = Vec::new();
            for model in execution_models {
                let execution = PostgresExecutionRepository::new(connection_manager.clone())
                    .load_execution_with_relations(model.id)
                    .await
                    .map_err(|e| CoreError::from(e))?;
                    
                executions.push(execution);
            }
            
            Ok(executions)
        })
    }
    
    fn find_timed_out_executions(&self) -> AsyncResult<Vec<Execution>> {
        let connection_manager = self.connection_manager.clone();
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            // Get timeout threshold by joining with workflows table and checking settings
            let execution_models = sqlx::query_as::<_, ExecutionModel>(
                "SELECT e.* FROM executions e
                 JOIN workflows w ON e.workflow_id = w.id
                 WHERE e.state = 'running'
                 AND e.started_at < NOW() - (w.settings->>'execution_timeout_seconds')::integer * interval '1 second'
                 ORDER BY e.started_at ASC"
            )
            .fetch_all(&conn)
            .await
            .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
            
            // Load full executions with relations
            let mut executions = Vec::new();
            for model in execution_models {
                let execution = PostgresExecutionRepository::new(connection_manager.clone())
                    .load_execution_with_relations(model.id)
                    .await
                    .map_err(|e| CoreError::from(e))?;
                    
                executions.push(execution);
            }
            
            Ok(executions)
        })
    }
    
    fn add_node_result(&self, execution_id: EntityId, result: NodeExecutionResult) -> AsyncResult<()> {
        let connection_manager = self.connection_manager.clone();
        let execution_uuid = Uuid::from_bytes(execution_id.as_bytes());
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            // Convert node ID to UUID
            let node_uuid = Uuid::from_bytes(result.node_id.as_bytes());
            
            // Convert to model
            let result_model = NodeExecutionResultModel::from_entity(&result, execution_uuid)
                .map_err(|e| CoreError::from(e))?;
                
            // Check if the result exists
            let result_exists = sqlx::query(
                "SELECT 1 FROM node_execution_results 
                 WHERE execution_id = $1 AND node_id = $2"
            )
            .bind(execution_uuid)
            .bind(node_uuid)
            .fetch_optional(&conn)
            .await
            .map_err(|e| CoreError::from(PersistenceError::from(e)))?
            .is_some();
            
            // Insert or update the result
            if result_exists {
                sqlx::query(
                    "UPDATE node_execution_results 
                     SET state = $1, started_at = $2, finished_at = $3,
                     duration_ms = $4, input_data = $5, output_data = $6,
                     failure_reason = $7, attempt = $8, worker_id = $9
                     WHERE execution_id = $10 AND node_id = $11"
                )
                .bind(&result_model.state)
                .bind(result_model.started_at)
                .bind(result_model.finished_at)
                .bind(result_model.duration_ms)
                .bind(&result_model.input_data)
                .bind(&result_model.output_data)
                .bind(&result_model.failure_reason)
                .bind(result_model.attempt)
                .bind(&result_model.worker_id)
                .bind(execution_uuid)
                .bind(node_uuid)
                .execute(&conn)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
            } else {
                sqlx::query(
                    "INSERT INTO node_execution_results 
                     (execution_id, node_id, state, started_at, finished_at,
                     duration_ms, input_data, output_data, failure_reason, 
                     attempt, worker_id)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)"
                )
                .bind(execution_uuid)
                .bind(node_uuid)
                .bind(&result_model.state)
                .bind(result_model.started_at)
                .bind(result_model.finished_at)
                .bind(result_model.duration_ms)
                .bind(&result_model.input_data)
                .bind(&result_model.output_data)
                .bind(&result_model.failure_reason)
                .bind(result_model.attempt)
                .bind(&result_model.worker_id)
                .execute(&conn)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
            }
            
            Ok(())
        })
    }
    
    fn add_logs(&self, execution_id: EntityId, logs: Vec<ExecutionLog>) -> AsyncResult<()> {
        let connection_manager = self.connection_manager.clone();
        let execution_uuid = Uuid::from_bytes(execution_id.as_bytes());
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            // Start a transaction
            let mut tx = conn.begin().await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
                
            // Insert logs that don't already exist
            for log in logs {
                let log_model = ExecutionLogModel::from_entity(&log, execution_uuid)
                    .map_err(|e| CoreError::from(e))?;
                
                // Check if this log already exists (using timestamp and message as composite key)
                let log_exists = sqlx::query(
                    "SELECT 1 FROM execution_logs 
                     WHERE execution_id = $1 AND timestamp = $2 AND message = $3"
                )
                .bind(execution_uuid)
                .bind(log_model.timestamp)
                .bind(&log_model.message)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?
                .is_some();
                
                // Only insert if the log doesn't exist
                if !log_exists {
                    sqlx::query(
                        "INSERT INTO execution_logs 
                         (execution_id, timestamp, level, message, node_id, metadata)
                         VALUES ($1, $2, $3, $4, $5, $6)"
                    )
                    .bind(execution_uuid)
                    .bind(log_model.timestamp)
                    .bind(&log_model.level)
                    .bind(&log_model.message)
                    .bind(log_model.node_id)
                    .bind(&log_model.metadata)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
                }
            }
            
            // Commit the transaction
            tx.commit()
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
                
            Ok(())
        })
    }
    
    fn update_state(&self, execution_id: EntityId, state: ExecutionState) -> AsyncResult<()> {
        let connection_manager = self.connection_manager.clone();
        let execution_uuid = Uuid::from_bytes(execution_id.as_bytes());
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            // Update the execution state
            sqlx::query(
                "UPDATE executions 
                 SET state = $1, updated_at = $2
                 WHERE id = $3"
            )
            .bind(&state.to_string())
            .bind(Utc::now())
            .bind(execution_uuid)
            .execute(&conn)
            .await
            .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
            
            // If the state is completed or failed, update finished_at and duration_ms
            if state == ExecutionState::Success || 
               state == ExecutionState::Failed ||
               state == ExecutionState::Cancelled {
                
                sqlx::query(
                    "UPDATE executions 
                     SET finished_at = $1, 
                     duration_ms = EXTRACT(EPOCH FROM ($1 - started_at)) * 1000
                     WHERE id = $2 AND finished_at IS NULL"
                )
                .bind(Utc::now())
                .bind(execution_uuid)
                .execute(&conn)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
            }
            
            Ok(())
        })
    }
    
    fn find_child_executions(&self, parent_execution_id: EntityId) -> AsyncResult<Vec<Execution>> {
        let connection_manager = self.connection_manager.clone();
        let parent_uuid = Uuid::from_bytes(parent_execution_id.as_bytes());
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            let execution_models = sqlx::query_as::<_, ExecutionModel>(
                "SELECT e.* FROM executions e
                 JOIN execution_children ec ON e.id = ec.child_id
                 WHERE ec.parent_id = $1
                 ORDER BY e.started_at ASC"
            )
            .bind(parent_uuid)
            .fetch_all(&conn)
            .await
            .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
            
            // Load full executions with relations
            let mut executions = Vec::new();
            for model in execution_models {
                let execution = PostgresExecutionRepository::new(connection_manager.clone())
                    .load_execution_with_relations(model.id)
                    .await
                    .map_err(|e| CoreError::from(e))?;
                    
                executions.push(execution);
            }
            
            Ok(executions)
        })
    }
}