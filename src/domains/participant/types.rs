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
/// # Enhanced Grouped Filtering Logic:
/// 
/// The filter supports both general disability filtering and specific disability type filtering:
/// 
/// ## Simple Disability Filtering:
/// ```rust
/// // Show all participants with ANY disability
/// let filter = ParticipantFilter::new().with_any_disability();
/// 
/// // Show all participants with NO disability  
/// let filter = ParticipantFilter::new().with_no_disability();
/// ```
/// 
/// ## Advanced Disability Type Filtering:
/// ```rust
/// // Show participants with specific disability types (takes precedence over general filter)
/// let disability_types = vec!["visual".to_string(), "hearing".to_string()];
/// let filter = ParticipantFilter::new().with_specific_disability_types(disability_types);
/// ```
/// 
/// ## Grouped UI Integration:
/// When used with the grouped filter UI:
/// - If `disability_types` is specified, it takes precedence and implies `disability = true`
/// - If only `disability` boolean is set, use that for general filtering
/// - The UI automatically handles the interaction between general and specific filters
/// 
/// ## Combined Filtering:
/// ```rust
/// // Age group + specific disability types (AND logic between categories, OR within)
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
        // Enhanced name validation with business rules
        ValidationBuilder::new("name", Some(self.name.clone()))
            .required()
            .min_length(2)
            .max_length(100)
            .validate()
            .map_err(|e| DomainError::Validation(ValidationError::format(
                "name", 
                &format!("Participant name validation failed: {}", e)
            )))?;
            
        // Check for suspicious patterns in name
        if self.name.trim().chars().any(|c| c.is_numeric() && self.name.chars().filter(|&x| x.is_numeric()).count() > 2) {
            return Err(DomainError::Validation(ValidationError::format(
                "name",
                "Participant name contains too many numbers. Please enter a valid name."
            )));
        }
        
        // Enhanced gender validation with specific error message
        if let Some(gender) = &self.gender {
            common::validate_gender(gender)
                .map_err(|_| DomainError::Validation(ValidationError::format(
                    "gender",
                    &format!("Invalid gender value '{}'. Valid options: male, female, other, prefer_not_to_say", gender)
                )))?;
        }
        
        // Enhanced age group validation with specific error message
        if let Some(age_group) = &self.age_group {
            common::validate_age_group(age_group)
                .map_err(|_| DomainError::Validation(ValidationError::format(
                    "age_group", 
                    &format!("Invalid age group '{}'. Valid options: child, youth, adult, elderly", age_group)
                )))?;
        }
        
        // Business rule: Disability type requires disability flag
        if self.disability_type.is_some() && self.disability.unwrap_or(false) == false {
            return Err(DomainError::Validation(ValidationError::custom(
                "Cannot specify disability_type when disability is false. Either set disability to true or remove disability_type."
            )));
        }
        
        // Enhanced disability type validation
        if let Some(disability_type) = &self.disability_type {
            if disability_type.trim().is_empty() {
                return Err(DomainError::Validation(ValidationError::format(
                    "disability_type",
                    "Disability type cannot be empty. Please specify the type of disability or remove this field."
                )));
            }
            
            let valid_disability_types = ["visual", "hearing", "physical", "intellectual", "psychosocial", "multiple", "other"];
            let normalized_type = disability_type.to_lowercase();
            if !valid_disability_types.contains(&normalized_type.as_str()) {
                return Err(DomainError::Validation(ValidationError::format(
                    "disability_type",
                    &format!("Invalid disability type '{}'. Valid options: {}", disability_type, valid_disability_types.join(", "))
                )));
            }
        }
        
        // Enhanced location validation
        if let Some(location) = &self.location {
            if location.trim().is_empty() {
                return Err(DomainError::Validation(ValidationError::format(
                    "location",
                    "Location cannot be empty. Please specify a location or remove this field."
                )));
            }
            
            if location.len() > 200 {
                return Err(DomainError::Validation(ValidationError::format(
                    "location",
                    &format!("Location is too long ({} characters). Maximum allowed is 200 characters.", location.len())
                )));
            }
        }
        
        // Business rule: Validate sync priority is sensible for new participants
        if let Some(priority) = &self.sync_priority {
            match priority {
                SyncPriority::Never => {
                    return Err(DomainError::Validation(ValidationError::format(
                        "sync_priority",
                        "New participants cannot be created with Never sync priority. This would prevent synchronization."
                    )));
                }
                _ => {} // Other priorities are acceptable (High, Normal, Low)
            }
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
    pub disability_type: Option<Option<String>>,
    pub age_group: Option<String>,
    pub location: Option<String>,
    pub updated_by_user_id: Uuid,
    pub sync_priority: Option<SyncPriority>,
}

