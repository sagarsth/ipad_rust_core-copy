use crate::errors::{DbError, DbResult};
use crate::domains::user::types::{User, UserRow};
use sqlx::{SqlitePool, query_as};
use uuid::Uuid;
use chrono::Utc;
use async_trait::async_trait;

#[async_trait]
pub(crate) trait AuthRepository: Send + Sync {
    async fn find_user_by_email(&self, email: &str) -> DbResult<User>;
    async fn update_last_login(&self, user_id: Uuid) -> DbResult<()>;
    async fn log_login_attempt(&self, email: &str, success: bool, user_id: Option<Uuid>, device_id: &str) -> DbResult<()>;
    async fn log_logout(&self, user_id: Uuid, device_id: &str) -> DbResult<()>;
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
        let user_id_str = user_id.map(|id| id.to_string());
        
        // Use sqlx::query for INSERT
        sqlx::query(
            "INSERT INTO audit_logs (id, user_id, action, entity_table, entity_id, details, timestamp) 
             VALUES (?, ?, ?, 'users', ?, ?, ?)")
            .bind(log_id.to_string())
            .bind(user_id_str.as_deref())
            .bind(action)
            .bind(user_id_str.as_deref())
            .bind(format!("{{\"email\":\"{}\",\"device_id\":\"{}\"}}", email, device_id))
            .bind(now)
            .execute(&self.pool)
            .await
            .map_err(DbError::from)?;
            
        Ok(())
    }
    
    async fn log_logout(&self, user_id: Uuid, device_id: &str) -> DbResult<()> {
        let log_id = Uuid::new_v4();
        let now = Utc::now().to_rfc3339();
        
        // Use sqlx::query for INSERT
        sqlx::query(
            "INSERT INTO audit_logs (id, user_id, action, entity_table, entity_id, details, timestamp) 
             VALUES (?, ?, 'logout', 'users', ?, ?, ?)")
            .bind(log_id.to_string())
            .bind(user_id.to_string())
            .bind(user_id.to_string())
            .bind(format!("{{\"device_id\":\"{}\"}}", device_id))
            .bind(now)
            .execute(&self.pool)
            .await
            .map_err(DbError::from)?;
            
        Ok(())
    }
}