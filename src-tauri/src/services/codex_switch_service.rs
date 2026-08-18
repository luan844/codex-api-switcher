use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use chrono::Utc;
use regex::Regex;
use serde_json::json;
use uuid::Uuid;

use crate::errors::app_error::{AppError, AppResult};
use crate::models::profile::{
    CodexAuthMode, CodexProfile, ProfileDatabase, ProviderCategory, DEFAULT_APIKEY_PROVIDER_NAME,
};
use crate::models::switch::{
    SwitchExecutionStatus, SwitchExecutionStep, SwitchExecutionSummary, SwitchPreview,
    SwitchStepStatus, SwitchTargetPreview,
};
use crate::services::backup_service::BackupService;
use crate::services::log_service::LogService;
use crate::services::profile_store::ProfileStore;
use crate::services::session_migration_service::SessionMigrationService;
use crate::services::target_service::{TargetContext, TargetService};
use crate::services::template_service::TemplateService;
use crate::state::app_state::AppPaths;
use crate::utils::dpapi::unprotect_from_base64;

pub struct CodexSwitchService<'a> {
    paths: &'a AppPaths,
}

enum PreparedFilePayload {
    Copy(PathBuf),
    Text(String),
}

struct PreparedSwitchPayloads {
    auth_json: PreparedFilePayload,
    config_toml: PreparedFilePayload,
}

#[derive(Debug, Clone)]
struct TargetBackupSnapshot {
    target: TargetContext,
    backup_dir: Option<PathBuf>,
}

impl<'a> CodexSwitchService<'a> {
    pub fn new(paths: &'a AppPaths) -> Self {
        Self { paths }
    }

    pub fn build_switch_preview(&self, profile_id: Uuid) -> AppResult<SwitchPreview> {
        let store = ProfileStore::new(self.paths);
        let database = store.load()?;
        let profile = find_profile(&database, profile_id)?;
        let provider_name = effective_api_key_provider_name(&database)?;
        let target_service = TargetService::new(self.paths);
        let targets = target_service.resolve_selected_targets(&database)?;

        Ok(SwitchPreview {
            profile_id,
            profile_name: profile.name.clone(),
            provider_name,
            targets: targets
                .iter()
                .map(|target| SwitchTargetPreview {
                    target_key: target.target_key.clone(),
                    display_name: target.display_name.clone(),
                })
                .collect(),
            warnings: targets
                .iter()
                .filter(|target| !target.codex_directory_path.exists())
                .map(|target| format!("{} 的目标目录当前不可用。", target.display_name))
                .collect(),
            steps: vec![
                "validate".into(),
                "backup".into(),
                "write auth".into(),
                "write config".into(),
                "migrate sessions".into(),
                "finalize".into(),
            ],
        })
    }

    pub fn execute_switch(&self, profile_id: Uuid) -> AppResult<SwitchExecutionSummary> {
        let store = ProfileStore::new(self.paths);
        let mut database = store.load()?;
        let target_service = TargetService::new(self.paths);
        let backup_service = BackupService::new(self.paths);
        let template_service = TemplateService::new(self.paths);
        let session_service = SessionMigrationService::new();
        let logger = LogService::new(self.paths);
        let profile = find_profile(&database, profile_id)?.clone();
        let provider_name = match effective_api_key_provider_name(&database) {
            Ok(value) => value,
            Err(error) => {
                persist_failure_summary(
                    &store,
                    &logger,
                    &mut database,
                    &profile,
                    &[],
                    Vec::new(),
                    &error,
                );
                return Err(error);
            }
        };
        let targets = match target_service.resolve_selected_targets(&database) {
            Ok(value) => value,
            Err(error) => {
                persist_failure_summary(
                    &store,
                    &logger,
                    &mut database,
                    &profile,
                    &[],
                    Vec::new(),
                    &error,
                );
                return Err(error);
            }
        };
        let mut steps = Vec::new();

        if let Err(error) = self.execute_switch_pipeline(
            &profile,
            &provider_name,
            &targets,
            &backup_service,
            &template_service,
            &session_service,
            &mut steps,
        ) {
            persist_failure_summary(
                &store,
                &logger,
                &mut database,
                &profile,
                &targets,
                steps,
                &error,
            );
            return Err(error);
        }

        let message = format!(
            "切换完成：{}。",
            targets
                .iter()
                .map(|item| item.display_name.clone())
                .collect::<Vec<_>>()
                .join("、")
        );
        let summary = SwitchExecutionSummary {
            executed_at_utc: Utc::now(),
            profile_id: profile.id,
            profile_name: profile.name.clone(),
            targets: targets
                .iter()
                .map(|item| item.display_name.clone())
                .collect(),
            message: message.clone(),
            status: SwitchExecutionStatus::Success,
            steps,
        };
        database.last_selected_profile_id = Some(profile.id);
        database.last_switch_summary = Some(summary.clone());
        store.save(&database)?;
        logger.append_success(
            "switch.execute",
            &message,
            json!({
                "profileId": profile.id,
                "targets": summary.targets,
                "providerName": provider_name
            }),
        )?;
        Ok(summary)
    }

