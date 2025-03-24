// r8s-persistence/src/postgres/workflow_repository.rs
//! PostgreSQL implementation of the WorkflowRepository interface.

use std::sync::Arc;
use async_trait::async_trait;
use sqlx::{postgres::PgRow, Row};
use uuid::Uuid;

use r8s_core::common::{EntityId, Result as CoreResult, AsyncResult};
use r8s_core::entity::{Workflow, Tag};
use r8s_core::repository::{Repository, WorkflowRepository, WorkflowFilter, WorkflowSortField};
use r8s_core::error::Error as CoreError;

use crate::connection::ConnectionManager;
use crate::error::{PersistenceError, Result};
use crate::models::{WorkflowModel, NodeModel, ConnectionModel, TagModel, NodePortModel};

/// PostgreSQL implementation of the WorkflowRepository.
pub struct PostgresWorkflowRepository {
    /// Database connection manager
    connection_manager: Arc<ConnectionManager>,
}

impl PostgresWorkflowRepository {
    /// Create a new PostgreSQL workflow repository.
    pub fn new(connection_manager: Arc<ConnectionManager>) -> Self {
        Self {
            connection_manager,
        }
    }
    
    /// Load a workflow with all its relations from the database.
    async fn load_workflow_with_relations(&self, id: Uuid) -> Result<Workflow> {
        let conn = self.connection_manager.get_connection().await?;
        
        // Load the workflow
        let workflow = sqlx::query_as::<_, WorkflowModel>(
            "SELECT * FROM workflows WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&conn)
        .await?
        .ok_or_else(|| PersistenceError::NotFoundError {
            entity: "Workflow".to_string(),
            id: id.to_string(),
        })?;
        
        // Load nodes
        let nodes = sqlx::query_as::<_, NodeModel>(
            "SELECT * FROM nodes WHERE workflow_id = $1",
        )
        .bind(id)
        .fetch_all(&conn)
        .await?;
        
        // Load node ports
        let mut node_ports = Vec::new();
        for node in &nodes {
            let ports = sqlx::query_as::<_, NodePortModel>(
                "SELECT * FROM node_ports WHERE node_id = $1",
            )
            .bind(node.id)
            .fetch_all(&conn)
            .await?;
            
            node_ports.extend(ports);
        }
        
        // Load connections
        let connections = sqlx::query_as::<_, ConnectionModel>(
            "SELECT * FROM connections WHERE workflow_id = $1",
        )
        .bind(id)
        .fetch_all(&conn)
        .await?;
        
        // Load tags
        let tags = sqlx::query_as::<_, TagModel>(
            "SELECT t.* FROM tags t
             JOIN workflow_tags wt ON t.id = wt.tag_id
             WHERE wt.workflow_id = $1",
        )
        .bind(id)
        .fetch_all(&conn)
        .await?;
        
        // Get credentials for nodes
        let mut node_credentials = std::collections::HashMap::new();
        let node_creds = sqlx::query(
            "SELECT node_id, credential_id FROM node_credentials
             WHERE node_id IN (SELECT id FROM nodes WHERE workflow_id = $1)",
        )
        .bind(id)
        .fetch_all(&conn)
        .await?;
        
        for row in node_creds {
            let node_id: Uuid = row.get("node_id");
            let credential_id: Uuid = row.get("credential_id");
            
            node_credentials
                .entry(node_id)
                .or_insert_with(Vec::new)
                .push(credential_id);
        }
        
        // Convert nodes with ports and credentials
        let converted_nodes = nodes
            .into_iter()
            .map(|node| {
                let node_ports_filtered = node_ports.iter()
                    .filter(|p| p.node_id == node.id)
                    .cloned()
                    .collect::<Vec<_>>();
                    
                let credentials = node_credentials
                    .get(&node.id)
                    .cloned()
                    .unwrap_or_default();
                    
                node.to_entity_with_ports(node_ports_filtered, credentials)
            })
            .collect::<Result<Vec<_>>>()?;
            
        // Convert connections
        let converted_connections = connections
            .into_iter()
            .map(|conn| conn.to_entity())
            .collect::<Result<Vec<_>>>()?;
            
        // Convert tags
        let converted_tags = tags
            .into_iter()
            .map(|tag| tag.to_entity())
            .collect::<Result<Vec<_>>>()?;
            
        // Create a complete workflow entity
        let mut workflow_entity = workflow.to_entity()?;
        workflow_entity.nodes = converted_nodes;
        workflow_entity.connections = converted_connections;
        workflow_entity.tags = converted_tags;
        
        Ok(workflow_entity)
    }
}

#[async_trait]
impl Repository<Workflow> for PostgresWorkflowRepository {
    fn find_by_id(&self, id: EntityId) -> AsyncResult<Workflow> {
        let connection_manager = self.connection_manager.clone();
        let uuid = Uuid::from_bytes(id.as_bytes());
        
        Box::pin(async move {
            let workflow = PostgresWorkflowRepository::new(connection_manager)
                .load_workflow_with_relations(uuid)
                .await
                .map_err(|e| e.into())?;
                
            Ok(workflow)
        })
    }
    
