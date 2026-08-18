use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::MutexGuard;

use chrono::Utc;
use tauri::State;
use uuid::Uuid;

use crate::errors::app_error::{AppError, AppErrorDto, AppResult};
use crate::models::app::{
    AppBootstrap, AppMeta, CachedWslEnvironmentInfo, DashboardSnapshot, PickFileInput,
    SaveSettingsInput, SettingsSnapshot,
};
use crate::models::backup::BackupItem;
use crate::models::connection::{
    BatchConnectionTestInput, BatchConnectionTestItemDto, BatchConnectionTestResponseDto,
    ConnectionTestResultDto, ConnectionTestStatus,
};
use crate::models::profile::{CodexProfileDto, ProfileDatabase, SaveProfileInput};
use crate::models::switch::SwitchPreview;
use crate::models::template::{SaveTemplateInput, TemplateKind};
use crate::services::backup_service::BackupService;
use crate::services::codex_switch_service::CodexSwitchService;
use crate::services::connection_test_service::ConnectionTestService;
use crate::services::log_service::LogService;
use crate::services::profile_store::ProfileStore;
use crate::services::target_service::TargetService;
use crate::services::template_service::TemplateService;
use crate::services::wsl_service::WslService;
use crate::state::app_state::{AppPaths, AppState};
use crate::utils::file_dialog::pick_single_file;

const DEFAULT_RECENT_LOG_LIMIT: usize = 200;
const MAX_RECENT_LOG_LIMIT: usize = 500;

#[tauri::command]
pub fn load_app_bootstrap(state: State<'_, AppState>) -> Result<AppBootstrap, AppErrorDto> {
    let _guard = lock_database(&state).map_err(|error| error.to_dto())?;
    build_bootstrap(&state.paths).map_err(|error| error.to_dto())
}

#[tauri::command]
pub fn save_profile(
    state: State<'_, AppState>,
    payload: SaveProfileInput,
) -> Result<AppBootstrap, AppErrorDto> {
    let _guard = lock_database(&state).map_err(|error| error.to_dto())?;
    ProfileStore::new(&state.paths)
        .save_profile(&payload)
        .map_err(|error| error.to_dto())?;
    build_bootstrap(&state.paths).map_err(|error| error.to_dto())
}

#[tauri::command]
pub fn delete_profile(
    state: State<'_, AppState>,
    profile_id: String,
) -> Result<AppBootstrap, AppErrorDto> {
    let _guard = lock_database(&state).map_err(|error| error.to_dto())?;
    ProfileStore::new(&state.paths)
        .delete_profile(parse_uuid(&profile_id).map_err(|error| error.to_dto())?)
        .map_err(|error| error.to_dto())?;
    build_bootstrap(&state.paths).map_err(|error| error.to_dto())
}

#[tauri::command]
pub fn duplicate_profile(
    state: State<'_, AppState>,
    profile_id: String,
) -> Result<AppBootstrap, AppErrorDto> {
    let _guard = lock_database(&state).map_err(|error| error.to_dto())?;
    ProfileStore::new(&state.paths)
        .duplicate_profile(parse_uuid(&profile_id).map_err(|error| error.to_dto())?)
        .map_err(|error| error.to_dto())?;
    build_bootstrap(&state.paths).map_err(|error| error.to_dto())
}

#[tauri::command]
pub fn update_settings(
    state: State<'_, AppState>,
    payload: SaveSettingsInput,
) -> Result<AppBootstrap, AppErrorDto> {
    let _guard = lock_database(&state).map_err(|error| error.to_dto())?;
    let store = ProfileStore::new(&state.paths);
    let mut database = store.load().map_err(|error| error.to_dto())?;

    database.replace_windows_target = payload.replace_windows_target || !payload.replace_wsl_target;
    database.replace_wsl_target = payload.replace_wsl_target;
    database.wsl_distro_name = payload
        .wsl_distro_name
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty());
    database.wsl_user_name = payload
        .wsl_user_name
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty());
    database.session_migration_days = payload.session_migration_days.clamp(0, 30);
    database.api_key_provider_name = payload.api_key_provider_name.trim().to_string();
    database.ui_preferences = payload.ui_preferences;
    store.save(&database).map_err(|error| error.to_dto())?;

    build_bootstrap(&state.paths).map_err(|error| error.to_dto())
}