impl Validate for UpdateParticipant {
    fn validate(&self) -> DomainResult<()> {
        // Enhanced name validation with business rules if provided
        if let Some(name) = &self.name {
            ValidationBuilder::new("name", Some(name.clone()))
                .min_length(2)
                .max_length(100)
                .validate()
                .map_err(|e| DomainError::Validation(ValidationError::format(
                    "name", 
                    &format!("Participant name validation failed: {}", e)
                )))?;
                
            // Check for suspicious patterns in name
            if name.trim().chars().any(|c| c.is_numeric() && name.chars().filter(|&x| x.is_numeric()).count() > 2) {
                return Err(DomainError::Validation(ValidationError::format(
                    "name",
                    "Participant name contains too many numbers. Please enter a valid name."
                )));
            }
        }
        
        // Enhanced gender validation with specific error message if provided
        if let Some(gender) = &self.gender {
            common::validate_gender(gender)
                .map_err(|_| DomainError::Validation(ValidationError::format(
                    "gender",
                    &format!("Invalid gender value '{}'. Valid options: male, female, other, prefer_not_to_say", gender)
                )))?;
        }
        
        // Enhanced age group validation with specific error message if provided
        if let Some(age_group) = &self.age_group {
            common::validate_age_group(age_group)
                .map_err(|_| DomainError::Validation(ValidationError::format(
                    "age_group", 
                    &format!("Invalid age group '{}'. Valid options: child, youth, adult, elderly", age_group)
                )))?;
        }
        
        // Business rule: Disability type consistency validation
        // Note: We can't check the current disability state here since this is just an update DTO
        // This validation will be handled at the service layer where we have access to current state
        
        // Enhanced disability type validation if provided
        if let Some(Some(disability_type)) = &self.disability_type {
            if disability_type.trim().is_empty() {
                return Err(DomainError::Validation(ValidationError::format(
                    "disability_type",
                    "Disability type cannot be empty. Please specify the type of disability or remove this field."
                )));
            }
            
            let valid_disability_types = ["visual", "hearing", "physical", "intellectual", "psychosocial", "multiple", "other"];
            let normalized_type = disability_type.to_lowercase();
            if !valid_disability_types.contains(&normalized_type.as_str()) {
                return Err(DomainError::Validation(ValidationError::format(
                    "disability_type",
                    &format!("Invalid disability type '{}'. Valid options: {}", disability_type, valid_disability_types.join(", "))
                )));
            }
        }
        
        // Enhanced location validation if provided
        if let Some(location) = &self.location {
            if location.trim().is_empty() {
                return Err(DomainError::Validation(ValidationError::format(
                    "location",
                    "Location cannot be empty. Please specify a location or remove this field."
                )));
            }
            
            if location.len() > 200 {
                return Err(DomainError::Validation(ValidationError::format(
                    "location",
                    &format!("Location is too long ({} characters). Maximum allowed is 200 characters.", location.len())
                )));
            }
        }
        
        // Business rule: Validate sync priority changes
        if let Some(priority) = &self.sync_priority {
            match priority {
                SyncPriority::Never => {
                    // Log warning for Never priority as it prevents sync
                    // Service layer can add additional validation based on user role
                }
                _ => {} // Other priorities are acceptable (High, Normal, Low)
            }
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
    
    // Enriched fields - populated based on ParticipantInclude options
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documents: Option<Vec<MediaDocumentResponse>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workshop_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub livelihood_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_livelihood_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_workshop_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upcoming_workshop_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workshops: Option<Vec<WorkshopSummary>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub livelihoods: Option<Vec<LivelihoodSummary>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_counts_by_type: Option<HashMap<String, i64>>,
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
            workshop_count: None,
            livelihood_count: None,
            document_count: None,
            active_livelihood_count: None,
            completed_workshop_count: None,
            upcoming_workshop_count: None,
            workshops: None,
            livelihoods: None,
            document_counts_by_type: None,
        }
    }
}

