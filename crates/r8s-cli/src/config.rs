//! Configuration for the r8s platform.

use config::{Config as ConfigLib, File};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;

/// Database configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DatabaseConfig {
    /// Database connection URL
    pub url: String,

    /// Maximum number of connections in the pool
    pub max_connections: u32,

    /// Whether to run database migrations on startup
    pub run_migrations: bool,
}

/// API server configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ApiConfig {
    /// Address to listen on
    pub host: IpAddr,

    /// Port to listen on
    pub port: u16,

    /// JWT secret key
    pub jwt_secret: String,

    /// Whether to enable OpenAPI documentation
    pub enable_openapi: bool,
}

/// Worker configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WorkerConfig {
    /// Number of worker threads
    pub threads: usize,

    /// Maximum number of concurrent executions
    pub max_concurrent_executions: usize,

    /// Worker ID (auto-generated if not specified)
    pub worker_id: Option<String>,
}

/// Plugin configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PluginConfig {
    /// Directories to search for plugins
    pub plugin_dirs: Vec<PathBuf>,

    /// Whether to load plugins on startup
    pub load_on_startup: bool,
}

/// Complete configuration for the r8s platform.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    /// Database configuration
    pub database: DatabaseConfig,

    /// API server configuration
    pub api: ApiConfig,

    /// Worker configuration
    pub worker: WorkerConfig,

    /// Plugin configuration
    pub plugin: PluginConfig,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "postgres://postgres:postgres@localhost:5432/r8s".to_string(),
            max_connections: 5,
            run_migrations: true,
        }
    }
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            host: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            port: 3000,
            jwt_secret: "change_me_in_production".to_string(),
            enable_openapi: true,
        }
    }
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            threads: num_cpus::get(),
            max_concurrent_executions: 10,
            worker_id: None,
        }
    }
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            plugin_dirs: vec![PathBuf::from("./plugins")],
            load_on_startup: true,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            database: DatabaseConfig::default(),
            api: ApiConfig::default(),
            worker: WorkerConfig::default(),
            plugin: PluginConfig::default(),
        }
    }
}

impl Config {
    /// Load configuration from files and environment variables.
    pub fn load() -> anyhow::Result<Self> {
        // Set up configuration sources
        let builder = ConfigLib::builder()
            // Start with default values
            .add_source(config::Config::try_from(&Self::default())?)
            // Add configuration from files if they exist
            .add_source(File::with_name("r8s").required(false))
            .add_source(File::with_name("/etc/r8s/config").required(false))
            // Add environment variables with prefix R8S_
            .add_source(config::Environment::with_prefix("R8S").separator("__"));

        // Build the configuration
        let config: Self = builder.build()?.try_deserialize()?;

        Ok(config)
    }

    /// Get the API server socket address.
    pub fn api_addr(&self) -> SocketAddr {
        SocketAddr::new(self.api.host, self.api.port)
    }
}