#[tauri::command]
pub fn refresh_default_wsl(state: State<'_, AppState>) -> Result<AppBootstrap, AppErrorDto> {
    let _guard = lock_database(&state).map_err(|error| error.to_dto())?;
    let store = ProfileStore::new(&state.paths);
    let mut database = store.load().map_err(|error| error.to_dto())?;
    let wsl_service = WslService::new();

    match wsl_service.refresh_default_environment() {
        Ok(info) => {
            database.cached_default_wsl_distro_name = Some(info.distro_name);
            database.cached_default_wsl_user_name = Some(info.user_name);
            database.cached_default_wsl_home_directory = Some(info.home_directory);
            database.cached_default_wsl_detected_at_utc = Some(Utc::now());
            database.cached_default_wsl_error_message = None;
            database.cached_default_wsl_error_at_utc = None;
        }
        Err(error) => {
            database.cached_default_wsl_error_message = Some(error.to_dto().message);
            database.cached_default_wsl_error_at_utc = Some(Utc::now());
        }
    }

    store.save(&database).map_err(|error| error.to_dto())?;
    build_bootstrap(&state.paths).map_err(|error| error.to_dto())
}

#[tauri::command]
pub fn get_switch_preview(
    state: State<'_, AppState>,
    profile_id: String,
) -> Result<SwitchPreview, AppErrorDto> {
    let _guard = lock_database(&state).map_err(|error| error.to_dto())?;
    CodexSwitchService::new(&state.paths)
        .build_switch_preview(parse_uuid(&profile_id).map_err(|error| error.to_dto())?)
        .map_err(|error| error.to_dto())
}

#[tauri::command]
pub fn execute_switch(
    state: State<'_, AppState>,
    profile_id: String,
) -> Result<AppBootstrap, AppErrorDto> {
    let _switch_guard = lock_switch(&state).map_err(|error| error.to_dto())?;
    let _database_guard = lock_database(&state).map_err(|error| error.to_dto())?;
    CodexSwitchService::new(&state.paths)
        .execute_switch(parse_uuid(&profile_id).map_err(|error| error.to_dto())?)
        .map_err(|error| error.to_dto())?;
    build_bootstrap(&state.paths).map_err(|error| error.to_dto())
}

#[tauri::command]
pub fn list_backups(state: State<'_, AppState>) -> Result<Vec<BackupItem>, AppErrorDto> {
    BackupService::new(&state.paths)
        .list_all_backups()
        .map_err(|error| error.to_dto())
}

#[tauri::command]
pub fn restore_latest_backup(state: State<'_, AppState>) -> Result<AppBootstrap, AppErrorDto> {
    let _switch_guard = lock_switch(&state).map_err(|error| error.to_dto())?;
    let _database_guard = lock_database(&state).map_err(|error| error.to_dto())?;
    let store = ProfileStore::new(&state.paths);
    let database = store.load().map_err(|error| error.to_dto())?;
    let restored = BackupService::new(&state.paths)
        .restore_latest_for_selected_targets(&database)
        .map_err(|error| error.to_dto())?;
    let _ = LogService::new(&state.paths).append_success(
        "backup.restore",
        "恢复最近备份完成。",
        serde_json::json!({ "targets": restored }),
    );
    build_bootstrap(&state.paths).map_err(|error| error.to_dto())
}

#[tauri::command]
pub fn save_template(
    state: State<'_, AppState>,
    payload: SaveTemplateInput,
) -> Result<AppBootstrap, AppErrorDto> {
    TemplateService::new(&state.paths)
        .save_template(&payload)
        .map_err(|error| error.to_dto())?;
    build_bootstrap(&state.paths).map_err(|error| error.to_dto())
}

#[tauri::command]
pub fn reset_template(
    state: State<'_, AppState>,
    kind: TemplateKind,
) -> Result<AppBootstrap, AppErrorDto> {
    TemplateService::new(&state.paths)
        .reset_template(kind)
        .map_err(|error| error.to_dto())?;
    build_bootstrap(&state.paths).map_err(|error| error.to_dto())
}

#[tauri::command]
pub async fn test_profile_connection(
    state: State<'_, AppState>,
    profile_id: String,
) -> Result<ConnectionTestResultDto, AppErrorDto> {
    let profile = {
        let _guard = lock_database(&state).map_err(|error| error.to_dto())?;
        let store = ProfileStore::new(&state.paths);
        let database = store.load().map_err(|error| error.to_dto())?;
        let profile_id = parse_uuid(&profile_id).map_err(|error| error.to_dto())?;
        database
            .profiles
            .iter()
            .find(|item| item.id == profile_id)
            .ok_or_else(|| {
                AppError::validation("profile_not_found", "未找到要测试的组合。").to_dto()
            })?
            .clone()
    };
    let service = ConnectionTestService::new().map_err(|error| error.to_dto())?;
    let result = service
        .test_profile(&profile, None)
        .await
        .map_err(|error| error.to_dto())?;
    let _ = LogService::new(&state.paths).append_success(
        "connection.test",
        "完成 APIKEY 连接测试。",
        serde_json::json!({
            "profileId": profile.id,
            "endpoint": result.endpoint,
            "status": result.status,
            "model": result.model
        }),
    );
    Ok(result)
}

