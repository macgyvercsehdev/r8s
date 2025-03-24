// r8s-persistence/src/connection.rs
//! Database connection management.

use deadpool_postgres::{Config, Pool, PoolError, Runtime};
use tokio_postgres::{NoTls, Error as PgError};
use tokio_postgres::config::SslMode;
use std::time::Duration;
use std::sync::Arc;

use crate::error::{PersistenceError, Result};

/// Configuration for database connections.
#[derive(Debug, Clone)]
pub struct ConnectionConfig {
    /// Database host
    pub host: String,
    
    /// Database port
    pub port: u16,
    
    /// Database name
    pub database: String,
    
    /// Database user
    pub user: String,
    
    /// Database password
    pub password: String,
    
    /// SSL mode
    pub ssl_mode: SslMode,
    
    /// Maximum number of connections in the pool
    pub max_connections: usize,
    
    /// Connection timeout in seconds
    pub connection_timeout_seconds: u64,
    
    /// Statement timeout in seconds
    pub statement_timeout_seconds: u64,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 5432,
            database: "r8s".to_string(),
            user: "postgres".to_string(),
            password: "postgres".to_string(),
            ssl_mode: SslMode::Prefer,
            max_connections: 20,
            connection_timeout_seconds: 30,
            statement_timeout_seconds: 30,
        }
    }
}

/// Manages database connections for the application.
#[derive(Clone)]
pub struct ConnectionManager {
    /// Connection pool
    pool: Pool,
}

impl ConnectionManager {
    /// Create a new connection manager with the specified configuration.
    pub fn new(config: ConnectionConfig) -> Result<Self> {
        let mut pg_config = Config::new();
        pg_config.host = Some(config.host);
        pg_config.port = Some(config.port);
        pg_config.dbname = Some(config.database);
        pg_config.user = Some(config.user);
        pg_config.password = Some(config.password);
        pg_config.ssl_mode = Some(config.ssl_mode);
        pg_config.pool = Some(deadpool_postgres::PoolConfig::new(config.max_connections));
        
        let pool = pg_config.create_pool(Some(Runtime::Tokio1), NoTls)
            .map_err(|e| PersistenceError::ConnectionError(e.to_string()))?;
        
        Ok(Self { pool })
    }
    
    /// Create a new connection manager with a connection string.
    pub fn from_conn_string(conn_string: &str) -> Result<Self> {
        let pg_config = conn_string.parse::<tokio_postgres::Config>()
            .map_err(|e| PersistenceError::ConnectionError(e.to_string()))?;
            
        let mut config = Config::new();
        config.host = pg_config.get_hosts().first().map(|h| h.to_string());
        config.port = pg_config.get_ports().first().copied();
        config.dbname = pg_config.get_dbname().map(|s| s.to_string());
        config.user = pg_config.get_user().map(|s| s.to_string());
        config.password = pg_config.get_password().map(|s| s.to_string());
        config.ssl_mode = pg_config.get_ssl_mode();
        config.pool = Some(deadpool_postgres::PoolConfig::new(20));
        
        let pool = config.create_pool(Some(Runtime::Tokio1), NoTls)
            .map_err(|e| PersistenceError::ConnectionError(e.to_string()))?;
            
        Ok(Self { pool })
    }
    
    /// Get a connection from the pool.
    pub async fn get_connection(&self) -> Result<deadpool_postgres::Client> {
        self.pool.get().await
            .map_err(|e| PersistenceError::ConnectionError(e.to_string()))
    }
    
    /// Get the connection pool.
    pub fn pool(&self) -> &Pool {
        &self.pool
    }
    
    /// Test the database connection.
    pub async fn test_connection(&self) -> Result<()> {
        let client = self.get_connection().await?;
        client.execute("SELECT 1", &[]).await
            .map_err(|e| PersistenceError::ConnectionError(e.to_string()))?;
        Ok(())
    }
}