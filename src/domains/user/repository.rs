// user/repository.rs

use crate::errors::{DbError, DomainError, DomainResult, ValidationError};
use crate::domains::user::types::{User, NewUser, UpdateUser, UserRow};
use crate::auth::AuthContext;
use crate::domains::sync::types::{ChangeLogEntry, ChangeOperationType, MergeOutcome};
use crate::domains::core::repository::{HardDeletable, FindById};
use uuid::Uuid;
use chrono::{Utc, DateTime};
use sqlx::{SqlitePool, query, query_as, query_scalar, Transaction, Sqlite};
use async_trait::async_trait;
use std::sync::Arc;
use crate::domains::sync::repository::ChangeLogRepository;
use serde::{Deserialize, Serialize};

/// User repository trait
#[async_trait]
pub trait UserRepository: Send + Sync + FindById<User> + HardDeletable + MergeableEntityRepository<User> {
    /// Find a user by email
    async fn find_by_email(&self, email: &str) -> DomainResult<User>;
    
    /// Find all users
    async fn find_all(&self) -> DomainResult<Vec<User>>;
    
    /// Create a new user
    async fn create(&self, user: NewUser, auth: &AuthContext) -> DomainResult<User>;
    
    /// Update an existing user
    async fn update(&self, id: Uuid, update: UpdateUser, auth: &AuthContext) -> DomainResult<User>;
    
    /// Update last login timestamp
    async fn update_last_login(&self, id: Uuid) -> DomainResult<()>;
    
    /// Check if email is unique
    async fn is_email_unique(&self, email: &str, exclude_id: Option<Uuid>) -> DomainResult<bool>;
}

/// MergeableEntityRepository trait definition
#[async_trait]
pub trait MergeableEntityRepository<E>: Send + Sync where E: Send + 'static {
    fn entity_name(&self) -> &'static str;

    async fn merge_remote_change<'t>(
        &self,
        tx: &mut Transaction<'t, Sqlite>,
        remote_change: &ChangeLogEntry,
    ) -> DomainResult<MergeOutcome>;
}

/// SQLite implementation of UserRepository
pub struct SqliteUserRepository {
    pool: SqlitePool,
    change_log_repo: Arc<dyn ChangeLogRepository + Send + Sync>,
}

/// Full user data structure for sync operations
#[derive(Serialize, Deserialize, Debug, Clone)]
struct UserFullState {
    id: Uuid,
    email: String,
    email_updated_at: Option<DateTime<Utc>>,
    email_updated_by: Option<Uuid>,
    password_hash: String,
    name: String,
    name_updated_at: Option<DateTime<Utc>>,
    name_updated_by: Option<Uuid>,
    role: String,
    role_updated_at: Option<DateTime<Utc>>,
    role_updated_by: Option<Uuid>,
    active: bool,
    active_updated_at: Option<DateTime<Utc>>,
    active_updated_by: Option<Uuid>,
    last_login: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    created_by_user_id: Option<Uuid>,
    updated_by_user_id: Option<Uuid>,
}

impl SqliteUserRepository {
    pub const ENTITY_TABLE: &'static str = "users";
    
    /// Create a new repository instance
    pub fn new(pool: SqlitePool, change_log_repo: Arc<dyn ChangeLogRepository + Send + Sync>) -> Self {
        Self { pool, change_log_repo }
    }
    
    // Helper to convert a device_id string to Uuid
    fn parse_device_id(device_id_str: &str) -> Option<Uuid> {
        Uuid::parse_str(device_id_str).ok()
    }
    
    // Helper function to map UserRow to User entity
    fn map_row_to_entity(row: UserRow) -> DomainResult<User> {
        row.into_entity()
    }
    
    // Helper to find user by ID within a transaction
    async fn find_by_id_with_tx<'t>(
        &self,
        id: Uuid,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<Option<User>> {
        let row_opt = query_as::<_, UserRow>(
            "SELECT * FROM users WHERE id = ? AND deleted_at IS NULL"
        )
        .bind(id.to_string())
        .fetch_optional(&mut **tx)
        .await
        .map_err(|e| DomainError::Database(DbError::from(e)))?;
        
        match row_opt {
            Some(row) => Ok(Some(Self::map_row_to_entity(row)?)),
            None => Ok(None),
        }
    }
    
