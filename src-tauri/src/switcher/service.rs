use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime};

use chrono::Utc;
use reqwest::StatusCode;
use rusqlite::{Connection, OpenFlags};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use toml_edit::{table, value, Array, DocumentMut, Item};
use uuid::Uuid;
use walkdir::WalkDir;

use crate::switcher::cdp::inject_model_catalog;
use crate::switcher::models::{
    BackupFileRecord, BackupManifest, BackupSummary, CodexRuntimeStatus, ConnectionTestResult,
    ExecuteSwitchInput, ModelDiscoveryResult, ModelEntry, OfficialSnapshot, StoredProviderProfile,
    SwitchPreview, SwitchResult, SwitchStep, SwitcherBootstrap, SwitcherDatabase,
};
use crate::switcher::state::SwitcherPaths;
use crate::switcher::store::SwitcherStore;
use crate::switcher::{ApiError, ApiResult};
use crate::utils::dpapi::{protect_to_base64, unprotect_from_base64};

const LEGACY_CATALOG_RELATIVE_PATH: &str = "model-catalogs/codex-api-switcher.json";
const CATALOG_TEMPLATE: &str = include_str!("resources/codex_native_responses_template.json");
const CODEX_PACKAGE_NAME: &str = "OpenAI.Codex";
const CODEX_APPLICATION_ID: &str = "App";

#[derive(Debug, Clone)]
struct CodexDesktopInstallation {
    executable_path: String,
    app_user_model_id: String,
}

pub fn build_bootstrap(paths: &SwitcherPaths, app_version: &str) -> ApiResult<SwitcherBootstrap> {
    let store = SwitcherStore::new(paths);
    recover_incomplete_transaction(paths, &store)?;
    let database = store.load()?;
    Ok(SwitcherBootstrap {
        profiles: SwitcherStore::profiles_for_ui(&database),
        settings: database.settings.clone(),
        active_profile_id: database.active_profile_id.clone(),
        backups: list_backups(paths)?,
        runtime: get_runtime_status(&database)?,
        app_version: app_version.to_string(),
    })
}

pub fn get_runtime_status(database: &SwitcherDatabase) -> ApiResult<CodexRuntimeStatus> {
    let codex_home = resolve_codex_home(database)?;
    let desktop = detect_codex_desktop();
    let executable_path = desktop
        .as_ref()
        .map(|desktop| desktop.executable_path.clone());
    let version = detect_codex_package().and_then(|value| {
        value
            .get("Version")
            .and_then(Value::as_str)
            .map(str::to_string)
    });
    Ok(CodexRuntimeStatus {
        installed: executable_path.is_some(),
        running: codex_is_running(),
        version,
        executable_path,
        codex_home: codex_home.to_string_lossy().to_string(),
        active_provider_id: database.active_provider_id.clone(),
    })
}

pub fn preview_switch(paths: &SwitcherPaths, profile_id: &str) -> ApiResult<SwitchPreview> {
    let store = SwitcherStore::new(paths);
    let database = store.load()?;
    let profile = find_profile(&database, profile_id)?;
    ensure_default_model(profile)?;
    let codex_home = resolve_codex_home(&database)?;
    let mut warnings = Vec::new();
    if !codex_home.exists() {
        warnings.push("CODEX_HOME 尚不存在，首次切换时将自动创建。".to_string());
    }
    if !profile.base_url.ends_with("/v1") {
        warnings.push("API 地址未以 /v1 结尾，请确认上游路由格式。".to_string());
    }
    if profile.models.is_empty() {
        warnings.push("模型列表为空，将仅写入默认模型。".to_string());
    }
    Ok(SwitchPreview {
        profile_id: profile.id.clone(),
        profile_name: profile.name.clone(),
        provider_id: profile.provider_id.clone(),
        codex_home: codex_home.to_string_lossy().to_string(),
        files_to_backup: vec![
            "config.toml".to_string(),
            "auth.json".to_string(),
            catalog_relative_path(&profile.provider_id),
            "state_5.sqlite".to_string(),
            "sessions/**/*.jsonl".to_string(),
        ],
        session_migration_days: database.settings.session_migration_days,
        codex_running: codex_is_running(),
        will_restart_codex: true,
        will_inject_models: profile.inject_models,
        warnings,
    })
}

pub async fn fetch_models(
    paths: &SwitcherPaths,
    profile_id: &str,
) -> ApiResult<ModelDiscoveryResult> {
    let store = SwitcherStore::new(paths);
    let database = store.load()?;
    let profile = find_profile(&database, profile_id)?;
    let api_key = store.provider_secret(profile)?;
    let endpoint = api_endpoint(&profile.base_url, "models");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(profile.timeout_seconds))
        .build()
        .map_err(http_client_error)?;
    let response = client
        .get(&endpoint)
        .bearer_auth(api_key)
        .send()
        .await
        .map_err(|error| http_error("model_fetch_failed", "获取模型列表失败。", error))?;
    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|error| http_error("model_body_failed", "读取模型列表响应失败。", error))?;
    if !status.is_success() {
        return Err(status_error(status, &text, "获取模型列表"));
    }
    let value: Value = serde_json::from_str(&text).map_err(|error| {
        ApiError::detailed(
            "model_response_invalid",
            "模型列表响应不是有效 JSON。",
            error.to_string(),
        )
    })?;
    let items = value
        .get("data")
        .or_else(|| value.get("models"))
        .and_then(Value::as_array)
        .or_else(|| value.as_array())
        .ok_or_else(|| {
            ApiError::new("model_list_missing", "响应中没有找到 data 或 models 数组。")
        })?;
    let mut seen = HashSet::new();
    let mut models = Vec::new();
    for item in items {
        let id = item
            .get("id")
            .or_else(|| item.get("slug"))
            .or_else(|| item.get("model"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let Some(id) = id else {
            continue;
        };
        if !seen.insert(id.to_string()) {
            continue;
        }
        let display_name = item
            .get("display_name")
            .or_else(|| item.get("displayName"))
            .or_else(|| item.get("name"))
            .and_then(Value::as_str)
            .unwrap_or(id);
        let context_window = item
            .get("context_window")
            .or_else(|| item.get("contextWindow"))
            .and_then(Value::as_u64);
        models.push(ModelEntry {
            id: id.to_string(),
            display_name: display_name.to_string(),
            context_window,
        });
    }
    models.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(ModelDiscoveryResult {
        endpoint,
        message: format!("发现 {} 个模型。", models.len()),
        models,
    })
}

pub async fn test_provider(
    paths: &SwitcherPaths,
    profile_id: &str,
) -> ApiResult<ConnectionTestResult> {
    let store = SwitcherStore::new(paths);
    let database = store.load()?;
    let profile = find_profile(&database, profile_id)?;
    ensure_default_model(profile)?;
    let api_key = store.provider_secret(profile)?;
    let endpoint = api_endpoint(&profile.base_url, "responses");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(profile.timeout_seconds))
        .build()
        .map_err(http_client_error)?;
    let started = Instant::now();
    let response = client
        .post(&endpoint)
        .bearer_auth(api_key)
        .json(&json!({
            "model": profile.default_model,
            "input": "Reply with OK.",
            "max_output_tokens": 8,
            "stream": false
        }))
        .send()
        .await
        .map_err(|error| http_error("connection_test_failed", "连接测试失败。", error))?;
    let latency_ms = started.elapsed().as_millis();
    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|error| http_error("connection_body_failed", "读取测试响应失败。", error))?;
    if !status.is_success() {
        return Err(status_error(status, &text, "连接测试"));
    }
    Ok(ConnectionTestResult {
        endpoint,
        model: profile.default_model.clone(),
        latency_ms,
        status: "success".to_string(),
        message: format!("连接成功，耗时 {latency_ms} ms。"),
    })
}