    fn save(&self, entity: Workflow) -> AsyncResult<Workflow> {
        let connection_manager = self.connection_manager.clone();
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            // Start a transaction
            let mut tx = conn.begin().await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
                
            // Convert the entity to a model
            let workflow_model = WorkflowModel::from_entity(&entity)
                .map_err(|e| CoreError::from(e))?;
                
            // Check if the workflow exists
            let exists = sqlx::query("SELECT 1 FROM workflows WHERE id = $1")
                .bind(workflow_model.id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?
                .is_some();
                
            // Insert or update the workflow
            if exists {
                sqlx::query(
                    "UPDATE workflows 
                     SET name = $1, description = $2, version = $3, active = $4, 
                     static_data = $5, settings = $6, metadata = $7, updated_at = $8,
                     created_by = $9
                     WHERE id = $10"
                )
                .bind(&workflow_model.name)
                .bind(&workflow_model.description)
                .bind(workflow_model.version)
                .bind(workflow_model.active)
                .bind(workflow_model.static_data)
                .bind(&workflow_model.settings)
                .bind(&workflow_model.metadata)
                .bind(workflow_model.updated_at)
                .bind(workflow_model.created_by)
                .bind(workflow_model.id)
                .execute(&mut *tx)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
            } else {
                sqlx::query(
                    "INSERT INTO workflows 
                     (id, name, description, version, active, static_data, settings, 
                     metadata, created_at, updated_at, created_by)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)"
                )
                .bind(workflow_model.id)
                .bind(&workflow_model.name)
                .bind(&workflow_model.description)
                .bind(workflow_model.version)
                .bind(workflow_model.active)
                .bind(workflow_model.static_data)
                .bind(&workflow_model.settings)
                .bind(&workflow_model.metadata)
                .bind(workflow_model.created_at)
                .bind(workflow_model.updated_at)
                .bind(workflow_model.created_by)
                .execute(&mut *tx)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
            }
            
            // Remove existing nodes, connections, and node credentials for the workflow
            sqlx::query("DELETE FROM connections WHERE workflow_id = $1")
                .bind(workflow_model.id)
                .execute(&mut *tx)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
                
            sqlx::query("DELETE FROM node_credentials WHERE node_id IN (SELECT id FROM nodes WHERE workflow_id = $1)")
                .bind(workflow_model.id)
                .execute(&mut *tx)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
                
            sqlx::query("DELETE FROM node_ports WHERE node_id IN (SELECT id FROM nodes WHERE workflow_id = $1)")
                .bind(workflow_model.id)
                .execute(&mut *tx)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
                
            sqlx::query("DELETE FROM nodes WHERE workflow_id = $1")
                .bind(workflow_model.id)
                .execute(&mut *tx)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
                
            // Insert nodes
            for node in &entity.nodes {
                let (node_model, node_ports) = NodeModel::from_entity(node, workflow_model.id)
                    .map_err(|e| CoreError::from(e))?;
                    
                // Insert the node
                sqlx::query(
                    "INSERT INTO nodes 
                     (id, workflow_id, name, type_name, type_version, type_category, 
                     plugin_id, position_x, position_y, parameters, disabled, settings, 
                     metadata, notes, created_at, updated_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)"
                )
                .bind(node_model.id)
                .bind(node_model.workflow_id)
                .bind(&node_model.name)
                .bind(&node_model.type_name)
                .bind(&node_model.type_version)
                .bind(&node_model.type_category)
                .bind(node_model.plugin_id)
                .bind(node_model.position_x)
                .bind(node_model.position_y)
                .bind(&node_model.parameters)
                .bind(node_model.disabled)
                .bind(&node_model.settings)
                .bind(&node_model.metadata)
                .bind(&node_model.notes)
                .bind(node_model.created_at)
                .bind(node_model.updated_at)
                .execute(&mut *tx)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
                
                // Insert node ports
                for port in node_ports {
                    sqlx::query(
                        "INSERT INTO node_ports 
                         (id, node_id, name, is_input, data_type, description, 
                         required, examples, schema, created_at, updated_at)
                         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)"
                    )
                    .bind(port.id)
                    .bind(port.node_id)
                    .bind(&port.name)
                    .bind(port.is_input)
                    .bind(&port.data_type)
                    .bind(&port.description)
                    .bind(port.required)
                    .bind(&port.examples)
                    .bind(&port.schema)
                    .bind(port.created_at)
                    .bind(port.updated_at)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
                }
                
                // Insert node credentials
                for credential_id in &node.credentials {
                    let cred_uuid = Uuid::from_bytes(credential_id.as_bytes());
                    
                    sqlx::query(
                        "INSERT INTO node_credentials (node_id, credential_id)
                         VALUES ($1, $2)"
                    )
                    .bind(node_model.id)
                    .bind(cred_uuid)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
                }
            }
            
