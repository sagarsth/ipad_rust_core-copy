use crate::errors::{DomainError, DomainResult};
use crate::validation::{Validate, ValidationBuilder, common};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use sqlx::FromRow;
use std::fmt;

// Added imports
use crate::domains::document::types::MediaDocumentResponse;
use crate::domains::sync::types::SyncPriority;
use crate::domains::core::document_linking::{DocumentLinkable, EntityFieldMetadata, FieldType};
use std::collections::{HashSet, HashMap};

/// Gender enum with string representation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Gender {
    Male,
    Female,
    Other,
    PreferNotToSay,
}

impl Gender {
    pub fn as_str(&self) -> &'static str {
        match self {
            Gender::Male => "male",
            Gender::Female => "female",
            Gender::Other => "other",
            Gender::PreferNotToSay => "prefer_not_to_say",
        }
    }
    
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "male" => Some(Gender::Male),
            "female" => Some(Gender::Female),
            "other" => Some(Gender::Other),
            "prefer_not_to_say" => Some(Gender::PreferNotToSay),
            _ => None,
        }
    }
}

impl fmt::Display for Gender {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Age group enum with string representation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgeGroup {
    Child,
    Youth,
    Adult,
    Elderly,
}

impl AgeGroup {
    pub fn as_str(&self) -> &'static str {
        match self {
            AgeGroup::Child => "child",
            AgeGroup::Youth => "youth",
            AgeGroup::Adult => "adult",
            AgeGroup::Elderly => "elderly",
        }
    }
    
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "child" => Some(AgeGroup::Child),
            "youth" => Some(AgeGroup::Youth),
            "adult" => Some(AgeGroup::Adult),
            "elderly" => Some(AgeGroup::Elderly),
            _ => None,
        }
    }
}

impl fmt::Display for AgeGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Participant entity - represents a participant in the system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Participant {
    pub id: Uuid,
    pub name: String,
    pub name_updated_at: Option<DateTime<Utc>>,
    pub name_updated_by: Option<Uuid>,
    pub gender: Option<String>,
    pub gender_updated_at: Option<DateTime<Utc>>,
    pub gender_updated_by: Option<Uuid>,
    pub disability: bool,
    pub disability_updated_at: Option<DateTime<Utc>>,
    pub disability_updated_by: Option<Uuid>,
    pub disability_type: Option<String>,
    pub disability_type_updated_at: Option<DateTime<Utc>>,
    pub disability_type_updated_by: Option<Uuid>,
    pub age_group: Option<String>,
    pub age_group_updated_at: Option<DateTime<Utc>>,
    pub age_group_updated_by: Option<Uuid>,
    pub location: Option<String>,
    pub location_updated_at: Option<DateTime<Utc>>,
    pub location_updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by_user_id: Option<Uuid>,
    pub updated_by_user_id: Option<Uuid>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub deleted_by_user_id: Option<Uuid>,
    pub sync_priority: Option<SyncPriority>,
}

impl Participant {
    // Helper to check if participant is deleted
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }
    
    // Helper to parse gender string to enum
    pub fn parsed_gender(&self) -> Option<Gender> {
        self.gender.as_ref().and_then(|g| Gender::from_str(g))
    }
    
    // Helper to parse age group string to enum
    pub fn parsed_age_group(&self) -> Option<AgeGroup> {
        self.age_group.as_ref().and_then(|a| AgeGroup::from_str(a))
    }
}

