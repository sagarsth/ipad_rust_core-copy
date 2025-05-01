use crate::auth::AuthContext;
use crate::domains::core::repository::{FindById, HardDeletable, SoftDeletable};
use crate::domains::core::delete_service::DeleteServiceRepository;
use crate::domains::core::document_linking::DocumentLinkable;
use crate::domains::donor::types::{Donor, NewDonor, UpdateDonor, DonorRow, UserDonorRole, DonorStatsSummary};
use crate::errors::{DbError, DomainError, DomainResult, ValidationError};
use crate::types::{PaginatedResult, PaginationParams};
use async_trait::async_trait;
use chrono::Utc;
use sqlx::{Pool, Sqlite, Transaction, query, query_as, query_scalar, QueryBuilder};
use std::collections::HashMap;
use uuid::Uuid;

/// Trait defining donor repository operations
#[async_trait]
pub trait DonorRepository: 
    DeleteServiceRepository<Donor> + Send + Sync 
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
    ) -> DomainResult<Donor> {
        let row = query_as::<_, DonorRow>(
            "SELECT * FROM donors WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(id.to_string())
        .fetch_optional(&mut **tx)
        .await
        .map_err(DbError::from)?
        .ok_or_else(|| DomainError::EntityNotFound("Donor".to_string(), id))?;

        Self::map_row_to_entity(row)
    }
}

#[async_trait]
impl FindById<Donor> for SqliteDonorRepository {
    async fn find_by_id(&self, id: Uuid) -> DomainResult<Donor> {
        let row = query_as::<_, DonorRow>(
            "SELECT * FROM donors WHERE id = ? AND deleted_at IS NULL",
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
        let result = query(
            "UPDATE donors SET deleted_at = ?, deleted_by_user_id = ? WHERE id = ? AND deleted_at IS NULL"
        )
        .bind(now)
        .bind(auth.user_id.to_string())
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
        SqliteDonorRepository::entity_name(self)
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

// Blanket implementation in core::delete_service handles DeleteServiceRepository

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
        let now = Utc::now().to_rfc3339();
        let user_id_str = auth.user_id.to_string();
        let created_by_id_str = new_donor.created_by_user_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| user_id_str.clone()); // Fallback to current user if not specified

        let mut builder = QueryBuilder::new(
            r#"INSERT INTO donors (
                id, name, name_updated_at, name_updated_by, 
                type_, type_updated_at, type_updated_by, 
                contact_person, contact_person_updated_at, contact_person_updated_by,
                email, email_updated_at, email_updated_by,
                phone, phone_updated_at, phone_updated_by,
                country, country_updated_at, country_updated_by,
                first_donation_date, first_donation_date_updated_at, first_donation_date_updated_by,
                notes, notes_updated_at, notes_updated_by,
                created_at, updated_at, created_by_user_id, updated_by_user_id,
                deleted_at, deleted_by_user_id
            ) "#
        );

        builder.push_values([ (
            id.to_string(), new_donor.name.clone(), now.clone(), user_id_str.clone(),
            new_donor.type_.clone(), new_donor.type_.as_ref().map(|_| &now), new_donor.type_.as_ref().map(|_| &user_id_str),
            new_donor.contact_person.clone(), new_donor.contact_person.as_ref().map(|_| &now), new_donor.contact_person.as_ref().map(|_| &user_id_str),
            new_donor.email.clone(), new_donor.email.as_ref().map(|_| &now), new_donor.email.as_ref().map(|_| &user_id_str),
            new_donor.phone.clone(), new_donor.phone.as_ref().map(|_| &now), new_donor.phone.as_ref().map(|_| &user_id_str),
            new_donor.country.clone(), new_donor.country.as_ref().map(|_| &now), new_donor.country.as_ref().map(|_| &user_id_str),
            new_donor.first_donation_date.clone(), new_donor.first_donation_date.as_ref().map(|_| &now), new_donor.first_donation_date.as_ref().map(|_| &user_id_str),
            new_donor.notes.clone(), new_donor.notes.as_ref().map(|_| &now), new_donor.notes.as_ref().map(|_| &user_id_str),
            now.clone(), now.clone(), created_by_id_str, user_id_str.clone(),
            Option::<String>::None, Option::<String>::None // deleted_at, deleted_by_user_id are NULL
        )], |mut b, values| {
            b.push_bind(values.0); b.push_bind(values.1); b.push_bind(values.2); b.push_bind(values.3);
            b.push_bind(values.4); b.push_bind(values.5); b.push_bind(values.6);
            b.push_bind(values.7); b.push_bind(values.8); b.push_bind(values.9);
            b.push_bind(values.10); b.push_bind(values.11); b.push_bind(values.12);
            b.push_bind(values.13); b.push_bind(values.14); b.push_bind(values.15);
            b.push_bind(values.16); b.push_bind(values.17); b.push_bind(values.18);
            b.push_bind(values.19); b.push_bind(values.20); b.push_bind(values.21);
            b.push_bind(values.22); b.push_bind(values.23); b.push_bind(values.24);
            b.push_bind(values.25); b.push_bind(values.26); b.push_bind(values.27); b.push_bind(values.28);
            b.push_bind(values.29); b.push_bind(values.30);
        });

        let query = builder.build();
        query.execute(&mut **tx).await.map_err(DbError::from)?;

        self.find_by_id_with_tx(id, tx).await
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
        let _ = self.find_by_id_with_tx(id, tx).await?; // Ensure exists
        
        let now = Utc::now().to_rfc3339();
        let user_id_str = auth.user_id.to_string();
        let id_str = id.to_string();

