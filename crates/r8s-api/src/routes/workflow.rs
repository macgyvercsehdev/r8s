// crates/r8s-api/src/routes/workflow.rs
//! Workflow API routes.

use std::sync::Arc;
use axum::{
    extract::{Path, Extension, Query, Json},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;
use utoipa::ToSchema;

use r8s_core::common::EntityId;
use r8s_core::entity::{Workflow, Node, Connection, WorkflowSettings};
use r8s_core::repository::{WorkflowRepository, WorkflowFilter};

use crate::error::{ApiError, Result};
use crate::server::ApiState;

/// Workflow response DTO.
#[derive(Serialize, Deserialize, ToSchema)]
pub struct WorkflowResponse {
    /// Workflow ID
    pub id: String,
    
    /// Workflow name
    pub name: String,
    
    /// Workflow description
    pub description: Option<String>,
    
    /// Workflow version
    pub version: u32,
    
    /// Whether the workflow is active
    pub active: bool,
    
    /// Workflow nodes
    pub nodes: Vec<NodeResponse>,
    
    /// Workflow connections
    pub connections: Vec<ConnectionResponse>,
    
    /// Workflow settings
    pub settings: WorkflowSettings,
    
    /// Workflow tags
    pub tags: Vec<String>,
    
    /// Creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
    
    /// Last update timestamp
    pub updated_at: chrono::DateTime<chrono::Utc>,
    
    /// Creator user ID
    pub created_by: Option<String>,
}

/// Node response DTO.
#[derive(Serialize, Deserialize, ToSchema)]
pub struct NodeResponse {
    /// Node ID
    pub id: String,
    
    /// Node name
    pub name: String,
    
    /// Node type information
    pub type_info: r8s_core::entity::NodeType,
    
    /// Node position (x, y)
    pub position: r8s_core::entity::Position,
    
    /// Node parameters
    pub parameters: serde_json::Value,
    
    /// Node inputs
    pub inputs: Vec<r8s_core::entity::NodePort>,
    
    /// Node outputs
    pub outputs: Vec<r8s_core::entity::NodePort>,
    
    /// Whether the node is disabled
    pub disabled: bool,
}

/// Connection response DTO.
#[derive(Serialize, Deserialize, ToSchema)]
pub struct ConnectionResponse {
    /// Connection ID
    pub id: String,
    
    /// Source node ID
    pub source_node: String,
    
    /// Source output name
    pub source_output: String,
    
    /// Target node ID
    pub target_node: String,
    
    /// Target input name
    pub target_input: String,
}

/// Create workflow request DTO.
#[derive(Deserialize, Validate, ToSchema)]
pub struct CreateWorkflowRequest {
    /// Workflow name
    #[validate(length(min = 1, max = 255))]
    pub name: String,
    
    /// Workflow description
    #[validate(length(max = 1000))]
    pub description: Option<String>,
    
    /// Workflow tags
    pub tags: Option<Vec<String>>,
    
    /// Initial workflow settings
    pub settings: Option<WorkflowSettings>,
}

/// Update workflow request DTO.
#[derive(Deserialize, Validate, ToSchema)]
pub struct UpdateWorkflowRequest {
    /// Workflow name
    #[validate(length(min = 1, max = 255))]
    pub name: Option<String>,
    
    /// Workflow description
    #[validate(length(max = 1000))]
    pub description: Option<String>,
    
    /// Whether the workflow is active
    pub active: Option<bool>,
    
    /// Workflow settings
    pub settings: Option<WorkflowSettings>,
    
    /// Workflow tags to add
    pub tags_to_add: Option<Vec<String>>,
    
    /// Workflow tags to remove
    pub tags_to_remove: Option<Vec<String>>,
}

/// Query parameters for workflow listing.
#[derive(Deserialize)]
pub struct WorkflowQuery {
    /// Filter by name
    pub name: Option<String>,
    
    /// Filter by tag
    pub tag: Option<String>,
    
    /// Filter by active status
    pub active: Option<bool>,
    
    /// Pagination limit
    pub limit: Option<usize>,
    
    /// Pagination offset
    pub offset: Option<usize>,
}

impl From<Workflow> for WorkflowResponse {
    fn from(workflow: Workflow) -> Self {
        Self {
            id: workflow.id.to_string(),
            name: workflow.name,
            description: workflow.description,
            version: workflow.version,
            active: workflow.active,
            nodes: workflow.nodes.into_iter().map(NodeResponse::from).collect(),
            connections: workflow.connections.into_iter().map(ConnectionResponse::from).collect(),
            settings: workflow.settings,
            tags: workflow.tags.into_iter().map(|t| t.name).collect(),
            created_at: workflow.timestamps.created_at,
            updated_at: workflow.timestamps.updated_at,
            created_by: workflow.created_by.map(|id| id.to_string()),
        }
    }
}

impl From<Node> for NodeResponse {
    fn from(node: Node) -> Self {
        Self {
            id: node.id.to_string(),
            name: node.name,
            type_info: node.type_info,
            position: node.position,
            parameters: node.parameters,
            inputs: node.inputs,
            outputs: node.outputs,
            disabled: node.disabled,
        }
    }
}

impl From<Connection> for ConnectionResponse {
    fn from(connection: Connection) -> Self {
        Self {
            id: connection.id.to_string(),
            source_node: connection.source_node.to_string(),
            source_output: connection.source_output,
            target_node: connection.target_node.to_string(),
            target_input: connection.target_input,
        }
    }
}

/// Get all workflows with optional filtering.
#[utoipa::path(
    get,
    path = "/api/workflows",
    tag = "Workflows",
    params(
        ("name" = Option<String>, Query, description = "Filter by name"),
        ("tag" = Option<String>, Query, description = "Filter by tag"),
        ("active" = Option<bool>, Query, description = "Filter by active status"),
        ("limit" = Option<usize>, Query, description = "Pagination limit"),
        ("offset" = Option<usize>, Query, description = "Pagination offset")
    ),
    responses(
        (status = 200, description = "List of workflows", body = Vec<WorkflowResponse>),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_workflows<W, E, C, U, V>(
    Extension(state): Extension<Arc<ApiState<W, E, C, U, V>>>,
    Query(query): Query<WorkflowQuery>,
) -> Result<Json<Vec<WorkflowResponse>>>
where
    W: WorkflowRepository,
    E: r8s_core::repository::ExecutionRepository,
    C: r8s_core::repository::CredentialRepository,
    U: r8s_core::repository::UserRepository,
    V: r8s_core::repository::VariableRepository,
{
    // Create filter from query parameters
    let filter = WorkflowFilter {
        name: query.name,
        tags: query.tag.map(|tag| {
            vec![EntityId::parse_str(&tag).unwrap_or_else(|_| EntityId::nil())]
        }),
        active: query.active,
        created_by: None,
        sort_by: None,
        ascending: true,
        limit: query.limit,
        offset: query.offset,
    };
    
    // Fetch workflows
    let workflows = state.workflow_repository.find_by_filter(filter).await?;
    
    // Convert to response DTOs
    let response = workflows.into_iter().map(WorkflowResponse::from).collect();
    
    Ok(Json(response))
}

/// Get a specific workflow by ID.
#[utoipa::path(
    get,
    path = "/api/workflows/{id}",
    tag = "Workflows",
    params(
        ("id" = String, Path, description = "Workflow ID")
    ),
    responses(
        (status = 200, description = "Workflow details", body = WorkflowResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Workflow not found"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_workflow<W, E, C, U, V>(
    Extension(state): Extension<Arc<ApiState<W, E, C, U, V>>>,
    Path(id): Path<String>,
) -> Result<Json<WorkflowResponse>>
where
    W: WorkflowRepository,
    E: r8s_core::repository::ExecutionRepository,
    C: r8s_core::repository::CredentialRepository,
    U: r8s_core::repository::UserRepository,
    V: r8s_core::repository::VariableRepository,
{
    // Parse workflow ID
    let workflow_id = EntityId::parse_str(&id)
        .map_err(|_| ApiError::BadRequest(format!("Invalid workflow ID: {}", id)))?;
    
    // Fetch workflow
    let workflow = state.workflow_repository.find_by_id(workflow_id).await?;
    
    // Convert to response DTO
    let response = WorkflowResponse::from(workflow);
    
    Ok(Json(response))
}

/// Create a new workflow.
#[utoipa::path(
    post,
    path = "/api/workflows",
    tag = "Workflows",
    request_body = CreateWorkflowRequest,
    responses(
        (status = 201, description = "Workflow created", body = WorkflowResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 422, description = "Validation error"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn create_workflow<W, E, C, U, V>(
    Extension(state): Extension<Arc<ApiState<W, E, C, U, V>>>,
    Json(request): Json<CreateWorkflowRequest>,
) -> Result<(StatusCode, Json<WorkflowResponse>)>
where
    W: WorkflowRepository,
    E: r8s_core::repository::ExecutionRepository,
    C: r8s_core::repository::CredentialRepository,
    U: r8s_core::repository::UserRepository,
    V: r8s_core::repository::VariableRepository,
{
    // Validate the request
    request.validate()
        .map_err(|e| ApiError::Validation(e.to_string()))?;
    
    // Create workflow entity
    let mut workflow = Workflow::new(request.name, request.description);
    
    // Apply settings if provided
    if let Some(settings) = request.settings {
        workflow.settings = settings;
    }
    
    // Save the workflow
    let workflow = state.workflow_repository.save(workflow).await?;
    
    // Add tags if provided
    if let Some(tags) = request.tags {
        for tag_name in tags {
            let tag = r8s_core::entity::Tag::new(
                tag_name,
                r8s_core::entity::TagColor::default(),
                None,
                None,
            );
            state.workflow_repository.add_tag(workflow.id, &tag).await?;
        }
        
        // Reload the workflow to get the tags
        let workflow = state.workflow_repository.find_by_id(workflow.id).await?;
        
        // Convert to response DTO
        let response = WorkflowResponse::from(workflow);
        
        Ok((StatusCode::CREATED, Json(response)))
    } else {
        // Convert to response DTO
        let response = WorkflowResponse::from(workflow);
        
        Ok((StatusCode::CREATED, Json(response)))
    }
}

/// Update an existing workflow.
#[utoipa::path(
    put,
    path = "/api/workflows/{id}",
    tag = "Workflows",
    params(
        ("id" = String, Path, description = "Workflow ID")
    ),
    request_body = UpdateWorkflowRequest,
    responses(
        (status = 200, description = "Workflow updated", body = WorkflowResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Workflow not found"),
        (status = 422, description = "Validation error"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn update_workflow<W, E, C, U, V>(
    Extension(state): Extension<Arc<ApiState<W, E, C, U, V>>>,
    Path(id): Path<String>,
    Json(request): Json<UpdateWorkflowRequest>,
) -> Result<Json<WorkflowResponse>>
where
    W: WorkflowRepository,
    E: r8s_core::repository::ExecutionRepository,
    C: r8s_core::repository::CredentialRepository,
    U: r8s_core::repository::UserRepository,
    V: r8s_core::repository::VariableRepository,
{
    // Validate the request
    request.validate()
        .map_err(|e| ApiError::Validation(e.to_string()))?;
    
    // Parse workflow ID
    let workflow_id = EntityId::parse_str(&id)
        .map_err(|_| ApiError::BadRequest(format!("Invalid workflow ID: {}", id)))?;
    
    // Fetch current workflow
    let mut workflow = state.workflow_repository.find_by_id(workflow_id).await?;
    
    // Update fields if provided
    if let Some(name) = request.name {
        workflow.name = name;
    }
    
    if let Some(description) = request.description {
        workflow.description = Some(description);
    }
    
    if let Some(active) = request.active {
        workflow.active = active;
    }
    
    if let Some(settings) = request.settings {
        workflow.settings = settings;
    }
    
    // Increment version
    workflow.increment_version();
    
    // Save the updated workflow
    let workflow = state.workflow_repository.save(workflow).await?;
    
    // Add tags if provided
    if let Some(tags) = request.tags_to_add {
        for tag_name in tags {
            let tag = r8s_core::entity::Tag::new(
                tag_name,
                r8s_core::entity::TagColor::default(),
                None,
                None,
            );
            state.workflow_repository.add_tag(workflow.id, &tag).await?;
        }
    }
    
    // Remove tags if provided
    if let Some(tags) = request.tags_to_remove {
        for tag_name in tags {
            // Find the tag ID by name
            for tag in &workflow.tags {
                if tag.name == tag_name {
                    state.workflow_repository.remove_tag(workflow.id, tag.id).await?;
                    break;
                }
            }
        }
    }
    
    // Reload the workflow to get the updated state
    let updated_workflow = state.workflow_repository.find_by_id(workflow.id).await?;
    
    // Convert to response DTO
    let response = WorkflowResponse::from(updated_workflow);
    
    Ok(Json(response))
}

/// Delete a workflow.
#[utoipa::path(
    delete,
    path = "/api/workflows/{id}",
    tag = "Workflows",
    params(
        ("id" = String, Path, description = "Workflow ID")
    ),
    responses(
        (status = 204, description = "Workflow deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Workflow not found"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn delete_workflow<W, E, C, U, V>(
    Extension(state): Extension<Arc<ApiState<W, E, C, U, V>>>,
    Path(id): Path<String>,
) -> Result<StatusCode>
where
    W: WorkflowRepository,
    E: r8s_core::repository::ExecutionRepository,
    C: r8s_core::repository::CredentialRepository,
    U: r8s_core::repository::UserRepository,
    V: r8s_core::repository::VariableRepository,
{
    // Parse workflow ID
    let workflow_id = EntityId::parse_str(&id)
        .map_err(|_| ApiError::BadRequest(format!("Invalid workflow ID: {}", id)))?;
    
    // Delete the workflow
    state.workflow_repository.delete(workflow_id).await?;
    
    Ok(StatusCode::NO_CONTENT)
}