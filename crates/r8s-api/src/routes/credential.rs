// crates/r8s-api/src/routes/credential.rs
//! Credential API routes.

use std::sync::Arc;
use axum::{
    extract::{Path, Extension, Json},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use validator::Validate;
use utoipa::ToSchema;

use r8s_core::common::EntityId;
use r8s_core::entity::Credential;
use r8s_core::repository::CredentialRepository;
use r8s_core::use_case::credential::{
    CreateCredentialUseCase, 
    CreateCredentialInput,
    UpdateCredentialUseCase,
    UpdateCredentialInput,
    DeleteCredentialUseCase,
    GetCredentialUseCase,
    ListUserCredentialsUseCase,
};

use crate::error::{ApiError, Result};
use crate::server::ApiState;

/// Credential response DTO.
#[derive(Serialize, ToSchema)]
pub struct CredentialResponse {
    /// Credential ID
    pub id: String,
    
    /// Credential name
    pub name: String,
    
    /// Credential type
    pub type_name: String,
    
    /// Whether the credential is shared
    pub shared: bool,
    
    /// Owner user ID
    pub owner_id: Option<String>,
    
    /// Notes
    pub notes: Option<String>,
    
    /// Creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
    
    /// Last update timestamp
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Create credential request DTO.
#[derive(Deserialize, Validate, ToSchema)]
pub struct CreateCredentialRequest {
    /// Credential name
    #[validate(length(min = 1, max = 255))]
    pub name: String,
    
    /// Credential type
    #[validate(length(min = 1, max = 100))]
    pub type_name: String,
    
    /// Credential data
    pub data: serde_json::Value,
    
    /// Whether the credential should be shared
    pub shared: Option<bool>,
    
    /// Notes
    #[validate(length(max = 1000))]
    pub notes: Option<String>,
}

/// Update credential request DTO.
#[derive(Deserialize, Validate, ToSchema)]
pub struct UpdateCredentialRequest {
    /// Credential name
    #[validate(length(min = 1, max = 255))]
    pub name: Option<String>,
    
    /// Credential data (if updating)
    pub data: Option<serde_json::Value>,
    
    /// Whether the credential should be shared
    pub shared: Option<bool>,
    
    /// Notes
    #[validate(length(max = 1000))]
    pub notes: Option<String>,
}

impl From<Credential> for CredentialResponse {
    fn from(credential: Credential) -> Self {
        Self {
            id: credential.id.to_string(),
            name: credential.name,
            type_name: credential.type_name,
            shared: credential.shared,
            owner_id: credential.owner_id.map(|id| id.to_string()),
            notes: credential.notes,
            created_at: credential.timestamps.created_at,
            updated_at: credential.timestamps.updated_at,
        }
    }
}

/// Get all credentials for the current user.
#[utoipa::path(
    get,
    path = "/api/credentials",
    tag = "Credentials",
    responses(
        (status = 200, description = "List of credentials", body = Vec<CredentialResponse>),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_credentials<W, E, C, U, V>(
    Extension(state): Extension<Arc<ApiState<W, E, C, U, V>>>,
) -> Result<Json<Vec<CredentialResponse>>>
where
    W: r8s_core::repository::WorkflowRepository,
    E: r8s_core::repository::ExecutionRepository,
    C: CredentialRepository,
    U: r8s_core::repository::UserRepository,
    V: r8s_core::repository::VariableRepository,
{
    // TODO: Get user ID from auth context
    let user_id = EntityId::nil(); // Placeholder
    
    // Create and execute the use case
    let use_case = ListUserCredentialsUseCase::new(state.credential_repository.clone());
    
    let credentials = use_case.execute(user_id).await?;
    
    // Convert to response DTOs
    let response = credentials.into_iter().map(CredentialResponse::from).collect();
    
    Ok(Json(response))
}

/// Get a specific credential by ID.
#[utoipa::path(
    get,
    path = "/api/credentials/{id}",
    tag = "Credentials",
    params(
        ("id" = String, Path, description = "Credential ID")
    ),
    responses(
        (status = 200, description = "Credential details", body = CredentialResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Credential not found"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_credential<W, E, C, U, V>(
    Extension(state): Extension<Arc<ApiState<W, E, C, U, V>>>,
    Path(id): Path<String>,
) -> Result<Json<CredentialResponse>>
where
    W: r8s_core::repository::WorkflowRepository,
    E: r8s_core::repository::ExecutionRepository,
    C: CredentialRepository,
    U: r8s_core::repository::UserRepository,
    V: r8s_core::repository::VariableRepository,
{
    // Parse credential ID
    let credential_id = EntityId::parse_str(&id)
        .map_err(|_| ApiError::BadRequest(format!("Invalid credential ID: {}", id)))?;
    
    // TODO: Get user ID from auth context
    let user_id = EntityId::nil(); // Placeholder
    
    // Mock encryption service for now
    struct NoopEncryptionService;
    impl r8s_core::use_case::credential::CredentialEncryptionService for NoopEncryptionService {
        fn encrypt(&self, data: serde_json::Value) -> r8s_core::common::Result<serde_json::Value> {
            Ok(data)
        }
        
        fn decrypt(&self, data: serde_json::Value) -> r8s_core::common::Result<serde_json::Value> {
            Ok(data)
        }
    }
    
    // Create and execute the use case
    let encryption_service = Arc::new(NoopEncryptionService);
    let use_case = GetCredentialUseCase::new(
        state.credential_repository.clone(),
        encryption_service,
    );
    
    let credential = use_case.execute(credential_id, user_id).await?;
    
    // Convert to response DTO
    let response = CredentialResponse::from(credential);
    
    Ok(Json(response))
}

/// Create a new credential.
#[utoipa::path(
    post,
    path = "/api/credentials",
    tag = "Credentials",
    request_body = CreateCredentialRequest,
    responses(
        (status = 201, description = "Credential created", body = CredentialResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 422, description = "Validation error"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn create_credential<W, E, C, U, V>(
    Extension(state): Extension<Arc<ApiState<W, E, C, U, V>>>,
    Json(request): Json<CreateCredentialRequest>,
) -> Result<(StatusCode, Json<CredentialResponse>)>
where
    W: r8s_core::repository::WorkflowRepository,
    E: r8s_core::repository::ExecutionRepository,
    C: CredentialRepository,
    U: r8s_core::repository::UserRepository,
    V: r8s_core::repository::VariableRepository,
{
    // Validate the request
    request.validate()
        .map_err(|e| ApiError::Validation(e.to_string()))?;
    
    // TODO: Get user ID from auth context
    let user_id = Some(EntityId::nil()); // Placeholder
    
    // Mock encryption service for now
    struct NoopEncryptionService;
    impl r8s_core::use_case::credential::CredentialEncryptionService for NoopEncryptionService {
        fn encrypt(&self, data: serde_json::Value) -> r8s_core::common::Result<serde_json::Value> {
            Ok(data)
        }
        
        fn decrypt(&self, data: serde_json::Value) -> r8s_core::common::Result<serde_json::Value> {
            Ok(data)
        }
    }
    
    // Create input for the use case
    let input = CreateCredentialInput {
        name: request.name,
        type_name: request.type_name,
        data: request.data,
        owner_id: user_id,
        shared: request.shared.unwrap_or(false),
        notes: request.notes,
    };
    
    // Create and execute the use case
    let encryption_service = Arc::new(NoopEncryptionService);
    let use_case = CreateCredentialUseCase::new(
        state.credential_repository.clone(),
        encryption_service,
    );
    
    let credential = use_case.execute(input).await?;
    
    // Convert to response DTO
    let response = CredentialResponse::from(credential);
    
    Ok((StatusCode::CREATED, Json(response)))
}

/// Update an existing credential.
#[utoipa::path(
    put,
    path = "/api/credentials/{id}",
    tag = "Credentials",
    params(
        ("id" = String, Path, description = "Credential ID")
    ),
    request_body = UpdateCredentialRequest,
    responses(
        (status = 200, description = "Credential updated", body = CredentialResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Credential not found"),
        (status = 422, description = "Validation error"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn update_credential<W, E, C, U, V>(
    Extension(state): Extension<Arc<ApiState<W, E, C, U, V>>>,
    Path(id): Path<String>,
    Json(request): Json<UpdateCredentialRequest>,
) -> Result<Json<CredentialResponse>>
where
    W: r8s_core::repository::WorkflowRepository,
    E: r8s_core::repository::ExecutionRepository,
    C: CredentialRepository,
    U: r8s_core::repository::UserRepository,
    V: r8s_core::repository::VariableRepository,
{
    // Validate the request
    request.validate()
        .map_err(|e| ApiError::Validation(e.to_string()))?;
    
    // Parse credential ID
    let credential_id = EntityId::parse_str(&id)
        .map_err(|_| ApiError::BadRequest(format!("Invalid credential ID: {}", id)))?;
    
    // TODO: Get user ID from auth context
    let user_id = EntityId::nil(); // Placeholder
    
    // Mock encryption service for now
    struct NoopEncryptionService;
    impl r8s_core::use_case::credential::CredentialEncryptionService for NoopEncryptionService {
        fn encrypt(&self, data: serde_json::Value) -> r8s_core::common::Result<serde_json::Value> {
            Ok(data)
        }
        
        fn decrypt(&self, data: serde_json::Value) -> r8s_core::common::Result<serde_json::Value> {
            Ok(data)
        }
    }
    
    // Create input for the use case
    let input = UpdateCredentialInput {
        id: credential_id,
        name: request.name,
        data: request.data,
        notes: request.notes,
        user_id,
    };
    
    // Create and execute the use case
    let encryption_service = Arc::new(NoopEncryptionService);
    let use_case = UpdateCredentialUseCase::new(
        state.credential_repository.clone(),
        encryption_service,
    );
    
    let credential = use_case.execute(input).await?;
    
    // If shared status needs to be updated
    if let Some(shared) = request.shared {
        let credential_id = credential.id;
        
        if shared && !credential.shared {
            state.credential_repository.share_credential(credential_id).await?;
        } else if !shared && credential.shared {
            state.credential_repository.unshare_credential(credential_id).await?;
        }
        
        // Reload the credential to get the updated state
        let credential = state.credential_repository.find_by_id(credential_id).await?;
        
        // Convert to response DTO
        let response = CredentialResponse::from(credential);
        
        Ok(Json(response))
    } else {
        // Convert to response DTO
        let response = CredentialResponse::from(credential);
        
        Ok(Json(response))
    }
}

/// Delete a credential.
#[utoipa::path(
    delete,
    path = "/api/credentials/{id}",
    tag = "Credentials",
    params(
        ("id" = String, Path, description = "Credential ID")
    ),
    responses(
        (status = 204, description = "Credential deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Credential not found"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn delete_credential<W, E, C, U, V>(
    Extension(state): Extension<Arc<ApiState<W, E, C, U, V>>>,
    Path(id): Path<String>,
) -> Result<StatusCode>
where
    W: r8s_core::repository::WorkflowRepository,
    E: r8s_core::repository::ExecutionRepository,
    C: CredentialRepository,
    U: r8s_core::repository::UserRepository,
    V: r8s_core::repository::VariableRepository,
{
    // Parse credential ID
    let credential_id = EntityId::parse_str(&id)
        .map_err(|_| ApiError::BadRequest(format!("Invalid credential ID: {}", id)))?;
    
    // TODO: Get user ID from auth context
    let user_id = EntityId::nil(); // Placeholder
    
    // Create and execute the use case
    let use_case = DeleteCredentialUseCase::new(state.credential_repository.clone());
    
    use_case.execute(credential_id, user_id).await?;
    
    Ok(StatusCode::NO_CONTENT)
}