use crate::errors::{DomainError, DomainResult, ValidationError};
use crate::validation::{Validate, ValidationBuilder};
use crate::domains::document::types::MediaDocumentResponse;
use crate::types::SyncPriority;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use sqlx::FromRow;
use crate::domains::core::document_linking::{DocumentLinkable, EntityFieldMetadata, FieldType};
use std::collections::{HashSet, HashMap};
use std::str::FromStr;
use crate::domains::project::types::ProjectSummary;
use crate::domains::sync::types::SyncPriority as SyncPriorityFromSyncDomain;

/// Role a user can have in relation to a strategic goal
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UserGoalRole {
    Created,    // User created the goal
    Updated,    // User last updated the goal
}

/// Summary of aggregate value statistics for strategic goals from the repository
#[derive(Debug, Clone, FromRow)]
pub struct GoalValueSummary {
    pub avg_target: Option<f64>,
    pub avg_actual: Option<f64>,
    pub total_target: Option<f64>,
    pub total_actual: Option<f64>,
    pub count: i64,
}

/// Response DTO for value summary statistics (calculated in service)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalValueSummaryResponse {
    pub avg_target: Option<f64>,
    pub avg_actual: Option<f64>,
    pub total_target: Option<f64>,
    pub total_actual: Option<f64>,
    pub count: i64,
    pub avg_progress_percentage: Option<f64>,
}

/// Enum to specify related data to include for Strategic Goals
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StrategicGoalInclude {
    Documents,
    Status,          // Include status information
    Projects,        // Include related projects
    ProjectCount,    // New: Include count of related projects
    Activities,      // Include activities (via projects)
    Participants,    // Include participants (via workshops)
    DocumentCounts,  // Include just document counts
}

impl FromStr for StrategicGoalInclude {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "documents" => Ok(Self::Documents),
            "status" => Ok(Self::Status),
            "projects" => Ok(Self::Projects),
            "projectcount" | "project_count" => Ok(Self::ProjectCount),
            "activities" => Ok(Self::Activities),
            "participants" => Ok(Self::Participants),
            "documentcounts" | "document_counts" => Ok(Self::DocumentCounts),
            _ => Err(format!("Unknown include option: {}", s)),
        }
    }
}

/// Strategic Goal entity - represents a strategic goal in the system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategicGoal {
    pub id: Uuid,
    pub objective_code: String,
    pub objective_code_updated_at: Option<DateTime<Utc>>,
    pub objective_code_updated_by: Option<Uuid>,
    pub objective_code_updated_by_device_id: Option<Uuid>,
    pub outcome: Option<String>,
    pub outcome_updated_at: Option<DateTime<Utc>>,
    pub outcome_updated_by: Option<Uuid>,
    pub outcome_updated_by_device_id: Option<Uuid>,
    pub kpi: Option<String>,
    pub kpi_updated_at: Option<DateTime<Utc>>,
    pub kpi_updated_by: Option<Uuid>,
    pub kpi_updated_by_device_id: Option<Uuid>,
    pub target_value: Option<f64>,
    pub target_value_updated_at: Option<DateTime<Utc>>,
    pub target_value_updated_by: Option<Uuid>,
    pub target_value_updated_by_device_id: Option<Uuid>,
    pub actual_value: Option<f64>,
    pub actual_value_updated_at: Option<DateTime<Utc>>,
    pub actual_value_updated_by: Option<Uuid>,
    pub actual_value_updated_by_device_id: Option<Uuid>,
    pub status_id: Option<i64>,
    pub status_id_updated_at: Option<DateTime<Utc>>,
    pub status_id_updated_by: Option<Uuid>,
    pub status_id_updated_by_device_id: Option<Uuid>,
    pub responsible_team: Option<String>,
    pub responsible_team_updated_at: Option<DateTime<Utc>>,
    pub responsible_team_updated_by: Option<Uuid>,
    pub responsible_team_updated_by_device_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by_user_id: Option<Uuid>,
    pub created_by_device_id: Option<Uuid>,
    pub updated_by_user_id: Option<Uuid>,
    pub updated_by_device_id: Option<Uuid>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub deleted_by_user_id: Option<Uuid>,
    pub deleted_by_device_id: Option<Uuid>,
    pub sync_priority: SyncPriorityFromSyncDomain,
}