pub async fn execute_switch(
    paths: &SwitcherPaths,
    input: ExecuteSwitchInput,
    app_version: &str,
) -> ApiResult<SwitchResult> {
    let store = SwitcherStore::new(paths);
    let mut database = store.load()?;
    let profile = find_profile(&database, &input.profile_id)?.clone();
    ensure_default_model(&profile)?;
    let codex_home = resolve_codex_home(&database)?;
    let mut steps = Vec::new();
    let mut warnings = Vec::new();
    fs::create_dir_all(&codex_home).map_err(|error| {
        ApiError::detailed(
            "codex_home_create_failed",
            "创建 CODEX_HOME 失败。",
            error.to_string(),
        )
    })?;
    steps.push(step(
        "preflight",
        "success",
        "配置、路径和磁盘写入权限检查完成。",
    ));

    let desktop = detect_codex_desktop();
    if codex_is_running() {
        request_codex_close();
        if !wait_for_codex_exit(Duration::from_secs(8)) {
            if !input.force_close {
                return Err(ApiError::new(
                    "codex_force_close_required",
                    "Codex 在 8 秒内未退出。确认没有未保存操作后，可再次执行并允许强制关闭。",
                ));
            }
            force_close_codex();
            if !wait_for_codex_exit(Duration::from_secs(4)) {
                return Err(ApiError::new(
                    "codex_close_failed",
                    "无法关闭正在运行的 Codex。",
                ));
            }
        }
        steps.push(step("closeCodex", "success", "Codex 已退出。"));
    } else {
        steps.push(step("closeCodex", "skipped", "Codex 当前未运行。"));
    }

    capture_official_snapshot_if_needed(&mut database, &codex_home)?;
    store.save(&database)?;
    let mut manifest = create_backup(
        paths,
        &codex_home,
        "switch",
        Some(&profile),
        database.active_provider_id.as_deref(),
        database.settings.session_migration_days,
    )?;
    write_transaction_marker(paths, &manifest.id, "backup-complete")?;
    steps.push(step(
        "backup",
        "success",
        format!("已创建备份 {}。", manifest.id),
    ));

    let apply_result = (|| -> ApiResult<Vec<String>> {
        apply_provider_files(&database, &profile, &codex_home, &store)?;
        update_transaction_stage(paths, &manifest.id, "config-written")?;
        let migrated = migrate_sessions(
            &codex_home,
            &profile.provider_id,
            database.settings.session_migration_days,
        )?;
        update_transaction_stage(paths, &manifest.id, "sessions-migrated")?;
        Ok(migrated)
    })();

    let migrated_sessions = match apply_result {
        Ok(ids) => ids,
        Err(error) => {
            let rollback_result = restore_manifest(&manifest, paths, true);
            manifest.status = if rollback_result.is_ok() {
                "rolledBack".to_string()
            } else {
                "rollbackFailed".to_string()
            };
            let _ = write_manifest(paths, &manifest);
            let _ = clear_transaction_marker(paths);
            return Err(if let Err(rollback_error) = rollback_result {
                ApiError::detailed(
                    "switch_and_rollback_failed",
                    "切换失败，并且自动回滚未能完整完成。",
                    format!(
                        "switch: {}; rollback: {}",
                        error.message, rollback_error.message
                    ),
                )
            } else {
                error
            });
        }
    };

    steps.push(step(
        "writeConfig",
        "success",
        "Provider、认证和模型目录已写入。",
    ));
    steps.push(step(
        "migrateSessions",
        "success",
        format!("已同步 {} 个历史会话。", migrated_sessions.len()),
    ));

    database.active_profile_id = Some(profile.id.clone());
    database.active_provider_id = Some(profile.provider_id.clone());
    let commit_result = (|| -> ApiResult<()> {
        store.save(&database)?;
        finalize_manifest(paths, &codex_home, &mut manifest)?;
        clear_transaction_marker(paths)
    })();
    if let Err(error) = commit_result {
        let rollback_result = restore_manifest(&manifest, paths, true).and_then(|_| {
            let mut restored_database = store.load()?;
            sync_active_profile_from_config(&mut restored_database, &codex_home)?;
            store.save(&restored_database)
        });
        manifest.status = if rollback_result.is_ok() {
            "rolledBack".to_string()
        } else {
            "rollbackFailed".to_string()
        };
        let _ = write_manifest(paths, &manifest);
        let _ = clear_transaction_marker(paths);
        return Err(if let Err(rollback_error) = rollback_result {
            ApiError::detailed(
                "switch_commit_and_rollback_failed",
                "切换提交失败，并且自动回滚未能完整完成。",
                format!(
                    "commit: {}; rollback: {}",
                    error.message, rollback_error.message
                ),
            )
        } else {
            error
        });
    }
    if let Err(error) = prune_backups(paths, database.settings.backup_retention) {
        warnings.push(format!("切换已完成，但清理旧备份失败：{}", error.message));
    }

    let mut restarted_codex = false;
    let mut injected_models = false;
    if input.restart_codex {
        match restart_codex(desktop.as_ref(), profile.inject_models) {
            Ok(port) => {
                restarted_codex = true;
                steps.push(step("restartCodex", "success", "Codex 已重新启动。"));
                if let Some(port) = port {
                    match wait_and_inject(port, &profile, &codex_home).await {
                        Ok(()) => {
                            injected_models = true;
                            steps.push(step(
                                "injectModels",
                                "success",
                                "自定义模型目录已注入 Codex Desktop。",
                            ));
                        }
                        Err(error) => {
                            warnings.push(format!(
                                "配置已经生效，但模型菜单注入失败：{}",
                                error.message
                            ));
                            steps.push(step(
                                "injectModels",
                                "failure",
                                "模型菜单注入失败，Provider 配置仍然有效。",
                            ));
                        }
                    }
                } else {
                    steps.push(step(
                        "injectModels",
                        "skipped",
                        "此 Provider 未启用模型菜单注入。",
                    ));
                }
            }
            Err(error) => {
                warnings.push(format!(
                    "配置已经生效，但 Codex 重启失败：{}",
                    error.message
                ));
                steps.push(step(
                    "restartCodex",
                    "failure",
                    "请手动启动 Codex，配置仍然有效。",
                ));
            }
        }
    } else {
        steps.push(step("restartCodex", "skipped", "已按设置跳过 Codex 重启。"));
    }

    let _ = store.append_log(
        "info",
        "provider.switch",
        &format!("已切换到 Provider {}", profile.name),
        json!({
            "profileId": profile.id,
            "providerId": profile.provider_id,
            "baseUrl": profile.base_url,
            "backupId": manifest.id,
            "migratedSessions": migrated_sessions.len()
        }),
    );
    let bootstrap = build_bootstrap(paths, app_version)?;
    Ok(SwitchResult {
        success: true,
        profile_id: Some(profile.id.clone()),
        profile_name: profile.name,
        backup_id: Some(manifest.id),
        restarted_codex,
        injected_models,
        warnings,
        steps,
        bootstrap,
    })
}

pub async fn restore_official(
    paths: &SwitcherPaths,
    force_close: bool,
    restart: bool,
    app_version: &str,
) -> ApiResult<SwitchResult> {
    let store = SwitcherStore::new(paths);
    let mut database = store.load()?;
    let codex_home = resolve_codex_home(&database)?;
    if database.official_snapshot.captured_at_utc.is_none() {
        return Err(ApiError::new(
            "official_snapshot_missing",
            "尚未保存官方配置快照，无法执行一键恢复。",
        ));
    }
    if codex_is_running() {
        request_codex_close();
        if !wait_for_codex_exit(Duration::from_secs(8)) {
            if !force_close {
                return Err(ApiError::new(
                    "codex_force_close_required",
                    "Codex 在 8 秒内未退出，需要确认后强制关闭。",
                ));
            }
            force_close_codex();
            if !wait_for_codex_exit(Duration::from_secs(4)) {
                return Err(ApiError::new(
                    "codex_close_failed",
                    "无法关闭正在运行的 Codex。",
                ));
            }
        }
    }

    let mut manifest = create_backup(
        paths,
        &codex_home,
        "restoreOfficial",
        None,
        database.active_provider_id.as_deref(),
        database.settings.session_migration_days,
    )?;
    write_transaction_marker(paths, &manifest.id, "backup-complete")?;
    let restore_result = (|| -> ApiResult<Vec<String>> {
        merge_official_snapshot(&database, &codex_home)?;
        migrate_sessions(
            &codex_home,
            "openai",
            database.settings.session_migration_days,
        )
    })();
    let migrated = match restore_result {
        Ok(value) => value,
        Err(error) => {
            let _ = restore_manifest(&manifest, paths, true);
            manifest.status = "rolledBack".to_string();
            let _ = write_manifest(paths, &manifest);
            let _ = clear_transaction_marker(paths);
            return Err(error);
        }
    };

    database.active_profile_id = None;
    database.active_provider_id = None;
    let mut warnings = Vec::new();
    let commit_result = (|| -> ApiResult<()> {
        store.save(&database)?;
        finalize_manifest(paths, &codex_home, &mut manifest)?;
        clear_transaction_marker(paths)
    })();
    if let Err(error) = commit_result {
        let rollback_result = restore_manifest(&manifest, paths, true).and_then(|_| {
            let mut restored_database = store.load()?;
            sync_active_profile_from_config(&mut restored_database, &codex_home)?;
            store.save(&restored_database)
        });
        manifest.status = if rollback_result.is_ok() {
            "rolledBack".to_string()
        } else {
            "rollbackFailed".to_string()
        };
        let _ = write_manifest(paths, &manifest);
        let _ = clear_transaction_marker(paths);
        return Err(if let Err(rollback_error) = rollback_result {
            ApiError::detailed(
                "official_commit_and_rollback_failed",
                "官方配置恢复提交失败，并且自动回滚未能完整完成。",
                format!(
                    "commit: {}; rollback: {}",
                    error.message, rollback_error.message
                ),
            )
        } else {
            error
        });
    }
    if let Err(error) = prune_backups(paths, database.settings.backup_retention) {
        warnings.push(format!(
            "官方配置已恢复，但清理旧备份失败：{}",
            error.message
        ));
    }
    let restarted_codex = if restart {
        match restart_codex(detect_codex_desktop().as_ref(), false) {
            Ok(_) => true,
            Err(error) => {
                warnings.push(format!(
                    "官方配置已恢复，但 Codex 重启失败：{}",
                    error.message
                ));
                false
            }
        }
    } else {
        false
    };
    let bootstrap = build_bootstrap(paths, app_version)?;
    Ok(SwitchResult {
        success: true,
        profile_id: None,
        profile_name: "官方 OpenAI".to_string(),
        backup_id: Some(manifest.id),
        restarted_codex,
        injected_models: false,
        warnings,
        steps: vec![
            step("backup", "success", "恢复前备份已创建。"),
            step("restoreOfficial", "success", "官方配置和认证字段已恢复。"),
            step(
                "migrateSessions",
                "success",
                format!("已同步 {} 个会话到 openai Provider。", migrated.len()),
            ),
        ],
        bootstrap,
    })
}

