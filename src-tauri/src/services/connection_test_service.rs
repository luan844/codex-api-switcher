use std::fs;
use std::time::Duration;

use reqwest::StatusCode;
use serde_json::json;
use toml::Value as TomlValue;

use crate::errors::app_error::{AppError, AppResult};
use crate::models::connection::{ConnectionTestResultDto, ConnectionTestStatus};
use crate::models::profile::{CodexAuthMode, CodexProfile, ProviderCategory};
use crate::utils::dpapi::unprotect_from_base64;

pub const DEFAULT_CONNECTION_TEST_MODEL: &str = "gpt-5.4-mini";

pub struct ConnectionTestService {
    client: reqwest::Client,
}

impl ConnectionTestService {
    pub fn new() -> AppResult<Self> {
        Ok(Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(15))
                .build()?,
        })
    }

    pub async fn test_profile(
        &self,
        profile: &CodexProfile,
        override_model: Option<&str>,
    ) -> AppResult<ConnectionTestResultDto> {
        if profile.provider_category != ProviderCategory::ApiKey {
            return Err(AppError::validation(
                "connection_test_provider_unsupported",
                "当前只支持测试 APIKEY 组合。",
            ));
        }

        let base_url = self.resolve_base_url(profile)?;
        let api_key = self.resolve_api_key(profile)?;
        let model = self.resolve_test_model(profile, override_model);
        let endpoint = self.build_responses_endpoint(&base_url)?;
        let first_attempt = self
            .send_responses_request(&endpoint, &api_key, &model, true)
            .await?;

        let (status, body) =
            if should_retry_without_max_output_tokens(first_attempt.0, &first_attempt.1) {
                self.send_responses_request(&endpoint, &api_key, &model, false)
                    .await?
            } else {
                first_attempt
            };

        Ok(create_connection_result(status, body, endpoint, model))
    }

    fn resolve_test_model(&self, profile: &CodexProfile, override_model: Option<&str>) -> String {
        override_model
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| {
                profile
                    .test_model
                    .clone()
                    .filter(|item| !item.trim().is_empty())
            })
            .unwrap_or_else(|| DEFAULT_CONNECTION_TEST_MODEL.to_string())
    }

    pub fn build_responses_endpoint(&self, base_url: &str) -> AppResult<String> {
        let normalized = base_url.trim().trim_end_matches('/');
        let endpoint = if normalized.ends_with("/responses") {
            normalized.to_string()
        } else {
            format!("{normalized}/responses")
        };

        let url = reqwest::Url::parse(&endpoint).map_err(|_| {
            AppError::validation("base_url_invalid", "连接失败：Base URL 格式不正确。")
        })?;
        Ok(url.to_string())
    }

    fn resolve_base_url(&self, profile: &CodexProfile) -> AppResult<String> {
        if let Some(config_path) = profile
            .stored_config_toml_path
            .as_ref()
            .filter(|item| !item.trim().is_empty())
        {
            return self.read_base_url_from_config_toml(config_path);
        }

        if profile.base_url.trim().is_empty() {
            return Err(AppError::validation(
                "base_url_missing",
                "当前组合没有可用的 Base URL。",
            ));
        }

        Ok(profile.base_url.trim().to_string())
    }

    fn resolve_api_key(&self, profile: &CodexProfile) -> AppResult<String> {
        match profile.auth_mode {
            CodexAuthMode::ApiKey => {
                let protected = profile.protected_api_key_base64.as_deref().ok_or_else(|| {
                    AppError::validation("api_key_missing", "当前组合没有保存 API Key。")
                })?;
                let api_key = unprotect_from_base64(protected)?;
                if api_key.trim().is_empty() {
                    return Err(AppError::validation(
                        "api_key_empty",
                        "当前组合里的 API Key 为空。",
                    ));
                }
                Ok(api_key.trim().to_string())
            }
            CodexAuthMode::AuthJsonFile => {
                let auth_path = profile.stored_auth_json_path.as_deref().ok_or_else(|| {
                    AppError::validation("auth_json_missing", "请先选择 auth.json。")
                })?;
                self.read_api_key_from_auth_json(auth_path)
            }
        }
    }

    fn read_base_url_from_config_toml(&self, path: &str) -> AppResult<String> {
        let content = fs::read_to_string(path).map_err(|source| AppError::Io {
            code: "config_toml_read_failed".into(),
            message: "选中的 config.toml 不存在。".into(),
            source,
        })?;
        let document = toml::from_str::<TomlValue>(&content).map_err(|_| {
            AppError::validation(
                "base_url_parse_failed",
                "config.toml 里的 base_url 格式不正确。",
            )
        })?;

        let value = document
            .get("base_url")
            .and_then(TomlValue::as_str)
            .map(ToOwned::to_owned)
            .or_else(|| {
                document
                    .get("model_providers")
                    .and_then(find_base_url_in_toml)
            })
            .or_else(|| find_base_url_in_toml(&document))
            .ok_or_else(|| {
                AppError::validation("base_url_not_found", "没有在 config.toml 里找到 base_url。")
            })?;

        if value.trim().is_empty() {
            return Err(AppError::validation(
                "base_url_empty",
                "config.toml 里的 base_url 为空。",
            ));
        }

        Ok(value.trim().to_string())
    }

    fn read_api_key_from_auth_json(&self, path: &str) -> AppResult<String> {
        let content = fs::read_to_string(path).map_err(|source| AppError::Io {
            code: "auth_json_read_failed".into(),
            message: "选中的 auth.json 不存在。".into(),
            source,
        })?;
        let document = serde_json::from_str::<serde_json::Value>(&content)?;
        let api_key = document
            .get("OPENAI_API_KEY")
            .and_then(|item| item.as_str())
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .ok_or_else(|| {
                AppError::validation(
                    "auth_json_key_missing",
                    "auth.json 里没有找到 OPENAI_API_KEY。",
                )
            })?;
        Ok(api_key.to_string())
    }

    async fn send_responses_request(
        &self,
        endpoint: &str,
        api_key: &str,
        model: &str,
        include_max_output_tokens: bool,
    ) -> AppResult<(StatusCode, String)> {
        let input = json!([
            {
                "role": "user",
                "content": [
                    {
                        "type": "input_text",
                        "text": "ping"
                    }
                ]
            }
        ]);

        let payload = if include_max_output_tokens {
            json!({
                "model": model,
                "input": input,
                "max_output_tokens": 1
            })
        } else {
            json!({
                "model": model,
                "input": input
            })
        };

        let response = self
            .client
            .post(endpoint)
            .bearer_auth(api_key.trim())
            .json(&payload)
            .send()
            .await?;
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        Ok((status, body))
    }
}

