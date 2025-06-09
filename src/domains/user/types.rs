use crate::errors::{DomainError, DomainResult};
use crate::validation::{Validate, ValidationBuilder};
use crate::types::UserRole;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use sqlx::FromRow;

/// Core User entity - represents a user in the system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub email_updated_at: Option<DateTime<Utc>>,
    pub email_updated_by: Option<Uuid>,
    pub email_updated_by_device_id: Option<Uuid>,
    pub password_hash: String,
    pub name: String,
    pub name_updated_at: Option<DateTime<Utc>>,
    pub name_updated_by: Option<Uuid>,
    pub name_updated_by_device_id: Option<Uuid>,
    pub role: UserRole,
    pub role_updated_at: Option<DateTime<Utc>>,
    pub role_updated_by: Option<Uuid>,
    pub role_updated_by_device_id: Option<Uuid>,
    pub last_login: Option<DateTime<Utc>>,
    pub active: bool,
    pub active_updated_at: Option<DateTime<Utc>>,
    pub active_updated_by: Option<Uuid>,
    pub active_updated_by_device_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by_user_id: Option<Uuid>,
    pub created_by_device_id: Option<Uuid>,
    pub updated_by_user_id: Option<Uuid>,
    pub updated_by_device_id: Option<Uuid>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub deleted_by_user_id: Option<Uuid>,
    pub deleted_by_device_id: Option<Uuid>,
}

impl User {
    // Helper to check if user is deleted
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }
    
    // Helper to check if user is active
    pub fn is_active(&self) -> bool {
        self.active && !self.is_deleted()
    }
    
    // Helper to check if user is admin
    pub fn is_admin(&self) -> bool {
        matches!(self.role, UserRole::Admin)
    }
}

/// NewUser DTO - used when creating a new user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewUser {
    pub email: String,
    pub password: String, // Plain text password (will be hashed)
    pub name: String,
    pub role: String,
    pub active: bool,
    pub created_by_user_id: Option<Uuid>,
}

impl Validate for NewUser {
    fn validate(&self) -> DomainResult<()> {
        // Validate email
        ValidationBuilder::new("email", Some(self.email.clone()))
            .required()
            .email()
            .validate()?;
            
        // Validate password (min length 8)
        ValidationBuilder::new("password", Some(self.password.clone()))
            .required()
            .min_length(8)
            .validate()?;
            
        // Validate name (required, min length 2)
        ValidationBuilder::new("name", Some(self.name.clone()))
            .required()
            .min_length(2)
            .max_length(50)
            .validate()?;
            
        // Validate role (must be one of allowed values)
        ValidationBuilder::new("role", Some(self.role.clone()))
            .required()
            .one_of(&["admin", "field_tl", "field"], Some("Invalid role"))
            .validate()?;
            
        Ok(())
    }
}

/// UpdateUser DTO - used when updating an existing user
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateUser {
    pub email: Option<String>,
    pub password: Option<String>, // Plain text password (will be hashed)
    pub name: Option<String>,
    pub role: Option<String>,
    pub active: Option<bool>,
    pub updated_by_user_id: Uuid,
}

impl Validate for UpdateUser {
    fn validate(&self) -> DomainResult<()> {
        // Validate email if provided
        if let Some(email) = &self.email {
            ValidationBuilder::new("email", Some(email.clone()))
                .email()
                .validate()?;
        }
        
        // Validate password if provided (min length 8)
        if let Some(password) = &self.password {
            ValidationBuilder::new("password", Some(password.clone()))
                .min_length(8)
                .validate()?;
        }
        
        // Validate name if provided (min length 2)
        if let Some(name) = &self.name {
            ValidationBuilder::new("name", Some(name.clone()))
                .min_length(2)
                .max_length(50)
                .validate()?;
        }
        
        // Validate role if provided (must be one of allowed values)
        if let Some(role) = &self.role {
            ValidationBuilder::new("role", Some(role.clone()))
                .one_of(&["admin", "field_tl", "field"], Some("Invalid role"))
                .validate()?;
        }
        
        Ok(())
    }
}

impl UpdateUser {
    /// Check whether the update payload carries any field changes.
    /// This is useful to short-circuit update logic when nothing would be modified.
    pub fn is_empty_update(&self) -> bool {
        self.email.is_none()
            && self.password.is_none()
            && self.name.is_none()
            && self.role.is_none()
            && self.active.is_none()
    }
}

/// Credentials DTO - used for login
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credentials {
    pub email: String,
    pub password: String,
}

impl Validate for Credentials {
    fn validate(&self) -> DomainResult<()> {
        // Validate email
        ValidationBuilder::new("email", Some(self.email.clone()))
            .required()
            .email()
            .validate()?;
            
        // Validate password (required)
        ValidationBuilder::new("password", Some(self.password.clone()))
            .required()
            .validate()?;
            
        Ok(())
    }
}

