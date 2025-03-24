// r8s-execution/src/scheduler/mod.rs
//! Scheduler for workflow executions.
//!
//! This module provides the scheduler implementation that coordinates
//! the execution of workflows across multiple workers.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use async_trait::async_trait;
use tokio::sync::{Mutex, RwLock};
use tokio::time;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use r8s_core::common::{EntityId, ExecutionState};
use r8s_core::repository::{WorkflowRepository, ExecutionRepository};

use crate::engine::execution_plan::ExecutionPlan;
use crate::error::{ExecutionError, Result};
use crate::worker::{Worker, Job};

/// Scheduled execution type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScheduledExecutionType {
    /// Immediate execution
    Immediate,
    
    /// Delayed execution
    Delayed(std::time::SystemTime),
    
    /// Recurring execution with a cron schedule
    Cron(String),
}

/// Scheduled execution.
#[derive(Debug, Clone)]
pub struct ScheduledExecution {
    /// ID of the execution
    pub execution_id: EntityId,
    
    /// Type of scheduling
    pub execution_type: ScheduledExecutionType,
    
    /// Execution plan
    pub plan: ExecutionPlan,
    
    /// When the execution was scheduled
    pub scheduled_at: std::time::SystemTime,
}

/// Interface for a scheduler that coordinates executions.
#[async_trait]
pub trait ExecutionScheduler: Send + Sync {
    /// Schedule an execution to run immediately.
    async fn schedule_execution(
        &self,
        execution_id: EntityId,
        plan: ExecutionPlan,
    ) -> Result<()>;
    
    /// Schedule an execution to run after a delay.
    async fn schedule_execution_with_delay(
        &self,
        execution_id: EntityId,
        plan: ExecutionPlan,
        delay_seconds: u32,
    ) -> Result<()>;
    
    /// Schedule an execution to run on a cron schedule.
    async fn schedule_execution_with_cron(
        &self,
        execution_id: EntityId,
        plan: ExecutionPlan,
        cron_expression: &str,
    ) -> Result<()>;
    
    /// Cancel a scheduled execution.
    async fn cancel_execution(&self, execution_id: EntityId) -> Result<()>;
    
    /// Get the current status of the scheduler.
    async fn status(&self) -> SchedulerStatus;
}

/// Status of the scheduler.
#[derive(Debug, Clone)]
pub struct SchedulerStatus {
    /// Number of pending executions
    pub pending_executions: usize,
    
    /// Number of running executions
    pub running_executions: usize,
    
    /// Number of workers registered
    pub worker_count: usize,
    
    /// Worker statuses
    pub worker_statuses: Vec<WorkerStatus>,
}

/// Worker status information.
#[derive(Debug, Clone)]
pub struct WorkerStatus {
    /// Worker ID
    pub worker_id: String,
    
    /// Whether the worker is active
    pub active: bool,
    
    /// Number of jobs assigned to this worker
    pub assigned_jobs: usize,
    
    /// Last time the worker reported status
    pub last_status_report: std::time::SystemTime,
}

/// Default execution scheduler implementation.
pub struct DefaultExecutionScheduler<W, E>
where
    W: WorkflowRepository + Send + Sync + 'static,
    E: ExecutionRepository + Send + Sync + 'static,
{
    /// Scheduler state
    state: Arc<Mutex<SchedulerState>>,
    
    /// Workers
    workers: Arc<RwLock<HashMap<String, Arc<dyn Worker>>>>,
    
    /// Repositories
    workflow_repository: Arc<W>,
    execution_repository: Arc<E>,
}

/// Internal state of the scheduler.
struct SchedulerState {
    /// Pending executions queue
    pending_executions: VecDeque<ScheduledExecution>,
    
    /// Delayed executions, sorted by execution time
    delayed_executions: Vec<ScheduledExecution>,
    
    /// Cron-scheduled executions
    cron_executions: Vec<ScheduledExecution>,
    
    /// Currently running executions
    running_executions: HashMap<EntityId, RunningExecution>,
    
    /// Job to worker mapping (which worker is handling which job)
    job_worker_map: HashMap<String, String>,
}

/// Information about a running execution.
#[derive(Debug)]
struct RunningExecution {
    /// ID of the execution
    execution_id: EntityId,
    
    /// Jobs in this execution
    jobs: HashMap<String, Job>,
    
    /// Nodes that have been executed
    executed_nodes: HashSet<EntityId>,
    
    /// When the execution started
    started_at: std::time::SystemTime,
}

