use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModelEntry {
    pub id: String,
    pub display_name: String,
    pub context_window: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredProviderProfile {
    pub id: String,
    pub name: String,
    pub provider_id: String,
    pub base_url: String,
    pub protected_api_key: Option<String>,
    pub default_model: String,
    pub models: Vec<ModelEntry>,
    pub timeout_seconds: u64,
    pub inject_models: bool,
    pub created_at_utc: DateTime<Utc>,
    pub updated_at_utc: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderProfile {
    pub id: String,
    pub name: String,
    pub provider_id: String,
    pub base_url: String,
    pub has_api_key: bool,
    pub default_model: String,
    pub models: Vec<ModelEntry>,
    pub timeout_seconds: u64,
    pub inject_models: bool,
    pub created_at_utc: DateTime<Utc>,
    pub updated_at_utc: DateTime<Utc>,
}

impl From<&StoredProviderProfile> for ProviderProfile {
    fn from(value: &StoredProviderProfile) -> Self {
        Self {
            id: value.id.clone(),
            name: value.name.clone(),
            provider_id: value.provider_id.clone(),
            base_url: value.base_url.clone(),
            has_api_key: value.protected_api_key.is_some(),
            default_model: value.default_model.clone(),
            models: value.models.clone(),
            timeout_seconds: value.timeout_seconds,
            inject_models: value.inject_models,
            created_at_utc: value.created_at_utc,
            updated_at_utc: value.updated_at_utc,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveProviderInput {
    pub id: Option<String>,
    pub name: String,
    pub provider_id: String,
    pub base_url: String,
    pub api_key: Option<String>,
    #[serde(default)]
    pub clear_api_key: bool,
    pub default_model: String,
    #[serde(default)]
    pub models: Vec<ModelEntry>,
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
    #[serde(default = "default_true")]
    pub inject_models: bool,
}

fn default_timeout() -> u64 {
    30
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SwitcherSettings {
    pub codex_home_override: Option<String>,
    pub session_migration_days: u32,
    pub backup_retention: usize,
    pub inject_models_default: bool,
}

impl Default for SwitcherSettings {
    fn default() -> Self {
        Self {
            codex_home_override: None,
            session_migration_days: 3,
            backup_retention: 10,
            inject_models_default: true,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveSettingsInput {
    pub codex_home_override: Option<String>,
    pub session_migration_days: u32,
    pub backup_retention: usize,
    pub inject_models_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct OfficialSnapshot {
    pub config_text: Option<String>,
    pub protected_auth_text: Option<String>,
    pub captured_at_utc: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SwitcherDatabase {
    pub schema_version: u32,
    pub profiles: Vec<StoredProviderProfile>,
    pub settings: SwitcherSettings,
    pub active_profile_id: Option<String>,
    pub active_provider_id: Option<String>,
    pub official_snapshot: OfficialSnapshot,
}

impl Default for SwitcherDatabase {
    fn default() -> Self {
        Self {
            schema_version: 1,
            profiles: Vec::new(),
            settings: SwitcherSettings::default(),
            active_profile_id: None,
            active_provider_id: None,
            official_snapshot: OfficialSnapshot::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupFileRecord {
    pub relative_path: String,
    pub existed_before: bool,
    pub sha256_before: Option<String>,
    pub sha256_after: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupManifest {
    pub id: String,
    pub created_at_utc: DateTime<Utc>,
    pub kind: String,
    pub profile_id: Option<String>,
    pub profile_name: Option<String>,
    pub codex_home: String,
    pub status: String,
    pub files: Vec<BackupFileRecord>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupSummary {
    pub id: String,
    pub created_at_utc: DateTime<Utc>,
    pub kind: String,
    pub profile_name: Option<String>,
    pub status: String,
    pub file_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexRuntimeStatus {
    pub installed: bool,
    pub running: bool,
    pub version: Option<String>,
    pub executable_path: Option<String>,
    pub codex_home: String,
    pub active_provider_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitcherBootstrap {
    pub profiles: Vec<ProviderProfile>,
    pub settings: SwitcherSettings,
    pub active_profile_id: Option<String>,
    pub backups: Vec<BackupSummary>,
    pub runtime: CodexRuntimeStatus,
    pub app_version: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelDiscoveryResult {
    pub endpoint: String,
    pub models: Vec<ModelEntry>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionTestResult {
    pub endpoint: String,
    pub model: String,
    pub latency_ms: u128,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchStep {
    pub phase: String,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchResult {
    pub success: bool,
    pub profile_id: Option<String>,
    pub profile_name: String,
    pub backup_id: Option<String>,
    pub restarted_codex: bool,
    pub injected_models: bool,
    pub warnings: Vec<String>,
    pub steps: Vec<SwitchStep>,
    pub bootstrap: SwitcherBootstrap,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchPreview {
    pub profile_id: String,
    pub profile_name: String,
    pub provider_id: String,
    pub codex_home: String,
    pub files_to_backup: Vec<String>,
    pub session_migration_days: u32,
    pub codex_running: bool,
    pub will_restart_codex: bool,
    pub will_inject_models: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteSwitchInput {
    pub profile_id: String,
    #[serde(default)]
    pub force_close: bool,
    #[serde(default = "default_true")]
    pub restart_codex: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreBackupInput {
    pub backup_id: String,
    #[serde(default)]
    pub force: bool,
}
