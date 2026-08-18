use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::OnceLock;

use chrono::Utc;
use regex::{Captures, Regex};
use reqwest::Url;
use uuid::Uuid;

use crate::switcher::models::{
    ModelEntry, ProviderProfile, SaveProviderInput, SaveSettingsInput, StoredProviderProfile,
    SwitcherDatabase,
};
use crate::switcher::state::SwitcherPaths;
use crate::switcher::{ApiError, ApiResult};
use crate::utils::dpapi::{protect_to_base64, unprotect_from_base64};

pub struct SwitcherStore<'a> {
    paths: &'a SwitcherPaths,
}

impl<'a> SwitcherStore<'a> {
    pub fn new(paths: &'a SwitcherPaths) -> Self {
        Self { paths }
    }

    pub fn load(&self) -> ApiResult<SwitcherDatabase> {
        self.ensure_layout()?;
        if !self.paths.database_path.exists() {
            return Ok(SwitcherDatabase::default());
        }
        let text = fs::read_to_string(&self.paths.database_path)
            .map_err(|error| io_error("database_read_failed", "读取应用数据失败。", error))?;
        serde_json::from_str(&text).map_err(|error| {
            ApiError::detailed("database_invalid", "应用数据格式无效。", error.to_string())
        })
    }

    pub fn save(&self, database: &SwitcherDatabase) -> ApiResult<()> {
        self.ensure_layout()?;
        let text = serde_json::to_string_pretty(database).map_err(|error| {
            ApiError::detailed(
                "database_serialize_failed",
                "序列化应用数据失败。",
                error.to_string(),
            )
        })?;
        atomic_write(&self.paths.database_path, text.as_bytes())
    }

    pub fn save_provider(&self, input: SaveProviderInput) -> ApiResult<SwitcherDatabase> {
        let mut database = self.load()?;
        let normalized = validate_provider_input(&input)?;
        let now = Utc::now();

        if let Some(id) = input.id.as_deref() {
            let profile = database
                .profiles
                .iter_mut()
                .find(|profile| profile.id == id)
                .ok_or_else(|| {
                    ApiError::new("provider_not_found", "没有找到要编辑的 Provider。")
                })?;

            let protected_api_key = resolve_saved_api_key(
                profile.protected_api_key.clone(),
                input.api_key.as_deref(),
                input.clear_api_key,
            )?;
            profile.name = normalized.name;
            profile.provider_id = normalized.provider_id;
            profile.base_url = normalized.base_url;
            profile.protected_api_key = protected_api_key;
            profile.default_model = normalized.default_model;
            profile.models = normalized.models;
            profile.timeout_seconds = normalized.timeout_seconds;
            profile.inject_models = normalized.inject_models;
            profile.updated_at_utc = now;
        } else {
            let protected_api_key =
                resolve_saved_api_key(None, input.api_key.as_deref(), input.clear_api_key)?;
            database.profiles.push(StoredProviderProfile {
                id: Uuid::new_v4().to_string(),
                name: normalized.name,
                provider_id: normalized.provider_id,
                base_url: normalized.base_url,
                protected_api_key,
                default_model: normalized.default_model,
                models: normalized.models,
                timeout_seconds: normalized.timeout_seconds,
                inject_models: normalized.inject_models,
                created_at_utc: now,
                updated_at_utc: now,
            });
        }

        ensure_unique_provider_ids(&database)?;
        self.save(&database)?;
        Ok(database)
    }

    pub fn duplicate_provider(&self, profile_id: &str) -> ApiResult<SwitcherDatabase> {
        let mut database = self.load()?;
        let source = database
            .profiles
            .iter()
            .find(|profile| profile.id == profile_id)
            .cloned()
            .ok_or_else(|| ApiError::new("provider_not_found", "没有找到要复制的 Provider。"))?;
        let now = Utc::now();
        let provider_id = unique_provider_id(&database, &format!("{}_copy", source.provider_id));
        database.profiles.push(StoredProviderProfile {
            id: Uuid::new_v4().to_string(),
            name: format!("{} 副本", source.name),
            provider_id,
            created_at_utc: now,
            updated_at_utc: now,
            ..source
        });
        self.save(&database)?;
        Ok(database)
    }

