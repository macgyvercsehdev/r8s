// crates/r8s-api/src/middleware/auth.rs
//! Authentication middleware.

use std::sync::Arc;
use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation, Algorithm};
use serde::{Deserialize, Serialize};
use chrono::{Utc, Duration};
use uuid::Uuid;

use crate::error::ApiError;

/// JWT claims for authentication tokens.
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    /// Subject (user ID)
    pub sub: String,
    
    /// User role
    pub role: String,
    
    /// Expiration time (Unix timestamp)
    pub exp: usize,
    
    /// Issued at (Unix timestamp)
    pub iat: usize,
}

/// Authentication middleware.
#[derive(Clone)]
pub struct AuthMiddleware {
    /// Secret key for JWT encoding/decoding
    secret: Arc<String>,
}

impl AuthMiddleware {
    /// Create a new authentication middleware with the given secret.
    pub fn new(secret: &str) -> Self {
        Self {
            secret: Arc::new(secret.to_string()),
        }
    }
    
    /// Generate a JWT token for a user.
    pub fn generate_token(&self, user_id: Uuid, role: &str, expires_in_days: i64) -> Result<String, ApiError> {
        let now = Utc::now();
        let expires_at = now + Duration::days(expires_in_days);
        
        let claims = Claims {
            sub: user_id.to_string(),
            role: role.to_string(),
            exp: expires_at.timestamp() as usize,
            iat: now.timestamp() as usize,
        };
        
        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.secret.as_bytes()),
        )
        .map_err(|e| ApiError::Authentication(format!("Failed to generate token: {}", e)))
    }
    
    /// Validate a JWT token and extract claims.
    pub fn validate_token(&self, token: &str) -> Result<Claims, ApiError> {
        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.secret.as_bytes()),
            &Validation::new(Algorithm::HS256),
        )
        .map_err(|e| ApiError::Authentication(format!("Invalid token: {}", e)))?;
        
        Ok(token_data.claims)
    }
    
    /// Extract the token from the Authorization header.
    fn extract_token(headers: &HeaderMap) -> Option<String> {
        headers
            .get("Authorization")
            .and_then(|header| header.to_str().ok())
            .and_then(|auth_header| {
                if auth_header.starts_with("Bearer ") {
                    Some(auth_header[7..].to_string())
                } else {
                    None
                }
            })
    }
    
    /// Authentication middleware handler.
    pub async fn check_auth(
        State(self_): Self,
        request: Request,
        next: Next,
    ) -> Result<Response, ApiError> {
        // Skip authentication for login route and public routes
        let path = request.uri().path();
        if path.starts_with("/api/auth/login") || 
           path.starts_with("/swagger-ui") || 
           path.starts_with("/api-doc") {
            return Ok(next.run(request).await);
        }
        
        // Extract and validate token
        let token = Self::extract_token(request.headers())
            .ok_or_else(|| ApiError::Authentication("Missing authorization token".to_string()))?;
        
        let claims = self_.validate_token(&token)?;
        
        // Proceed with the request
        Ok(next.run(request).await)
    }
}