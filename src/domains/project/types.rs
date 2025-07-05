use crate::errors::{DomainError, DomainResult, ValidationError};
use crate::validation::{Validate, ValidationBuilder};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use sqlx::FromRow;
use crate::domains::document::types::MediaDocumentResponse;
use crate::domains::core::document_linking::{DocumentLinkable, EntityFieldMetadata, FieldType};
use std::collections::{HashSet, HashMap};
use std::str::FromStr;
use crate::domains::sync::types::SyncPriority as SyncPriorityFromSyncDomain;

// --- ADDED: Response Types for Dashboard Aggregations ---



/// Project entity - represents a project in the system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: Uuid,
    pub strategic_goal_id: Option<Uuid>,
    pub name: String,
    pub name_updated_at: Option<DateTime<Utc>>,
    pub name_updated_by: Option<Uuid>,
    pub name_updated_by_device_id: Option<Uuid>,
    pub objective: Option<String>,
    pub objective_updated_at: Option<DateTime<Utc>>,
    pub objective_updated_by: Option<Uuid>,
    pub objective_updated_by_device_id: Option<Uuid>,
    pub outcome: Option<String>,
    pub outcome_updated_at: Option<DateTime<Utc>>,
    pub outcome_updated_by: Option<Uuid>,
    pub outcome_updated_by_device_id: Option<Uuid>,
    pub status_id: Option<i64>,
    pub status_id_updated_at: Option<DateTime<Utc>>,
    pub status_id_updated_by: Option<Uuid>,
    pub status_id_updated_by_device_id: Option<Uuid>,
    pub timeline: Option<String>,
    pub timeline_updated_at: Option<DateTime<Utc>>,
    pub timeline_updated_by: Option<Uuid>,
    pub timeline_updated_by_device_id: Option<Uuid>,
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

impl Project {
    // Helper to check if project is deleted
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }
    
    // Helper to get status name (this would typically join with status_types table)
    pub fn status_name(&self) -> &str {
        match self.status_id {
            Some(1) => "On Track",
            Some(2) => "At Risk",
            Some(3) => "Delayed",
            Some(4) => "Completed",
            _ => "Unknown"
        }
    }
}

impl DocumentLinkable for Project {
    fn field_metadata() -> Vec<EntityFieldMetadata> {
        vec![
            EntityFieldMetadata { field_name: "name", display_name: "Project Name", supports_documents: false, field_type: FieldType::Text, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "objective", display_name: "Objective", supports_documents: true, field_type: FieldType::Text, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "outcome", display_name: "Outcome", supports_documents: true, field_type: FieldType::Text, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "timeline", display_name: "Timeline", supports_documents: true, field_type: FieldType::Text, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "responsible_team", display_name: "Responsible Team", supports_documents: false, field_type: FieldType::Text, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "strategic_goal_id", display_name: "Strategic Goal", supports_documents: false, field_type: FieldType::Uuid, is_document_reference_only: false },
            // Document Reference Fields from Migration
            EntityFieldMetadata { field_name: "proposal_document", display_name: "Proposal Document", supports_documents: true, field_type: FieldType::DocumentRef, is_document_reference_only: true },
            EntityFieldMetadata { field_name: "budget_document", display_name: "Budget Document", supports_documents: true, field_type: FieldType::DocumentRef, is_document_reference_only: true },
            EntityFieldMetadata { field_name: "logical_framework", display_name: "Logical Framework", supports_documents: true, field_type: FieldType::DocumentRef, is_document_reference_only: true },
            EntityFieldMetadata { field_name: "final_report", display_name: "Final Report", supports_documents: true, field_type: FieldType::DocumentRef, is_document_reference_only: true },
            EntityFieldMetadata { field_name: "monitoring_plan", display_name: "Monitoring Plan", supports_documents: true, field_type: FieldType::DocumentRef, is_document_reference_only: true },
        ]
    }
}