impl StrategicGoal {
    // Helper to check if strategic goal is deleted
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }
    
    // Helper to calculate progress percentage
    pub fn progress_percentage(&self) -> Option<f64> {
        if let Some(target) = self.target_value {
            if target > 0.0 {
                return Some((self.actual_value.unwrap_or(0.0) / target) * 100.0);
            }
        }
        None
    }
}

impl DocumentLinkable for StrategicGoal {
    fn field_metadata() -> Vec<EntityFieldMetadata> {
        vec![
            EntityFieldMetadata { field_name: "objective_code", display_name: "Objective Code", supports_documents: false, field_type: FieldType::Text, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "outcome", display_name: "Outcome", supports_documents: true, field_type: FieldType::Text, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "kpi", display_name: "KPI", supports_documents: true, field_type: FieldType::Text, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "target_value", display_name: "Target Value", supports_documents: false, field_type: FieldType::Number, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "actual_value", display_name: "Actual Value", supports_documents: true, field_type: FieldType::Number, is_document_reference_only: false }, // Can link proof
            EntityFieldMetadata { field_name: "responsible_team", display_name: "Responsible Team", supports_documents: false, field_type: FieldType::Text, is_document_reference_only: false },
            // Document Reference Fields from Migration
            EntityFieldMetadata { field_name: "supporting_documentation", display_name: "Supporting Documentation", supports_documents: true, field_type: FieldType::DocumentRef, is_document_reference_only: true },
            EntityFieldMetadata { field_name: "impact_assessment", display_name: "Impact Assessment", supports_documents: true, field_type: FieldType::DocumentRef, is_document_reference_only: true },
            EntityFieldMetadata { field_name: "theory_of_change", display_name: "Theory of Change", supports_documents: true, field_type: FieldType::DocumentRef, is_document_reference_only: true },
            EntityFieldMetadata { field_name: "baseline_data", display_name: "Baseline Data", supports_documents: true, field_type: FieldType::DocumentRef, is_document_reference_only: true },
        ]
    }
}

/// NewStrategicGoal DTO - used when creating a new strategic goal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewStrategicGoal {
    // Added optional ID for cases where we need to pre-assign an ID for document linking
    pub id: Option<Uuid>,
    pub objective_code: String,
    pub outcome: Option<String>,
    pub kpi: Option<String>,
    pub target_value: Option<f64>,
    pub actual_value: Option<f64>,
    pub status_id: Option<i64>,
    pub responsible_team: Option<String>,
    pub sync_priority: SyncPriorityFromSyncDomain,
    pub created_by_user_id: Option<Uuid>,
}

