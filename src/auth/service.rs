use crate::errors::{ServiceError, ServiceResult, DomainError};
use crate::auth::{AuthContext, AuthRepository, jwt};
use crate::types::UserRole;
use uuid::Uuid;
use argon2::{Argon2, PasswordHash, PasswordVerifier, PasswordHasher, password_hash::SaltString};
// Use the older rand version for compatibility with argon2
use rand_core::OsRng as ArgonOsRng;
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use std::sync::Arc;

/// Updated results from a successful login, including refresh token
#[derive(Debug)]
pub struct LoginResult {
    pub user_id: Uuid,
    pub role: UserRole,
    pub auth_context: AuthContext,
    pub access_token: String,
    pub access_expiry: DateTime<Utc>,
    pub refresh_token: String,
    pub refresh_expiry: DateTime<Utc>,
}

/// Auth service for handling user authentication
pub struct AuthService {
    auth_repo: Arc<dyn AuthRepository>,
    device_id: String,
    offline_mode: bool,
}

impl AuthService {
    /// Create a new auth service
    pub fn new(
        pool: SqlitePool,
        device_id: String,
        offline_mode: bool,
    ) -> Self {
        let auth_repo = Arc::new(super::repository::SqliteAuthRepository::new(pool));
        
        Self {
            auth_repo,
            device_id,
            offline_mode,
        }
    }
    
    /// Authenticate a user with email and password, returning access and refresh tokens
    pub async fn login(&self, email: &str, password: &str) -> ServiceResult<LoginResult> {
        // Attempt to find user by email
        let user = match self.auth_repo.find_user_by_email(email).await {
            Ok(user) => user,
            Err(_) => {
                // Log failed login attempt (ensure DbError is mapped)
                let _ = self.auth_repo.log_login_attempt(email, false, None, &self.device_id)
                    .await.map_err(DomainError::Database)?;
                return Err(ServiceError::Authentication("Invalid email or password".to_string()));
            }
        };
        
        // Check if user is active
        if !user.active {
            self.auth_repo.log_login_attempt(email, false, Some(user.id), &self.device_id)
                .await.map_err(DomainError::Database)?;
            return Err(ServiceError::Authentication("Account is inactive".to_string()));
        }
        
        // Verify password
        if let Err(_) = self.verify_password(password, &user.password_hash) {
            self.auth_repo.log_login_attempt(email, false, Some(user.id), &self.device_id)
                .await.map_err(DomainError::Database)?;
            return Err(ServiceError::Authentication("Invalid email or password".to_string()));
        }
        
        // Update last login timestamp
        self.auth_repo.update_last_login(user.id).await.map_err(DomainError::Database)?;
        
        // Log successful login
        self.auth_repo.log_login_attempt(email, true, Some(user.id), &self.device_id)
            .await.map_err(DomainError::Database)?;
        
        // Generate tokens using the jwt module
        let (access_token, access_expiry) = jwt::generate_token(
            &user.id, &user.role, &self.device_id, jwt::TokenType::Access
        )?;
        let (refresh_token, _, refresh_expiry) = jwt::generate_refresh_token(
             &user.id, &user.role, &self.device_id
        )?;
        
        // Create initial auth context for immediate use after login
        let auth_context = AuthContext::new(user.id, user.role, self.device_id.clone(), self.offline_mode);
        
        Ok(LoginResult {
            user_id: user.id,
            role: user.role,
            auth_context,
            access_token,
            access_expiry,
            refresh_token,
            refresh_expiry,
        })
    }
    
    /// Verify an access token and create an auth context
    pub async fn verify_token(&self, token: &str) -> ServiceResult<AuthContext> {
        // Verify token signature and standard claims (like expiry)
        let claims = jwt::verify_token(token)?;

        // Check blocklist
        let is_revoked = self.auth_repo.is_token_revoked(&claims.jti)
            .await
            .map_err(|db_err| ServiceError::Domain(DomainError::Database(db_err)))?; // Map DbError to ServiceError

        if is_revoked {
            log::warn!("Attempted to use revoked token JTI: {}", claims.jti);
            return Err(ServiceError::Authentication("Token has been revoked".to_string()));
        }
        
        // Extract necessary information from claims
        let user_id = Uuid::parse_str(&claims.sub)
             .map_err(|_| ServiceError::Authentication("Invalid user ID in token".to_string()))?;
        
        let role = UserRole::from_str(&claims.role)
             .ok_or_else(|| ServiceError::Authentication("Invalid role in token".to_string()))?;
        
        // Check if the token is an access token (not a refresh token)
        if claims.refresh_exp.is_some() {
            return Err(ServiceError::Authentication("Expected access token, received refresh token".to_string()));
        }
        
        // Create auth context using info from the validated claims
        let auth_context = AuthContext::new(
            user_id,
            role,
            claims.device_id, // Use device ID from token
            self.offline_mode,
        );
        
        Ok(auth_context)
    }
    