/// NewProject DTO - used when creating a new project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewProject {
    pub strategic_goal_id: Option<Uuid>,
    pub name: String,
    pub objective: Option<String>,
    pub outcome: Option<String>,
    pub status_id: Option<i64>,
    pub timeline: Option<String>,
    pub responsible_team: Option<String>,
    pub sync_priority: SyncPriorityFromSyncDomain,
    pub created_by_user_id: Option<Uuid>,
}

impl Validate for NewProject {
    fn validate(&self) -> DomainResult<()> {
        // Validate name (required, min length 2)
        ValidationBuilder::new("name", Some(self.name.clone()))
            .required()
            .min_length(2)
            .max_length(100)
            .validate()?;
            
        // Validate strategic_goal_id IF provided (non-nil UUID)
        if let Some(sg_id) = self.strategic_goal_id {
             ValidationBuilder::new("strategic_goal_id", Some(sg_id))
                .not_nil() // Ensures it's not Uuid::nil() if present
                .validate()?;
        }
            
        Ok(())
    }
}

/// UpdateProject DTO - used when updating an existing project
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateProject {
    pub strategic_goal_id: Option<Option<Uuid>>,
    pub name: Option<String>,
    pub objective: Option<String>,
    pub outcome: Option<String>,
    pub status_id: Option<i64>,
    pub timeline: Option<String>,
    pub responsible_team: Option<String>,
    pub sync_priority: Option<SyncPriorityFromSyncDomain>,
    pub updated_by_user_id: Uuid,
}

impl Validate for UpdateProject {
    fn validate(&self) -> DomainResult<()> {
        // Validate name if provided
        if let Some(name) = &self.name {
            ValidationBuilder::new("name", Some(name.clone()))
                .min_length(2)
                .max_length(100)
                .validate()?;
        }
        
        // Validate strategic_goal_id if provided (even if setting to None)
        if let Some(opt_sg_id) = self.strategic_goal_id {
            if let Some(sg_id) = opt_sg_id {
                // If an ID is actually provided, ensure it's not nil
                 ValidationBuilder::new("strategic_goal_id", Some(sg_id))
                    .not_nil()
                    .validate()?;
            }
            // Allow Some(None) - represents setting the field to null
        }
        
        Ok(())
    }
}