            // Insert connections
            for connection in &entity.connections {
                let conn_model = ConnectionModel::from_entity(connection, workflow_model.id)
                    .map_err(|e| CoreError::from(e))?;
                    
                sqlx::query(
                    "INSERT INTO connections 
                     (id, workflow_id, source_node, source_output, target_node, 
                     target_input, condition_type, condition_expression, transform, 
                     metadata, created_at, updated_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)"
                )
                .bind(conn_model.id)
                .bind(conn_model.workflow_id)
                .bind(conn_model.source_node)
                .bind(&conn_model.source_output)
                .bind(conn_model.target_node)
                .bind(&conn_model.target_input)
                .bind(&conn_model.condition_type)
                .bind(&conn_model.condition_expression)
                .bind(&conn_model.transform)
                .bind(&conn_model.metadata)
                .bind(conn_model.created_at)
                .bind(conn_model.updated_at)
                .execute(&mut *tx)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
            }
            
            // Remove existing workflow tags
            sqlx::query("DELETE FROM workflow_tags WHERE workflow_id = $1")
                .bind(workflow_model.id)
                .execute(&mut *tx)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
                
            // Insert tags
            for tag in &entity.tags {
                let tag_model = TagModel::from_entity(tag)
                    .map_err(|e| CoreError::from(e))?;
                    
                // Check if the tag exists
                let tag_exists = sqlx::query("SELECT 1 FROM tags WHERE id = $1")
                    .bind(tag_model.id)
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(|e| CoreError::from(PersistenceError::from(e)))?
                    .is_some();
                    
                // Insert the tag if it doesn't exist
                if !tag_exists {
                    sqlx::query(
                        "INSERT INTO tags 
                         (id, name, color, description, created_by, system_tag, 
                         created_at, updated_at)
                         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"
                    )
                    .bind(tag_model.id)
                    .bind(&tag_model.name)
                    .bind(&tag_model.color)
                    .bind(&tag_model.description)
                    .bind(tag_model.created_by)
                    .bind(tag_model.system_tag)
                    .bind(tag_model.created_at)
                    .bind(tag_model.updated_at)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
                }
                
                // Insert the workflow-tag link
                sqlx::query(
                    "INSERT INTO workflow_tags (workflow_id, tag_id)
                     VALUES ($1, $2)"
                )
                .bind(workflow_model.id)
                .bind(tag_model.id)
                .execute(&mut *tx)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
            }
            
