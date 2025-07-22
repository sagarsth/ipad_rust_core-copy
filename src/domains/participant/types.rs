use crate::errors::{DomainError, DomainResult, ValidationError};
use crate::validation::{Validate, ValidationBuilder, common};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use sqlx::FromRow;
use std::fmt;
use std::str::FromStr;

// Added imports
use crate::domains::document::types::MediaDocumentResponse;
use crate::domains::sync::types::SyncPriority;
use crate::domains::core::document_linking::{DocumentLinkable, EntityFieldMetadata, FieldType};
use std::collections::{HashSet, HashMap};

/// Filter for complex participant queries - enables bulk filtering like project domain
///
/// # Disability Filtering Examples:
/// 
/// ```rust
/// // Simple: Show all participants with ANY disability
/// let filter = ParticipantFilter::new().with_any_disability();
/// 
/// // Simple: Show all participants with NO disability  
/// let filter = ParticipantFilter::new().with_no_disability();
/// 
/// // Advanced: Show participants with specific disability types (after long-press)
/// let disability_types = vec!["visual".to_string(), "hearing".to_string()];
/// let filter = ParticipantFilter::new().with_specific_disability_types(disability_types);
/// 
/// // Combined filtering: Age group + specific disability types
/// let filter = ParticipantFilter::new()
///     .with_age_groups(vec!["adult".to_string()])
///     .with_specific_disability_types(vec!["mobility".to_string()]);
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ParticipantFilter {
    pub genders: Option<Vec<String>>,
    pub age_groups: Option<Vec<String>>,
    pub locations: Option<Vec<String>>,
    /// Simple disability toggle: true = any disability, false = no disability
    /// If disability_types is also specified, disability_types takes precedence
    pub disability: Option<bool>,
    /// Advanced disability filtering: filter by specific disability types (OR logic within types)
    /// When specified, this takes precedence over the simple disability boolean
    /// Automatically implies disability = true since you can't have a type without having a disability
    pub disability_types: Option<Vec<String>>,
    pub search_text: Option<String>,
    pub date_range: Option<(String, String)>, // (start_date, end_date)
    pub created_by_user_ids: Option<Vec<Uuid>>,
    pub workshop_ids: Option<Vec<Uuid>>, // Filter by participants in specific workshops
    pub has_documents: Option<bool>,
    pub document_linked_fields: Option<Vec<String>>, // Filter by participants with documents in specific fields
    #[serde(default = "default_true")]
    pub exclude_deleted: bool,
}

fn default_true() -> bool {
    true
}

impl ParticipantFilter {
    /// Create a new empty filter
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Add gender filter
    pub fn with_genders(mut self, genders: Vec<String>) -> Self {
        self.genders = Some(genders);
        self
    }
    
    /// Add age group filter
    pub fn with_age_groups(mut self, age_groups: Vec<String>) -> Self {
        self.age_groups = Some(age_groups);
        self
    }
    
    /// Add location filter
    pub fn with_locations(mut self, locations: Vec<String>) -> Self {
        self.locations = Some(locations);
        self
    }
    
    /// Add disability filter
    pub fn with_disability(mut self, has_disability: bool) -> Self {
        self.disability = Some(has_disability);
        self
    }
    
    /// Add disability type filter
    pub fn with_disability_types(mut self, disability_types: Vec<String>) -> Self {
        self.disability_types = Some(disability_types);
        self
    }
    
    /// Simple method: filter for participants with ANY disability
    pub fn with_any_disability(mut self) -> Self {
        self.disability = Some(true);
        // Clear disability_types to ensure the boolean filter is used
        self.disability_types = None;
        self
    }
    
    /// Simple method: filter for participants with NO disability
    pub fn with_no_disability(mut self) -> Self {
        self.disability = Some(false);
        // Clear disability_types to ensure the boolean filter is used
        self.disability_types = None;
        self
    }
    
    /// Advanced method: filter for participants with specific disability types
    /// This automatically implies disability = true and takes precedence over disability boolean
    pub fn with_specific_disability_types(mut self, disability_types: Vec<String>) -> Self {
        self.disability_types = Some(disability_types);
        // Don't clear disability boolean - it might be used for validation but disability_types takes precedence
        self
    }
    
    /// Add search text filter
    pub fn with_search_text(mut self, search_text: String) -> Self {
        self.search_text = Some(search_text);
        self
    }
    
