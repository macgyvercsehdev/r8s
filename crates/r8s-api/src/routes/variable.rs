// crates/r8s-api/src/routes/variable.rs
//! Variable API routes.

use std::sync::Arc;
use axum::{
    extract::{Path, Extension, Query, Json},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use validator::Validate;
use utoipa::ToSchema;

use r8s_core::common::EntityId;
use r8s_core::entity::{Variable, VariableType, VariableScope};
use r8s_core::repository::VariableRepository;
use r8s_core::use_case::variable::{
    CreateVariableUseCase,
    CreateVariableInput,
    UpdateVariableValueUseCase,
    UpdateVariableValueInput,
    DeleteVariableUseCase,
};

use crate::error::{ApiError, Result};
use crate::server::ApiState;

/// Variable response DTO.
#[derive(Serialize, ToSchema)]
pub struct VariableResponse {
    /// Variable ID
    pub id: String,
    
    /// Variable key
    pub key: String,
    
    /// Variable value (masked if protected)
    pub value: String,
    
    /// Variable type
    pub var_type: String,
    
    /// Variable scope
    pub scope: VariableScopeResponse,
    
    /// Whether the variable is protected
    pub protected: bool,
    
    /// Variable description
    pub description: Option<String>,
    
    /// Creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
    
    /// Last update timestamp
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Variable scope response DTO.
#[derive(Serialize, ToSchema)]
pub struct VariableScopeResponse {
    /// Scope type
    pub scope_type: String,
    
    /// Scope ID or name
    pub scope_id: Option<String>,
}

/// Create variable request DTO.
#[derive(Deserialize, Validate, ToSchema)]
pub struct CreateVariableRequest {
    /// Variable key
    #[validate(length(min = 1, max = 255))]
    pub key: String,
    
    /// Variable value
    pub value: String,
    
    /// Variable type
    pub var_type: String,
    
    /// Variable scope type
    pub scope_type: String,
    
    /// Variable scope ID
    pub scope_id: Option<String>,
    
    /// Whether the variable is protected
    pub protected: Option<bool>,
    
    /// Variable description
    #[validate(length(max = 1000))]
    pub description: Option<String>,
}

/// Update variable request DTO.
#[derive(Deserialize, Validate, ToSchema)]
pub struct UpdateVariableRequest {
    /// Variable value
    pub value: String,
    
    /// Whether the variable is protected
    pub protected: Option<bool>,
    
    /// Variable description
    #[validate(length(max = 1000))]
    pub description: Option<String>,
}

/// Query parameters for variable listing.
#[derive(Deserialize)]
pub struct VariableQuery {
    /// Filter by scope type
    pub scope_type: Option<String>,
    
    /// Filter by scope ID
    pub scope_id: Option<String>,
}

impl From<Variable> for VariableResponse {
    fn from(variable: Variable) -> Self {
        // Mask value if protected
        let value = if variable.protected {
            "*****".to_string()
        } else {
            variable.value
        };
        
        // Convert scope to response DTO
        let scope = match &variable.scope {
            VariableScope::Global => VariableScopeResponse {
                scope_type: "global".to_string(),
                scope_id: None,
            },
            VariableScope::Workflow(id) => VariableScopeResponse {
                scope_type: "workflow".to_string(),
                scope_id: Some(id.to_string()),
            },
            VariableScope::User(id) => VariableScopeResponse {
                scope_type: "user".to_string(),
                scope_id: Some(id.to_string()),
            },
            VariableScope::Environment(env) => VariableScopeResponse {
                scope_type: "environment".to_string(),
                scope_id: Some(env.clone()),
            },
        };
        
        Self {
            id: variable.id.to_string(),
            key: variable.key,
            value,
            var_type: variable.var_type.to_string().to_lowercase(),
            scope,
            protected: variable.protected,
            description: variable.description,
            created_at: variable.timestamps.created_at,
            updated_at: variable.timestamps.updated_at,
        }
    }
}

/// Get all variables with optional filtering.
#[utoipa::path(
    get,
    path = "/api/variables",
    tag = "Variables",
    params(
        ("scope_type" = Option<String>, Query, description = "Filter by scope type"),
        ("scope_id" = Option<String>, Query, description = "Filter by scope ID")
    ),
    responses(
        (status = 200, description = "List of variables", body = Vec<VariableResponse>),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_variables<W, E, C, U, V>(
    Extension(state): Extension<Arc<ApiState<W, E, C, U, V>>>,
    Query(query): Query<VariableQuery>,
) -> Result<Json<Vec<VariableResponse>>>
where
    W: r8s_core::repository::WorkflowRepository,
    E: r8s_core::repository::ExecutionRepository,
    C: r8s_core::repository::CredentialRepository,
    U: r8s_core::repository::UserRepository,
    V: VariableRepository,
{
    let variables = match (query.scope_type.as_deref(), query.scope_id.as_ref()) {
        // Global variables
        (Some("global"), None) => state.variable_repository.find_global_variables().await?,
        
        // Workflow variables
        (Some("workflow"), Some(id)) => {
            let workflow_id = EntityId::parse_str(id)
                .map_err(|_| ApiError::BadRequest(format!("Invalid workflow ID: {}", id)))?;
            state.variable_repository.find_by_workflow(workflow_id).await?
        },
        
        // User variables
        (Some("user"), Some(id)) => {
            let user_id = EntityId::parse_str(id)
                .map_err(|_| ApiError::BadRequest(format!("Invalid user ID: {}", id)))?;
            state.variable_repository.find_by_user(user_id).await?
        },
        
        // Environment variables
        (Some("environment"), Some(env)) => {
            state.variable_repository.find_by_environment(env).await?
        },
        
        // Invalid combination
        (Some(_), None) => {
            return Err(ApiError::BadRequest("Scope ID is required for the specified scope type".to_string()));
        },
        
        // Default: fetch all variables accessible to the user
        _ => {
            // Get global variables
            let mut all_vars = state.variable_repository.find_global_variables().await?;
            
            // TODO: Get user ID from auth context
            let user_id = EntityId::nil(); // Placeholder
            
            // Add user variables
            let user_vars = state.variable_repository.find_by_user(user_id).await?;
            all_vars.extend(user_vars);
            
            all_vars
        }
    };
    
    // Convert to response DTOs
    let response = variables.into_iter().map(VariableResponse::from).collect();
    
    Ok(Json(response))
}

/// Get a specific variable by ID.
#[utoipa::path(
    get,
    path = "/api/variables/{id}",
    tag = "Variables",
    params(
        ("id" = String, Path, description = "Variable ID")
    ),
    responses(
        (status = 200, description = "Variable details", body = VariableResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Variable not found"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_variable<W, E, C, U, V>(
    Extension(state): Extension<Arc<ApiState<W, E, C, U, V>>>,
    Path(id): Path<String>,
) -> Result<Json<VariableResponse>>
where
    W: r8s_core::repository::WorkflowRepository,
    E: r8s_core::repository::ExecutionRepository,
    C: r8s_core::repository::CredentialRepository,
    U: r8s_core::repository::UserRepository,
    V: VariableRepository,
{
    // Parse variable ID
    let variable_id = EntityId::parse_str(&id)
        .map_err(|_| ApiError::BadRequest(format!("Invalid variable ID: {}", id)))?;
    
    // TODO: Get user ID from auth context
    let user_id = EntityId::nil(); // Placeholder
    
    // Check if user has access to this variable
    let has_access = state.variable_repository.has_access(variable_id, user_id).await?;
    
    if !has_access {
        return Err(ApiError::Authorization("You don't have access to this variable".to_string()));
    }
    
    // Fetch variable
    let variable = state.variable_repository.find_by_id(variable_id).await?;
    
    // Convert to response DTO
    let response = VariableResponse::from(variable);
    
    Ok(Json(response))
}

/// Create a new variable.
#[utoipa::path(
    post,
    path = "/api/variables",
    tag = "Variables",
    request_body = CreateVariableRequest,
    responses(
        (status = 201, description = "Variable created", body = VariableResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 409, description = "Variable already exists"),
        (status = 422, description = "Validation error"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn create_variable<W, E, C, U, V>(
    Extension(state): Extension<Arc<ApiState<W, E, C, U, V>>>,
    Json(request): Json<CreateVariableRequest>,
) -> Result<(StatusCode, Json<VariableResponse>)>
where
    W: r8s_core::repository::WorkflowRepository,
    E: r8s_core::repository::ExecutionRepository,
    C: r8s_core::repository::CredentialRepository,
    U: r8s_core::repository::UserRepository,
    V: VariableRepository,
{
    // Validate the request
    request.validate()
        .map_err(|e| ApiError::Validation(e.to_string()))?;
    
    // Parse variable type
    let var_type = match request.var_type.to_lowercase().as_str() {
        "string" => VariableType::String,
        "number" => VariableType::Number,
        "boolean" => VariableType::Boolean,
        "json" => VariableType::Json,
        "binary" => VariableType::Binary,
        _ => return Err(ApiError::BadRequest(format!("Invalid variable type: {}", request.var_type))),
    };
    
    // Parse scope
    let scope = match request.scope_type.to_lowercase().as_str() {
        "global" => VariableScope::Global,
        
        "workflow" => {
            let id = request.scope_id
                .as_ref()
                .ok_or_else(|| ApiError::BadRequest("Scope ID is required for workflow scope".to_string()))?;
                
            let workflow_id = EntityId::parse_str(id)
                .map_err(|_| ApiError::BadRequest(format!("Invalid workflow ID: {}", id)))?;
                
            VariableScope::Workflow(workflow_id)
        },
        
        "user" => {
            let id = request.scope_id
                .as_ref()
                .ok_or_else(|| ApiError::BadRequest("Scope ID is required for user scope".to_string()))?;
                
            let user_id = EntityId::parse_str(id)
                .map_err(|_| ApiError::BadRequest(format!("Invalid user ID: {}", id)))?;
                
            VariableScope::User(user_id)
        },
        
        "environment" => {
            let env = request.scope_id
                .as_ref()
                .ok_or_else(|| ApiError::BadRequest("Scope ID (environment name) is required for environment scope".to_string()))?;
                
            VariableScope::Environment(env.clone())
        },
        
        _ => return Err(ApiError::BadRequest(format!("Invalid scope type: {}", request.scope_type))),
    };
    
    // TODO: Get user ID from auth context
    let user_id = EntityId::nil(); // Placeholder
    
    // Create input for the use case
    let input = CreateVariableInput {
        key: request.key,
        value: request.value,
        var_type,
        scope,
        protected: request.protected.unwrap_or(false),
        description: request.description,
        user_id,
    };
    
    // Create and execute the use case
    let use_case = CreateVariableUseCase::new(
        state.variable_repository.clone(),
        state.user_repository.clone(),
    );
    
    let variable = use_case.execute(input).await?;
    
    // Convert to response DTO
    let response = VariableResponse::from(variable);
    
    Ok((StatusCode::CREATED, Json(response)))
}

/// Update an existing variable.
#[utoipa::path(
    put,
    path = "/api/variables/{id}",
    tag = "Variables",
    params(
        ("id" = String, Path, description = "Variable ID")
    ),
    request_body = UpdateVariableRequest,
    responses(
        (status = 200, description = "Variable updated", body = VariableResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Variable not found"),
        (status = 422, description = "Validation error"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn update_variable<W, E, C, U, V>(
    Extension(state): Extension<Arc<ApiState<W, E, C, U, V>>>,
    Path(id): Path<String>,
    Json(request): Json<UpdateVariableRequest>,
) -> Result<Json<VariableResponse>>
where
    W: r8s_core::repository::WorkflowRepository,
    E: r8s_core::repository::ExecutionRepository,
    C: r8s_core::repository::CredentialRepository,
    U: r8s_core::repository::UserRepository,
    V: VariableRepository,
{
    // Validate the request
    request.validate()
        .map_err(|e| ApiError::Validation(e.to_string()))?;
    
    // Parse variable ID
    let variable_id = EntityId::parse_str(&id)
        .map_err(|_| ApiError::BadRequest(format!("Invalid variable ID: {}", id)))?;
    
    // TODO: Get user ID from auth context
    let user_id = EntityId::nil(); // Placeholder
    
    // Update the variable value
    let input = UpdateVariableValueInput {
        variable_id,
        value: request.value,
        user_id,
    };
    
    let use_case = UpdateVariableValueUseCase::new(
        state.variable_repository.clone(),
        state.user_repository.clone(),
    );
    
    let variable = use_case.execute(input).await?;
    
    // Update protected status if provided
    if let Some(protected) = request.protected {
        state.variable_repository.set_protected(variable_id, protected).await?;
    }
    
    // Fetch the updated variable
    let updated_variable = state.variable_repository.find_by_id(variable_id).await?;
    
    // Convert to response DTO
    let response = VariableResponse::from(updated_variable);
    
    Ok(Json(response))
}

/// Delete a variable.
#[utoipa::path(
    delete,
    path = "/api/variables/{id}",
    tag = "Variables",
    params(
        ("id" = String, Path, description = "Variable ID")
    ),
    responses(
        (status = 204, description = "Variable deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Variable not found"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn delete_variable<W, E, C, U, V>(
    Extension(state): Extension<Arc<ApiState<W, E, C, U, V>>>,
    Path(id): Path<String>,
) -> Result<StatusCode>
where
    W: r8s_core::repository::WorkflowRepository,
    E: r8s_core::repository::ExecutionRepository,
    C: r8s_core::repository::CredentialRepository,
    U: r8s_core::repository::UserRepository,
    V: VariableRepository,
{
    // Parse variable ID
    let variable_id = EntityId::parse_str(&id)
        .map_err(|_| ApiError::BadRequest(format!("Invalid variable ID: {}", id)))?;
    
    // TODO: Get user ID from auth context
    let user_id = EntityId::nil(); // Placeholder
    
    // Create and execute the use case
    let use_case = DeleteVariableUseCase::new(
        state.variable_repository.clone(),
        state.user_repository.clone(),
    );
    
    use_case.execute(variable_id, user_id).await?;
    
    Ok(StatusCode::NO_CONTENT)
}