    pub fn delete_provider(&self, profile_id: &str) -> ApiResult<SwitcherDatabase> {
        let mut database = self.load()?;
        if database.active_profile_id.as_deref() == Some(profile_id) {
            return Err(ApiError::new(
                "provider_is_active",
                "当前正在使用此 Provider，请先恢复官方模式或切换到其他 Provider。",
            ));
        }
        let previous_len = database.profiles.len();
        database.profiles.retain(|profile| profile.id != profile_id);
        if database.profiles.len() == previous_len {
            return Err(ApiError::new(
                "provider_not_found",
                "没有找到要删除的 Provider。",
            ));
        }
        self.save(&database)?;
        Ok(database)
    }

    pub fn save_settings(&self, input: SaveSettingsInput) -> ApiResult<SwitcherDatabase> {
        let mut database = self.load()?;
        let codex_home_override = input
            .codex_home_override
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        if let Some(path) = codex_home_override.as_deref() {
            let candidate = Path::new(path);
            if !candidate.is_absolute() {
                return Err(ApiError::new(
                    "codex_home_not_absolute",
                    "自定义 CODEX_HOME 必须是绝对路径。",
                ));
            }
        }
        database.settings.codex_home_override = codex_home_override;
        database.settings.session_migration_days = input.session_migration_days.min(30);
        database.settings.backup_retention = input.backup_retention.clamp(1, 50);
        database.settings.inject_models_default = input.inject_models_default;
        self.save(&database)?;
        Ok(database)
    }

    pub fn provider_secret(&self, profile: &StoredProviderProfile) -> ApiResult<String> {
        let protected = profile
            .protected_api_key
            .as_deref()
            .ok_or_else(|| ApiError::new("api_key_missing", "此 Provider 尚未保存 API Key。"))?;
        unprotect_from_base64(protected).map_err(|error| {
            ApiError::detailed(
                "api_key_decrypt_failed",
                "API Key 解密失败。",
                error.to_string(),
            )
        })
    }

    pub fn profiles_for_ui(database: &SwitcherDatabase) -> Vec<ProviderProfile> {
        database
            .profiles
            .iter()
            .map(ProviderProfile::from)
            .collect()
    }

    pub fn append_log(
        &self,
        level: &str,
        action: &str,
        message: &str,
        context: serde_json::Value,
    ) -> ApiResult<()> {
        self.ensure_layout()?;
        let record = serde_json::json!({
            "timestamp": Utc::now(),
            "level": level,
            "action": action,
            "message": sanitize_log_text(message),
            "context": sanitize_context(context),
        });
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.paths.log_path)
            .map_err(|error| io_error("log_open_failed", "打开日志文件失败。", error))?;
        writeln!(
            file,
            "{}",
            serde_json::to_string(&record).unwrap_or_else(|_| "{}".to_string())
        )
        .map_err(|error| io_error("log_write_failed", "写入日志失败。", error))
    }

    fn ensure_layout(&self) -> ApiResult<()> {
        for path in [
            &self.paths.data_root,
            &self.paths.backups_root,
            &self.paths.logs_root,
        ] {
            fs::create_dir_all(path).map_err(|error| {
                io_error("directory_create_failed", "创建应用目录失败。", error)
            })?;
        }
        Ok(())
    }
}

struct NormalizedProviderInput {
    name: String,
    provider_id: String,
    base_url: String,
    default_model: String,
    models: Vec<ModelEntry>,
    timeout_seconds: u64,
    inject_models: bool,
}

