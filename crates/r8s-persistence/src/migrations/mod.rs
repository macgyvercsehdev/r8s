// r8s-persistence/src/migrations/mod.rs
//! Database schema migrations.

use refinery::embed_migrations;
use tokio_postgres::Client;

use crate::error::{PersistenceError, Result};
use crate::connection::ConnectionManager;

// Define migrations
embed_migrations!("./src/migrations");

/// Run migrations on the database.
pub async fn run_migrations(conn_manager: &ConnectionManager) -> Result<()> {
    // Get a raw client from the pool
    let client = conn_manager.get_connection().await?;
    
    // Get a raw connection from the client
    let (client, connection) = tokio_postgres::connect(
        &format!(
            "host={} port={} dbname={} user={} password={}",
            client.host(),
            client.port(),
            client.dbname().unwrap_or("r8s"),
            client.user().unwrap_or("postgres"),
            client.password().unwrap_or(""),
        ),
        tokio_postgres::NoTls,
    )
    .await
    .map_err(|e| PersistenceError::ConnectionError(e.to_string()))?;
    
    // Spawn a task to process connection events
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("Connection error: {}", e);
        }
    });
    
    // Run migrations
    embedded::migrations::runner()
        .run_async(&client)
        .await
        .map_err(|e| PersistenceError::MigrationError(e.to_string()))?;
    
    Ok(())
}

/// Create initial schema migration
pub const INITIAL_SCHEMA: &str = r#"
-- Create a UUID extension for generating UUIDs
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- Create enum types
CREATE TYPE execution_state AS ENUM (
    'pending',
    'running',
    'waiting',
    'success',
    'failed',
    'cancelled'
);

CREATE TYPE priority_level AS ENUM (
    'low',
    'normal',
    'high',
    'critical'
);

CREATE TYPE log_level AS ENUM (
    'debug',
    'info',
    'warning',
    'error'
);

CREATE TYPE variable_type AS ENUM (
    'string',
    'number',
    'boolean',
    'json',
    'binary'
);

CREATE TYPE node_type_category AS ENUM (
    'trigger',
    'core',
    'integration',
    'transformation',
    'flow',
    'generator',
    'ai',
    'io',
    'code',
    'utility'
);

CREATE TYPE user_role AS ENUM (
    'user',
    'power_user',
    'team_admin',
    'admin'
);

CREATE TYPE tag_color AS ENUM (
    'red',
    'green',
    'blue',
    'yellow',
    'orange',
    'purple',
    'gray',
    'teal',
    'pink',
    'brown'
);

-- Workflows table
CREATE TABLE workflows (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name VARCHAR(255) NOT NULL,
    description TEXT,
    version INTEGER NOT NULL DEFAULT 1,
    active BOOLEAN NOT NULL DEFAULT FALSE,
    static_data BOOLEAN NOT NULL DEFAULT TRUE,
    settings JSONB NOT NULL DEFAULT '{}',
    metadata JSONB,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    created_by UUID
);