/// Enum to specify related data to include in participant responses - comprehensive like project domain
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParticipantInclude {
    /// Include total workshop count
    WorkshopCount,
    /// Include total livelihood count
    LivelihoodCount,
    /// Include active livelihood count
    ActiveLivelihoodCount,
    /// Include completed workshop count
    CompletedWorkshopCount,
    /// Include upcoming workshop count
    UpcomingWorkshopCount,
    /// Include basic document count
    DocumentCount,
    /// Include document counts grouped by type
    DocumentCountsByType,
    /// Include full document list
    Documents,
    /// Include full workshop list with details
    Workshops,
    /// Include full livelihood list with details
    Livelihoods,
    /// Include all counts but not full data
    AllCounts,
    /// Include everything
    All,
}

impl ParticipantInclude {
    /// Check if this include option requests count data only
    pub fn is_count_only(&self) -> bool {
        matches!(self, 
            ParticipantInclude::WorkshopCount |
            ParticipantInclude::LivelihoodCount |
            ParticipantInclude::ActiveLivelihoodCount |
            ParticipantInclude::CompletedWorkshopCount |
            ParticipantInclude::UpcomingWorkshopCount |
            ParticipantInclude::DocumentCount |
            ParticipantInclude::DocumentCountsByType |
            ParticipantInclude::AllCounts
        )
    }
    
    /// Check if this include option requests full data
    pub fn is_full_data(&self) -> bool {
        matches!(self, 
            ParticipantInclude::Documents |
            ParticipantInclude::Workshops |
            ParticipantInclude::Livelihoods |
            ParticipantInclude::All
        )
    }
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
    // Basic counts
    pub total_participants: i64,
    pub active_participants: i64, // Non-deleted
    pub deleted_participants: i64,
    
    // Demographic breakdowns
    pub by_gender: HashMap<String, i64>,
    pub by_age_group: HashMap<String, i64>,
    pub by_location: HashMap<String, i64>,
    pub by_disability: HashMap<String, i64>,
    pub by_disability_type: HashMap<String, i64>,
    
    // Engagement statistics
    pub participants_with_workshops: i64,
    pub participants_with_livelihoods: i64,
    pub participants_with_documents: i64,
    pub participants_with_no_engagement: i64,
    
    // Workshop engagement
    pub avg_workshops_per_participant: f64,
    pub max_workshops_per_participant: i64,
    pub participants_by_workshop_count: HashMap<i64, i64>, // workshop_count -> participant_count
    
    // Livelihood engagement  
    pub avg_livelihoods_per_participant: f64,
    pub max_livelihoods_per_participant: i64,
    pub participants_by_livelihood_count: HashMap<i64, i64>, // livelihood_count -> participant_count
    
    // Document engagement
    pub avg_documents_per_participant: f64,
    pub max_documents_per_participant: i64,
    pub participants_by_document_count: HashMap<i64, i64>, // document_count -> participant_count
    pub document_types_usage: HashMap<String, i64>, // document_type -> usage_count
    
    // Temporal statistics
    pub participants_added_this_month: i64,
    pub participants_added_this_year: i64,
    pub monthly_registration_trend: HashMap<String, i64>, // "YYYY-MM" -> count
    
    // Data quality metrics
    pub participants_missing_gender: i64,
    pub participants_missing_age_group: i64,
    pub participants_missing_location: i64,
    pub data_completeness_percentage: f64,
    
    // Last updated timestamp
    pub generated_at: DateTime<Utc>,
}

