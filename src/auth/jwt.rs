use serde::{Deserialize, Serialize};
use std::str::FromStr;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use crate::errors::{ServiceError, ServiceResult, DomainError};
use crate::types::UserRole;
use std::sync::OnceLock;
use jsonwebtoken::{encode, decode, Header, EncodingKey, DecodingKey, Validation, Algorithm, TokenData};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub role: String,
    pub device_id: String,
    pub iat: i64,
    pub exp: i64,
    pub jti: String,
    pub refresh_exp: Option<i64>,
}

// JWT secret - in a real app this should be loaded from a secure environment variable
static JWT_SECRET: OnceLock<String> = OnceLock::new();

/// Token type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenType {
    /// Access token (short-lived)
    Access,
    /// Refresh token (long-lived)
    Refresh,
}

/// Initialize JWT module with secret
pub fn initialize(secret: &str) {
    JWT_SECRET.get_or_init(|| secret.to_string());
}

/// Get JWT secret
fn get_secret() -> ServiceResult<&'static str> {
    JWT_SECRET.get()
        .map(|s| s.as_str())
        .ok_or_else(|| ServiceError::Configuration("JWT secret not initialized".to_string()))
}

/// Generate a JWT token
pub fn generate_token(
    user_id: &Uuid,
    role: &UserRole,
    device_id: &str,
    token_type: TokenType,
) -> ServiceResult<(String, DateTime<Utc>)> {
    let secret = get_secret()?;
    
    let now = Utc::now();
    let token_id = Uuid::new_v4().to_string();
    
    // Set expiration based on token type
    let (expiry, refresh_exp) = match token_type {
        TokenType::Access => {
            // Access tokens expire in 15 minutes
            let exp = now + chrono::Duration::minutes(15);
            (exp, None)
        },
        TokenType::Refresh => {
            // Refresh tokens expire in 30 days
            let access_exp = now + chrono::Duration::minutes(15);
            let refresh_exp = now + chrono::Duration::days(30);
            (access_exp, Some(refresh_exp.timestamp()))
        }
    };
    
    // Create claims
    let claims = Claims {
        sub: user_id.to_string(),
        role: role.as_str().to_string(),
        device_id: device_id.to_string(),
        iat: now.timestamp(),
        exp: expiry.timestamp(),
        jti: token_id,
        refresh_exp,
    };
    
    // Encode token
    let token = jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| ServiceError::Domain(DomainError::Internal(format!("JWT encoding error: {}", e))))?;
    
    Ok((token, expiry))
}

/// Verify a JWT token
pub fn verify_token(token: &str) -> ServiceResult<Claims> {
    let secret = get_secret()?;
    
    // Decode and validate token
    let token_data = jsonwebtoken::decode::<Claims>(
        token,
        &jsonwebtoken::DecodingKey::from_secret(secret.as_bytes()),
        &jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256),
    )
    .map_err(|e| match e.kind() {
        jsonwebtoken::errors::ErrorKind::ExpiredSignature => ServiceError::SessionExpired,
        _ => ServiceError::Authentication(format!("Invalid token: {}", e)),
    })?;
    
    Ok(token_data.claims)
}

/// Generate a refresh token
pub fn generate_refresh_token(
    user_id: &Uuid,
    role: &UserRole,
    device_id: &str,
) -> ServiceResult<(String, DateTime<Utc>, DateTime<Utc>)> {
    let (token, access_expiry) = generate_token(user_id, role, device_id, TokenType::Refresh)?;
    
    // Parse claims to get refresh expiry
    let claims = verify_token(&token)?;
    let refresh_expiry = claims.refresh_exp
        .ok_or_else(|| ServiceError::Domain(DomainError::Internal("Refresh token missing refresh_exp".to_string())))?;
        
    let refresh_expiry_dt = DateTime::from_timestamp(refresh_expiry, 0)
        .ok_or_else(|| ServiceError::Domain(DomainError::Internal("Invalid refresh expiry timestamp".to_string())))?;
        
    Ok((token, access_expiry, refresh_expiry_dt))
}

/// Refresh an access token using a refresh token
pub fn refresh_access_token(refresh_token: &str) -> ServiceResult<(String, DateTime<Utc>)> {
    // Verify the refresh token first
    let claims = verify_token(refresh_token)?;
    
    // Ensure it's a refresh token
    if claims.refresh_exp.is_none() {
        return Err(ServiceError::Authentication("Not a refresh token".to_string()));
    }
    
    // Check if refresh token is expired
    let now = Utc::now().timestamp();
    if let Some(refresh_exp) = claims.refresh_exp {
        if refresh_exp < now {
            return Err(ServiceError::SessionExpired);
        }
    }
    
    // Parse user ID, role, and device ID from claims
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| ServiceError::Authentication("Invalid user ID in token".to_string()))?;
        
    let role = UserRole::from_str(&claims.role)
        .ok_or_else(|| ServiceError::Authentication("Invalid role in token".to_string()))?;
        
    // Generate a new access token
    generate_token(&user_id, &role, &claims.device_id, TokenType::Access)
}

/// Decodes token claims without verifying signature or expiry.
/// Useful for retrieving JTI/expiry for logging out/revocation even if token is expired.
pub fn decode_unverified(token: &str) -> ServiceResult<Claims> {
    // Use a validation struct that ignores expiry and signature
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = false;
    validation.insecure_disable_signature_validation(); // Added to bypass signature check
    
    // Attempt to decode using a dummy key, as signature isn't checked
    // Note: The algorithm still needs to match the header for the library to proceed.
    let dummy_key = DecodingKey::from_secret(b"dummy");
    
    let token_data = decode::<Claims>(token, &dummy_key, &validation)
        .map_err(|e| {
            log::error!("Unverified token decode error: {}", e);
            // Distinguish between structural errors and others if needed
            ServiceError::Authentication(format!("Invalid token structure: {}", e))
        })?;
    
    Ok(token_data.claims)
}

/// Revoke a token (in a real app, this would add it to a blocklist)
pub fn revoke_token(token: &str) -> ServiceResult<()> {
    // Placeholder - Actual revocation logic is now in AuthService using the repository
    // This function could potentially decode the token to get JTI/expiry if needed elsewhere,
    // but currently AuthService handles that directly.
    log::warn!("jwt::revoke_token called, but it's a placeholder. Revocation handled by AuthService.");
    // You might decode here just to log the JTI being 'revoked' conceptually
    match decode_unverified(token) {
        Ok(claims) => log::info!("Placeholder revocation for JTI: {}", claims.jti),
        Err(_) => log::warn!("Could not decode token for placeholder revocation log.")
    }
    Ok(())
} 