    // Helper to log change entries consistently
    async fn log_change_entry<'t>(
        &self,
        entry: ChangeLogEntry,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<()> {
        self.change_log_repo.create_change_log_with_tx(&entry, tx).await
    }
    
    // Helper to update user from full state
    async fn update_user_from_full_state<'t>(
        &self,
        tx: &mut Transaction<'t, Sqlite>,
        entity_id: Uuid,
        state: &UserFullState,
    ) -> DomainResult<()> {
        query(
            r#"UPDATE users SET 
                email = ?,
                email_updated_at = ?,
                email_updated_by = ?,
                password_hash = ?,
                name = ?,
                name_updated_at = ?,
                name_updated_by = ?,
                role = ?,
                role_updated_at = ?,
                role_updated_by = ?,
                active = ?,
                active_updated_at = ?,
                active_updated_by = ?,
                last_login = ?,
                updated_at = ?,
                updated_by_user_id = ?
            WHERE id = ?"#
        )
        .bind(&state.email)
        .bind(state.email_updated_at.map(|dt| dt.to_rfc3339()))
        .bind(state.email_updated_by.map(|id| id.to_string()))
        .bind(&state.password_hash)
        .bind(&state.name)
        .bind(state.name_updated_at.map(|dt| dt.to_rfc3339()))
        .bind(state.name_updated_by.map(|id| id.to_string()))
        .bind(&state.role)
        .bind(state.role_updated_at.map(|dt| dt.to_rfc3339()))
        .bind(state.role_updated_by.map(|id| id.to_string()))
        .bind(if state.active { 1 } else { 0 })
        .bind(state.active_updated_at.map(|dt| dt.to_rfc3339()))
        .bind(state.active_updated_by.map(|id| id.to_string()))
        .bind(state.last_login.map(|dt| dt.to_rfc3339()))
        .bind(state.updated_at.to_rfc3339())
        .bind(state.updated_by_user_id.map(|id| id.to_string()))
        .bind(entity_id.to_string())
        .execute(&mut **tx)
        .await
        .map_err(|e| DomainError::Database(DbError::from(e)))?;
        
        Ok(())
    }
    
    // Helper to log field update
    async fn log_field_update<'t>(
        &self,
        tx: &mut Transaction<'t, Sqlite>,
        entity_id: Uuid,
        field: &str,
        old_value: Option<String>,
        new_value: Option<String>,
        auth: &AuthContext,
        timestamp: DateTime<Utc>,
    ) -> DomainResult<()> {
        // Don't log if there's no actual change (except password)
        if old_value == new_value && field != "password_hash" {
            return Ok(());
        }
        
        let entry = ChangeLogEntry {
            operation_id: Uuid::new_v4(),
            entity_table: Self::ENTITY_TABLE.to_string(),
            entity_id,
            operation_type: ChangeOperationType::Update,
            field_name: Some(field.to_string()),
            old_value: if field == "password_hash" { None } else { old_value },
            new_value: if field == "password_hash" { None } else { new_value },
            document_metadata: None,
            timestamp,
            user_id: auth.user_id,
            device_id: Self::parse_device_id(&auth.device_id),
            sync_batch_id: None,
            processed_at: None,
            sync_error: None,
        };
        
        self.log_change_entry(entry, tx).await
    }
}

// Implement FindById for SqliteUserRepository
#[async_trait]
impl FindById<User> for SqliteUserRepository {
    async fn find_by_id(&self, id: Uuid) -> DomainResult<User> {
        let row = query_as::<_, UserRow>(
            "SELECT * FROM users WHERE id = ? AND deleted_at IS NULL"
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DomainError::Database(DbError::from(e)))?
        .ok_or_else(|| DomainError::EntityNotFound(Self::ENTITY_TABLE.to_string(), id))?;
        
        Self::map_row_to_entity(row)
    }
}

