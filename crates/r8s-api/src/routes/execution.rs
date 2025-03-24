// crates/r8s-api/src/routes/execution.rs
//! Execution API routes.

use std::sync::Arc;
use axum::{
    extract::{Path, Extension, Query, Json},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;
use utoipa::ToSchema;

use r8s_core::common::{EntityId, ExecutionState, Priority};
use r8s_core::entity::{Execution, ExecutionLog, NodeExecutionResult};
use r8s_core::repository::{ExecutionRepository, ExecutionFilter};
use r8s_core::use_case::execution::{StartWorkflowUseCase, StartWorkflowInput, CancelExecutionUseCase};

use crate::error::{ApiError, Result};
use crate::server::ApiState;

/// Execution response DTO.
#[derive(Serialize, Deserialize, ToSchema)]
pub struct ExecutionResponse {
    /// Execution ID
    pub id: String,
    
    /// Workflow ID
    pub workflow_id: String,
    
    /// Workflow version
    pub workflow_version: u32,
    
    /// Execution state
    pub state: String,
    
    /// Start time
    pub started_at: chrono::DateTime<chrono::Utc>,
    
    /// End time (if completed)
    pub finished_at: Option<chrono::DateTime<chrono::Utc>>,
    
    /// Duration in milliseconds (if completed)
    pub duration_ms: Option<u64>,
    
    /// User ID that initiated the execution
    pub initiated_by: Option<String>,
    
    /// Execution results by node
    pub results: Vec<NodeResultResponse>,
    
    /// Execution logs
    pub logs: Vec<LogResponse>,
    
    /// Error message (if failed)
    pub error: Option<String>,
    
    /// Failed node ID (if failed)
    pub failed_node_id: Option<String>,
    
    /// Execution tags
    pub tags: Vec<String>,
}

/// Node execution result DTO.
#[derive(Serialize, Deserialize, ToSchema)]
pub struct NodeResultResponse {
    /// Node ID
    pub node_id: String,
    
    /// Execution state
    pub state: String,
    
    /// Start time
    pub started_at: chrono::DateTime<chrono::Utc>,
    
    /// End time (if completed)
    pub finished_at: Option<chrono::DateTime<chrono::Utc>>,
    
    /// Duration in milliseconds (if completed)
    pub duration_ms: Option<u64>,
    
    /// Output data
    pub output: serde_json::Value,
    
    /// Error message (if failed)
    pub error: Option<String>,
}

/// Execution log DTO.
#[derive(Serialize, Deserialize, ToSchema)]
pub struct LogResponse {
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    
    /// Log level
    pub level: String,
    
    /// Log message
    pub message: String,
    
    /// Node ID (if applicable)
    pub node_id: Option<String>,
}

/// Request to start a workflow execution.
#[derive(Deserialize, Validate, ToSchema)]
pub struct StartWorkflowRequest {
    /// Initial input data for the workflow
    pub initial_data: Option<serde_json::Value>,
    
    /// Execution priority
    pub priority: Option<String>,
    
    /// Execution tags
    pub tags: Option<Vec<String>>,
}

/// Query parameters for execution listing.
#[derive(Deserialize)]
pub struct ExecutionQuery {
    /// Filter by workflow ID
    pub workflow_id: Option<String>,
    
    /// Filter by state
    pub state: Option<String>,
    
    /// Filter by initiator user ID
    pub initiated_by: Option<String>,
    
    /// Filter by failed only
    pub failed_only: Option<bool>,
    
    /// Pagination limit
    pub limit: Option<usize>,
    
    /// Pagination offset
    pub offset: Option<usize>,
}

impl From<Execution> for ExecutionResponse {
    fn from(execution: Execution) -> Self {
        Self {
            id: execution.id.to_string(),
            workflow_id: execution.workflow_id.to_string(),
            workflow_version: execution.workflow_version,
            state: execution.state.to_string(),
            started_at: execution.started_at,
            finished_at: execution.finished_at,
            duration_ms: execution.duration_ms,
            initiated_by: execution.initiated_by.map(|id| id.to_string()),
            results: execution.results.into_iter()
                .map(|(_, result)| NodeResultResponse::from(result))
                .collect(),
            logs: execution.logs.into_iter().map(LogResponse::from).collect(),
            error: execution.failure_reason,
            failed_node_id: execution.failed_node_id.map(|id| id.to_string()),
            tags: execution.tags,
        }
    }
}

impl From<NodeExecutionResult> for NodeResultResponse {
    fn from(result: NodeExecutionResult) -> Self {
        Self {
            node_id: result.node_id.to_string(),
            state: result.state.to_string(),
            started_at: result.started_at,
            finished_at: result.finished_at,
            duration_ms: result.duration_ms,
            output: serde_json::to_value(result.output_data).unwrap_or(serde_json::Value::Null),
            error: result.failure_reason,
        }
    }
}

impl From<ExecutionLog> for LogResponse {
    fn from(log: ExecutionLog) -> Self {
        Self {
            timestamp: log.timestamp,
            level: log.level.to_string().to_lowercase(),
            message: log.message,
            node_id: log.node_id.map(|id| id.to_string()),
        }
    }
}

/// Get all executions with optional filtering.
#[utoipa::path(
    get,
    path = "/api/executions",
    tag = "Executions",
    params(
        ("workflow_id" = Option<String>, Query, description = "Filter by workflow ID"),
        ("state" = Option<String>, Query, description = "Filter by state"),
        ("initiated_by" = Option<String>, Query, description = "Filter by initiator"),
        ("failed_only" = Option<bool>, Query, description = "Filter by failed only"),
        ("limit" = Option<usize>, Query, description = "Pagination limit"),
        ("offset" = Option<usize>, Query, description = "Pagination offset")
    ),
    responses(
        (status = 200, description = "List of executions", body = Vec<ExecutionResponse>),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_executions<W, E, C, U, V>(
    Extension(state): Extension<Arc<ApiState<W, E, C, U, V>>>,
    Query(query): Query<ExecutionQuery>,
) -> Result<Json<Vec<ExecutionResponse>>>
where
    W: r8s_core::repository::WorkflowRepository,
    E: ExecutionRepository,
    C: r8s_core::repository::CredentialRepository,
    U: r8s_core::repository::UserRepository,
    V: r8s_core::repository::VariableRepository,
{
    // Parse query parameters
    let workflow_id = query.workflow_id.as_ref().map(|id| {
        EntityId::parse_str(id).unwrap_or_else(|_| EntityId::nil())
    });
    
    let state = query.state.as_ref().and_then(|s| {
        match s.to_lowercase().as_str() {
            "pending" => Some(ExecutionState::Pending),
            "running" => Some(ExecutionState::Running),
            "success" => Some(ExecutionState::Success),
            "failed" => Some(ExecutionState::Failed),
            "cancelled" => Some(ExecutionState::Cancelled),
            "waiting" => Some(ExecutionState::Waiting),
            _ => None,
        }
    });
    
    let initiated_by = query.initiated_by.as_ref().map(|id| {
        EntityId::parse_str(id).unwrap_or_else(|_| EntityId::nil())
    });
    
    // Create filter
    let filter = ExecutionFilter {
        workflow_id,
        state,
        initiated_by,
        from_date: None,
        to_date: None,
        failed_only: query.failed_only.unwrap_or(false),
        sort_by: None,
        ascending: false,
        limit: query.limit,
        offset: query.offset,
    };
    
    // Fetch executions
    let executions = state.execution_repository.find_by_filter(filter).await?;
    
    // Convert to response DTOs
    let response = executions.into_iter().map(ExecutionResponse::from).collect();
    
    Ok(Json(response))
}

/// Get a specific execution by ID.
#[utoipa::path(
    get,
    path = "/api/executions/{id}",
    tag = "Executions",
    params(
        ("id" = String, Path, description = "Execution ID")
    ),
    responses(
        (status = 200, description = "Execution details", body = ExecutionResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Execution not found"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_execution<W, E, C, U, V>(
    Extension(state): Extension<Arc<ApiState<W, E, C, U, V>>>,
    Path(id): Path<String>,
) -> Result<Json<ExecutionResponse>>
where
    W: r8s_core::repository::WorkflowRepository,
    E: ExecutionRepository,
    C: r8s_core::repository::CredentialRepository,
    U: r8s_core::repository::UserRepository,
    V: r8s_core::repository::VariableRepository,
{
    // Parse execution ID
    let execution_id = EntityId::parse_str(&id)
        .map_err(|_| ApiError::BadRequest(format!("Invalid execution ID: {}", id)))?;
    
    // Fetch execution
    let execution = state.execution_repository.find_by_id(execution_id).await?;
    
    // Convert to response DTO
    let response = ExecutionResponse::from(execution);
    
    Ok(Json(response))
}

/// Start a workflow execution.
#[utoipa::path(
    post,
    path = "/api/workflows/{id}/execute",
    tag = "Executions",
    params(
        ("id" = String, Path, description = "Workflow ID")
    ),
    request_body = StartWorkflowRequest,
    responses(
        (status = 202, description = "Execution started", body = ExecutionResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Workflow not found"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn start_workflow<W, E, C, U, V>(
    Extension(state): Extension<Arc<ApiState<W, E, C, U, V>>>,
    Path(id): Path<String>,
    Json(request): Json<StartWorkflowRequest>,
) -> Result<(StatusCode, Json<ExecutionResponse>)>
where
    W: r8s_core::repository::WorkflowRepository,
    E: ExecutionRepository,
    C: r8s_core::repository::CredentialRepository,
    U: r8s_core::repository::UserRepository,
    V: r8s_core::repository::VariableRepository,
{
    // Parse workflow ID
    let workflow_id = EntityId::parse_str(&id)
        .map_err(|_| ApiError::BadRequest(format!("Invalid workflow ID: {}", id)))?;
    
    // Parse priority if provided
    let priority = request.priority.as_ref().map(|p| {
        match p.to_lowercase().as_str() {
            "high" => Priority::High,
            "low" => Priority::Low,
            _ => Priority::Normal,
        }
    });
    
    // Create input for the use case
    let input = StartWorkflowInput {
        workflow_id,
        initial_data: request.initial_data,
        initiated_by: None, // TODO: Get from auth context
        priority,
        tags: request.tags.unwrap_or_default(),
    };
    
    // Create and execute the use case
    let use_case = StartWorkflowUseCase::new(
        state.workflow_repository.clone(),
        state.execution_repository.clone(),
    );
    
    let execution = use_case.execute(input).await?;
    
    // Convert to response DTO
    let response = ExecutionResponse::from(execution);
    
    Ok((StatusCode::ACCEPTED, Json(response)))
}

/// Cancel an execution.
#[utoipa::path(
    post,
    path = "/api/executions/{id}/cancel",
    tag = "Executions",
    params(
        ("id" = String, Path, description = "Execution ID")
    ),
    responses(
        (status = 200, description = "Execution cancelled", body = ExecutionResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Execution not found"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn cancel_execution<W, E, C, U, V>(
    Extension(state): Extension<Arc<ApiState<W, E, C, U, V>>>,
    Path(id): Path<String>,
) -> Result<Json<ExecutionResponse>>
where
    W: r8s_core::repository::WorkflowRepository,
    E: ExecutionRepository,
    C: r8s_core::repository::CredentialRepository,
    U: r8s_core::repository::UserRepository,
    V: r8s_core::repository::VariableRepository,
{
    // Parse execution ID
    let execution_id = EntityId::parse_str(&id)
        .map_err(|_| ApiError::BadRequest(format!("Invalid execution ID: {}", id)))?;
    
    // Create and execute the use case
    let use_case = CancelExecutionUseCase::new(state.execution_repository.clone());
    
    let execution = use_case.execute(execution_id, Some("Cancelled via API".to_string())).await?;
    
    // Convert to response DTO
    let response = ExecutionResponse::from(execution);
    
    Ok(Json(response))
}