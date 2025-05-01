use crate::errors::{DbError, DomainError, DomainResult};
use crate::domains::user::types::{User, NewUser, UpdateUser, UserRow};
use crate::auth::AuthContext;
use crate::types::ChangeLogOperationType;
use uuid::Uuid;
use chrono::Utc;
use sqlx::{SqlitePool, query, query_as, query_scalar, Transaction, Sqlite};
use async_trait::async_trait;
use std::sync::Arc;
use crate::domains::sync::repository::ChangeLogRepository;
use crate::domains::sync::types::{ChangeLogEntry, ChangeOperationType};
use crate::domains::core::repository::{HardDeletable, FindById};

/// User repository trait
#[async_trait]
pub trait UserRepository: Send + Sync + FindById<User> + HardDeletable {
    /// Find a user by ID
    // async fn find_by_id(&self, id: Uuid) -> DomainResult<User>; // Defined by FindById
    
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

/// SQLite implementation of UserRepository
pub struct SqliteUserRepository {
    pool: SqlitePool,
    change_log_repo: Arc<dyn ChangeLogRepository + Send + Sync>,
}

impl SqliteUserRepository {
    /// Create a new repository instance
    pub fn new(pool: SqlitePool, change_log_repo: Arc<dyn ChangeLogRepository + Send + Sync>) -> Self {
        Self { pool, change_log_repo }
    }
    
    // Helper function to map UserRow to User entity
    fn map_row_to_entity(row: UserRow) -> DomainResult<User> {
        row.into_entity()
    }
    
    // Helper to find user by ID within a transaction (needed for updates)
    async fn find_by_id_with_tx<'t>(
        &self,
        id: Uuid,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<User> {
        let row = query_as::<_, UserRow>(
            "SELECT * FROM users WHERE id = ? AND deleted_at IS NULL"
        )
        .bind(id.to_string())
        .fetch_optional(&mut **tx)
        .await
        .map_err(|e| DomainError::Database(DbError::from(e)))?
        .ok_or_else(|| DomainError::EntityNotFound("User".to_string(), id))?;
        
        Self::map_row_to_entity(row)
    }
    
    // Helper to log change entries consistently
    async fn log_change_entry<'t>(
        &self,
        entry: ChangeLogEntry,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<()> {
        self.change_log_repo.create_change_log_with_tx(&entry, tx).await
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
        .ok_or_else(|| DomainError::EntityNotFound("User".to_string(), id))?;
        
        Self::map_row_to_entity(row)
    }
}

// Implement HardDeletable for SqliteUserRepository
#[async_trait]
impl HardDeletable for SqliteUserRepository {
    fn entity_name(&self) -> &'static str {
        "users"
    }

    async fn hard_delete_with_tx(
        &self,
        id: Uuid,
        _auth: &AuthContext, // Auth context might be used later for checks
        tx: &mut Transaction<'_, Sqlite>,
    ) -> DomainResult<()> {
         // Check if user exists first to return correct error
        let _ = query_scalar::<_, String>("SELECT id FROM users WHERE id = ?")
            .bind(id.to_string())
            .fetch_optional(&mut **tx)
            .await
            .map_err(|e| DomainError::Database(DbError::from(e)))?
            .ok_or_else(|| DomainError::EntityNotFound(self.entity_name().to_string(), id))?;
            
        // Hard delete the user
        let result = query("DELETE FROM users WHERE id = ?")
            .bind(id.to_string())
            .execute(&mut **tx)
            .await
            .map_err(|e| DomainError::Database(DbError::from(e)))?;
            
        // Check rows affected to confirm deletion (optional but good practice)
        if result.rows_affected() == 0 {
            // Should not happen if fetch_optional found the user, but handle defensively
            Err(DomainError::EntityNotFound(self.entity_name().to_string(), id))
        } else {
             // No logging here - BaseDeleteService handles it
            Ok(())
        }
    }
    