// Implement HardDeletable for SqliteUserRepository
#[async_trait]
impl HardDeletable for SqliteUserRepository {
    fn entity_name(&self) -> &'static str {
        Self::ENTITY_TABLE
    }

    async fn hard_delete_with_tx(
        &self,
        id: Uuid,
        _auth: &AuthContext,
        tx: &mut Transaction<'_, Sqlite>,
    ) -> DomainResult<()> {
         // Check if user exists first to return correct error
        let exists = query_scalar::<_, String>("SELECT id FROM users WHERE id = ?")
            .bind(id.to_string())
            .fetch_optional(&mut **tx)
            .await
            .map_err(|e| DomainError::Database(DbError::from(e)))?
            .is_some();
            
        if !exists {
            return Err(DomainError::EntityNotFound(Self::ENTITY_TABLE.to_string(), id));
        }
            
        // Hard delete the user
        query("DELETE FROM users WHERE id = ?")
            .bind(id.to_string())
            .execute(&mut **tx)
            .await
            .map_err(|e| DomainError::Database(DbError::from(e)))?;
            
        Ok(())
    }
    
    async fn hard_delete(&self, id: Uuid, auth: &AuthContext) -> DomainResult<()> {
        log::warn!("Direct hard_delete called on UserRepository for {}, bypassing BaseDeleteService.", id);
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        
        match self.hard_delete_with_tx(id, auth, &mut tx).await {
            Ok(()) => {
                tx.commit().await.map_err(DbError::from)?;
                Ok(())
            },
            Err(e) => {
                let _ = tx.rollback().await;
                Err(e)
            }
        }
    }
}

