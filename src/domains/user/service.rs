use crate::errors::{ServiceError, ServiceResult, DomainError};
use crate::domains::user::types::{User, NewUser, UpdateUser, UserResponse};
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
        Ok(user.into())
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
        Ok(users.into_iter().map(|u| u.into()).collect())
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
    
    /// Initialize default admin, team lead, and officer accounts
    pub async fn initialize_default_accounts(&self, auth_context: &AuthContext) -> ServiceResult<()> {
        println!("👥 [USER_SERVICE] Starting default account initialization");
        println!("👤 [USER_SERVICE] Auth context - User: {}, Role: {:?}", auth_context.user_id, auth_context.role);
        
        // Create admin directly through repository to bypass permission checks
        println!("🔧 [USER_SERVICE] Creating admin account...");
        let admin = NewUser {
            email: "admin@example.com".to_string(),
            password: "Admin123!".to_string(),
            name: "System Administrator".to_string(),
            role: "admin".to_string(),
            active: true,
            created_by_user_id: None, // System created, not tied to context user
        };
        
        println!("🔐 [USER_SERVICE] Hashing admin password...");
        let admin_password_hash = self.auth_service.hash_password(&admin.password)
            .map_err(|e| {
                println!("❌ [USER_SERVICE] Failed to hash admin password: {}", e);
                e
            })?;
        
        let mut admin_with_hash = admin;
        admin_with_hash.password = admin_password_hash;
        
        println!("💾 [USER_SERVICE] Creating admin user in repository...");
        match self.user_repo.create(admin_with_hash, auth_context).await {
            Ok(user) => {
                println!("✅ [USER_SERVICE] Admin user created successfully: {}", user.email);
            },
            Err(e) => {
                println!("❌ [USER_SERVICE] Failed to create admin user: {}", e);
                return Err(ServiceError::Domain(e));
            }
        }
        
        // Create default Team Lead account
        println!("🔧 [USER_SERVICE] Creating team lead account...");
        let team_lead = NewUser {
            email: "lead@example.com".to_string(),
            password: "Lead123!".to_string(), // Should be changed on first login
            name: "Field Team Lead".to_string(),
            role: "field_tl".to_string(),
            active: true,
            created_by_user_id: None, // System created
        };
        
        println!("🔐 [USER_SERVICE] Hashing team lead password...");
        let tl_password_hash = self.auth_service.hash_password(&team_lead.password)
            .map_err(|e| {
                println!("❌ [USER_SERVICE] Failed to hash team lead password: {}", e);
                e
            })?;
        
        let mut tl_with_hash = team_lead;
        tl_with_hash.password = tl_password_hash;
        
        println!("💾 [USER_SERVICE] Creating team lead user in repository...");
        match self.user_repo.create(tl_with_hash, auth_context).await {
            Ok(user) => {
                println!("✅ [USER_SERVICE] Team lead user created successfully: {}", user.email);
            },
            Err(e) => {
                println!("❌ [USER_SERVICE] Failed to create team lead user: {}", e);
                return Err(ServiceError::Domain(e));
            }
        }

        // Create default Officer account
        println!("🔧 [USER_SERVICE] Creating officer account...");
        let officer = NewUser {
            email: "officer@example.com".to_string(),
            password: "Officer123!".to_string(), // Should be changed on first login
            name: "Field Officer".to_string(),
            role: "field".to_string(),
            active: true,
            created_by_user_id: None, // System created
        };
        
        println!("🔐 [USER_SERVICE] Hashing officer password...");
        let officer_password_hash = self.auth_service.hash_password(&officer.password)
            .map_err(|e| {
                println!("❌ [USER_SERVICE] Failed to hash officer password: {}", e);
                e
            })?;
        
        let mut officer_with_hash = officer;
        officer_with_hash.password = officer_password_hash;
        
        println!("💾 [USER_SERVICE] Creating officer user in repository...");
        match self.user_repo.create(officer_with_hash, auth_context).await {
            Ok(user) => {
                println!("✅ [USER_SERVICE] Officer user created successfully: {}", user.email);
            },
            Err(e) => {
                println!("❌ [USER_SERVICE] Failed to create officer user: {}", e);
                return Err(ServiceError::Domain(e));
            }
        }

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
}