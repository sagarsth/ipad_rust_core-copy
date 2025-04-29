use crate::errors::{DomainError, DomainResult};
use crate::validation::{Validate, ValidationBuilder};
use uuid::Uuid;
use chrono::{DateTime, Utc, NaiveDate};
use serde::{Serialize, Deserialize};
use sqlx::FromRow;
use crate::domains::document::types::MediaDocumentResponse;
use crate::domains::core::document_linking::{DocumentLinkable, EntityFieldMetadata, FieldType};
use std::collections::HashSet;

/// Livelihood entity - represents a livelihood grant for a participant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Livelihood {
    pub id: Uuid,
    pub participant_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    pub grant_amount: Option<f64>,
    pub grant_amount_updated_at: Option<DateTime<Utc>>,
    pub grant_amount_updated_by: Option<Uuid>,
    pub purpose: Option<String>,
    pub purpose_updated_at: Option<DateTime<Utc>>,
    pub purpose_updated_by: Option<Uuid>,
    pub progress1: Option<String>,
    pub progress1_updated_at: Option<DateTime<Utc>>,
    pub progress1_updated_by: Option<Uuid>,
    pub progress2: Option<String>,
    pub progress2_updated_at: Option<DateTime<Utc>>,
    pub progress2_updated_by: Option<Uuid>,
    pub outcome: Option<String>,
    pub outcome_updated_at: Option<DateTime<Utc>>,
    pub outcome_updated_by: Option<Uuid>,
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
        let initial = self.grant_amount.unwrap_or(0.0);
        let subsequent: f64 = subsequent_grants.iter()
            .filter(|grant| !grant.is_deleted())
            .filter_map(|grant| grant.amount)
            .sum();
        
        initial + subsequent
    }
}

impl DocumentLinkable for Livelihood {
    fn field_metadata() -> Vec<EntityFieldMetadata> {
        vec![
            EntityFieldMetadata { field_name: "grant_amount", display_name: "Initial Grant Amount", supports_documents: true, field_type: FieldType::Number, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "purpose", display_name: "Purpose", supports_documents: true, field_type: FieldType::Text, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "progress1", display_name: "Progress 1", supports_documents: true, field_type: FieldType::Text, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "progress2", display_name: "Progress 2", supports_documents: true, field_type: FieldType::Text, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "outcome", display_name: "Outcome", supports_documents: true, field_type: FieldType::Text, is_document_reference_only: false },
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
    // Helper to check if grant is deleted
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }
    
    // Helper to parse grant date
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
            // Document Reference Fields from Migration
            EntityFieldMetadata { field_name: "grant_application", display_name: "Grant Application", supports_documents: true, field_type: FieldType::DocumentRef, is_document_reference_only: true },
            EntityFieldMetadata { field_name: "grant_report", display_name: "Grant Report", supports_documents: true, field_type: FieldType::DocumentRef, is_document_reference_only: true },
            EntityFieldMetadata { field_name: "receipts", display_name: "Receipts", supports_documents: true, field_type: FieldType::DocumentRef, is_document_reference_only: true }, // May represent multiple docs
        ]
    }
}

/// NewLivelihood DTO - used when creating a new livelihood
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewLivelihood {
    pub participant_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    pub grant_amount: Option<f64>,
    pub purpose: Option<String>,
    pub created_by_user_id: Option<Uuid>,
}

impl Validate for NewLivelihood {
    fn validate(&self) -> DomainResult<()> {
        // Validate participant_id if provided
        if let Some(participant_id) = self.participant_id {
            ValidationBuilder::new("participant_id", Some(participant_id))
                .not_nil()
                .validate()?;
        }
            
        // Validate project_id if provided
        if let Some(project_id) = self.project_id {
            ValidationBuilder::new("project_id", Some(project_id))
                .not_nil()
                .validate()?;
        }
            
        // Validate grant_amount if provided
        if let Some(amount) = self.grant_amount {
            ValidationBuilder::new("grant_amount", Some(amount))
                .min(0.0)
                .validate()?;
        }
        
        Ok(())
    }
}

/// UpdateLivelihood DTO - used when updating an existing livelihood
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateLivelihood {
    pub grant_amount: Option<f64>,
    pub purpose: Option<String>,
    pub progress1: Option<String>,
    pub progress2: Option<String>,
    pub outcome: Option<String>,
    pub updated_by_user_id: Uuid,
}

impl Validate for UpdateLivelihood {
    fn validate(&self) -> DomainResult<()> {
        // Validate grant_amount if provided
        if let Some(amount) = self.grant_amount {
            ValidationBuilder::new("grant_amount", Some(amount))
                .min(0.0)
                .validate()?;
        }
        
        Ok(())
    }
}

/// NewSubsequentGrant DTO - used when creating a new subsequent grant
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
        // Validate livelihood_id
        ValidationBuilder::new("livelihood_id", Some(self.livelihood_id))
            .not_nil()
            .validate()?;
            
        // Validate amount if provided
        if let Some(amount) = self.amount {
            ValidationBuilder::new("amount", Some(amount))
                .min(0.0)
                .validate()?;
        }
        
        // Validate grant_date if provided
        if let Some(date) = &self.grant_date {
            if NaiveDate::parse_from_str(date, "%Y-%m-%d").is_err() {
                return Err(DomainError::Validation(
                    crate::errors::ValidationError::format(
                        "grant_date", 
                        "Invalid date format. Expected YYYY-MM-DD"
                    )
                ));
            }
        }
        
        Ok(())
    }
}