impl<W, E> DefaultExecutionScheduler<W, E>
where
    W: WorkflowRepository + Send + Sync + 'static,
    E: ExecutionRepository + Send + Sync + 'static,
{
    /// Create a new execution scheduler.
    pub fn new(
        workflow_repository: Arc<W>,
        execution_repository: Arc<E>,
    ) -> Self {
        Self {
            state: Arc::new(Mutex::new(SchedulerState {
                pending_executions: VecDeque::new(),
                delayed_executions: Vec::new(),
                cron_executions: Vec::new(),
                running_executions: HashMap::new(),
                job_worker_map: HashMap::new(),
            })),
            workers: Arc::new(RwLock::new(HashMap::new())),
            workflow_repository,
            execution_repository,
        }
    }
    
    /// Register a worker with the scheduler.
    pub async fn register_worker(&self, worker: Arc<dyn Worker>) {
        let worker_id = worker.id().to_string();
        info!("Registering worker: {}", worker_id);
        
        let mut workers = self.workers.write().await;
        workers.insert(worker_id, worker);
    }
    
    /// Unregister a worker from the scheduler.
    pub async fn unregister_worker(&self, worker_id: &str) {
        info!("Unregistering worker: {}", worker_id);
        
        let mut workers = self.workers.write().await;
        workers.remove(worker_id);
        
        // Reassign jobs from this worker
        let mut state = self.state.lock().await;
        
        // Find jobs assigned to this worker
        let jobs_to_reassign: Vec<String> = state.job_worker_map.iter()
            .filter(|(_, w_id)| *w_id == worker_id)
            .map(|(job_id, _)| job_id.clone())
            .collect();
        
        // Remove the mapping
        for job_id in &jobs_to_reassign {
            state.job_worker_map.remove(job_id);
        }
        
        // Reassign the jobs
        for job_id in jobs_to_reassign {
            // Find which execution this job belongs to
            for (_, running) in &mut state.running_executions {
                if let Some(job) = running.jobs.remove(&job_id) {
                    // Create a new job for the same node
                    let new_job = Job::new(job.execution_id, job.node_id);
                    
                    // Add to the running execution
                    running.jobs.insert(new_job.id.clone(), new_job.clone());
                    
                    // Add to the pending executions to be reassigned
                    state.pending_executions.push_front(ScheduledExecution {
                        execution_id: job.execution_id,
                        execution_type: ScheduledExecutionType::Immediate,
                        plan: ExecutionPlan::placeholder(), // We don't need the plan for reassignment
                        scheduled_at: std::time::SystemTime::now(),
                    });
                    
                    // Remove the node from executed nodes so it gets reassigned
                    running.executed_nodes.remove(&job.node_id);
                    
                    break;
                }
            }
        }
    }
    
    /// Start the scheduler loop.
    pub async fn start(&self) -> Result<()> {
        info!("Starting execution scheduler");
        
        let poll_interval = Duration::from_secs(1);
        
        loop {
            // Process any pending executions
            self.process_pending_executions().await?;
            
            // Check for delayed executions that are due
            self.check_delayed_executions().await?;
            
            // Check for cron executions that are due
            self.check_cron_executions().await?;
            
            // Wait for the next poll interval
            time::sleep(poll_interval).await;
        }
    }
    
    /// Process pending executions.
    async fn process_pending_executions(&self) -> Result<()> {
        // Get the next pending execution
        let next_execution = {
            let mut state = self.state.lock().await;
            state.pending_executions.pop_front()
        };
        
        if let Some(execution) = next_execution {
            // Assign to a worker
            self.assign_execution_to_worker(execution).await?;
        }
        
        Ok(())
    }
    
    /// Check for delayed executions that are due.
    async fn check_delayed_executions(&self) -> Result<()> {
        let now = std::time::SystemTime::now();
        let mut due_executions = Vec::new();
        
        // Find executions that are due
        {
            let mut state = self.state.lock().await;
            
            // Find all executions that are due
            let mut i = 0;
            while i < state.delayed_executions.len() {
                if let ScheduledExecutionType::Delayed(execution_time) = state.delayed_executions[i].execution_type {
                    if execution_time <= now {
                        // Due for execution
                        let execution = state.delayed_executions.remove(i);
                        due_executions.push(execution);
                        // Don't increment i since we removed an element
                        continue;
                    }
                }
                i += 1;
            }
        }
        
        // Schedule the due executions
        for mut execution in due_executions {
            // Convert to immediate execution
            execution.execution_type = ScheduledExecutionType::Immediate;
            
            // Add to pending executions
            {
                let mut state = self.state.lock().await;
                state.pending_executions.push_back(execution);
            }
        }
        
        Ok(())
    }
    
    /// Check for cron executions that are due.
    async fn check_cron_executions(&self) -> Result<()> {
        // In a real implementation, this would check the cron schedule
        // For now, we'll skip this part
        Ok(())
    }
    
    /// Assign an execution to a worker.
    async fn assign_execution_to_worker(&self, execution: ScheduledExecution) -> Result<()> {
        // Get the execution plan
        let plan = execution.plan;
        
        // Get a list of available workers
        let workers = self.workers.read().await;
        if workers.is_empty() {
            // No workers available, put back in queue
            let mut state = self.state.lock().await;
            state.pending_executions.push_front(execution);
            return Ok(());
        }
        
        // Choose a worker (simple round-robin for now)
        let worker_ids: Vec<_> = workers.keys().cloned().collect();
        let worker_id = &worker_ids[0]; // Just use the first worker
        let worker = workers.get(worker_id).unwrap().clone();
        
        // Get the nodes that need to be executed
        let initial_nodes = plan.get_initial_nodes();
        
        // Create a running execution entry
        let mut running_execution = RunningExecution {
            execution_id: execution.execution_id,
            jobs: HashMap::new(),
            executed_nodes: HashSet::new(),
            started_at: std::time::SystemTime::now(),
        };
        
        // Create jobs for each initial node
        for node_id in initial_nodes {
            let job = Job::new(execution.execution_id, node_id);
            
            // Add to running execution
            running_execution.jobs.insert(job.id.clone(), job.clone());
            
            // Map job to worker
            {
                let mut state = self.state.lock().await;
                state.job_worker_map.insert(job.id.clone(), worker_id.clone());
            }
            
            // Submit to worker
            worker.submit_job(job).await?;
        }
        
        // Add to running executions
        {
            let mut state = self.state.lock().await;
            state.running_executions.insert(execution.execution_id, running_execution);
        }
        
        Ok(())
    }
    
    /// Handle a job completion.
    pub async fn handle_job_completion(
        &self,
        job_id: &str,
        execution_id: EntityId,
        node_id: EntityId,
        success: bool,
    ) -> Result<()> {
        // If job failed, mark the execution as failed
        if !success {
            self.execution_repository.update_state(execution_id, ExecutionState::Failed).await
                .map_err(ExecutionError::CoreError)?;
            
            // Remove from running executions
            let mut state = self.state.lock().await;
            state.running_executions.remove(&execution_id);
            state.job_worker_map.remove(job_id);
            
            return Ok(());
        }
        
        // Update the execution state
        let mut next_nodes = HashSet::new();
        
        {
            let mut state = self.state.lock().await;
            
            // Get the running execution
            let running = state.running_executions.get_mut(&execution_id)
                .ok_or_else(|| ExecutionError::ExecutionError(
                    format!("Running execution {} not found", execution_id)
                ))?;
            
            // Remove the job
            running.jobs.remove(job_id);
            state.job_worker_map.remove(job_id);
            
            // Mark node as executed
            running.executed_nodes.insert(node_id);
            
            // Get the execution plan
            // In a real implementation, we would store this with the running execution
            let execution = self.execution_repository.find_by_id(execution_id).await
                .map_err(ExecutionError::CoreError)?;
                
            let workflow = self.workflow_repository.find_by_id(execution.workflow_id).await
                .map_err(ExecutionError::CoreError)?;
                
            let plan = ExecutionPlan::new(&workflow)?;
            
            // Find dependent nodes that can now be executed
            for dependent_id in plan.get_dependent_nodes(node_id) {
                let dependencies = plan.get_node_dependencies(dependent_id);
                
                // Check if all dependencies have been executed
                if dependencies.iter().all(|dep| running.executed_nodes.contains(dep)) {
                    next_nodes.insert(dependent_id);
                }
            }
            
            // If no more nodes to execute and no running jobs, execution is complete
            if next_nodes.is_empty() && running.jobs.is_empty() {
                // Mark execution as complete
                self.execution_repository.update_state(execution_id, ExecutionState::Success).await
                    .map_err(ExecutionError::CoreError)?;
                
                // Remove from running executions
                state.running_executions.remove(&execution_id);
                
                return Ok(());
            }
        }
        
        // Create jobs for next nodes
        for node_id in next_nodes {
            // Choose a worker
            let workers = self.workers.read().await;
            if workers.is_empty() {
                return Err(ExecutionError::ExecutionError("No workers available".to_string()));
            }
            
            // Simple round-robin
            let worker_ids: Vec<_> = workers.keys().cloned().collect();
            let worker_id = &worker_ids[0]; // Just use the first worker
            let worker = workers.get(worker_id).unwrap().clone();
            
            // Create a job
            let job = Job::new(execution_id, node_id);
            
            // Add to running execution
            {
                let mut state = self.state.lock().await;
                if let Some(running) = state.running_executions.get_mut(&execution_id) {
                    running.jobs.insert(job.id.clone(), job.clone());
                    state.job_worker_map.insert(job.id.clone(), worker_id.clone());
                }
            }
            
            // Submit to worker
            worker.submit_job(job).await?;
        }
        
        Ok(())
    }
}