    fn execute_switch_pipeline(
        &self,
        profile: &CodexProfile,
        provider_name: &str,
        targets: &[TargetContext],
        backup_service: &BackupService,
        template_service: &TemplateService,
        session_service: &SessionMigrationService,
        steps: &mut Vec<SwitchExecutionStep>,
    ) -> AppResult<()> {
        let prepared_payloads =
            self.validate_switch_inputs(profile, provider_name, targets, template_service)?;
        append_step(
            steps,
            "validate",
            format!("校验完成，共 {} 个目标。", targets.len()),
            SwitchStepStatus::Success,
        );

        let backup_snapshots = self.create_backup_snapshots(targets, backup_service, steps)?;
        let mut touched_snapshots = Vec::new();

        for snapshot in &backup_snapshots {
            if let Err(error) =
                self.replace_auth_json(&prepared_payloads.auth_json, &snapshot.target)
            {
                self.rollback_snapshots(backup_service, &touched_snapshots, steps);
                return Err(error);
            }
            touched_snapshots.push(snapshot.clone());
            append_step(
                steps,
                "write auth",
                format!("{} 的 auth.json 已写入。", snapshot.target.display_name),
                SwitchStepStatus::Success,
            );
        }

        for snapshot in &backup_snapshots {
            if let Err(error) =
                self.replace_config_toml(&prepared_payloads.config_toml, &snapshot.target)
            {
                self.rollback_snapshots(backup_service, &touched_snapshots, steps);
                return Err(error);
            }
            append_step(
                steps,
                "write config",
                format!("{} 的 config.toml 已写入。", snapshot.target.display_name),
                SwitchStepStatus::Success,
            );
        }

        for snapshot in &backup_snapshots {
            if let Err(error) =
                session_service.migrate(&snapshot.target, profile.provider_category, provider_name)
            {
                self.rollback_snapshots(backup_service, &touched_snapshots, steps);
                return Err(error);
            }
            append_step(
                steps,
                "migrate sessions",
                format!(
                    "{} 的活动对话、归档对话和 state_5.sqlite 迁移完成。",
                    snapshot.target.display_name
                ),
                SwitchStepStatus::Success,
            );
        }

        append_step(
            steps,
            "finalize",
            "已更新最后选中组合和最近一次执行结果。".into(),
            SwitchStepStatus::Success,
        );
        Ok(())
    }

    fn validate_switch_inputs(
        &self,
        profile: &CodexProfile,
        provider_name: &str,
        targets: &[TargetContext],
        template_service: &TemplateService,
    ) -> AppResult<PreparedSwitchPayloads> {
        for target in targets {
            if !target.codex_directory_path.exists() {
                return Err(AppError::validation(
                    "target_directory_missing",
                    format!("未找到 {} 的 Codex 配置目录。", target.display_name),
                ));
            }
        }

        let auth_json = match (profile.provider_category, profile.auth_mode) {
            (ProviderCategory::OpenAI, _) | (_, CodexAuthMode::AuthJsonFile) => {
                let source = profile
                    .stored_auth_json_path
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| {
                        AppError::validation(
                            "auth_json_missing",
                            "此组合未保存 auth.json 文件路径。",
                        )
                    })?;
                if !Path::new(source).exists() {
                    return Err(AppError::validation(
                        "auth_json_source_missing",
                        "已保存的 auth.json 不存在。",
                    ));
                }
                PreparedFilePayload::Copy(PathBuf::from(source))
            }
            (_, CodexAuthMode::ApiKey) => {
                let protected = profile.protected_api_key_base64.as_deref().ok_or_else(|| {
                    AppError::validation("api_key_missing", "此组合未保存 API Key。")
                })?;
                let api_key = unprotect_from_base64(protected)?;
                if api_key.trim().is_empty() {
                    return Err(AppError::validation(
                        "api_key_empty",
                        "此组合保存的 API Key 为空。",
                    ));
                }
                PreparedFilePayload::Text(serde_json::to_string_pretty(&json!({
                    "OPENAI_API_KEY": api_key
                }))?)
            }
        };