fn validate_provider_input(input: &SaveProviderInput) -> ApiResult<NormalizedProviderInput> {
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err(ApiError::new(
            "provider_name_missing",
            "Provider 名称不能为空。",
        ));
    }
    if name.chars().count() > 80 {
        return Err(ApiError::new(
            "provider_name_too_long",
            "Provider 名称不能超过 80 个字符。",
        ));
    }

    let provider_id = input.provider_id.trim().to_ascii_lowercase();
    if provider_id.is_empty()
        || provider_id.len() > 48
        || !provider_id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        return Err(ApiError::new(
            "provider_id_invalid",
            "Provider ID 只能包含小写字母、数字、下划线和短横线，最长 48 位。",
        ));
    }
    if provider_id == "openai" {
        return Err(ApiError::new(
            "provider_id_reserved",
            "openai 是 Codex 内置 Provider ID，请换一个名称。",
        ));
    }

    let base_url = normalize_base_url(&input.base_url)?;
    let default_model = input.default_model.trim().to_string();

    let mut models = Vec::new();
    for item in &input.models {
        let id = item.id.trim();
        if id.is_empty() || models.iter().any(|existing: &ModelEntry| existing.id == id) {
            continue;
        }
        models.push(ModelEntry {
            id: id.to_string(),
            display_name: if item.display_name.trim().is_empty() {
                id.to_string()
            } else {
                item.display_name.trim().to_string()
            },
            context_window: item.context_window.filter(|value| *value > 0),
        });
    }
    if !default_model.is_empty() && !models.iter().any(|model| model.id == default_model) {
        models.insert(
            0,
            ModelEntry {
                id: default_model.clone(),
                display_name: default_model.clone(),
                context_window: Some(128_000),
            },
        );
    }

    Ok(NormalizedProviderInput {
        name,
        provider_id,
        base_url,
        default_model,
        models,
        timeout_seconds: input.timeout_seconds.clamp(5, 180),
        inject_models: input.inject_models,
    })
}

fn normalize_base_url(raw: &str) -> ApiResult<String> {
    let value = raw.trim().trim_end_matches('/');
    let url = Url::parse(value).map_err(|error| {
        ApiError::detailed("base_url_invalid", "API 地址格式无效。", error.to_string())
    })?;
    let host = url.host_str().unwrap_or_default();
    let local_http = url.scheme() == "http" && matches!(host, "localhost" | "127.0.0.1" | "::1");
    if url.scheme() != "https" && !local_http {
        return Err(ApiError::new(
            "base_url_insecure",
            "远程 API 地址必须使用 HTTPS；仅 localhost/127.0.0.1 可以使用 HTTP。",
        ));
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err(ApiError::new(
            "base_url_query_not_allowed",
            "API 地址不能包含查询参数或锚点。",
        ));
    }
    Ok(value.to_string())
}

fn resolve_saved_api_key(
    existing: Option<String>,
    input: Option<&str>,
    clear: bool,
) -> ApiResult<Option<String>> {
    if clear {
        return Ok(None);
    }
    let Some(value) = input.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(existing);
    };
    protect_to_base64(value).map(Some).map_err(|error| {
        ApiError::detailed(
            "api_key_encrypt_failed",
            "API Key 加密失败。",
            error.to_string(),
        )
    })
}

fn ensure_unique_provider_ids(database: &SwitcherDatabase) -> ApiResult<()> {
    for (index, profile) in database.profiles.iter().enumerate() {
        if database.profiles[..index]
            .iter()
            .any(|other| other.provider_id == profile.provider_id)
        {
            return Err(ApiError::new(
                "provider_id_duplicate",
                format!("Provider ID“{}”已存在。", profile.provider_id),
            ));
        }
    }
    Ok(())
}

fn unique_provider_id(database: &SwitcherDatabase, desired: &str) -> String {
    for suffix in 0..1000 {
        let value = if suffix == 0 {
            desired.to_string()
        } else {
            format!("{desired}_{suffix}")
        };
        if !database
            .profiles
            .iter()
            .any(|profile| profile.provider_id == value)
        {
            return value;
        }
    }
    format!("provider_{}", Uuid::new_v4().simple())
}

fn atomic_write(path: &Path, bytes: &[u8]) -> ApiResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| io_error("directory_create_failed", "创建目录失败。", error))?;
    }
    let temp_path = path.with_extension("tmp");
    {
        let mut file = fs::File::create(&temp_path)
            .map_err(|error| io_error("temp_file_create_failed", "创建临时文件失败。", error))?;
        file.write_all(bytes)
            .map_err(|error| io_error("temp_file_write_failed", "写入临时文件失败。", error))?;
        file.sync_all()
            .map_err(|error| io_error("temp_file_sync_failed", "同步临时文件失败。", error))?;
    }
    replace_file(&temp_path, path)
}

