// r8s-execution/src/worker/mod.rs
//! Worker implementation for distributed execution.
//!
//! This module provides the worker implementation that executes workflow nodes
//! as part of a distributed execution system.

use std::sync::Arc;
use std::time::{Duration, Instant};
use std::collections::HashMap;
use async_trait::async_trait;
use tokio::sync::{mpsc, Mutex};
use tokio::time;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use r8s_core::common::EntityId;
use r8s_core::repository::{WorkflowRepository, ExecutionRepository};
use r8s_core::entity::ExecutionLog;

use crate::error::{ExecutionError, Result};
use crate::executor::NodeExecutor;
use crate::scheduler::ExecutionScheduler;

/// Job status for a worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobStatus {
    /// Job is pending execution
    Pending,
    
    /// Job is currently running
    Running,
    
    /// Job completed successfully
    Completed,
    
    /// Job failed
    Failed(String),
    
    /// Job was canceled
    Canceled,
}

/// Job for a worker to execute.
#[derive(Debug, Clone)]
pub struct Job {
    /// Unique ID of the job
    pub id: String,
    
    /// Execution ID the job belongs to
    pub execution_id: EntityId,
    
    /// Node ID to execute
    pub node_id: EntityId,
    
    /// Current status of the job
    pub status: JobStatus,
    
    /// When the job was created
    pub created_at: std::time::SystemTime,
    
    /// When the job was started
    pub started_at: Option<std::time::SystemTime>,
    
    /// When the job was completed
    pub completed_at: Option<std::time::SystemTime>,
}

impl Job {
    /// Create a new job.
    pub fn new(execution_id: EntityId, node_id: EntityId) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            execution_id,
            node_id,
            status: JobStatus::Pending,
            created_at: std::time::SystemTime::now(),
            started_at: None,
            completed_at: None,
        }
    }
    
    /// Mark the job as running.
    pub fn start(&mut self) {
        self.status = JobStatus::Running;
        self.started_at = Some(std::time::SystemTime::now());
    }
    
    /// Mark the job as completed.
    pub fn complete(&mut self) {
        self.status = JobStatus::Completed;
        self.completed_at = Some(std::time::SystemTime::now());
    }
    
    /// Mark the job as failed.
    pub fn fail(&mut self, reason: String) {
        self.status = JobStatus::Failed(reason);
        self.completed_at = Some(std::time::SystemTime::now());
    }
    
    /// Mark the job as canceled.
    pub fn cancel(&mut self) {
        self.status = JobStatus::Canceled;
        self.completed_at = Some(std::time::SystemTime::now());
    }
}

/// Commands that can be sent to a worker.
#[derive(Debug)]
pub enum WorkerCommand {
    /// Execute a job
    ExecuteJob(Job),
    
    /// Cancel a job
    CancelJob(String),
    
    /// Pause worker (stop accepting new jobs)
    Pause,
    
    /// Resume worker
    Resume,
    
    /// Stop worker (gracefully)
    Stop,
}

/// Interface for a worker that executes jobs.
#[async_trait]
pub trait Worker: Send + Sync {
    /// Get the worker ID.
    fn id(&self) -> &str;
    
    /// Get the current status of the worker.
    async fn status(&self) -> WorkerStatus;
    
    /// Submit a job to the worker.
    async fn submit_job(&self, job: Job) -> Result<()>;
    
    /// Cancel a job.
    async fn cancel_job(&self, job_id: &str) -> Result<()>;
    
    /// Pause the worker.
    async fn pause(&self) -> Result<()>;
    
    /// Resume the worker.
    async fn resume(&self) -> Result<()>;
    
    /// Stop the worker gracefully.
    async fn stop(&self) -> Result<()>;
}

/// Status of a worker.
#[derive(Debug, Clone)]
pub struct WorkerStatus {
    /// Worker ID
    pub worker_id: String,
    
    /// Whether the worker is active (accepting jobs)
    pub active: bool,
    
    /// Number of jobs currently in queue
    pub queued_jobs: usize,
    
    /// Number of jobs currently running
    pub running_jobs: usize,
    
    /// Total jobs processed since worker started
    pub total_processed: usize,
    
    /// Number of jobs completed successfully
    pub successful_jobs: usize,
    