// Implement MergeableEntityRepository for SqliteUserRepository
#[async_trait]
impl MergeableEntityRepository<User> for SqliteUserRepository {
    fn entity_name(&self) -> &'static str {
        Self::ENTITY_TABLE
    }

    async fn merge_remote_change<'t>(
        &self,
        tx: &mut Transaction<'t, Sqlite>,
        remote_change: &ChangeLogEntry,
    ) -> DomainResult<MergeOutcome> {
        log::debug!("Merging remote change for entity_id: {}, table: {}, operation: {:?}", 
            remote_change.entity_id, remote_change.entity_table, remote_change.operation_type);

        // Ensure the change is for the users table
        if remote_change.entity_table != Self::ENTITY_TABLE {
            return Err(DomainError::Internal(format!(
                "UserRepository received change for incorrect table: {}",
                remote_change.entity_table
            )));
        }

        let entity_id = remote_change.entity_id;

        match remote_change.operation_type {
            ChangeOperationType::Create => {
                let user_data_json = remote_change.new_value.as_ref()
                    .ok_or_else(|| DomainError::Validation(ValidationError::custom("Missing new_value for create operation")))?;
                
                let payload: UserFullState = serde_json::from_str(user_data_json)
                    .map_err(|e| DomainError::Validation(ValidationError::format("new_value_user_create", &format!("Invalid JSON: {}", e))))?;
                
                // Check if entity already exists (ID conflict)
                if let Some(local_user) = self.find_by_id_with_tx(entity_id, tx).await? {
                    log::warn!("Conflict: Remote CREATE for user ID {} which already exists locally.", entity_id);
                    
                    // If remote timestamp is newer, overwrite local
                    if payload.updated_at > local_user.updated_at {
                        log::info!("Remote CREATE for {} wins due to newer timestamp. Overwriting local record.", entity_id);
                        self.update_user_from_full_state(tx, entity_id, &payload).await?;
                        return Ok(MergeOutcome::Updated(entity_id));
                    } else {
                        // Local is newer or same, keep it
                        return Ok(MergeOutcome::NoOp(format!("Local user {} is newer or same", entity_id)));
                    }
                }
                
                // No local user with this ID, create it
                let active_val = if payload.active { 1 } else { 0 };
                
                query(
                    "INSERT INTO users (
                        id, email, email_updated_at, email_updated_by,
                        password_hash, name, name_updated_at, name_updated_by,
                        role, role_updated_at, role_updated_by,
                        active, active_updated_at, active_updated_by,
                        last_login, created_at, updated_at, created_by_user_id, updated_by_user_id
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
                )
                .bind(entity_id.to_string())
                .bind(&payload.email)
                .bind(payload.email_updated_at.map(|dt| dt.to_rfc3339()))
                .bind(payload.email_updated_by.map(|id| id.to_string()))
                .bind(&payload.password_hash)
                .bind(&payload.name)
                .bind(payload.name_updated_at.map(|dt| dt.to_rfc3339()))
                .bind(payload.name_updated_by.map(|id| id.to_string()))
                .bind(&payload.role)
                .bind(payload.role_updated_at.map(|dt| dt.to_rfc3339()))
                .bind(payload.role_updated_by.map(|id| id.to_string()))
                .bind(active_val)
                .bind(payload.active_updated_at.map(|dt| dt.to_rfc3339()))
                .bind(payload.active_updated_by.map(|id| id.to_string()))
                .bind(payload.last_login.map(|dt| dt.to_rfc3339()))
                .bind(payload.created_at.to_rfc3339())
                .bind(payload.updated_at.to_rfc3339())
                .bind(payload.created_by_user_id.map(|id| id.to_string()))
                .bind(payload.updated_by_user_id.map(|id| id.to_string()))
                .execute(&mut **tx)
                .await
                .map_err(|e| DomainError::Database(DbError::from(e)))?;
                
                log::info!("Applied remote CREATE for user ID {}", entity_id);
                Ok(MergeOutcome::Created(entity_id))
            },
            
            ChangeOperationType::Update => {
                // Get the local user first
                let local_user_opt = self.find_by_id_with_tx(entity_id, tx).await?;
                
                if local_user_opt.is_none() {
                    log::warn!("Remote UPDATE for user ID {} which does not exist locally.", entity_id);
                    return Ok(MergeOutcome::NoOp(format!("Remote UPDATE for non-existent local user ID {}.", entity_id)));
                }
                
                let local_user = local_user_opt.unwrap();
                
                // For field-level updates
                if let Some(field_name) = &remote_change.field_name {
                    let new_value = remote_change.new_value.as_ref()
                        .ok_or_else(|| DomainError::Internal("Missing new_value for update operation".to_string()))?;
                    
                    // Check field-level timestamp before applying (Last-Write-Wins per field)
                    let field_updated_at: Option<DateTime<Utc>> = match field_name.as_str() {
                        "email" => local_user.email_updated_at,
                        "name" => local_user.name_updated_at,
                        "role" => local_user.role_updated_at,
                        "active" => local_user.active_updated_at,
                        _ => None,
                    };

                    if let Some(local_ts) = field_updated_at {
                        if remote_change.timestamp <= local_ts {
                            log::info!("Skipping update for field '{}' as local is newer", field_name);
                            return Ok(MergeOutcome::NoOp(format!("Local field '{}' is newer", field_name)));
                        }
                    }
                    
                    // Apply field-specific update
                    match field_name.as_str() {
                        "email" => {
                            let email_value: String = match serde_json::from_str(new_value) {
                                Ok(val) => val,
                                Err(_) => {
                                    if new_value.starts_with('"') && new_value.ends_with('"') {
                                        return Err(DomainError::Validation(ValidationError::format("email", &format!("Invalid JSON string format for email: {}", new_value))));
                                    } else {
                                        new_value.to_string()
                                    }
                                }
                            };
                            
                            query(
                                "UPDATE users SET email = ?, email_updated_at = ?, email_updated_by = ?, updated_at = ? WHERE id = ?"
                            )
                            .bind(&email_value)
                            .bind(remote_change.timestamp.to_rfc3339())
                            .bind(remote_change.user_id.to_string())
                            .bind(remote_change.timestamp.to_rfc3339())
                            .bind(entity_id.to_string())
                            .execute(&mut **tx)
                            .await
                            .map_err(|e| DomainError::Database(DbError::from(e)))?;
                        },
                        "name" => {
                            let name_value: String = match serde_json::from_str(new_value) {
                                Ok(val) => val,
                                Err(_) => {
                                    if new_value.starts_with('"') && new_value.ends_with('"') {
                                        return Err(DomainError::Validation(ValidationError::format("name", &format!("Invalid JSON string format for name: {}", new_value))));
                                    } else {
                                        new_value.to_string()
                                    }
                                }
                            };
                            
                            query(
                                "UPDATE users SET name = ?, name_updated_at = ?, name_updated_by = ?, updated_at = ? WHERE id = ?"
                            )
                            .bind(&name_value)
                            .bind(remote_change.timestamp.to_rfc3339())
                            .bind(remote_change.user_id.to_string())
                            .bind(remote_change.timestamp.to_rfc3339())
                            .bind(entity_id.to_string())
                            .execute(&mut **tx)
                            .await
                            .map_err(|e| DomainError::Database(DbError::from(e)))?;
                        },
                        "role" => {
                            let role_value: String = match serde_json::from_str(new_value) {
                                Ok(val) => val,
                                Err(_) => {
                                    if new_value.starts_with('"') && new_value.ends_with('"') {
                                        return Err(DomainError::Validation(ValidationError::format("role", &format!("Invalid JSON string format for role: {}", new_value))));
                                    } else {
                                        new_value.to_string()
                                    }
                                }
                            };
                            
                            query(
                                "UPDATE users SET role = ?, role_updated_at = ?, role_updated_by = ?, updated_at = ? WHERE id = ?"
                            )
                            .bind(&role_value)
                            .bind(remote_change.timestamp.to_rfc3339())
                            .bind(remote_change.user_id.to_string())
                            .bind(remote_change.timestamp.to_rfc3339())
                            .bind(entity_id.to_string())
                            .execute(&mut **tx)
                            .await
                            .map_err(|e| DomainError::Database(DbError::from(e)))?;
                        },
                        "active" => {
                            let active_value: bool = serde_json::from_str(new_value).or_else(|_parse_err| {
                                match new_value.to_lowercase().as_str() {
                                    "true" | "1" => Ok(true),
                                    "false" | "0" => Ok(false),
                                    _ => Err(DomainError::Validation(ValidationError::format("active", &format!("Invalid boolean value for active: {}", new_value)))),
                                }
                            })?;
                            
                            query(
                                "UPDATE users SET active = ?, active_updated_at = ?, active_updated_by = ?, updated_at = ? WHERE id = ?"
                            )
                            .bind(if active_value { 1 } else { 0 })
                            .bind(remote_change.timestamp.to_rfc3339())
                            .bind(remote_change.user_id.to_string())
                            .bind(remote_change.timestamp.to_rfc3339())
                            .bind(entity_id.to_string())
                            .execute(&mut **tx)
                            .await
                            .map_err(|e| DomainError::Database(DbError::from(e)))?;
                        },
                        "password_hash" => {
                            let password_value: String = match serde_json::from_str(new_value) {
                                Ok(val) => val,
                                Err(_) => {
                                    if new_value.starts_with('"') && new_value.ends_with('"') {
                                        // For password_hash, it's less about "format" and more that it should be a hash string
                                        return Err(DomainError::Validation(ValidationError::format("password_hash", "Invalid JSON string for password_hash")));
                                    } else {
                                        new_value.to_string() 
                                    }
                                }
                            };
                            
                            query(
                                "UPDATE users SET password_hash = ?, updated_at = ? WHERE id = ?"
                            )
                            .bind(&password_value)
                            .bind(remote_change.timestamp.to_rfc3339())
                            .bind(entity_id.to_string())
                            .execute(&mut **tx)
                            .await
                            .map_err(|e| DomainError::Database(DbError::from(e)))?;
                        },
                        _ => {
                            log::warn!("Unhandled field update: {} for user {}", field_name, entity_id);
                            return Ok(MergeOutcome::NoOp(format!("Unhandled field: {}", field_name)));
                        }
                    }
                } else {
                    // Full entity update (rarely used)
                    log::warn!("Full user update received for {}", entity_id);
                    
                    if let Some(new_value) = &remote_change.new_value {
                        let update_user: UserFullState = serde_json::from_str(new_value)
                            .map_err(|e| DomainError::Validation(ValidationError::format("full_user_update", &format!("Invalid JSON: {}", e))))?;
                        
                        // If local is newer, don't update
                        if local_user.updated_at >= update_user.updated_at {
                            return Ok(MergeOutcome::NoOp("Local version is newer".to_string()));
                        }
                        
                        self.update_user_from_full_state(tx, entity_id, &update_user).await?;
                    }
                }
                
                log::info!("Applied remote UPDATE for user {}", entity_id);
                Ok(MergeOutcome::Updated(entity_id))
            },
            
            ChangeOperationType::Delete => {
                // Soft deletes are not synced - local only
                log::info!("Ignoring remote soft delete for user {}", entity_id);
                Ok(MergeOutcome::NoOp("Soft deletes are local-only".to_string()))
            },
            
            ChangeOperationType::HardDelete => {
                if self.find_by_id_with_tx(entity_id, tx).await?.is_none() {
                    return Ok(MergeOutcome::NoOp(format!("User {} already deleted", entity_id)));
                }
                
                // Hard delete just removes the user record
                // Database constraints will handle cascading to related tables
                query("DELETE FROM users WHERE id = ?")
                    .bind(entity_id.to_string())
                    .execute(&mut **tx)
                    .await
                    .map_err(|e| DomainError::Database(DbError::from(e)))?;
                
                log::info!("Applied remote HARD DELETE for user {}", entity_id);
                Ok(MergeOutcome::HardDeleted(entity_id))
            }
        }
    }
}

