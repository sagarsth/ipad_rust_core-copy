use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// Export job statuses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExportStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

/// Row mapped to the `export_jobs` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportJob {
    pub id: Uuid,
    pub requested_by_user_id: Option<Uuid>,
    pub requested_at: DateTime<Utc>,
    pub include_blobs: bool,
    pub status: ExportStatus,
    pub local_path: Option<String>,
    pub total_entities: Option<i64>,
    pub total_bytes: Option<i64>,
    pub error_message: Option<String>,
}

/// High-level request coming from UI / FFI describing what should be exported.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExportRequest {
    pub filters: Vec<EntityFilter>,
    pub include_blobs: bool,
    pub target_path: Option<PathBuf>,
}

/// Summary returned to the caller after `create_export` or `get_export_status`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportSummary {
    pub job: ExportJob,
}

/// Filter wrappers so that the export layer can stay repository-agnostic.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum EntityFilter {
    /// Export all strategic goals. Optionally restrict by `status_id`.
    StrategicGoals { status_id: Option<i64> },
    /// Export strategic goals by specific IDs
    StrategicGoalsByIds { ids: Vec<Uuid> },
    /// Export strategic goals within date range
    StrategicGoalsByDateRange { 
        start_date: DateTime<Utc>, 
        end_date: DateTime<Utc>,
        status_id: Option<i64> 
    },
    /// Export all projects.
    ProjectsAll,
    /// Export projects by specific IDs
    ProjectsByIds { ids: Vec<Uuid> },
    /// Export projects within date range
    ProjectsByDateRange { 
        start_date: DateTime<Utc>, 
        end_date: DateTime<Utc> 
    },
    /// Export all activities.
    ActivitiesAll,
    /// Export activities by specific IDs
    ActivitiesByIds { ids: Vec<Uuid> },
    /// Export activities within date range
    ActivitiesByDateRange { 
        start_date: DateTime<Utc>, 
        end_date: DateTime<Utc> 
    },
    /// Export all donors.
    DonorsAll,
    /// Export donors by specific IDs
    DonorsByIds { ids: Vec<Uuid> },
    /// Export donors within date range
    DonorsByDateRange { 
        start_date: DateTime<Utc>, 
        end_date: DateTime<Utc> 
    },
    /// Export all project funding records.
    FundingAll,
    /// Export funding by specific IDs
    FundingByIds { ids: Vec<Uuid> },
    /// Export funding within date range
    FundingByDateRange { 
        start_date: DateTime<Utc>, 
        end_date: DateTime<Utc> 
    },
    /// Export all livelihoods.
    LivelihoodsAll,
    /// Export livelihoods by specific IDs
    LivelihoodsByIds { ids: Vec<Uuid> },
    /// Export livelihoods within date range
    LivelihoodsByDateRange { 
        start_date: DateTime<Utc>, 
        end_date: DateTime<Utc> 
    },
    /// Export all workshops
    WorkshopsAll { include_participants: bool },
    /// Export workshops by specific IDs
    WorkshopsByIds { ids: Vec<Uuid>, include_participants: bool },
    /// Export workshops within date range
    WorkshopsByDateRange { 
        start_date: DateTime<Utc>, 
        end_date: DateTime<Utc>,
        include_participants: bool 
    },
    /// Export all workshop participants
    WorkshopParticipantsAll,
    /// Export workshop participants by specific IDs
    WorkshopParticipantsByIds { ids: Vec<Uuid> },
    /// Export media docs for a single related entity.
    MediaDocumentsByRelatedEntity { related_table: String, related_id: Uuid },
    /// Export media documents by specific IDs
    MediaDocumentsByIds { ids: Vec<Uuid> },
    /// Export media documents within date range
    MediaDocumentsByDateRange { 
        start_date: DateTime<Utc>, 
        end_date: DateTime<Utc> 
    },
    /// NEW: Export all domains in a unified file with mixed records
    UnifiedAllDomains { 
        include_type_tags: bool 
    },
    /// NEW: Export all domains within date range in a unified file
    UnifiedByDateRange { 
        start_date: DateTime<Utc>, 
        end_date: DateTime<Utc>,
        include_type_tags: bool 
    },
} 