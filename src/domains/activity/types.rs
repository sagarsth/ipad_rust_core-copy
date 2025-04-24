// src/domains/activity/types.rs

use crate::errors::{DomainError, DomainResult};
use crate::validation::{Validate, ValidationBuilder};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use sqlx::FromRow;

/// Activity entity - represents a specific activity within a project
/// Aligned with v1_schema.sql
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Activity {
    pub id: Uuid,
    pub project_id: Option<Uuid>,
    // Removed name fields
    pub description: Option<String>,
    pub description_updated_at: Option<DateTime<Utc>>,
    pub description_updated_by: Option<Uuid>,
    // Added KPI fields
    pub kpi: Option<String>,
    pub kpi_updated_at: Option<DateTime<Utc>>,
    pub kpi_updated_by: Option<Uuid>,
    // Added Target Value fields
    pub target_value: Option<f64>,
    pub target_value_updated_at: Option<DateTime<Utc>>,
    pub target_value_updated_by: Option<Uuid>,
    // Added Actual Value fields
    pub actual_value: f64, // Assumes DEFAULT 0 in schema
    pub actual_value_updated_at: Option<DateTime<Utc>>,
    pub actual_value_updated_by: Option<Uuid>,
    // Removed start/end date fields
    pub status_id: Option<i64>,
    pub status_id_updated_at: Option<DateTime<Utc>>,
    pub status_id_updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by_user_id: Option<Uuid>,
    pub updated_by_user_id: Option<Uuid>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub deleted_by_user_id: Option<Uuid>,
}

impl Activity {
    /// Helper to check if activity is deleted
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }

    // Removed date-related helpers: parsed_start_date, parsed_end_date, is_active

     // Helper to calculate progress percentage against target, if applicable
    pub fn progress_percentage(&self) -> Option<f64> {
        if let Some(target) = self.target_value {
            if target > 0.0 {
                // Ensure we don't divide by zero
                return Some((self.actual_value / target) * 100.0);
            }
        }
        None // No target or target is zero
    }
}

/// NewActivity DTO - used when creating a new activity
/// Aligned with v1_schema.sql
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewActivity {
    pub project_id: Option<Uuid>,
    // Removed name
    pub description: Option<String>, // Made optional, adjust if description is required
    // Added KPI, Target, Actual
    pub kpi: Option<String>,
    pub target_value: Option<f64>,
    pub actual_value: Option<f64>, // Optional on create, defaults to 0 in DB
    // Removed start/end date
    pub status_id: Option<i64>,
    pub created_by_user_id: Option<Uuid>,
}

impl Validate for NewActivity {
    fn validate(&self) -> DomainResult<()> {
        // Validate project_id IF provided
        if let Some(p_id) = self.project_id {
            ValidationBuilder::new("project_id", Some(p_id))
                .not_nil()
                .validate()?;
        }

        // Validate description if it's considered required
        // If optional, this validation can be removed or adjusted.
        // ValidationBuilder::new("description", self.description.as_deref())
        //     .required()
        //     .min_length(2)
        //     .max_length(500) // Example max length
        //     .validate()?;

        // Validate target_value if provided (must be non-negative)
        if let Some(target) = self.target_value {
            ValidationBuilder::new("target_value", Some(target))
                .min(0.0)
                .validate()?;
        }

        // Validate actual_value if provided (must be non-negative)
        if let Some(actual) = self.actual_value {
            ValidationBuilder::new("actual_value", Some(actual))
                .min(0.0)
                .validate()?;
        }

        Ok(())
    }
}

/// UpdateActivity DTO - used when updating an existing activity
/// Aligned with v1_schema.sql
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateActivity {
    // Use Option<Option<Uuid>> to allow setting project_id to NULL
    pub project_id: Option<Option<Uuid>>, 
    // Removed name
    pub description: Option<String>,
    // Added KPI, Target, Actual
    pub kpi: Option<String>,
    pub target_value: Option<f64>,
    pub actual_value: Option<f64>,
    // Removed start/end date
    pub status_id: Option<i64>,
    pub updated_by_user_id: Uuid, // Required for tracking who made the update
}

impl Validate for UpdateActivity {
     fn validate(&self) -> DomainResult<()> {
        // Validate project_id if explicitly provided (even if setting to None)
        if let Some(opt_p_id) = self.project_id {
            if let Some(p_id) = opt_p_id {
                // If an ID is actually provided, ensure it's not nil
                 ValidationBuilder::new("project_id", Some(p_id))
                    .not_nil()
                    .validate()?;
            }
            // Allow Some(None) - represents setting the field to null
        }

        // Validate description if provided
        // if let Some(desc) = &self.description {
        //     ValidationBuilder::new("description", Some(desc.clone()))
        //         .min_length(2)
        //         .max_length(500)
        //         .validate()?;
        // }

         // Validate target_value if provided (must be non-negative)
        if let Some(target) = self.target_value {
            ValidationBuilder::new("target_value", Some(target))
                .min(0.0)
                .validate()?;
        }

        // Validate actual_value if provided (must be non-negative)
        if let Some(actual) = self.actual_value {
            ValidationBuilder::new("actual_value", Some(actual))
                .min(0.0)
                .validate()?;
        }

        Ok(())
    }
}