// Implement UserRepository for SqliteUserRepository
#[async_trait]
impl UserRepository for SqliteUserRepository {
    async fn find_by_email(&self, email: &str) -> DomainResult<User> {
        let row = query_as::<_, UserRow>(
            "SELECT * FROM users WHERE email = ? AND deleted_at IS NULL"
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DomainError::Database(DbError::from(e)))?
        .ok_or_else(|| DomainError::EntityNotFound(format!("User with email {}", email), Uuid::nil()))?;
        
        Self::map_row_to_entity(row)
    }
    
    async fn find_all(&self) -> DomainResult<Vec<User>> {
        let rows = query_as::<_, UserRow>(
            "SELECT * FROM users WHERE deleted_at IS NULL ORDER BY name"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DomainError::Database(DbError::from(e)))?;
        
        let mut users = Vec::with_capacity(rows.len());
        for row in rows {
            users.push(Self::map_row_to_entity(row)?);
        }
        
        Ok(users)
    }
    
    async fn create(&self, user: NewUser, auth: &AuthContext) -> DomainResult<User> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;

        if !self.is_email_unique(&user.email, None).await? {
            return Err(DomainError::Validation(ValidationError::unique("email")));
        }
        
        

        let create_result = async {
            let id = Uuid::new_v4();
            let now = Utc::now();
            let now_str = now.to_rfc3339();
            
            let created_by_user_id = user.created_by_user_id.unwrap_or(auth.user_id);
            let created_by_user_id_str = created_by_user_id.to_string();
            let auth_user_id_str = auth.user_id.to_string();
            
            let active_val = if user.active { 1 } else { 0 };
            
            query(
                "INSERT INTO users (
                    id, email, email_updated_at, email_updated_by,
                    password_hash, name, name_updated_at, name_updated_by,
                    role, role_updated_at, role_updated_by,
                    active, active_updated_at, active_updated_by,
                    created_at, updated_at, created_by_user_id, updated_by_user_id
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
            )
            .bind(id.to_string())
            .bind(&user.email)
            .bind(&now_str)
            .bind(&auth_user_id_str)
            .bind(&user.password)
            .bind(&user.name)
            .bind(&now_str)
            .bind(&auth_user_id_str)
            .bind(&user.role)
            .bind(&now_str)
            .bind(&auth_user_id_str)
            .bind(active_val)
            .bind(&now_str)
            .bind(&auth_user_id_str)
            .bind(&now_str)
            .bind(&now_str)
            .bind(&created_by_user_id_str)
            .bind(&auth_user_id_str)
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Database(DbError::from(e)))?;
            