            // Commit the transaction
            tx.commit()
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
                
            // Return the saved workflow
            PostgresWorkflowRepository::new(connection_manager)
                .load_workflow_with_relations(workflow_model.id)
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
                
            // Check if the workflow exists
            let exists = sqlx::query("SELECT 1 FROM workflows WHERE id = $1")
                .bind(uuid)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?
                .is_some();
                
            if !exists {
                return Err(CoreError::NotFound(format!("Workflow with id {} not found", id)));
            }
            
            // Delete in proper order to respect foreign key constraints
            
            // Delete workflow-tag associations
            sqlx::query("DELETE FROM workflow_tags WHERE workflow_id = $1")
                .bind(uuid)
                .execute(&mut *tx)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
                
            // Delete connections
            sqlx::query("DELETE FROM connections WHERE workflow_id = $1")
                .bind(uuid)
                .execute(&mut *tx)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
                
            // Delete node credentials
            sqlx::query("DELETE FROM node_credentials WHERE node_id IN (SELECT id FROM nodes WHERE workflow_id = $1)")
                .bind(uuid)
                .execute(&mut *tx)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
                
            // Delete node ports
            sqlx::query("DELETE FROM node_ports WHERE node_id IN (SELECT id FROM nodes WHERE workflow_id = $1)")
                .bind(uuid)
                .execute(&mut *tx)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
                
            // Delete nodes
            sqlx::query("DELETE FROM nodes WHERE workflow_id = $1")
                .bind(uuid)
                .execute(&mut *tx)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
                
            // Delete workflow variables
            sqlx::query("DELETE FROM variables WHERE scope_type = 'workflow' AND scope_id = $1")
                .bind(uuid)
                .execute(&mut *tx)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
                
            // Finally delete the workflow
            sqlx::query("DELETE FROM workflows WHERE id = $1")
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
                
