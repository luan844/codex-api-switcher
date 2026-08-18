use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupItem {
    pub target_key: String,
    pub display_name: String,
    pub directory_name: String,
    pub created_at_label: String,
    pub has_auth_json: bool,
    pub has_config_toml: bool,
}