#[tauri::command]
pub async fn batch_test_profile_connections(
    state: State<'_, AppState>,
    payload: BatchConnectionTestInput,
) -> Result<BatchConnectionTestResponseDto, AppErrorDto> {
    if payload.profile_ids.is_empty() {
        return Err(
            AppError::validation("profile_ids_missing", "请至少选择一个 APIKEY 组合。")
                .to_dto(),
        );
    }

    let profiles = {
        let _guard = lock_database(&state).map_err(|error| error.to_dto())?;
        let store = ProfileStore::new(&state.paths);
        let database = store.load().map_err(|error| error.to_dto())?;
        payload
            .profile_ids
            .iter()
            .map(|profile_id| {
                database
                    .profiles
                    .iter()
                    .find(|item| item.id == *profile_id)
                    .ok_or_else(|| {
                        AppError::validation("profile_not_found", "存在未找到的测试组合。")
                    })
                    .cloned()
            })
            .collect::<AppResult<Vec<_>>>()
            .map_err(|error| error.to_dto())?
    };

    let service = ConnectionTestService::new().map_err(|error| error.to_dto())?;
    let mut results = Vec::with_capacity(profiles.len());
    let mut failures_to_disable = Vec::new();

    for profile in &profiles {
        let result = service
            .test_profile(profile, payload.override_model.as_deref())
            .await
            .map_err(|error| error.to_dto())?;
        let should_disable = payload.disable_on_failure
            && matches!(result.status, ConnectionTestStatus::Failure)
            && !profile.auto_disabled;
        if should_disable {
            failures_to_disable.push((profile.id, result.message.clone()));
        }
        results.push(BatchConnectionTestItemDto {
            profile_id: profile.id,
            profile_name: profile.name.clone(),
            status: result.status,
            endpoint: result.endpoint,
            model: result.model,
            message: result.message,
            auto_disabled: profile.auto_disabled || should_disable,
        });
    }

    {
        let _guard = lock_database(&state).map_err(|error| error.to_dto())?;
        if !failures_to_disable.is_empty() {
            ProfileStore::new(&state.paths)
                .mark_profiles_auto_disabled(&failures_to_disable)
                .map_err(|error| error.to_dto())?;
        }
    }

    let bootstrap = build_bootstrap(&state.paths).map_err(|error| error.to_dto())?;
    let _ = LogService::new(&state.paths).append_success(
        "connection.test.batch",
        "完成批量 APIKEY 连接测试。",
        serde_json::json!({
            "profileIds": payload.profile_ids,
            "overrideModel": payload.override_model,
            "disableOnFailure": payload.disable_on_failure,
            "disabledCount": failures_to_disable.len(),
            "results": results
                .iter()
                .map(|item| serde_json::json!({
                    "profileId": item.profile_id,
                    "status": item.status,
                    "model": item.model,
                    "autoDisabled": item.auto_disabled
                }))
                .collect::<Vec<_>>()
        }),
    );

    Ok(BatchConnectionTestResponseDto { results, bootstrap })
}

#[tauri::command]
pub fn set_profile_test_disabled(
    state: State<'_, AppState>,
    profile_id: String,
    disabled: bool,
) -> Result<AppBootstrap, AppErrorDto> {
    let _guard = lock_database(&state).map_err(|error| error.to_dto())?;
    let profile_id = parse_uuid(&profile_id).map_err(|error| error.to_dto())?;
    ProfileStore::new(&state.paths)
        .set_profile_auto_disabled(profile_id, disabled)
        .map_err(|error| error.to_dto())?;
    let _ = LogService::new(&state.paths).append_success(
        "profile.test.disable",
        if disabled {
            "组合已手动禁用。"
        } else {
            "组合已恢复启用。"
        },
        serde_json::json!({
            "profileId": profile_id,
            "disabled": disabled
        }),
    );
    build_bootstrap(&state.paths).map_err(|error| error.to_dto())
}

