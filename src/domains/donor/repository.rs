use crate::auth::AuthContext;
use crate::domains::core::repository::{FindById, HardDeletable, SoftDeletable};
use crate::domains::core::delete_service::DeleteServiceRepository;
use crate::domains::core::document_linking::DocumentLinkable;
use crate::domains::donor::types::{Donor, NewDonor, UpdateDonor, DonorRow, UserDonorRole, DonorStatsSummary};
use crate::errors::{DbError, DomainError, DomainResult, ValidationError};
use crate::types::{PaginatedResult, PaginationParams};
use async_trait::async_trait;
use chrono::{Utc, DateTime, Local};
use sqlx::{Pool, Sqlite, Transaction, query, query_as, query_scalar, QueryBuilder};
use std::collections::HashMap;
use uuid::Uuid;
use crate::domains::sync::types::{ChangeLogEntry, ChangeOperationType, MergeOutcome};
use crate::domains::user::repository::MergeableEntityRepository;
use serde::{Deserialize, Serialize};

/// Placeholder - Define this properly in donor/types.rs based on your schema
#[derive(Serialize, Deserialize, Debug, Clone)]
struct DonorFullState {
    id: Uuid,
    name: String,
    name_updated_at: Option<DateTime<Utc>>,
    name_updated_by: Option<Uuid>,
    name_updated_by_device_id: Option<Uuid>,
    type_: Option<String>,
    type_updated_at: Option<DateTime<Utc>>,
    type_updated_by: Option<Uuid>,
    type_updated_by_device_id: Option<Uuid>,
    contact_person: Option<String>,
    contact_person_updated_at: Option<DateTime<Utc>>,
    contact_person_updated_by: Option<Uuid>,
    contact_person_updated_by_device_id: Option<Uuid>,
    email: Option<String>,
    email_updated_at: Option<DateTime<Utc>>,
    email_updated_by: Option<Uuid>,
    email_updated_by_device_id: Option<Uuid>,
    phone: Option<String>,
    phone_updated_at: Option<DateTime<Utc>>,
    phone_updated_by: Option<Uuid>,
    phone_updated_by_device_id: Option<Uuid>,
    country: Option<String>,
    country_updated_at: Option<DateTime<Utc>>,
    country_updated_by: Option<Uuid>,
    country_updated_by_device_id: Option<Uuid>,
    first_donation_date: Option<String>,
    first_donation_date_updated_at: Option<DateTime<Utc>>,
    first_donation_date_updated_by: Option<Uuid>,
    first_donation_date_updated_by_device_id: Option<Uuid>,
    notes: Option<String>,
    notes_updated_at: Option<DateTime<Utc>>,
    notes_updated_by: Option<Uuid>,
    notes_updated_by_device_id: Option<Uuid>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    created_by_user_id: Option<Uuid>,
    created_by_device_id: Option<Uuid>,
    updated_by_user_id: Option<Uuid>,
    updated_by_device_id: Option<Uuid>,
    deleted_at: Option<DateTime<Utc>>,
    deleted_by_user_id: Option<Uuid>,
    deleted_by_device_id: Option<Uuid>,
}