            // Create full state for changelog
            let user_state = UserFullState {
                id,
                email: user.email.clone(),
                email_updated_at: Some(now),
                email_updated_by: Some(auth.user_id),
                password_hash: user.password.clone(),
                name: user.name.clone(),
                name_updated_at: Some(now),
                name_updated_by: Some(auth.user_id),
                role: user.role.clone(),
                role_updated_at: Some(now),
                role_updated_by: Some(auth.user_id),
                active: user.active,
                active_updated_at: Some(now),
                active_updated_by: Some(auth.user_id),
                last_login: None,
                created_at: now,
                updated_at: now,
                created_by_user_id: Some(created_by_user_id),
                updated_by_user_id: Some(auth.user_id),
            };
            
            let user_json = serde_json::to_string(&user_state)
                .map_err(|e| DomainError::Internal(format!("Failed to serialize user state: {}", e)))?;
            
            let entry = ChangeLogEntry {
                operation_id: Uuid::new_v4(),
                entity_table: Self::ENTITY_TABLE.to_string(),
                entity_id: id,
                operation_type: ChangeOperationType::Create,
                field_name: None,
                old_value: None,
                new_value: Some(user_json),
                document_metadata: None,
                timestamp: now,
                user_id: auth.user_id,
                device_id: Self::parse_device_id(&auth.device_id),
                sync_batch_id: None,
                processed_at: None,
                sync_error: None,
            };
            