    /// Add date range filter
    pub fn with_date_range(mut self, start_date: String, end_date: String) -> Self {
        self.date_range = Some((start_date, end_date));
        self
    }
    
    /// Add created by user filter
    pub fn with_created_by_users(mut self, user_ids: Vec<Uuid>) -> Self {
        self.created_by_user_ids = Some(user_ids);
        self
    }
    
    /// Add workshop participation filter
    pub fn with_workshops(mut self, workshop_ids: Vec<Uuid>) -> Self {
        self.workshop_ids = Some(workshop_ids);
        self
    }
    
    /// Add document existence filter
    pub fn with_has_documents(mut self, has_documents: bool) -> Self {
        self.has_documents = Some(has_documents);
        self
    }
    
    /// Add document linked field filter
    pub fn with_document_linked_fields(mut self, fields: Vec<String>) -> Self {
        self.document_linked_fields = Some(fields);
        self
    }
    
    /// Include soft-deleted records
    pub fn include_deleted(mut self) -> Self {
        self.exclude_deleted = false;
        self
    }
    
    /// Check if filter is empty (no filtering criteria)
    pub fn is_empty(&self) -> bool {
        self.genders.is_none() 
            && self.age_groups.is_none()
            && self.locations.is_none()
            && self.disability.is_none()
            && self.disability_types.is_none()
            && self.search_text.is_none()
            && self.date_range.is_none()
            && self.created_by_user_ids.is_none()
            && self.workshop_ids.is_none()
            && self.has_documents.is_none()
            && self.document_linked_fields.is_none()
    }
}

impl Validate for ParticipantFilter {
    fn validate(&self) -> DomainResult<()> {
        // Validate gender values
        if let Some(genders) = &self.genders {
            for gender in genders {
                common::validate_gender(gender)?;
            }
        }
        
        // Validate age group values
        if let Some(age_groups) = &self.age_groups {
            for age_group in age_groups {
                common::validate_age_group(age_group)?;
            }
        }
        
        // Validate date range format
        if let Some((start_date, end_date)) = &self.date_range {
            DateTime::parse_from_rfc3339(start_date)
                .map_err(|_| DomainError::Validation(ValidationError::format("start_date", "Invalid RFC3339 format")))?;
            DateTime::parse_from_rfc3339(end_date)
                .map_err(|_| DomainError::Validation(ValidationError::format("end_date", "Invalid RFC3339 format")))?;
        }
        
        // Validate document linked fields
        if let Some(fields) = &self.document_linked_fields {
            let valid_fields: HashSet<String> = Participant::field_metadata()
                .into_iter()
                .filter(|f| f.supports_documents)
                .map(|f| f.field_name.to_string())
                .collect();
                
            for field in fields {
                if !valid_fields.contains(field) {
                    return Err(DomainError::Validation(ValidationError::format(
                        "document_linked_fields",
                        &format!("Invalid field '{}'. Valid fields: {:?}", field, valid_fields)
                    )));
                }
            }
        }
        
        Ok(())
    }
}

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
    pub name_updated_by_device_id: Option<Uuid>,
    pub gender: Option<String>,
    pub gender_updated_at: Option<DateTime<Utc>>,
    pub gender_updated_by: Option<Uuid>,
    pub gender_updated_by_device_id: Option<Uuid>,
    pub disability: bool,
    pub disability_updated_at: Option<DateTime<Utc>>,
    pub disability_updated_by: Option<Uuid>,
    pub disability_updated_by_device_id: Option<Uuid>,
    pub disability_type: Option<String>,
    pub disability_type_updated_at: Option<DateTime<Utc>>,
    pub disability_type_updated_by: Option<Uuid>,
    pub disability_type_updated_by_device_id: Option<Uuid>,
    pub age_group: Option<String>,
    pub age_group_updated_at: Option<DateTime<Utc>>,
    pub age_group_updated_by: Option<Uuid>,
    pub age_group_updated_by_device_id: Option<Uuid>,
    pub location: Option<String>,
    pub location_updated_at: Option<DateTime<Utc>>,
    pub location_updated_by: Option<Uuid>,
    pub location_updated_by_device_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by_user_id: Option<Uuid>,
    pub created_by_device_id: Option<Uuid>,
    pub updated_by_user_id: Option<Uuid>,
    pub updated_by_device_id: Option<Uuid>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub deleted_by_user_id: Option<Uuid>,
    pub deleted_by_device_id: Option<Uuid>,
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
    pub name_updated_by_device_id: Option<String>,
    pub gender: Option<String>,
    pub gender_updated_at: Option<String>,
    pub gender_updated_by: Option<String>,
    pub gender_updated_by_device_id: Option<String>,
    pub disability: i64,
    pub disability_updated_at: Option<String>,
    pub disability_updated_by: Option<String>,
    pub disability_updated_by_device_id: Option<String>,
    pub disability_type: Option<String>,
    pub disability_type_updated_at: Option<String>,
    pub disability_type_updated_by: Option<String>,
    pub disability_type_updated_by_device_id: Option<String>,
    pub age_group: Option<String>,
    pub age_group_updated_at: Option<String>,
    pub age_group_updated_by: Option<String>,
    pub age_group_updated_by_device_id: Option<String>,
    pub location: Option<String>,
    pub location_updated_at: Option<String>,
    pub location_updated_by: Option<String>,
    pub location_updated_by_device_id: Option<String>,
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