        // Define LWW macros locally
        macro_rules! add_lww_option {($builder:expr, $separated:expr, $field_sql:literal, $value:expr, $now_ref:expr, $user_id_ref:expr, $fields_updated_flag:expr) => {
            if let Some(ref val) = $value {
                $separated.push(concat!($field_sql, " = "));
                $separated.push_bind_unseparated(val.clone());
                $separated.push(concat!(" ", $field_sql, "_updated_at = "));
                $separated.push_bind_unseparated($now_ref.clone());
                $separated.push(concat!(" ", $field_sql, "_updated_by = "));
                $separated.push_bind_unseparated($user_id_ref.clone());
                $fields_updated_flag = true;
            }
        };}

        let mut builder = QueryBuilder::new("UPDATE donors SET ");
        let mut separated = builder.separated(", ");
        let mut fields_updated = false;

        // Apply LWW updates using the macro
        add_lww_option!(builder, separated, "name", update_data.name, &now, &user_id_str, fields_updated);
        add_lww_option!(builder, separated, "type_", update_data.type_, &now, &user_id_str, fields_updated);
        add_lww_option!(builder, separated, "contact_person", update_data.contact_person, &now, &user_id_str, fields_updated);
        add_lww_option!(builder, separated, "email", update_data.email, &now, &user_id_str, fields_updated);
        add_lww_option!(builder, separated, "phone", update_data.phone, &now, &user_id_str, fields_updated);
        add_lww_option!(builder, separated, "country", update_data.country, &now, &user_id_str, fields_updated);
        add_lww_option!(builder, separated, "first_donation_date", update_data.first_donation_date, &now, &user_id_str, fields_updated);
        add_lww_option!(builder, separated, "notes", update_data.notes, &now, &user_id_str, fields_updated);

        // If no fields were updated, just return the existing entity
        if !fields_updated {
            return self.find_by_id_with_tx(id, tx).await;
        }

        // Always update updated_at and updated_by_user_id
        separated.push("updated_at = ");
        separated.push_bind_unseparated(now);
        separated.push("updated_by_user_id = ");
        separated.push_bind_unseparated(user_id_str);

        builder.push(" WHERE id = ");
        builder.push_bind(id_str);
        builder.push(" AND deleted_at IS NULL");

        let query = builder.build();
        let result = query.execute(&mut **tx).await.map_err(DbError::from)?;

        if result.rows_affected() == 0 {
            // This case should ideally not happen due to the find_by_id check earlier,
            // but handle it just in case (e.g., race condition or deleted between checks)
            return Err(DomainError::EntityNotFound("Donor".to_string(), id));
        }

        self.find_by_id_with_tx(id, tx).await
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
            "SELECT * FROM donors WHERE deleted_at IS NULL ORDER BY name ASC LIMIT ? OFFSET ?"
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
        // Get total donor count
        let total_donors: i64 = query_scalar(
            "SELECT COUNT(*) FROM donors WHERE deleted_at IS NULL"
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        // Get active donors (those with active fundings)
        let active_donors: i64 = query_scalar(
            "SELECT COUNT(DISTINCT d.id) 
             FROM donors d
             JOIN project_funding pf ON d.id = pf.donor_id
             WHERE d.deleted_at IS NULL
             AND pf.deleted_at IS NULL
             AND (pf.status = 'Committed' OR pf.status = 'Received')" // Adjust status check as needed
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        // Get funding amounts
        // Note: SUM/AVG might return NULL if no matching rows, hence Option<f64>
        let (total_amount, avg_amount): (Option<f64>, Option<f64>) = query_as(
            "SELECT SUM(amount), AVG(amount)
             FROM project_funding
             WHERE deleted_at IS NULL
             AND donor_id IN (SELECT id FROM donors WHERE deleted_at IS NULL)"
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        // Get donor counts by type
        let type_counts = self.count_by_type().await?;
        let mut donor_count_by_type = HashMap::new();
        for (type_opt, count) in type_counts {
            let type_name = type_opt.unwrap_or_else(|| "Unspecified".to_string());
            donor_count_by_type.insert(type_name, count);
        }

        // Get donor counts by country
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

        // Get total count
        let total: i64 = query_scalar(
            "SELECT COUNT(*) FROM donors WHERE type_ = ? AND deleted_at IS NULL"
        )
        .bind(donor_type)
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        // Fetch paginated rows
        let rows = query_as::<_, DonorRow>(
            "SELECT * FROM donors WHERE type_ = ? AND deleted_at IS NULL 
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

        // Get total count
        let total: i64 = query_scalar(
            "SELECT COUNT(*) FROM donors WHERE country = ? AND deleted_at IS NULL"
        )
        .bind(country)
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        // Fetch paginated rows
        let rows = query_as::<_, DonorRow>(
            "SELECT * FROM donors WHERE country = ? AND deleted_at IS NULL 
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

        // Get total count of donors with recent donations
        // Ensure date format matches DB storage (assuming TEXT YYYY-MM-DD for start_date)
        let total: i64 = query_scalar(
            "SELECT COUNT(DISTINCT d.id) 
             FROM donors d
             JOIN project_funding pf ON d.id = pf.donor_id
             WHERE d.deleted_at IS NULL
             AND pf.deleted_at IS NULL
             AND pf.start_date >= ?" // Direct comparison assumes compatible date formats
        )
        .bind(since_date)
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        // Fetch paginated rows
        let rows = query_as::<_, DonorRow>(
            "SELECT DISTINCT d.* 
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
        
        // Build query based on role
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

        // Convert string IDs to UUIDs, handling potential errors
        let ids = id_strings
            .into_iter()
            .map(|id_str| Uuid::parse_str(&id_str).map_err(|_| DomainError::InvalidUuid(id_str)))
            .collect::<Result<Vec<Uuid>, DomainError>>()?;

        Ok(ids)
    }
}