#[async_trait]
impl<W, E> ExecutionScheduler for DefaultExecutionScheduler<W, E>
where
    W: WorkflowRepository + Send + Sync + 'static,
    E: ExecutionRepository + Send + Sync + 'static,
{
    async fn schedule_execution(
        &self,
        execution_id: EntityId,
        plan: ExecutionPlan,
    ) -> Result<()> {
        let scheduled = ScheduledExecution {
            execution_id,
            execution_type: ScheduledExecutionType::Immediate,
            plan,
            scheduled_at: std::time::SystemTime::now(),
        };
        
        let mut state = self.state.lock().await;
        state.pending_executions.push_back(scheduled);
        
        Ok(())
    }
    
    async fn schedule_execution_with_delay(
        &self,
        execution_id: EntityId,
        plan: ExecutionPlan,
        delay_seconds: u32,
    ) -> Result<()> {
        let now = std::time::SystemTime::now();
        let execution_time = now + Duration::from_secs(delay_seconds as u64);
        
        let scheduled = ScheduledExecution {
            execution_id,
            execution_type: ScheduledExecutionType::Delayed(execution_time),
            plan,
            scheduled_at: now,
        };
        
        let mut state = self.state.lock().await;
        
        // Insert in sorted order by execution time
        let mut i = 0;
        while i < state.delayed_executions.len() {
            if let ScheduledExecutionType::Delayed(time) = state.delayed_executions[i].execution_type {
                if time > execution_time {
                    break;
                }
            }
            i += 1;
        }
        
        state.delayed_executions.insert(i, scheduled);
        
        Ok(())
    }
    
    async fn schedule_execution_with_cron(
        &self,
        execution_id: EntityId,
        plan: ExecutionPlan,
        cron_expression: &str,
    ) -> Result<()> {
        let scheduled = ScheduledExecution {
            execution_id,
            execution_type: ScheduledExecutionType::Cron(cron_expression.to_string()),
            plan,
            scheduled_at: std::time::SystemTime::now(),
        };
        
        let mut state = self.state.lock().await;
        state.cron_executions.push(scheduled);
        
        Ok(())
    }
    
    async fn cancel_execution(&self, execution_id: EntityId) -> Result<()> {
        let mut state = self.state.lock().await;
        
        // Check pending executions
        state.pending_executions.retain(|exec| exec.execution_id != execution_id);
        
        // Check delayed executions
        state.delayed_executions.retain(|exec| exec.execution_id != execution_id);
        
        // Check cron executions
        state.cron_executions.retain(|exec| exec.execution_id != execution_id);
        
        // Check running executions
        if let Some(running) = state.running_executions.remove(&execution_id) {
            // Cancel all jobs
            let workers = self.workers.read().await;
            
            for (job_id, _) in &running.jobs {
                if let Some(worker_id) = state.job_worker_map.get(job_id) {
                    if let Some(worker) = workers.get(worker_id) {
                        let _ = worker.cancel_job(job_id).await;
                    }
                }
                
                state.job_worker_map.remove(job_id);
            }
        }
        
        // Update execution state in repository
        self.execution_repository.update_state(execution_id, ExecutionState::Cancelled).await
            .map_err(ExecutionError::CoreError)?;
        
        Ok(())
    }
    
    async fn status(&self) -> SchedulerStatus {
        let state = self.state.lock().await;
        let workers = self.workers.read().await;
        
        // Create worker statuses
        let mut worker_statuses = Vec::new();
        
        for (worker_id, worker) in &*workers {
            // Count jobs assigned to this worker
            let assigned_jobs = state.job_worker_map.values()
                .filter(|w_id| *w_id == worker_id)
                .count();
            
            // Get worker status
            worker_statuses.push(WorkerStatus {
                worker_id: worker_id.clone(),
                active: true, // We would get this from the worker
                assigned_jobs,
                last_status_report: std::time::SystemTime::now(), // Would come from worker
            });
        }
        
        SchedulerStatus {
            pending_executions: state.pending_executions.len(),
            running_executions: state.running_executions.len(),
            worker_count: workers.len(),
            worker_statuses,
        }
    }
}

/// Extension methods for ExecutionPlan.
impl ExecutionPlan {
    /// Create a placeholder execution plan.
    pub fn placeholder() -> Self {
        // This should never be used for actual execution,
        // only as a placeholder in the scheduler
        Self {
            dependencies: std::collections::HashMap::new(),
            dependents: std::collections::HashMap::new(),
            initial_nodes: std::collections::HashSet::new(),
            final_nodes: std::collections::HashSet::new(),
        }
    }
}