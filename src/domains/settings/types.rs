use crate::errors::{DbError, DomainError, ValidationError};
use chrono::{DateTime, Utc};
use chrono::Timelike;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// Enum for compression timing options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompressionTiming {
    /// Compress files immediately upon upload
    Immediate,
    /// Compress files in the background when the device is idle
    Background,
    /// Only compress files when manually triggered
    Manual,
}

impl CompressionTiming {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "immediate" => Some(CompressionTiming::Immediate),
            "background" => Some(CompressionTiming::Background),
            "manual" => Some(CompressionTiming::Manual),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            CompressionTiming::Immediate => "immediate",
            CompressionTiming::Background => "background",
            CompressionTiming::Manual => "manual",
        }
    }
}

impl From<String> for CompressionTiming {
    fn from(s: String) -> Self {
        Self::from_str(&s).unwrap_or(CompressionTiming::Immediate)
    }
}

impl From<CompressionTiming> for String {
    fn from(timing: CompressionTiming) -> Self {
        timing.as_str().to_string()
    }
}

/// Enum for app theme options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppTheme {
    /// Light mode
    Light,
    /// Dark mode
    Dark,
    /// System default theme
    System,
}

impl AppTheme {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "light" => Some(AppTheme::Light),
            "dark" => Some(AppTheme::Dark),
            "system" => Some(AppTheme::System),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            AppTheme::Light => "light",
            AppTheme::Dark => "dark",
            AppTheme::System => "system",
        }
    }
}

impl From<String> for AppTheme {
    fn from(s: String) -> Self {
        Self::from_str(&s).unwrap_or(AppTheme::System)
    }
}

impl From<AppTheme> for String {
    fn from(theme: AppTheme) -> Self {
        theme.as_str().to_string()
    }
}

/// Global application settings (admin controlled)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub id: String, // Always "global"
    
    // Compression settings
    pub compression_enabled: bool,
    pub compression_enabled_updated_at: Option<String>,
    pub compression_enabled_updated_by: Option<String>,
    
    pub default_compression_timing: String, // Uses CompressionTiming enum
    pub default_compression_timing_updated_at: Option<String>,
    pub default_compression_timing_updated_by: Option<String>,
    
    pub background_service_interval: i32, // Seconds between background service runs
    pub background_service_interval_updated_at: Option<String>,
    pub background_service_interval_updated_by: Option<String>,
    
    // Local state, not synced
    pub last_background_run: Option<String>,
    
    // Standard metadata
    pub created_at: String,
    pub updated_at: String,
    pub created_by_user_id: Option<String>,
    pub updated_by_user_id: Option<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        let now = Utc::now().to_rfc3339();
        
        Self {
            id: "global".to_string(),
            compression_enabled: true,
            compression_enabled_updated_at: None,
            compression_enabled_updated_by: None,
            
            default_compression_timing: "immediate".to_string(),
            default_compression_timing_updated_at: None,
            default_compression_timing_updated_by: None,
            
            background_service_interval: 300, // 5 minutes
            background_service_interval_updated_at: None,
            background_service_interval_updated_by: None,
            
            last_background_run: None,
            created_at: now.clone(),
            updated_at: now,
            created_by_user_id: None,
            updated_by_user_id: None,
        }
    }
}

impl AppSettings {
    pub fn update_last_background_run(&mut self) -> &mut Self {
        self.last_background_run = Some(Utc::now().to_rfc3339());
        self.updated_at = Utc::now().to_rfc3339();
        self
    }
    
    pub fn get_compression_timing(&self) -> CompressionTiming {
        CompressionTiming::from_str(&self.default_compression_timing)
            .unwrap_or(CompressionTiming::Immediate)
    }
    
    pub fn should_run_background_service(&self) -> bool {
        if !self.compression_enabled {
            return false;
        }
        
        if self.get_compression_timing() == CompressionTiming::Manual {
            return false;
        }
        
        match &self.last_background_run {
            Some(last_run_str) => {
                match DateTime::parse_from_rfc3339(last_run_str) {
                    Ok(last_run) => {
                        let now = Utc::now();
                        let duration = now.signed_duration_since(last_run.with_timezone(&Utc));
                        duration.num_seconds() >= i64::from(self.background_service_interval)
                    },
                    Err(_) => true, // If we can't parse the date, run it to be safe
                }
            },
            None => true, // If no last run recorded, then run it
        }
    }
}