/// UserRow - SQLite row representation for mapping from database
#[derive(Debug, Clone, FromRow)]
pub struct UserRow {
    pub id: String,
    pub email: String,
    pub email_updated_at: Option<String>,
    pub email_updated_by: Option<String>,
    pub email_updated_by_device_id: Option<String>,
    pub password_hash: String,
    pub name: String,
    pub name_updated_at: Option<String>,
    pub name_updated_by: Option<String>,
    pub name_updated_by_device_id: Option<String>,
    pub role: String,
    pub role_updated_at: Option<String>,
    pub role_updated_by: Option<String>,
    pub role_updated_by_device_id: Option<String>,
    pub last_login: Option<String>,
    pub active: i64,
    pub active_updated_at: Option<String>,
    pub active_updated_by: Option<String>,
    pub active_updated_by_device_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub created_by_user_id: Option<String>,
    pub created_by_device_id: Option<String>,
    pub updated_by_user_id: Option<String>,
    pub updated_by_device_id: Option<String>,
    pub deleted_at: Option<String>,
    pub deleted_by_user_id: Option<String>,
    pub deleted_by_device_id: Option<String>,
}

impl UserRow {
    /// Convert database row to domain entity
    pub fn into_entity(self) -> DomainResult<User> {
        let parse_uuid = |s: &Option<String>| -> Option<DomainResult<Uuid>> {
            s.as_ref().map(|id| {
                Uuid::parse_str(id).map_err(|_| DomainError::InvalidUuid(id.clone()))
            })
        };
        
        // Helper to parse optional UUID string, specific for device IDs to give clearer error context
        let parse_optional_uuid = |s: &Option<String>, field_name: &str| -> DomainResult<Option<Uuid>> {
            match s {
                Some(id_str) => Uuid::parse_str(id_str)
                    .map(Some)
                    .map_err(|_| DomainError::Validation(crate::errors::ValidationError::format(
                        field_name, &format!("Invalid UUID format for {}: {}", field_name, id_str)
                    ))),
                None => Ok(None),
            }
        };
        
        let parse_datetime = |s: &Option<String>| -> Option<DomainResult<DateTime<Utc>>> {
            s.as_ref().map(|dt| {
                DateTime::parse_from_rfc3339(dt)
                    .map(|dt| dt.with_timezone(&Utc))
                    .map_err(|_| DomainError::Internal(format!("Invalid date format: {}", dt)))
            })
        };
        
        Ok(User {
            id: Uuid::parse_str(&self.id)
                .map_err(|_| DomainError::InvalidUuid(self.id))?,
            email: self.email,
            email_updated_at: parse_datetime(&self.email_updated_at)
                .transpose()?,
            email_updated_by: parse_uuid(&self.email_updated_by)
                .transpose()?,
            email_updated_by_device_id: parse_optional_uuid(&self.email_updated_by_device_id, "email_updated_by_device_id")?,
            password_hash: self.password_hash,
            name: self.name,
            name_updated_at: parse_datetime(&self.name_updated_at)
                .transpose()?,
            name_updated_by: parse_uuid(&self.name_updated_by)
                .transpose()?,
            name_updated_by_device_id: parse_optional_uuid(&self.name_updated_by_device_id, "name_updated_by_device_id")?,
            role: UserRole::from_str(&self.role)
                .ok_or_else(|| DomainError::Internal(format!("Invalid role: {}", self.role)))?,
            role_updated_at: parse_datetime(&self.role_updated_at)
                .transpose()?,
            role_updated_by: parse_uuid(&self.role_updated_by)
                .transpose()?,
            role_updated_by_device_id: parse_optional_uuid(&self.role_updated_by_device_id, "role_updated_by_device_id")?,
            last_login: parse_datetime(&self.last_login)
                .transpose()?,
            active: self.active != 0,
            active_updated_at: parse_datetime(&self.active_updated_at)
                .transpose()?,
            active_updated_by: parse_uuid(&self.active_updated_by)
                .transpose()?,
            active_updated_by_device_id: parse_optional_uuid(&self.active_updated_by_device_id, "active_updated_by_device_id")?,
            created_at: DateTime::parse_from_rfc3339(&self.created_at)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|_| DomainError::Internal(format!("Invalid date format: {}", self.created_at)))?,
            updated_at: DateTime::parse_from_rfc3339(&self.updated_at)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|_| DomainError::Internal(format!("Invalid date format: {}", self.updated_at)))?,
            created_by_user_id: parse_uuid(&self.created_by_user_id)
                .transpose()?,
            created_by_device_id: parse_optional_uuid(&self.created_by_device_id, "created_by_device_id")?,
            updated_by_user_id: parse_uuid(&self.updated_by_user_id)
                .transpose()?,
            updated_by_device_id: parse_optional_uuid(&self.updated_by_device_id, "updated_by_device_id")?,
            deleted_at: parse_datetime(&self.deleted_at)
                .transpose()?,
            deleted_by_user_id: parse_uuid(&self.deleted_by_user_id)
                .transpose()?,
            deleted_by_device_id: parse_optional_uuid(&self.deleted_by_device_id, "deleted_by_device_id")?,
        })
    }
}

/// UserResponse DTO - used for API responses (excludes sensitive fields)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserResponse {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub role: String,
    pub active: bool,
    pub last_login: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<User> for UserResponse {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            email: user.email,
            name: user.name,
            role: user.role.as_str().to_string(),
            active: user.active,
            last_login: user.last_login.map(|dt| dt.to_rfc3339()),
            created_at: user.created_at.to_rfc3339(),
            updated_at: user.updated_at.to_rfc3339(),
        }
    }
}

/// Provides a summary of user counts by role and status.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UserStats {
    pub total: i64,
    pub active: i64,
    pub inactive: i64,
    pub admin: i64,
    pub field_tl: i64,
    pub field: i64,
}