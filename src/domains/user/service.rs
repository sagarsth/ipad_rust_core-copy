use crate::errors::{ServiceError, ServiceResult, DomainError};
use crate::domains::user::types::{User, NewUser, UpdateUser, UserResponse, UserStats};
use crate::domains::user::repository::UserRepository;
use crate::auth::{AuthContext, AuthService};
use crate::types::Permission;
use crate::validation::Validate;
use uuid::Uuid;
use std::sync::Arc;
use crate::domains::core::delete_service::{DeleteService, DeleteOptions};
use crate::domains::core::repository::DeleteResult;
use crate::domains::core::delete_service::DeleteServiceRepository;
use crate::domains::core::repository::HardDeletable;
use crate::domains::core::repository::SoftDeletable;

/// Service for user-related operations
pub struct UserService {
    user_repo: Arc<dyn UserRepository>,
    auth_service: Arc<AuthService>,
    delete_service: Arc<dyn DeleteService<User>>,
}

impl UserService {
    /// Create a new user service
    pub fn new(user_repo: Arc<dyn UserRepository>, auth_service: Arc<AuthService>, delete_service: Arc<dyn DeleteService<User>>) -> Self {
        Self { user_repo, auth_service, delete_service }
    }
    
    /// Get a user by ID
    pub async fn get_user(&self, id: Uuid, auth: &AuthContext) -> ServiceResult<User> {
        // Check permission
        auth.authorize(Permission::ManageUsers)?;
        
        // Get user from repository
        let user = self.user_repo.find_by_id(id)
            .await
            .map_err(ServiceError::Domain)?;
            
        Ok(user)
    }
    
    /// Get a user by ID as response DTO
    pub async fn get_user_response(&self, id: Uuid, auth: &AuthContext) -> ServiceResult<UserResponse> {
        let user = self.get_user(id, auth).await?;
        let response = UserResponse::from(user);
        self.enrich_response(response).await
    }
    
    /// Get all users
    pub async fn get_all_users(&self, auth: &AuthContext) -> ServiceResult<Vec<User>> {
        // Check permission
        auth.authorize(Permission::ManageUsers)?;
        
        // Get users from repository
        let users = self.user_repo.find_all()
            .await
            .map_err(ServiceError::Domain)?;
            
        Ok(users)
    }
    
    /// Get all users as response DTOs
    pub async fn get_all_user_responses(&self, auth: &AuthContext) -> ServiceResult<Vec<UserResponse>> {
        let users = self.get_all_users(auth).await?;
        let mut responses = Vec::new();
        
        for user in users {
            let response = UserResponse::from(user);
            let enriched = self.enrich_response(response).await?;
            responses.push(enriched);
        }
        
        Ok(responses)
    }
    
    /// Enrich a UserResponse with additional metadata (usernames, etc.)
    async fn enrich_response(&self, mut response: UserResponse) -> ServiceResult<UserResponse> {
        // Populate created_by username
        if let Some(created_by_id) = response.created_by_user_id {
            if let Ok(creator) = self.user_repo.find_by_id(created_by_id).await {
                response.created_by = Some(creator.name.clone());
                
                // Populate updated_by username - check if same as creator
                if let Some(updated_by_id) = response.updated_by_user_id {
                    if updated_by_id == created_by_id {
                        // Same person as creator
                        response.updated_by = Some(creator.name);
                    } else {
                        // Different person - fetch separately
                        if let Ok(updater) = self.user_repo.find_by_id(updated_by_id).await {
                            response.updated_by = Some(updater.name);
                        }
                    }
                }
            }
        } else if let Some(updated_by_id) = response.updated_by_user_id {
            // No creator found, but we have an updater
            if let Ok(updater) = self.user_repo.find_by_id(updated_by_id).await {
                response.updated_by = Some(updater.name);
            }
        }
        
        Ok(response)
    }
    
    /// Create a new user
    pub async fn create_user(&self, user: NewUser, auth: &AuthContext) -> ServiceResult<User> {
        // Check permission
        auth.authorize(Permission::ManageUsers)?;
        
        // Check if this operation is allowed in offline mode
        auth.check_offline_feature("create_user", false)?;
        
        // Validate user data
        user.validate().map_err(ServiceError::Domain)?;
        
        // Hash the password
        let password_hash = self.auth_service.hash_password(&user.password)?;
        
        // Create user with hashed password
        let mut user_with_hash = user;
        user_with_hash.password = password_hash;
        
        // Create user in repository
        let new_user = self.user_repo.create(user_with_hash, auth)
            .await
            .map_err(ServiceError::Domain)?;
            
        Ok(new_user)
    }
    