#[cfg(windows)]
fn replace_file(source: &Path, destination: &Path) -> ApiResult<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };

    let source_wide: Vec<u16> = source
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let destination_wide: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let flags = MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH;
    unsafe {
        MoveFileExW(
            PCWSTR(source_wide.as_ptr()),
            PCWSTR(destination_wide.as_ptr()),
            flags,
        )
        .map_err(|error| {
            ApiError::detailed(
                "atomic_replace_failed",
                format!("原子替换文件失败：{}", destination.display()),
                error.to_string(),
            )
        })
    }
}

#[cfg(not(windows))]
fn replace_file(source: &Path, destination: &Path) -> ApiResult<()> {
    if destination.exists() {
        fs::remove_file(destination)
            .map_err(|error| io_error("atomic_replace_remove_failed", "替换旧文件失败。", error))?;
    }
    fs::rename(source, destination)
        .map_err(|error| io_error("atomic_replace_failed", "原子替换文件失败。", error))
}

fn sanitize_context(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::String(text) => serde_json::Value::String(sanitize_log_text(&text)),
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.into_iter().map(sanitize_context).collect())
        }
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.into_iter()
                .map(|(key, value)| {
                    let sensitive = matches!(
                        key.to_ascii_lowercase().as_str(),
                        "apikey" | "api_key" | "authorization" | "token" | "secret"
                    );
                    (
                        key,
                        if sensitive {
                            serde_json::Value::String("***".to_string())
                        } else {
                            sanitize_context(value)
                        },
                    )
                })
                .collect(),
        ),
        other => other,
    }
}

fn sanitize_log_text(value: &str) -> String {
    static BEARER_RE: OnceLock<Regex> = OnceLock::new();
    static TOKEN_RE: OnceLock<Regex> = OnceLock::new();
    static USER_PATH_RE: OnceLock<Regex> = OnceLock::new();

    let bearer_re = BEARER_RE.get_or_init(|| Regex::new(r#"(?i)\bBearer\s+[^\s"',}]+"#).unwrap());
    let token_re =
        TOKEN_RE.get_or_init(|| Regex::new(r"(?i)\b(?:sk|sess)-[A-Za-z0-9._-]+").unwrap());
    let user_path_re =
        USER_PATH_RE.get_or_init(|| Regex::new(r"(?i)([A-Z]:\\Users\\)[^\\/\s]+").unwrap());

    let output = bearer_re.replace_all(value, "Bearer ***");
    let output = token_re.replace_all(&output, "***");
    user_path_re
        .replace_all(&output, |captures: &Captures<'_>| {
            format!("{}***", &captures[1])
        })
        .into_owned()
}

fn io_error(code: &str, message: &str, error: std::io::Error) -> ApiError {
    ApiError::detailed(code, message, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{normalize_base_url, sanitize_log_text, validate_provider_input};
    use crate::switcher::models::SaveProviderInput;

    #[test]
    fn base_url_accepts_https_and_local_http() {
        assert_eq!(
            normalize_base_url("https://example.com/v1/").unwrap(),
            "https://example.com/v1"
        );
        assert!(normalize_base_url("http://127.0.0.1:8080/v1").is_ok());
    }

    #[test]
    fn base_url_rejects_remote_http() {
        assert!(normalize_base_url("http://example.com/v1").is_err());
    }

    #[test]
    fn provider_draft_allows_empty_default_model_before_discovery() {
        let normalized = validate_provider_input(&SaveProviderInput {
            id: None,
            name: "Local API".to_string(),
            provider_id: "local_api".to_string(),
            base_url: "http://127.0.0.1:8080".to_string(),
            api_key: None,
            clear_api_key: false,
            default_model: String::new(),
            models: Vec::new(),
            timeout_seconds: 30,
            inject_models: true,
        })
        .unwrap();

        assert!(normalized.default_model.is_empty());
        assert!(normalized.models.is_empty());
    }

    #[test]
    fn log_sanitizer_masks_tokens_and_user_paths() {
        let sanitized = sanitize_log_text(
            r#"Authorization: Bearer abc.def, key=sk-example path=C:\Users\test-user\.codex sess-demo"#,
        );
        assert_eq!(
            sanitized,
            r#"Authorization: Bearer ***, key=*** path=C:\Users\***\.codex ***"#
        );
    }
}
