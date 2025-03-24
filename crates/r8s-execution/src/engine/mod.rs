// r8s-execution/src/engine/mod.rs
//! Workflow execution engine.
//!
//! This module provides the core functionality for executing workflows
//! by orchestrating node execution and data flow.

mod execution_plan;
mod execution_context;

use std::sync::Arc;
use std::collections::{HashMap, HashSet};
use async_trait::async_trait;
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, error, info, warn};
use futures::stream::{self, StreamExt};

use r8s_core::common::{EntityId, ExecutionState};
use r8s_core::entity::{Workflow, Node, Connection, Execution, NodeExecutionResult, ExecutionLog, LogLevel};
use r8s_core::repository::{WorkflowRepository, ExecutionRepository};

use crate::error::{ExecutionError, Result};
use crate::executor::NodeExecutor;
use crate::worker::Worker;
use crate::scheduler::ExecutionScheduler;

use self::execution_plan::ExecutionPlan;
use self::execution_context::ExecutionContext;

/// Workflow execution engine responsible for running workflows.
pub struct WorkflowEngine<W, E, N> 
where
    W: WorkflowRepository + Send + Sync + 'static,
    E: ExecutionRepository + Send + Sync + 'static,
    N: NodeExecutor + Send + Sync + 'static,
{
    workflow_repository: Arc<W>,
    execution_repository: Arc<E>,
    node_executor: Arc<N>,
    workers: Arc<Mutex<HashMap<String, Arc<dyn Worker>>>>,
    scheduler: Arc<dyn ExecutionScheduler>,
    execution_pool_size: usize,
}