    /// Update an existing user
    pub async fn update_user(&self, id: Uuid, update: UpdateUser, auth: &AuthContext) -> ServiceResult<User> {
        // Check permission
        if id != auth.user_id {
            // Only admins can update other users
            auth.authorize(Permission::ManageUsers)?;
        }
        
        // Check if this operation is allowed in offline mode
        auth.check_offline_feature("update_user", false)?;
        
        // Validate update data
        update.validate().map_err(ServiceError::Domain)?;
        
        // Hash the password if provided
        let mut update_with_hash = update;
        if let Some(password) = update_with_hash.password {
            let password_hash = self.auth_service.hash_password(&password)?;
            update_with_hash.password = Some(password_hash);
        }
        
        // Update user in repository
        let updated_user = self.user_repo.update(id, update_with_hash, auth)
            .await
            .map_err(ServiceError::Domain)?;
            
        Ok(updated_user)
    }
    
    /// Hard delete a user (permanent delete)
    pub async fn hard_delete_user(&self, id: Uuid, auth: &AuthContext) -> ServiceResult<()> {
        // Check if user has permission to hard delete
        auth.authorize_admin()?;
        auth.authorize_hard_delete()?;
        
        // Check if this operation is allowed in offline mode
        auth.check_offline_feature("hard_delete_user", false)?;
        
        // Check if trying to delete self
        if id == auth.user_id {
            return Err(ServiceError::Domain(
                DomainError::Validation(
                    crate::errors::ValidationError::custom("Cannot delete your own user account")
                )
            ));
        }
        
        // Use DeleteService
        let options = DeleteOptions {
            allow_hard_delete: true,
            fallback_to_soft_delete: false,
            force: false,
        };

        match self.delete_service.delete(id, auth, options).await {
            Ok(DeleteResult::HardDeleted) => Ok(()),
            Ok(DeleteResult::SoftDeleted { dependencies }) => {
                log::warn!("User {} was soft-deleted unexpectedly during hard delete attempt due to dependencies: {:?}", id, dependencies);
                Err(ServiceError::DependenciesPreventDeletion(dependencies))
            }
            Ok(DeleteResult::DependenciesPrevented { dependencies }) => {
                Err(ServiceError::DependenciesPreventDeletion(dependencies))
            }
            Err(e @ DomainError::EntityNotFound(_, entity_id)) => {
                log::warn!("Attempted to delete non-existent user {}", entity_id);
                Err(ServiceError::Domain(DomainError::EntityNotFound("User".to_string(), entity_id)))
            }
            Err(e) => Err(ServiceError::Domain(e)),
        }
    }
    
    /// Check if email is unique
    pub async fn is_email_unique(&self, email: &str, exclude_id: Option<Uuid>) -> ServiceResult<bool> {
        let result = self.user_repo.is_email_unique(email, exclude_id)
            .await
            .map_err(ServiceError::Domain)?;
            
        Ok(result)
    }
    
    /// Get current user profile
    pub async fn get_current_user(&self, auth: &AuthContext) -> ServiceResult<User> {
        // Get user from repository
        let user = self.user_repo.find_by_id(auth.user_id)
            .await
            .map_err(ServiceError::Domain)?;
            
        Ok(user)
    }
    
    /// Update current user's profile
    pub async fn update_current_user(&self, update: UpdateUser, auth: &AuthContext) -> ServiceResult<User> {
        // Only allow updating certain fields for own profile
        // Prevent changing role or active status
        if update.role.is_some() || update.active.is_some() {
            return Err(ServiceError::PermissionDenied(
                "Cannot change role or active status for your own account".to_string()
            ));
        }
        
        // Update with standard method
        self.update_user(auth.user_id, update, auth).await
    }
    
    /// Change password with old password verification
    pub async fn change_password(
        &self, 
        old_password: &str, 
        new_password: &str, 
        auth: &AuthContext
    ) -> ServiceResult<()> {
        // Get current user
        let user = self.user_repo.find_by_id(auth.user_id)
            .await
            .map_err(ServiceError::Domain)?;
            
        // Verify old password
        self.auth_service.verify_token(&old_password) 
            .await
            .map_err(|_| ServiceError::Authentication("Current password is incorrect".to_string()))?;
            
        // Validate new password
        if new_password.len() < 8 {
            return Err(ServiceError::Domain(
                DomainError::Validation(
                    crate::errors::ValidationError::min_length("password", 8)
                )
            ));
        }
        
        // Hash new password
        let password_hash = self.auth_service.hash_password(new_password)?;
        
        // Update password
        let update = UpdateUser {
            password: Some(password_hash),
            updated_by_user_id: auth.user_id,
            ..Default::default()
        };
        
        self.user_repo.update(auth.user_id, update, auth)
            .await
            .map_err(ServiceError::Domain)?;
            
        Ok(())
    }
    