#[tauri::command]
pub fn read_recent_logs(
    state: State<'_, AppState>,
    limit: Option<usize>,
) -> Result<Vec<crate::models::app::LogEntry>, AppErrorDto> {
    let safe_limit = limit
        .unwrap_or(DEFAULT_RECENT_LOG_LIMIT)
        .min(MAX_RECENT_LOG_LIMIT);

    LogService::new(&state.paths)
        .read_recent(safe_limit)
        .map_err(|error| error.to_dto())
}

#[tauri::command]
pub fn open_data_directory(state: State<'_, AppState>) -> Result<(), AppErrorDto> {
    open_directory_in_shell(
        &state.paths.data_root,
        &state.paths.data_root,
        "打开数据目录失败。",
    )
}

#[tauri::command]
pub fn open_backups_directory(state: State<'_, AppState>) -> Result<(), AppErrorDto> {
    open_directory_in_shell(
        &state.paths.backups_root,
        &state.paths.backups_root,
        "打开备份目录失败。",
    )
}

#[tauri::command]
pub fn open_logs_directory(state: State<'_, AppState>) -> Result<(), AppErrorDto> {
    open_directory_in_shell(
        &state.paths.logs_root,
        &state.paths.logs_root,
        "打开日志目录失败。",
    )
}

#[tauri::command]
pub fn open_backup_directory(
    state: State<'_, AppState>,
    target_key: String,
    directory_name: String,
) -> Result<(), AppErrorDto> {
    let target_root =
        resolve_backup_target_root(&state.paths, &target_key).map_err(|error| error.to_dto())?;
    let directory_name =
        validate_backup_directory_name(&directory_name).map_err(|error| error.to_dto())?;
    open_directory_in_shell(
        &target_root.join(directory_name),
        &target_root,
        "打开备份目录失败。",
    )
}

#[tauri::command]
pub fn pick_file_path(payload: PickFileInput) -> Result<Option<String>, AppErrorDto> {
    pick_single_file(
        payload.title.as_deref(),
        &payload.filter_name,
        &payload.extensions,
    )
    .map_err(|error| error.to_dto())
}

fn build_bootstrap(paths: &AppPaths) -> AppResult<AppBootstrap> {
    let store = ProfileStore::new(paths);
    let database = store.load()?;
    let target_service = TargetService::new(paths);
    let backup_service = BackupService::new(paths);
    let active_profile = database
        .last_selected_profile_id
        .and_then(|profile_id| database.profiles.iter().find(|item| item.id == profile_id));
    let selected_targets = target_service
        .resolve_selected_targets(&database)
        .map(|items| items.into_iter().map(|item| item.display_name).collect())
        .unwrap_or_default();

    Ok(AppBootstrap {
        profiles: database
            .profiles
            .iter()
            .map(CodexProfileDto::from)
            .collect::<Vec<_>>(),
        settings: build_settings_snapshot(&database, &target_service),
        dashboard: DashboardSnapshot {
            active_profile_id: active_profile.map(|item| item.id.to_string()),
            active_profile_name: active_profile.map(|item| item.name.clone()),
            selected_target_labels: selected_targets,
            wsl_status: target_service.wsl_status_text(&database),
            backup_overview: backup_service.build_overview(&database).unwrap_or_default(),
            last_switch_summary: database.last_switch_summary.clone(),
        },
        templates: TemplateService::new(paths).load_snapshot()?,
        backups: backup_service.list_all_backups()?,
        app_meta: AppMeta {
            version: env!("CARGO_PKG_VERSION").into(),
        },
    })
}

fn build_settings_snapshot(
    database: &ProfileDatabase,
    target_service: &TargetService<'_>,
) -> SettingsSnapshot {
    SettingsSnapshot {
        replace_windows_target: database.replace_windows_target,
        replace_wsl_target: database.replace_wsl_target,
        wsl_distro_name: database.wsl_distro_name.clone(),
        wsl_user_name: database.wsl_user_name.clone(),
        cached_default_wsl: target_service
            .wsl_service()
            .cached_default_environment(database)
            .map(|item| CachedWslEnvironmentInfo {
                distro_name: item.distro_name,
                user_name: item.user_name,
            }),
        cached_default_wsl_error_message: database.cached_default_wsl_error_message.clone(),
        session_migration_days: database.session_migration_days,
        api_key_provider_name: database.api_key_provider_name.clone(),
        ui_preferences: database.ui_preferences.clone(),
        migration_version: database.migration_version,
    }
}