            let exists = sqlx::query("SELECT 1 FROM workflows WHERE id = $1")
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
impl WorkflowRepository for PostgresWorkflowRepository {
    fn find_by_filter(&self, filter: WorkflowFilter) -> AsyncResult<Vec<Workflow>> {
        let connection_manager = self.connection_manager.clone();
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            // Build the query
            let mut query = "SELECT DISTINCT w.* FROM workflows w".to_string();
            let mut conditions = Vec::new();
            let mut params: Vec<String> = Vec::new();
            let mut param_count = 1;
            
            // Add name filter
            if let Some(name) = filter.name {
                conditions.push(format!("w.name ILIKE ${}", param_count));
                params.push(format!("%{}%", name));
                param_count += 1;
            }
            
            // Add active filter
            if let Some(active) = filter.active {
                conditions.push(format!("w.active = ${}", param_count));
                params.push(active.to_string());
                param_count += 1;
            }
            
            // Add created_by filter
            if let Some(created_by) = filter.created_by {
                conditions.push(format!("w.created_by = ${}", param_count));
                params.push(Uuid::from_bytes(created_by.as_bytes()).to_string());
                param_count += 1;
            }
            
            // Add tags filter
            if let Some(tags) = filter.tags {
                if !tags.is_empty() {
                    query.push_str(" LEFT JOIN workflow_tags wt ON w.id = wt.workflow_id");
                    let tag_params: Vec<String> = tags.iter()
                        .map(|tag_id| Uuid::from_bytes(tag_id.as_bytes()).to_string())
                        .collect();
                        
                    let tag_placeholders: Vec<String> = (0..tag_params.len())
                        .map(|i| format!("${}", param_count + i))
                        .collect();
                        
                    conditions.push(format!("wt.tag_id IN ({})", tag_placeholders.join(", ")));
                    params.extend(tag_params);
                    param_count += tags.len();
                    
                    // Ensure all tags are matched if multiple are provided
                    if tags.len() > 1 {
                        conditions.push(format!("(SELECT COUNT(DISTINCT tag_id) FROM workflow_tags WHERE workflow_id = w.id AND tag_id IN ({})) = {}", 
                            tag_placeholders.join(", "), tags.len()));
                    }
                }
            }
            
            // Add WHERE clause if there are conditions
            if !conditions.is_empty() {
                query.push_str(" WHERE ");
                query.push_str(&conditions.join(" AND "));
            }
            
            // Add ORDER BY clause
            if let Some(sort_by) = filter.sort_by {
                let sort_field = match sort_by {
                    WorkflowSortField::Name => "w.name",
                    WorkflowSortField::CreatedAt => "w.created_at",
                    WorkflowSortField::UpdatedAt => "w.updated_at",
                };
                
                let direction = if filter.ascending { "ASC" } else { "DESC" };
                query.push_str(&format!(" ORDER BY {} {}", sort_field, direction));
            } else {
                // Default sorting
                query.push_str(" ORDER BY w.updated_at DESC");
            }
            
            // Add LIMIT and OFFSET
            if let Some(limit) = filter.limit {
                query.push_str(&format!(" LIMIT {}", limit));
            }
            
            if let Some(offset) = filter.offset {
                query.push_str(&format!(" OFFSET {}", offset));
            }
            
            // Execute the query
            let mut q = sqlx::query_as::<_, WorkflowModel>(&query);
            
            // Bind parameters
            for param in params {
                q = q.bind(param);
            }
            
            let workflow_models = q.fetch_all(&conn)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
                
            // Load full workflows with relations
            let mut workflows = Vec::new();
            for model in workflow_models {
                let workflow = PostgresWorkflowRepository::new(connection_manager.clone())
                    .load_workflow_with_relations(model.id)
                    .await
                    .map_err(|e| CoreError::from(e))?;
                    
                workflows.push(workflow);
            }
            
            Ok(workflows)
        })
    }
    