pub fn list_backups(paths: &SwitcherPaths) -> ApiResult<Vec<BackupSummary>> {
    fs::create_dir_all(&paths.backups_root).map_err(|error| {
        ApiError::detailed(
            "backup_directory_failed",
            "创建备份目录失败。",
            error.to_string(),
        )
    })?;
    let mut backups = Vec::new();
    for entry in fs::read_dir(&paths.backups_root).map_err(|error| {
        ApiError::detailed(
            "backup_list_failed",
            "读取备份列表失败。",
            error.to_string(),
        )
    })? {
        let entry = entry.map_err(|error| {
            ApiError::detailed("backup_entry_failed", "读取备份项失败。", error.to_string())
        })?;
        if !entry.path().is_dir() {
            continue;
        }
        let manifest_path = entry.path().join("manifest.json");
        let Ok(text) = fs::read_to_string(manifest_path) else {
            continue;
        };
        let Ok(manifest) = serde_json::from_str::<BackupManifest>(&text) else {
            continue;
        };
        backups.push(BackupSummary {
            id: manifest.id,
            created_at_utc: manifest.created_at_utc,
            kind: manifest.kind,
            profile_name: manifest.profile_name,
            status: manifest.status,
            file_count: manifest.files.len(),
        });
    }
    backups.sort_by(|left, right| right.created_at_utc.cmp(&left.created_at_utc));
    Ok(backups)
}

pub fn restore_backup(
    paths: &SwitcherPaths,
    backup_id: &str,
    force: bool,
    app_version: &str,
) -> ApiResult<SwitcherBootstrap> {
    if codex_is_running() {
        return Err(ApiError::new(
            "codex_must_be_closed",
            "恢复备份前请先关闭 Codex，避免覆盖正在使用的会话和数据库文件。",
        ));
    }
    let manifest = read_manifest(paths, backup_id)?;
    restore_manifest(&manifest, paths, force)?;
    let store = SwitcherStore::new(paths);
    let mut database = store.load()?;
    sync_active_profile_from_config(&mut database, Path::new(&manifest.codex_home))?;
    store.save(&database)?;
    let _ = store.append_log(
        "info",
        "backup.restore",
        &format!("已恢复备份 {backup_id}"),
        json!({ "backupId": backup_id }),
    );
    build_bootstrap(paths, app_version)
}

pub fn read_logs(paths: &SwitcherPaths, limit: usize) -> ApiResult<Vec<Value>> {
    if !paths.log_path.exists() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(&paths.log_path).map_err(|error| {
        ApiError::detailed("log_read_failed", "读取日志失败。", error.to_string())
    })?;
    let mut values: Vec<Value> = text
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    let keep = limit.clamp(1, 500);
    if values.len() > keep {
        values.drain(0..values.len() - keep);
    }
    values.reverse();
    Ok(values)
}

pub fn open_directory(path: &Path) -> ApiResult<()> {
    fs::create_dir_all(path).map_err(|error| {
        ApiError::detailed(
            "directory_create_failed",
            "创建目录失败。",
            error.to_string(),
        )
    })?;
    Command::new("explorer.exe")
        .arg(path)
        .spawn()
        .map_err(|error| {
            ApiError::detailed("directory_open_failed", "打开目录失败。", error.to_string())
        })?;
    Ok(())
}

fn find_profile<'a>(
    database: &'a SwitcherDatabase,
    profile_id: &str,
) -> ApiResult<&'a StoredProviderProfile> {
    database
        .profiles
        .iter()
        .find(|profile| profile.id == profile_id)
        .ok_or_else(|| ApiError::new("provider_not_found", "没有找到指定的 Provider。"))
}

fn ensure_default_model(profile: &StoredProviderProfile) -> ApiResult<()> {
    if profile.default_model.trim().is_empty() {
        return Err(ApiError::new(
            "default_model_missing",
            "请先获取模型并选择默认模型。",
        ));
    }
    Ok(())
}

fn recover_incomplete_transaction(
    paths: &SwitcherPaths,
    store: &SwitcherStore<'_>,
) -> ApiResult<()> {
    if !paths.transaction_path.exists() {
        return Ok(());
    }
    if codex_is_running() {
        return Err(ApiError::new(
            "incomplete_transaction_codex_running",
            "检测到上次未完成的切换事务。请关闭 Codex 后重新打开本工具，应用将自动回滚。",
        ));
    }

    let marker_text = fs::read_to_string(&paths.transaction_path).map_err(|error| {
        ApiError::detailed(
            "transaction_marker_read_failed",
            "读取未完成事务检查点失败。",
            error.to_string(),
        )
    })?;
    let marker: Value = serde_json::from_str(&marker_text).map_err(|error| {
        ApiError::detailed(
            "transaction_marker_invalid",
            "未完成事务检查点格式无效。",
            error.to_string(),
        )
    })?;
    let backup_id = marker
        .get("backupId")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            ApiError::new(
                "transaction_backup_missing",
                "未完成事务检查点没有关联备份。",
            )
        })?;
    let mut manifest = read_manifest(paths, backup_id)?;
    restore_manifest(&manifest, paths, true)?;

    let mut database = store.load()?;
    sync_active_profile_from_config(&mut database, Path::new(&manifest.codex_home))?;
    store.save(&database)?;
    manifest.status = "interruptedRolledBack".to_string();
    write_manifest(paths, &manifest)?;
    clear_transaction_marker(paths)?;
    let _ = store.append_log(
        "warning",
        "transaction.recover",
        &format!("已自动回滚未完成事务 {backup_id}"),
        json!({ "backupId": backup_id }),
    );
    Ok(())
}

fn sync_active_profile_from_config(
    database: &mut SwitcherDatabase,
    codex_home: &Path,
) -> ApiResult<()> {
    let provider_id = read_optional_text(&codex_home.join("config.toml"))?
        .and_then(|text| text.parse::<DocumentMut>().ok())
        .and_then(|document| {
            document
                .get("model_provider")
                .and_then(Item::as_value)
                .and_then(toml_edit::Value::as_str)
                .map(str::to_string)
        });
    let matching_profile = provider_id.as_deref().and_then(|provider_id| {
        database
            .profiles
            .iter()
            .find(|profile| profile.provider_id == provider_id)
    });
    database.active_profile_id = matching_profile.map(|profile| profile.id.clone());
    database.active_provider_id = matching_profile.map(|profile| profile.provider_id.clone());
    Ok(())
}

fn resolve_codex_home(database: &SwitcherDatabase) -> ApiResult<PathBuf> {
    if let Some(path) = database.settings.codex_home_override.as_deref() {
        return Ok(PathBuf::from(path));
    }
    if let Some(path) = std::env::var_os("CODEX_HOME").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(path));
    }
    dirs::home_dir()
        .map(|home| home.join(".codex"))
        .ok_or_else(|| ApiError::new("home_directory_missing", "无法识别用户主目录。"))
}

fn capture_official_snapshot_if_needed(
    database: &mut SwitcherDatabase,
    codex_home: &Path,
) -> ApiResult<()> {
    if database.active_profile_id.is_some() {
        return Ok(());
    }
    let config_text = read_optional_text(&codex_home.join("config.toml"))?
        .map(|text| canonical_official_config_snapshot(database, &text))
        .transpose()?;
    let auth_text = read_optional_text(&codex_home.join("auth.json"))?;
    let official_auth_text = auth_text
        .as_deref()
        .map(canonical_official_auth_snapshot)
        .transpose()?;
    let protected_auth_text = official_auth_text
        .as_deref()
        .map(protect_to_base64)
        .transpose()
        .map_err(|error| {
            ApiError::detailed(
                "official_auth_encrypt_failed",
                "保存官方认证快照失败。",
                error.to_string(),
            )
        })?;
    database.official_snapshot = OfficialSnapshot {
        config_text,
        protected_auth_text,
        captured_at_utc: Some(Utc::now()),
    };
    Ok(())
}

fn canonical_official_config_snapshot(
    database: &SwitcherDatabase,
    config_text: &str,
) -> ApiResult<String> {
    let mut document = if config_text.trim().is_empty() {
        DocumentMut::new()
    } else {
        config_text.parse::<DocumentMut>().map_err(|error| {
            ApiError::detailed(
                "config_toml_invalid",
                "现有 config.toml 不是有效 TOML，无法保存官方快照。",
                error.to_string(),
            )
        })?
    };
    document["model_provider"] = value("openai");
    document.as_table_mut().remove("model_catalog_json");
    for profile in &database.profiles {
        remove_provider_table(&mut document, &profile.provider_id);
    }
    Ok(document.to_string())
}

fn canonical_official_auth_snapshot(auth_text: &str) -> ApiResult<String> {
    let mut auth = serde_json::from_str::<Value>(auth_text)
        .map_err(|error| {
            ApiError::detailed(
                "auth_json_invalid",
                "现有 auth.json 不是有效 JSON，无法保存官方快照。",
                error.to_string(),
            )
        })?
        .as_object()
        .cloned()
        .ok_or_else(|| ApiError::new("auth_json_invalid", "auth.json 必须是 JSON 对象。"))?;
    if is_chatgpt_auth(&auth) {
        auth.remove("OPENAI_API_KEY");
    }
    serde_json::to_string_pretty(&auth).map_err(|error| {
        ApiError::detailed(
            "auth_serialize_failed",
            "生成官方认证快照失败。",
            error.to_string(),
        )
    })
}

