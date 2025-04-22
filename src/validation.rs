use crate::errors::{ValidationError, DomainResult, DomainError};
use std::collections::HashMap;
use uuid::Uuid;
use chrono::{DateTime, Utc, NaiveDate};
use regex::Regex;
use std::sync::OnceLock;
use sqlx::{query_scalar, SqlitePool};
use serde::{Serialize, Deserialize};

/// A trait that entities should implement for validation.
pub trait Validate {
    /// Validates the entity and returns an error if validation fails.
    fn validate(&self) -> DomainResult<()>;
}

// Common regex patterns
fn email_regex() -> &'static Regex {
    static EMAIL_REGEX: OnceLock<Regex> = OnceLock::new();
    EMAIL_REGEX.get_or_init(|| Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").unwrap())
}

fn phone_regex() -> &'static Regex {
    static PHONE_REGEX: OnceLock<Regex> = OnceLock::new();
    PHONE_REGEX.get_or_init(|| Regex::new(r"^\+?[0-9]{8,15}$").unwrap())
}

fn uuid_regex() -> &'static Regex {
    static UUID_REGEX: OnceLock<Regex> = OnceLock::new();
    UUID_REGEX.get_or_init(|| Regex::new(r"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$").unwrap())
}

/// Struct for configuring validations in a fluent style
#[derive(Default)]
pub struct ValidationBuilder<T> {
    field_name: String,
    value: Option<T>,
    errors: Vec<ValidationError>,
}

/// Helper struct for validating nested objects with different validation rules
pub struct NestedValidator {
    errors: Vec<ValidationError>,
}

impl NestedValidator {
    pub fn new() -> Self {
        Self { errors: Vec::new() }
    }

    pub fn add_error(&mut self, error: ValidationError) {
        self.errors.push(error);
    }

    pub fn add_errors(&mut self, errors: Vec<ValidationError>) {
        self.errors.extend(errors);
    }

    pub fn validate(self) -> DomainResult<()> {
        if self.errors.is_empty() {
            Ok(())
        } else {
            // Choose the first error or combine them if needed
            Err(DomainError::Validation(self.errors[0].clone()))
        }
    }
}

/// Generic validation implementations
impl<T> ValidationBuilder<T> {
    pub fn new(field_name: &str, value: Option<T>) -> Self {
        Self {
            field_name: field_name.to_string(),
            value,
            errors: Vec::new(),
        }
    }

    pub fn required(mut self) -> Self 
    where T: Default + PartialEq {
        if self.value.is_none() || self.value == Some(T::default()) {
            self.errors.push(ValidationError::required(&self.field_name));
        }
        self
    }

    pub fn validate_with<F>(mut self, validator: F) -> Self
    where F: FnOnce(&T) -> Result<(), ValidationError>, T: Clone {
        if let Some(value) = &self.value {
            if let Err(err) = validator(value) {
                self.errors.push(err);
            }
        }
        self
    }

    /// Complete validation and return result
    pub fn validate(self) -> DomainResult<()> {
        if self.errors.is_empty() {
            Ok(())
        } else {
            // Return the first error for simplicity
            Err(DomainError::Validation(self.errors[0].clone()))
        }
    }
}

/// String-specific validations
impl ValidationBuilder<String> {
    pub fn min_length(mut self, min: usize) -> Self {
        if let Some(value) = &self.value {
            if value.len() < min {
                self.errors.push(ValidationError::min_length(&self.field_name, min));
            }
        }
        self
    }

    pub fn max_length(mut self, max: usize) -> Self {
        if let Some(value) = &self.value {
            if value.len() > max {
                self.errors.push(ValidationError::max_length(&self.field_name, max));
            }
        }
        self
    }

    pub fn matches_pattern(mut self, pattern: &Regex, message: &str) -> Self {
        if let Some(value) = &self.value {
            if !pattern.is_match(value) {
                self.errors.push(ValidationError::format(&self.field_name, message));
            }
        }
        self
    }

    pub fn email(self) -> Self {
        self.matches_pattern(email_regex(), "must be a valid email address")
    }

    pub fn phone(self) -> Self {
        self.matches_pattern(phone_regex(), "must be a valid phone number")
    }

    pub fn uuid_string(self) -> Self {
        self.matches_pattern(uuid_regex(), "must be a valid UUID")
    }

