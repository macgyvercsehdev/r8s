// r8s-persistence/src/models/execution.rs
//! Database model for workflow executions.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::uuid::Uuid;
use sqlx::types::JsonValue;
use std::collections::HashMap;

use r8s_core::common::{EntityId, ExecutionState, Priority, Timestamp};
use r8s_core::entity::{Execution, ExecutionLog, LogLevel, NodeExecutionResult};

use crate::error::{PersistenceError, Result};

/// Database model for workflow executions.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ExecutionModel {
    /// Primary key
    pub id: Uuid,

    /// Foreign key to workflow
    pub workflow_id: Uuid,

    /// Workflow version being executed
    pub workflow_version: i32,

    /// Execution state
    pub state: String,

    /// Initial data provided to the execution
    pub initial_data: Option<JsonValue>,

    /// When the execution started
    pub started_at: DateTime<Utc>,

    /// When the execution finished (if it has)
    pub finished_at: Option<DateTime<Utc>>,

    /// Duration in milliseconds
    pub duration_ms: Option<i64>,

    /// User who initiated the execution
    pub initiated_by: Option<Uuid>,

    /// Priority level
    pub priority: String,

    /// Attempt number
    pub attempt: i32,

    /// Reason for failure (if failed)
    pub failure_reason: Option<String>,

    /// Node that failed (if any)
    pub failed_node_id: Option<Uuid>,

    /// Parent execution ID (if this is a child execution)
    pub parent_execution_id: Option<Uuid>,

    /// Worker ID processing this execution
    pub worker_id: Option<String>,

    /// Tags for this execution
    pub tags: Option<Vec<String>>,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
}

/// Database model for node execution results.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct NodeExecutionResultModel {
    /// Primary key
    pub id: Uuid,

    /// Foreign key to execution
    pub execution_id: Uuid,

    /// Node ID
    pub node_id: Uuid,

    /// Execution state
    pub state: String,

    /// When the node execution started
    pub started_at: DateTime<Utc>,

    /// When the node execution finished (if it has)
    pub finished_at: Option<DateTime<Utc>>,

    /// Duration in milliseconds
    pub duration_ms: Option<i64>,

    /// Input data
    pub input_data: JsonValue,

    /// Output data
    pub output_data: JsonValue,

    /// Reason for failure (if failed)
    pub failure_reason: Option<String>,

    /// Attempt number
    pub attempt: i32,

    /// Worker ID that processed this node
    pub worker_id: Option<String>,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
}

/// Database model for execution logs.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ExecutionLogModel {
    /// Primary key
    pub id: Uuid,

    /// Foreign key to execution
    pub execution_id: Uuid,

    /// Log timestamp
    pub timestamp: DateTime<Utc>,

    /// Log level
    pub level: String,

    /// Log message
    pub message: String,

    /// Associated node ID (if any)
    pub node_id: Option<Uuid>,

    /// Additional metadata
    pub metadata: Option<JsonValue>,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,
}

/// Map an execution state string from the database to the domain enum.
pub fn map_execution_state_from_db(state: &str) -> Result<ExecutionState> {
    match state {
        "pending" => Ok(ExecutionState::Pending),
        "running" => Ok(ExecutionState::Running),
        "waiting" => Ok(ExecutionState::Waiting),
        "success" => Ok(ExecutionState::Success),
        "failed" => Ok(ExecutionState::Failed),
        "cancelled" => Ok(ExecutionState::Cancelled),
        _ => Err(PersistenceError::ConversionError(format!(
            "Invalid execution state: {}",
            state
        ))),
    }
}

/// Map a domain enum execution state to a database string.
pub fn map_execution_state_to_db(state: &ExecutionState) -> &'static str {
    match state {
        ExecutionState::Pending => "pending",
        ExecutionState::Running => "running",
        ExecutionState::Waiting => "waiting",
        ExecutionState::Success => "success",
        ExecutionState::Failed => "failed",
        ExecutionState::Cancelled => "cancelled",
    }
}

/// Map a priority string from the database to the domain enum.
pub fn map_priority_from_db(priority: &str) -> Result<Priority> {
    match priority {
        "low" => Ok(Priority::Low),
        "normal" => Ok(Priority::Normal),
        "high" => Ok(Priority::High),
        "critical" => Ok(Priority::Critical),
        _ => Err(PersistenceError::ConversionError(format!(
            "Invalid priority: {}",
            priority
        ))),
    }
}

/// Map a domain enum priority to a database string.
pub fn map_priority_to_db(priority: &Priority) -> &'static str {
    match priority {
        Priority::Low => "low",
        Priority::Normal => "normal",
        Priority::High => "high",
        Priority::Critical => "critical",
    }
}