/// ProjectRow - SQLite row representation for mapping from database
#[derive(Debug, Clone, FromRow)]
pub struct ProjectRow {
    pub id: String,
    pub strategic_goal_id: Option<String>,
    pub name: String,
    pub name_updated_at: Option<String>,
    pub name_updated_by: Option<String>,
    pub name_updated_by_device_id: Option<String>,
    pub objective: Option<String>,
    pub objective_updated_at: Option<String>,
    pub objective_updated_by: Option<String>,
    pub objective_updated_by_device_id: Option<String>,
    pub outcome: Option<String>,
    pub outcome_updated_at: Option<String>,
    pub outcome_updated_by: Option<String>,
    pub outcome_updated_by_device_id: Option<String>,
    pub status_id: Option<i64>,
    pub status_id_updated_at: Option<String>,
    pub status_id_updated_by: Option<String>,
    pub status_id_updated_by_device_id: Option<String>,
    pub timeline: Option<String>,
    pub timeline_updated_at: Option<String>,
    pub timeline_updated_by: Option<String>,
    pub timeline_updated_by_device_id: Option<String>,
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

impl ProjectRow {
    /// Convert database row to domain entity
    pub fn into_entity(self) -> DomainResult<Project> {
        // Helper to parse Option<String> to Option<Uuid>
        let parse_optional_uuid = |s: &Option<String>, field_name: &str| -> DomainResult<Option<Uuid>> {
            match s {
                Some(id_str) => Uuid::parse_str(id_str)
                    .map(Some)
                    .map_err(|_| DomainError::Validation(ValidationError::format(field_name, &format!("Invalid UUID format: {}", id_str)))),
                None => Ok(None),
            }
        };
        
        // Helper to parse Option<String> to Option<DateTime<Utc>>
        let parse_optional_datetime = |s: &Option<String>, field_name: &str| -> DomainResult<Option<DateTime<Utc>>> {
            match s {
                Some(dt_str) => DateTime::parse_from_rfc3339(dt_str)
                    .map(|dt| Some(dt.with_timezone(&Utc)))
                    .map_err(|_| DomainError::Validation(ValidationError::format(field_name, &format!("Invalid RFC3339 format: {}", dt_str)))),
                None => Ok(None),
            }
        };
        
        Ok(Project {
            id: Uuid::parse_str(&self.id)
                .map_err(|_| DomainError::Validation(ValidationError::format("id", &format!("Invalid UUID format: {}", self.id))))?,
            strategic_goal_id: parse_optional_uuid(&self.strategic_goal_id, "strategic_goal_id")?,
            name: self.name,
            name_updated_at: parse_optional_datetime(&self.name_updated_at, "name_updated_at")?,
            name_updated_by: parse_optional_uuid(&self.name_updated_by, "name_updated_by")?,
            name_updated_by_device_id: parse_optional_uuid(&self.name_updated_by_device_id, "name_updated_by_device_id")?,
            objective: self.objective,
            objective_updated_at: parse_optional_datetime(&self.objective_updated_at, "objective_updated_at")?,
            objective_updated_by: parse_optional_uuid(&self.objective_updated_by, "objective_updated_by")?,
            objective_updated_by_device_id: parse_optional_uuid(&self.objective_updated_by_device_id, "objective_updated_by_device_id")?,
            outcome: self.outcome,
            outcome_updated_at: parse_optional_datetime(&self.outcome_updated_at, "outcome_updated_at")?,
            outcome_updated_by: parse_optional_uuid(&self.outcome_updated_by, "outcome_updated_by")?,
            outcome_updated_by_device_id: parse_optional_uuid(&self.outcome_updated_by_device_id, "outcome_updated_by_device_id")?,
            status_id: self.status_id,
            status_id_updated_at: parse_optional_datetime(&self.status_id_updated_at, "status_id_updated_at")?,
            status_id_updated_by: parse_optional_uuid(&self.status_id_updated_by, "status_id_updated_by")?,
            status_id_updated_by_device_id: parse_optional_uuid(&self.status_id_updated_by_device_id, "status_id_updated_by_device_id")?,
            timeline: self.timeline,
            timeline_updated_at: parse_optional_datetime(&self.timeline_updated_at, "timeline_updated_at")?,
            timeline_updated_by: parse_optional_uuid(&self.timeline_updated_by, "timeline_updated_by")?,
            timeline_updated_by_device_id: parse_optional_uuid(&self.timeline_updated_by_device_id, "timeline_updated_by_device_id")?,
            responsible_team: self.responsible_team,
            responsible_team_updated_at: parse_optional_datetime(&self.responsible_team_updated_at, "responsible_team_updated_at")?,
            responsible_team_updated_by: parse_optional_uuid(&self.responsible_team_updated_by, "responsible_team_updated_by")?,
            responsible_team_updated_by_device_id: parse_optional_uuid(&self.responsible_team_updated_by_device_id, "responsible_team_updated_by_device_id")?,
            created_at: DateTime::parse_from_rfc3339(&self.created_at)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|_| DomainError::Validation(ValidationError::format("created_at", &format!("Invalid RFC3339 format: {}", self.created_at))))?,
            updated_at: DateTime::parse_from_rfc3339(&self.updated_at)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|_| DomainError::Validation(ValidationError::format("updated_at", &format!("Invalid RFC3339 format: {}", self.updated_at))))?,
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

/// Basic project summary for nested responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSummary {
    pub id: Uuid,
    pub name: String,
    pub status_id: Option<i64>,
    pub status_name: String,
    pub responsible_team: Option<String>,
}

impl From<Project> for ProjectSummary {
    fn from(project: Project) -> Self {
        // Clone String fields to avoid partial move
        let status_name = project.status_name().to_string();
        Self {
            id: project.id,
            name: project.name.clone(),
            status_id: project.status_id,
            status_name,
            responsible_team: project.responsible_team.clone(),
        }
    }
}

/// Status information for projects and other entities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusInfo {
    pub id: i64,
    pub value: String,
}

/// Strategic goal summary for project responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategicGoalSummary {
    pub id: Uuid,
    pub objective_code: String,
    pub outcome: Option<String>,
}