/// User-specific sync settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSyncSettings {
    pub user_id: String, // References users.id
    
    pub max_file_size: i64, // Maximum file size in bytes to sync
    pub max_file_size_updated_at: Option<String>,
    pub max_file_size_updated_by: Option<String>,
    
    pub compression_enabled: bool,
    pub compression_enabled_updated_at: Option<String>,
    pub compression_enabled_updated_by: Option<String>,
    
    pub compression_timing: String, // Uses CompressionTiming enum
    pub compression_timing_updated_at: Option<String>,
    pub compression_timing_updated_by: Option<String>,
    
    // Standard metadata
    pub created_at: String,
    pub updated_at: String,
}

impl UserSyncSettings {
    pub fn new(user_id: String) -> Self {
        let now = Utc::now().to_rfc3339();
        
        Self {
            user_id,
            max_file_size: 10 * 1024 * 1024, // 10MB default
            max_file_size_updated_at: None,
            max_file_size_updated_by: None,
            
            compression_enabled: true,
            compression_enabled_updated_at: None,
            compression_enabled_updated_by: None,
            
            compression_timing: "immediate".to_string(),
            compression_timing_updated_at: None,
            compression_timing_updated_by: None,
            
            created_at: now.clone(),
            updated_at: now,
        }
    }
    
    pub fn get_compression_timing(&self) -> CompressionTiming {
        CompressionTiming::from_str(&self.compression_timing)
            .unwrap_or(CompressionTiming::Immediate)
    }
}

/// Connection settings for API and cloud services
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConnectionSettings {
    pub id: String, // Always "cloud"
    pub api_endpoint: String,
    pub api_version: Option<String>,
    pub connection_timeout: Option<i32>, // in milliseconds
    pub offline_mode_enabled: Option<bool>,
    pub retry_count: Option<i32>,
    pub retry_delay: Option<i32>, // in milliseconds
    pub updated_at: String,
}

impl Default for AppConnectionSettings {
    fn default() -> Self {
        Self {
            id: "cloud".to_string(),
            api_endpoint: "https://api.example.org/v1".to_string(), // Default endpoint
            api_version: Some("1.0".to_string()),
            connection_timeout: Some(30000), // 30 seconds
            offline_mode_enabled: Some(false),
            retry_count: Some(3),
            retry_delay: Some(5000), // 5 seconds
            updated_at: Utc::now().to_rfc3339(),
        }
    }
}

/// UI preferences stored locally (not synced)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiPreferences {
    pub user_id: String,
    pub theme: String, // Uses AppTheme enum
    pub font_size: i32, // Base font size in points
    pub high_contrast: bool,
    pub animations_enabled: bool,
    pub notifications_enabled: bool,
    pub last_viewed_tab: Option<String>,
    pub default_view_mode: Option<String>, // e.g., "list", "grid", "map"
    pub language: String, // ISO language code (e.g., "en", "fr")
    pub updated_at: String,
}

impl UiPreferences {
    pub fn new(user_id: String) -> Self {
        Self {
            user_id,
            theme: "system".to_string(),
            font_size: 14,
            high_contrast: false,
            animations_enabled: true,
            notifications_enabled: true,
            last_viewed_tab: None,
            default_view_mode: Some("list".to_string()),
            language: "en".to_string(),
            updated_at: Utc::now().to_rfc3339(),
        }
    }
    
    pub fn get_theme(&self) -> AppTheme {
        AppTheme::from_str(&self.theme).unwrap_or(AppTheme::System)
    }
}

/// Data Transfer Object for updating app settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateAppSettingsDto {
    pub compression_enabled: Option<bool>,
    pub default_compression_timing: Option<String>,
    pub background_service_interval: Option<i32>,
}

/// Data Transfer Object for updating user sync settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateUserSyncSettingsDto {
    pub max_file_size: Option<i64>,
    pub compression_enabled: Option<bool>,
    pub compression_timing: Option<String>,
}

