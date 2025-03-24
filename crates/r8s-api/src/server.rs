// crates/r8s-api/src/server.rs
//! API server implementation.

use std::sync::Arc;
use std::net::SocketAddr;
use axum::{
    Router,
    routing::{get, post, put, delete},
    Extension,
    middleware,
};
use tower_http::cors::{CorsLayer, Any};
use tower_http::trace::TraceLayer;
use tracing::{info, error};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use r8s_core::repository::{
    WorkflowRepository, 
    ExecutionRepository, 
    CredentialRepository,
    UserRepository, 
    VariableRepository
};
use r8s_plugin::registry::PluginRegistry;

use crate::routes;
use crate::middleware::auth::AuthMiddleware;
use crate::middleware::logging::LoggingMiddleware;

/// OpenAPI documentation.
#[derive(OpenApi)]
#[openapi(
    paths(
        routes::workflow::get_workflows,
        routes::workflow::get_workflow,
        routes::workflow::create_workflow,
        routes::workflow::update_workflow,
        routes::workflow::delete_workflow,
        routes::execution::get_executions,
        routes::execution::get_execution,
        routes::execution::start_workflow,
        routes::execution::cancel_execution,
        routes::credential::get_credentials,
        routes::credential::get_credential,
        routes::credential::create_credential,
        routes::credential::update_credential,
        routes::credential::delete_credential,
        routes::user::get_users,
        routes::user::get_user,
        routes::user::create_user,
        routes::user::update_user,
        routes::user::login,
        routes::variable::get_variables,
        routes::variable::get_variable,
        routes::variable::create_variable,
        routes::variable::update_variable,
        routes::variable::delete_variable
    ),
    components(
        schemas(
            routes::workflow::WorkflowResponse,
            routes::workflow::CreateWorkflowRequest,
            routes::workflow::UpdateWorkflowRequest,
            routes::execution::ExecutionResponse,
            routes::execution::StartWorkflowRequest,
            routes::credential::CredentialResponse,
            routes::credential::CreateCredentialRequest,
            routes::credential::UpdateCredentialRequest,
            routes::user::UserResponse,
            routes::user::CreateUserRequest,
            routes::user::UpdateUserRequest,
            routes::user::LoginRequest,
            routes::user::LoginResponse,
            routes::variable::VariableResponse,
            routes::variable::CreateVariableRequest,
            routes::variable::UpdateVariableRequest
        )
    ),
    tags(
        (name = "r8s API", description = "r8s Workflow Orchestrator API")
    )
)]
struct ApiDoc;

/// API server configuration.
pub struct ApiServerConfig {
    /// Address to bind the server to
    pub bind_address: SocketAddr,
    
    /// Authentication secret key
    pub auth_secret: String,
    
    /// Whether to enable OpenAPI documentation
    pub enable_openapi: bool,
}

/// API server state.
pub struct ApiState<W, E, C, U, V> 
where
    W: WorkflowRepository + 'static,
    E: ExecutionRepository + 'static,
    C: CredentialRepository + 'static,
    U: UserRepository + 'static,
    V: VariableRepository + 'static,
{
    /// Workflow repository
    pub workflow_repository: Arc<W>,
    
    /// Execution repository
    pub execution_repository: Arc<E>,
    
    /// Credential repository
    pub credential_repository: Arc<C>,
    
    /// User repository
    pub user_repository: Arc<U>,
    
    /// Variable repository
    pub variable_repository: Arc<V>,
    
    /// Plugin registry
    pub plugin_registry: Arc<PluginRegistry>,
}

/// API server for the r8s platform.
pub struct ApiServer<W, E, C, U, V>
where
    W: WorkflowRepository + 'static,
    E: ExecutionRepository + 'static,
    C: CredentialRepository + 'static,
    U: UserRepository + 'static,
    V: VariableRepository + 'static,
{
    /// Server configuration
    config: ApiServerConfig,
    
    /// Server state
    state: Arc<ApiState<W, E, C, U, V>>,
}

impl<W, E, C, U, V> ApiServer<W, E, C, U, V>
where
    W: WorkflowRepository + 'static,
    E: ExecutionRepository + 'static,
    C: CredentialRepository + 'static,
    U: UserRepository + 'static,
    V: VariableRepository + 'static,
{
    /// Create a new API server with the given configuration and dependencies.
    pub fn new(
        config: ApiServerConfig,
        workflow_repository: Arc<W>,
        execution_repository: Arc<E>,
        credential_repository: Arc<C>,
        user_repository: Arc<U>,
        variable_repository: Arc<V>,
        plugin_registry: Arc<PluginRegistry>,
    ) -> Self {
        let state = Arc::new(ApiState {
            workflow_repository,
            execution_repository,
            credential_repository,
            user_repository,
            variable_repository,
            plugin_registry,
        });
        
        Self { config, state }
    }
    
    /// Run the API server.
    pub async fn run(&self) -> Result<(), hyper::Error> {
        let auth = AuthMiddleware::new(&self.config.auth_secret);
        let logging = LoggingMiddleware::new();
        
        // Create the router with all routes
        let mut app = Router::new()
            .merge(self.workflows_routes())
            .merge(self.executions_routes())
            .merge(self.credentials_routes())
            .merge(self.users_routes())
            .merge(self.variables_routes())
            .layer(Extension(self.state.clone()))
            .layer(middleware::from_fn_with_state(auth.clone(), AuthMiddleware::check_auth))
            .layer(middleware::from_fn_with_state(logging.clone(), LoggingMiddleware::log_request))
            .layer(CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any))
            .layer(TraceLayer::new_for_http());
        
        // Add OpenAPI documentation if enabled
        if self.config.enable_openapi {
            app = app.merge(
                SwaggerUi::new("/swagger-ui")
                    .url("/api-doc/openapi.json", ApiDoc::openapi())
            );
        }
        
        // Start the server
        info!("Starting API server on {}", self.config.bind_address);
        axum::Server::bind(&self.config.bind_address)
            .serve(app.into_make_service())
            .await
    }
    
    // Define routes for different API endpoints
    
    fn workflows_routes(&self) -> Router {
        Router::new()
            .route("/api/workflows", get(routes::workflow::get_workflows))
            .route("/api/workflows", post(routes::workflow::create_workflow))
            .route("/api/workflows/:id", get(routes::workflow::get_workflow))
            .route("/api/workflows/:id", put(routes::workflow::update_workflow))
            .route("/api/workflows/:id", delete(routes::workflow::delete_workflow))
    }
    
    fn executions_routes(&self) -> Router {
        Router::new()
            .route("/api/executions", get(routes::execution::get_executions))
            .route("/api/executions/:id", get(routes::execution::get_execution))
            .route("/api/workflows/:id/execute", post(routes::execution::start_workflow))
            .route("/api/executions/:id/cancel", post(routes::execution::cancel_execution))
    }
    
    fn credentials_routes(&self) -> Router {
        Router::new()
            .route("/api/credentials", get(routes::credential::get_credentials))
            .route("/api/credentials", post(routes::credential::create_credential))
            .route("/api/credentials/:id", get(routes::credential::get_credential))
            .route("/api/credentials/:id", put(routes::credential::update_credential))
            .route("/api/credentials/:id", delete(routes::credential::delete_credential))
    }
    
    fn users_routes(&self) -> Router {
        Router::new()
            .route("/api/users", get(routes::user::get_users))
            .route("/api/users", post(routes::user::create_user))
            .route("/api/users/:id", get(routes::user::get_user))
            .route("/api/users/:id", put(routes::