    // Standalone hard_delete is removed as it's handled by the service
    async fn hard_delete(&self, id: Uuid, auth: &AuthContext) -> DomainResult<()> {
        // This implementation is now effectively unused, but kept to satisfy 
        // potential direct calls if the service pattern isn't fully adopted yet.
        // It lacks the Tombstone + ChangeLog from BaseDeleteService.
        log::warn!("Direct hard_delete called on UserRepository for {}, bypassing BaseDeleteService logic.", id);
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        match self.hard_delete_with_tx(id, auth, &mut tx).await {
            Ok(()) => { tx.commit().await.map_err(DbError::from)?; Ok(()) },
            Err(e) => { let _ = tx.rollback().await; Err(e) }
        }
    }
}

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
        .ok_or_else(|| DomainError::Internal(format!("User not found with email: {}", email)))?;
        
        row.into_entity()
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
            users.push(row.into_entity()?);
        }
        
        Ok(users)
    }
    
    async fn create(&self, user: NewUser, auth: &AuthContext) -> DomainResult<User> {
        // Check if email is unique
        if !self.is_email_unique(&user.email, None).await? {
            return Err(DomainError::Validation(
                crate::errors::ValidationError::unique("email")
            ));
        }
        
        // --- Start Transaction --- 
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;

        let create_result = async {
            // Generate ID
            let id = Uuid::new_v4();
            let now = Utc::now().to_rfc3339();
            let now_dt = Utc::now(); // For logging
            let user_uuid = auth.user_id; // Capture UUID
            let device_uuid: Option<Uuid> = auth.device_id.parse().ok(); // Capture device UUID
            
            // Set created_by to the authenticated user if not specified
            let created_by = user.created_by_user_id
                .unwrap_or(auth.user_id)
                .to_string();
            
            // Default to active if not specified
            let active = if user.active { 1 } else { 0 };
            
            // Insert user
            query(
                "INSERT INTO users (
                    id, email, email_updated_at, email_updated_by,
                    password_hash, name, name_updated_at, name_updated_by,
                    role, role_updated_at, role_updated_by,
                    active, active_updated_at, active_updated_by,
                    created_at, updated_at, created_by_user_id, updated_by_user_id
                ) VALUES (
                    ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?
                )"
            )
            .bind(id.to_string())
            .bind(&user.email)
            .bind(&now)
            .bind(auth.user_id.to_string())
            .bind(&user.password)  // Note: This should be hashed before calling repository
            .bind(&user.name)
            .bind(&now)
            .bind(auth.user_id.to_string())
            .bind(&user.role)
            .bind(&now)
            .bind(auth.user_id.to_string())
            .bind(active)
            .bind(&now)
            .bind(auth.user_id.to_string())
            .bind(&now)
            .bind(&now)
            .bind(created_by)
            .bind(auth.user_id.to_string())
            .execute(&mut *tx) // Execute within transaction
            .await
            .map_err(|e| DomainError::Database(DbError::from(e)))?;
            
            // Create changelog entry
            let entry = ChangeLogEntry {
                operation_id: Uuid::new_v4(), // Generate new op ID
                entity_table: self.entity_name().to_string(), // Use entity_name()
                entity_id: id,
                operation_type: ChangeOperationType::Create,
                field_name: None,
                old_value: None,
                new_value: None,
                timestamp: now_dt,
                user_id: user_uuid,
                device_id: device_uuid,
                document_metadata: None,
                sync_batch_id: None,
                processed_at: None,
                sync_error: None,
            };
            self.log_change_entry(entry, &mut tx).await?;
            
            // Return the ID for fetching outside the transaction
            Ok(id)
        }.await;

        // --- Commit or Rollback --- 
        match create_result {
            Ok(created_id) => {
                tx.commit().await.map_err(DbError::from)?;
                // Fetch the newly created record outside the transaction
                self.find_by_id(created_id).await
            },
            Err(e) => {
                let _ = tx.rollback().await; // Ensure rollback on error
                Err(e)
            }
        }
    }
    
    async fn update(&self, id: Uuid, update: UpdateUser, auth: &AuthContext) -> DomainResult<User> {
        // Check if user exists
        // let user = self.find_by_id(id).await?; // Fetch within transaction later
        
        // Begin transaction
        let mut tx = self.pool.begin().await
            .map_err(|e| DomainError::Database(DbError::from(e)))?;
            
        // Fetch the user within the transaction to get the old state accurately
        let user = self.find_by_id_with_tx(id, &mut tx).await?;
        
        let now = Utc::now().to_rfc3339();
        let now_dt = Utc::now(); // For logging
        let user_uuid = auth.user_id;
        let device_uuid: Option<Uuid> = auth.device_id.parse().ok();
        
        // Update email if provided
        if let Some(email) = &update.email {
            // Check if email is unique
            if email != &user.email && !self.is_email_unique(email, Some(id)).await? {
                return Err(DomainError::Validation(
                    crate::errors::ValidationError::unique("email")
                ));
            }
            
            query(
                "UPDATE users SET email = ?, email_updated_at = ?, email_updated_by = ? WHERE id = ?"
            )
            .bind(email)
            .bind(&now)
            .bind(update.updated_by_user_id.to_string())
            .bind(id.to_string())
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Database(DbError::from(e)))?;
            
            // Log Change
            let entry = ChangeLogEntry {
                operation_id: Uuid::new_v4(),
                entity_table: self.entity_name().to_string(),
                entity_id: id,
                operation_type: ChangeOperationType::Update,
                field_name: Some("email".to_string()),
                old_value: Some(serde_json::to_string(&user.email).unwrap_or_default()),
                new_value: Some(serde_json::to_string(email).unwrap_or_default()),
                timestamp: now_dt,
                user_id: user_uuid,
                device_id: device_uuid.clone(),
                document_metadata: None,
                sync_batch_id: None,
                processed_at: None,
                sync_error: None,
            };
            self.log_change_entry(entry, &mut tx).await?;
        }
        
        // Update password if provided
        if let Some(password) = &update.password {
            query(
                "UPDATE users SET password_hash = ? WHERE id = ?"
            )
            .bind(password)
            .bind(id.to_string())
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Database(DbError::from(e)))?;
            
            // No changelog for password updates for security reasons
        }
        
        // Update name if provided
        if let Some(name) = &update.name {
            query(
                "UPDATE users SET name = ?, name_updated_at = ?, name_updated_by = ? WHERE id = ?"
            )
            .bind(name)
            .bind(&now)
            .bind(update.updated_by_user_id.to_string())
            .bind(id.to_string())
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Database(DbError::from(e)))?;
            
            // Log Change
            let entry = ChangeLogEntry {
                operation_id: Uuid::new_v4(),
                entity_table: self.entity_name().to_string(),
                entity_id: id,
                operation_type: ChangeOperationType::Update,
                field_name: Some("name".to_string()),
                old_value: Some(serde_json::to_string(&user.name).unwrap_or_default()),
                new_value: Some(serde_json::to_string(name).unwrap_or_default()),
                timestamp: now_dt,
                user_id: user_uuid,
                device_id: device_uuid.clone(),
                document_metadata: None,
                sync_batch_id: None,
                processed_at: None,
                sync_error: None,
            };
            self.log_change_entry(entry, &mut tx).await?;
        }
        
        // Update role if provided
        if let Some(role) = &update.role {
            query(
                "UPDATE users SET role = ?, role_updated_at = ?, role_updated_by = ? WHERE id = ?"
            )
            .bind(role)
            .bind(&now)
            .bind(update.updated_by_user_id.to_string())
            .bind(id.to_string())
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Database(DbError::from(e)))?;
            
            // Log Change
            let entry = ChangeLogEntry {
                operation_id: Uuid::new_v4(),
                entity_table: self.entity_name().to_string(),
                entity_id: id,
                operation_type: ChangeOperationType::Update,
                field_name: Some("role".to_string()),
                old_value: Some(serde_json::to_string(user.role.as_str()).unwrap_or_default()),
                new_value: Some(serde_json::to_string(role).unwrap_or_default()),
                timestamp: now_dt,
                user_id: user_uuid,
                device_id: device_uuid.clone(),
                document_metadata: None,
                sync_batch_id: None,
                processed_at: None,
                sync_error: None,
            };
            self.log_change_entry(entry, &mut tx).await?;
        }
        
        // Update active if provided
        if let Some(active) = update.active {
            let active_value = if active { 1 } else { 0 };
            let current_active = if user.active { 1 } else { 0 };
            
            query(
                "UPDATE users SET active = ?, active_updated_at = ?, active_updated_by = ? WHERE id = ?"
            )
            .bind(active_value)
            .bind(&now)
            .bind(update.updated_by_user_id.to_string())
            .bind(id.to_string())
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Database(DbError::from(e)))?;
            
            // Log Change
            let entry = ChangeLogEntry {
                operation_id: Uuid::new_v4(),
                entity_table: self.entity_name().to_string(),
                entity_id: id,
                operation_type: ChangeOperationType::Update,
                field_name: Some("active".to_string()),
                old_value: Some(serde_json::to_string(&user.active).unwrap_or_default()), // Log bool directly
                new_value: Some(serde_json::to_string(&active).unwrap_or_default()), // Log bool directly
                timestamp: now_dt,
                user_id: user_uuid,
                device_id: device_uuid.clone(),
                document_metadata: None,
                sync_batch_id: None,
                processed_at: None,
                sync_error: None,
            };
            self.log_change_entry(entry, &mut tx).await?;
        }
        
        // Update the updated_at and updated_by fields
        query(
            "UPDATE users SET updated_at = ?, updated_by_user_id = ? WHERE id = ?"
        )
        .bind(&now)
        .bind(update.updated_by_user_id.to_string())
        .bind(id.to_string())
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Database(DbError::from(e)))?;
        
        // Commit the transaction
        tx.commit().await
            .map_err(|e| DomainError::Database(DbError::from(e)))?;
        
        // Return the updated user
        self.find_by_id(id).await
    }
    
    async fn update_last_login(&self, id: Uuid) -> DomainResult<()> {
        let now = Utc::now().to_rfc3339();
        
        query("UPDATE users SET last_login = ? WHERE id = ?")
            .bind(&now)
            .bind(id.to_string())
            .execute(&self.pool)
            .await
            .map_err(|e| DomainError::Database(DbError::from(e)))?;
            
        Ok(())
    }
    
    async fn is_email_unique(&self, email: &str, exclude_id: Option<Uuid>) -> DomainResult<bool> {
        let count: i64 = match exclude_id {
            Some(id) => {
                query_scalar(
                    "SELECT COUNT(*) FROM users WHERE email = ? AND id != ? AND deleted_at IS NULL"
                )
                .bind(email)
                .bind(id.to_string())
                .fetch_one(&self.pool)
                .await
                .map_err(|e| DomainError::Database(DbError::from(e)))?
            },
            None => {
                query_scalar(
                    "SELECT COUNT(*) FROM users WHERE email = ? AND deleted_at IS NULL"
                )
                .bind(email)
                .fetch_one(&self.pool)
                .await
                .map_err(|e| DomainError::Database(DbError::from(e)))?
            }
        };
        
        Ok(count == 0)
    }
}