/// UpdateSubsequentGrant DTO - used when updating an existing subsequent grant
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateSubsequentGrant {
    pub amount: Option<f64>,
    pub purpose: Option<String>,
    pub grant_date: Option<String>,
    pub updated_by_user_id: Uuid,
}

impl Validate for UpdateSubsequentGrant {
    fn validate(&self) -> DomainResult<()> {
        // Validate amount if provided
        if let Some(amount) = self.amount {
            ValidationBuilder::new("amount", Some(amount))
                .min(0.0)
                .validate()?;
        }
        
        // Validate grant_date if provided
        if let Some(date) = &self.grant_date {
            if NaiveDate::parse_from_str(date, "%Y-%m-%d").is_err() {
                return Err(DomainError::Validation(
                    crate::errors::ValidationError::format(
                        "grant_date", 
                        "Invalid date format. Expected YYYY-MM-DD"
                    )
                ));
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
    pub grant_amount: Option<f64>,
    pub grant_amount_updated_at: Option<String>,
    pub grant_amount_updated_by: Option<String>,
    pub purpose: Option<String>,
    pub purpose_updated_at: Option<String>,
    pub purpose_updated_by: Option<String>,
    pub progress1: Option<String>,
    pub progress1_updated_at: Option<String>,
    pub progress1_updated_by: Option<String>,
    pub progress2: Option<String>,
    pub progress2_updated_at: Option<String>,
    pub progress2_updated_by: Option<String>,
    pub outcome: Option<String>,
    pub outcome_updated_at: Option<String>,
    pub outcome_updated_by: Option<String>,
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
                    .map(|dt| dt.with_timezone(&Utc))
                    .map_err(|_| DomainError::Internal(format!("Invalid date format: {}", dt)))
            })
        };
        
        Ok(Livelihood {
            id: Uuid::parse_str(&self.id)
                .map_err(|_| DomainError::InvalidUuid(self.id))?,
            participant_id: parse_uuid(&self.participant_id).transpose()?,
            project_id: parse_uuid(&self.project_id).transpose()?,
            grant_amount: self.grant_amount,
            grant_amount_updated_at: parse_datetime(&self.grant_amount_updated_at)
                .transpose()?,
            grant_amount_updated_by: parse_uuid(&self.grant_amount_updated_by)
                .transpose()?,
            purpose: self.purpose,
            purpose_updated_at: parse_datetime(&self.purpose_updated_at)
                .transpose()?,
            purpose_updated_by: parse_uuid(&self.purpose_updated_by)
                .transpose()?,
            progress1: self.progress1,
            progress1_updated_at: parse_datetime(&self.progress1_updated_at)
                .transpose()?,
            progress1_updated_by: parse_uuid(&self.progress1_updated_by)
                .transpose()?,
            progress2: self.progress2,
            progress2_updated_at: parse_datetime(&self.progress2_updated_at)
                .transpose()?,
            progress2_updated_by: parse_uuid(&self.progress2_updated_by)
                .transpose()?,
            outcome: self.outcome,
            outcome_updated_at: parse_datetime(&self.outcome_updated_at)
                .transpose()?,
            outcome_updated_by: parse_uuid(&self.outcome_updated_by)
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
        })
    }
}

/// SubsequentGrantRow - SQLite row representation for mapping from database
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
    /// Convert database row to domain entity
    pub fn into_entity(self) -> DomainResult<SubsequentGrant> {
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
        
        Ok(SubsequentGrant {
            id: Uuid::parse_str(&self.id)
                .map_err(|_| DomainError::InvalidUuid(self.id))?,
            livelihood_id: Uuid::parse_str(&self.livelihood_id)
                .map_err(|_| DomainError::InvalidUuid(self.livelihood_id))?,
            amount: self.amount,
            amount_updated_at: parse_datetime(&self.amount_updated_at)
                .transpose()?,
            amount_updated_by: parse_uuid(&self.amount_updated_by)
                .transpose()?,
            purpose: self.purpose,
            purpose_updated_at: parse_datetime(&self.purpose_updated_at)
                .transpose()?,
            purpose_updated_by: parse_uuid(&self.purpose_updated_by)
                .transpose()?,
            grant_date: self.grant_date,
            grant_date_updated_at: parse_datetime(&self.grant_date_updated_at)
                .transpose()?,
            grant_date_updated_by: parse_uuid(&self.grant_date_updated_by)
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
    pub grant_amount: Option<f64>,
    pub purpose: Option<String>,
    pub progress1: Option<String>,
    pub progress2: Option<String>,
    pub outcome: Option<String>,
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
            grant_amount: livelihood.grant_amount,
            purpose: livelihood.purpose,
            progress1: livelihood.progress1,
            progress2: livelihood.progress2,
            outcome: livelihood.outcome,
            created_at: livelihood.created_at.to_rfc3339(),
            updated_at: livelihood.updated_at.to_rfc3339(),
            subsequent_grants: None,
            total_grant_amount: livelihood.grant_amount,
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
        let total = self.grant_amount.unwrap_or(0.0) + 
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
    SubsequentGrants,
    Documents,
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
    pub participant_id: Uuid,
    pub participant_name: String,
    pub grant_amount: Option<f64>,
    pub purpose: Option<String>,
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