    fn count_by_filter(&self, filter: WorkflowFilter) -> AsyncResult<usize> {
        let connection_manager = self.connection_manager.clone();
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            // Build the query
            let mut query = "SELECT COUNT(DISTINCT w.id) FROM workflows w".to_string();
            let mut conditions = Vec::new();
            let mut params: Vec<String> = Vec::new();
            let mut param_count = 1;
            
            // Add name filter
            if let Some(name) = filter.name {
                conditions.push(format!("w.name ILIKE ${}", param_count));
                params.push(format!("%{}%", name));
                param_count += 1;
            }
            
            // Add active filter
            if let Some(active) = filter.active {
                conditions.push(format!("w.active = ${}", param_count));
                params.push(active.to_string());
                param_count += 1;
            }
            
            // Add created_by filter
            if let Some(created_by) = filter.created_by {
                conditions.push(format!("w.created_by = ${}", param_count));
                params.push(Uuid::from_bytes(created_by.as_bytes()).to_string());
                param_count += 1;
            }
            
            // Add tags filter
            if let Some(tags) = filter.tags {
                if !tags.is_empty() {
                    query.push_str(" LEFT JOIN workflow_tags wt ON w.id = wt.workflow_id");
                    let tag_params: Vec<String> = tags.iter()
                        .map(|tag_id| Uuid::from_bytes(tag_id.as_bytes()).to_string())
                        .collect();
                        
                    let tag_placeholders: Vec<String> = (0..tag_params.len())
                        .map(|i| format!("${}", param_count + i))
                        .collect();
                        
                    conditions.push(format!("wt.tag_id IN ({})", tag_placeholders.join(", ")));
                    params.extend(tag_params);
                    param_count += tags.len();
                    
                    // Ensure all tags are matched if multiple are provided
                    if tags.len() > 1 {
                        conditions.push(format!("(SELECT COUNT(DISTINCT tag_id) FROM workflow_tags WHERE workflow_id = w.id AND tag_id IN ({})) = {}", 
                            tag_placeholders.join(", "), tags.len()));
                    }
                }
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
    
    fn find_by_tag(&self, tag_id: EntityId) -> AsyncResult<Vec<Workflow>> {
        let connection_manager = self.connection_manager.clone();
        let uuid = Uuid::from_bytes(tag_id.as_bytes());
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            let workflow_models = sqlx::query_as::<_, WorkflowModel>(
                "SELECT w.* FROM workflows w
                 JOIN workflow_tags wt ON w.id = wt.workflow_id
                 WHERE wt.tag_id = $1
                 ORDER BY w.updated_at DESC"
            )
            .bind(uuid)
            .fetch_all(&conn)
            .await
            .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
            
            // Load full workflows with relations
            let mut workflows = Vec::new();
            for model in workflow_models {
                let workflow = PostgresWorkflowRepository::new(connection_manager.clone())
                    .load_workflow_with_relations(model.id)
                    .await
                    .map_err(|e| CoreError::from(e))?;
                    
                workflows.push(workflow);
            }
            
            Ok(workflows)
        })
    }
    
