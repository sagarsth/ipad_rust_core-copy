use uuid::Uuid;
use crate::types::{UserRole, Permission};
use crate::errors::ServiceError;

/// Represents the authentication context for the current operation
#[derive(Debug, Clone)]
pub struct AuthContext {
    /// The ID of the authenticated user
    pub user_id: Uuid,
    
    /// The role of the authenticated user
    pub role: UserRole,
    
    /// The ID of the current device
    pub device_id: String,
    
    /// Whether or not the app is currently in offline mode
    pub offline_mode: bool,
}

impl AuthContext {
    /// Create a new authentication context
    pub fn new(user_id: Uuid, role: UserRole, device_id: String, offline_mode: bool) -> Self {
        Self {
            user_id,
            role,
            device_id,
            offline_mode,
        }
    }
    
    /// Create a new authentication context for internal system operations
    pub fn internal_system_context() -> Self {
        Self {
            user_id: Uuid::nil(), // Or a specific system UUID
            role: UserRole::Admin, // System operations usually have admin privileges
            device_id: "system".to_string(),
            offline_mode: false, // System operations are typically not in offline mode
        }
    }
    
    /// Check if user has a specific permission
    pub fn has_permission(&self, permission: Permission) -> bool {
        self.role.has_permission(permission)
    }
    
    /// Authorize a specific permission, returning an error if not allowed
    pub fn authorize(&self, permission: Permission) -> Result<(), ServiceError> {
        if self.has_permission(permission) {
            Ok(())
        } else {
            Err(ServiceError::PermissionDenied(format!(
                "User does not have permission: {:?}",
                permission
            )))
        }
    }
    
    /// Authorize multiple permissions, requiring all of them
    pub fn authorize_all(&self, permissions: &[Permission]) -> Result<(), ServiceError> {
        if self.role.has_permissions(permissions) {
            Ok(())
        } else {
            Err(ServiceError::PermissionDenied(
                format!("User does not have all required permissions")
            ))
        }
    }
    
    /// Verify user is an admin
    pub fn authorize_admin(&self) -> Result<(), ServiceError> {
        if matches!(self.role, UserRole::Admin) {
            Ok(())
        } else {
            Err(ServiceError::PermissionDenied(
                "This action requires administrator privileges".to_string()
            ))
        }
    }
    
    /// Check if feature is available offline when in offline mode
    pub fn check_offline_feature(&self, feature_name: &str, available_offline: bool) -> Result<(), ServiceError> {
        if self.offline_mode && !available_offline {
            Err(ServiceError::OfflineFeatureUnavailable(feature_name.to_string()))
        } else {
            Ok(())
        }
    }
    
    /// For certain operations restricted to the user's own records
    pub fn authorize_self_or_admin(&self, resource_owner_id: &Uuid) -> Result<(), ServiceError> {
        if &self.user_id == resource_owner_id || matches!(self.role, UserRole::Admin) {
            Ok(())
        } else {
            Err(ServiceError::PermissionDenied(
                "You do not have permission to access this resource".to_string()
            ))
        }
    }
    
    /// Check if the user can hard delete records
    pub fn authorize_hard_delete(&self) -> Result<(), ServiceError> {
        if self.role.can_hard_delete() {
            Ok(())
        } else {
            Err(ServiceError::PermissionDenied(
                "You do not have permission to permanently delete records".to_string()
            ))
        }
    }
}