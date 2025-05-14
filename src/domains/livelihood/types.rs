use crate::errors::{DomainError, DomainResult};
use crate::validation::{Validate, ValidationBuilder};
use uuid::Uuid;
use chrono::{DateTime, Utc, NaiveDate};
use serde::{Serialize, Deserialize};
use sqlx::FromRow;
use crate::domains::document::types::MediaDocumentResponse;
use crate::domains::core::document_linking::{DocumentLinkable, EntityFieldMetadata, FieldType};
use std::collections::HashSet;
use std::collections::HashMap;
use crate::types::SyncPriority;
use std::str::FromStr;
use crate::domains::sync::types::SyncPriority as SyncPriorityFromSyncDomain;

/// Livelihood entity - represents a livelihood grant for a participant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Livelihood {
    pub id: Uuid,
    pub participant_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    
    pub type_: String, // Renamed from 'type' to avoid keyword clash
    pub type_updated_at: Option<DateTime<Utc>>,
    pub type_updated_by: Option<Uuid>,
    
    pub description: Option<String>,
    pub description_updated_at: Option<DateTime<Utc>>,
    pub description_updated_by: Option<Uuid>,
    
    pub status_id: Option<i64>, // Assuming status_id refers to an integer key for a status_types table
    pub status_id_updated_at: Option<DateTime<Utc>>,
    pub status_id_updated_by: Option<Uuid>,

    pub initial_grant_date: Option<String>, // ISO date format YYYY-MM-DD
    pub initial_grant_date_updated_at: Option<DateTime<Utc>>,
    pub initial_grant_date_updated_by: Option<Uuid>,

    pub initial_grant_amount: Option<f64>,
    pub initial_grant_amount_updated_at: Option<DateTime<Utc>>,
    pub initial_grant_amount_updated_by: Option<Uuid>,

    pub sync_priority: SyncPriorityFromSyncDomain,
    
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by_user_id: Option<Uuid>,
    pub updated_by_user_id: Option<Uuid>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub deleted_by_user_id: Option<Uuid>,
}

impl Livelihood {
    // Helper to check if livelihood is deleted
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }
    
    // Helper to calculate total grant amount (including subsequents)
    pub fn total_grant_amount(&self, subsequent_grants: &[SubsequentGrant]) -> f64 {
        let initial = self.initial_grant_amount.unwrap_or(0.0);
        let subsequent: f64 = subsequent_grants.iter()
            .filter(|grant| !grant.is_deleted())
            .filter_map(|grant| grant.amount)
            .sum();
        
        initial + subsequent
    }

    pub fn parsed_initial_grant_date(&self) -> Option<NaiveDate> {
        self.initial_grant_date.as_ref().and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
    }
}

impl DocumentLinkable for Livelihood {
    fn field_metadata() -> Vec<EntityFieldMetadata> {
        vec![
            EntityFieldMetadata { field_name: "type", display_name: "Type", supports_documents: false, field_type: FieldType::Text, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "description", display_name: "Description", supports_documents: true, field_type: FieldType::Text, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "initial_grant_amount", display_name: "Initial Grant Amount", supports_documents: true, field_type: FieldType::Number, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "initial_grant_date", display_name: "Initial Grant Date", supports_documents: false, field_type: FieldType::Date, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "participant_id", display_name: "Participant", supports_documents: false, field_type: FieldType::Uuid, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "project_id", display_name: "Project", supports_documents: false, field_type: FieldType::Uuid, is_document_reference_only: false },
            // Document Reference Fields from Migration
            EntityFieldMetadata { field_name: "business_plan", display_name: "Business Plan", supports_documents: true, field_type: FieldType::DocumentRef, is_document_reference_only: true },
            EntityFieldMetadata { field_name: "grant_agreement", display_name: "Grant Agreement", supports_documents: true, field_type: FieldType::DocumentRef, is_document_reference_only: true },
            EntityFieldMetadata { field_name: "receipts", display_name: "Receipts", supports_documents: true, field_type: FieldType::DocumentRef, is_document_reference_only: true }, // This might represent multiple linked docs
            EntityFieldMetadata { field_name: "progress_photos", display_name: "Progress Photos", supports_documents: true, field_type: FieldType::DocumentRef, is_document_reference_only: true }, // This might represent multiple linked docs
            EntityFieldMetadata { field_name: "case_study", display_name: "Case Study", supports_documents: true, field_type: FieldType::DocumentRef, is_document_reference_only: true },
        ]
    }
}

