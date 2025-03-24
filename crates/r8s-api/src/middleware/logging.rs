// crates/r8s-api/src/middleware/logging.rs
//! Request logging middleware.

use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use tracing::{info, error};
use uuid::Uuid;
use std::time::Instant;

/// Logging middleware.
#[derive(Clone)]
pub struct LoggingMiddleware {}

impl LoggingMiddleware {
    /// Create a new logging middleware.
    pub fn new() -> Self {
        Self {}
    }
    
    /// Log request information.
    pub async fn log_request(
        State(self_): Self,
        request: Request,
        next: Next,
    ) -> Response {
        let request_id = Uuid::new_v4();
        let method = request.method().clone();
        let uri = request.uri().clone();
        
        // Start timing
        let start = Instant::now();
        
        info!(
            request_id = %request_id,
            method = %method,
            uri = %uri,
            "Request started"
        );
        
        // Process the request
        let response = next.run(request).await;
        
        // Calculate request duration
        let duration = start.elapsed();
        
        // Log response information
        let status = response.status();
        
        if status.is_success() || status.is_redirection() {
            info!(
                request_id = %request_id,
                method = %method,
                uri = %uri,
                status = status.as_u16(),
                duration_ms = duration.as_millis(),
                "Request completed"
            );
        } else {
            error!(
                request_id = %request_id,
                method = %method,
                uri = %uri,
                status = status.as_u16(),
                duration_ms = duration.as_millis(),
                "Request failed"
            );
        }
        
        response
    }
}