    /// Get user statistics (counts by role and status).
    pub async fn get_user_stats(&self, auth: &AuthContext) -> ServiceResult<UserStats> {
        // Only admins can view user stats
        auth.authorize(Permission::ManageUsers)?;

        let stats = self.user_repo.get_stats()
            .await
            .map_err(ServiceError::Domain)?;

        Ok(stats)
    }
    
    /// Initialize default admin, team lead, and officer accounts
    pub async fn initialize_default_accounts(&self, auth_context: &AuthContext) -> ServiceResult<()> {
        println!("👥 [USER_SERVICE] Starting default account initialization");
        println!("👤 [USER_SERVICE] Auth context - User: {}, Role: {:?}", auth_context.user_id, auth_context.role);
        
        // Helper function to create an account if it doesn't exist
        let create_account_if_needed = |email: String, password: String, name: String, role: String| async move {
            println!("🔍 [USER_SERVICE] Checking if {} account exists...", email);
            
            // Check if account already exists
            match self.user_repo.find_by_email(&email).await {
                Ok(_user) => {
                    println!("✅ [USER_SERVICE] Account {} already exists, skipping creation", email);
                    return Ok(());
                },
                Err(crate::errors::DomainError::EntityNotFound(_, _)) => {
                    println!("🔧 [USER_SERVICE] Account {} not found, creating...", email);
                    // Account doesn't exist, create it
                },
                Err(e) => {
                    println!("❌ [USER_SERVICE] Error checking for existing account {}: {}", email, e);
                    return Err(ServiceError::Domain(e));
                }
            }
            
            let new_user = NewUser {
                email: email.to_string(),
                password: password.to_string(),
                name: name.to_string(),
                role: role.to_string(),
                active: true,
                created_by_user_id: None, // System created
            };
            
            println!("🔐 [USER_SERVICE] Hashing password for {}...", email);
            let password_hash = self.auth_service.hash_password(&new_user.password)
                .map_err(|e| {
                    println!("❌ [USER_SERVICE] Failed to hash password for {}: {}", email, e);
                    e
                })?;
            
            let mut user_with_hash = new_user;
            user_with_hash.password = password_hash;
            
            println!("💾 [USER_SERVICE] Creating {} user in repository...", email);
            match self.user_repo.create(user_with_hash, auth_context).await {
                Ok(user) => {
                    println!("✅ [USER_SERVICE] {} user created successfully: {}", email, user.email);
                    Ok(())
                },
                Err(e) => {
                    println!("❌ [USER_SERVICE] Failed to create {} user: {}", email, e);
                    Err(ServiceError::Domain(e))
                }
            }
        };
        
        // Create admin account if needed
        create_account_if_needed("admin@example.com".to_string(), "Admin123!".to_string(), "System Administrator".to_string(), "admin".to_string()).await?;
        
        // Create team lead account if needed
        create_account_if_needed("lead@example.com".to_string(), "Lead123!".to_string(), "Field Team Lead".to_string(), "field_tl".to_string()).await?;

        // Create officer account if needed
        create_account_if_needed("officer@example.com".to_string(), "Officer123!".to_string(), "Field Officer".to_string(), "field".to_string()).await?;

        println!("🎉 [USER_SERVICE] All default accounts initialized successfully!");
        log::info!("Initialized default admin, team lead, and officer accounts.");
        Ok(())
    }

    /// Initialize basic test data for user domain only
    pub async fn initialize_test_data(&self, auth_context: &AuthContext) -> ServiceResult<()> {
        println!("🧪 [USER_SERVICE] Starting user domain test data initialization...");
        
        // Only check user-related data (this is appropriate for user service)
        let pool = crate::globals::get_db_pool()
            .map_err(|e| ServiceError::Domain(crate::errors::DomainError::Internal(format!("Failed to get DB pool: {}", e))))?;
        
        let user_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users")
            .fetch_one(&pool)
            .await
            .map_err(|e| ServiceError::Domain(crate::errors::DomainError::Internal(format!("Failed to count users: {}", e))))?;
        
        println!("👤 [USER_SERVICE] Current user count: {}", user_count);
        
        // Could add user-specific test data here if needed
        // For example: create test user accounts, user preferences, etc.
        
        println!("✅ [USER_SERVICE] User domain test data initialization completed!");
        println!("ℹ️ [USER_SERVICE] Note: Each domain should implement its own test data initialization");
        println!("💡 [USER_SERVICE] Suggestion: Create domain-specific test data services or use Swift-side data population");
        
        Ok(())
    }

    /// Update an existing user and return enriched response
    pub async fn update_user_with_response(&self, id: Uuid, update: UpdateUser, auth: &AuthContext) -> ServiceResult<UserResponse> {
        let updated_user = self.update_user(id, update, auth).await?;
        let response = UserResponse::from(updated_user);
        self.enrich_response(response).await
    }
}