/// Data Transfer Object for updating connection settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateConnectionSettingsDto {
    pub api_endpoint: Option<String>,
    pub api_version: Option<String>,
    pub connection_timeout: Option<i32>,
    pub offline_mode_enabled: Option<bool>,
    pub retry_count: Option<i32>,
    pub retry_delay: Option<i32>,
}

/// Data Transfer Object for updating UI preferences
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateUiPreferencesDto {
    pub theme: Option<String>,
    pub font_size: Option<i32>,
    pub high_contrast: Option<bool>,
    pub animations_enabled: Option<bool>,
    pub notifications_enabled: Option<bool>,
    pub last_viewed_tab: Option<String>,
    pub default_view_mode: Option<String>,
    pub language: Option<String>,
}

/// Configuration for sync scheduling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncScheduleConfig {
    pub user_id: String,
    pub auto_sync_enabled: bool,
    pub wifi_only: bool,
    pub charging_only: Option<bool>,
    pub min_battery_percentage: Option<i32>,
    pub background_sync_interval_minutes: Option<i32>,
    pub quiet_hours_start: Option<i32>, // Hour of day (0-23)
    pub quiet_hours_end: Option<i32>,   // Hour of day (0-23)
    pub max_sync_frequency_minutes: Option<i32>,
    pub allow_metered_connection: Option<bool>,
    pub created_at: String,
    pub updated_at: String,
}

impl SyncScheduleConfig {
    pub fn new(user_id: String) -> Self {
        let now = Utc::now().to_rfc3339();
        
        Self {
            user_id,
            auto_sync_enabled: true,
            wifi_only: true,
            charging_only: Some(false),
            min_battery_percentage: Some(20),
            background_sync_interval_minutes: Some(60), // 1 hour
            quiet_hours_start: Some(22), // 10 PM
            quiet_hours_end: Some(7),    // 7 AM
            max_sync_frequency_minutes: Some(15), // 15 minutes minimum between syncs
            allow_metered_connection: Some(false),
            created_at: now.clone(),
            updated_at: now,
        }
    }
    
    pub fn is_sync_allowed(&self, wifi_available: bool, battery_percentage: i32, is_charging: bool, is_metered: bool) -> bool {
        // Check if auto sync is enabled
        if !self.auto_sync_enabled {
            return false;
        }
        
        // Check network conditions
        if self.wifi_only && !wifi_available {
            return false;
        }
        
        if let Some(allow_metered) = self.allow_metered_connection {
            if !allow_metered && is_metered {
                return false;
            }
        }
        
        // Check battery conditions
        if let Some(min_battery) = self.min_battery_percentage {
            if battery_percentage < min_battery {
                return false;
            }
        }
        
        if let Some(charging_only) = self.charging_only {
            if charging_only && !is_charging {
                return false;
            }
        }
        
        // Check if we're in quiet hours
        if let (Some(start), Some(end)) = (self.quiet_hours_start, self.quiet_hours_end) {
            let current_hour = Utc::now().hour();
            
            // Handle both cases: when quiet hours span midnight and when they don't
            if start < end {
                // Simple case: e.g., 9:00 to 17:00
                if current_hour >= start as u32 && current_hour < end as u32 {
                    return false;
                }
            } else {
                // Spanning midnight: e.g., 22:00 to 07:00
                if current_hour >= start as u32 || current_hour < end as u32 {
                    return false;
                }
            }
        }
        
        true
    }
}

/// Data Transfer Object for updating sync schedule config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSyncScheduleConfigDto {
    pub auto_sync_enabled: Option<bool>,
    pub wifi_only: Option<bool>,
    pub charging_only: Option<bool>,
    pub min_battery_percentage: Option<i32>,
    pub background_sync_interval_minutes: Option<i32>,
    pub quiet_hours_start: Option<i32>,
    pub quiet_hours_end: Option<i32>,
    pub max_sync_frequency_minutes: Option<i32>,
    pub allow_metered_connection: Option<bool>,
}