-- Nodes table
CREATE TABLE nodes (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    workflow_id UUID NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    type_name VARCHAR(100) NOT NULL,
    type_version VARCHAR(50) NOT NULL,
    type_category node_type_category NOT NULL,
    plugin_id UUID,
    position_x FLOAT NOT NULL,
    position_y FLOAT NOT NULL,
    parameters JSONB NOT NULL DEFAULT '{}',
    disabled BOOLEAN NOT NULL DEFAULT FALSE,
    settings JSONB NOT NULL DEFAULT '{}',
    metadata JSONB,
    notes TEXT,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Node ports table (inputs and outputs)
CREATE TABLE node_ports (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    node_id UUID NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    is_input BOOLEAN NOT NULL, -- TRUE for input, FALSE for output
    data_type VARCHAR(50) NOT NULL,
    description TEXT,
    required BOOLEAN NOT NULL DEFAULT FALSE,
    examples JSONB,
    schema JSONB,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    UNIQUE (node_id, name, is_input)
);

-- Connections table
CREATE TABLE connections (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    workflow_id UUID NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
    source_node UUID NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    source_output VARCHAR(255) NOT NULL,
    target_node UUID NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    target_input VARCHAR(255) NOT NULL,
    condition_type VARCHAR(50),
    condition_expression TEXT,
    transform TEXT,
    metadata JSONB,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    UNIQUE (workflow_id, source_node, source_output, target_node, target_input)
);

-- Executions table
CREATE TABLE executions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    workflow_id UUID NOT NULL REFERENCES workflows(id),
    workflow_version INTEGER NOT NULL,
    state execution_state NOT NULL DEFAULT 'pending',
    initial_data JSONB,
    started_at TIMESTAMP WITH TIME ZONE NOT NULL,
    finished_at TIMESTAMP WITH TIME ZONE,
    duration_ms BIGINT,
    initiated_by UUID,
    priority priority_level NOT NULL DEFAULT 'normal',
    attempt INTEGER NOT NULL DEFAULT 1,
    failure_reason TEXT,
    failed_node_id UUID,
    parent_execution_id UUID REFERENCES executions(id),
    worker_id VARCHAR(255),
    tags TEXT[],
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Node execution results table
CREATE TABLE node_execution_results (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    execution_id UUID NOT NULL REFERENCES executions(id) ON DELETE CASCADE,
    node_id UUID NOT NULL,
    state execution_state NOT NULL DEFAULT 'pending',
    started_at TIMESTAMP WITH TIME ZONE NOT NULL,
    finished_at TIMESTAMP WITH TIME ZONE,
    duration_ms BIGINT,
    input_data JSONB NOT NULL DEFAULT '{}',
    output_data JSONB NOT NULL DEFAULT '{}',
    failure_reason TEXT,
    attempt INTEGER NOT NULL DEFAULT 1,
    worker_id VARCHAR(255),
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    UNIQUE (execution_id, node_id)
);

-- Execution logs table
CREATE TABLE execution_logs (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    execution_id UUID NOT NULL REFERENCES executions(id) ON DELETE CASCADE,
    timestamp TIMESTAMP WITH TIME ZONE NOT NULL,
    level log_level NOT NULL,
    message TEXT NOT NULL,
    node_id UUID,
    metadata JSONB,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Variables table
CREATE TABLE variables (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    key VARCHAR(255) NOT NULL,
    value TEXT NOT NULL,
    var_type variable_type NOT NULL,
    scope_type VARCHAR(50) NOT NULL, -- 'global', 'workflow', 'user', 'environment'
    scope_id UUID, -- NULL for global and environment
    environment_name VARCHAR(255), -- Only for environment scope
    protected BOOLEAN NOT NULL DEFAULT FALSE,
    description TEXT,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    UNIQUE (key, scope_type, scope_id, environment_name)
);

-- Credentials table
CREATE TABLE credentials (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name VARCHAR(255) NOT NULL,
    type_name VARCHAR(100) NOT NULL,
    data JSONB NOT NULL,
    owner_id UUID,
    shared BOOLEAN NOT NULL DEFAULT FALSE,
    notes TEXT,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Node credentials table (mapping between nodes and credentials)
CREATE TABLE node_credentials (
    node_id UUID NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    credential_id UUID NOT NULL REFERENCES credentials(id) ON DELETE CASCADE,
    PRIMARY KEY (node_id, credential_id)
);

-- Users table
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    username VARCHAR(50) NOT NULL UNIQUE,
    email VARCHAR(255) NOT NULL UNIQUE,
    full_name VARCHAR(100) NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    role user_role NOT NULL DEFAULT 'user',
    active BOOLEAN NOT NULL DEFAULT TRUE,
    last_login TIMESTAMP WITH TIME ZONE,
    preferences JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Tags table
CREATE TABLE tags (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name VARCHAR(50) NOT NULL UNIQUE,
    color tag_color NOT NULL DEFAULT 'blue',
    description TEXT,
    created_by UUID REFERENCES users(id),
    system_tag BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Workflow tags table (mapping between workflows and tags)
CREATE TABLE workflow_tags (
    workflow_id UUID NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
    tag_id UUID NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (workflow_id, tag_id)
);

-- Create indexes
CREATE INDEX idx_workflows_created_by ON workflows(created_by);
CREATE INDEX idx_nodes_workflow_id ON nodes(workflow_id);
CREATE INDEX idx_connections_workflow_id ON connections(workflow_id);
CREATE INDEX idx_executions_workflow_id ON executions(workflow_id);
CREATE INDEX idx_executions_state ON executions(state);
CREATE INDEX idx_node_execution_results_execution_id ON node_execution_results(execution_id);
CREATE INDEX idx_execution_logs_execution_id ON execution_logs(execution_id);
CREATE INDEX idx_variables_scope ON variables(scope_type, scope_id);
CREATE INDEX idx_credentials_owner_id ON credentials(owner_id);
CREATE INDEX idx_workflow_tags_workflow_id ON workflow_tags(workflow_id);
CREATE INDEX idx_workflow_tags_tag_id ON workflow_tags(tag_id);

-- Add history tables for version control
CREATE TABLE workflow_history (
    history_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    id UUID NOT NULL,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    version INTEGER NOT NULL,
    active BOOLEAN NOT NULL,
    static_data BOOLEAN NOT NULL,
    settings JSONB NOT NULL,
    metadata JSONB,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL,
    created_by UUID,
    recorded_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Create function and trigger for workflow versioning
CREATE OR REPLACE FUNCTION log_workflow_history()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO workflow_history (
        id, name, description, version, active, static_data, 
        settings, metadata, created_at, updated_at, created_by
    ) VALUES (
        OLD.id, OLD.name, OLD.description, OLD.version, OLD.active, OLD.static_data,
        OLD.settings, OLD.metadata, OLD.created_at, OLD.updated_at, OLD.created_by
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER workflow_history_trigger
AFTER UPDATE ON workflows
FOR EACH ROW
WHEN (OLD.version <> NEW.version)
EXECUTE FUNCTION log_workflow_history();
"#;