/// SubsequentGrant entity - represents additional grants for a livelihood
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubsequentGrant {
    pub id: Uuid,
    pub livelihood_id: Uuid,
    pub amount: Option<f64>,
    pub amount_updated_at: Option<DateTime<Utc>>,
    pub amount_updated_by: Option<Uuid>,
    pub purpose: Option<String>,
    pub purpose_updated_at: Option<DateTime<Utc>>,
    pub purpose_updated_by: Option<Uuid>,
    pub grant_date: Option<String>, // ISO date format YYYY-MM-DD
    pub grant_date_updated_at: Option<DateTime<Utc>>,
    pub grant_date_updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by_user_id: Option<Uuid>,
    pub updated_by_user_id: Option<Uuid>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub deleted_by_user_id: Option<Uuid>,
}

impl SubsequentGrant {
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }
    pub fn parsed_grant_date(&self) -> Option<NaiveDate> {
        self.grant_date.as_ref().and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
    }
}

impl DocumentLinkable for SubsequentGrant {
     fn field_metadata() -> Vec<EntityFieldMetadata> {
        vec![
            EntityFieldMetadata { field_name: "amount", display_name: "Grant Amount", supports_documents: true, field_type: FieldType::Number, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "purpose", display_name: "Purpose", supports_documents: true, field_type: FieldType::Text, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "grant_date", display_name: "Grant Date", supports_documents: false, field_type: FieldType::Date, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "livelihood_id", display_name: "Livelihood", supports_documents: false, field_type: FieldType::Uuid, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "grant_application", display_name: "Grant Application", supports_documents: true, field_type: FieldType::DocumentRef, is_document_reference_only: true },
            EntityFieldMetadata { field_name: "grant_report", display_name: "Grant Report", supports_documents: true, field_type: FieldType::DocumentRef, is_document_reference_only: true },
            EntityFieldMetadata { field_name: "receipts", display_name: "Receipts", supports_documents: true, field_type: FieldType::DocumentRef, is_document_reference_only: true },
        ]
    }
}

/// NewLivelihood DTO - used when creating a new livelihood
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewLivelihood {
    pub id: Option<Uuid>, // Added for pre-allocation if needed
    pub participant_id: Option<Uuid>, // Made nullable in schema
    pub project_id: Option<Uuid>,     // Made nullable in schema
    pub type_: String, // Renamed from 'type'
    pub description: Option<String>,
    pub status_id: Option<i64>,
    pub initial_grant_date: Option<String>, // YYYY-MM-DD
    pub initial_grant_amount: Option<f64>,
    pub sync_priority: SyncPriorityFromSyncDomain,
    pub created_by_user_id: Option<Uuid>, // For explicit setting if needed
}

impl Validate for NewLivelihood {
    fn validate(&self) -> DomainResult<()> {
        ValidationBuilder::new("type_", Some(self.type_.clone()))
            .required()
            .min_length(1) // Basic validation for type
            .validate()?;

        if let Some(date) = &self.initial_grant_date {
            if NaiveDate::parse_from_str(date, "%Y-%m-%d").is_err() {
                return Err(DomainError::Validation(
                    crate::errors::ValidationError::format(
                        "initial_grant_date", 
                        "Invalid date format. Expected YYYY-MM-DD"
                    )
                ));
            }
        }
        if let Some(amount) = self.initial_grant_amount {
            ValidationBuilder::new("initial_grant_amount", Some(amount))
                .min(0.0)
                .validate()?;
        }
        Ok(())
    }
}

/// UpdateLivelihood DTO - used when updating an existing livelihood
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateLivelihood {
    pub participant_id: Option<Option<Uuid>>, // Allow setting to NULL
    pub project_id: Option<Option<Uuid>>,     // Allow setting to NULL
    pub type_: Option<String>, // Renamed from 'type'
    pub description: Option<Option<String>>, // Allow setting to NULL
    pub status_id: Option<Option<i64>>,     // Allow setting to NULL
    pub initial_grant_date: Option<Option<String>>, // YYYY-MM-DD, allow setting to NULL
    pub initial_grant_amount: Option<Option<f64>>, // Allow setting to NULL
    pub sync_priority: Option<SyncPriorityFromSyncDomain>,
    pub updated_by_user_id: Option<Uuid>, // Keep Option for system updates, service layer ensures it for user ops
}