impl DocumentLinkable for Participant {
    fn field_metadata() -> Vec<EntityFieldMetadata> {
        vec![
            EntityFieldMetadata { field_name: "name", display_name: "Full Name", supports_documents: false, field_type: FieldType::Text, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "gender", display_name: "Gender", supports_documents: false, field_type: FieldType::Text, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "disability", display_name: "Has Disability", supports_documents: true, field_type: FieldType::Boolean, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "disability_type", display_name: "Type of Disability", supports_documents: true, field_type: FieldType::Text, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "age_group", display_name: "Age Group", supports_documents: false, field_type: FieldType::Text, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "location", display_name: "Location", supports_documents: false, field_type: FieldType::Text, is_document_reference_only: false },
            // Document Reference Fields from Migration
            EntityFieldMetadata { field_name: "profile_photo", display_name: "Profile Photo", supports_documents: true, field_type: FieldType::DocumentRef, is_document_reference_only: true },
            EntityFieldMetadata { field_name: "identification", display_name: "Identification", supports_documents: true, field_type: FieldType::DocumentRef, is_document_reference_only: true },
            EntityFieldMetadata { field_name: "consent_form", display_name: "Consent Form", supports_documents: true, field_type: FieldType::DocumentRef, is_document_reference_only: true },
            EntityFieldMetadata { field_name: "needs_assessment", display_name: "Needs Assessment", supports_documents: true, field_type: FieldType::DocumentRef, is_document_reference_only: true },
        ]
    }
}

/// NewParticipant DTO - used when creating a new participant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewParticipant {
    pub name: String,
    pub gender: Option<String>,
    pub disability: Option<bool>,
    pub disability_type: Option<String>,
    pub age_group: Option<String>,
    pub location: Option<String>,
    pub created_by_user_id: Option<Uuid>,
    pub sync_priority: Option<SyncPriority>,
}

impl Validate for NewParticipant {
    fn validate(&self) -> DomainResult<()> {
        ValidationBuilder::new("name", Some(self.name.clone()))
            .required()
            .min_length(2)
            .max_length(100)
            .validate()?;
        
        if let Some(gender) = &self.gender {
            common::validate_gender(gender)?;
        }
        if let Some(age_group) = &self.age_group {
            common::validate_age_group(age_group)?;
        }
        
        Ok(())
    }
}

/// UpdateParticipant DTO - used when updating an existing participant
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateParticipant {
    pub name: Option<String>,
    pub gender: Option<String>,
    pub disability: Option<bool>,
    pub disability_type: Option<String>,
    pub age_group: Option<String>,
    pub location: Option<String>,
    pub updated_by_user_id: Uuid,
    pub sync_priority: Option<SyncPriority>,
}

impl Validate for UpdateParticipant {
    fn validate(&self) -> DomainResult<()> {
        if let Some(name) = &self.name {
            ValidationBuilder::new("name", Some(name.clone()))
                .min_length(2)
                .max_length(100)
                .validate()?;
        }
        if let Some(gender) = &self.gender {
            common::validate_gender(gender)?;
        }
        if let Some(age_group) = &self.age_group {
            common::validate_age_group(age_group)?;
        }
        
        Ok(())
    }
}

/// ParticipantRow - SQLite row representation for mapping from database
#[derive(Debug, Clone, FromRow)]
pub struct ParticipantRow {
    pub id: String,
    pub name: String,
    pub name_updated_at: Option<String>,
    pub name_updated_by: Option<String>,
    pub gender: Option<String>,
    pub gender_updated_at: Option<String>,
    pub gender_updated_by: Option<String>,
    pub disability: i64,
    pub disability_updated_at: Option<String>,
    pub disability_updated_by: Option<String>,
    pub disability_type: Option<String>,
    pub disability_type_updated_at: Option<String>,
    pub disability_type_updated_by: Option<String>,
    pub age_group: Option<String>,
    pub age_group_updated_at: Option<String>,
    pub age_group_updated_by: Option<String>,
    pub location: Option<String>,
    pub location_updated_at: Option<String>,
    pub location_updated_by: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub created_by_user_id: Option<String>,
    pub updated_by_user_id: Option<String>,
    pub deleted_at: Option<String>,
    pub deleted_by_user_id: Option<String>,
    pub sync_priority: i64,
}