/// Trait defining donor repository operations
#[async_trait]
pub trait DonorRepository: 
    DeleteServiceRepository<Donor> + FindById<Donor> + SoftDeletable + HardDeletable + MergeableEntityRepository<Donor> + Send + Sync
{
    // Basic CRUD methods (assuming similar LWW patterns)
    async fn create(&self, new_donor: &NewDonor, auth: &AuthContext) -> DomainResult<Donor>;
    async fn create_with_tx<'t>(
        &self,
        new_donor: &NewDonor,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<Donor>;

    async fn update(&self, id: Uuid, update_data: &UpdateDonor, auth: &AuthContext) -> DomainResult<Donor>;
    async fn update_with_tx<'t>(
        &self,
        id: Uuid,
        update_data: &UpdateDonor,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<Donor>;

    async fn find_all(&self, params: PaginationParams) -> DomainResult<PaginatedResult<Donor>>;

    /// Find donors by specific IDs
    async fn find_by_ids(
        &self,
        ids: &[Uuid],
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Donor>>;

    // Add new method signatures here
    /// Count donors by type
    async fn count_by_type(&self) -> DomainResult<Vec<(Option<String>, i64)>>;

    /// Count donors by country
    async fn count_by_country(&self) -> DomainResult<Vec<(Option<String>, i64)>>;

    /// Get aggregate statistics for donors
    async fn get_donation_stats(&self) -> DomainResult<DonorStatsSummary>;

    /// Find donors by type
    async fn find_by_type(
        &self,
        donor_type: &str,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Donor>>;

    /// Find donors by country
    async fn find_by_country(
        &self,
        country: &str,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Donor>>;

    /// Find donors with recent donations since a specific date
    async fn find_with_recent_donations(
        &self,
        since_date: &str,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Donor>>;

    /// Find donors created or updated by a specific user
    async fn find_ids_by_user_role(
        &self,
        user_id: Uuid,
        role: UserDonorRole,
    ) -> DomainResult<Vec<Uuid>>;

    /// Find donors within a date range (created_at or updated_at)
    async fn find_by_date_range(
        &self,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Donor>>;
}

/// SQLite implementation for DonorRepository
#[derive(Debug, Clone)]
pub struct SqliteDonorRepository {
    pool: Pool<Sqlite>,
}

impl SqliteDonorRepository {
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }

    fn map_row_to_entity(row: DonorRow) -> DomainResult<Donor> {
        row.into_entity()
            .map_err(|e| DomainError::Internal(format!("Failed to map donor row to entity: {}", e)))
    }

    fn entity_name(&self) -> &'static str {
        "donors"
    }

    async fn find_by_id_with_tx<'t>(
        &self,
        id: Uuid,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<Option<Donor>> {
        let row_opt = query_as::<_, DonorRow>(
            "SELECT id, name, name_updated_at, name_updated_by, name_updated_by_device_id, type_ AS type, type_updated_at, type_updated_by, type_updated_by_device_id, contact_person, contact_person_updated_at, contact_person_updated_by, contact_person_updated_by_device_id, email, email_updated_at, email_updated_by, email_updated_by_device_id, phone, phone_updated_at, phone_updated_by, phone_updated_by_device_id, country, country_updated_at, country_updated_by, country_updated_by_device_id, first_donation_date, first_donation_date_updated_at, first_donation_date_updated_by, first_donation_date_updated_by_device_id, notes, notes_updated_at, notes_updated_by, notes_updated_by_device_id, created_at, updated_at, created_by_user_id, created_by_device_id, updated_by_user_id, updated_by_device_id, deleted_at, deleted_by_user_id, deleted_by_device_id FROM donors WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(id.to_string())
        .fetch_optional(&mut **tx)
        .await
        .map_err(DbError::from)?;
        
        match row_opt {
            Some(row) => Ok(Some(Self::map_row_to_entity(row)?)),
            None => Ok(None),
        }
    }

    /// Upsert remote Donor state within a transaction
    async fn upsert_remote_state_with_tx<'t>(
        &self,
        tx: &mut Transaction<'t, Sqlite>,
        remote: &Donor,
        remote_device_id_str: Option<String>
    ) -> DomainResult<()> {
        sqlx::query(
            r#"INSERT OR REPLACE INTO donors (
                id, name, name_updated_at, name_updated_by, name_updated_by_device_id,
                type_, type_updated_at, type_updated_by, type_updated_by_device_id,
                contact_person, contact_person_updated_at, contact_person_updated_by, contact_person_updated_by_device_id,
                email, email_updated_at, email_updated_by, email_updated_by_device_id,
                phone, phone_updated_at, phone_updated_by, phone_updated_by_device_id,
                country, country_updated_at, country_updated_by, country_updated_by_device_id,
                first_donation_date, first_donation_date_updated_at, first_donation_date_updated_by, first_donation_date_updated_by_device_id,
                notes, notes_updated_at, notes_updated_by, notes_updated_by_device_id,
                created_at, updated_at, created_by_user_id, created_by_device_id, updated_by_user_id, updated_by_device_id,
                deleted_at, deleted_by_user_id, deleted_by_device_id
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ? )"#
        )
        .bind(remote.id.to_string())
        .bind(remote.name.clone())
        .bind(remote.name_updated_at.map(|dt| dt.to_rfc3339()))
        .bind(remote.name_updated_by.map(|id| id.to_string()))
        .bind(remote.name_updated_by_device_id.map(|id| id.to_string()).or(remote_device_id_str.clone()))
        .bind(remote.type_.clone())
        .bind(remote.type_updated_at.map(|dt| dt.to_rfc3339()))
        .bind(remote.type_updated_by.map(|id| id.to_string()))
        .bind(remote.type_updated_by_device_id.map(|id| id.to_string()).or(remote_device_id_str.clone()))
        .bind(remote.contact_person.clone())
        .bind(remote.contact_person_updated_at.map(|dt| dt.to_rfc3339()))
        .bind(remote.contact_person_updated_by.map(|id| id.to_string()))
        .bind(remote.contact_person_updated_by_device_id.map(|id| id.to_string()).or(remote_device_id_str.clone()))
        .bind(remote.email.clone())
        .bind(remote.email_updated_at.map(|dt| dt.to_rfc3339()))
        .bind(remote.email_updated_by.map(|id| id.to_string()))
        .bind(remote.email_updated_by_device_id.map(|id| id.to_string()).or(remote_device_id_str.clone()))
        .bind(remote.phone.clone())
        .bind(remote.phone_updated_at.map(|dt| dt.to_rfc3339()))
        .bind(remote.phone_updated_by.map(|id| id.to_string()))
        .bind(remote.phone_updated_by_device_id.map(|id| id.to_string()).or(remote_device_id_str.clone()))
        .bind(remote.country.clone())
        .bind(remote.country_updated_at.map(|dt| dt.to_rfc3339()))
        .bind(remote.country_updated_by.map(|id| id.to_string()))
        .bind(remote.country_updated_by_device_id.map(|id| id.to_string()).or(remote_device_id_str.clone()))
        .bind(remote.first_donation_date.clone())
        .bind(remote.first_donation_date_updated_at.map(|dt| dt.to_rfc3339()))
        .bind(remote.first_donation_date_updated_by.map(|id| id.to_string()))
        .bind(remote.first_donation_date_updated_by_device_id.map(|id| id.to_string()).or(remote_device_id_str.clone()))
        .bind(remote.notes.clone())
        .bind(remote.notes_updated_at.map(|dt| dt.to_rfc3339()))
        .bind(remote.notes_updated_by.map(|id| id.to_string()))
        .bind(remote.notes_updated_by_device_id.map(|id| id.to_string()).or(remote_device_id_str.clone()))
        .bind(remote.created_at.to_rfc3339())
        .bind(remote.updated_at.to_rfc3339())
        .bind(remote.created_by_user_id.map(|id| id.to_string()))
        .bind(remote.created_by_device_id.map(|id| id.to_string()).or(remote_device_id_str.clone()))
        .bind(remote.updated_by_user_id.map(|id| id.to_string()))
        .bind(remote.updated_by_device_id.map(|id| id.to_string()).or(remote_device_id_str.clone()))
        .bind(remote.deleted_at.map(|dt| dt.to_rfc3339()))
        .bind(remote.deleted_by_user_id.map(|id| id.to_string()))
        .bind(remote.deleted_by_device_id.map(|id| id.to_string()).or(remote_device_id_str.clone()))
        .execute(&mut **tx)
        .await
        .map_err(DbError::from)?;
        Ok(())
    }
}

#[async_trait]
impl FindById<Donor> for SqliteDonorRepository {
    async fn find_by_id(&self, id: Uuid) -> DomainResult<Donor> {
        let row = query_as::<_, DonorRow>(
            "SELECT id, name, name_updated_at, name_updated_by, name_updated_by_device_id, type_ AS type, type_updated_at, type_updated_by, type_updated_by_device_id, contact_person, contact_person_updated_at, contact_person_updated_by, contact_person_updated_by_device_id, email, email_updated_at, email_updated_by, email_updated_by_device_id, phone, phone_updated_at, phone_updated_by, phone_updated_by_device_id, country, country_updated_at, country_updated_by, country_updated_by_device_id, first_donation_date, first_donation_date_updated_at, first_donation_date_updated_by, first_donation_date_updated_by_device_id, notes, notes_updated_at, notes_updated_by, notes_updated_by_device_id, created_at, updated_at, created_by_user_id, created_by_device_id, updated_by_user_id, updated_by_device_id, deleted_at, deleted_by_user_id, deleted_by_device_id FROM donors WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await
        .map_err(DbError::from)?
        .ok_or_else(|| DomainError::EntityNotFound("Donor".to_string(), id))?;

        Self::map_row_to_entity(row)
    }
}

#[async_trait]
impl SoftDeletable for SqliteDonorRepository {
    async fn soft_delete_with_tx(
        &self,
        id: Uuid,
        auth: &AuthContext,
        tx: &mut Transaction<'_, Sqlite>,
    ) -> DomainResult<()> {
        let now = Utc::now().to_rfc3339();
        let user_id_str = auth.user_id.to_string();
        let device_id_str = auth.device_id.parse::<Uuid>().ok().map(|u| u.to_string());

        let result = query(
            "UPDATE donors SET deleted_at = ?, deleted_by_user_id = ?, deleted_by_device_id = ?, updated_at = ?, updated_by_user_id = ?, updated_by_device_id = ? WHERE id = ? AND deleted_at IS NULL"
        )
        .bind(&now)
        .bind(&user_id_str)
        .bind(&device_id_str)
        .bind(&now) // updated_at
        .bind(&user_id_str) // updated_by_user_id
        .bind(&device_id_str) // updated_by_device_id
        .bind(id.to_string())
        .execute(&mut **tx)
        .await
        .map_err(DbError::from)?;

        if result.rows_affected() == 0 {
            Err(DomainError::EntityNotFound("Donor".to_string(), id))
        } else {
            Ok(())
        }
    }

    async fn soft_delete(&self, id: Uuid, auth: &AuthContext) -> DomainResult<()> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        match self.soft_delete_with_tx(id, auth, &mut tx).await {
            Ok(()) => { tx.commit().await.map_err(DbError::from)?; Ok(()) },
            Err(e) => { let _ = tx.rollback().await; Err(e) }
        }
    }
}

#[async_trait]
impl HardDeletable for SqliteDonorRepository {
    fn entity_name(&self) -> &'static str {
        "donors"
    }

    async fn hard_delete_with_tx(
        &self,
        id: Uuid,
        _auth: &AuthContext,
        tx: &mut Transaction<'_, Sqlite>,
    ) -> DomainResult<()> {
        let result = query("DELETE FROM donors WHERE id = ?")
            .bind(id.to_string())
            .execute(&mut **tx)
            .await
            .map_err(DbError::from)?;

        if result.rows_affected() == 0 {
            Err(DomainError::EntityNotFound("Donor".to_string(), id))
        } else {
            Ok(())
        }
    }

    async fn hard_delete(&self, id: Uuid, auth: &AuthContext) -> DomainResult<()> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        match self.hard_delete_with_tx(id, auth, &mut tx).await {
            Ok(()) => { tx.commit().await.map_err(DbError::from)?; Ok(()) },
            Err(e) => { let _ = tx.rollback().await; Err(e) }
        }
    }
}