            self.log_change_entry(entry, &mut tx).await?;
            
            // Retrieve created user
            self.find_by_id_with_tx(id, &mut tx).await?
                .ok_or_else(|| DomainError::Internal("Failed to retrieve user after creation".to_string()))
        }.await;

        match create_result {
            Ok(created_user) => {
                tx.commit().await.map_err(DbError::from)?;
                Ok(created_user)
            }
            Err(e) => {
                let _ = tx.rollback().await;
                Err(e)
            }
        }
    }
    
    async fn update(&self, id: Uuid, update_data: UpdateUser, auth: &AuthContext) -> DomainResult<User> {
        if let Some(email) = &update_data.email {
            if !self.is_email_unique(email, Some(id)).await? {
                return Err(DomainError::Validation(ValidationError::unique("email")));
            }
        }

        let mut tx = self.pool.begin().await.map_err(DbError::from)?;

        let update_result = async {
            // Get current user to compare changed fields
            let current_user = self.find_by_id_with_tx(id, &mut tx).await?
                .ok_or_else(|| DomainError::EntityNotFound(Self::ENTITY_TABLE.to_string(), id))?;

            let now = Utc::now();
            let now_str = now.to_rfc3339();
            let auth_user_id_str = auth.user_id.to_string();
            let mut changes_made = false;
            
            // Track individual field updates for better LWW conflict resolution
            if let Some(email) = &update_data.email {
                if email != &current_user.email {
                    query(
                        "UPDATE users SET email = ?, email_updated_at = ?, email_updated_by = ? WHERE id = ?"
                    )
                    .bind(email)
                    .bind(&now_str)
                    .bind(&auth_user_id_str)
                    .bind(id.to_string())
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| DomainError::Database(DbError::from(e)))?;
                    
                    self.log_field_update(
                        &mut tx, 
                        id, 
                        "email", 
                        Some(serde_json::to_string(&current_user.email).unwrap_or_default()), 
                        Some(serde_json::to_string(email).unwrap_or_default()),
                        auth,
                        now
                    ).await?;
                    
                    changes_made = true;
                }
            }
            
            if let Some(password) = &update_data.password {
                query(
                    "UPDATE users SET password_hash = ? WHERE id = ?"
                )
                .bind(password)
                .bind(id.to_string())
                .execute(&mut *tx)
                .await
                .map_err(|e| DomainError::Database(DbError::from(e)))?;
                
                // Don't include actual password in changelog
                self.log_field_update(
                    &mut tx,
                    id,
                    "password_hash",
                    None,
                    None,
                    auth,
                    now
                ).await?;
                
                changes_made = true;
            }
            
            if let Some(name) = &update_data.name {
                if name != &current_user.name {
                    query(
                        "UPDATE users SET name = ?, name_updated_at = ?, name_updated_by = ? WHERE id = ?"
                    )
                    .bind(name)
                    .bind(&now_str)
                    .bind(&auth_user_id_str)
                    .bind(id.to_string())
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| DomainError::Database(DbError::from(e)))?;
                    
                    self.log_field_update(
                        &mut tx,
                        id,
                        "name",
                        Some(serde_json::to_string(&current_user.name).unwrap_or_default()),
                        Some(serde_json::to_string(name).unwrap_or_default()),
                        auth,
                        now
                    ).await?;
                    
                    changes_made = true;
                }
            }
            
            if let Some(role) = &update_data.role {
                if role != &current_user.role.as_str() {
                    query(
                        "UPDATE users SET role = ?, role_updated_at = ?, role_updated_by = ? WHERE id = ?"
                    )
                    .bind(role)
                    .bind(&now_str)
                    .bind(&auth_user_id_str)
                    .bind(id.to_string())
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| DomainError::Database(DbError::from(e)))?;
                    
                    self.log_field_update(
                        &mut tx,
                        id,
                        "role",
                        Some(serde_json::to_string(&current_user.role.as_str()).unwrap_or_default()),
                        Some(serde_json::to_string(role).unwrap_or_default()),
                        auth,
                        now
                    ).await?;
                    
                    changes_made = true;
                }
            }
            
            if let Some(active) = update_data.active {
                if active != current_user.active {
                    query(
                        "UPDATE users SET active = ?, active_updated_at = ?, active_updated_by = ? WHERE id = ?"
                    )
                    .bind(if active { 1 } else { 0 })
                    .bind(&now_str)
                    .bind(&auth_user_id_str)
                    .bind(id.to_string())
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| DomainError::Database(DbError::from(e)))?;
                    
                    self.log_field_update(
                        &mut tx,
                        id,
                        "active",
                        Some(serde_json::to_string(&current_user.active).unwrap_or_default()),
                        Some(serde_json::to_string(&active).unwrap_or_default()),
                        auth,
                        now
                    ).await?;
                    
                    changes_made = true;
                }
            }
            
            // Update main record timestamp if any changes were made
            if changes_made {
                query(
                    "UPDATE users SET updated_at = ?, updated_by_user_id = ? WHERE id = ?"
                )
                .bind(&now_str)
                .bind(&auth_user_id_str)
                .bind(id.to_string())
                .execute(&mut *tx)
                .await
                .map_err(|e| DomainError::Database(DbError::from(e)))?;
            } else {
                log::debug!("No changes made to user {}", id);
            }
            
            // Return updated user
            self.find_by_id_with_tx(id, &mut tx).await?
                .ok_or_else(|| DomainError::EntityNotFound(Self::ENTITY_TABLE.to_string(), id))
        }.await;

        match update_result {
            Ok(updated_user) => {
                tx.commit().await.map_err(DbError::from)?;
                Ok(updated_user)
            }
            Err(e) => {
                let _ = tx.rollback().await;
                Err(e)
            }
        }
    }
    
    async fn update_last_login(&self, id: Uuid) -> DomainResult<()> {
        query("UPDATE users SET last_login = ? WHERE id = ?")
            .bind(Utc::now().to_rfc3339())
            .bind(id.to_string())
            .execute(&self.pool)
            .await
            .map_err(|e| DomainError::Database(DbError::from(e)))?;
        Ok(())
    }
    
    async fn is_email_unique(&self, email: &str, exclude_id: Option<Uuid>) -> DomainResult<bool> {
        let normalized_email = email.to_lowercase();
        let query_str = if let Some(id) = exclude_id {
            "SELECT COUNT(*) FROM users WHERE LOWER(email) = ? AND id != ? AND deleted_at IS NULL"
        } else {
            "SELECT COUNT(*) FROM users WHERE LOWER(email) = ? AND deleted_at IS NULL"
        };
        
        let mut query_builder = query_scalar::<_, i64>(query_str).bind(normalized_email);
        if let Some(id) = exclude_id {
            query_builder = query_builder.bind(id.to_string());
        }
        
        let count = query_builder
            .fetch_one(&self.pool)
            .await
            .map_err(|e| DomainError::Database(DbError::from(e)))?;
            
        Ok(count == 0)
    }
}