    pub fn one_of(mut self, allowed_values: &[&str], message: Option<&str>) -> Self {
        if let Some(value) = &self.value {
            if !allowed_values.contains(&value.as_str()) {
                let reason = message.unwrap_or_else(|| "must be one of the allowed values");
                self.errors.push(ValidationError::invalid_value(&self.field_name, reason));
            }
        }
        self
    }
}

/// Numeric validations
impl<T> ValidationBuilder<T> 
where T: PartialOrd + Clone + std::fmt::Display
{
    pub fn min(mut self, min: T) -> Self {
        if let Some(value) = &self.value {
            if value < &min {
                self.errors.push(ValidationError::range(
                    &self.field_name, 
                    min.to_string(), 
                    "maximum".to_string()
                ));
            }
        }
        self
    }

    pub fn max(mut self, max: T) -> Self {
        if let Some(value) = &self.value {
            if value > &max {
                self.errors.push(ValidationError::range(
                    &self.field_name, 
                    "minimum".to_string(), 
                    max.to_string()
                ));
            }
        }
        self
    }

    pub fn range(mut self, min: T, max: T) -> Self {
        if let Some(value) = &self.value {
            if value < &min || value > &max {
                self.errors.push(ValidationError::range(
                    &self.field_name, 
                    min.to_string(), 
                    max.to_string()
                ));
            }
        }
        self
    }
}

/// Uniqueness validation helper (relies on database access)
pub async fn validate_unique(
    pool: &SqlitePool, 
    table: &str, 
    field: &str, 
    value: &str, 
    exclude_id: Option<&str>,
    field_name: &str,
) -> DomainResult<()> {
    let query = match exclude_id {
        Some(id) => {
            format!(
                "SELECT COUNT(*) FROM {} WHERE {} = ? AND id != ? AND deleted_at IS NULL", 
                table, field
            )
        },
        None => {
            format!(
                "SELECT COUNT(*) FROM {} WHERE {} = ? AND deleted_at IS NULL", 
                table, field
            )
        }
    };

    let count: i64 = match exclude_id {
        Some(id) => {
            query_scalar(&query)
                .bind(value)
                .bind(id)
                .fetch_one(pool)
                .await
                .map_err(|e| DomainError::Database(e.into()))?
        },
        None => {
            query_scalar(&query)
                .bind(value)
                .fetch_one(pool)
                .await
                .map_err(|e| DomainError::Database(e.into()))?
        }
    };

    if count > 0 {
        return Err(DomainError::Validation(ValidationError::unique(field_name)));
    }

    Ok(())
}

/// DateTime validation helpers
impl ValidationBuilder<DateTime<Utc>> {
    pub fn not_in_future(mut self) -> Self {
        if let Some(value) = &self.value {
            let now = Utc::now();
            if value > &now {
                self.errors.push(ValidationError::invalid_value(
                    &self.field_name, 
                    "cannot be in the future"
                ));
            }
        }
        self
    }

    pub fn after(mut self, date: DateTime<Utc>) -> Self {
        if let Some(value) = &self.value {
            if value <= &date {
                self.errors.push(ValidationError::invalid_value(
                    &self.field_name, 
                    &format!("must be after {}", date.to_rfc3339())
                ));
            }
        }
        self
    }
}

/// UUID validation helpers
impl ValidationBuilder<Uuid> {
    pub fn not_nil(mut self) -> Self {
        if let Some(value) = &self.value {
            if *value == Uuid::nil() {
                self.errors.push(ValidationError::invalid_value(
                    &self.field_name, 
                    "cannot be a nil UUID"
                ));
            }
        }
        self
    }
}

/// File extension validation helper
pub fn validate_file_extension(filename: &str, allowed_extensions: &[&str]) -> bool {
    if let Some(extension) = filename.split('.').last() {
        allowed_extensions.iter().any(|&ext| ext.eq_ignore_ascii_case(extension))
    } else {
        false
    }
}

/// Helper for validating file sizes
pub fn validate_file_size(size: usize, max_size: usize) -> bool {
    size <= max_size
}

