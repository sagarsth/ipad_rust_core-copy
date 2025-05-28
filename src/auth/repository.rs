use crate::errors::{DbError, DbResult};
use crate::domains::user::types::{User, UserRow};
use sqlx::{SqlitePool, query_as};
use uuid::Uuid;
use chrono::Utc;
use async_trait::async_trait;
use crate::types::UserRole;
use chrono::DateTime;

#[async_trait]
pub(crate) trait AuthRepository: Send + Sync {
    async fn find_user_by_email(&self, email: &str) -> DbResult<User>;
    async fn update_last_login(&self, user_id: Uuid) -> DbResult<()>;
    async fn log_login_attempt(&self, email: &str, success: bool, user_id: Option<Uuid>, device_id: &str) -> DbResult<()>;
    async fn log_logout(&self, user_id: Uuid, device_id: &str) -> DbResult<()>;
    async fn add_revoked_token(&self, jti: &str, expiry: i64) -> DbResult<()>;
    async fn is_token_revoked(&self, jti: &str) -> DbResult<bool>;
    async fn delete_expired_revoked_tokens(&self) -> DbResult<u64>;
}

pub(crate) struct SqliteAuthRepository {
    pool: SqlitePool,
}

impl SqliteAuthRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AuthRepository for SqliteAuthRepository {
    async fn find_user_by_email(&self, email: &str) -> DbResult<User> {
        let row = query_as::<_, UserRow>(
            "SELECT * FROM users WHERE email = ? AND deleted_at IS NULL")
            .bind(email)
            .fetch_optional(&self.pool)
            .await
            .map_err(DbError::from)?
            .ok_or_else(|| DbError::NotFound("User".to_string(), email.to_string()))?;
        
        row.into_entity().map_err(|e| match e {
            crate::errors::DomainError::Database(db_err) => db_err,
            _ => DbError::Other(e.to_string()),
        })
    }
    
    async fn update_last_login(&self, user_id: Uuid) -> DbResult<()> {
        let now = Utc::now().to_rfc3339();
        
        // Use sqlx::query for UPDATE
        sqlx::query(
            "UPDATE users SET last_login = ? WHERE id = ?")
            .bind(now)
            .bind(user_id.to_string())
            .execute(&self.pool)
            .await
            .map_err(DbError::from)?;
            
        Ok(())
    }
    
    async fn log_login_attempt(&self, email: &str, success: bool, user_id: Option<Uuid>, device_id: &str) -> DbResult<()> {
        let log_id = Uuid::new_v4();
        let now = Utc::now().to_rfc3339();
        let action = if success { "login_success" } else { "login_fail" };
        
        if success {
            // For successful logins, we have a valid user_id
            if let Some(user_id) = user_id {
                sqlx::query(
                    "INSERT INTO audit_logs (id, user_id, action, entity_table, entity_id, details, timestamp, device_id) 
                     VALUES (?, ?, ?, 'users', ?, ?, ?, ?)")
                    .bind(log_id.to_string())
                    .bind(user_id.to_string())
                    .bind(action)
                    .bind(user_id.to_string())
                    .bind(format!("{{\"email\":\"{}\"}}", email))
                    .bind(now)
                    .bind(device_id)
                    .execute(&self.pool)
                    .await
                    .map_err(DbError::from)?;
            }
        } else {
            // For failed logins, we don't have a valid user_id, so we skip audit logging
            // or we could use a system user ID if one exists
            println!("⚠️ [AUTH] Skipping audit log for failed login attempt for email: {}", email);
        }
            
        Ok(())
    }
    
    async fn log_logout(&self, user_id: Uuid, device_id: &str) -> DbResult<()> {
        let log_id = Uuid::new_v4();
        let now = Utc::now().to_rfc3339();
        
        // Use sqlx::query for INSERT
        sqlx::query(
            "INSERT INTO audit_logs (id, user_id, action, entity_table, entity_id, details, timestamp, device_id) 
             VALUES (?, ?, 'logout', 'users', ?, ?, ?, ?)")
            .bind(log_id.to_string())
            .bind(user_id.to_string())
            .bind(user_id.to_string())
            .bind(Option::<String>::None)
            .bind(now)
            .bind(device_id)
            .execute(&self.pool)
            .await
            .map_err(DbError::from)?;
            
        Ok(())
    }

    async fn add_revoked_token(&self, jti: &str, expiry: i64) -> DbResult<()> {
        sqlx::query("INSERT OR IGNORE INTO revoked_tokens (jti, expiry) VALUES (?, ?)")
            .bind(jti)
            .bind(expiry)
            .execute(&self.pool)
            .await
            .map_err(DbError::from)?; // Ignore potential unique constraint violation if already revoked
        Ok(())
    }

    async fn is_token_revoked(&self, jti: &str) -> DbResult<bool> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM revoked_tokens WHERE jti = ?")
            .bind(jti)
            .fetch_one(&self.pool)
            .await
            .map_err(DbError::from)?;
        Ok(count > 0)
    }

    async fn delete_expired_revoked_tokens(&self) -> DbResult<u64> {
        let now = Utc::now().timestamp();
        let result = sqlx::query("DELETE FROM revoked_tokens WHERE expiry < ?")
            .bind(now)
            .execute(&self.pool)
            .await
            .map_err(DbError::from)?;
        Ok(result.rows_affected())
    }
}