impl ParticipantDemographics {
    /// Create a new demographics struct with sensible defaults
    pub fn new() -> Self {
        Self {
            total_participants: 0,
            active_participants: 0,
            deleted_participants: 0,
            by_gender: HashMap::new(),
            by_age_group: HashMap::new(),
            by_location: HashMap::new(),
            by_disability: HashMap::new(),
            by_disability_type: HashMap::new(),
            participants_with_workshops: 0,
            participants_with_livelihoods: 0,
            participants_with_documents: 0,
            participants_with_no_engagement: 0,
            avg_workshops_per_participant: 0.0,
            max_workshops_per_participant: 0,
            participants_by_workshop_count: HashMap::new(),
            avg_livelihoods_per_participant: 0.0,
            max_livelihoods_per_participant: 0,
            participants_by_livelihood_count: HashMap::new(),
            avg_documents_per_participant: 0.0,
            max_documents_per_participant: 0,
            participants_by_document_count: HashMap::new(),
            document_types_usage: HashMap::new(),
            participants_added_this_month: 0,
            participants_added_this_year: 0,
            monthly_registration_trend: HashMap::new(),
            participants_missing_gender: 0,
            participants_missing_age_group: 0,
            participants_missing_location: 0,
            data_completeness_percentage: 0.0,
            generated_at: Utc::now(),
        }
    }
    
    /// Calculate data completeness percentage based on core fields
    pub fn calculate_data_completeness(&mut self) {
        if self.active_participants == 0 {
            self.data_completeness_percentage = 100.0;
            return;
        }
        
        let total_fields = self.active_participants * 3; // gender, age_group, location
        let missing_fields = self.participants_missing_gender + 
                           self.participants_missing_age_group + 
                           self.participants_missing_location;
        let complete_fields = total_fields - missing_fields;
        
        self.data_completeness_percentage = if total_fields > 0 {
            (complete_fields as f64 / total_fields as f64) * 100.0
        } else {
            100.0
        };
    }
    
    /// Get engagement summary
    pub fn engagement_summary(&self) -> HashMap<String, i64> {
        let mut summary = HashMap::new();
        summary.insert("with_workshops".to_string(), self.participants_with_workshops);
        summary.insert("with_livelihoods".to_string(), self.participants_with_livelihoods);
        summary.insert("with_documents".to_string(), self.participants_with_documents);
        summary.insert("no_engagement".to_string(), self.participants_with_no_engagement);
        summary
    }
    
    /// Get top locations by participant count
    pub fn top_locations(&self, limit: usize) -> Vec<(String, i64)> {
        let mut locations: Vec<_> = self.by_location.iter().collect();
        locations.sort_by(|a, b| b.1.cmp(a.1));
        locations.into_iter()
            .take(limit)
            .map(|(k, v)| (k.clone(), *v))
            .collect()
    }
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

/// Workshop attendance statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkshopAttendance {
    pub total_workshops: i64,
    pub total_participants: i64,
    pub avg_participants_per_workshop: f64,
    pub workshops_by_participant_count: HashMap<i64, i64>, // Map of participant count -> workshop count
    pub participants_by_workshop_count: HashMap<i64, i64>, // Map of workshop count -> participant count
}

/// Document reference summary for a participant - matches project domain's exact signature  
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantDocumentReference {
    pub field_name: String,
    pub display_name: String,
    pub document_id: Option<Uuid>,
    pub filename: Option<String>,
    pub upload_date: Option<DateTime<Utc>>,
    pub file_size: Option<u64>,
}

/// Participant with document timeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantWithDocumentTimeline {
    pub participant: ParticipantResponse,
    pub documents_by_month: HashMap<String, Vec<MediaDocumentResponse>>,
    pub total_document_count: u64,
}

/// Participant with document timeline organized by type - alternative view matching project pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantWithDocumentsByType {
    pub participant: ParticipantResponse,
    pub documents_by_type: HashMap<String, Vec<MediaDocumentResponse>>,
    pub total_document_count: u64,
}

/// Participant activity timeline for engagement tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantActivityTimeline {
    pub participant_id: Uuid,
    pub participant_name: String,
    pub workshop_participation: Vec<ParticipantWorkshopActivity>,
    pub livelihood_progression: Vec<ParticipantLivelihoodActivity>,
    pub document_uploads: Vec<ParticipantDocumentActivity>,
    pub engagement_score: f64,
    pub last_activity_date: Option<DateTime<Utc>>,
}