impl ParticipantRow {
    /// Convert database row to domain entity
    pub fn into_entity(self) -> DomainResult<Participant> {
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
        
        Ok(Participant {
            id: Uuid::parse_str(&self.id)
                 .map_err(|_| DomainError::Internal(format!("Invalid primary key UUID format in DB: {}", self.id)))?,
            name: self.name,
            name_updated_at: parse_datetime(&self.name_updated_at).transpose()?,
            name_updated_by: parse_uuid(&self.name_updated_by).transpose()?,
            gender: self.gender,
            gender_updated_at: parse_datetime(&self.gender_updated_at).transpose()?,
            gender_updated_by: parse_uuid(&self.gender_updated_by).transpose()?,
            disability: self.disability != 0,
            disability_updated_at: parse_datetime(&self.disability_updated_at).transpose()?,
            disability_updated_by: parse_uuid(&self.disability_updated_by).transpose()?,
            disability_type: self.disability_type,
            disability_type_updated_at: parse_datetime(&self.disability_type_updated_at).transpose()?,
            disability_type_updated_by: parse_uuid(&self.disability_type_updated_by).transpose()?,
            age_group: self.age_group,
            age_group_updated_at: parse_datetime(&self.age_group_updated_at).transpose()?,
            age_group_updated_by: parse_uuid(&self.age_group_updated_by).transpose()?,
            location: self.location,
            location_updated_at: parse_datetime(&self.location_updated_at).transpose()?,
            location_updated_by: parse_uuid(&self.location_updated_by).transpose()?,
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
            sync_priority: Some(SyncPriority::from(self.sync_priority)),
        })
    }
}

/// Basic participant summary for nested responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantSummary {
    pub id: Uuid,
    pub name: String,
    pub gender: Option<String>,
    pub age_group: Option<String>,
    pub disability: bool,
}

impl From<Participant> for ParticipantSummary {
    fn from(participant: Participant) -> Self {
        Self {
            id: participant.id,
            name: participant.name,
            gender: participant.gender,
            age_group: participant.age_group,
            disability: participant.disability,
        }
    }
}

/// ParticipantResponse DTO - used as the API response for a participant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantResponse {
    pub id: Uuid,
    pub name: String,
    pub gender: Option<String>,
    pub disability: bool,
    pub disability_type: Option<String>,
    pub age_group: Option<String>,
    pub location: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documents: Option<Vec<MediaDocumentResponse>>,
}

impl From<Participant> for ParticipantResponse {
    fn from(p: Participant) -> Self {
        Self {
            id: p.id,
            name: p.name,
            gender: p.gender,
            disability: p.disability,
            disability_type: p.disability_type,
            age_group: p.age_group,
            location: p.location,
            created_at: p.created_at.to_rfc3339(),
            updated_at: p.updated_at.to_rfc3339(),
            documents: None,
        }
    }
}

/// Enum to specify related data to include in participant responses
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParticipantInclude {
    WorkshopCount,
    LivelihoodCount,
    Documents,
    Workshops,
    Livelihoods,
    DocumentCounts,
    All,
}

/// Workshop-Participant junction table representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkshopParticipant {
    pub id: Uuid,
    pub workshop_id: Uuid,
    pub participant_id: Uuid,
    pub pre_evaluation: Option<String>,
    pub pre_evaluation_updated_at: Option<DateTime<Utc>>,
    pub pre_evaluation_updated_by: Option<Uuid>,
    pub post_evaluation: Option<String>,
    pub post_evaluation_updated_at: Option<DateTime<Utc>>,
    pub post_evaluation_updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by_user_id: Option<Uuid>,
    pub updated_by_user_id: Option<Uuid>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub deleted_by_user_id: Option<Uuid>,
}

/// DTO for adding a participant to a workshop
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddParticipantToWorkshop {
    pub workshop_id: Uuid,
    pub participant_id: Uuid,
    pub pre_evaluation: Option<String>,
    pub created_by_user_id: Option<Uuid>,
}

impl Validate for AddParticipantToWorkshop {
    fn validate(&self) -> DomainResult<()> {
        // Validate workshop_id
        ValidationBuilder::new("workshop_id", Some(self.workshop_id))
            .not_nil()
            .validate()?;
            
        // Validate participant_id
        ValidationBuilder::new("participant_id", Some(self.participant_id))
            .not_nil()
            .validate()?;
            
        Ok(())
    }
}

