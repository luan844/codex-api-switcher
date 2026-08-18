use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::app::AppBootstrap;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ConnectionTestStatus {
    Success,
    Warning,
    Failure,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionTestResultDto {
    pub status: ConnectionTestStatus,
    pub endpoint: String,
    pub model: String,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchConnectionTestInput {
    pub profile_ids: Vec<Uuid>,
    pub override_model: Option<String>,
    pub disable_on_failure: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchConnectionTestItemDto {
    pub profile_id: Uuid,
    pub profile_name: String,
    pub status: ConnectionTestStatus,
    pub endpoint: String,
    pub model: String,
    pub message: String,
    pub auto_disabled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchConnectionTestResponseDto {
    pub results: Vec<BatchConnectionTestItemDto>,
    pub bootstrap: AppBootstrap,
}