    /// Number of jobs that failed
    pub failed_jobs: usize,
    
    /// Time the worker started
    pub start_time: std::time::SystemTime,
    
    /// System metrics (CPU, memory, etc.)
    pub system_metrics: Option<SystemMetrics>,
}

/// System metrics for a worker.
#[derive(Debug, Clone)]
pub struct SystemMetrics {
    /// CPU usage percentage
    pub cpu_usage: f32,
    
    /// Memory usage in MB
    pub memory_usage: u64,
    
    /// Available memory in MB
    pub available_memory: u64,
}

/// Configuration for a worker.
#[derive(Debug, Clone)]
pub struct WorkerConfig {
    /// Worker ID
    pub worker_id: String,
    
    /// Maximum number of concurrent jobs
    pub max_concurrent_jobs: usize,
    
    /// Job polling interval
    pub poll_interval: Duration,
    
    /// Command channel capacity
    pub command_channel_capacity: usize,
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            worker_id: format!("worker-{}", Uuid::new_v4()),
            max_concurrent_jobs: 4,
            poll_interval: Duration::from_secs(1),
            command_channel_capacity: 100,
        }
    }
}

/// Default worker implementation.
pub struct DefaultWorker<W, E, N>
where
    W: WorkflowRepository + Send + Sync + 'static,
    E: ExecutionRepository + Send + Sync + 'static,
    N: NodeExecutor + Send + Sync + 'static,
{
    /// Worker configuration
    config: WorkerConfig,
    
    /// Worker state
    state: Arc<Mutex<WorkerState>>,
    
    /// Command sender
    command_tx: mpsc::Sender<WorkerCommand>,
    
    /// Repositories and executors
    workflow_repository: Arc<W>,
    execution_repository: Arc<E>,
    node_executor: Arc<N>,
}

/// Internal state of a worker.
struct WorkerState {
    /// Whether the worker is active
    active: bool,
    
    /// Jobs currently in queue
    queued_jobs: HashMap<String, Job>,
    
    /// Jobs currently running
    running_jobs: HashMap<String, Job>,
    
    /// Statistics
    total_processed: usize,
    successful_jobs: usize,
    failed_jobs: usize,
    
    /// Start time
    start_time: std::time::SystemTime,
}

