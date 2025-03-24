// crates/r8s-cli/src/commands/serve.rs
//! Command to start the r8s server.

use clap::Args;
use std::sync::Arc;
use tracing::{error, info};
use uuid::Uuid;

use r8s_api::server::{ApiServer, ApiServerConfig};
use r8s_core::repository::{
    CredentialRepository, ExecutionRepository, UserRepository, VariableRepository,
    WorkflowRepository,
};
use r8s_persistence::connection::ConnectionManager;
use r8s_persistence::postgres::{
    PostgresCredentialRepository, PostgresExecutionRepository, PostgresUserRepository,
    PostgresVariableRepository, PostgresWorkflowRepository,
};
use r8s_plugin::loader::PluginLoader;
use r8s_plugin::registry::PluginRegistry;

use crate::config::Config;

/// Arguments for the serve command.
#[derive(Args)]
pub struct ServeArgs {
    /// Database connection string
    #[arg(long, env = "R8S_DATABASE__URL")]
    db_url: Option<String>,

    /// API host
    #[arg(long, env = "R8S_API__HOST")]
    api_host: Option<String>,

    /// API port
    #[arg(long, env = "R8S_API__PORT")]
    api_port: Option<u16>,
}

/// Execute the serve command.
pub async fn execute(args: ServeArgs, mut config: Config) -> anyhow::Result<()> {
    // Override configuration with command line arguments
    if let Some(db_url) = args.db_url {
        config.database.url = db_url;
    }

    if let Some(api_host) = args.api_host {
        config.api.host = api_host.parse()?;
    }

    if let Some(api_port) = args.api_port {
        config.api.port = api_port;
    }

    // Generate worker ID if not specified
    if config.worker.worker_id.is_none() {
        config.worker.worker_id = Some(Uuid::new_v4().to_string());
    }

    // Set up database connection
    info!("Connecting to database at {}", config.database.url);
    let conn_manager = Arc::new(ConnectionManager::new(
        &config.database.url,
        config.database.max_connections,
    )?);

    // Run migrations if configured
    if config.database.run_migrations {
        info!("Running database migrations");
        conn_manager.run_migrations().await?;
    }

    // Create repositories
    let workflow_repo = Arc::new(PostgresWorkflowRepository::new(conn_manager.clone()));
    let execution_repo = Arc::new(PostgresExecutionRepository::new(conn_manager.clone()));
    let credential_repo = Arc::new(PostgresCredentialRepository::new(conn_manager.clone()));
    let user_repo = Arc::new(PostgresUserRepository::new(conn_manager.clone()));
    let variable_repo = Arc::new(PostgresVariableRepository::new(conn_manager.clone()));

    // Set up plugin registry
    let plugin_registry = Arc::new(PluginRegistry::new());

    // Load plugins if configured
    if config.plugin.load_on_startup {
        info!("Loading plugins");
        let mut plugin_loader = PluginLoader::new(plugin_registry.clone());

        for dir in &config.plugin.plugin_dirs {
            if let Err(e) = plugin_loader.add_plugin_directory(dir) {
                error!("Failed to add plugin directory {:?}: {}", dir, e);
                continue;
            }

            info!("Added plugin directory: {:?}", dir);
        }

        if let Err(e) = plugin_loader.load_all_plugins() {
            error!("Failed to load plugins: {}", e);
        }

        info!("Initializing plugins");
        if let Err(e) = plugin_registry.initialize_all_plugins().await {
            error!("Failed to initialize plugins: {}", e);
        }
    }

    // Create and start API server
    let api_config = ApiServerConfig {
        bind_address: config.api_addr(),
        auth_secret: config.api.jwt_secret.clone(),
        enable_openapi: config.api.enable_openapi,
    };

    let api_server = ApiServer::new(
        api_config,
        workflow_repo,
        execution_repo,
        credential_repo,
        user_repo,
        variable_repo,
        plugin_registry,
    );

    info!("Starting API server at {}", config.api_addr());
    api_server.run().await?;

    Ok(())
}