fn apply_provider_files(
    database: &SwitcherDatabase,
    profile: &StoredProviderProfile,
    codex_home: &Path,
    store: &SwitcherStore<'_>,
) -> ApiResult<()> {
    ensure_default_model(profile)?;
    let config_path = codex_home.join("config.toml");
    let existing_config = read_optional_text(&config_path)?.unwrap_or_default();
    let mut document = if existing_config.trim().is_empty() {
        DocumentMut::new()
    } else {
        existing_config.parse::<DocumentMut>().map_err(|error| {
            ApiError::detailed(
                "config_toml_invalid",
                "现有 config.toml 不是有效 TOML，未进行修改。",
                error.to_string(),
            )
        })?
    };
    if let Some(previous_provider) = database.active_provider_id.as_deref() {
        remove_provider_table(&mut document, previous_provider);
    }
    document["model_provider"] = value(profile.provider_id.clone());
    document["model"] = value(profile.default_model.clone());
    let catalog_relative_path = catalog_relative_path(&profile.provider_id);
    document["model_catalog_json"] = value(catalog_relative_path.clone());
    if document.get("model_providers").is_none() {
        document["model_providers"] = table();
    }
    let providers = document["model_providers"].as_table_mut().ok_or_else(|| {
        ApiError::new(
            "model_providers_not_table",
            "config.toml 中的 model_providers 不是表结构。",
        )
    })?;
    providers.insert(&profile.provider_id, table());
    let provider_table = providers[&profile.provider_id]
        .as_table_mut()
        .ok_or_else(|| ApiError::new("provider_table_failed", "创建 Provider 配置失败。"))?;
    provider_table.insert("name", value(profile.name.clone()));
    provider_table.insert("base_url", value(profile.base_url.clone()));
    provider_table.insert("wire_api", value("responses"));
    provider_table.insert("supports_websockets", value(false));
    provider_table.insert("auth", table());
    let auth_table = provider_table["auth"].as_table_mut().ok_or_else(|| {
        ApiError::new("provider_auth_table_failed", "创建 Provider 认证配置失败。")
    })?;
    let helper_path = std::env::current_exe().map_err(|error| {
        ApiError::detailed(
            "credential_helper_path_failed",
            "无法识别凭据助手路径。",
            error.to_string(),
        )
    })?;
    let mut helper_args = Array::new();
    helper_args.push("--codex-provider-token");
    helper_args.push(profile.id.clone());
    auth_table.insert("command", value(helper_path.to_string_lossy().to_string()));
    auth_table.insert("args", value(helper_args));
    auth_table.insert("timeout_ms", value(5_000));
    auth_table.insert("refresh_interval_ms", value(0));
    atomic_write(&config_path, document.to_string().as_bytes())?;
    store.provider_secret(profile)?;

    let catalog_path = codex_home.join(&catalog_relative_path);
    let catalog = build_model_catalog(profile, codex_home)?;
    atomic_write(&catalog_path, catalog.as_bytes())?;

    if let Some(previous_provider) = database
        .active_provider_id
        .as_deref()
        .filter(|provider_id| *provider_id != profile.provider_id)
    {
        remove_managed_catalog(codex_home, previous_provider)?;
    }
    remove_legacy_catalog(codex_home)
}

fn merge_official_snapshot(database: &SwitcherDatabase, codex_home: &Path) -> ApiResult<()> {
    let config_path = codex_home.join("config.toml");
    let current_text = read_optional_text(&config_path)?.unwrap_or_default();
    let baseline_text = database
        .official_snapshot
        .config_text
        .as_deref()
        .unwrap_or_default();
    let mut current = if current_text.trim().is_empty() {
        DocumentMut::new()
    } else {
        current_text.parse::<DocumentMut>().map_err(|error| {
            ApiError::detailed(
                "config_toml_invalid",
                "现有 config.toml 不是有效 TOML。",
                error.to_string(),
            )
        })?
    };
    let baseline = if baseline_text.trim().is_empty() {
        DocumentMut::new()
    } else {
        baseline_text.parse::<DocumentMut>().map_err(|error| {
            ApiError::detailed(
                "official_config_invalid",
                "保存的官方配置快照不是有效 TOML。",
                error.to_string(),
            )
        })?
    };
    restore_toml_item(&mut current, &baseline, "model");
    current["model_provider"] = value("openai");
    current.as_table_mut().remove("model_catalog_json");
    if let Some(provider_id) = database.active_provider_id.as_deref() {
        remove_provider_table(&mut current, provider_id);
    }
    atomic_write(&config_path, current.to_string().as_bytes())?;

    let auth_path = codex_home.join("auth.json");
    let current_auth = read_optional_text(&auth_path)?
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    let baseline_auth = database
        .official_snapshot
        .protected_auth_text
        .as_deref()
        .map(unprotect_from_base64)
        .transpose()
        .map_err(|error| {
            ApiError::detailed(
                "official_auth_decrypt_failed",
                "解密官方认证快照失败。",
                error.to_string(),
            )
        })?
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .and_then(|value| value.as_object().cloned());
    let mut official_auth = select_official_auth(current_auth, baseline_auth);
    if is_chatgpt_auth(&official_auth) {
        official_auth.remove("OPENAI_API_KEY");
    }
    let auth_text = serde_json::to_string_pretty(&official_auth).map_err(|error| {
        ApiError::detailed(
            "auth_serialize_failed",
            "生成官方 auth.json 失败。",
            error.to_string(),
        )
    })?;
    atomic_write(&auth_path, auth_text.as_bytes())?;
    if let Some(provider_id) = database.active_provider_id.as_deref() {
        remove_managed_catalog(codex_home, provider_id)?;
    }
    remove_legacy_catalog(codex_home)
}

fn select_official_auth(
    current_auth: serde_json::Map<String, Value>,
    baseline_auth: Option<serde_json::Map<String, Value>>,
) -> serde_json::Map<String, Value> {
    if has_chatgpt_tokens(&current_auth) {
        let mut selected = baseline_auth.unwrap_or_default();
        for key in ["auth_mode", "tokens", "last_refresh"] {
            if let Some(value) = current_auth.get(key).cloned() {
                selected.insert(key.to_string(), value);
            }
        }
        return selected;
    }
    baseline_auth.unwrap_or(current_auth)
}

fn is_chatgpt_auth(auth: &serde_json::Map<String, Value>) -> bool {
    auth.get("auth_mode")
        .and_then(Value::as_str)
        .is_some_and(|mode| mode.eq_ignore_ascii_case("chatgpt"))
}

fn has_chatgpt_tokens(auth: &serde_json::Map<String, Value>) -> bool {
    is_chatgpt_auth(auth)
        && auth
            .get("tokens")
            .and_then(Value::as_object)
            .is_some_and(|tokens| {
                tokens
                    .get("access_token")
                    .and_then(Value::as_str)
                    .is_some_and(|token| !token.trim().is_empty())
                    && tokens
                        .get("refresh_token")
                        .and_then(Value::as_str)
                        .is_some_and(|token| !token.trim().is_empty())
            })
}

fn restore_toml_item(current: &mut DocumentMut, baseline: &DocumentMut, key: &str) {
    if let Some(item) = baseline.get(key) {
        current[key] = item.clone();
    } else {
        current.as_table_mut().remove(key);
    }
}

fn remove_provider_table(document: &mut DocumentMut, provider_id: &str) {
    if let Some(providers) = document
        .get_mut("model_providers")
        .and_then(Item::as_table_mut)
    {
        providers.remove(provider_id);
        if providers.is_empty() {
            document.as_table_mut().remove("model_providers");
        }
    }
}

fn build_model_catalog(profile: &StoredProviderProfile, codex_home: &Path) -> ApiResult<String> {
    let models = build_model_catalog_entries(profile, codex_home)?;
    serde_json::to_string_pretty(&json!({ "models": models })).map_err(|error| {
        ApiError::detailed(
            "catalog_serialize_failed",
            "生成模型目录失败。",
            error.to_string(),
        )
    })
}

fn build_model_catalog_entries(
    profile: &StoredProviderProfile,
    codex_home: &Path,
) -> ApiResult<Vec<Value>> {
    let template: Value = serde_json::from_str(CATALOG_TEMPLATE).map_err(|error| {
        ApiError::detailed(
            "catalog_template_invalid",
            "内置模型目录模板无效。",
            error.to_string(),
        )
    })?;
    let native_templates = load_native_model_templates(codex_home);
    let models = profile
        .models
        .iter()
        .enumerate()
        .filter_map(|(index, model)| {
            let mut entry = native_templates
                .get(&model.id.to_ascii_lowercase())
                .cloned()
                .unwrap_or_else(|| template.clone());
            let object = entry.as_object_mut()?;
            object.insert("slug".to_string(), json!(model.id));
            object.insert("display_name".to_string(), json!(model.display_name));
            object.insert("description".to_string(), json!(model.display_name));
            object.insert("priority".to_string(), json!(1000 + index));
            object.insert("visibility".to_string(), json!("list"));
            object.insert("supported_in_api".to_string(), json!(true));
            if let Some(context) = model.context_window {
                object.insert("context_window".to_string(), json!(context));
                object.insert("max_context_window".to_string(), json!(context));
            }
            Some(entry)
        })
        .collect();
    Ok(models)
}

fn load_native_model_templates(codex_home: &Path) -> HashMap<String, Value> {
    let cache_path = codex_home.join("models_cache.json");
    let Ok(text) = fs::read_to_string(cache_path) else {
        return HashMap::new();
    };
    let Ok(cache) = serde_json::from_str::<Value>(&text) else {
        return HashMap::new();
    };
    cache
        .get("models")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|model| {
            let slug = model.get("slug")?.as_str()?.trim();
            if slug.is_empty() || !model.is_object() {
                return None;
            }
            Some((slug.to_ascii_lowercase(), model.clone()))
        })
        .collect()
}

