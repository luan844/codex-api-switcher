use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SwitchExecutionStatus {
    Success,
    Failure,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SwitchStepStatus {
    Success,
    Skipped,
    Failure,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchExecutionStep {
    pub name: String,
    pub detail: String,
    pub status: SwitchStepStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchExecutionSummary {
    pub executed_at_utc: DateTime<Utc>,
    pub profile_id: Uuid,
    pub profile_name: String,
    pub targets: Vec<String>,
    pub message: String,
    pub status: SwitchExecutionStatus,
    pub steps: Vec<SwitchExecutionStep>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchTargetPreview {
    pub target_key: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchPreview {
    pub profile_id: Uuid,
    pub profile_name: String,
    pub provider_name: String,
    pub targets: Vec<SwitchTargetPreview>,
    pub warnings: Vec<String>,
    pub steps: Vec<String>,
}