fn should_retry_without_max_output_tokens(status: StatusCode, body: &str) -> bool {
    if status != StatusCode::BAD_REQUEST {
        return false;
    }

    let normalized = body.to_lowercase();
    normalized.contains("max_output_tokens")
        || normalized.contains("unknown parameter")
        || normalized.contains("unknown field")
        || normalized.contains("additional properties")
        || normalized.contains("extra inputs are not permitted")
}

fn create_connection_result(
    status: StatusCode,
    body: String,
    endpoint: String,
    model: String,
) -> ConnectionTestResultDto {
    if status.is_success() {
        return ConnectionTestResultDto {
            status: ConnectionTestStatus::Success,
            endpoint,
            model,
            message: "连接成功，测试模型可用。".into(),
        };
    }

    let detail = try_get_response_message(&body);
    let warning = looks_like_model_issue(status, detail.as_deref().unwrap_or_default(), &body)
        || status == StatusCode::TOO_MANY_REQUESTS;
    let message = if looks_like_model_issue(status, detail.as_deref().unwrap_or_default(), &body) {
        append_detail(
            "已经连到服务，但测试模型不可用。你可以换一个测试模型再试。",
            detail,
        )
    } else if matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN) {
        append_detail(
            "连接失败：认证没有通过，请检查 API Key 或 auth.json。",
            detail,
        )
    } else if status == StatusCode::TOO_MANY_REQUESTS {
        append_detail(
            "连接到了提供商，但当前被限流。一般说明地址和认证信息是通的。",
            detail,
        )
    } else if status == StatusCode::NOT_FOUND {
        append_detail(
            "连接失败：没有找到 responses 接口，请检查 Base URL 是否正确。",
            detail,
        )
    } else {
        append_detail(
            &format!("连接失败：服务返回 {} {}。", status.as_u16(), status),
            detail,
        )
    };

    ConnectionTestResultDto {
        status: if warning {
            ConnectionTestStatus::Warning
        } else {
            ConnectionTestStatus::Failure
        },
        endpoint,
        model,
        message,
    }
}