fn migrate_sessions(codex_home: &Path, provider_id: &str, days: u32) -> ApiResult<Vec<String>> {
    if days == 0 {
        return Ok(Vec::new());
    }
    let session_files = collect_session_files(codex_home, days)?;
    let mut session_ids = Vec::new();
    for path in session_files {
        let text = fs::read_to_string(&path).map_err(|error| {
            ApiError::detailed(
                "session_read_failed",
                format!("读取会话文件失败：{}", path.display()),
                error.to_string(),
            )
        })?;
        let (first_line, rest) = text
            .split_once('\n')
            .map(|(first, rest)| (first, Some(rest)))
            .unwrap_or((&text, None));
        let Ok(mut value) = serde_json::from_str::<Value>(first_line) else {
            continue;
        };
        let target = if value.get("type").and_then(Value::as_str) == Some("session_meta") {
            value.get_mut("payload")
        } else {
            Some(&mut value)
        };
        let Some(object) = target.and_then(Value::as_object_mut) else {
            continue;
        };
        let session_id = object
            .get("id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| {
                path.file_stem()
                    .and_then(|value| value.to_str())
                    .map(str::to_string)
            });
        if object.get("model_provider").and_then(Value::as_str) == Some(provider_id) {
            if let Some(session_id) = session_id {
                session_ids.push(session_id);
            }
            continue;
        }
        object.insert(
            "model_provider".to_string(),
            Value::String(provider_id.to_string()),
        );
        let mut output = serde_json::to_string(&value).map_err(|error| {
            ApiError::detailed(
                "session_serialize_failed",
                "序列化会话元数据失败。",
                error.to_string(),
            )
        })?;
        if let Some(rest) = rest {
            output.push('\n');
            output.push_str(rest);
        }
        atomic_write(&path, output.as_bytes())?;
        if let Some(session_id) = session_id {
            session_ids.push(session_id);
        }
    }
    update_state_database(codex_home, provider_id, &session_ids)?;
    Ok(session_ids)
}

fn update_state_database(
    codex_home: &Path,
    provider_id: &str,
    session_ids: &[String],
) -> ApiResult<()> {
    let database_path = codex_home.join("state_5.sqlite");
    if session_ids.is_empty() || !database_path.exists() {
        return Ok(());
    }
    let mut connection = Connection::open_with_flags(
        &database_path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|error| {
        ApiError::detailed(
            "state_database_open_failed",
            "打开 state_5.sqlite 失败。",
            error.to_string(),
        )
    })?;
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(|error| {
            ApiError::detailed(
                "state_database_timeout_failed",
                "设置 SQLite 超时失败。",
                error.to_string(),
            )
        })?;
    let has_column = {
        let mut statement = connection
            .prepare("PRAGMA table_info(threads)")
            .map_err(sqlite_error)?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(1))
            .map_err(sqlite_error)?;
        let found = rows
            .filter_map(Result::ok)
            .any(|column| column == "model_provider");
        found
    };
    if !has_column {
        return Ok(());
    }
    let transaction = connection.transaction().map_err(sqlite_error)?;
    {
        let mut statement = transaction
            .prepare("UPDATE threads SET model_provider = ?1 WHERE id = ?2")
            .map_err(sqlite_error)?;
        for session_id in session_ids {
            statement
                .execute((provider_id, session_id))
                .map_err(sqlite_error)?;
        }
    }
    transaction.commit().map_err(sqlite_error)
}

fn collect_session_files(codex_home: &Path, days: u32) -> ApiResult<Vec<PathBuf>> {
    let root = codex_home.join("sessions");
    if !root.exists() {
        return Ok(Vec::new());
    }
    let cutoff = SystemTime::now()
        .checked_sub(Duration::from_secs(u64::from(days) * 24 * 60 * 60))
        .unwrap_or(SystemTime::UNIX_EPOCH);
    let mut files = Vec::new();
    for entry in WalkDir::new(root) {
        let entry = entry.map_err(|error| {
            ApiError::detailed(
                "session_scan_failed",
                "扫描会话目录失败。",
                error.to_string(),
            )
        })?;
        if !entry.file_type().is_file()
            || entry.path().extension().and_then(|value| value.to_str()) != Some("jsonl")
        {
            continue;
        }
        let modified = entry
            .metadata()
            .ok()
            .and_then(|metadata| metadata.modified().ok())
            .unwrap_or(SystemTime::UNIX_EPOCH);
        if modified >= cutoff {
            files.push(entry.path().to_path_buf());
        }
    }
    Ok(files)
}

fn create_backup(
    paths: &SwitcherPaths,
    codex_home: &Path,
    kind: &str,
    profile: Option<&StoredProviderProfile>,
    previous_provider_id: Option<&str>,
    session_days: u32,
) -> ApiResult<BackupManifest> {
    fs::create_dir_all(&paths.backups_root).map_err(|error| {
        ApiError::detailed(
            "backup_directory_failed",
            "创建备份目录失败。",
            error.to_string(),
        )
    })?;
    let id = format!(
        "{}-{}",
        Utc::now().format("%Y%m%d-%H%M%S"),
        &Uuid::new_v4().simple().to_string()[..8]
    );
    let backup_dir = paths.backups_root.join(&id);
    let files_dir = backup_dir.join("files");
    fs::create_dir_all(&files_dir).map_err(|error| {
        ApiError::detailed("backup_create_failed", "创建备份失败。", error.to_string())
    })?;
    let mut targets = vec![
        codex_home.join("config.toml"),
        codex_home.join("auth.json"),
        codex_home.join(LEGACY_CATALOG_RELATIVE_PATH),
        codex_home.join("state_5.sqlite"),
        codex_home.join("state_5.sqlite-wal"),
        codex_home.join("state_5.sqlite-shm"),
    ];
    if let Some(profile) = profile {
        targets.push(codex_home.join(catalog_relative_path(&profile.provider_id)));
    }
    if let Some(provider_id) = previous_provider_id {
        targets.push(codex_home.join(catalog_relative_path(provider_id)));
    }
    targets.extend(collect_session_files(codex_home, session_days)?);
    targets.sort();
    targets.dedup();

    let mut records = Vec::new();
    for target in targets {
        let relative = target.strip_prefix(codex_home).map_err(|error| {
            ApiError::detailed(
                "backup_relative_path_failed",
                "计算备份相对路径失败。",
                error.to_string(),
            )
        })?;
        let relative_string = relative.to_string_lossy().replace('\\', "/");
        let existed = target.exists();
        if existed {
            let destination = files_dir.join(relative);
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent).map_err(|error| {
                    ApiError::detailed(
                        "backup_parent_failed",
                        "创建备份子目录失败。",
                        error.to_string(),
                    )
                })?;
            }
            fs::copy(&target, &destination).map_err(|error| {
                ApiError::detailed(
                    "backup_copy_failed",
                    format!("备份文件失败：{}", relative_string),
                    error.to_string(),
                )
            })?;
        }
        records.push(BackupFileRecord {
            relative_path: relative_string,
            existed_before: existed,
            sha256_before: if existed {
                Some(hash_file(&target)?)
            } else {
                None
            },
            sha256_after: None,
        });
    }
    let manifest = BackupManifest {
        id,
        created_at_utc: Utc::now(),
        kind: kind.to_string(),
        profile_id: profile.map(|value| value.id.clone()),
        profile_name: profile.map(|value| value.name.clone()),
        codex_home: codex_home.to_string_lossy().to_string(),
        status: "inProgress".to_string(),
        files: records,
    };
    write_manifest(paths, &manifest)?;
    Ok(manifest)
}

fn finalize_manifest(
    paths: &SwitcherPaths,
    codex_home: &Path,
    manifest: &mut BackupManifest,
) -> ApiResult<()> {
    for record in &mut manifest.files {
        let path = safe_join(codex_home, &record.relative_path)?;
        record.sha256_after = if path.exists() {
            Some(hash_file(&path)?)
        } else {
            None
        };
    }
    manifest.status = "complete".to_string();
    write_manifest(paths, manifest)
}

fn write_manifest(paths: &SwitcherPaths, manifest: &BackupManifest) -> ApiResult<()> {
    let path = paths.backups_root.join(&manifest.id).join("manifest.json");
    let text = serde_json::to_string_pretty(manifest).map_err(|error| {
        ApiError::detailed(
            "manifest_serialize_failed",
            "生成备份清单失败。",
            error.to_string(),
        )
    })?;
    atomic_write(&path, text.as_bytes())
}

fn read_manifest(paths: &SwitcherPaths, backup_id: &str) -> ApiResult<BackupManifest> {
    if backup_id.contains('/') || backup_id.contains('\\') || backup_id.contains("..") {
        return Err(ApiError::new("backup_id_invalid", "备份 ID 无效。"));
    }
    let path = paths.backups_root.join(backup_id).join("manifest.json");
    let text = fs::read_to_string(&path).map_err(|error| {
        ApiError::detailed(
            "manifest_read_failed",
            "读取备份清单失败。",
            error.to_string(),
        )
    })?;
    serde_json::from_str(&text).map_err(|error| {
        ApiError::detailed("manifest_invalid", "备份清单格式无效。", error.to_string())
    })
}

fn restore_manifest(
    manifest: &BackupManifest,
    paths: &SwitcherPaths,
    force: bool,
) -> ApiResult<()> {
    let codex_home = PathBuf::from(&manifest.codex_home);
    let files_root = paths.backups_root.join(&manifest.id).join("files");
    for record in &manifest.files {
        let target = safe_join(&codex_home, &record.relative_path)?;
        if !force {
            let current = if target.exists() {
                Some(hash_file(&target)?)
            } else {
                None
            };
            if current != record.sha256_after {
                return Err(ApiError::new(
                    "backup_conflict",
                    format!(
                        "文件 {} 在备份后被其他程序修改，需要确认强制恢复。",
                        record.relative_path
                    ),
                ));
            }
        }
        if record.existed_before {
            let source = safe_join(&files_root, &record.relative_path)?;
            let bytes = fs::read(&source).map_err(|error| {
                ApiError::detailed(
                    "backup_source_read_failed",
                    format!("读取备份文件失败：{}", record.relative_path),
                    error.to_string(),
                )
            })?;
            atomic_write(&target, &bytes)?;
        } else if target.exists() {
            fs::remove_file(&target).map_err(|error| {
                ApiError::detailed(
                    "backup_created_file_remove_failed",
                    format!("删除切换后创建的文件失败：{}", record.relative_path),
                    error.to_string(),
                )
            })?;
        }
    }
    Ok(())
}