/// Expanded ProjectInclude options
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectInclude {
    StrategicGoal,       // Already existing
    Status,              // Already existing
    CreatedBy,           // Already existing
    ActivityCount,       // Already existing
    WorkshopCount,       // Already existing
    Documents,           // Already existing
    DocumentReferences,  // Include document reference fields
    ActivityTimeline,    // Include recent activities
    StatusDetails,       // Include detailed status information
    Counts,              // ADDED: Include all count-based enrichment (activity_count, workshop_count, etc.)
    All,                 // Already existing
}

/// ProjectResponse DTO - used for API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectResponse {
    pub id: Uuid,
    pub name: String,
    pub objective: Option<String>,
    pub outcome: Option<String>,
    pub status_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<StatusInfo>,  // Populated when status details are fetched
    pub timeline: Option<String>,
    pub responsible_team: Option<String>,
    // --- ADDED: Strategic goal ID for enrichment logic ---
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strategic_goal_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strategic_goal: Option<StrategicGoalSummary>, // Optional fetched relation
    pub created_at: String,
    pub updated_at: String,
    // --- ADDED: User ID fields following strategic_goal pattern ---
    pub created_by_user_id: Option<Uuid>,
    pub updated_by_user_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_by: Option<String>, // Username, fetched from users table
    // --- ADDED: Updated by username for enrichment ---
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_by_username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_by_username: Option<String>,
    // --- ADDED: Strategic goal name for enrichment ---
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strategic_goal_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activity_count: Option<i64>, // Count of related activities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workshop_count: Option<i64>, // Count of related workshops
    // --- ADDED: Document count for enrichment ---
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_count: Option<i64>,
    pub sync_priority: SyncPriorityFromSyncDomain,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documents: Option<Vec<MediaDocumentResponse>>,
}

impl ProjectResponse {
    /// Create a base ProjectResponse from a Project entity
    pub fn from_project(project: Project) -> Self {
        Self {
            id: project.id,
            name: project.name.clone(),
            objective: project.objective.clone(),
            outcome: project.outcome.clone(),
            status_id: project.status_id,
            status: None,
            timeline: project.timeline.clone(),
            responsible_team: project.responsible_team.clone(),
            // --- ADDED: Include strategic goal ID from entity ---
            strategic_goal_id: project.strategic_goal_id,
            strategic_goal: None,
            created_at: project.created_at.to_rfc3339(),
            updated_at: project.updated_at.to_rfc3339(),
            // --- ADDED: Include user ID fields from entity ---
            created_by_user_id: project.created_by_user_id,
            updated_by_user_id: project.updated_by_user_id,
            created_by: None,
            // --- ADDED: Initialize enrichment fields to None ---
            created_by_username: None,
            updated_by_username: None,
            strategic_goal_name: None,
            activity_count: None,
            workshop_count: None,
            document_count: None,
            sync_priority: project.sync_priority,
            documents: None,
        }
    }
    
    /// Set strategic goal information
    pub fn with_strategic_goal(mut self, goal: StrategicGoalSummary) -> Self {
        self.strategic_goal = Some(goal);
        self
    }
    
    /// Set status information
    pub fn with_status(mut self, status: StatusInfo) -> Self {
        self.status = Some(status);
        self
    }
    
    /// Set created by username
    pub fn with_created_by(mut self, username: String) -> Self {
        self.created_by = Some(username);
        self
    }
    
    /// Set activity count
    pub fn with_activity_count(mut self, count: i64) -> Self {
        self.activity_count = Some(count);
        self
    }
    