fn looks_like_model_issue(status: StatusCode, detail: &str, body: &str) -> bool {
    if !matches!(
        status,
        StatusCode::BAD_REQUEST | StatusCode::NOT_FOUND | StatusCode::UNPROCESSABLE_ENTITY
    ) {
        return false;
    }

    let normalized = format!("{detail}\n{body}").to_lowercase();
    normalized.contains("model")
        && (normalized.contains("not found")
            || normalized.contains("does not exist")
            || normalized.contains("unknown model")
            || normalized.contains("unsupported")
            || normalized.contains("not available")
            || normalized.contains("invalid model")
            || normalized.contains("模型不存在")
            || normalized.contains("不支持"))
}

fn try_get_response_message(body: &str) -> Option<String> {
    let document = serde_json::from_str::<serde_json::Value>(body).ok()?;
    if let Some(error) = document.get("error") {
        if let Some(message) = error.get("message").and_then(|item| item.as_str()) {
            return Some(message.to_string());
        }
        if let Some(message) = error.as_str() {
            return Some(message.to_string());
        }
    }

    document
        .get("message")
        .and_then(|item| item.as_str())
        .map(ToOwned::to_owned)
        .or_else(|| {
            document
                .get("detail")
                .and_then(|item| item.as_str())
                .map(ToOwned::to_owned)
        })
}

fn append_detail(message: &str, detail: Option<String>) -> String {
    detail
        .filter(|item| !item.trim().is_empty())
        .map(|item| format!("{message} 详情：{item}"))
        .unwrap_or_else(|| message.to_string())
}

fn find_base_url_in_toml(value: &TomlValue) -> Option<String> {
    match value {
        TomlValue::Table(table) => {
            if let Some(base_url) = table.get("base_url").and_then(TomlValue::as_str) {
                return Some(base_url.to_string());
            }

            table.values().find_map(find_base_url_in_toml)
        }
        TomlValue::Array(items) => items.iter().find_map(find_base_url_in_toml),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn read_base_url_from_nested_toml_provider_table() {
        let workspace = tempdir().unwrap();
        let path = workspace.path().join("config.toml");
        fs::write(
            &path,
            "model_provider = \"right\"\n[model_providers.right]\nbase_url = 'https://example.com/v1'\nwire_api = \"responses\"\n",
        )
        .unwrap();

        let service = ConnectionTestService::new().unwrap();
        let base_url = service
            .read_base_url_from_config_toml(path.to_string_lossy().as_ref())
            .unwrap();

        assert_eq!(base_url, "https://example.com/v1");
    }

    #[test]
    fn read_base_url_returns_validation_error_when_missing() {
        let workspace = tempdir().unwrap();
        let path = workspace.path().join("config.toml");
        fs::write(&path, "model = \"gpt-5.4\"\n").unwrap();

        let service = ConnectionTestService::new().unwrap();
        let error = service
            .read_base_url_from_config_toml(path.to_string_lossy().as_ref())
            .unwrap_err();

        assert_eq!(error.to_dto().code, "base_url_not_found");
    }

    #[test]
    fn override_model_takes_priority_over_profile_default() {
        let service = ConnectionTestService::new().unwrap();
        let profile = CodexProfile {
            test_model: Some("gpt-4.1-mini".into()),
            ..CodexProfile::default()
        };

        let model = service.resolve_test_model(&profile, Some(" custom-model "));

        assert_eq!(model, "custom-model");
    }
}
