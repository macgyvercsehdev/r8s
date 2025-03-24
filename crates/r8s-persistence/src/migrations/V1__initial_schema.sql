// crates/r8s-persistence/src/migrations/V1__initial_schema.sql
-- Initial schema for r8s

-- Create UUID extension
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- Users table
CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY,
    username VARCHAR(50) UNIQUE NOT NULL,
    email VARCHAR(255) UNIQUE NOT NULL,
    full_name VARCHAR(100) NOT NULL,
    password_hash TEXT NOT NULL,
    role VARCHAR(20) NOT NULL DEFAULT 'user',
    active BOOLEAN NOT NULL DEFAULT true,
    last_login TIMESTAMP WITH TIME ZONE,
    preferences JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL
);

-- Tags table
CREATE TABLE IF NOT EXISTS tags (
    id UUID PRIMARY KEY,
    name VARCHAR(50) NOT NULL,
    color VARCHAR(20) NOT NULL,
    description TEXT,
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    system_tag BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL,
    UNIQUE(name)
);

-- Workflows table
CREATE TABLE IF NOT EXISTS workflows (
    id UUID PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    version INT NOT NULL DEFAULT 1,
    active BOOLEAN NOT NULL DEFAULT false,
    static_data JSONB,
    settings JSONB NOT NULL DEFAULT '{}',
    metadata JSONB NOT NULL DEFAULT '{}',
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL
);

-- Workflow versions table (for version history)
CREATE TABLE IF NOT EXISTS workflow_versions (
    workflow_id UUID NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
    version INT NOT NULL,
    data JSONB NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    PRIMARY KEY (workflow_id, version)
);

-- Workflow tags table
CREATE TABLE IF NOT EXISTS workflow_tags (
    workflow_id UUID NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
    tag_id UUID NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (workflow_id, tag_id)
);

-- Workflow collaborators table
CREATE TABLE IF NOT EXISTS workflow_collaborators (
    workflow_id UUID NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    permission VARCHAR(20) NOT NULL DEFAULT 'read',
    PRIMARY KEY (workflow_id, user_id)
);

-- Nodes table
CREATE TABLE IF NOT EXISTS nodes (
    id UUID PRIMARY KEY,
    workflow_id UUID NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    type_name VARCHAR(100) NOT NULL,
    type_version VARCHAR(20) NOT NULL,
    type_category VARCHAR(50) NOT NULL,
    plugin_id UUID,
    position_x FLOAT NOT NULL,
    position_y FLOAT NOT NULL,
    parameters JSONB NOT NULL DEFAULT '{}',
    disabled BOOLEAN NOT NULL DEFAULT false,
    settings JSONB NOT NULL DEFAULT '{}',
    metadata JSONB NOT NULL DEFAULT '{}',
    notes TEXT,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL
);

-- Node ports table
CREATE TABLE IF NOT EXISTS node_ports (
    id UUID PRIMARY KEY,
    node_id UUID NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    is_input BOOLEAN NOT NULL,
    data_type VARCHAR(50) NOT NULL,
    description TEXT,
    required BOOLEAN NOT NULL DEFAULT false,
    examples JSONB NOT NULL DEFAULT '[]',
    schema JSONB,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL
);

-- Connections table
CREATE TABLE IF NOT EXISTS connections (
    id UUID PRIMARY KEY,
    workflow_id UUID NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
    source_node UUID NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    source_output VARCHAR(255) NOT NULL,
    target_node UUID NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    target_input VARCHAR(255) NOT NULL,
    condition_type VARCHAR(50),
    condition_expression TEXT,
    transform TEXT,
    metadata JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL
);

-- Credentials table
CREATE TABLE IF NOT EXISTS credentials (
    id UUID PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    type_name VARCHAR(100) NOT NULL,
    data JSONB NOT NULL,
    owner_id UUID REFERENCES users(id) ON DELETE CASCADE,
    shared BOOLEAN NOT NULL DEFAULT false,
    notes TEXT,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL
);

-- Node credentials table
CREATE TABLE IF NOT EXISTS node_credentials (
    node_id UUID NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    credential_id UUID NOT NULL REFERENCES credentials(id) ON DELETE CASCADE,
    PRIMARY KEY (node_id, credential_id)
);