fn prune_backups(paths: &SwitcherPaths, keep: usize) -> ApiResult<()> {
    let backups = list_backups(paths)?;
    for backup in backups.into_iter().skip(keep.max(1)) {
        let target = paths.backups_root.join(&backup.id);
        let resolved = target.canonicalize().unwrap_or(target.clone());
        let root = paths
            .backups_root
            .canonicalize()
            .unwrap_or(paths.backups_root.clone());
        if !resolved.starts_with(&root) || resolved == root {
            return Err(ApiError::new(
                "backup_prune_path_invalid",
                "拒绝清理不在备份根目录内的路径。",
            ));
        }
        fs::remove_dir_all(&resolved).map_err(|error| {
            ApiError::detailed(
                "backup_prune_failed",
                format!("清理旧备份失败：{}", backup.id),
                error.to_string(),
            )
        })?;
    }
    Ok(())
}

fn write_transaction_marker(paths: &SwitcherPaths, backup_id: &str, stage: &str) -> ApiResult<()> {
    let value = json!({
        "backupId": backup_id,
        "stage": stage,
        "updatedAtUtc": Utc::now()
    });
    atomic_write(
        &paths.transaction_path,
        serde_json::to_string_pretty(&value)
            .unwrap_or_else(|_| "{}".to_string())
            .as_bytes(),
    )
}

fn update_transaction_stage(paths: &SwitcherPaths, backup_id: &str, stage: &str) -> ApiResult<()> {
    write_transaction_marker(paths, backup_id, stage)
}

fn clear_transaction_marker(paths: &SwitcherPaths) -> ApiResult<()> {
    if paths.transaction_path.exists() {
        fs::remove_file(&paths.transaction_path).map_err(|error| {
            ApiError::detailed(
                "transaction_marker_remove_failed",
                "清理事务检查点失败。",
                error.to_string(),
            )
        })?;
    }
    Ok(())
}

fn hash_file(path: &Path) -> ApiResult<String> {
    let mut file = fs::File::open(path).map_err(|error| {
        ApiError::detailed(
            "hash_file_open_failed",
            format!("打开文件计算哈希失败：{}", path.display()),
            error.to_string(),
        )
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|error| {
            ApiError::detailed(
                "hash_file_read_failed",
                format!("读取文件计算哈希失败：{}", path.display()),
                error.to_string(),
            )
        })?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn safe_join(root: &Path, relative: &str) -> ApiResult<PathBuf> {
    let relative_path = Path::new(relative);
    if relative_path.is_absolute()
        || relative_path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(ApiError::new(
            "backup_path_invalid",
            "备份清单包含不安全路径。",
        ));
    }
    Ok(root.join(relative_path))
}

fn read_optional_text(path: &Path) -> ApiResult<Option<String>> {
    if !path.exists() {
        return Ok(None);
    }
    fs::read_to_string(path).map(Some).map_err(|error| {
        ApiError::detailed(
            "file_read_failed",
            format!("读取文件失败：{}", path.display()),
            error.to_string(),
        )
    })
}

fn atomic_write(path: &Path, bytes: &[u8]) -> ApiResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            ApiError::detailed(
                "directory_create_failed",
                format!("创建目录失败：{}", parent.display()),
                error.to_string(),
            )
        })?;
    }
    let temp_path = path.with_extension(format!(
        "{}.tmp",
        path.extension()
            .and_then(|value| value.to_str())
            .unwrap_or("file")
    ));
    {
        let mut file = fs::File::create(&temp_path).map_err(|error| {
            ApiError::detailed(
                "temp_file_create_failed",
                "创建临时文件失败。",
                error.to_string(),
            )
        })?;
        file.write_all(bytes).map_err(|error| {
            ApiError::detailed(
                "temp_file_write_failed",
                "写入临时文件失败。",
                error.to_string(),
            )
        })?;
        file.sync_all().map_err(|error| {
            ApiError::detailed(
                "temp_file_sync_failed",
                "同步临时文件失败。",
                error.to_string(),
            )
        })?;
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
        fs::remove_file(destination).map_err(|error| {
            ApiError::detailed(
                "atomic_replace_remove_failed",
                "替换旧文件失败。",
                error.to_string(),
            )
        })?;
    }
    fs::rename(source, destination).map_err(|error| {
        ApiError::detailed(
            "atomic_replace_failed",
            "原子替换文件失败。",
            error.to_string(),
        )
    })
}

fn api_endpoint(base_url: &str, route: &str) -> String {
    let base_url = base_url.trim_end_matches('/');
    let needs_v1 = reqwest::Url::parse(base_url)
        .ok()
        .is_some_and(|url| matches!(url.path(), "" | "/"));
    if needs_v1 {
        format!("{base_url}/v1/{route}")
    } else {
        format!("{base_url}/{route}")
    }
}

fn catalog_relative_path(provider_id: &str) -> String {
    format!("model-catalogs/{provider_id}.json")
}

fn remove_managed_catalog(codex_home: &Path, provider_id: &str) -> ApiResult<()> {
    remove_catalog_file(&codex_home.join(catalog_relative_path(provider_id)))
}

fn remove_legacy_catalog(codex_home: &Path) -> ApiResult<()> {
    remove_catalog_file(&codex_home.join(LEGACY_CATALOG_RELATIVE_PATH))
}

fn remove_catalog_file(path: &Path) -> ApiResult<()> {
    if !path.exists() {
        return Ok(());
    }
    fs::remove_file(path).map_err(|error| {
        ApiError::detailed(
            "catalog_remove_failed",
            "删除自定义模型目录失败。",
            error.to_string(),
        )
    })
}

fn http_client_error(error: reqwest::Error) -> ApiError {
    ApiError::detailed(
        "http_client_failed",
        "创建网络客户端失败。",
        error.to_string(),
    )
}

fn http_error(code: &str, message: &str, error: reqwest::Error) -> ApiError {
    ApiError::detailed(code, message, error.to_string())
}

fn status_error(status: StatusCode, body: &str, action: &str) -> ApiError {
    let message = match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
            format!("{action}失败：API Key 未通过认证。")
        }
        StatusCode::TOO_MANY_REQUESTS => format!("{action}失败：上游返回 429 限流。"),
        StatusCode::NOT_FOUND => format!("{action}失败：接口地址不存在，请检查 /v1 路径。"),
        _ => format!("{action}失败：HTTP {}。", status.as_u16()),
    };
    let detail: String = body.chars().take(500).collect();
    ApiError::detailed("upstream_http_error", message, detail)
}

fn sqlite_error(error: rusqlite::Error) -> ApiError {
    ApiError::detailed(
        "sqlite_operation_failed",
        "更新 state_5.sqlite 失败。",
        error.to_string(),
    )
}

fn step(
    phase: impl Into<String>,
    status: impl Into<String>,
    message: impl Into<String>,
) -> SwitchStep {
    SwitchStep {
        phase: phase.into(),
        status: status.into(),
        message: message.into(),
    }
}

fn detect_codex_package() -> Option<Value> {
    let script = format!(
        "Get-AppxPackage -Name {CODEX_PACKAGE_NAME} | Select-Object Version,InstallLocation,PackageFamilyName | ConvertTo-Json -Compress"
    );
    let output = run_powershell(&script)?;
    serde_json::from_str(output.trim()).ok()
}

fn detect_codex_desktop() -> Option<CodexDesktopInstallation> {
    let package = detect_codex_package()?;
    let install = package.get("InstallLocation")?.as_str()?;
    let package_family_name = package.get("PackageFamilyName")?.as_str()?;
    let executable = Path::new(install).join("app").join("ChatGPT.exe");
    executable.exists().then(|| CodexDesktopInstallation {
        executable_path: executable.to_string_lossy().to_string(),
        app_user_model_id: format!("{package_family_name}!{CODEX_APPLICATION_ID}"),
    })
}

fn codex_is_running() -> bool {
    let Some(desktop) = detect_codex_desktop() else {
        return false;
    };
    let executable = powershell_single_quote(&desktop.executable_path);
    let script = format!(
        "if (Get-Process -Name ChatGPT -ErrorAction SilentlyContinue | Where-Object {{ $_.Path -ieq '{executable}' }}) {{ '1' }} else {{ '0' }}"
    );
    run_powershell(&script).is_some_and(|value| value.trim() == "1")
}

fn request_codex_close() {
    let Some(desktop) = detect_codex_desktop() else {
        return;
    };
    let executable = powershell_single_quote(&desktop.executable_path);
    let script = format!(
        "Get-Process -Name ChatGPT -ErrorAction SilentlyContinue | Where-Object {{ $_.Path -ieq '{executable}' -and $_.MainWindowHandle -ne 0 }} | ForEach-Object {{ [void]$_.CloseMainWindow() }}"
    );
    let _ = run_powershell(&script);
}

fn force_close_codex() {
    let Some(desktop) = detect_codex_desktop() else {
        return;
    };
    let executable = powershell_single_quote(&desktop.executable_path);
    let script = format!(
        "$roots = Get-CimInstance Win32_Process -Filter \"Name='ChatGPT.exe'\" | Where-Object {{ $_.ExecutablePath -ieq '{executable}' -and $_.CommandLine -notmatch '(?:^|\\s)--type=' }}; foreach ($process in $roots) {{ & taskkill.exe /PID $process.ProcessId /T /F *> $null }}"
    );
    let _ = run_powershell(&script);
}

fn wait_for_codex_exit(timeout: Duration) -> bool {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if !codex_is_running() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    !codex_is_running()
}