impl<W, E, N> WorkflowEngine<W, E, N>
where
    W: WorkflowRepository + Send + Sync + 'static,
    E: ExecutionRepository + Send + Sync + 'static,
    N: NodeExecutor + Send + Sync + 'static,
{
    /// Create a new workflow engine.
    pub fn new(
        workflow_repository: Arc<W>,
        execution_repository: Arc<E>,
        node_executor: Arc<N>,
        scheduler: Arc<dyn ExecutionScheduler>,
        execution_pool_size: usize,
    ) -> Self {
        Self {
            workflow_repository,
            execution_repository,
            node_executor,
            workers: Arc::new(Mutex::new(HashMap::new())),
            scheduler,
            execution_pool_size,
        }
    }

    /// Register a worker with the engine.
    pub async fn register_worker(&self, worker_id: String, worker: Arc<dyn Worker>) {
        let mut workers = self.workers.lock().await;
        workers.insert(worker_id, worker);
    }

    /// Unregister a worker from the engine.
    pub async fn unregister_worker(&self, worker_id: &str) {
        let mut workers = self.workers.lock().await;
        workers.remove(worker_id);
    }

    /// Start execution of a workflow.
    pub async fn start_execution(&self, execution_id: EntityId) -> Result<()> {
        // Retrieve the execution from the repository
        let mut execution = self.execution_repository.find_by_id(execution_id).await
            .map_err(ExecutionError::CoreError)?;
        
        // Get the workflow definition
        let workflow = self.workflow_repository.find_by_id(execution.workflow_id).await
            .map_err(ExecutionError::CoreError)?;
        
        // Create execution plan
        let plan = ExecutionPlan::new(&workflow)?;
        
        // Start the execution
        execution.start(Some("workflow-engine".to_string()));
        self.execution_repository.save(execution.clone()).await
            .map_err(ExecutionError::CoreError)?;
        
        // Submit to the scheduler for execution
        self.scheduler.schedule_execution(execution_id, plan).await
            .map_err(|e| ExecutionError::SchedulerError(e.to_string()))?;
        
        Ok(())
    }

    /// Execute a workflow directly (used by scheduler).
    pub async fn execute_workflow(&self, execution_id: EntityId, plan: ExecutionPlan) -> Result<()> {
        // Retrieve the execution
        let mut execution = self.execution_repository.find_by_id(execution_id).await
            .map_err(ExecutionError::CoreError)?;
        
        // Get the workflow
        let workflow = self.workflow_repository.find_by_id(execution.workflow_id).await
            .map_err(ExecutionError::CoreError)?;
        
        // Create execution context
        let context = ExecutionContext::new(
            execution_id,
            workflow.clone(),
            execution.initial_data.clone(),
        );
        
        // Execute workflow with retry logic
        let result = match self.execute_with_retry(&context, &plan, execution.attempt).await {
            Ok(_) => {
                debug!("Workflow execution completed successfully: {}", execution_id);
                execution.complete();
                self.execution_repository.save(execution).await
                    .map_err(ExecutionError::CoreError)?;
                Ok(())
            }
            Err(err) => {
                error!("Workflow execution failed: {}", err);
                execution.fail(err.to_string(), None);
                self.execution_repository.save(execution).await
                    .map_err(ExecutionError::CoreError)?;
                Err(err)
            }
        };
        
        result
    }

    /// Execute workflow with retry logic.
    async fn execute_with_retry(
        &self,
        context: &ExecutionContext,
        plan: &ExecutionPlan,
        attempt: u32,
    ) -> Result<()> {
        // Try to execute the workflow
        let result = self.execute_nodes(context, plan).await;
        
        // If execution failed and retries are available, reschedule with backoff
        if let Err(err) = &result {
            let workflow = &context.workflow;
            let retry_policy = &workflow.settings.retry_policy;
            
            if attempt <= retry_policy.max_attempts {
                let next_attempt = attempt + 1;
                let backoff_seconds = Self::calculate_backoff(
                    retry_policy.initial_interval_seconds,
                    retry_policy.backoff_factor,
                    retry_policy.max_interval_seconds,
                    attempt,
                );
                
                warn!(
                    "Execution failed, scheduling retry {}/{} after {} seconds: {}",
                    next_attempt, retry_policy.max_attempts, backoff_seconds, err
                );
                
                // Update execution for retry
                let mut execution = self.execution_repository.find_by_id(context.execution_id).await
                    .map_err(ExecutionError::CoreError)?;
                    
                execution.attempt = next_attempt;
                self.execution_repository.save(execution).await
                    .map_err(ExecutionError::CoreError)?;
                
                // Schedule the retry
                self.scheduler.schedule_execution_with_delay(
                    context.execution_id,
                    plan.clone(),
                    backoff_seconds,
                ).await
                    .map_err(|e| ExecutionError::SchedulerError(e.to_string()))?;
                
                // Return early, the retry will be handled later
                return Ok(());
            }
        }
        
        // Return the original result
        result
    }

    /// Calculate the backoff time for retries.
    fn calculate_backoff(
        initial_seconds: u32,
        factor: f32,
        max_seconds: u32,
        attempt: u32,
    ) -> u32 {
        let backoff = (initial_seconds as f32) * factor.powi((attempt - 1) as i32);
        backoff.min(max_seconds as f32) as u32
    }

    /// Execute nodes according to the execution plan.
    async fn execute_nodes(&self, context: &ExecutionContext, plan: &ExecutionPlan) -> Result<()> {
        let execution_id = context.execution_id;
        
        // Get initial node set to execute (nodes without dependencies)
        let initial_nodes = plan.get_initial_nodes();
        if initial_nodes.is_empty() {
            return Err(ExecutionError::ExecutionError(
                "No starting nodes found in workflow".to_string()
            ));
        }
        
        // Execute nodes in topological order
        let mut executed_nodes = HashSet::new();
        let mut node_pool = initial_nodes;
        
        while !node_pool.is_empty() {
            let nodes_to_execute: Vec<_> = node_pool.drain().collect();
            debug!("Executing node batch, size: {}", nodes_to_execute.len());
            
            // Execute nodes in parallel with a semaphore to limit concurrency
            let results = stream::iter(nodes_to_execute)
                .map(|node_id| {
                    let node_executor = Arc::clone(&self.node_executor);
                    let execution_repo = Arc::clone(&self.execution_repository);
                    let context = context.clone();
                    
                    async move {
                        let node = context.workflow.nodes.iter()
                            .find(|n| n.id == node_id)
                            .ok_or_else(|| ExecutionError::ExecutionError(
                                format!("Node {} not found in workflow", node_id)
                            ))?;
                        
                        // Prepare inputs for the node
                        let inputs = self.prepare_node_inputs(&context, plan, node_id).await?;
                        
                        // Execute the node
                        let result = node_executor.execute_node(
                            context.execution_id,
                            node,
                            inputs,
                        ).await;
                        
                        // Record the result
                        match result {
                            Ok(node_result) => {
                                // Record successful execution
                                execution_repo.add_node_result(execution_id, node_result.clone()).await
                                    .map_err(ExecutionError::CoreError)?;
                                
                                Ok((node_id, node_result))
                            }
                            Err(err) => {
                                // Create failure result
                                let mut node_result = NodeExecutionResult::new(node_id);
                                node_result.fail(err.to_string());
                                
                                // Record the failure
                                execution_repo.add_node_result(execution_id, node_result.clone()).await
                                    .map_err(ExecutionError::CoreError)?;
                                
                                // Add error log
                                let log = ExecutionLog {
                                    timestamp: chrono::Utc::now(),
                                    level: LogLevel::Error,
                                    message: format!("Node execution failed: {}", err),
                                    node_id: Some(node_id),
                                    metadata: None,
                                };
                                
                                execution_repo.add_logs(execution_id, vec![log]).await
                                    .map_err(ExecutionError::CoreError)?;
                                
                                Err(err)
                            }
                        }
                    }
                })
                .buffer_unordered(self.execution_pool_size)
                .collect::<Vec<_>>()
                .await;
            
            // Process results and find next nodes to execute
            for result in results {
                match result {
                    Ok((node_id, _)) => {
                        executed_nodes.insert(node_id);
                        
                        // Add next nodes to the execution pool
                        for next_node_id in plan.get_dependent_nodes(node_id) {
                            // Only add a node if all its dependencies have been executed
                            let dependencies = plan.get_node_dependencies(next_node_id);
                            if dependencies.iter().all(|dep| executed_nodes.contains(dep)) {
                                node_pool.insert(next_node_id);
                            }
                        }
                    }
                    Err(err) => {
                        // If any node fails, stop execution
                        return Err(err);
                    }
                }
            }
        }
        
        // Success - all nodes executed
        Ok(())
    }

    /// Prepare inputs for a node based on connection and execution context.
    async fn prepare_node_inputs(
        &self,
        context: &ExecutionContext,
        plan: &ExecutionPlan,
        node_id: EntityId,
    ) -> Result<HashMap<String, serde_json::Value>> {
        let mut inputs = HashMap::new();
        
        // Get the node
        let node = context.workflow.nodes.iter()
            .find(|n| n.id == node_id)
            .ok_or_else(|| ExecutionError::ExecutionError(
                format!("Node {} not found in workflow", node_id)
            ))?;
        
        // For trigger/start nodes, use the initial data if available
        if plan.is_initial_node(node_id) && context.initial_data.is_some() {
            let initial_data = context.initial_data.as_ref().unwrap();
            
            // Map initial data to node inputs
            if let serde_json::Value::Object(obj) = initial_data {
                for input in &node.inputs {
                    if let Some(value) = obj.get(&input.name) {
                        inputs.insert(input.name.clone(), value.clone());
                    }
                }
            }
            
            return Ok(inputs);
        }
        
        // For other nodes, get inputs from incoming connections
        let execution = self.execution_repository.find_by_id(context.execution_id).await
            .map_err(ExecutionError::CoreError)?;
        
        let connections: Vec<&Connection> = context.workflow.connections.iter()
            .filter(|c| c.target_node == node_id)
            .collect();
        
        for connection in connections {
            // Get the source node result
            let source_result = execution.results.get(&connection.source_node)
                .ok_or_else(|| ExecutionError::ExecutionError(
                    format!("Missing execution result for source node {}", connection.source_node)
                ))?;
            
            // Get the output value from the source node
            let output_value = source_result.output_data.get(&connection.source_output)
                .ok_or_else(|| ExecutionError::ExecutionError(
                    format!(
                        "Missing output '{}' in source node {}",
                        connection.source_output, connection.source_node
                    )
                ))?;
            
            // Apply any transformations defined in the connection
            let final_value = if let Some(transform) = &connection.transform {
                self.apply_transform(output_value, transform)?
            } else {
                output_value.clone()
            };
            
            // Store the value in the inputs map
            inputs.insert(connection.target_input.clone(), final_value);
        }
        
        // Check if all required inputs are satisfied
        for input in &node.inputs {
            if input.required && !inputs.contains_key(&input.name) {
                return Err(ExecutionError::MissingInput {
                    node_id,
                    input_name: input.name.clone(),
                });
            }
        }
        
        Ok(inputs)
    }

    /// Apply a transformation to data.
    fn apply_transform(
        &self,
        value: &serde_json::Value,
        transform_script: &str,
    ) -> Result<serde_json::Value> {
        // For now, just return the original value
        // In a real implementation, we would evaluate the transform script
        // using JavaScript or a similar mechanism
        Ok(value.clone())
    }

    /// Cancel an execution.
    pub async fn cancel_execution(&self, execution_id: EntityId, reason: Option<String>) -> Result<()> {
        // Get the execution
        let mut execution = self.execution_repository.find_by_id(execution_id).await
            .map_err(ExecutionError::CoreError)?;
        
        // Can only cancel if not already completed
        if execution.is_finished() {
            return Err(ExecutionError::ExecutionError(
                format!("Cannot cancel execution {} that is already finished", execution_id)
            ));
        }
        
        // Cancel the execution
        execution.cancel(reason);
        self.execution_repository.save(execution).await
            .map_err(ExecutionError::CoreError)?;
        
        // Notify the scheduler
        self.scheduler.cancel_execution(execution_id).await
            .map_err(|e| ExecutionError::SchedulerError(e.to_string()))?;
        
        Ok(())
    }
}