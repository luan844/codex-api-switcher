use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::app::UiPreferences;
use crate::models::switch::SwitchExecutionSummary;

pub const CURRENT_MIGRATION_VERSION: u32 = 1;
pub const DEFAULT_APIKEY_PROVIDER_NAME: &str = "apikey";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ProviderCategory {
    ApiKey,
    OpenAI,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CodexAuthMode {
    AuthJsonFile,
    ApiKey,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CodexProfile {
    pub id: Uuid,
    pub name: String,
    pub base_url: String,
    pub test_model: Option<String>,
    pub auto_disabled: bool,
    pub auto_disabled_reason: Option<String>,
    pub auto_disabled_at_utc: Option<DateTime<Utc>>,
    pub provider_category: ProviderCategory,
    pub auth_mode: CodexAuthMode,
    pub stored_auth_json_path: Option<String>,
    pub protected_api_key_base64: Option<String>,
    pub stored_config_toml_path: Option<String>,
    pub created_at_utc: DateTime<Utc>,
    pub updated_at_utc: DateTime<Utc>,
}

impl Default for CodexProfile {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name: String::new(),
            base_url: String::new(),
            test_model: None,
            auto_disabled: false,
            auto_disabled_reason: None,
            auto_disabled_at_utc: None,
            provider_category: ProviderCategory::ApiKey,
            auth_mode: CodexAuthMode::AuthJsonFile,
            stored_auth_json_path: None,
            protected_api_key_base64: None,
            stored_config_toml_path: None,
            created_at_utc: now,
            updated_at_utc: now,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProfileDatabase {
    pub profiles: Vec<CodexProfile>,
    pub last_selected_profile_id: Option<Uuid>,
    pub replace_windows_target: bool,
    pub replace_wsl_target: bool,
    pub wsl_distro_name: Option<String>,
    pub wsl_user_name: Option<String>,
    pub cached_default_wsl_distro_name: Option<String>,
    pub cached_default_wsl_user_name: Option<String>,
    pub cached_default_wsl_home_directory: Option<String>,
    pub cached_default_wsl_detected_at_utc: Option<DateTime<Utc>>,
    pub cached_default_wsl_error_message: Option<String>,
    pub cached_default_wsl_error_at_utc: Option<DateTime<Utc>>,
    pub session_migration_days: i32,
    pub api_key_provider_name: String,
    pub migration_version: u32,
    pub ui_preferences: UiPreferences,
    pub last_switch_summary: Option<SwitchExecutionSummary>,
}

impl Default for ProfileDatabase {
    fn default() -> Self {
        Self {
            profiles: Vec::new(),
            last_selected_profile_id: None,
            replace_windows_target: true,
            replace_wsl_target: false,
            wsl_distro_name: None,
            wsl_user_name: None,
            cached_default_wsl_distro_name: None,
            cached_default_wsl_user_name: None,
            cached_default_wsl_home_directory: None,
            cached_default_wsl_detected_at_utc: None,
            cached_default_wsl_error_message: None,
            cached_default_wsl_error_at_utc: None,
            session_migration_days: 3,
            api_key_provider_name: DEFAULT_APIKEY_PROVIDER_NAME.to_string(),
            migration_version: 0,
            ui_preferences: UiPreferences::default(),
            last_switch_summary: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexProfileDto {
    pub id: Uuid,
    pub name: String,
    pub base_url: String,
    pub test_model: Option<String>,
    pub auto_disabled: bool,
    pub auto_disabled_reason: Option<String>,
    pub auto_disabled_at_utc: Option<DateTime<Utc>>,
    pub provider_category: ProviderCategory,
    pub auth_mode: CodexAuthMode,
    pub has_stored_auth_json: bool,
    pub has_stored_config_toml: bool,
    pub has_stored_api_key: bool,
    pub created_at_utc: DateTime<Utc>,
    pub updated_at_utc: DateTime<Utc>,
}

impl From<&CodexProfile> for CodexProfileDto {
    fn from(value: &CodexProfile) -> Self {
        Self {
            id: value.id,
            name: value.name.clone(),
            base_url: value.base_url.clone(),
            test_model: value.test_model.clone(),
            auto_disabled: value.auto_disabled,
            auto_disabled_reason: value.auto_disabled_reason.clone(),
            auto_disabled_at_utc: value.auto_disabled_at_utc,
            provider_category: value.provider_category,
            auth_mode: value.auth_mode,
            has_stored_auth_json: value
                .stored_auth_json_path
                .as_ref()
                .is_some_and(|item| !item.trim().is_empty()),
            has_stored_config_toml: value
                .stored_config_toml_path
                .as_ref()
                .is_some_and(|item| !item.trim().is_empty()),
            has_stored_api_key: value
                .protected_api_key_base64
                .as_ref()
                .is_some_and(|item| !item.trim().is_empty()),
            created_at_utc: value.created_at_utc,
            updated_at_utc: value.updated_at_utc,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveProfileInput {
    pub id: Option<Uuid>,
    pub name: String,
    pub provider_category: ProviderCategory,
    pub import_config_toml: bool,
    pub base_url: Option<String>,
    pub config_toml_source_path: Option<String>,
    pub auth_mode: CodexAuthMode,
    pub auth_json_source_path: Option<String>,
    pub api_key: Option<String>,
    pub test_model: Option<String>,
}