impl Validate for UpdateLivelihood {
    fn validate(&self) -> DomainResult<()> {
        if let Some(type_val) = &self.type_ {
             ValidationBuilder::new("type_", Some(type_val.clone()))
                .min_length(1)
                .validate()?;
        }
        if let Some(date_opt) = &self.initial_grant_date {
            if let Some(date_str) = date_opt {
                if NaiveDate::parse_from_str(date_str, "%Y-%m-%d").is_err() {
                    return Err(DomainError::Validation(
                        crate::errors::ValidationError::format(
                            "initial_grant_date", 
                            "Invalid date format. Expected YYYY-MM-DD"
                        )
                    ));
                }
            }
        }
        if let Some(amount_opt) = &self.initial_grant_amount {
            if let Some(amount) = amount_opt {
                 ValidationBuilder::new("initial_grant_amount", Some(*amount))
                    .min(0.0)
                    .validate()?;
            }
        }
        Ok(())
    }
}

/// LivelihoodRow - SQLite row representation for mapping from database
#[derive(Debug, Clone, FromRow)]
pub struct LivelihoodRow {
    pub id: String,
    pub participant_id: Option<String>,
    pub project_id: Option<String>,
    
    pub type_: String, // Renamed from 'type'
    pub type_updated_at: Option<String>,
    pub type_updated_by: Option<String>,
    
    pub description: Option<String>,
    pub description_updated_at: Option<String>,
    pub description_updated_by: Option<String>,
    
    pub status_id: Option<i64>,
    pub status_id_updated_at: Option<String>,
    pub status_id_updated_by: Option<String>,

    pub initial_grant_date: Option<String>,
    pub initial_grant_date_updated_at: Option<String>,
    pub initial_grant_date_updated_by: Option<String>,

    pub initial_grant_amount: Option<f64>,
    pub initial_grant_amount_updated_at: Option<String>,
    pub initial_grant_amount_updated_by: Option<String>,

    pub sync_priority: String, // Will be parsed to SyncPriorityFromSyncDomain
    
    pub created_at: String,
    pub updated_at: String,
    pub created_by_user_id: Option<String>,
    pub updated_by_user_id: Option<String>,
    pub deleted_at: Option<String>,
    pub deleted_by_user_id: Option<String>,
}

impl LivelihoodRow {
    /// Convert database row to domain entity
    pub fn into_entity(self) -> DomainResult<Livelihood> {
        let parse_uuid = |s: &Option<String>| -> Option<DomainResult<Uuid>> {
            s.as_ref().map(|id| {
                Uuid::parse_str(id).map_err(|_| DomainError::InvalidUuid(id.clone()))
            })
        };
        
        let parse_datetime = |s: &Option<String>| -> Option<DomainResult<DateTime<Utc>>> {
            s.as_ref().map(|dt| {
                DateTime::parse_from_rfc3339(dt)
                    .map(|dt_with_tz| dt_with_tz.with_timezone(&Utc))
                    .map_err(|e| DomainError::Internal(format!("Invalid date format: {} ({})", dt, e)))
            })
        };
        
        Ok(Livelihood {
            id: Uuid::parse_str(&self.id)
                .map_err(|_| DomainError::InvalidUuid(self.id.clone()))?,
            participant_id: parse_uuid(&self.participant_id).transpose()?,
            project_id: parse_uuid(&self.project_id).transpose()?,
            
            type_: self.type_,
            type_updated_at: parse_datetime(&self.type_updated_at).transpose()?,
            type_updated_by: parse_uuid(&self.type_updated_by).transpose()?,
            
            description: self.description,
            description_updated_at: parse_datetime(&self.description_updated_at).transpose()?,
            description_updated_by: parse_uuid(&self.description_updated_by).transpose()?,
            
            status_id: self.status_id,
            status_id_updated_at: parse_datetime(&self.status_id_updated_at).transpose()?,
            status_id_updated_by: parse_uuid(&self.status_id_updated_by).transpose()?,

            initial_grant_date: self.initial_grant_date,
            initial_grant_date_updated_at: parse_datetime(&self.initial_grant_date_updated_at).transpose()?,
            initial_grant_date_updated_by: parse_uuid(&self.initial_grant_date_updated_by).transpose()?,

            initial_grant_amount: self.initial_grant_amount,
            initial_grant_amount_updated_at: parse_datetime(&self.initial_grant_amount_updated_at).transpose()?,
            initial_grant_amount_updated_by: parse_uuid(&self.initial_grant_amount_updated_by).transpose()?,

            sync_priority: SyncPriorityFromSyncDomain::from_str(&self.sync_priority)
                .map_err(|e| DomainError::Internal(format!("Failed to parse sync_priority: {}", e)))?,
            
            created_at: DateTime::parse_from_rfc3339(&self.created_at)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|_| DomainError::Internal(format!("Invalid created_at date format: {}", self.created_at)))?,
            updated_at: DateTime::parse_from_rfc3339(&self.updated_at)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|_| DomainError::Internal(format!("Invalid updated_at date format: {}", self.updated_at)))?,
            created_by_user_id: parse_uuid(&self.created_by_user_id).transpose()?,
            updated_by_user_id: parse_uuid(&self.updated_by_user_id).transpose()?,
            deleted_at: parse_datetime(&self.deleted_at).transpose()?,
            deleted_by_user_id: parse_uuid(&self.deleted_by_user_id).transpose()?,
        })
    }
}

