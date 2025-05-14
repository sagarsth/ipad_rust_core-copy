use crate::errors::{ServiceError, DomainError, ValidationError};

/// Utility function to sanitize SQL identifiers
pub fn sanitize_identifier(identifier: &str) -> String {
    // Only allow alphanumerics and underscores in identifiers
    // This prevents SQL injection in dynamic queries
    let safe_id: String = identifier.chars()
        .filter(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    
    // Ensure identifier is not empty after filtering
    if safe_id.is_empty() {
        return "_invalid".to_string();
    }
    
    // Prevent numeric-only identifiers (not valid in SQL)
    if safe_id.chars().all(|c| c.is_numeric()) {
        return format!("_{}", safe_id);
    }
    
    safe_id
}

/// Utility function to validate entity type
pub fn validate_entity_type(entity_type: &str) -> Result<(), ServiceError> {
    // Entity type must be in the allowed list
    const ALLOWED_ENTITIES: &[&str] = &[
        "media_documents", "projects", "activities", "workshops", 
        "strategic_goals", "participants", "livelihoods", "donors",
        "status_types", "document_types", "subsequent_grants", "project_funding"
    ];
    
    let sanitized = sanitize_identifier(entity_type);
    if sanitized != entity_type {
        return Err(ServiceError::Domain(DomainError::Validation(ValidationError::Entity(format!(
            "Entity type contains invalid characters: {}", entity_type
        )))));
    }
    
    if !ALLOWED_ENTITIES.contains(&entity_type) {
        return Err(ServiceError::Domain(DomainError::Validation(ValidationError::Entity(format!(
            "Unknown entity type: {}", entity_type
        )))));
    }
    
    Ok(())
}

// Optional: Utility function for logging sync operations
pub fn format_sync_operation(
    operation: &str,
    entity_type: &str,
    entity_id: &str,
    status: &str,
    error: Option<&str>,
) -> String {
    if let Some(err) = error {
        format!("{} {}:{} - {} - Error: {}", operation, entity_type, entity_id, status, err)
    } else {
        format!("{} {}:{} - {}", operation, entity_type, entity_id, status)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_identifier() {
        assert_eq!(sanitize_identifier("valid_table"), "valid_table");
        assert_eq!(sanitize_identifier("projects"), "projects");
        assert_eq!(sanitize_identifier("DROP TABLE users;"), "DROPTABLEusers");
        assert_eq!(sanitize_identifier("123"), "_123");
        assert_eq!(sanitize_identifier(""), "_invalid");
        assert_eq!(sanitize_identifier("!@#$"), "_invalid");
    }

    #[test]
    fn test_validate_entity_type() {
        assert!(validate_entity_type("projects").is_ok());
        assert!(validate_entity_type("media_documents").is_ok());
        assert!(validate_entity_type("invalid_entity").is_err());
        assert!(validate_entity_type("DROP TABLE;").is_err());
    }
}