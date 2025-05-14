use crate::errors::{DomainError, DomainResult};
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
    pub outcome: Option<String>,
    pub outcome_updated_at: Option<DateTime<Utc>>,
    pub outcome_updated_by: Option<Uuid>,
    pub kpi: Option<String>,
    pub kpi_updated_at: Option<DateTime<Utc>>,
    pub kpi_updated_by: Option<Uuid>,
    pub target_value: Option<f64>,
    pub target_value_updated_at: Option<DateTime<Utc>>,
    pub target_value_updated_by: Option<Uuid>,
    pub actual_value: Option<f64>,
    pub actual_value_updated_at: Option<DateTime<Utc>>,
    pub actual_value_updated_by: Option<Uuid>,
    pub status_id: Option<i64>,
    pub status_id_updated_at: Option<DateTime<Utc>>,
    pub status_id_updated_by: Option<Uuid>,
    pub responsible_team: Option<String>,
    pub responsible_team_updated_at: Option<DateTime<Utc>>,
    pub responsible_team_updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by_user_id: Option<Uuid>,
    pub updated_by_user_id: Option<Uuid>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub deleted_by_user_id: Option<Uuid>,
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
    pub outcome: Option<String>,
    pub outcome_updated_at: Option<String>,
    pub outcome_updated_by: Option<String>,
    pub kpi: Option<String>,
    pub kpi_updated_at: Option<String>,
    pub kpi_updated_by: Option<String>,
    pub target_value: Option<f64>,
    pub target_value_updated_at: Option<String>,
    pub target_value_updated_by: Option<String>,
    pub actual_value: Option<f64>,
    pub actual_value_updated_at: Option<String>,
    pub actual_value_updated_by: Option<String>,
    pub status_id: Option<i64>,
    pub status_id_updated_at: Option<String>,
    pub status_id_updated_by: Option<String>,
    pub responsible_team: Option<String>,
    pub responsible_team_updated_at: Option<String>,
    pub responsible_team_updated_by: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub created_by_user_id: Option<String>,
    pub updated_by_user_id: Option<String>,
    pub deleted_at: Option<String>,
    pub deleted_by_user_id: Option<String>,
    pub sync_priority: String,
}

impl StrategicGoalRow {
    /// Convert database row to domain entity
    pub fn into_entity(self) -> DomainResult<StrategicGoal> {
        let parse_uuid = |s: &Option<String>| -> Option<DomainResult<Uuid>> {
            s.as_ref().map(|id| {
                Uuid::parse_str(id).map_err(|_| DomainError::InvalidUuid(id.clone()))
            })
        };
        
        let parse_datetime = |s: &Option<String>| -> Option<DomainResult<DateTime<Utc>>> {
            s.as_ref().map(|dt| {
                DateTime::parse_from_rfc3339(dt)
                    .map(|dt| dt.with_timezone(&Utc))
                    .map_err(|_| DomainError::Internal(format!("Invalid date format: {}", dt)))
            })
        };
        
        Ok(StrategicGoal {
            id: Uuid::parse_str(&self.id)
                .map_err(|_| DomainError::InvalidUuid(self.id))?,
            objective_code: self.objective_code,
            objective_code_updated_at: parse_datetime(&self.objective_code_updated_at)
                .transpose()?,
            objective_code_updated_by: parse_uuid(&self.objective_code_updated_by)
                .transpose()?,
            outcome: self.outcome,
            outcome_updated_at: parse_datetime(&self.outcome_updated_at)
                .transpose()?,
            outcome_updated_by: parse_uuid(&self.outcome_updated_by)
                .transpose()?,
            kpi: self.kpi,
            kpi_updated_at: parse_datetime(&self.kpi_updated_at)
                .transpose()?,
            kpi_updated_by: parse_uuid(&self.kpi_updated_by)
                .transpose()?,
            target_value: self.target_value,
            target_value_updated_at: parse_datetime(&self.target_value_updated_at)
                .transpose()?,
            target_value_updated_by: parse_uuid(&self.target_value_updated_by)
                .transpose()?,
            actual_value: self.actual_value,
            actual_value_updated_at: parse_datetime(&self.actual_value_updated_at)
                .transpose()?,
            actual_value_updated_by: parse_uuid(&self.actual_value_updated_by)
                .transpose()?,
            status_id: self.status_id,
            status_id_updated_at: parse_datetime(&self.status_id_updated_at)
                .transpose()?,
            status_id_updated_by: parse_uuid(&self.status_id_updated_by)
                .transpose()?,
            responsible_team: self.responsible_team,
            responsible_team_updated_at: parse_datetime(&self.responsible_team_updated_at)
                .transpose()?,
            responsible_team_updated_by: parse_uuid(&self.responsible_team_updated_by)
                .transpose()?,
            created_at: DateTime::parse_from_rfc3339(&self.created_at)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|_| DomainError::Internal(format!("Invalid date format: {}", self.created_at)))?,
            updated_at: DateTime::parse_from_rfc3339(&self.updated_at)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|_| DomainError::Internal(format!("Invalid date format: {}", self.updated_at)))?,
            created_by_user_id: parse_uuid(&self.created_by_user_id)
                .transpose()?,
            updated_by_user_id: parse_uuid(&self.updated_by_user_id)
                .transpose()?,
            deleted_at: parse_datetime(&self.deleted_at)
                .transpose()?,
            deleted_by_user_id: parse_uuid(&self.deleted_by_user_id)
                .transpose()?,
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