/// ProjectSummary for livelihood responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSummary {
    pub id: Uuid,
    pub name: String,
}

/// ParticipantSummary for livelihood responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantSummary {
    pub id: Uuid,
    pub name: String,
    pub gender: Option<String>,
    pub age_group: Option<String>,
    pub disability: bool,
}

/// SubsequentGrantSummary for livelihood responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubsequentGrantSummary {
    pub id: Uuid,
    pub amount: Option<f64>,
    pub purpose: Option<String>,
    pub grant_date: Option<String>,
}

impl From<SubsequentGrant> for SubsequentGrantSummary {
    fn from(grant: SubsequentGrant) -> Self {
        Self {
            id: grant.id,
            amount: grant.amount,
            purpose: grant.purpose,
            grant_date: grant.grant_date,
        }
    }
}

/// LivelihoodResponse DTO - used for API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LivelihoodResponse {
    pub id: Uuid,
    pub participant_id: Option<Uuid>,
    pub participant: Option<ParticipantSummary>,
    pub project_id: Option<Uuid>,
    pub project: Option<ProjectSummary>,
    pub type_: String,
    pub description: Option<String>,
    pub status_id: Option<i64>,
    pub initial_grant_date: Option<String>,
    pub initial_grant_amount: Option<f64>,
    pub sync_priority: SyncPriorityFromSyncDomain,
    pub created_at: String,
    pub updated_at: String,
    pub subsequent_grants: Option<Vec<SubsequentGrantSummary>>,
    pub total_grant_amount: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documents: Option<Vec<MediaDocumentResponse>>,
}

impl From<Livelihood> for LivelihoodResponse {
    fn from(livelihood: Livelihood) -> Self {
        Self {
            id: livelihood.id,
            participant_id: livelihood.participant_id,
            participant: None,
            project_id: livelihood.project_id,
            project: None,
            type_: livelihood.type_,
            description: livelihood.description,
            status_id: livelihood.status_id,
            initial_grant_date: livelihood.initial_grant_date,
            initial_grant_amount: livelihood.initial_grant_amount,
            sync_priority: livelihood.sync_priority,
            created_at: livelihood.created_at.to_rfc3339(),
            updated_at: livelihood.updated_at.to_rfc3339(),
            subsequent_grants: None,
            total_grant_amount: livelihood.initial_grant_amount,
            documents: None,
        }
    }
}

impl LivelihoodResponse {
    /// Add participant details
    pub fn with_participant(mut self, participant: ParticipantSummary) -> Self {
        self.participant = Some(participant);
        self
    }
    
    /// Add project details
    pub fn with_project(mut self, project: ProjectSummary) -> Self {
        self.project = Some(project);
        self
    }
    
    /// Add subsequent grants
    pub fn with_subsequent_grants(mut self, grants: Vec<SubsequentGrantSummary>) -> Self {
        let total = self.initial_grant_amount.unwrap_or(0.0) + 
            grants.iter().filter_map(|g| g.amount).sum::<f64>();
        
        self.subsequent_grants = Some(grants);
        self.total_grant_amount = Some(total);
        self
    }
}

/// Enum to specify included relations when fetching livelihoods
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LivelihoodInclude {
    Project,
    Participant,
    ParticipantDetails,
    SubsequentGrants,
    Documents,
    DocumentCounts,
    OutcomeMetrics,
    All,
}

/// SubsequentGrantResponse DTO - used for API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubsequentGrantResponse {
    pub id: Uuid,
    pub livelihood_id: Uuid,
    pub amount: Option<f64>,
    pub purpose: Option<String>,
    pub grant_date: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub livelihood: Option<LivelihoodSummary>,
}