/// ActivityRow - SQLite row representation for mapping from database
/// Aligned with v1_schema.sql
#[derive(Debug, Clone, FromRow)]
pub struct ActivityRow {
    pub id: String,
    pub project_id: Option<String>,
    // Removed name fields
    pub description: Option<String>,
    pub description_updated_at: Option<String>,
    pub description_updated_by: Option<String>,
    // Added KPI fields
    pub kpi: Option<String>,
    pub kpi_updated_at: Option<String>,
    pub kpi_updated_by: Option<String>,
    // Added Target Value fields
    pub target_value: Option<f64>,
    pub target_value_updated_at: Option<String>,
    pub target_value_updated_by: Option<String>,
    // Added Actual Value fields
    pub actual_value: f64,
    pub actual_value_updated_at: Option<String>,
    pub actual_value_updated_by: Option<String>,
    // Removed start/end date fields
    pub status_id: Option<i64>,
    pub status_id_updated_at: Option<String>,
    pub status_id_updated_by: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub created_by_user_id: Option<String>,
    pub updated_by_user_id: Option<String>,
    pub deleted_at: Option<String>,
    pub deleted_by_user_id: Option<String>,
}

impl ActivityRow {
    /// Convert database row to domain entity
    pub fn into_entity(self) -> DomainResult<Activity> {
        let parse_uuid = |s: &Option<String>| -> Option<DomainResult<Uuid>> {
            s.as_ref().map(|id_str| {
                Uuid::parse_str(id_str)
                    .map_err(|_| DomainError::Internal(format!("Invalid UUID format in DB: {}", id_str)))
            })
        };

        let parse_datetime = |s: &Option<String>| -> Option<DomainResult<DateTime<Utc>>> {
            s.as_ref().map(|dt_str| {
                DateTime::parse_from_rfc3339(dt_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .map_err(|_| DomainError::Internal(format!("Invalid RFC3339 format in DB: {}", dt_str)))
            })
        };

        Ok(Activity {
            id: Uuid::parse_str(&self.id)
                 .map_err(|_| DomainError::Internal(format!("Invalid primary key UUID format in DB: {}", self.id)))?,
            project_id: parse_uuid(&self.project_id).transpose()?,
            // name removed
            description: self.description,
            description_updated_at: parse_datetime(&self.description_updated_at).transpose()?,
            description_updated_by: parse_uuid(&self.description_updated_by).transpose()?,
            // kpi added
            kpi: self.kpi,
            kpi_updated_at: parse_datetime(&self.kpi_updated_at).transpose()?,
            kpi_updated_by: parse_uuid(&self.kpi_updated_by).transpose()?,
            // target_value added
            target_value: self.target_value,
            target_value_updated_at: parse_datetime(&self.target_value_updated_at).transpose()?,
            target_value_updated_by: parse_uuid(&self.target_value_updated_by).transpose()?,
            // actual_value added
            actual_value: self.actual_value,
            actual_value_updated_at: parse_datetime(&self.actual_value_updated_at).transpose()?,
            actual_value_updated_by: parse_uuid(&self.actual_value_updated_by).transpose()?,
            // start/end date removed
            status_id: self.status_id,
            status_id_updated_at: parse_datetime(&self.status_id_updated_at).transpose()?,
            status_id_updated_by: parse_uuid(&self.status_id_updated_by).transpose()?,
            created_at: DateTime::parse_from_rfc3339(&self.created_at)
                .map(|dt| dt.with_timezone(&Utc))
                 .map_err(|_| DomainError::Internal(format!("Invalid created_at format in DB: {}", self.created_at)))?,
            updated_at: DateTime::parse_from_rfc3339(&self.updated_at)
                .map(|dt| dt.with_timezone(&Utc))
                 .map_err(|_| DomainError::Internal(format!("Invalid updated_at format in DB: {}", self.updated_at)))?,
            created_by_user_id: parse_uuid(&self.created_by_user_id).transpose()?,
            updated_by_user_id: parse_uuid(&self.updated_by_user_id).transpose()?,
            deleted_at: parse_datetime(&self.deleted_at).transpose()?,
            deleted_by_user_id: parse_uuid(&self.deleted_by_user_id).transpose()?,
        })
    }
}

// --- Response DTOs ---

/// Basic project summary for nested responses (consider moving to shared location)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSummary {
    pub id: Uuid,
    pub name: String,
}

/// Status information (consider moving to shared location)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusInfo {
    pub id: i64,
    pub value: String,
}

/// ActivityResponse DTO - used for API responses
/// Aligned with v1_schema.sql
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityResponse {
    pub id: Uuid,
    pub project_id: Option<Uuid>,
    // Removed name
    pub description: Option<String>,
    // Added KPI, Target, Actual
    pub kpi: Option<String>,
    pub target_value: Option<f64>,
    pub actual_value: f64,
    pub progress_percentage: Option<f64>, // Calculated field
    // Removed start/end date, is_active
    pub status_id: Option<i64>,
    pub status: Option<StatusInfo>,      // Populated when details are fetched
    pub project: Option<ProjectSummary>, // Populated when details are fetched
    pub created_at: String,
    pub updated_at: String,
}

impl From<Activity> for ActivityResponse {
    fn from(activity: Activity) -> Self {
        let progress = activity.progress_percentage(); // Calculate before move
        Self {
            id: activity.id,
            project_id: activity.project_id,
            // name removed
            description: activity.description,
            // kpi, target, actual added
            kpi: activity.kpi,
            target_value: activity.target_value,
            actual_value: activity.actual_value,
            progress_percentage: progress,
            // start/end date, is_active removed
            status_id: activity.status_id,
            status: None, // Needs to be populated separately
            project: None, // Needs to be populated separately
            created_at: activity.created_at.to_rfc3339(),
            updated_at: activity.updated_at.to_rfc3339(),
        }
    }
}

impl ActivityResponse {
     /// Add project information
    pub fn with_project(mut self, project: ProjectSummary) -> Self {
        self.project = Some(project);
        self
    }

    /// Add status information
    pub fn with_status(mut self, status: StatusInfo) -> Self {
        self.status = Some(status);
        self
    }
}