        if profile.provider_category == ProviderCategory::OpenAI {
            let template = template_service.load_openai_template()?;
            if template.trim().is_empty() {
                return Err(AppError::validation(
                    "openai_template_empty",
                    "OpenAI 模板不能为空。",
                ));
            }
            return Ok(PreparedSwitchPayloads {
                auth_json,
                config_toml: PreparedFilePayload::Text(ensure_trailing_newline(&template)),
            });
        }

        if let Some(stored_path) = profile
            .stored_config_toml_path
            .as_deref()
            .map(str::trim)
            .filter(|item| !item.is_empty())
        {
            if !Path::new(stored_path).exists() {
                return Err(AppError::validation(
                    "config_toml_source_missing",
                    "已保存的 config.toml 不存在。",
                ));
            }
            return Ok(PreparedSwitchPayloads {
                auth_json,
                config_toml: PreparedFilePayload::Copy(PathBuf::from(stored_path)),
            });
        }

        if profile.base_url.trim().is_empty() {
            return Err(AppError::validation(
                "base_url_missing",
                "当前组合缺少 Base URL。",
            ));
        }

        let template = template_service.load_apikey_template()?;
        if template.trim().is_empty() {
            return Err(AppError::validation(
                "apikey_template_empty",
                "APIKEY 模板不能为空。",
            ));
        }

        let rendered = template
            .replace("{base_url}", &to_toml_string(profile.base_url.trim()))
            .replace("{provider_name}", &to_toml_string(provider_name))
            .replace("{provider_key}", provider_name);

        Ok(PreparedSwitchPayloads {
            auth_json,
            config_toml: PreparedFilePayload::Text(ensure_trailing_newline(&rendered)),
        })
    }

    fn create_backup_snapshots(
        &self,
        targets: &[TargetContext],
        backup_service: &BackupService,
        steps: &mut Vec<SwitchExecutionStep>,
    ) -> AppResult<Vec<TargetBackupSnapshot>> {
        let mut snapshots = Vec::with_capacity(targets.len());

        for target in targets {
            let backup_dir = backup_service.create_backup_if_needed(target)?;
            if backup_dir.is_some() {
                append_step(
                    steps,
                    "backup",
                    format!("{} 已创建受管备份。", target.display_name),
                    SwitchStepStatus::Success,
                );
            } else {
                append_step(
                    steps,
                    "backup",
                    format!(
                        "{} 没有现有 auth/config 文件，跳过备份。",
                        target.display_name
                    ),
                    SwitchStepStatus::Skipped,
                );
            }

            snapshots.push(TargetBackupSnapshot {
                target: target.clone(),
                backup_dir,
            });
        }

        Ok(snapshots)
    }

    fn rollback_snapshots(
        &self,
        backup_service: &BackupService,
        touched_snapshots: &[TargetBackupSnapshot],
        steps: &mut Vec<SwitchExecutionStep>,
    ) {
        for snapshot in touched_snapshots.iter().rev() {
            match backup_service.restore_snapshot(&snapshot.target, snapshot.backup_dir.as_deref())
            {
                Ok(changed) => append_step(
                    steps,
                    "rollback",
                    if changed {
                        format!("{} 已回滚到切换前快照。", snapshot.target.display_name)
                    } else {
                        format!("{} 无需回滚。", snapshot.target.display_name)
                    },
                    if changed {
                        SwitchStepStatus::Success
                    } else {
                        SwitchStepStatus::Skipped
                    },
                ),
                Err(error) => append_step(
                    steps,
                    "rollback",
                    format!(
                        "{} 回滚失败：{}",
                        snapshot.target.display_name,
                        error.to_dto().message
                    ),
                    SwitchStepStatus::Failure,
                ),
            }
        }
    }

    fn replace_auth_json(
        &self,
        payload: &PreparedFilePayload,
        target: &TargetContext,
    ) -> AppResult<()> {
        let destination = target.codex_directory_path.join("auth.json");
        write_prepared_payload(payload, &destination)
    }

    fn replace_config_toml(
        &self,
        payload: &PreparedFilePayload,
        target: &TargetContext,
    ) -> AppResult<()> {
        let destination = target.codex_directory_path.join("config.toml");
        write_prepared_payload(payload, &destination)
    }
}

