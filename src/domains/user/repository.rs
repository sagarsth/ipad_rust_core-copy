use crate::errors::{DbError, DbResult, DomainError, DomainResult};
use crate::domains::user::types::{User, NewUser, UpdateUser, UserRow};
use crate::domains::core::repository::Repository;
use crate::auth::AuthContext;
use crate::types::ChangeLogOperationType;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use sqlx::{SqlitePool, query, query_as, query_scalar};
use async_trait::async_trait;

/// User repository trait
#[async_trait]
pub trait UserRepository: Send + Sync {
    /// Find a user by ID
    async fn find_by_id(&self, id: Uuid) -> DomainResult<User>;
    
    /// Find a user by email
    async fn find_by_email(&self, email: &str) -> DomainResult<User>;
    
    /// Find all users
    async fn find_all(&self) -> DomainResult<Vec<User>>;
    
    /// Create a new user
    async fn create(&self, user: NewUser, auth: &AuthContext) -> DomainResult<User>;
    
    /// Update an existing user
    async fn update(&self, id: Uuid, update: UpdateUser, auth: &AuthContext) -> DomainResult<User>;
    
    /// Hard delete a user
    async fn hard_delete(&self, id: Uuid, auth: &AuthContext) -> DomainResult<()>;
    
    /// Update last login timestamp
    async fn update_last_login(&self, id: Uuid) -> DomainResult<()>;
    
    /// Check if email is unique
    async fn is_email_unique(&self, email: &str, exclude_id: Option<Uuid>) -> DomainResult<bool>;
}

/// SQLite implementation of UserRepository
pub struct SqliteUserRepository {
    pool: SqlitePool,
}

impl SqliteUserRepository {
    /// Create a new repository instance
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
    
    /// Create a changelog entry
    async fn create_changelog(
        &self,
        operation_type: &str,
        entity_id: Uuid,
        field_name: Option<&str>,
        old_value: Option<&str>,
        new_value: Option<&str>,
        user_id: &Uuid,
        device_id: &str,
    ) -> DomainResult<()> {
        let operation_id = Uuid::new_v4();
        let now = Utc::now().to_rfc3339();
        
        query(
            "INSERT INTO change_log (
                operation_id, entity_table, entity_id, operation_type,
                field_name, old_value, new_value, timestamp, user_id, device_id
            ) VALUES (?, 'users', ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(operation_id.to_string())
        .bind(entity_id.to_string())
        .bind(operation_type)
        .bind(field_name)
        .bind(old_value)
        .bind(new_value)
        .bind(now)
        .bind(user_id.to_string())
        .bind(device_id)
        .execute(&self.pool)
        .await
        .map_err(|e| DomainError::Database(DbError::from(e)))?;
        
        Ok(())
    }
}

#[async_trait]
impl UserRepository for SqliteUserRepository {
    async fn find_by_id(&self, id: Uuid) -> DomainResult<User> {
        let row = query_as::<_, UserRow>(
            "SELECT * FROM users WHERE id = ? AND deleted_at IS NULL"
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DomainError::Database(DbError::from(e)))?
        .ok_or_else(|| DomainError::EntityNotFound("User".to_string(), id))?;
        
        row.into_entity()
    }
    
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
        
        // Generate ID
        let id = Uuid::new_v4();
        let now = Utc::now().to_rfc3339();
        
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
        .execute(&self.pool)
        .await
        .map_err(|e| DomainError::Database(DbError::from(e)))?;
        
        // Create changelog entry
        self.create_changelog(
            ChangeLogOperationType::Create.as_str(),
            id,
            None,
            None,
            Some(&format!("User created: {}", &user.email)),
            &auth.user_id,
            &auth.device_id,
        ).await?;
        
        // Return the created user
        self.find_by_id(id).await
    }
    
    async fn update(&self, id: Uuid, update: UpdateUser, auth: &AuthContext) -> DomainResult<User> {
        // Check if user exists
        let user = self.find_by_id(id).await?;
        
        // Begin transaction
        let mut tx = self.pool.begin().await
            .map_err(|e| DomainError::Database(DbError::from(e)))?;
            
        let now = Utc::now().to_rfc3339();
        
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
            
            // Create changelog entry
            self.create_changelog(
                ChangeLogOperationType::Update.as_str(),
                id,
                Some("email"),
                Some(&user.email),
                Some(email),
                &auth.user_id,
                &auth.device_id,
            ).await?;
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
            
            // Create changelog entry
            self.create_changelog(
                ChangeLogOperationType::Update.as_str(),
                id,
                Some("name"),
                Some(&user.name),
                Some(name),
                &auth.user_id,
                &auth.device_id,
            ).await?;
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
            
            // Create changelog entry
            self.create_changelog(
                ChangeLogOperationType::Update.as_str(),
                id,
                Some("role"),
                Some(user.role.as_str()),
                Some(role),
                &auth.user_id,
                &auth.device_id,
            ).await?;
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
            
            // Create changelog entry
            self.create_changelog(
                ChangeLogOperationType::Update.as_str(),
                id,
                Some("active"),
                Some(&current_active.to_string()),
                Some(&active_value.to_string()),
                &auth.user_id,
                &auth.device_id,
            ).await?;
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
    
    async fn hard_delete(&self, id: Uuid, auth: &AuthContext) -> DomainResult<()> {
        // Check if user exists (even if deleted) to prevent errors on double delete
        let exists: bool = query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE id = ?)")
            .bind(id.to_string())
            .fetch_one(&self.pool)
            .await
            .map_err(|e| DomainError::Database(DbError::from(e)))?;

        if !exists {
             return Ok(()); // Already deleted or never existed, consider it success
        }

        // Hard delete the user
        query("DELETE FROM users WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool) // Execute directly on the pool
            .await
            .map_err(|e| DomainError::Database(DbError::from(e)))?;
            
        // Log the hard delete action (optional, using standard logging)
        log::info!("Hard deleted user {} by user {}", id, auth.user_id);

        Ok(())
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