/// DTO for updating a workshop-participant relationship
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateWorkshopParticipant {
    pub pre_evaluation: Option<String>,
    pub post_evaluation: Option<String>,
    pub updated_by_user_id: Uuid,
}

/// SQLite row representation for workshop-participant junction
#[derive(Debug, Clone, FromRow)]
pub struct WorkshopParticipantRow {
    pub id: String,
    pub workshop_id: String,
    pub participant_id: String,
    pub pre_evaluation: Option<String>,
    pub pre_evaluation_updated_at: Option<String>,
    pub pre_evaluation_updated_by: Option<String>,
    pub post_evaluation: Option<String>,
    pub post_evaluation_updated_at: Option<String>,
    pub post_evaluation_updated_by: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub created_by_user_id: Option<String>,
    pub updated_by_user_id: Option<String>,
    pub deleted_at: Option<String>,
    pub deleted_by_user_id: Option<String>,
}

impl WorkshopParticipantRow {
    /// Convert database row to domain entity
    pub fn into_entity(self) -> DomainResult<WorkshopParticipant> {
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
        
        Ok(WorkshopParticipant {
            id: Uuid::parse_str(&self.id)
                .map_err(|_| DomainError::InvalidUuid(self.id))?,
            workshop_id: Uuid::parse_str(&self.workshop_id)
                .map_err(|_| DomainError::InvalidUuid(self.workshop_id))?,
            participant_id: Uuid::parse_str(&self.participant_id)
                .map_err(|_| DomainError::InvalidUuid(self.participant_id))?,
            pre_evaluation: self.pre_evaluation,
            pre_evaluation_updated_at: parse_datetime(&self.pre_evaluation_updated_at)
                .transpose()?,
            pre_evaluation_updated_by: parse_uuid(&self.pre_evaluation_updated_by)
                .transpose()?,
            post_evaluation: self.post_evaluation,
            post_evaluation_updated_at: parse_datetime(&self.post_evaluation_updated_at)
                .transpose()?,
            post_evaluation_updated_by: parse_uuid(&self.post_evaluation_updated_by)
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

/// Demographic statistics for participants
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantDemographics {
    pub total_participants: i64,
    pub by_gender: HashMap<String, i64>,
    pub by_age_group: HashMap<String, i64>,
    pub by_location: HashMap<String, i64>,
    pub by_disability: HashMap<String, i64>,
    pub by_disability_type: HashMap<String, i64>,
}

/// Workshop summary for a participant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkshopSummary {
    pub id: Uuid,
    pub name: String,
    pub date: Option<String>,
    pub location: Option<String>,
    pub has_completed: bool,
    pub pre_evaluation: Option<String>,
    pub post_evaluation: Option<String>,
}

/// Livelihood summary for a participant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LivelihoodSummary {
    pub id: Uuid,
    pub name: String,
    pub type_: Option<String>,
    pub status: Option<String>,
    pub start_date: Option<String>,
}

/// Participant with their workshop history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantWithWorkshops {
    pub participant: ParticipantResponse,
    pub workshops: Vec<WorkshopSummary>,
    pub total_workshops: i64,
    pub completed_workshops: i64,
    pub upcoming_workshops: i64,
}

/// Participant with their livelihood history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantWithLivelihoods {
    pub participant: ParticipantResponse,
    pub livelihoods: Vec<LivelihoodSummary>,
    pub total_livelihoods: i64,
    pub active_livelihoods: i64,
}

/// Participant with document timeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantWithDocumentTimeline {
    pub participant: ParticipantResponse,
    pub documents_by_month: HashMap<String, Vec<MediaDocumentResponse>>,
    pub total_document_count: u64,
}

/// Workshop attendance statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkshopAttendance {
    pub total_workshops: i64,
    pub total_participants: i64,
    pub avg_participants_per_workshop: f64,
    pub workshops_by_participant_count: HashMap<i64, i64>, // Map of participant count -> workshop count
    pub participants_by_workshop_count: HashMap<i64, i64>, // Map of workshop count -> participant count
}