/// Validation utility for checking entity exists in the database
pub async fn validate_entity_exists(
    pool: &SqlitePool,
    table: &str,
    id: &Uuid,
    field_name: &str,
) -> DomainResult<()> {
    let query = format!(
        "SELECT COUNT(*) FROM {} WHERE id = ? AND deleted_at IS NULL",
        table
    );

    let count: i64 = query_scalar(&query)
        .bind(id.to_string())
        .fetch_one(pool)
        .await
        .map_err(|e| DomainError::Database(e.into()))?;

    if count == 0 {
        return Err(DomainError::Validation(
            ValidationError::relationship(&format!("{} does not exist", field_name))
        ));
    }

    Ok(())
}

/// Validation utility for checking dependencies before deletion
pub async fn validate_no_dependencies(
    pool: &SqlitePool,
    table: &str,
    foreign_key: &str,
    id: &Uuid,
) -> DomainResult<()> {
    let query = format!(
        "SELECT COUNT(*) FROM {} WHERE {} = ? AND deleted_at IS NULL",
        table, foreign_key
    );

    let count: i64 = query_scalar(&query)
        .bind(id.to_string())
        .fetch_one(pool)
        .await
        .map_err(|e| DomainError::Database(e.into()))?;

    if count > 0 {
        return Err(DomainError::DependentRecordsExist {
            entity_type: table.to_string(),
            id: *id,
            dependencies: vec![table.to_string()],
        });
    }

    Ok(())
}

/// Helper to check all dependencies before deletion
pub async fn check_all_dependencies(
    pool: &SqlitePool,
    entity_type: &str,
    id: &Uuid,
    dependencies: &[(&str, &str)],
) -> Result<Vec<String>, DomainError> {
    let mut found_dependencies = Vec::new();

    for (table, foreign_key) in dependencies {
        let query = format!(
            "SELECT COUNT(*) FROM {} WHERE {} = ? AND deleted_at IS NULL",
            table, foreign_key
        );

        let count: i64 = query_scalar(&query)
            .bind(id.to_string())
            .fetch_one(pool)
            .await
            .map_err(|e| DomainError::Database(e.into()))?;

        if count > 0 {
            found_dependencies.push(table.to_string());
        }
    }

    Ok(found_dependencies)
}

/// Strongly typed wrapper models for validated input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Email(pub String);