fn parse_uuid(value: &str) -> AppResult<Uuid> {
    Uuid::parse_str(value).map_err(|error| {
        AppError::internal("uuid_parse_failed", "标识符格式无效。", error.to_string())
    })
}

fn lock_switch<'a>(state: &'a State<'_, AppState>) -> AppResult<MutexGuard<'a, ()>> {
    state.switch_lock.lock().map_err(|_| {
        AppError::internal(
            "switch_lock_failed",
            "切换任务状态锁定失败。",
            "poisoned mutex",
        )
    })
}

fn lock_database<'a>(state: &'a State<'_, AppState>) -> AppResult<MutexGuard<'a, ()>> {
    state.database_lock.lock().map_err(|_| {
        AppError::internal(
            "database_lock_failed",
            "配置数据状态锁定失败。",
            "poisoned mutex",
        )
    })
}

fn open_directory_in_shell(path: &Path, allowed_root: &Path, message: &str) -> Result<(), AppErrorDto> {
    let resolved_path = resolve_allowed_directory(path, allowed_root).map_err(|error| error.to_dto())?;

    Command::new("explorer.exe")
        .arg(&resolved_path)
        .spawn()
        .map(|_| ())
        .map_err(|source| {
            AppError::Io {
                code: "open_path_failed".into(),
                message: message.into(),
                source,
            }
            .to_dto()
        })
}

fn resolve_allowed_directory(path: &Path, allowed_root: &Path) -> AppResult<PathBuf> {
    let canonical_root = resolve_existing_directory(allowed_root)?;
    let canonical_path = resolve_existing_directory(path)?;

    if !canonical_path.starts_with(&canonical_root) {
        return Err(AppError::validation(
            "directory_out_of_scope",
            "目录不在允许访问范围内。",
        ));
    }

    Ok(canonical_path)
}

fn resolve_existing_directory(path: &Path) -> AppResult<PathBuf> {
    let metadata = fs::metadata(path).map_err(|source| AppError::Io {
        code: "directory_metadata_failed".into(),
        message: "目录不存在或无法访问。".into(),
        source,
    })?;

    if !metadata.is_dir() {
        return Err(AppError::validation(
            "directory_not_found",
            "目录不存在或无法访问。",
        ));
    }

    path.canonicalize().map_err(|source| AppError::Io {
        code: "directory_canonicalize_failed".into(),
        message: "目录路径解析失败。".into(),
        source,
    })
}

fn resolve_backup_target_root(paths: &AppPaths, target_key: &str) -> AppResult<PathBuf> {
    match target_key {
        "windows" => Ok(paths.backups_root.join("windows")),
        "wsl" => Ok(paths.backups_root.join("wsl")),
        _ => Err(AppError::validation(
            "invalid_backup_target",
            "备份目标无效。",
        )),
    }
}

fn validate_backup_directory_name(value: &str) -> AppResult<String> {
    let trimmed = value.trim();
    let mut components = Path::new(trimmed).components();

    match (components.next(), components.next()) {
        (Some(Component::Normal(name)), None) if name.to_string_lossy() == trimmed => {
            Ok(trimmed.to_string())
        }
        _ => Err(AppError::validation(
            "invalid_backup_directory",
            "备份目录标识无效。",
        )),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::{resolve_allowed_directory, validate_backup_directory_name};
    use crate::errors::app_error::AppError;

    #[test]
    fn resolve_allowed_directory_accepts_child_directory() {
        let workspace = tempdir().unwrap();
        let allowed_root = workspace.path().join("backups");
        let child = allowed_root.join("windows").join("20260328-120000");
        fs::create_dir_all(&child).unwrap();

        let resolved = resolve_allowed_directory(&child, &allowed_root).unwrap();

        assert_eq!(resolved, child.canonicalize().unwrap());
    }

    #[test]
    fn resolve_allowed_directory_rejects_outside_directory() {
        let workspace = tempdir().unwrap();
        let allowed_root = workspace.path().join("backups");
        let outside = workspace.path().join("outside");
        fs::create_dir_all(&allowed_root).unwrap();
        fs::create_dir_all(&outside).unwrap();

        let error = resolve_allowed_directory(&outside, &allowed_root).unwrap_err();

        match error {
            AppError::Validation { code, .. } => {
                assert_eq!(code, "directory_out_of_scope");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn validate_backup_directory_name_rejects_nested_paths() {
        let error = validate_backup_directory_name("windows/escape").unwrap_err();

        match error {
            AppError::Validation { code, .. } => {
                assert_eq!(code, "invalid_backup_directory");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