    /// Set workshop count
    pub fn with_workshop_count(mut self, count: i64) -> Self {
        self.workshop_count = Some(count);
        self
    }
    
    /// Add documents to the response
    pub fn with_documents(mut self, documents: Vec<MediaDocumentResponse>) -> Self {
        self.documents = Some(documents);
        self
    }
}

/// Project statistics for dashboard views
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectStatistics {
    pub total_projects: i64,
    pub by_status: HashMap<String, i64>,
    pub by_strategic_goal: HashMap<String, i64>,
    pub by_responsible_team: HashMap<String, i64>,
    pub document_count: i64,
}

/// Project status breakdown for reporting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectStatusBreakdown {
    pub status_id: i64,
    pub status_name: String,
    pub count: i64,
    pub percentage: f64,
}

/// Project with document timeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectWithDocumentTimeline {
    pub project: ProjectResponse,
    pub documents_by_type: HashMap<String, Vec<MediaDocumentResponse>>,
    pub total_document_count: u64,
}

/// Project metadata counts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMetadataCounts {
    pub projects_by_team: HashMap<String, i64>,
    pub projects_by_status: HashMap<String, i64>,
    pub projects_by_goal: HashMap<String, i64>,
}

/// Extended status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtendedStatusInfo {
    pub id: i64,
    pub value: String,
    pub color_code: String,
    pub description: Option<String>,
    pub sort_order: i64,
}

/// Project activity timeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectActivitySummary {
    pub project_id: Uuid,
    pub project_name: String,
    pub activities: Vec<ProjectActivity>,
    pub last_updated: DateTime<Utc>,
}

/// Single project activity entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectActivity {
    pub timestamp: DateTime<Utc>,
    pub user_id: Uuid,
    pub username: Option<String>,
    pub action_type: String,
    pub field_name: Option<String>,
    pub description: String,
}

/// Document reference summary for a project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectDocumentReference {
    pub field_name: String,
    pub display_name: String,
    pub document_id: Option<Uuid>,
    pub filename: Option<String>,
    pub upload_date: Option<DateTime<Utc>>,
    pub file_size: Option<u64>,
}

/// ADDED: A comprehensive filter for searching projects with multiple criteria,
/// allowing for intuitive, multi-faceted filtering from the UI.
/// This follows the same pattern as StrategicGoalFilter for consistency.
#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, Default)]
pub struct ProjectFilter {
    /// Filter by one or more statuses (e.g., "At Risk", "Delayed").
    /// Uses OR logic internally: status_id = 1 OR status_id = 2.
    pub status_ids: Option<Vec<i64>>,
    
    /// Filter by one or more parent strategic goals.
    /// Uses OR logic internally.
    pub strategic_goal_ids: Option<Vec<Uuid>>,
    
    /// Filter by one or more responsible teams.
    /// Uses OR logic internally.
    pub responsible_teams: Option<Vec<String>>,
    
    /// A free-text search term to apply to name, objective, outcome, etc.
    pub search_text: Option<String>,
    
    /// Filter for projects updated within a specific date range.
    pub date_range: Option<(String, String)>, // (start_rfc3339, end_rfc3339)
    
    /// Whether to exclude soft-deleted records. Defaults to true.
    pub exclude_deleted: Option<bool>,
}
 
/// ADDED: Document coverage analysis response for dashboard widgets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentCoverageAnalysis {
    pub projects_with_documents: i64,
    pub projects_without_documents: i64,
    pub total_projects: i64,
    pub average_documents_per_project: f64,
    pub coverage_percentage: f64,
    pub document_count_by_type: HashMap<String, i64>,
}

/// ADDED: Project activity timeline response for dashboard widgets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectActivityTimeline {
    pub active_projects: i64,
    pub inactive_projects: i64,
    pub total_projects: i64,
    pub activity_percentage: f64,
    pub stale_projects: i64,
    pub recently_updated_projects: i64,
}