/// Validation error specific to settings
#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("Invalid compression timing: {0}")]
    InvalidCompressionTiming(String),
    
    #[error("Invalid theme: {0}")]
    InvalidTheme(String),
    
    #[error("Invalid background service interval: {0}")]
    InvalidBackgroundServiceInterval(i32),
    
    #[error("Invalid max file size: {0}")]
    InvalidMaxFileSize(i64),
    
    #[error("Invalid battery percentage: {0}")]
    InvalidBatteryPercentage(i32),
    
    #[error("Invalid hour value: {0}")]
    InvalidHourValue(i32),
    
    #[error("Settings not found")]
    SettingsNotFound,
    
    #[error("User sync settings not found for user: {0}")]
    UserSyncSettingsNotFound(String),
    
    #[error("Insufficient permissions")]
    InsufficientPermissions,
    
    #[error("Database error: {0}")]
    DbError(#[from] DbError),
    
    #[error("Domain error: {0}")]
    DomainError(#[from] DomainError),
    
    #[error("Validation error: {0}")]
    ValidationError(#[from] ValidationError),
}

/// Result type for settings operations
pub type SettingsResult<T> = Result<T, SettingsError>;

/// Validate app settings DTO before updating
pub fn validate_app_settings(dto: &UpdateAppSettingsDto) -> SettingsResult<()> {
    // Validate compression timing if provided
    if let Some(timing) = &dto.default_compression_timing {
        if CompressionTiming::from_str(timing).is_none() {
            return Err(SettingsError::InvalidCompressionTiming(timing.clone()));
        }
    }
    
    // Validate background service interval
    if let Some(interval) = dto.background_service_interval {
        if interval < 60 || interval > 86400 { // Between 1 minute and 24 hours
            return Err(SettingsError::InvalidBackgroundServiceInterval(interval));
        }
    }
    
    Ok(())
}

/// Validate user sync settings DTO before updating
pub fn validate_user_sync_settings(dto: &UpdateUserSyncSettingsDto) -> SettingsResult<()> {
    // Validate max file size
    if let Some(size) = dto.max_file_size {
        if size < 0 || size > 100_000_000 { // 100MB maximum
            return Err(SettingsError::InvalidMaxFileSize(size));
        }
    }
    
    // Validate compression timing if provided
    if let Some(timing) = &dto.compression_timing {
        if CompressionTiming::from_str(timing).is_none() {
            return Err(SettingsError::InvalidCompressionTiming(timing.clone()));
        }
    }
    
    Ok(())
}

/// Validate UI preferences DTO before updating
pub fn validate_ui_preferences(dto: &UpdateUiPreferencesDto) -> SettingsResult<()> {
    // Validate theme if provided
    if let Some(theme) = &dto.theme {
        if AppTheme::from_str(theme).is_none() {
            return Err(SettingsError::InvalidTheme(theme.clone()));
        }
    }
    
    // Validate font size
    if let Some(size) = dto.font_size {
        if size < 8 || size > 32 {
             return Err(SettingsError::ValidationError(ValidationError::range(
                "font_size", 8, 32 // Use range helper
            )));
        }
    }
    
    Ok(())
}

/// Validate sync schedule config DTO before updating
pub fn validate_sync_schedule_config(dto: &UpdateSyncScheduleConfigDto) -> SettingsResult<()> {
    // Validate battery percentage
    if let Some(percentage) = dto.min_battery_percentage {
        if percentage < 0 || percentage > 100 {
            return Err(SettingsError::InvalidBatteryPercentage(percentage));
        }
    }
    
    // Validate quiet hours
    if let Some(hour) = dto.quiet_hours_start {
        if hour < 0 || hour > 23 {
            return Err(SettingsError::InvalidHourValue(hour));
        }
    }
    
    if let Some(hour) = dto.quiet_hours_end {
        if hour < 0 || hour > 23 {
            return Err(SettingsError::InvalidHourValue(hour));
        }
    }
    
    // Validate sync interval
    if let Some(interval) = dto.background_sync_interval_minutes {
        if interval < 15 || interval > 1440 { // Between 15 minutes and 24 hours
            return Err(SettingsError::ValidationError(ValidationError::range(
                "background_sync_interval_minutes", 15, 1440 // Use range helper
            )));
        }
    }
    
    // Validate max frequency
    if let Some(frequency) = dto.max_sync_frequency_minutes {
        if frequency < 1 || frequency > 1440 { // Between 1 minute and 24 hours
            return Err(SettingsError::ValidationError(ValidationError::range(
                "max_sync_frequency_minutes", 1, 1440 // Use range helper
            )));
        }
    }
    
    Ok(())
}