/// Single participant workshop activity entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantWorkshopActivity {
    pub workshop_id: Uuid,
    pub workshop_name: String,
    pub participation_date: DateTime<Utc>,
    pub pre_evaluation: Option<String>,
    pub post_evaluation: Option<String>,
    pub completion_status: String,
}

/// Single participant livelihood activity entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantLivelihoodActivity {
    pub livelihood_id: Uuid,
    pub livelihood_name: String,
    pub start_date: DateTime<Utc>,
    pub status: String,
    pub progression_milestones: Vec<String>,
}

/// Single participant document activity entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantDocumentActivity {
    pub document_id: Uuid,
    pub document_name: String,
    pub upload_date: DateTime<Utc>,
    pub document_type: String,
    pub linked_field: Option<String>,
}

/// **ADVANCED QUERY RESULT: Participant with enriched relationship data**
/// Optimized structure for dashboard and detailed views with pre-computed counts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantWithEnrichment {
    pub participant: Participant,
    pub workshop_count: i64,
    pub livelihood_count: i64,
    pub active_livelihood_count: i64,
    pub document_count: i64,
    pub recent_document_count: i64, // Documents uploaded in last 30 days
}

/// **ADVANCED ANALYTICS: Participant engagement metrics**
/// Used for performance analytics and dashboard widgets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantEngagementMetrics {
    pub participant_id: Uuid,
    pub engagement_score: f64, // 0-100 score based on activity
    pub workshop_participation_rate: f64, // Percentage of available workshops attended
    pub livelihood_success_rate: f64, // Percentage of livelihoods marked as successful
    pub document_submission_frequency: f64, // Average documents per month
    pub last_activity_date: Option<DateTime<Utc>>,
    pub total_program_duration_days: i64,
}

/// **QUERY OPTIMIZATION: Participant statistics aggregation**
/// Cache-friendly structure for dashboard analytics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantStatistics {
    pub total_participants: i64,
    pub active_participants: i64,
    pub participants_with_disabilities: i64,
    pub by_gender: HashMap<String, i64>,
    pub by_age_group: HashMap<String, i64>,
    pub by_location: HashMap<String, i64>,
    pub by_disability_type: HashMap<String, i64>,
    pub engagement_distribution: HashMap<String, i64>, // High/Medium/Low engagement
    pub monthly_registration_trends: HashMap<String, i64>, // Month -> count
    pub data_completeness: f64, // Percentage of participants with complete profiles
}

/// **BATCH PROCESSING: Bulk operation result**
/// For efficient reporting of batch operations like bulk updates or exports
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantBulkOperationResult {
    pub total_requested: usize,
    pub successful: usize,
    pub failed: usize,
    pub skipped: usize,
    pub error_details: Vec<(Uuid, String)>, // (participant_id, error_message)
    pub operation_duration_ms: u64,
}

/// **PERFORMANCE OPTIMIZATION: Participant search index**
/// Optimized structure for search operations across multiple fields
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantSearchIndex {
    pub participant_id: Uuid,
    pub search_text: String, // Concatenated searchable fields
    pub related_workshop_names: Vec<String>,
    pub related_livelihood_names: Vec<String>,
    pub document_keywords: Vec<String>,
    pub last_indexed_at: DateTime<Utc>,
}

/// Information about a potential duplicate participant for UI duplicate detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantDuplicateInfo {
    pub id: Uuid,
    pub name: String,
    pub gender: Option<String>,
    pub age_group: Option<String>,
    pub location: Option<String>,
    pub disability: bool,
    pub disability_type: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    
    // Document information for duplicate detection
    pub profile_photo_url: Option<String>,
    pub identification_documents: Vec<DuplicateDocumentInfo>,
    pub other_documents: Vec<DuplicateDocumentInfo>,
    pub total_document_count: i64,
    
    // Activity summary
    pub workshop_count: i64,
    pub livelihood_count: i64,
}

/// Document information for duplicate detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateDocumentInfo {
    pub id: Uuid,
    pub original_filename: String,
    pub file_path: String,
    pub linked_field: Option<String>,
    pub document_type_name: Option<String>,
    pub uploaded_at: String,
}