fn find_profile(database: &ProfileDatabase, profile_id: Uuid) -> AppResult<&CodexProfile> {
    database
        .profiles
        .iter()
        .find(|profile| profile.id == profile_id)
        .ok_or_else(|| AppError::validation("profile_not_found", "未找到要操作的组合。"))
}

fn effective_api_key_provider_name(database: &ProfileDatabase) -> AppResult<String> {
    let provider_name = if database.api_key_provider_name.trim().is_empty() {
        DEFAULT_APIKEY_PROVIDER_NAME.to_string()
    } else {
        database.api_key_provider_name.trim().to_string()
    };
    let regex = provider_name_regex();
    if !regex.is_match(&provider_name) {
        return Err(AppError::validation(
            "provider_name_invalid",
            "设置里的 API Key 提供商名无效。请只使用字母、数字、下划线或短横线。",
        ));
    }
    Ok(provider_name)
}

fn provider_name_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"^[A-Za-z0-9_-]+$").expect("provider name regex 初始化失败"))
}

fn append_step(
    steps: &mut Vec<SwitchExecutionStep>,
    name: &str,
    detail: String,
    status: SwitchStepStatus,
) {
    steps.push(SwitchExecutionStep {
        name: name.to_string(),
        detail,
        status,
    });
}

fn failure_summary(
    profile: &CodexProfile,
    targets: &[TargetContext],
    mut steps: Vec<SwitchExecutionStep>,
    message: String,
) -> SwitchExecutionSummary {
    append_step(
        &mut steps,
        "finalize",
        "流程已中止。".into(),
        SwitchStepStatus::Failure,
    );
    SwitchExecutionSummary {
        executed_at_utc: Utc::now(),
        profile_id: profile.id,
        profile_name: profile.name.clone(),
        targets: targets
            .iter()
            .map(|item| item.display_name.clone())
            .collect(),
        message,
        status: SwitchExecutionStatus::Failure,
        steps,
    }
}

fn ensure_trailing_newline(content: &str) -> String {
    if content.ends_with('\n') {
        content.to_string()
    } else {
        format!("{content}\n")
    }
}

fn to_toml_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".into())
}

fn persist_failure_summary(
    store: &ProfileStore<'_>,
    logger: &LogService<'_>,
    database: &mut ProfileDatabase,
    profile: &CodexProfile,
    targets: &[TargetContext],
    steps: Vec<SwitchExecutionStep>,
    error: &AppError,
) {
    let summary = failure_summary(profile, targets, steps, error.to_dto().message.clone());
    database.last_switch_summary = Some(summary.clone());
    let _ = store.save(database);
    let _ = logger.append_failure("switch.execute", &summary.message, error.to_dto().message);
}

fn copy_file_atomically(source: &Path, destination: &Path) -> AppResult<()> {
    let temp_path = build_temp_path(destination);
    fs::copy(source, &temp_path)?;
    finalize_temp_file(&temp_path, destination)
}

fn write_prepared_payload(payload: &PreparedFilePayload, destination: &Path) -> AppResult<()> {
    match payload {
        PreparedFilePayload::Copy(source) => copy_file_atomically(source.as_path(), destination),
        PreparedFilePayload::Text(content) => write_text_atomically(destination, content),
    }
}

fn write_text_atomically(destination: &Path, content: &str) -> AppResult<()> {
    let temp_path = build_temp_path(destination);
    fs::write(&temp_path, content)?;
    finalize_temp_file(&temp_path, destination)
}

fn build_temp_path(destination: &Path) -> PathBuf {
    let file_name = destination
        .file_name()
        .and_then(|item| item.to_str())
        .unwrap_or("codex-switch");
    destination.with_file_name(format!("{file_name}.{}.tmp", Uuid::new_v4().simple()))
}

fn finalize_temp_file(temp_path: &Path, destination: &Path) -> AppResult<()> {
    let result = (|| -> AppResult<()> {
        if destination.exists() {
            fs::remove_file(destination)?;
        }
        fs::rename(temp_path, destination)?;
        Ok(())
    })();

    if result.is_err() && temp_path.exists() {
        let _ = fs::remove_file(temp_path);
    }

    result
}