fn restart_codex(
    desktop: Option<&CodexDesktopInstallation>,
    inject_models: bool,
) -> ApiResult<Option<u16>> {
    let desktop = desktop.ok_or_else(|| {
        ApiError::new(
            "codex_executable_missing",
            "未找到 Codex Desktop 可执行文件，请手动启动 Codex。",
        )
    })?;
    let port = if inject_models {
        Some(allocate_local_port()?)
    } else {
        None
    };
    let arguments = if let Some(port) = port {
        format!(
            "--remote-debugging-address=127.0.0.1 --remote-debugging-port={port} --remote-allow-origins=http://127.0.0.1:{port}"
        )
    } else {
        String::new()
    };
    launch_codex_desktop(desktop, &arguments)?;
    Ok(port)
}

#[cfg(windows)]
fn launch_codex_desktop(desktop: &CodexDesktopInstallation, arguments: &str) -> ApiResult<()> {
    use windows::core::PCWSTR;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_LOCAL_SERVER,
        COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::{
        ApplicationActivationManager, IApplicationActivationManager, AO_NONE,
    };

    let app_id: Vec<u16> = desktop
        .app_user_model_id
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let arguments: Vec<u16> = arguments.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let initialized = CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok();
        let manager: IApplicationActivationManager =
            CoCreateInstance(&ApplicationActivationManager, None, CLSCTX_LOCAL_SERVER).map_err(
                |error| {
                    ApiError::detailed(
                        "codex_activation_manager_failed",
                        "初始化 Codex Desktop 启动器失败。",
                        error.to_string(),
                    )
                },
            )?;
        manager
            .ActivateApplication(PCWSTR(app_id.as_ptr()), PCWSTR(arguments.as_ptr()), AO_NONE)
            .map_err(|error| {
                ApiError::detailed(
                    "codex_restart_failed",
                    "启动 Codex Desktop 失败。",
                    error.to_string(),
                )
            })?;
        drop(manager);
        if initialized {
            CoUninitialize();
        }
    }
    Ok(())
}

#[cfg(not(windows))]
fn launch_codex_desktop(desktop: &CodexDesktopInstallation, arguments: &str) -> ApiResult<()> {
    let mut command = Command::new(&desktop.executable_path);
    if !arguments.is_empty() {
        command.args(arguments.split_whitespace());
    }
    command.spawn().map_err(|error| {
        ApiError::detailed(
            "codex_restart_failed",
            "启动 Codex Desktop 失败。",
            error.to_string(),
        )
    })?;
    Ok(())
}