impl Validate for NewStrategicGoal {
    fn validate(&self) -> DomainResult<()> {
        // Validate objective_code (required, min length 2)
        ValidationBuilder::new("objective_code", Some(self.objective_code.clone()))
            .required()
            .min_length(2)
            .max_length(50)
            .validate()?;
            
        // Validate target_value if provided (must be positive)
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

/// UpdateStrategicGoal DTO - used when updating an existing strategic goal
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateStrategicGoal {
    pub objective_code: Option<String>,
    pub outcome: Option<String>,
    pub kpi: Option<String>,
    pub target_value: Option<f64>,
    pub actual_value: Option<f64>,
    pub status_id: Option<i64>,
    pub responsible_team: Option<String>,
    pub sync_priority: Option<SyncPriorityFromSyncDomain>,
    pub updated_by_user_id: Uuid,
}

impl Validate for UpdateStrategicGoal {
    fn validate(&self) -> DomainResult<()> {
        // Validate objective_code if provided
        if let Some(code) = &self.objective_code {
            ValidationBuilder::new("objective_code", Some(code.clone()))
                .min_length(2)
                .max_length(50)
                .validate()?;
        }
        
        // Validate target_value if provided (must be positive)
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

/// StrategicGoalRow - SQLite row representation for mapping from database
#[derive(Debug, Clone, FromRow)]
pub struct StrategicGoalRow {
    pub id: String,
    pub objective_code: String,
    pub objective_code_updated_at: Option<String>,
    pub objective_code_updated_by: Option<String>,
    pub objective_code_updated_by_device_id: Option<String>,
    pub outcome: Option<String>,
    pub outcome_updated_at: Option<String>,
    pub outcome_updated_by: Option<String>,
    pub outcome_updated_by_device_id: Option<String>,
    pub kpi: Option<String>,
    pub kpi_updated_at: Option<String>,
    pub kpi_updated_by: Option<String>,
    pub kpi_updated_by_device_id: Option<String>,
    pub target_value: Option<f64>,
    pub target_value_updated_at: Option<String>,
    pub target_value_updated_by: Option<String>,
    pub target_value_updated_by_device_id: Option<String>,
    pub actual_value: Option<f64>,
    pub actual_value_updated_at: Option<String>,
    pub actual_value_updated_by: Option<String>,
    pub actual_value_updated_by_device_id: Option<String>,
    pub status_id: Option<i64>,
    pub status_id_updated_at: Option<String>,
    pub status_id_updated_by: Option<String>,
    pub status_id_updated_by_device_id: Option<String>,
    pub responsible_team: Option<String>,
    pub responsible_team_updated_at: Option<String>,
    pub responsible_team_updated_by: Option<String>,
    pub responsible_team_updated_by_device_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub created_by_user_id: Option<String>,
    pub created_by_device_id: Option<String>,
    pub updated_by_user_id: Option<String>,
    pub updated_by_device_id: Option<String>,
    pub deleted_at: Option<String>,
    pub deleted_by_user_id: Option<String>,
    pub deleted_by_device_id: Option<String>,
    pub sync_priority: String,
}

impl StrategicGoalRow {
    /// Convert database row to domain entity
    pub fn into_entity(self) -> DomainResult<StrategicGoal> {
        let parse_optional_uuid = |s: &Option<String>, field_name: &str| -> DomainResult<Option<Uuid>> {
            match s {
                Some(id_str) => Uuid::parse_str(id_str)
                    .map(Some)
                    .map_err(|_| DomainError::Validation(ValidationError::format(field_name, &format!("Invalid UUID format: {}", id_str)))),
                None => Ok(None),
            }
        };
        let parse_datetime = |s: &str, field_name: &str| DateTime::parse_from_rfc3339(s).map(|dt| dt.with_timezone(&Utc)).map_err(|_| DomainError::Validation(ValidationError::format(field_name, &format!("Invalid RFC3339 format: {}", s))));
        let parse_optional_datetime = |s: &Option<String>, field_name: &str| -> DomainResult<Option<DateTime<Utc>>> {
            match s {
                Some(dt_str) => DateTime::parse_from_rfc3339(dt_str)
                    .map(|dt| Some(dt.with_timezone(&Utc)))
                    .map_err(|_| DomainError::Validation(ValidationError::format(field_name, &format!("Invalid RFC3339 format: {}", dt_str)))),
                None => Ok(None),
            }
        };

        Ok(StrategicGoal {
            id: Uuid::parse_str(&self.id).map_err(|_| DomainError::Validation(ValidationError::format("id", &format!("Invalid UUID format: {}", self.id))))?,
            objective_code: self.objective_code,
            objective_code_updated_at: parse_optional_datetime(&self.objective_code_updated_at, "objective_code_updated_at")?,
            objective_code_updated_by: parse_optional_uuid(&self.objective_code_updated_by, "objective_code_updated_by")?,
            objective_code_updated_by_device_id: parse_optional_uuid(&self.objective_code_updated_by_device_id, "objective_code_updated_by_device_id")?,
            outcome: self.outcome,
            outcome_updated_at: parse_optional_datetime(&self.outcome_updated_at, "outcome_updated_at")?,
            outcome_updated_by: parse_optional_uuid(&self.outcome_updated_by, "outcome_updated_by")?,
            outcome_updated_by_device_id: parse_optional_uuid(&self.outcome_updated_by_device_id, "outcome_updated_by_device_id")?,
            kpi: self.kpi,
            kpi_updated_at: parse_optional_datetime(&self.kpi_updated_at, "kpi_updated_at")?,
            kpi_updated_by: parse_optional_uuid(&self.kpi_updated_by, "kpi_updated_by")?,
            kpi_updated_by_device_id: parse_optional_uuid(&self.kpi_updated_by_device_id, "kpi_updated_by_device_id")?,
            target_value: self.target_value,
            target_value_updated_at: parse_optional_datetime(&self.target_value_updated_at, "target_value_updated_at")?,
            target_value_updated_by: parse_optional_uuid(&self.target_value_updated_by, "target_value_updated_by")?,
            target_value_updated_by_device_id: parse_optional_uuid(&self.target_value_updated_by_device_id, "target_value_updated_by_device_id")?,
            actual_value: self.actual_value,
            actual_value_updated_at: parse_optional_datetime(&self.actual_value_updated_at, "actual_value_updated_at")?,
            actual_value_updated_by: parse_optional_uuid(&self.actual_value_updated_by, "actual_value_updated_by")?,
            actual_value_updated_by_device_id: parse_optional_uuid(&self.actual_value_updated_by_device_id, "actual_value_updated_by_device_id")?,
            status_id: self.status_id,
            status_id_updated_at: parse_optional_datetime(&self.status_id_updated_at, "status_id_updated_at")?,
            status_id_updated_by: parse_optional_uuid(&self.status_id_updated_by, "status_id_updated_by")?,
            status_id_updated_by_device_id: parse_optional_uuid(&self.status_id_updated_by_device_id, "status_id_updated_by_device_id")?,
            responsible_team: self.responsible_team,
            responsible_team_updated_at: parse_optional_datetime(&self.responsible_team_updated_at, "responsible_team_updated_at")?,
            responsible_team_updated_by: parse_optional_uuid(&self.responsible_team_updated_by, "responsible_team_updated_by")?,
            responsible_team_updated_by_device_id: parse_optional_uuid(&self.responsible_team_updated_by_device_id, "responsible_team_updated_by_device_id")?,
            created_at: parse_datetime(&self.created_at, "created_at")?,
            updated_at: parse_datetime(&self.updated_at, "updated_at")?,
            created_by_user_id: parse_optional_uuid(&self.created_by_user_id, "created_by_user_id")?,
            created_by_device_id: parse_optional_uuid(&self.created_by_device_id, "created_by_device_id")?,
            updated_by_user_id: parse_optional_uuid(&self.updated_by_user_id, "updated_by_user_id")?,
            updated_by_device_id: parse_optional_uuid(&self.updated_by_device_id, "updated_by_device_id")?,
            deleted_at: parse_optional_datetime(&self.deleted_at, "deleted_at")?,
            deleted_by_user_id: parse_optional_uuid(&self.deleted_by_user_id, "deleted_by_user_id")?,
            deleted_by_device_id: parse_optional_uuid(&self.deleted_by_device_id, "deleted_by_device_id")?,
            sync_priority: SyncPriorityFromSyncDomain::from_str(&self.sync_priority).unwrap_or_default(),
        })
    }
}

/// StrategicGoalResponse DTO - used for API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategicGoalResponse {
    pub id: Uuid,
    pub objective_code: String,
    pub outcome: Option<String>,
    pub kpi: Option<String>,
    pub target_value: Option<f64>,
    pub actual_value: Option<f64>,
    pub progress_percentage: Option<f64>,
    pub status_id: Option<i64>,
    pub responsible_team: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub sync_priority: SyncPriorityFromSyncDomain,
    pub created_by_user_id: Option<Uuid>,
    pub updated_by_user_id: Option<Uuid>,
    pub last_synced_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_by_username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_by_username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documents: Option<Vec<MediaDocumentResponse>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_upload_errors: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub projects: Option<Vec<ProjectSummary>>,
}

impl From<StrategicGoal> for StrategicGoalResponse {
    fn from(goal: StrategicGoal) -> Self {
        let progress = goal.progress_percentage();
        Self {
            id: goal.id,
            objective_code: goal.objective_code,
            outcome: goal.outcome,
            kpi: goal.kpi,
            target_value: goal.target_value,
            actual_value: goal.actual_value,
            progress_percentage: progress,
            status_id: goal.status_id,
            responsible_team: goal.responsible_team,
            created_at: goal.created_at.to_rfc3339(),
            updated_at: goal.updated_at.to_rfc3339(),
            sync_priority: goal.sync_priority,
            created_by_user_id: goal.created_by_user_id,
            updated_by_user_id: goal.updated_by_user_id,
            last_synced_at: None, // TODO: Implement sync tracking
            created_by_username: None, // Will be populated by service enrichment
            updated_by_username: None, // Will be populated by service enrichment
            documents: None,
            document_upload_errors: None,
            project_count: None,
            projects: None,
        }
    }
}

// Optional: Add a new response type for the create_with_documents operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategicGoalWithDocumentsResponse {
    pub strategic_goal: StrategicGoalResponse,
    pub successful_uploads: Vec<MediaDocumentResponse>,
    pub failed_uploads: Vec<String>,
}

impl StrategicGoalWithDocumentsResponse {
    pub fn new(
        goal: StrategicGoalResponse, 
        successful: Vec<MediaDocumentResponse>,
        failed: Vec<String>
    ) -> Self {
        Self {
            strategic_goal: goal,
            successful_uploads: successful,
            failed_uploads: failed,
        }
    }
}

/// Comprehensive filter structure for strategic goals
/// Supports complex AND/OR logic for year, month, status, team, and date range filters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategicGoalFilter {
    /// Status IDs (OR logic within, AND with other filters)
    pub status_ids: Option<Vec<i64>>,
    
    /// Responsible teams (OR logic within, AND with other filters)
    pub responsible_teams: Option<Vec<String>>,
    
    /// Years (OR logic within, AND with other filters)
    pub years: Option<Vec<i32>>,
    
    /// Months (OR logic within, AND with other filters)
    /// When combined with years: (year1 OR year2) AND (month1 OR month2)
    pub months: Option<Vec<i32>>, // 1-12
    
    /// User role filter
    pub user_role: Option<(Uuid, UserGoalRole)>,
    
    /// Sync priority filter
    pub sync_priorities: Option<Vec<SyncPriorityFromSyncDomain>>,
    
    /// Search text (searches in objective_code, outcome, kpi, responsible_team)
    pub search_text: Option<String>,
    
    /// Progress percentage range
    pub progress_range: Option<(f64, f64)>, // (min, max)
    
    /// Value range filters
    pub target_value_range: Option<(f64, f64)>,
    pub actual_value_range: Option<(f64, f64)>,
    
    /// Date range filter (RFC3339 format)
    pub date_range: Option<(String, String)>, // (start, end)
    
    /// Days stale filter (items not updated in X days)
    pub days_stale: Option<u32>,
    
    /// Include only non-deleted items (default: true)
    pub exclude_deleted: Option<bool>,
}

impl Default for StrategicGoalFilter {
    fn default() -> Self {
        Self {
            status_ids: None,
            responsible_teams: None,
            years: None,
            months: None,
            user_role: None,
            sync_priorities: None,
            search_text: None,
            progress_range: None,
            target_value_range: None,
            actual_value_range: None,
            date_range: None,
            days_stale: None,
            exclude_deleted: Some(true),
        }
    }
}

impl StrategicGoalFilter {
    /// Create a filter that matches all items (no restrictions)
    pub fn all() -> Self {
        Self::default()
    }
    
    /// Create a filter for specific status IDs
    pub fn by_status(status_ids: Vec<i64>) -> Self {
        Self {
            status_ids: Some(status_ids),
            ..Default::default()
        }
    }
    
    /// Create a filter for specific years and months
    pub fn by_date_parts(years: Option<Vec<i32>>, months: Option<Vec<i32>>) -> Self {
        Self {
            years,
            months,
            ..Default::default()
        }
    }
    
    /// Check if filter has any constraints
    pub fn is_empty(&self) -> bool {
        self.status_ids.is_none() &&
        self.responsible_teams.is_none() &&
        self.years.is_none() &&
        self.months.is_none() &&
        self.user_role.is_none() &&
        self.sync_priorities.is_none() &&
        self.search_text.is_none() &&
        self.progress_range.is_none() &&
        self.target_value_range.is_none() &&
        self.actual_value_range.is_none() &&
        self.date_range.is_none() &&
        self.days_stale.is_none()
    }
}