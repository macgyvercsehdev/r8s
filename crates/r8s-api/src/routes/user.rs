// crates/r8s-api/src/routes/user.rs
//! User API routes.

use std::sync::Arc;
use axum::{
    extract::{Path, Extension, Json},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use validator::Validate;
use utoipa::ToSchema;

use r8s_core::common::EntityId;
use r8s_core::entity::{User, UserRole};
use r8s_core::repository::UserRepository;
use r8s_core::use_case::user::{
    RegisterUserUseCase,
    RegisterUserInput,
    AuthenticateUserUseCase,
    AuthenticateUserInput,
    UpdateUserUseCase,
    UpdateUserInput,
    ChangeUserRoleUseCase,
    ChangeUserRoleInput,
};

use crate::error::{ApiError, Result};
use crate::server::ApiState;
use crate::middleware::auth::AuthMiddleware;

/// User response DTO.
#[derive(Serialize, ToSchema)]
pub struct UserResponse {
    /// User ID
    pub id: String,
    
    /// Username
    pub username: String,
    
    /// Email
    pub email: String,
    
    /// Full name
    pub full_name: String,
    
    /// User role
    pub role: String,
    
    /// Whether the user is active
    pub active: bool,
    
    /// Last login timestamp
    pub last_login: Option<chrono::DateTime<chrono::Utc>>,
    
    /// Creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
    
    /// Last update timestamp
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Create user request DTO.
#[derive(Deserialize, Validate, ToSchema)]
pub struct CreateUserRequest {
    /// Username
    #[validate(length(min = 3, max = 50))]
    pub username: String,
    
    /// Email
    #[validate(email)]
    pub email: String,
    
    /// Full name
    #[validate(length(min = 1, max = 100))]
    pub full_name: String,
    
    /// Password
    #[validate(length(min = 8))]
    pub password: String,
    
    /// User role (admin only)
    pub role: Option<String>,
}

/// Update user request DTO.
#[derive(Deserialize, Validate, ToSchema)]
pub struct UpdateUserRequest {
    /// Email
    #[validate(email)]
    pub email: Option<String>,
    
    /// Full name
    #[validate(length(min = 1, max = 100))]
    pub full_name: Option<String>,
    
    /// User preferences
    pub preferences: Option<serde_json::Value>,
}

/// Login request DTO.
#[derive(Deserialize, Validate, ToSchema)]
pub struct LoginRequest {
    /// Username or email
    #[validate(length(min = 1))]
    pub username_or_email: String,
    
    /// Password
    #[validate(length(min = 1))]
    pub password: String,
}

/// Login response DTO.
#[derive(Serialize, ToSchema)]
pub struct LoginResponse {
    /// JWT access token
    pub access_token: String,
    
    /// Token type
    pub token_type: String,
    
    /// Token expiration in seconds
    pub expires_in: u32,
    
    /// User information
    pub user: UserResponse,
}

impl From<User> for UserResponse {
    fn from(user: User) -> Self {
        Self {
            id: user.id.to_string(),
            username: user.username,
            email: user.email,
            full_name: user.full_name,
            role: user.role.to_string().to_lowercase(),
            active: user.active,
            last_login: user.last_login,
            created_at: user.timestamps.created_at,
            updated_at: user.timestamps.updated_at,
        }
    }
}

/// Get all users (admin only).
#[utoipa::path(
    get,
    path = "/api/users",
    tag = "Users",
    responses(
        (status = 200, description = "List of users", body = Vec<UserResponse>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_users<W, E, C, U, V>(
    Extension(state): Extension<Arc<ApiState<W, E, C, U, V>>>,
) -> Result<Json<Vec<UserResponse>>>
where
    W: r8s_core::repository::WorkflowRepository,
    E: r8s_core::repository::ExecutionRepository,
    C: r8s_core::repository::CredentialRepository,
    U: UserRepository,
    V: r8s_core::repository::VariableRepository,
{
    // TODO: Get user ID from auth context and check if admin
    let admin_id = EntityId::nil(); // Placeholder
    
    // Fetch active users
    let users = state.user_repository.find_active_users().await?;
    
    // Convert to response DTOs
    let response = users.into_iter().map(UserResponse::from).collect();
    
    Ok(Json(response))
}

/// Get a specific user by ID.
#[utoipa::path(
    get,
    path = "/api/users/{id}",
    tag = "Users",
    params(
        ("id" = String, Path, description = "User ID")
    ),
    responses(
        (status = 200, description = "User details", body = UserResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_user<W, E, C, U, V>(
    Extension(state): Extension<Arc<ApiState<W, E, C, U, V>>>,
    Path(id): Path<String>,
) -> Result<Json<UserResponse>>
where
    W: r8s_core::repository::WorkflowRepository,
    E: r8s_core::repository::ExecutionRepository,
    C: r8s_core::repository::CredentialRepository,
    U: UserRepository,
    V: r8s_core::repository::VariableRepository,
{
    // Parse user ID
    let user_id = EntityId::parse_str(&id)
        .map_err(|_| ApiError::BadRequest(format!("Invalid user ID: {}", id)))?;
    
    // TODO: Check if current user has permission to view this user
    
    // Fetch user
    let user = state.user_repository.find_by_id(user_id).await?;
    
    // Convert to response DTO
    let response = UserResponse::from(user);
    
    Ok(Json(response))
}

/// Create a new user.
#[utoipa::path(
    post,
    path = "/api/users",
    tag = "Users",
    request_body = CreateUserRequest,
    responses(
        (status = 201, description = "User created", body = UserResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 409, description = "Username or email already exists"),
        (status = 422, description = "Validation error"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn create_user<W, E, C, U, V>(
    Extension(state): Extension<Arc<ApiState<W, E, C, U, V>>>,
    Json(request): Json<CreateUserRequest>,
) -> Result<(StatusCode, Json<UserResponse>)>
where
    W: r8s_core::repository::WorkflowRepository,
    E: r8s_core::repository::ExecutionRepository,
    C: r8s_core::repository::CredentialRepository,
    U: UserRepository,
    V: r8s_core::repository::VariableRepository,
{
    // Validate the request
    request.validate()
        .map_err(|e| ApiError::Validation(e.to_string()))?;
    
    // Parse role if provided
    let role = request.role.as_ref().map(|r| {
        match r.to_lowercase().as_str() {
            "admin" => UserRole::Admin,
            "team_admin" => UserRole::TeamAdmin,
            "power_user" => UserRole::PowerUser,
            _ => UserRole::User,
        }
    });
    
    // Mock password hash service for now
    struct SimplePasswordHashService;
    impl r8s_core::use_case::user::PasswordHashService for SimplePasswordHashService {
        fn hash_password(&self, password: &str) -> r8s_core::common::Result<String> {
            // Warning: This is not secure! Just for demo purposes
            Ok(format!("hashed_{}", password))
        }
        
        fn verify_password(&self, password: &str, hash: &str) -> r8s_core::common::Result<bool> {
            // Warning: This is not secure! Just for demo purposes
            Ok(hash == format!("hashed_{}", password))
        }
    }
    
    // Create input for the use case
    let input = RegisterUserInput {
        username: request.username,
        email: request.email,
        full_name: request.full_name,
        password: request.password,
        role,
    };
    
    // Create and execute the use case
    let password_service = Arc::new(SimplePasswordHashService);
    let use_case = RegisterUserUseCase::new(
        state.user_repository.clone(),
        password_service,
    );
    
    let user = use_case.execute(input).await?;
    
    // Convert to response DTO
    let response = UserResponse::from(user);
    
    Ok((StatusCode::CREATED, Json(response)))
}

/// Update an existing user.
#[utoipa::path(
    put,
    path = "/api/users/{id}",
    tag = "Users",
    params(
        ("id" = String, Path, description = "User ID")
    ),
    request_body = UpdateUserRequest,
    responses(
        (status = 200, description = "User updated", body = UserResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "User not found"),
        (status = 409, description = "Email already exists"),
        (status = 422, description = "Validation error"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn update_user<W, E, C, U, V>(
    Extension(state): Extension<Arc<ApiState<W, E, C, U, V>>>,
    Path(id): Path<String>,
    Json(request): Json<UpdateUserRequest>,
) -> Result<Json<UserResponse>>
where
    W: r8s_core::repository::WorkflowRepository,
    E: r8s_core::repository::ExecutionRepository,
    C: r8s_core::repository::CredentialRepository,
    U: UserRepository,
    V: r8s_core::repository::VariableRepository,
{
    // Validate the request
    request.validate()
        .map_err(|e| ApiError::Validation(e.to_string()))?;
    
    // Parse user ID
    let user_id = EntityId::parse_str(&id)
        .map_err(|_| ApiError::BadRequest(format!("Invalid user ID: {}", id)))?;
    
    // TODO: Check if current user has permission to update this user
    
    // Create input for the use case
    let input = UpdateUserInput {
        id: user_id,
        email: request.email,
        full_name: request.full_name,
        preferences: request.preferences,
    };
    
    // Create and execute the use case
    let use_case = UpdateUserUseCase::new(state.user_repository.clone());
    
    let user = use_case.execute(input).await?;
    
    // Convert to response DTO
    let response = UserResponse::from(user);
    
    Ok(Json(response))
}

/// Authenticate a user and get a token.
#[utoipa::path(
    post,
    path = "/api/auth/login",
    tag = "Authentication",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Authentication successful", body = LoginResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Authentication failed"),
        (status = 422, description = "Validation error"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn login<W, E, C, U, V>(
    Extension(state): Extension<Arc<ApiState<W, E, C, U, V>>>,
    Json(request): Json<LoginRequest>,
) -> Result<Json<LoginResponse>>
where
    W: r8s_core::repository::WorkflowRepository,
    E: r8s_core::repository::ExecutionRepository,
    C: r8s_core::repository::CredentialRepository,
    U: UserRepository,
    V: r8s_core::repository::VariableRepository,
{
    // Validate the request
    request.validate()
        .map_err(|e| ApiError::Validation(e.to_string()))?;
    
    // Mock password hash service for now
    struct SimplePasswordHashService;
    impl r8s_core::use_case::user::PasswordHashService for SimplePasswordHashService {
        fn hash_password(&self, password: &str) -> r8s_core::common::Result<String> {
            // Warning: This is not secure! Just for demo purposes
            Ok(format!("hashed_{}", password))
        }
        
        fn verify_password(&self, password: &str, hash: &str) -> r8s_core::common::Result<bool> {
            // Warning: This is not secure! Just for demo purposes
            Ok(hash == format!("hashed_{}", password))
        }
    }
    
    // Create input for the use case
    let input = AuthenticateUserInput {
        username_or_email: request.username_or_email,
        password: request.password,
    };
    
    // Create and execute the use case
    let password_service = Arc::new(SimplePasswordHashService);
    let use_case = AuthenticateUserUseCase::new(
        state.user_repository.clone(),
        password_service,
    );
    
    let result = use_case.execute(input).await?;
    
    // Create a JWT token
    let auth = AuthMiddleware::new("your_secret_key"); // TODO: Get from config
    let token = auth.generate_token(
        Uuid::from_bytes(result.user.id.as_bytes()),
        &result.user.role.to_string(),
        30 // 30 days
    )?;
    
    // Create response
    let response = LoginResponse {
        access_token: token,
        token_type: "Bearer".to_string(),
        expires_in: 30 * 24 * 60 * 60, // 30 days in seconds
        user: UserResponse::from(result.user),
    };
    
    Ok(Json(response))
}

/// Refresh an authentication token.
#[utoipa::path(
    post,
    path = "/api/auth/refresh",
    tag = "Authentication",
    responses(
        (status = 200, description = "Token refreshed", body = LoginResponse),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn refresh_token<W, E, C, U, V>(
    Extension(state): Extension<Arc<ApiState<W, E, C, U, V>>>,
) -> Result<Json<LoginResponse>>
where
    W: r8s_core::repository::WorkflowRepository,
    E: r8s_core::repository::ExecutionRepository,
    C: r8s_core::repository::CredentialRepository,
    U: UserRepository,
    V: r8s_core::repository::VariableRepository,
{
    // TODO: Get user ID from auth context
    let user_id = EntityId::nil(); // Placeholder
    
    // Fetch the user
    let user = state.user_repository.find_by_id(user_id).await?;
    
    // Create a new JWT token
    let auth = AuthMiddleware::new("your_secret_key"); // TODO: Get from config
    let token = auth.generate_token(
        Uuid::from_bytes(user.id.as_bytes()),
        &user.role.to_string(),
        30 // 30 days
    )?;
    
    // Create response
    let response = LoginResponse {
        access_token: token,
        token_type: "Bearer".to_string(),
        expires_in: 30 * 24 * 60 * 60, // 30 days in seconds
        user: UserResponse::from(user),
    };
    
    Ok(Json(response))
}