impl ParticipantRow {
    /// Convert database row to domain entity
    pub fn into_entity(self) -> DomainResult<Participant> {
        let parse_optional_uuid = |s: &Option<String>, field_name: &str| -> DomainResult<Option<Uuid>> {
            match s {
                Some(id_str) => Uuid::parse_str(id_str)
                    .map(Some)
                    .map_err(|_| DomainError::Validation(ValidationError::format(field_name, &format!("Invalid UUID format: {}", id_str)))),
                None => Ok(None),
            }
        };
        
        let parse_optional_datetime = |s: &Option<String>, field_name: &str| -> DomainResult<Option<DateTime<Utc>>> {
            match s {
                Some(dt_str) => DateTime::parse_from_rfc3339(dt_str)
                    .map(|dt| Some(dt.with_timezone(&Utc)))
                    .map_err(|_| DomainError::Validation(ValidationError::format(field_name, &format!("Invalid RFC3339 format: {}", dt_str)))),
                None => Ok(None),
            }
        };
        
        Ok(Participant {
            id: Uuid::parse_str(&self.id)
                 .map_err(|_| DomainError::Validation(ValidationError::format("id", &format!("Invalid UUID format: {}", self.id))))?,
            name: self.name,
            name_updated_at: parse_optional_datetime(&self.name_updated_at, "name_updated_at")?,
            name_updated_by: parse_optional_uuid(&self.name_updated_by, "name_updated_by")?,
            name_updated_by_device_id: parse_optional_uuid(&self.name_updated_by_device_id, "name_updated_by_device_id")?,
            gender: self.gender,
            gender_updated_at: parse_optional_datetime(&self.gender_updated_at, "gender_updated_at")?,
            gender_updated_by: parse_optional_uuid(&self.gender_updated_by, "gender_updated_by")?,
            gender_updated_by_device_id: parse_optional_uuid(&self.gender_updated_by_device_id, "gender_updated_by_device_id")?,
            disability: self.disability != 0,
            disability_updated_at: parse_optional_datetime(&self.disability_updated_at, "disability_updated_at")?,
            disability_updated_by: parse_optional_uuid(&self.disability_updated_by, "disability_updated_by")?,
            disability_updated_by_device_id: parse_optional_uuid(&self.disability_updated_by_device_id, "disability_updated_by_device_id")?,
            disability_type: self.disability_type,
            disability_type_updated_at: parse_optional_datetime(&self.disability_type_updated_at, "disability_type_updated_at")?,
            disability_type_updated_by: parse_optional_uuid(&self.disability_type_updated_by, "disability_type_updated_by")?,
            disability_type_updated_by_device_id: parse_optional_uuid(&self.disability_type_updated_by_device_id, "disability_type_updated_by_device_id")?,
            age_group: self.age_group,
            age_group_updated_at: parse_optional_datetime(&self.age_group_updated_at, "age_group_updated_at")?,
            age_group_updated_by: parse_optional_uuid(&self.age_group_updated_by, "age_group_updated_by")?,
            age_group_updated_by_device_id: parse_optional_uuid(&self.age_group_updated_by_device_id, "age_group_updated_by_device_id")?,
            location: self.location,
            location_updated_at: parse_optional_datetime(&self.location_updated_at, "location_updated_at")?,
            location_updated_by: parse_optional_uuid(&self.location_updated_by, "location_updated_by")?,
            location_updated_by_device_id: parse_optional_uuid(&self.location_updated_by_device_id, "location_updated_by_device_id")?,
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
            sync_priority: Some(SyncPriority::from_str(&self.sync_priority).unwrap_or_default()),
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    pub pre_evaluation_updated_by_device_id: Option<Uuid>,
    pub post_evaluation: Option<String>,
    pub post_evaluation_updated_at: Option<DateTime<Utc>>,
    pub post_evaluation_updated_by: Option<Uuid>,
    pub post_evaluation_updated_by_device_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by_user_id: Option<Uuid>,
    pub created_by_device_id: Option<Uuid>,
    pub updated_by_user_id: Option<Uuid>,
    pub updated_by_device_id: Option<Uuid>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub deleted_by_user_id: Option<Uuid>,
    pub deleted_by_device_id: Option<Uuid>,
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
    pub pre_evaluation_updated_by_device_id: Option<String>,
    pub post_evaluation: Option<String>,
    pub post_evaluation_updated_at: Option<String>,
    pub post_evaluation_updated_by: Option<String>,
    pub post_evaluation_updated_by_device_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub created_by_user_id: Option<String>,
    pub created_by_device_id: Option<String>,
    pub updated_by_user_id: Option<String>,
    pub updated_by_device_id: Option<String>,
    pub deleted_at: Option<String>,
    pub deleted_by_user_id: Option<String>,
    pub deleted_by_device_id: Option<String>,
}

impl WorkshopParticipantRow {
    /// Convert database row to domain entity
    pub fn into_entity(self) -> DomainResult<WorkshopParticipant> {
        let parse_optional_uuid = |s: &Option<String>, field_name: &str| -> DomainResult<Option<Uuid>> {
            match s {
                Some(id_str) => Uuid::parse_str(id_str)
                    .map(Some)
                    .map_err(|_| DomainError::Validation(ValidationError::format(field_name, &format!("Invalid UUID format: {}", id_str)))),
                None => Ok(None),
            }
        };
        
        let parse_optional_datetime = |s: &Option<String>, field_name: &str| -> DomainResult<Option<DateTime<Utc>>> {
            match s {
                Some(dt_str) => DateTime::parse_from_rfc3339(dt_str)
                    .map(|dt| Some(dt.with_timezone(&Utc)))
                    .map_err(|_| DomainError::Validation(ValidationError::format(field_name, &format!("Invalid RFC3339 format: {}", dt_str)))),
                None => Ok(None),
            }
        };
        
        Ok(WorkshopParticipant {
            id: Uuid::parse_str(&self.id)
                .map_err(|_| DomainError::Validation(ValidationError::format("id", &format!("Invalid UUID format: {}", self.id))))?,
            workshop_id: Uuid::parse_str(&self.workshop_id)
                .map_err(|_| DomainError::Validation(ValidationError::format("workshop_id", &format!("Invalid UUID format: {}", self.workshop_id))))?,
            participant_id: Uuid::parse_str(&self.participant_id)
                .map_err(|_| DomainError::Validation(ValidationError::format("participant_id", &format!("Invalid UUID format: {}", self.participant_id))))?,
            pre_evaluation: self.pre_evaluation,
            pre_evaluation_updated_at: parse_optional_datetime(&self.pre_evaluation_updated_at, "pre_evaluation_updated_at")?,
            pre_evaluation_updated_by: parse_optional_uuid(&self.pre_evaluation_updated_by, "pre_evaluation_updated_by")?,
            pre_evaluation_updated_by_device_id: parse_optional_uuid(&self.pre_evaluation_updated_by_device_id, "pre_evaluation_updated_by_device_id")?,
            post_evaluation: self.post_evaluation,
            post_evaluation_updated_at: parse_optional_datetime(&self.post_evaluation_updated_at, "post_evaluation_updated_at")?,
            post_evaluation_updated_by: parse_optional_uuid(&self.post_evaluation_updated_by, "post_evaluation_updated_by")?,
            post_evaluation_updated_by_device_id: parse_optional_uuid(&self.post_evaluation_updated_by_device_id, "post_evaluation_updated_by_device_id")?,
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