#[async_trait]
impl MergeableEntityRepository<Donor> for SqliteDonorRepository {
    fn entity_name(&self) -> &'static str { "donors" }

    async fn merge_remote_change<'t>(
        &self,
        tx: &mut Transaction<'t, Sqlite>,
        remote_change: &ChangeLogEntry,
    ) -> DomainResult<MergeOutcome> {
        log::debug!(
            "Merging remote donor change: id={}, table={}, op={:?}",
            remote_change.entity_id, remote_change.entity_table, remote_change.operation_type
        );

        if remote_change.entity_table != self.entity_name() {
            return Err(DomainError::Internal(format!(
                "DonorRepository received change for incorrect table: {}",
                remote_change.entity_table
            )));
        }

        let remote_device_id_str = remote_change.device_id.map(|id| id.to_string());

        match remote_change.operation_type {
            ChangeOperationType::Create | ChangeOperationType::Update => {
                let state_json = remote_change.new_value.as_ref()
                    .ok_or_else(|| DomainError::Validation(ValidationError::custom("Missing new_value for donor change")))?;
                let remote_state: Donor = serde_json::from_str(state_json)
                    .map_err(|e| DomainError::Validation(ValidationError::format("new_value_donor_change", &format!("Invalid JSON: {}", e))))?;
                // Fetch local record (None if soft-deleted or not exists)
                let local_opt = self.find_by_id_with_tx(remote_state.id, tx).await?;
                if let Some(local) = local_opt {
                    // Only proceed if remote is newer
                    if remote_state.updated_at <= local.updated_at {
                        return Ok(MergeOutcome::NoOp("Local donor copy newer or equal".into()));
                    }
                    self.upsert_remote_state_with_tx(tx, &remote_state, remote_device_id_str.clone()).await?;
                    Ok(MergeOutcome::Updated(remote_state.id))
                } else {
                    self.upsert_remote_state_with_tx(tx, &remote_state, remote_device_id_str.clone()).await?;
                    Ok(MergeOutcome::Created(remote_state.id))
                }
            }
            ChangeOperationType::Delete => {
                log::info!("Remote soft DELETE for donor {} - NoOp as soft deletes are local-only.", remote_change.entity_id);
                Ok(MergeOutcome::NoOp("Soft deletes are local-only".to_string()))
            }
            ChangeOperationType::HardDelete => {
                log::info!("Applying HARD DELETE for donor {} directly in merge_remote_change", remote_change.entity_id);
                if self.find_by_id_with_tx(remote_change.entity_id, tx).await?.is_none() {
                    return Ok(MergeOutcome::NoOp(format!("Donor {} already deleted or not found", remote_change.entity_id)));
                }
                let temp_auth = AuthContext::internal_system_context();
                self.hard_delete_with_tx(remote_change.entity_id, &temp_auth, tx).await?;
                Ok(MergeOutcome::HardDeleted(remote_change.entity_id))
            }
        }
    }
}