impl<W, E, N> DefaultWorker<W, E, N>
where
    W: WorkflowRepository + Send + Sync + 'static,
    E: ExecutionRepository + Send + Sync + 'static,
    N: NodeExecutor + Send + Sync + 'static,
{
    /// Create a new worker.
    pub fn new(
        config: WorkerConfig,
        workflow_repository: Arc<W>,
        execution_repository: Arc<E>,
        node_executor: Arc<N>,
    ) -> (Arc<Self>, mpsc::Receiver<WorkerCommand>) {
        let (command_tx, command_rx) = mpsc::channel(config.command_channel_capacity);
        
        let state = Arc::new(Mutex::new(WorkerState {
            active: true,
            queued_jobs: HashMap::new(),
            running_jobs: HashMap::new(),
            total_processed: 0,
            successful_jobs: 0,
            failed_jobs: 0,
            start_time: std::time::SystemTime::now(),
        }));
        
        let worker = Arc::new(Self {
            config,
            state,
            command_tx,
            workflow_repository,
            execution_repository,
            node_executor,
        });
        
        (worker, command_rx)
    }
    
    /// Start the worker loop.
    pub async fn start(
        worker: Arc<Self>,
        mut command_rx: mpsc::Receiver<WorkerCommand>,
    ) -> Result<()> {
        info!("Starting worker: {}", worker.config.worker_id);
        
        let worker_id = worker.config.worker_id.clone();
        let max_concurrent = worker.config.max_concurrent_jobs;
        let poll_interval = worker.config.poll_interval;
        
        // Create a channel for job completion notifications
        let (completion_tx, mut completion_rx) = mpsc::channel::<(String, Result<()>)>(max_concurrent);
        
        // Main worker loop
        loop {
            // Process any completed jobs
            while let Ok((job_id, result)) = completion_rx.try_recv() {
                worker.handle_job_completion(job_id, result).await;
            }
            
            // Check if we need to start more jobs
            let running_count = {
                let state = worker.state.lock().await;
                state.running_jobs.len()
            };
            
            let worker_active = {
                let state = worker.state.lock().await;
                state.active
            };
            
            // Start jobs if we have capacity and worker is active
            if running_count < max_concurrent && worker_active {
                // Get next job
                let next_job = {
                    let mut state = worker.state.lock().await;
                    if state.queued_jobs.is_empty() {
                        None
                    } else {
                        let job_id = state.queued_jobs.keys().next().unwrap().clone();
                        state.queued_jobs.remove(&job_id)
                    }
                };
                
                // Execute the job
                if let Some(mut job) = next_job {
                    job.start();
                    
                    // Store in running jobs
                    {
                        let mut state = worker.state.lock().await;
                        state.running_jobs.insert(job.id.clone(), job.clone());
                    }
                    
                    // Create completion sender
                    let completion_tx = completion_tx.clone();
                    let job_id = job.id.clone();
                    let worker_clone = worker.clone();
                    
                    // Spawn a task to execute the job
                    tokio::spawn(async move {
                        let result = worker_clone.execute_job(&job).await;
                        let _ = completion_tx.send((job_id, result)).await;
                    });
                }
            }
            
            // Process any incoming commands
            tokio::select! {
                Some(cmd) = command_rx.recv() => {
                    match cmd {
                        WorkerCommand::ExecuteJob(job) => {
                            debug!("Worker {} received job {}", worker_id, job.id);
                            let mut state = worker.state.lock().await;
                            state.queued_jobs.insert(job.id.clone(), job);
                        }
                        WorkerCommand::CancelJob(job_id) => {
                            debug!("Worker {} canceling job {}", worker_id, job_id);
                            let mut state = worker.state.lock().await;
                            
                            // Check queued jobs first
                            if let Some(mut job) = state.queued_jobs.remove(&job_id) {
                                job.cancel();
                                // Update statistics
                                state.total_processed += 1;
                            }
                            // Check running jobs (will be canceled at next check)
                            else if state.running_jobs.contains_key(&job_id) {
                                // Mark as canceled, but leave in running jobs
                                // Will be handled by the completion handler
                                if let Some(job) = state.running_jobs.get_mut(&job_id) {
                                    job.cancel();
                                }
                            }
                        }
                        WorkerCommand::Pause => {
                            debug!("Worker {} paused", worker_id);
                            let mut state = worker.state.lock().await;
                            state.active = false;
                        }
                        WorkerCommand::Resume => {
                            debug!("Worker {} resumed", worker_id);
                            let mut state = worker.state.lock().await;
                            state.active = true;
                        }
                        WorkerCommand::Stop => {
                            info!("Worker {} stopping", worker_id);
                            // Cancel all queued jobs
                            {
                                let mut state = worker.state.lock().await;
                                for (_, mut job) in state.queued_jobs.drain() {
                                    job.cancel();
                                }
                                state.active = false;
                            }
                            
                            // Wait for running jobs to complete
                            loop {
                                let running_count = {
                                    let state = worker.state.lock().await;
                                    state.running_jobs.len()
                                };
                                
                                if running_count == 0 {
                                    break;
                                }
                                
                                // Process any completed jobs
                                while let Ok((job_id, result)) = completion_rx.try_recv() {
                                    worker.handle_job_completion(job_id, result).await;
                                }
                                
                                // Wait a bit
                                time::sleep(Duration::from_millis(100)).await;
                            }
                            
                            info!("Worker {} stopped", worker_id);
                            return Ok(());
                        }
                    }
                }
                _ = time::sleep(poll_interval) => {
                    // Just a timeout to ensure we process completions and commands
                }
            }
        }
    }
    
    /// Execute a job.
    async fn execute_job(&self, job: &Job) -> Result<()> {
        debug!("Worker {} executing job {}", self.config.worker_id, job.id);
        
        // Get the execution
        let execution = self.execution_repository.find_by_id(job.execution_id).await
            .map_err(ExecutionError::CoreError)?;
        
        // Get the workflow
        let workflow = self.workflow_repository.find_by_id(execution.workflow_id).await
            .map_err(ExecutionError::CoreError)?;
        
        // Find the node
        let node = workflow.nodes.iter().find(|n| n.id == job.node_id)
            .ok_or_else(|| ExecutionError::NodeExecutionFailed {
                node_id: job.node_id,
                message: format!("Node not found in workflow"),
            })?;
        
        // Prepare inputs (usually would come from execution state)
        // For demonstration, we'll use an empty input set
        let inputs = HashMap::new();
        
        // Execute the node
        let result = self.node_executor.execute_node(
            job.execution_id,
            node,
            inputs,
        ).await;
        
        match result {
            Ok(node_result) => {
                // Record the result
                self.execution_repository.add_node_result(job.execution_id, node_result).await
                    .map_err(ExecutionError::CoreError)?;
                
                Ok(())
            }
            Err(err) => {
                // Record the failure
                let log = ExecutionLog {
                    timestamp: chrono::Utc::now(),
                    level: r8s_core::entity::LogLevel::Error,
                    message: format!("Node execution failed: {}", err),
                    node_id: Some(job.node_id),
                    metadata: None,
                };
                
                self.execution_repository.add_logs(job.execution_id, vec![log]).await
                    .map_err(ExecutionError::CoreError)?;
                
                Err(err)
            }
        }
    }
    
    /// Handle job completion.
    async fn handle_job_completion(&self, job_id: String, result: Result<()>) {
        let mut state = self.state.lock().await;
        
        // Find the job
        if let Some(mut job) = state.running_jobs.remove(&job_id) {
            // Update job status
            match result {
                Ok(_) => {
                    // If the job wasn't already canceled
                    if job.status != JobStatus::Canceled {
                        job.complete();
                        state.successful_jobs += 1;
                    }
                }
                Err(err) => {
                    // If the job wasn't already canceled
                    if job.status != JobStatus::Canceled {
                        job.fail(err.to_string());
                        state.failed_jobs += 1;
                    }
                }
            }
            
            // Update statistics
            state.total_processed += 1;
            
            debug!(
                "Worker {} completed job {} with status {:?}",
                self.config.worker_id, job_id, job.status
            );
        }
    }
}