/// Map a log level string from the database to the domain enum.
pub fn map_log_level_from_db(level: &str) -> Result<LogLevel> {
    match level {
        "debug" => Ok(LogLevel::Debug),
        "info" => Ok(LogLevel::Info),
        "warning" => Ok(LogLevel::Warning),
        "error" => Ok(LogLevel::Error),
        _ => Err(PersistenceError::ConversionError(format!(
            "Invalid log level: {}",
            level
        ))),
    }
}

/// Map a domain enum log level to a database string.
pub fn map_log_level_to_db(level: &LogLevel) -> &'static str {
    match level {
        LogLevel::Debug => "debug",
        LogLevel::Info => "info",
        LogLevel::Warning => "warning",
        LogLevel::Error => "error",
    }
}

impl ExecutionModel {
    /// Convert a domain Execution entity to a database model.
    pub fn from_entity(
        entity: &Execution,
    ) -> Result<(Self, Vec<NodeExecutionResultModel>, Vec<ExecutionLogModel>)> {
        let initial_data = entity.initial_data.clone();

        let model = Self {
            id: Uuid::from_bytes(entity.id.as_bytes()),
            workflow_id: Uuid::from_bytes(entity.workflow_id.as_bytes()),
            workflow_version: entity.workflow_version as i32,
            state: map_execution_state_to_db(&entity.state).to_string(),
            initial_data,
            started_at: entity.started_at,
            finished_at: entity.finished_at,
            duration_ms: entity.duration_ms.map(|d| d as i64),
            initiated_by: entity
                .initiated_by
                .map(|id| Uuid::from_bytes(id.as_bytes())),
            priority: map_priority_to_db(&entity.priority).to_string(),
            attempt: entity.attempt as i32,
            failure_reason: entity.failure_reason.clone(),
            failed_node_id: entity
                .failed_node_id
                .map(|id| Uuid::from_bytes(id.as_bytes())),
            parent_execution_id: entity
                .parent_execution_id
                .map(|id| Uuid::from_bytes(id.as_bytes())),
            worker_id: entity.worker_id.clone(),
            tags: if entity.tags.is_empty() {
                None
            } else {
                Some(entity.tags.clone())
            },
            created_at: entity.timestamps.created_at,
            updated_at: entity.timestamps.updated_at,
        };

        // Convert node results
        let mut node_results = Vec::new();
        for (node_id, result) in &entity.results {
            node_results.push(NodeExecutionResultModel {
                id: Uuid::new_v4(),
                execution_id: model.id,
                node_id: Uuid::from_bytes(node_id.as_bytes()),
                state: map_execution_state_to_db(&result.state).to_string(),
                started_at: result.started_at,
                finished_at: result.finished_at,
                duration_ms: result.duration_ms.map(|d| d as i64),
                input_data: serde_json::to_value(&result.input_data)
                    .map_err(|e| PersistenceError::SerializationError(e.to_string()))?,
                output_data: serde_json::to_value(&result.output_data)
                    .map_err(|e| PersistenceError::SerializationError(e.to_string()))?,
                failure_reason: result.failure_reason.clone(),
                attempt: result.attempt as i32,
                worker_id: result.worker_id.clone(),
                created_at: entity.timestamps.created_at,
                updated_at: entity.timestamps.updated_at,
            });
        }

        // Convert logs
        let mut logs = Vec::new();
        for log in &entity.logs {
            logs.push(ExecutionLogModel {
                id: Uuid::new_v4(),
                execution_id: model.id,
                timestamp: log.timestamp,
                level: map_log_level_to_db(&log.level).to_string(),
                message: log.message.clone(),
                node_id: log.node_id.map(|id| Uuid::from_bytes(id.as_bytes())),
                metadata: log.metadata.clone(),
                created_at: entity.timestamps.created_at,
            });
        }

        Ok((model, node_results, logs))
    }