/// LivelihoodSummary for subsequent grant responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LivelihoodSummary {
    pub id: Uuid,
    pub type_: String,
    pub description: Option<String>,
    pub initial_grant_amount: Option<f64>,
}

impl From<SubsequentGrant> for SubsequentGrantResponse {
    fn from(grant: SubsequentGrant) -> Self {
        Self {
            id: grant.id,
            livelihood_id: grant.livelihood_id,
            amount: grant.amount,
            purpose: grant.purpose,
            grant_date: grant.grant_date,
            created_at: grant.created_at.to_rfc3339(),
            updated_at: grant.updated_at.to_rfc3339(),
            livelihood: None,
        }
    }
}

impl SubsequentGrantResponse {
    /// Add livelihood details
    pub fn with_livelihood(mut self, livelihood: LivelihoodSummary) -> Self {
        self.livelihood = Some(livelihood);
        self
    }
}

/// Livelihood statistics summary for reports and dashboards
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LivelioodStatsSummary {
    pub total_livelihoods: i64,
    pub active_livelihoods: i64,
    pub total_initial_grant_amount: f64,
    pub average_initial_grant_amount: f64,
    pub total_subsequent_grants: i64,
    pub total_subsequent_grant_amount: f64,
    pub livelihoods_by_project: HashMap<Uuid, i64>,
    pub initial_grant_amounts_by_project: HashMap<Uuid, f64>,
    pub livelihoods_by_type: HashMap<String, i64>,
}

/// Livelihood with full participant details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LivelioodWithParticipantDetails {
    pub livelihood: LivelihoodResponse,
    pub participant_details: ParticipantDetails,
    pub subsequent_grants: Vec<SubsequentGrantSummary>,
    pub total_grant_amount: f64,
    pub documents_count: i64,
}

/// Extended participant details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantDetails {
    pub id: Uuid,
    pub name: String,
    pub gender: Option<String>,
    pub age_group: Option<String>,
    pub disability: bool,
    pub address: Option<String>,
    pub phone: Option<String>,
    pub occupation: Option<String>,
    pub family_size: Option<i32>,
    pub created_at: String,
}

/// Outcome tracking status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum OutcomeStatus {
    NotStarted,
    InProgress,
    Completed,
    Discontinued,
}

/// Extended livelihood summary with outcome tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LivelioodOutcomeSummary {
    pub id: Uuid,
    pub participant_name: String,
    pub project_name: Option<String>,
    pub grant_amount: Option<f64>,
    pub total_grant_amount: f64,
    pub purpose: Option<String>,
    pub outcome: Option<String>,
    pub outcome_status: OutcomeStatus,
    pub has_progress_photos: bool,
    pub last_updated: String,
}

/// Livelihood dashboard metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LivelihoodDashboardMetrics {
    pub total_participants_supported: i64,
    pub total_grant_amount: f64,
    pub grant_count_by_month: HashMap<String, i64>,
    pub grant_amount_by_month: HashMap<String, f64>,
    pub outcome_status_distribution: HashMap<String, i64>,
    pub gender_distribution: HashMap<String, i64>,
    pub age_group_distribution: HashMap<String, i64>,
}

/// Participant outcome metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantOutcomeMetrics {
    pub participant_id: Uuid,
    pub participant_name: String,
    pub gender: Option<String>,
    pub total_grants_received: i64,
    pub total_grant_amount: f64,
    pub first_grant_date: Option<String>,
    pub last_grant_date: Option<String>,
    pub has_positive_outcome: bool,
    pub outcome_description: Option<String>,
}

/// Livelihood with document timeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LivelioodWithDocumentTimeline {
    pub livelihood: LivelihoodResponse,
    pub documents_by_month: HashMap<String, Vec<MediaDocumentResponse>>,
    pub total_document_count: u64,
}

/// NewSubsequentGrant DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewSubsequentGrant {
    pub livelihood_id: Uuid,
    pub amount: Option<f64>,
    pub purpose: Option<String>,
    pub grant_date: Option<String>,
    pub created_by_user_id: Option<Uuid>,
}