#[async_trait]
impl<W, E, N> Worker for DefaultWorker<W, E, N>
where
    W: WorkflowRepository + Send + Sync + 'static,
    E: ExecutionRepository + Send + Sync + 'static,
    N: NodeExecutor + Send + Sync + 'static,
{
    fn id(&self) -> &str {
        &self.config.worker_id
    }
    
    async fn status(&self) -> WorkerStatus {
        let state = self.state.lock().await;
        
        WorkerStatus {
            worker_id: self.config.worker_id.clone(),
            active: state.active,
            queued_jobs: state.queued_jobs.len(),
            running_jobs: state.running_jobs.len(),
            total_processed: state.total_processed,
            successful_jobs: state.successful_jobs,
            failed_jobs: state.failed_jobs,
            start_time: state.start_time,
            system_metrics: None, // Would be populated with actual metrics
        }
    }
    
    async fn submit_job(&self, job: Job) -> Result<()> {
        self.command_tx.send(WorkerCommand::ExecuteJob(job))
            .await
            .map_err(|_| ExecutionError::WorkerFailure("Failed to submit job".to_string()))?;
        
        Ok(())
    }
    
    async fn cancel_job(&self, job_id: &str) -> Result<()> {
        self.command_tx.send(WorkerCommand::CancelJob(job_id.to_string()))
            .await
            .map_err(|_| ExecutionError::WorkerFailure("Failed to cancel job".to_string()))?;
        
        Ok(())
    }
    
    async fn pause(&self) -> Result<()> {
        self.command_tx.send(WorkerCommand::Pause)
            .await
            .map_err(|_| ExecutionError::WorkerFailure("Failed to pause worker".to_string()))?;
        
        Ok(())
    }
    
    async fn resume(&self) -> Result<()> {
        self.command_tx.send(WorkerCommand::Resume)
            .await
            .map_err(|_| ExecutionError::WorkerFailure("Failed to resume worker".to_string()))?;
        
        Ok(())
    }
    
    async fn stop(&self) -> Result<()> {
        self.command_tx.send(WorkerCommand::Stop)
            .await
            .map_err(|_| ExecutionError::WorkerFailure("Failed to stop worker".to_string()))?;
        
        Ok(())
    }
}