    /// Convert this database model to a domain Execution entity.
    pub fn to_entity(
        &self,
        node_results: Vec<NodeExecutionResultModel>,
        logs: Vec<ExecutionLogModel>,
    ) -> Result<Execution> {
        let state = map_execution_state_from_db(&self.state)?;
        let priority = map_priority_from_db(&self.priority)?;

        let timestamps = Timestamp {
            created_at: self.created_at,
            updated_at: self.updated_at,
        };

        // Convert node results
        let mut results = HashMap::new();
        for result in node_results {
            let node_id = EntityId::from_bytes(result.node_id.as_bytes());

            let input_data: HashMap<String, serde_json::Value> =
                serde_json::from_value(result.input_data)
                    .map_err(|e| PersistenceError::SerializationError(e.to_string()))?;

            let output_data: HashMap<String, serde_json::Value> =
                serde_json::from_value(result.output_data)
                    .map_err(|e| PersistenceError::SerializationError(e.to_string()))?;

            let node_result = NodeExecutionResult {
                node_id,
                state: map_execution_state_from_db(&result.state)?,
                started_at: result.started_at,
                finished_at: result.finished_at,
                duration_ms: result.duration_ms.map(|d| d as u64),
                input_data,
                output_data,
                failure_reason: result.failure_reason,
                attempt: result.attempt as u32,
                worker_id: result.worker_id,
            };

            results.insert(node_id, node_result);
        }

        // Convert logs
        let mut execution_logs = Vec::new();
        for log in logs {
            execution_logs.push(ExecutionLog {
                timestamp: log.timestamp,
                level: map_log_level_from_db(&log.level)?,
                message: log.message,
                node_id: log.node_id.map(|id| EntityId::from_bytes(id.as_bytes())),
                metadata: log.metadata,
            });
        }

        let execution = Execution {
            id: EntityId::from_bytes(self.id.as_bytes()),
            workflow_id: EntityId::from_bytes(self.workflow_id.as_bytes()),
            workflow_version: self.workflow_version as u32,
            state,
            results,
            initial_data: self.initial_data.clone(),
            started_at: self.started_at,
            finished_at: self.finished_at,
            duration_ms: self.duration_ms.map(|d| d as u64),
            initiated_by: self
                .initiated_by
                .map(|id| EntityId::from_bytes(id.as_bytes())),
            priority,
            attempt: self.attempt as u32,
            failure_reason: self.failure_reason.clone(),
            failed_node_id: self
                .failed_node_id
                .map(|id| EntityId::from_bytes(id.as_bytes())),
            parent_execution_id: self
                .parent_execution_id
                .map(|id| EntityId::from_bytes(id.as_bytes())),
            child_execution_ids: Vec::new(), // Will be populated separately if needed
            worker_id: self.worker_id.clone(),
            tags: self.tags.clone().unwrap_or_default(),
            logs: execution_logs,
            timestamps,
        };

        Ok(execution)
    }
}

impl NodeExecutionResultModel {
    /// Convert a domain NodeExecutionResult entity to a database model.
    pub fn from_entity(entity: &NodeExecutionResult, execution_id: Uuid) -> Result<Self> {
        Ok(Self {
            id: Uuid::new_v4(),
            execution_id,
            node_id: Uuid::from_bytes(entity.node_id.as_bytes()),
            state: map_execution_state_to_db(&entity.state).to_string(),
            started_at: entity.started_at,
            finished_at: entity.finished_at,
            duration_ms: entity.duration_ms.map(|d| d as i64),
            input_data: serde_json::to_value(&entity.input_data)
                .map_err(|e| PersistenceError::SerializationError(e.to_string()))?,
            output_data: serde_json::to_value(&entity.output_data)
                .map_err(|e| PersistenceError::SerializationError(e.to_string()))?,
            failure_reason: entity.failure_reason.clone(),
            attempt: entity.attempt as i32,
            worker_id: entity.worker_id.clone(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        })
    }

    /// Convert this database model to a domain NodeExecutionResult entity.
    pub fn to_entity(&self) -> Result<NodeExecutionResult> {
        let input_data: HashMap<String, serde_json::Value> =
            serde_json::from_value(self.input_data.clone())
                .map_err(|e| PersistenceError::SerializationError(e.to_string()))?;

        let output_data: HashMap<String, serde_json::Value> =
            serde_json::from_value(self.output_data.clone())
                .map_err(|e| PersistenceError::SerializationError(e.to_string()))?;

        Ok(NodeExecutionResult {
            node_id: EntityId::from_bytes(self.node_id.as_bytes()),
            state: map_execution_state_from_db(&self.state)?,
            started_at: self.started_at,
            finished_at: self.finished_at,
            duration_ms: self.duration_ms.map(|d| d as u64),
            input_data,
            output_data,
            failure_reason: self.failure_reason.clone(),
            attempt: self.attempt as u32,
            worker_id: self.worker_id.clone(),
        })
    }
}

impl ExecutionLogModel {
    /// Convert a domain ExecutionLog entity to a database model.
    pub fn from_entity(entity: &ExecutionLog, execution_id: Uuid) -> Self {
        Self {
            id: Uuid::new_v4(),
            execution_id,
            timestamp: entity.timestamp,
            level: map_log_level_to_db(&entity.level).to_string(),
            message: entity.message.clone(),
            node_id: entity.node_id.map(|id| Uuid::from_bytes(id.as_bytes())),
            metadata: entity.metadata.clone(),
            created_at: chrono::Utc::now(),
        }
    }

    /// Convert this database model to a domain ExecutionLog entity.
    pub fn to_entity(&self) -> Result<ExecutionLog> {
        Ok(ExecutionLog {
            timestamp: self.timestamp,
            level: map_log_level_from_db(&self.level)?,
            message: self.message.clone(),
            node_id: self.node_id.map(|id| EntityId::from_bytes(id.as_bytes())),
            metadata: self.metadata.clone(),
        })
    }
}