impl Validate for NewSubsequentGrant {
    fn validate(&self) -> DomainResult<()> {
        ValidationBuilder::new("livelihood_id", Some(self.livelihood_id)).not_nil().validate()?;
        if let Some(amount) = self.amount {
            ValidationBuilder::new("amount", Some(amount)).min(0.0).validate()?;
        }
        if let Some(date) = &self.grant_date {
            if NaiveDate::parse_from_str(date, "%Y-%m-%d").is_err() {
                return Err(DomainError::Validation(crate::errors::ValidationError::format("grant_date", "Invalid date format. Expected YYYY-MM-DD")));
            }
        }
        Ok(())
    }
}

/// UpdateSubsequentGrant DTO
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateSubsequentGrant {
    pub amount: Option<f64>,
    pub purpose: Option<String>,
    pub grant_date: Option<String>,
    pub updated_by_user_id: Uuid, 
}

impl Validate for UpdateSubsequentGrant {
    fn validate(&self) -> DomainResult<()> {
        if let Some(amount) = self.amount {
            ValidationBuilder::new("amount", Some(amount)).min(0.0).validate()?;
        }
        if let Some(date) = &self.grant_date {
            if NaiveDate::parse_from_str(date, "%Y-%m-%d").is_err() {
                return Err(DomainError::Validation(crate::errors::ValidationError::format("grant_date", "Invalid date format. Expected YYYY-MM-DD")));
            }
        }
        Ok(())
    }
}

/// SubsequentGrantRow - SQLite row representation
#[derive(Debug, Clone, FromRow)]
pub struct SubsequentGrantRow {
    pub id: String,
    pub livelihood_id: String,
    pub amount: Option<f64>,
    pub amount_updated_at: Option<String>,
    pub amount_updated_by: Option<String>,
    pub purpose: Option<String>,
    pub purpose_updated_at: Option<String>,
    pub purpose_updated_by: Option<String>,
    pub grant_date: Option<String>,
    pub grant_date_updated_at: Option<String>,
    pub grant_date_updated_by: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub created_by_user_id: Option<String>,
    pub updated_by_user_id: Option<String>,
    pub deleted_at: Option<String>,
    pub deleted_by_user_id: Option<String>,
}

impl SubsequentGrantRow {
    pub fn into_entity(self) -> DomainResult<SubsequentGrant> {
        let parse_uuid = |s: &Option<String>| -> Option<DomainResult<Uuid>> {
            s.as_ref().map(|id_str| Uuid::parse_str(id_str).map_err(|_| DomainError::InvalidUuid(id_str.clone())))
        };
        let parse_datetime = |s: &Option<String>| -> Option<DomainResult<DateTime<Utc>>> {
            s.as_ref().map(|dt_str| DateTime::parse_from_rfc3339(dt_str).map(|dt| dt.with_timezone(&Utc)).map_err(|_| DomainError::Internal(format!("Invalid date format: {}", dt_str))))
        };
        Ok(SubsequentGrant {
            id: Uuid::parse_str(&self.id).map_err(|_| DomainError::InvalidUuid(self.id.clone()))?,
            livelihood_id: Uuid::parse_str(&self.livelihood_id).map_err(|_| DomainError::InvalidUuid(self.livelihood_id.clone()))?,
            amount: self.amount,
            amount_updated_at: parse_datetime(&self.amount_updated_at).transpose()?,
            amount_updated_by: parse_uuid(&self.amount_updated_by).transpose()?,
            purpose: self.purpose,
            purpose_updated_at: parse_datetime(&self.purpose_updated_at).transpose()?,
            purpose_updated_by: parse_uuid(&self.purpose_updated_by).transpose()?,
            grant_date: self.grant_date,
            grant_date_updated_at: parse_datetime(&self.grant_date_updated_at).transpose()?,
            grant_date_updated_by: parse_uuid(&self.grant_date_updated_by).transpose()?,
            created_at: DateTime::parse_from_rfc3339(&self.created_at).map(|dt| dt.with_timezone(&Utc)).map_err(|_| DomainError::Internal(format!("Invalid created_at date format: {}", self.created_at)))?,
            updated_at: DateTime::parse_from_rfc3339(&self.updated_at).map(|dt| dt.with_timezone(&Utc)).map_err(|_| DomainError::Internal(format!("Invalid updated_at date format: {}", self.updated_at)))?,
            created_by_user_id: parse_uuid(&self.created_by_user_id).transpose()?,
            updated_by_user_id: parse_uuid(&self.updated_by_user_id).transpose()?,
            deleted_at: parse_datetime(&self.deleted_at).transpose()?,
            deleted_by_user_id: parse_uuid(&self.deleted_by_user_id).transpose()?,
        })
    }
}