#[async_trait]
impl DonorRepository for SqliteDonorRepository {
    async fn create(
        &self,
        new_donor: &NewDonor,
        auth: &AuthContext,
    ) -> DomainResult<Donor> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        match self.create_with_tx(new_donor, auth, &mut tx).await {
            Ok(donor) => { tx.commit().await.map_err(DbError::from)?; Ok(donor) },
            Err(e) => { let _ = tx.rollback().await; Err(e) }
        }
    }

    async fn create_with_tx<'t>(
        &self,
        new_donor: &NewDonor,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<Donor> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let user_id_str = auth.user_id.to_string();
        let device_id_str = auth.device_id.parse::<Uuid>().ok().map(|u| u.to_string());
        let created_by_user_id_str = new_donor.created_by_user_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| user_id_str.clone());
        let current_device_id_str = device_id_str.clone();

        let sql = r#"INSERT INTO donors (
            id, name, name_updated_at, name_updated_by, name_updated_by_device_id,
            type_, type_updated_at, type_updated_by, type_updated_by_device_id,
            contact_person, contact_person_updated_at, contact_person_updated_by, contact_person_updated_by_device_id,
            email, email_updated_at, email_updated_by, email_updated_by_device_id,
            phone, phone_updated_at, phone_updated_by, phone_updated_by_device_id,
            country, country_updated_at, country_updated_by, country_updated_by_device_id,
            first_donation_date, first_donation_date_updated_at, first_donation_date_updated_by, first_donation_date_updated_by_device_id,
            notes, notes_updated_at, notes_updated_by, notes_updated_by_device_id,
            created_at, updated_at, created_by_user_id, created_by_device_id, updated_by_user_id, updated_by_device_id
        ) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)"#; // 40 fields

        query(sql)
            .bind(id.to_string())
            .bind(new_donor.name.clone())
            .bind(now_str.clone()) // name_updated_at
            .bind(user_id_str.clone()) // name_updated_by
            .bind(current_device_id_str.clone()) // name_updated_by_device_id
            .bind(new_donor.type_.clone())
            .bind(new_donor.type_.as_ref().map(|_| now_str.clone()))
            .bind(new_donor.type_.as_ref().map(|_| user_id_str.clone()))
            .bind(new_donor.type_.as_ref().and_then(|_| current_device_id_str.clone()))
            .bind(new_donor.contact_person.clone())
            .bind(new_donor.contact_person.as_ref().map(|_| now_str.clone()))
            .bind(new_donor.contact_person.as_ref().map(|_| user_id_str.clone()))
            .bind(new_donor.contact_person.as_ref().and_then(|_| current_device_id_str.clone()))
            .bind(new_donor.email.clone())
            .bind(new_donor.email.as_ref().map(|_| now_str.clone()))
            .bind(new_donor.email.as_ref().map(|_| user_id_str.clone()))
            .bind(new_donor.email.as_ref().and_then(|_| current_device_id_str.clone()))
            .bind(new_donor.phone.clone())
            .bind(new_donor.phone.as_ref().map(|_| now_str.clone()))
            .bind(new_donor.phone.as_ref().map(|_| user_id_str.clone()))
            .bind(new_donor.phone.as_ref().and_then(|_| current_device_id_str.clone()))
            .bind(new_donor.country.clone())
            .bind(new_donor.country.as_ref().map(|_| now_str.clone()))
            .bind(new_donor.country.as_ref().map(|_| user_id_str.clone()))
            .bind(new_donor.country.as_ref().and_then(|_| current_device_id_str.clone()))
            .bind(new_donor.first_donation_date.clone()) // Already Option<String>
            .bind(new_donor.first_donation_date.as_ref().map(|_| now_str.clone()))
            .bind(new_donor.first_donation_date.as_ref().map(|_| user_id_str.clone()))
            .bind(new_donor.first_donation_date.as_ref().and_then(|_| current_device_id_str.clone()))
            .bind(new_donor.notes.clone())
            .bind(new_donor.notes.as_ref().map(|_| now_str.clone()))
            .bind(new_donor.notes.as_ref().map(|_| user_id_str.clone()))
            .bind(new_donor.notes.as_ref().and_then(|_| current_device_id_str.clone()))
            .bind(now_str.clone()) // created_at
            .bind(now_str.clone()) // updated_at
            .bind(created_by_user_id_str)
            .bind(device_id_str.clone()) // created_by_device_id
            .bind(user_id_str.clone()) // updated_by_user_id
            .bind(device_id_str.clone()) // updated_by_device_id
            .execute(&mut **tx).await.map_err(DbError::from)?;
        
        self.find_by_id_with_tx(id, tx).await?
            .ok_or_else(|| DomainError::EntityNotFound(self.entity_name().to_string(), id))
    }

    async fn update(
        &self,
        id: Uuid,
        update_data: &UpdateDonor,
        auth: &AuthContext,
    ) -> DomainResult<Donor> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        match self.update_with_tx(id, update_data, auth, &mut tx).await {
            Ok(donor) => { tx.commit().await.map_err(DbError::from)?; Ok(donor) },
            Err(e) => { let _ = tx.rollback().await; Err(e) }
        }
    }

    async fn update_with_tx<'t>(
        &self,
        id: Uuid,
        update_data: &UpdateDonor,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<Donor> {
        let _ = self.find_by_id_with_tx(id, tx).await?;
        
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let user_id_str = auth.user_id.to_string();
        let device_id_str = auth.device_id.parse::<Uuid>().ok().map(|u| u.to_string());
        let id_str = id.to_string();

        macro_rules! add_lww_option {
            ($builder:expr, $separated:expr, $field_sql:literal, $value:expr, $now_ref:expr, $user_id_ref:expr, $device_id_ref:expr, $fields_updated_flag:expr) => {
                if let Some(ref val) = $value {
                    $separated.push(concat!($field_sql, " = "));
                    $separated.push_bind_unseparated(val.clone());
                    $separated.push(concat!(" ", $field_sql, "_updated_at = "));
                    $separated.push_bind_unseparated($now_ref.clone());
                    $separated.push(concat!(" ", $field_sql, "_updated_by = "));
                    $separated.push_bind_unseparated($user_id_ref.clone());
                    $separated.push(concat!(" ", $field_sql, "_updated_by_device_id = "));
                    $separated.push_bind_unseparated($device_id_ref.clone());
                    $fields_updated_flag = true;
                }
            };
        }

        let mut builder = QueryBuilder::new("UPDATE donors SET ");
        let mut separated = builder.separated(", ");
        let mut fields_updated = false;

        add_lww_option!(builder, separated, "name", update_data.name, &now_str, &user_id_str, &device_id_str, fields_updated);
        add_lww_option!(builder, separated, "type_", update_data.type_, &now_str, &user_id_str, &device_id_str, fields_updated);
        add_lww_option!(builder, separated, "contact_person", update_data.contact_person, &now_str, &user_id_str, &device_id_str, fields_updated);
        add_lww_option!(builder, separated, "email", update_data.email, &now_str, &user_id_str, &device_id_str, fields_updated);
        add_lww_option!(builder, separated, "phone", update_data.phone, &now_str, &user_id_str, &device_id_str, fields_updated);
        add_lww_option!(builder, separated, "country", update_data.country, &now_str, &user_id_str, &device_id_str, fields_updated);
        add_lww_option!(builder, separated, "first_donation_date", update_data.first_donation_date, &now_str, &user_id_str, &device_id_str, fields_updated);
        add_lww_option!(builder, separated, "notes", update_data.notes, &now_str, &user_id_str, &device_id_str, fields_updated);

        if !fields_updated {
            return self.find_by_id_with_tx(id, tx).await?
                .ok_or_else(|| DomainError::EntityNotFound(self.entity_name().to_string(), id));
        }

        separated.push("updated_at = ");
        separated.push_bind_unseparated(now_str);
        separated.push("updated_by_user_id = ");
        separated.push_bind_unseparated(user_id_str);
        separated.push("updated_by_device_id = ");
        separated.push_bind_unseparated(device_id_str);


        builder.push(" WHERE id = ");
        builder.push_bind(id_str);

        let query = builder.build();
        let result = query.execute(&mut **tx).await.map_err(DbError::from)?;

        if result.rows_affected() == 0 {
            return Err(DomainError::EntityNotFound(self.entity_name().to_string(), id));
        }

        self.find_by_id_with_tx(id, tx).await?
            .ok_or_else(|| DomainError::EntityNotFound(self.entity_name().to_string(), id))
    }

    async fn find_all(
        &self,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Donor>> {
        let offset = (params.page - 1) * params.per_page;

        let total: i64 = query_scalar("SELECT COUNT(*) FROM donors WHERE deleted_at IS NULL")
            .fetch_one(&self.pool)
            .await
            .map_err(DbError::from)?;

        let rows = query_as::<_, DonorRow>(
            "SELECT id, name, name_updated_at, name_updated_by, name_updated_by_device_id, type_ AS type, type_updated_at, type_updated_by, type_updated_by_device_id, contact_person, contact_person_updated_at, contact_person_updated_by, contact_person_updated_by_device_id, email, email_updated_at, email_updated_by, email_updated_by_device_id, phone, phone_updated_at, phone_updated_by, phone_updated_by_device_id, country, country_updated_at, country_updated_by, country_updated_by_device_id, first_donation_date, first_donation_date_updated_at, first_donation_date_updated_by, first_donation_date_updated_by_device_id, notes, notes_updated_at, notes_updated_by, notes_updated_by_device_id, created_at, updated_at, created_by_user_id, created_by_device_id, updated_by_user_id, updated_by_device_id, deleted_at, deleted_by_user_id, deleted_by_device_id FROM donors WHERE deleted_at IS NULL ORDER BY name ASC LIMIT ? OFFSET ?"
        )
        .bind(params.per_page as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        let entities = rows
            .into_iter()
            .map(Self::map_row_to_entity)
            .collect::<DomainResult<Vec<Donor>>>()?;

        Ok(PaginatedResult::new(entities, total as u64, params))
    }

    async fn count_by_type(&self) -> DomainResult<Vec<(Option<String>, i64)>> {
        let counts = query_as::<_, (Option<String>, i64)>(
            "SELECT type_, COUNT(*) 
             FROM donors 
             WHERE deleted_at IS NULL 
             GROUP BY type_"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        Ok(counts)
    }

    async fn count_by_country(&self) -> DomainResult<Vec<(Option<String>, i64)>> {
        let counts = query_as::<_, (Option<String>, i64)>(
            "SELECT country, COUNT(*) 
             FROM donors 
             WHERE deleted_at IS NULL 
             GROUP BY country"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        Ok(counts)
    }

    async fn get_donation_stats(&self) -> DomainResult<DonorStatsSummary> {
        let total_donors: i64 = query_scalar(
            "SELECT COUNT(*) FROM donors WHERE deleted_at IS NULL"
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        let active_donors: i64 = query_scalar(
            "SELECT COUNT(DISTINCT d.id) 
             FROM donors d
             JOIN project_funding pf ON d.id = pf.donor_id
             WHERE d.deleted_at IS NULL
             AND pf.deleted_at IS NULL
             AND (pf.status = 'Committed' OR pf.status = 'Received')"
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        let (total_amount, avg_amount): (Option<f64>, Option<f64>) = query_as(
            "SELECT SUM(amount), AVG(amount)
             FROM project_funding
             WHERE deleted_at IS NULL
             AND donor_id IN (SELECT id FROM donors WHERE deleted_at IS NULL)"
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        let type_counts = self.count_by_type().await?;
        let mut donor_count_by_type = HashMap::new();
        for (type_opt, count) in type_counts {
            let type_name = type_opt.unwrap_or_else(|| "Unspecified".to_string());
            donor_count_by_type.insert(type_name, count);
        }

        let country_counts = self.count_by_country().await?;
        let mut donor_count_by_country = HashMap::new();
        for (country_opt, count) in country_counts {
            let country_name = country_opt.unwrap_or_else(|| "Unspecified".to_string());
            donor_count_by_country.insert(country_name, count);
        }

        Ok(DonorStatsSummary {
            total_donors,
            active_donors,
            total_donation_amount: total_amount,
            avg_donation_amount: avg_amount,
            donor_count_by_type,
            donor_count_by_country,
        })
    }

    async fn find_by_type(
        &self,
        donor_type: &str,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Donor>> {
        let offset = (params.page - 1) * params.per_page;

        let total: i64 = query_scalar(
            "SELECT COUNT(*) FROM donors WHERE type_ = ? AND deleted_at IS NULL"
        )
        .bind(donor_type)
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        let rows = query_as::<_, DonorRow>(
            "SELECT id, name, name_updated_at, name_updated_by, name_updated_by_device_id, type_ AS type, type_updated_at, type_updated_by, type_updated_by_device_id, contact_person, contact_person_updated_at, contact_person_updated_by, contact_person_updated_by_device_id, email, email_updated_at, email_updated_by, email_updated_by_device_id, phone, phone_updated_at, phone_updated_by, phone_updated_by_device_id, country, country_updated_at, country_updated_by, country_updated_by_device_id, first_donation_date, first_donation_date_updated_at, first_donation_date_updated_by, first_donation_date_updated_by_device_id, notes, notes_updated_at, notes_updated_by, notes_updated_by_device_id, created_at, updated_at, created_by_user_id, created_by_device_id, updated_by_user_id, updated_by_device_id, deleted_at, deleted_by_user_id, deleted_by_device_id FROM donors WHERE type_ = ? AND deleted_at IS NULL 
             ORDER BY name ASC LIMIT ? OFFSET ?"
        )
        .bind(donor_type)
        .bind(params.per_page as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        let entities = rows
            .into_iter()
            .map(Self::map_row_to_entity)
            .collect::<DomainResult<Vec<Donor>>>()?;

        Ok(PaginatedResult::new(
            entities,
            total as u64,
            params,
        ))
    }

    async fn find_by_country(
        &self,
        country: &str,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Donor>> {
        let offset = (params.page - 1) * params.per_page;

        let total: i64 = query_scalar(
            "SELECT COUNT(*) FROM donors WHERE country = ? AND deleted_at IS NULL"
        )
        .bind(country)
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        let rows = query_as::<_, DonorRow>(
            "SELECT id, name, name_updated_at, name_updated_by, name_updated_by_device_id, type_ AS type, type_updated_at, type_updated_by, type_updated_by_device_id, contact_person, contact_person_updated_at, contact_person_updated_by, contact_person_updated_by_device_id, email, email_updated_at, email_updated_by, email_updated_by_device_id, phone, phone_updated_at, phone_updated_by, phone_updated_by_device_id, country, country_updated_at, country_updated_by, country_updated_by_device_id, first_donation_date, first_donation_date_updated_at, first_donation_date_updated_by, first_donation_date_updated_by_device_id, notes, notes_updated_at, notes_updated_by, notes_updated_by_device_id, created_at, updated_at, created_by_user_id, created_by_device_id, updated_by_user_id, updated_by_device_id, deleted_at, deleted_by_user_id, deleted_by_device_id FROM donors WHERE country = ? AND deleted_at IS NULL 
             ORDER BY name ASC LIMIT ? OFFSET ?"
        )
        .bind(country)
        .bind(params.per_page as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        let entities = rows
            .into_iter()
            .map(Self::map_row_to_entity)
            .collect::<DomainResult<Vec<Donor>>>()?;

        Ok(PaginatedResult::new(
            entities,
            total as u64,
            params,
        ))
    }

    async fn find_with_recent_donations(
        &self,
        since_date: &str,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Donor>> {
        let offset = (params.page - 1) * params.per_page;

        let total: i64 = query_scalar(
            "SELECT COUNT(DISTINCT d.id) 
             FROM donors d
             JOIN project_funding pf ON d.id = pf.donor_id
             WHERE d.deleted_at IS NULL
             AND pf.deleted_at IS NULL
             AND pf.start_date >= ?"
        )
        .bind(since_date)
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        let rows = query_as::<_, DonorRow>(
            "SELECT DISTINCT d.id, d.name, d.name_updated_at, d.name_updated_by, d.name_updated_by_device_id, d.type_ AS type, d.type_updated_at, d.type_updated_by, d.type_updated_by_device_id, d.contact_person, d.contact_person_updated_at, d.contact_person_updated_by, d.contact_person_updated_by_device_id, d.email, d.email_updated_at, d.email_updated_by, d.email_updated_by_device_id, d.phone, d.phone_updated_at, d.phone_updated_by, d.phone_updated_by_device_id, d.country, d.country_updated_at, d.country_updated_by, d.country_updated_by_device_id, d.first_donation_date, d.first_donation_date_updated_at, d.first_donation_date_updated_by, d.first_donation_date_updated_by_device_id, d.notes, d.notes_updated_at, d.notes_updated_by, d.notes_updated_by_device_id, d.created_at, d.updated_at, d.created_by_user_id, d.created_by_device_id, d.updated_by_user_id, d.updated_by_device_id, d.deleted_at, d.deleted_by_user_id, d.deleted_by_device_id
             FROM donors d
             JOIN project_funding pf ON d.id = pf.donor_id
             WHERE d.deleted_at IS NULL
             AND pf.deleted_at IS NULL
             AND pf.start_date >= ?
             ORDER BY d.name ASC
             LIMIT ? OFFSET ?"
        )
        .bind(since_date)
        .bind(params.per_page as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        let entities = rows
            .into_iter()
            .map(Self::map_row_to_entity)
            .collect::<DomainResult<Vec<Donor>>>()?;

        Ok(PaginatedResult::new(
            entities,
            total as u64,
            params,
        ))
    }

    async fn find_ids_by_user_role(
        &self,
        user_id: Uuid,
        role: UserDonorRole,
    ) -> DomainResult<Vec<Uuid>> {
        let user_id_str = user_id.to_string();
        
        let query_str = match role {
            UserDonorRole::Created => {
                "SELECT id FROM donors WHERE created_by_user_id = ? AND deleted_at IS NULL"
            }
            UserDonorRole::Updated => {
                "SELECT id FROM donors WHERE updated_by_user_id = ? AND deleted_at IS NULL"
            }
        };

        let id_strings: Vec<String> = query_scalar(query_str)
            .bind(&user_id_str)
            .fetch_all(&self.pool)
            .await
            .map_err(DbError::from)?;

        let ids = id_strings
            .into_iter()
            .map(|id_str| Uuid::parse_str(&id_str).map_err(|_| DomainError::InvalidUuid(id_str)))
            .collect::<Result<Vec<Uuid>, DomainError>>()?;

        Ok(ids)
    }

    /// Find donors within a date range (created_at or updated_at)
    async fn find_by_date_range(
        &self,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Donor>> {
        let offset = (params.page - 1) * params.per_page;

        let total: i64 = query_scalar(
            "SELECT COUNT(DISTINCT d.id) 
             FROM donors d
             WHERE d.deleted_at IS NULL
             AND (d.created_at >= ? OR d.updated_at >= ?)
             AND (d.created_at < ? OR d.updated_at < ?)"
        )
        .bind(start_date.to_rfc3339())
        .bind(start_date.to_rfc3339())
        .bind(end_date.to_rfc3339())
        .bind(end_date.to_rfc3339())
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        let rows = query_as::<_, DonorRow>(
            "SELECT DISTINCT d.id, d.name, d.name_updated_at, d.name_updated_by, d.name_updated_by_device_id, d.type_ AS type, d.type_updated_at, d.type_updated_by, d.type_updated_by_device_id, d.contact_person, d.contact_person_updated_at, d.contact_person_updated_by, d.contact_person_updated_by_device_id, d.email, d.email_updated_at, d.email_updated_by, d.email_updated_by_device_id, d.phone, d.phone_updated_at, d.phone_updated_by, d.phone_updated_by_device_id, d.country, d.country_updated_at, d.country_updated_by, d.country_updated_by_device_id, d.first_donation_date, d.first_donation_date_updated_at, d.first_donation_date_updated_by, d.first_donation_date_updated_by_device_id, d.notes, d.notes_updated_at, d.notes_updated_by, d.notes_updated_by_device_id, d.created_at, d.updated_at, d.created_by_user_id, d.created_by_device_id, d.updated_by_user_id, d.updated_by_device_id, d.deleted_at, d.deleted_by_user_id, d.deleted_by_device_id
             FROM donors d
             WHERE d.deleted_at IS NULL
             AND (d.created_at >= ? OR d.updated_at >= ?)
             AND (d.created_at < ? OR d.updated_at < ?)
             ORDER BY d.name ASC
             LIMIT ? OFFSET ?"
        )
        .bind(start_date.to_rfc3339())
        .bind(start_date.to_rfc3339())
        .bind(end_date.to_rfc3339())
        .bind(end_date.to_rfc3339())
        .bind(params.per_page as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        let entities = rows
            .into_iter()
            .map(Self::map_row_to_entity)
            .collect::<DomainResult<Vec<Donor>>>()?;

        Ok(PaginatedResult::new(
            entities,
            total as u64,
            params,
        ))
    }

    /// Find donors by specific IDs
    async fn find_by_ids(
        &self,
        ids: &[Uuid],
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Donor>> {
        if ids.is_empty() {
            return Ok(PaginatedResult::new(Vec::new(), 0, params));
        }

        let offset = (params.page - 1) * params.per_page;

        // Build COUNT query with dynamic placeholders
        let count_placeholders = vec!["?"; ids.len()].join(", ");
        let count_query = format!(
            "SELECT COUNT(*) FROM donors WHERE id IN ({}) AND deleted_at IS NULL",
            count_placeholders
        );

        let mut count_builder = QueryBuilder::new(&count_query);
        for id in ids {
            count_builder.push_bind(id.to_string());
        }

        let total: i64 = count_builder
            .build_query_scalar()
            .fetch_one(&self.pool)
            .await
            .map_err(DbError::from)?;

        // Build SELECT query with dynamic placeholders
        let select_placeholders = vec!["?"; ids.len()].join(", ");
        let select_query = format!(
            "SELECT id, name, name_updated_at, name_updated_by, name_updated_by_device_id, type_ AS type, type_updated_at, type_updated_by, type_updated_by_device_id, contact_person, contact_person_updated_at, contact_person_updated_by, contact_person_updated_by_device_id, email, email_updated_at, email_updated_by, email_updated_by_device_id, phone, phone_updated_at, phone_updated_by, phone_updated_by_device_id, country, country_updated_at, country_updated_by, country_updated_by_device_id, first_donation_date, first_donation_date_updated_at, first_donation_date_updated_by, first_donation_date_updated_by_device_id, notes, notes_updated_at, notes_updated_by, notes_updated_by_device_id, created_at, updated_at, created_by_user_id, created_by_device_id, updated_by_user_id, updated_by_device_id, deleted_at, deleted_by_user_id, deleted_by_device_id FROM donors WHERE id IN ({}) AND deleted_at IS NULL ORDER BY name ASC LIMIT ? OFFSET ?",
            select_placeholders
        );

        let mut select_builder = QueryBuilder::new(&select_query);
        for id in ids {
            select_builder.push_bind(id.to_string());
        }
        select_builder.push_bind(params.per_page as i64);
        select_builder.push_bind(offset as i64);

        let rows = select_builder
            .build_query_as::<DonorRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(DbError::from)?;

        let entities = rows
            .into_iter()
            .map(Self::map_row_to_entity)
            .collect::<DomainResult<Vec<Donor>>>()?;

        Ok(PaginatedResult::new(
            entities,
            total as u64,
            params,
        ))
    }
}