    /// Refresh an access token using a refresh token
    pub async fn refresh_session(&self, refresh_token: &str) -> ServiceResult<(String, DateTime<Utc>)> {
        // Use the jwt module to refresh the token
        let (new_access_token, new_access_expiry) = jwt::refresh_access_token(refresh_token)?;
        
        Ok((new_access_token, new_access_expiry))
    }
    
    /// Generate a hash for a new password
    pub fn hash_password(&self, password: &str) -> ServiceResult<String> {
        // Generate a random salt using argon2's compatible OsRng
        // This uses rand_core::OsRng directly instead of trying to bridge versions
        let mut rng = ArgonOsRng; // Create an instance of the RNG
        let salt = SaltString::generate(&mut rng); // Pass a mutable reference to the instance
        
        // Configure Argon2 with default parameters
        let argon2 = Argon2::default();
        
        // Hash the password
        let password_hash = argon2.hash_password(password.as_bytes(), &salt)
            .map_err(|e| ServiceError::Domain(DomainError::Internal(format!("Failed to hash password: {}", e))))?
            .to_string();
            
        Ok(password_hash)
    }
    
    /// Verify a password against a hash
    fn verify_password(&self, password: &str, hash: &str) -> Result<(), ServiceError> {
        // Parse the hash string
        let parsed_hash = PasswordHash::new(hash)
            .map_err(|_| ServiceError::Domain(DomainError::Internal("Invalid password hash format".to_string())))?;
            
        // Verify the password
        Argon2::default().verify_password(password.as_bytes(), &parsed_hash)
            .map_err(|_| ServiceError::Authentication("Invalid password".to_string()))
    }
    
    /// Log out a user (revoking tokens by adding JTI to blocklist)
    pub async fn logout(&self, auth_context: &AuthContext, access_token: &str, refresh_token: Option<&str>) -> ServiceResult<()> {
        // Decode access token to get JTI and expiry
        // We use decode_unverified here as we don't need to check expiry/signature again, just get claims.
        // If decode fails, we still log the user out but log an error.
        match jwt::decode_unverified(access_token) {
            Ok(claims) => {
                if let Err(e) = self.auth_repo.add_revoked_token(&claims.jti, claims.exp).await {
                    log::error!("Failed to add access token JTI {} to blocklist: {}", claims.jti, e);
                }
            },
            Err(e) => {
                 log::error!("Failed to decode access token during logout for user {}: {}", auth_context.user_id, e);
            }
        }
        
        // Revoke refresh token if provided
        if let Some(rt) = refresh_token {
             match jwt::decode_unverified(rt) {
                 Ok(claims) => {
                    if let Some(refresh_exp) = claims.refresh_exp {
                        // Use refresh_exp if available, otherwise fall back to exp (though refresh should always have refresh_exp)
                        let expiry = refresh_exp;
                        if let Err(e) = self.auth_repo.add_revoked_token(&claims.jti, expiry).await {
                            log::error!("Failed to add refresh token JTI {} to blocklist: {}", claims.jti, e);
                        }
                    } else {
                        log::error!("Refresh token missing refresh_exp claim during logout for user {}", auth_context.user_id);
                    }
                 },
                 Err(e) => {
                    log::error!("Failed to decode refresh token during logout for user {}: {}", auth_context.user_id, e);
                 }
             }
        }
        
        // Log the logout action in the database
        self.auth_repo.log_logout(auth_context.user_id, &auth_context.device_id)
             .await.map_err(DomainError::Database)?;
        
        log::info!("User {} logged out from device {}", auth_context.user_id, auth_context.device_id);
        Ok(())
    }
}