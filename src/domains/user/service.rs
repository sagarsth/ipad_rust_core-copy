use crate::errors::{ServiceError, ServiceResult, DomainError};
use crate::domains::user::types::{User, NewUser, UpdateUser, UserResponse};
use crate::domains::user::repository::UserRepository;
use crate::auth::{AuthContext, AuthService};
use crate::types::Permission;
use crate::validation::Validate;
use uuid::Uuid;
use std::sync::Arc;

/// Service for user-related operations
pub struct UserService {
    user_repo: Arc<dyn UserRepository>,
    auth_service: Arc<AuthService>,
}

impl UserService {
    /// Create a new user service
    pub fn new(user_repo: Arc<dyn UserRepository>, auth_service: Arc<AuthService>) -> Self {
        Self { user_repo, auth_service }
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
        
        // Hard delete user in repository
        self.user_repo.hard_delete(id, auth)
            .await
            .map_err(|e| match e {
                DomainError::DependentRecordsExist { dependencies, .. } => {
                    ServiceError::DependenciesPreventDeletion(dependencies)
                },
                other => ServiceError::Domain(other)
            })?;
            
        Ok(())
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
        // Create admin directly through repository to bypass permission checks
        let admin = NewUser {
            email: "admin@example.com".to_string(),
            password: "Admin123!".to_string(),
            name: "System Administrator".to_string(),
            role: "admin".to_string(),
            active: true,
            created_by_user_id: None, // System created, not tied to context user
        };
        let admin_password_hash = self.auth_service.hash_password(&admin.password)?;
        let mut admin_with_hash = admin;
        admin_with_hash.password = admin_password_hash;
        self.user_repo.create(admin_with_hash, auth_context)
            .await
            .map_err(ServiceError::Domain)?;
        
        // Create default Team Lead account
        let team_lead = NewUser {
            email: "lead@example.com".to_string(),
            password: "Lead123!".to_string(), // Should be changed on first login
            name: "Field Team Lead".to_string(),
            role: "field_team_lead".to_string(),
            active: true,
            created_by_user_id: None, // System created
        };
        let tl_password_hash = self.auth_service.hash_password(&team_lead.password)?;
        let mut tl_with_hash = team_lead;
        tl_with_hash.password = tl_password_hash;
        self.user_repo.create(tl_with_hash, auth_context)
            .await
            .map_err(ServiceError::Domain)?;

        // Create default Officer account
        let officer = NewUser {
            email: "officer@example.com".to_string(),
            password: "Officer123!".to_string(), // Should be changed on first login
            name: "Field Officer".to_string(),
            role: "field_officer".to_string(),
            active: true,
            created_by_user_id: None, // System created
        };
        let officer_password_hash = self.auth_service.hash_password(&officer.password)?;
        let mut officer_with_hash = officer;
        officer_with_hash.password = officer_password_hash;
        self.user_repo.create(officer_with_hash, auth_context)
            .await
            .map_err(ServiceError::Domain)?;

        log::info!("Initialized default admin, team lead, and officer accounts.");
        Ok(())
    }
}