impl Email {
    pub fn new(email: &str) -> Result<Self, ValidationError> {
        if email_regex().is_match(email) {
            Ok(Email(email.to_string()))
        } else {
            Err(ValidationError::format("email", "must be a valid email address"))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhoneNumber(pub String);

impl PhoneNumber {
    pub fn new(phone: &str) -> Result<Self, ValidationError> {
        if phone_regex().is_match(phone) {
            Ok(PhoneNumber(phone.to_string()))
        } else {
            Err(ValidationError::format("phone", "must be a valid phone number"))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NonEmptyString(pub String);

impl NonEmptyString {
    pub fn new(value: &str) -> Result<Self, ValidationError> {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            Ok(NonEmptyString(trimmed.to_string()))
        } else {
            Err(ValidationError::required("value"))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositiveNumber<T>(pub T)
where
    T: PartialOrd + Default + Clone;

impl<T> PositiveNumber<T>
where
    T: PartialOrd + Default + Clone,
{
    pub fn new(value: T) -> Result<Self, ValidationError>
    where
        T: PartialOrd + Default + Clone + std::fmt::Display,
    {
        if value > T::default() {
            Ok(PositiveNumber(value))
        } else {
            Err(ValidationError::invalid_value(
                "value",
                "must be a positive number",
            ))
        }
    }

    pub fn get(&self) -> &T {
        &self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidUuid(pub String);

impl ValidUuid {
    pub fn new(uuid_str: &str) -> Result<Self, ValidationError> {
        if uuid_regex().is_match(uuid_str) {
            Ok(ValidUuid(uuid_str.to_string()))
        } else {
            Err(ValidationError::format("uuid", "must be a valid UUID"))
        }
    }

    pub fn parse(&self) -> Result<Uuid, ValidationError> {
        Uuid::parse_str(&self.0).map_err(|_| ValidationError::format("uuid", "invalid UUID format"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// Implement conversions for wrapper types
impl AsRef<str> for Email {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for PhoneNumber {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for NonEmptyString {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for ValidUuid {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

// Common validation utility module for frequently validated entities
pub mod common {
    use super::*;

    pub async fn validate_user_exists(
        pool: &SqlitePool,
        user_id: &Uuid,
        field_name: &str,
    ) -> DomainResult<()> {
        validate_entity_exists(pool, "users", user_id, field_name).await
    }

    pub async fn validate_project_exists(
        pool: &SqlitePool,
        project_id: &Uuid,
        field_name: &str,
    ) -> DomainResult<()> {
        validate_entity_exists(pool, "projects", project_id, field_name).await
    }

    pub async fn validate_strategic_goal_exists(
        pool: &SqlitePool,
        goal_id: &Uuid,
        field_name: &str,
    ) -> DomainResult<()> {
        validate_entity_exists(pool, "strategic_goals", goal_id, field_name).await
    }

    pub async fn validate_participant_exists(
        pool: &SqlitePool,
        participant_id: &Uuid,
        field_name: &str,
    ) -> DomainResult<()> {
        validate_entity_exists(pool, "participants", participant_id, field_name).await
    }

    pub async fn validate_status_type_exists(
        pool: &SqlitePool,
        status_id: i64,
        field_name: &str,
    ) -> DomainResult<()> {
        let query = "SELECT COUNT(*) FROM status_types WHERE id = ? AND deleted_at IS NULL";

        let count: i64 = query_scalar(query)
            .bind(status_id)
            .fetch_one(pool)
            .await
            .map_err(|e| DomainError::Database(e.into()))?;

        if count == 0 {
            return Err(DomainError::Validation(
                ValidationError::relationship(&format!("{} is not a valid status", field_name))
            ));
        }

        Ok(())
    }

    pub async fn validate_unique_email(
        pool: &SqlitePool,
        email: &str,
        exclude_id: Option<&str>,
    ) -> DomainResult<()> {
        validate_unique(pool, "users", "email", email, exclude_id, "email").await
    }

    pub fn validate_password_strength(password: &str) -> DomainResult<()> {
        let mut builder = ValidationBuilder::new("password", Some(password.to_string()));
        
        builder = builder.min_length(8);
        
        // Check for complexity (at least one uppercase, one lowercase, one number)
        let has_uppercase = password.chars().any(|c| c.is_uppercase());
        let has_lowercase = password.chars().any(|c| c.is_lowercase());
        let has_digit = password.chars().any(|c| c.is_digit(10));
        
        if !has_uppercase || !has_lowercase || !has_digit {
            builder.errors.push(ValidationError::format(
                "password",
                "must contain at least one uppercase letter, one lowercase letter, and one number",
            ));
        }
        
        builder.validate()
    }

    pub fn validate_age_group(age_group: &str) -> DomainResult<()> {
        ValidationBuilder::new("age_group", Some(age_group.to_string()))
            .one_of(&["child", "youth", "adult", "elderly"], None)
            .validate()
    }

    pub fn validate_gender(gender: &str) -> DomainResult<()> {
        ValidationBuilder::new("gender", Some(gender.to_string()))
            .one_of(&["male", "female", "other", "prefer_not_to_say"], None)
            .validate()
    }

    pub fn validate_date_format(date_str: &str, field_name: &str) -> DomainResult<()> {
        match NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
            Ok(_) => Ok(()),
            Err(_) => Err(DomainError::Validation(ValidationError::format(
                field_name,
                "must be in the format YYYY-MM-DD",
            ))),
        }
    }
    
    pub fn validate_iso8601_datetime(date_str: &str, field_name: &str) -> DomainResult<()> {
        match DateTime::parse_from_rfc3339(date_str) {
            Ok(_) => Ok(()),
            Err(_) => Err(DomainError::Validation(ValidationError::format(
                field_name,
                "must be in ISO 8601 format (YYYY-MM-DDTHH:MM:SS.sssZ)",
            ))),
        }
    }
}

// Test module with comprehensive validation tests
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_email_validation() {
        assert!(email_regex().is_match("user@example.com"));
        assert!(email_regex().is_match("user.name+tag@example.co.uk"));
        assert!(!email_regex().is_match("user@"));
        assert!(!email_regex().is_match("@example.com"));
        assert!(!email_regex().is_match("user@example"));
        
        // Test the wrapper
        assert!(Email::new("valid@example.com").is_ok());
        assert!(Email::new("invalid@").is_err());
    }
    
    #[test]
    fn test_phone_validation() {
        assert!(phone_regex().is_match("1234567890"));
        assert!(phone_regex().is_match("+12345678901"));
        assert!(!phone_regex().is_match("123"));
        assert!(!phone_regex().is_match("abcdefghij"));
        
        // Test the wrapper
        assert!(PhoneNumber::new("1234567890").is_ok());
        assert!(PhoneNumber::new("123").is_err());
    }
    
    #[test]
    fn test_uuid_validation() {
        assert!(uuid_regex().is_match("550e8400-e29b-41d4-a716-446655440000"));
        assert!(!uuid_regex().is_match("not-a-uuid"));
        
        // Test the wrapper
        assert!(ValidUuid::new("550e8400-e29b-41d4-a716-446655440000").is_ok());
        assert!(ValidUuid::new("not-a-uuid").is_err());
    }
    
    #[test]
    fn test_non_empty_string() {
        assert!(NonEmptyString::new("hello").is_ok());
        assert!(NonEmptyString::new("   hello   ").is_ok());
        assert!(NonEmptyString::new("").is_err());
        assert!(NonEmptyString::new("   ").is_err());
    }
    
    #[test]
    fn test_positive_number() {
        assert!(PositiveNumber::new(5).is_ok());
        assert!(PositiveNumber::new(0.1).is_ok());
        assert!(PositiveNumber::new(0).is_err());
        assert!(PositiveNumber::new(-1).is_err());
    }
    
    #[test]
    fn test_validation_builder() {
        // String validations
        let result = ValidationBuilder::new("name", Some("".to_string()))
            .required()
            .validate();
        assert!(result.is_err());
        
        let result = ValidationBuilder::new("name", Some("test".to_string()))
            .required()
            .min_length(5)
            .validate();
        assert!(result.is_err());
        
        let result = ValidationBuilder::new("email", Some("invalid".to_string()))
            .email()
            .validate();
        assert!(result.is_err());
        
        let result = ValidationBuilder::new("email", Some("valid@example.com".to_string()))
            .email()
            .validate();
        assert!(result.is_ok());
        
        // Numeric validations
        let result = ValidationBuilder::new("age", Some(15))
            .min(18)
            .validate();
        assert!(result.is_err());
        
        let result = ValidationBuilder::new("age", Some(25))
            .range(18, 65)
            .validate();
        assert!(result.is_ok());
        
        // Required validation for Option
        let value: Option<String> = None;
        let result = ValidationBuilder::new("name", value)
            .required()
            .validate();
        assert!(result.is_err());
    }
    
    #[test]
    fn test_file_validations() {
        assert!(validate_file_extension("image.jpg", &["jpg", "png", "gif"]));
        assert!(validate_file_extension("image.PNG", &["jpg", "png", "gif"]));
        assert!(!validate_file_extension("image.pdf", &["jpg", "png", "gif"]));
        assert!(!validate_file_extension("image", &["jpg", "png", "gif"]));
        
        assert!(validate_file_size(1000, 2000));
        assert!(!validate_file_size(3000, 2000));
    }

    #[test]
    fn test_common_validations() {
        // Test password strength
        assert!(common::validate_password_strength("Abcdef123").is_ok());
        assert!(common::validate_password_strength("abc123").is_err()); // Too short
        assert!(common::validate_password_strength("ABCDEF123").is_err()); // No lowercase
        assert!(common::validate_password_strength("abcdefghi").is_err()); // No uppercase or digits
        
        // Test age group
        assert!(common::validate_age_group("adult").is_ok());
        assert!(common::validate_age_group("unknown").is_err());
        
        // Test gender
        assert!(common::validate_gender("male").is_ok());
        assert!(common::validate_gender("other").is_ok());
        assert!(common::validate_gender("unknown").is_err());
        
        // Test date format
        assert!(common::validate_date_format("2023-01-01", "date").is_ok());
        assert!(common::validate_date_format("01/01/2023", "date").is_err());
        
        // Test ISO8601 datetime
        assert!(common::validate_iso8601_datetime("2023-01-01T12:00:00Z", "datetime").is_ok());
        assert!(common::validate_iso8601_datetime("2023-01-01", "datetime").is_err());
    }
}