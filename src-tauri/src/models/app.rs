use serde::{Deserialize, Serialize};

use crate::models::backup::BackupItem;
use crate::models::profile::CodexProfileDto;
use crate::models::switch::SwitchExecutionSummary;
use crate::models::template::TemplateSnapshot;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct UiPreferences {
    pub sidebar_collapsed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WslEnvironmentInfo {
    pub distro_name: String,
    pub user_name: String,
    pub home_directory: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedWslEnvironmentInfo {
    pub distro_name: String,
    pub user_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsSnapshot {
    pub replace_windows_target: bool,
    pub replace_wsl_target: bool,
    pub wsl_distro_name: Option<String>,
    pub wsl_user_name: Option<String>,
    pub cached_default_wsl: Option<CachedWslEnvironmentInfo>,
    pub cached_default_wsl_error_message: Option<String>,
    pub session_migration_days: i32,
    pub api_key_provider_name: String,
    pub ui_preferences: UiPreferences,
    pub migration_version: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveSettingsInput {
    pub replace_windows_target: bool,
    pub replace_wsl_target: bool,
    pub wsl_distro_name: Option<String>,
    pub wsl_user_name: Option<String>,
    pub session_migration_days: i32,
    pub api_key_provider_name: String,
    pub ui_preferences: UiPreferences,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PickFileInput {
    pub title: Option<String>,
    pub filter_name: String,
    pub extensions: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupOverview {
    pub target_key: String,
    pub display_name: String,
    pub has_recent_backup: bool,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardSnapshot {
    pub active_profile_id: Option<String>,
    pub active_profile_name: Option<String>,
    pub selected_target_labels: Vec<String>,
    pub wsl_status: String,
    pub backup_overview: Vec<BackupOverview>,
    pub last_switch_summary: Option<SwitchExecutionSummary>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppMeta {
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub action: String,
    pub message: String,
    pub context: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppBootstrap {
    pub profiles: Vec<CodexProfileDto>,
    pub settings: SettingsSnapshot,
    pub dashboard: DashboardSnapshot,
    pub templates: TemplateSnapshot,
    pub backups: Vec<BackupItem>,
    pub app_meta: AppMeta,
}