-- Variables table
CREATE TABLE IF NOT EXISTS variables (
    id UUID PRIMARY KEY,
    key VARCHAR(255) NOT NULL,
    value TEXT NOT NULL,
    var_type VARCHAR(20) NOT NULL,
    scope_type VARCHAR(20) NOT NULL,
    scope_id UUID,
    scope_id_text TEXT,
    protected BOOLEAN NOT NULL DEFAULT false,
    description TEXT,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL,
    UNIQUE (key, scope_type, scope_id, scope_id_text)
);

-- Executions table
CREATE TABLE IF NOT EXISTS executions (
    id UUID PRIMARY KEY,
    workflow_id UUID NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
    workflow_version INT NOT NULL,
    state VARCHAR(20) NOT NULL,
    initial_data JSONB,
    started_at TIMESTAMP WITH TIME ZONE NOT NULL,
    finished_at TIMESTAMP WITH TIME ZONE,
    duration_ms BIGINT,
    initiated_by UUID REFERENCES users(id) ON DELETE SET NULL,
    priority VARCHAR(20) NOT NULL DEFAULT 'normal',
    attempt INT NOT NULL DEFAULT 1,
    failure_reason TEXT,
    failed_node_id UUID REFERENCES nodes(id) ON DELETE SET NULL,
    parent_execution_id UUID REFERENCES executions(id) ON DELETE SET NULL,
    worker_id TEXT,
    tags JSONB NOT NULL DEFAULT '[]',
    created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL
);

-- Execution children table
CREATE TABLE IF NOT EXISTS execution_children (
    parent_id UUID NOT NULL REFERENCES executions(id) ON DELETE CASCADE,
    child_id UUID NOT NULL REFERENCES executions(id) ON DELETE CASCADE,
    PRIMARY KEY (parent_id, child_id)
);

-- Node execution results table
CREATE TABLE IF NOT EXISTS node_execution_results (
    execution_id UUID NOT NULL REFERENCES executions(id) ON DELETE CASCADE,
    node_id UUID NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    state VARCHAR(20) NOT NULL,
    started_at TIMESTAMP WITH TIME ZONE NOT NULL,
    finished_at TIMESTAMP WITH TIME ZONE,
    duration_ms BIGINT,
    input_data JSONB NOT NULL DEFAULT '{}',
    output_data JSONB NOT NULL DEFAULT '{}',
    failure_reason TEXT,
    attempt INT NOT NULL DEFAULT 1,
    worker_id TEXT,
    PRIMARY KEY (execution_id, node_id)
);

-- Execution logs table
CREATE TABLE IF NOT EXISTS execution_logs (
    execution_id UUID NOT NULL REFERENCES executions(id) ON DELETE CASCADE,
    timestamp TIMESTAMP WITH TIME ZONE NOT NULL,
    level VARCHAR(10) NOT NULL,
    message TEXT NOT NULL,
    node_id UUID REFERENCES nodes(id) ON DELETE SET NULL,
    metadata JSONB
);

-- Create indexes for performance
CREATE INDEX idx_workflows_created_by ON workflows(created_by);
CREATE INDEX idx_workflows_active ON workflows(active);
CREATE INDEX idx_nodes_workflow_id ON nodes(workflow_id);
CREATE INDEX idx_connections_workflow_id ON connections(workflow_id);
CREATE INDEX idx_connections_source_node ON connections(source_node);
CREATE INDEX idx_connections_target_node ON connections(target_node);
CREATE INDEX idx_credentials_owner_id ON credentials(owner_id);
CREATE INDEX idx_credentials_shared ON credentials(shared);
CREATE INDEX idx_executions_workflow_id ON executions(workflow_id);
CREATE INDEX idx_executions_state ON executions(state);
CREATE INDEX idx_executions_started_at ON executions(started_at);
CREATE INDEX idx_node_execution_results_execution_id ON node_execution_results(execution_id);
CREATE INDEX idx_execution_logs_execution_id ON execution_logs(execution_id);
CREATE INDEX idx_execution_logs_timestamp ON execution_logs(timestamp);