async fn wait_and_inject(
    port: u16,
    profile: &StoredProviderProfile,
    codex_home: &Path,
) -> ApiResult<()> {
    let models = build_model_catalog_entries(profile, codex_home)?;
    let started = Instant::now();
    let mut last_error = None;
    while started.elapsed() < Duration::from_secs(15) {
        match inject_model_catalog(port, &models).await {
            Ok(()) => return Ok(()),
            Err(error) => last_error = Some(error),
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    Err(last_error
        .unwrap_or_else(|| ApiError::new("cdp_injection_timeout", "等待 Codex 调试端口超时。")))
}

fn allocate_local_port() -> ApiResult<u16> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(|error| {
        ApiError::detailed(
            "cdp_port_allocate_failed",
            "分配本地调试端口失败。",
            error.to_string(),
        )
    })?;
    listener
        .local_addr()
        .map(|address| address.port())
        .map_err(|error| {
            ApiError::detailed(
                "cdp_port_read_failed",
                "读取本地调试端口失败。",
                error.to_string(),
            )
        })
}

fn run_powershell(script: &str) -> Option<String> {
    let output = hidden_command("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

fn powershell_single_quote(value: &str) -> String {
    value.replace('\'', "''")
}

#[cfg(windows)]
fn hidden_command(program: &str) -> Command {
    use std::os::windows::process::CommandExt;
    let mut command = Command::new(program);
    command.creation_flags(0x08000000);
    command
}

#[cfg(not(windows))]
fn hidden_command(program: &str) -> Command {
    Command::new(program)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::{Read, Write};
    use std::net::TcpListener as TestTcpListener;
    use std::thread;

    use super::{
        api_endpoint, apply_provider_files, build_model_catalog,
        capture_official_snapshot_if_needed, create_backup, fetch_models, finalize_manifest,
        merge_official_snapshot, migrate_sessions, restore_manifest, test_provider,
    };
    use crate::switcher::models::{
        ModelEntry, OfficialSnapshot, StoredProviderProfile, SwitcherDatabase,
    };
    use crate::switcher::state::SwitcherPaths;
    use crate::switcher::store::SwitcherStore;
    use crate::utils::dpapi::{protect_to_base64, unprotect_from_base64};
    use chrono::Utc;
    use rusqlite::Connection;
    use serde_json::Value;
    use tempfile::TempDir;

    fn test_paths(temp: &TempDir) -> SwitcherPaths {
        let data_root = temp.path().join("app-data");
        SwitcherPaths {
            database_path: data_root.join("switcher.json"),
            backups_root: data_root.join("backups"),
            logs_root: data_root.join("logs"),
            log_path: data_root.join("logs").join("app.jsonl"),
            transaction_path: data_root.join("active-transaction.json"),
            data_root,
        }
    }

    fn test_profile(provider_id: &str) -> StoredProviderProfile {
        let now = Utc::now();
        StoredProviderProfile {
            id: "profile-1".into(),
            name: "Example".into(),
            provider_id: provider_id.into(),
            base_url: "https://example.com/v1".into(),
            protected_api_key: Some(protect_to_base64("sk-test-secret").unwrap()),
            default_model: "demo".into(),
            models: vec![ModelEntry {
                id: "demo".into(),
                display_name: "Demo".into(),
                context_window: Some(64_000),
            }],
            timeout_seconds: 30,
            inject_models: true,
            created_at_utc: now,
            updated_at_utc: now,
        }
    }

    #[test]
    fn endpoint_preserves_v1_path() {
        assert_eq!(
            api_endpoint("https://example.com/v1/", "models"),
            "https://example.com/v1/models"
        );
        assert_eq!(
            api_endpoint("https://example.com", "responses"),
            "https://example.com/v1/responses"
        );
    }

    #[test]
    fn catalog_contains_models() {
        let temp = TempDir::new().unwrap();
        let profile = test_profile("example");
        let catalog = build_model_catalog(&profile, temp.path()).unwrap();
        assert!(catalog.contains("\"slug\": \"demo\""));
        assert!(catalog.contains("\"context_window\": 64000"));
        assert!(catalog.contains("\"default_reasoning_level\": \"medium\""));
        assert!(catalog.contains("\"effort\": \"low\""));
        assert!(catalog.contains("\"effort\": \"xhigh\""));
    }

    #[test]
    fn catalog_inherits_native_reasoning_levels_for_matching_model() {
        let temp = TempDir::new().unwrap();
        fs::write(
            temp.path().join("models_cache.json"),
            r#"{
                "models": [{
                    "slug": "demo",
                    "display_name": "Native Demo",
                    "default_reasoning_level": "low",
                    "supported_reasoning_levels": [
                        {"effort": "low", "description": "Low"},
                        {"effort": "medium", "description": "Medium"},
                        {"effort": "high", "description": "High"},
                        {"effort": "xhigh", "description": "Extra high"},
                        {"effort": "max", "description": "Maximum"},
                        {"effort": "ultra", "description": "Ultra"}
                    ],
                    "support_verbosity": true,
                    "context_window": 256000,
                    "max_context_window": 256000
                }]
            }"#,
        )
        .unwrap();
        let mut profile = test_profile("example");
        profile.models[0].context_window = None;

        let catalog = build_model_catalog(&profile, temp.path()).unwrap();
        let value: Value = serde_json::from_str(&catalog).unwrap();
        let model = &value["models"][0];
        let levels = model["supported_reasoning_levels"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|level| level["effort"].as_str())
            .collect::<Vec<_>>();

        assert_eq!(model["display_name"], "Demo");
        assert_eq!(model["default_reasoning_level"], "low");
        assert_eq!(model["context_window"], 256000);
        assert_eq!(
            levels,
            vec!["low", "medium", "high", "xhigh", "max", "ultra"]
        );
    }

    #[test]
    fn official_snapshot_canonicalizes_existing_third_party_provider() {
        let temp = TempDir::new().unwrap();
        let codex_home = temp.path().join("codex-home");
        fs::create_dir_all(&codex_home).unwrap();
        fs::write(
            codex_home.join("config.toml"),
            r#"
model_provider = "aivr"
model = "gpt-5.6-sol"
model_catalog_json = "model-catalogs/aivr.json"

[model_providers.aivr]
name = "AIVR"

[model_providers.example]
name = "Managed"

[mcp_servers.demo]
command = "demo.exe"
"#,
        )
        .unwrap();
        fs::write(
            codex_home.join("auth.json"),
            r#"{
                "auth_mode": "chatgpt",
                "OPENAI_API_KEY": "third-party-key",
                "last_refresh": "2026-08-01T00:00:00Z",
                "tokens": {
                    "access_token": "official-access",
                    "refresh_token": "official-refresh"
                }
            }"#,
        )
        .unwrap();
        let mut database = SwitcherDatabase::default();
        database.profiles.push(test_profile("example"));

        capture_official_snapshot_if_needed(&mut database, &codex_home).unwrap();

        let config = database.official_snapshot.config_text.unwrap();
        assert!(config.contains(r#"model_provider = "openai""#));
        assert!(!config.contains("model_catalog_json"));
        assert!(config.contains("[model_providers.aivr]"));
        assert!(!config.contains("[model_providers.example]"));
        assert!(config.contains("[mcp_servers.demo]"));

        let protected = database
            .official_snapshot
            .protected_auth_text
            .as_deref()
            .unwrap();
        let auth: Value = serde_json::from_str(&unprotect_from_base64(protected).unwrap()).unwrap();
        assert_eq!(auth["auth_mode"], "chatgpt");
        assert_eq!(auth["tokens"]["access_token"], "official-access");
        assert!(auth.get("OPENAI_API_KEY").is_none());
    }

    #[test]
    fn official_restore_uses_openai_and_restores_subscription_tokens() {
        let temp = TempDir::new().unwrap();
        let codex_home = temp.path().join("codex-home");
        fs::create_dir_all(codex_home.join("model-catalogs")).unwrap();
        fs::write(
            codex_home.join("config.toml"),
            r#"
model_provider = "example"
model = "custom-model"
model_catalog_json = "model-catalogs/example.json"

[model_providers.aivr]
name = "AIVR"

[model_providers.example]
name = "Managed"

[mcp_servers.demo]
command = "demo.exe"
"#,
        )
        .unwrap();
        fs::write(
            codex_home.join("auth.json"),
            r#"{"auth_mode":"apiKey","OPENAI_API_KEY":"third-party-key"}"#,
        )
        .unwrap();
        fs::write(
            codex_home.join("model-catalogs/example.json"),
            r#"{"models":[]}"#,
        )
        .unwrap();
        let baseline_auth = r#"{
            "auth_mode": "chatgpt",
            "OPENAI_API_KEY": "stale-third-party-key",
            "last_refresh": "2026-07-30T00:00:00Z",
            "tokens": {
                "access_token": "official-access",
                "refresh_token": "official-refresh",
                "account_id": "official-account"
            }
        }"#;
        let mut database = SwitcherDatabase::default();
        database.active_profile_id = Some("profile-1".into());
        database.active_provider_id = Some("example".into());
        database.official_snapshot = OfficialSnapshot {
            config_text: Some(
                r#"
model_provider = "aivr"
model = "gpt-5.6-sol"
model_catalog_json = "model-catalogs/aivr.json"

[model_providers.aivr]
name = "AIVR"
"#
                .into(),
            ),
            protected_auth_text: Some(protect_to_base64(baseline_auth).unwrap()),
            captured_at_utc: Some(Utc::now()),
        };

        merge_official_snapshot(&database, &codex_home).unwrap();

        let config = fs::read_to_string(codex_home.join("config.toml")).unwrap();
        assert!(config.contains(r#"model_provider = "openai""#));
        assert!(config.contains(r#"model = "gpt-5.6-sol""#));
        assert!(!config.contains("model_catalog_json"));
        assert!(!config.contains("[model_providers.example]"));
        assert!(config.contains("[model_providers.aivr]"));
        assert!(config.contains("[mcp_servers.demo]"));
        assert!(!codex_home.join("model-catalogs/example.json").exists());

        let auth: Value =
            serde_json::from_str(&fs::read_to_string(codex_home.join("auth.json")).unwrap())
                .unwrap();
        assert_eq!(auth["auth_mode"], "chatgpt");
        assert_eq!(auth["tokens"]["access_token"], "official-access");
        assert_eq!(auth["tokens"]["refresh_token"], "official-refresh");
        assert!(auth.get("OPENAI_API_KEY").is_none());
    }

    #[test]
    fn provider_write_preserves_unrelated_toml_and_auth_fields() {
        let temp = TempDir::new().unwrap();
        let paths = test_paths(&temp);
        let codex_home = temp.path().join("codex-home");
        fs::create_dir_all(&codex_home).unwrap();
        fs::write(
            codex_home.join("config.toml"),
            r#"
model_provider = "old_provider"
model = "old-model"

[sandbox_workspace_write]
network_access = true

[mcp_servers.demo]
command = "demo.exe"

[model_providers.old_provider]
name = "Old"
base_url = "https://old.example/v1"
wire_api = "responses"
"#,
        )
        .unwrap();
        fs::write(
            codex_home.join("auth.json"),
            r#"{"OPENAI_API_KEY":"old-key","tokens":{"access_token":"keep-me"}}"#,
        )
        .unwrap();

        let profile = test_profile("example");
        let mut database = SwitcherDatabase::default();
        database.active_provider_id = Some("old_provider".into());
        let store = SwitcherStore::new(&paths);
        apply_provider_files(&database, &profile, &codex_home, &store).unwrap();

        let config = fs::read_to_string(codex_home.join("config.toml")).unwrap();
        assert!(config.contains("[mcp_servers.demo]"));
        assert!(config.contains("[sandbox_workspace_write]"));
        assert!(!config.contains("[model_providers.old_provider]"));
        assert!(config.contains("[model_providers.example]"));
        assert!(config.contains("model_catalog_json = \"model-catalogs/example.json\""));

        let auth: Value =
            serde_json::from_str(&fs::read_to_string(codex_home.join("auth.json")).unwrap())
                .unwrap();
        assert_eq!(auth["OPENAI_API_KEY"], "old-key");
        assert_eq!(auth["tokens"]["access_token"], "keep-me");
        assert!(config.contains("[model_providers.example.auth]"));
        assert!(config.contains("--codex-provider-token"));
        assert!(!config.contains("requires_openai_auth"));
        assert!(codex_home.join("model-catalogs/example.json").exists());
    }

    #[test]
    fn session_migration_updates_jsonl_and_sqlite_together() {
        let temp = TempDir::new().unwrap();
        let codex_home = temp.path().join("codex-home");
        let sessions = codex_home.join("sessions/2026/07/22");
        fs::create_dir_all(&sessions).unwrap();
        let session_path = sessions.join("session-1.jsonl");
        fs::write(
            &session_path,
            "{\"type\":\"session_meta\",\"payload\":{\"id\":\"thread-1\",\"model_provider\":\"openai\"}}\n{\"type\":\"event\"}\n",
        )
        .unwrap();

        let database_path = codex_home.join("state_5.sqlite");
        let connection = Connection::open(&database_path).unwrap();
        connection
            .execute(
                "CREATE TABLE threads (id TEXT PRIMARY KEY, model_provider TEXT NOT NULL)",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO threads (id, model_provider) VALUES (?1, ?2)",
                ("thread-1", "openai"),
            )
            .unwrap();
        drop(connection);

        let migrated = migrate_sessions(&codex_home, "example", 3).unwrap();
        assert_eq!(migrated, vec!["thread-1"]);
        let first_line = fs::read_to_string(&session_path)
            .unwrap()
            .lines()
            .next()
            .unwrap()
            .to_string();
        let metadata: Value = serde_json::from_str(&first_line).unwrap();
        assert_eq!(metadata["payload"]["model_provider"], "example");

        let connection = Connection::open(&database_path).unwrap();
        let provider: String = connection
            .query_row(
                "SELECT model_provider FROM threads WHERE id = ?1",
                ["thread-1"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(provider, "example");
    }

    #[test]
    fn backup_detects_conflict_and_force_restore_is_exact() {
        let temp = TempDir::new().unwrap();
        let paths = test_paths(&temp);
        let codex_home = temp.path().join("codex-home");
        fs::create_dir_all(&codex_home).unwrap();
        let config_path = codex_home.join("config.toml");
        fs::write(&config_path, "model = \"official\"\n").unwrap();

        let mut manifest = create_backup(&paths, &codex_home, "test", None, None, 0).unwrap();
        fs::write(&config_path, "model = \"custom\"\n").unwrap();
        finalize_manifest(&paths, &codex_home, &mut manifest).unwrap();
        fs::write(&config_path, "model = \"external-change\"\n").unwrap();

        let error = restore_manifest(&manifest, &paths, false).unwrap_err();
        assert_eq!(error.code, "backup_conflict");
        restore_manifest(&manifest, &paths, true).unwrap();
        assert_eq!(
            fs::read_to_string(&config_path).unwrap(),
            "model = \"official\"\n"
        );
    }

    #[tokio::test]
    async fn local_responses_service_supports_base_url_without_v1() {
        let listener = TestTcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = thread::spawn(move || {
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request_bytes = [0_u8; 8192];
                let count = stream.read(&mut request_bytes).unwrap();
                let request = String::from_utf8_lossy(&request_bytes[..count]);
                let (expected_path, body) = if request.starts_with("GET /v1/models ") {
                    (
                        "/v1/models",
                        r#"{"data":[{"id":"demo","name":"Demo","context_window":64000}]}"#,
                    )
                } else {
                    assert!(request.starts_with("POST /v1/responses "));
                    ("/v1/responses", r#"{"id":"resp_test","output":[]}"#)
                };
                assert!(request
                    .to_ascii_lowercase()
                    .contains("authorization: bearer sk-test-secret"));
                assert!(request.contains(expected_path));
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                )
                .unwrap();
            }
        });

        let temp = TempDir::new().unwrap();
        let paths = test_paths(&temp);
        let mut profile = test_profile("local_test");
        profile.base_url = format!("http://127.0.0.1:{port}");
        profile.default_model.clear();
        profile.models.clear();
        let profile_id = profile.id.clone();
        let mut database = SwitcherDatabase::default();
        database.profiles.push(profile);
        let store = SwitcherStore::new(&paths);
        store.save(&database).unwrap();

        let discovery = fetch_models(&paths, &profile_id).await.unwrap();
        assert_eq!(discovery.models[0].id, "demo");
        assert!(discovery.endpoint.ends_with("/v1/models"));

        let mut database = store.load().unwrap();
        let profile = database
            .profiles
            .iter_mut()
            .find(|profile| profile.id == profile_id)
            .unwrap();
        profile.default_model = discovery.models[0].id.clone();
        profile.models = discovery.models;
        store.save(&database).unwrap();

        let connection = test_provider(&paths, &profile_id).await.unwrap();
        assert_eq!(connection.status, "success");
        assert!(connection.endpoint.ends_with("/v1/responses"));
        server.join().unwrap();
    }
}