    fn find_by_creator(&self, user_id: EntityId) -> AsyncResult<Vec<Workflow>> {
        let connection_manager = self.connection_manager.clone();
        let uuid = Uuid::from_bytes(user_id.as_bytes());
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            let workflow_models = sqlx::query_as::<_, WorkflowModel>(
                "SELECT * FROM workflows
                 WHERE created_by = $1
                 ORDER BY updated_at DESC"
            )
            .bind(uuid)
            .fetch_all(&conn)
            .await
            .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
            
            // Load full workflows with relations
            let mut workflows = Vec::new();
            for model in workflow_models {
                let workflow = PostgresWorkflowRepository::new(connection_manager.clone())
                    .load_workflow_with_relations(model.id)
                    .await
                    .map_err(|e| CoreError::from(e))?;
                    
                workflows.push(workflow);
            }
            
            Ok(workflows)
        })
    }
    
    fn find_version(&self, id: EntityId, version: u32) -> AsyncResult<Option<Workflow>> {
        let connection_manager = self.connection_manager.clone();
        let uuid = Uuid::from_bytes(id.as_bytes());
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            // Check if the workflow version exists
            let version_exists = sqlx::query(
                "SELECT 1 FROM workflow_versions
                 WHERE workflow_id = $1 AND version = $2"
            )
            .bind(uuid)
            .bind(version as i32)
            .fetch_optional(&conn)
            .await
            .map_err(|e| CoreError::from(PersistenceError::from(e)))?
            .is_some();
            
            if !version_exists {
                return Ok(None);
            }
            
            // Load the workflow version
            let workflow_version = sqlx::query(
                "SELECT data FROM workflow_versions
                 WHERE workflow_id = $1 AND version = $2"
            )
            .bind(uuid)
            .bind(version as i32)
            .fetch_one(&conn)
            .await
            .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
            
            let workflow_data: serde_json::Value = workflow_version.get("data");
            
            // Deserialize the workflow data
            let workflow = serde_json::from_value::<Workflow>(workflow_data)
                .map_err(|e| CoreError::from(PersistenceError::Serialization(e.to_string())))?;
                
            Ok(Some(workflow))
        })
    }
    
    fn list_versions(&self, id: EntityId) -> AsyncResult<Vec<u32>> {
        let connection_manager = self.connection_manager.clone();
        let uuid = Uuid::from_bytes(id.as_bytes());
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            // Fetch all versions for the workflow
            let versions: Vec<i32> = sqlx::query(
                "SELECT version FROM workflow_versions
                 WHERE workflow_id = $1
                 ORDER BY version DESC"
            )
            .bind(uuid)
            .fetch_all(&conn)
            .await
            .map_err(|e| CoreError::from(PersistenceError::from(e)))?
            .into_iter()
            .map(|row: PgRow| row.get("version"))
            .collect();
            
            // Convert to u32
            let versions = versions.into_iter()
                .map(|v| v as u32)
                .collect();
                
            Ok(versions)
        })
    }
    
    fn add_tag(&self, workflow_id: EntityId, tag: &Tag) -> AsyncResult<()> {
        let connection_manager = self.connection_manager.clone();
        let workflow_uuid = Uuid::from_bytes(workflow_id.as_bytes());
        let tag_uuid = Uuid::from_bytes(tag.id.as_bytes());
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            // Start a transaction
            let mut tx = conn.begin().await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
                
            // Convert the tag to a model
            let tag_model = TagModel::from_entity(tag)
                .map_err(|e| CoreError::from(e))?;
                
            // Check if the tag exists
            let tag_exists = sqlx::query("SELECT 1 FROM tags WHERE id = $1")
                .bind(tag_uuid)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?
                .is_some();
                
            // Insert the tag if it doesn't exist
            if !tag_exists {
                sqlx::query(
                    "INSERT INTO tags 
                     (id, name, color, description, created_by, system_tag, 
                     created_at, updated_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"
                )
                .bind(tag_model.id)
                .bind(&tag_model.name)
                .bind(&tag_model.color)
                .bind(&tag_model.description)
                .bind(tag_model.created_by)
                .bind(tag_model.system_tag)
                .bind(tag_model.created_at)
                .bind(tag_model.updated_at)
                .execute(&mut *tx)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
            }
            
            // Check if the tag is already linked to the workflow
            let tag_linked = sqlx::query(
                "SELECT 1 FROM workflow_tags 
                 WHERE workflow_id = $1 AND tag_id = $2"
            )
            .bind(workflow_uuid)
            .bind(tag_uuid)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| CoreError::from(PersistenceError::from(e)))?
            .is_some();
            
            // Insert the workflow-tag link if it doesn't exist
            if !tag_linked {
                sqlx::query(
                    "INSERT INTO workflow_tags (workflow_id, tag_id)
                     VALUES ($1, $2)"
                )
                .bind(workflow_uuid)
                .bind(tag_uuid)
                .execute(&mut *tx)
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
            }
            
            // Commit the transaction
            tx.commit()
                .await
                .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
                
            Ok(())
        })
    }
    
    fn remove_tag(&self, workflow_id: EntityId, tag_id: EntityId) -> AsyncResult<()> {
        let connection_manager = self.connection_manager.clone();
        let workflow_uuid = Uuid::from_bytes(workflow_id.as_bytes());
        let tag_uuid = Uuid::from_bytes(tag_id.as_bytes());
        
        Box::pin(async move {
            let conn = connection_manager.get_connection().await
                .map_err(|e| CoreError::from(e))?;
                
            // Remove the workflow-tag link
            sqlx::query(
                "DELETE FROM workflow_tags 
                 WHERE workflow_id = $1 AND tag_id = $2"
            )
            .bind(workflow_uuid)
            .bind(tag_uuid)
            .execute(&conn)
            .await
            .map_err(|e| CoreError::from(PersistenceError::from(e)))?;
            
            Ok(())
        })
    }
}