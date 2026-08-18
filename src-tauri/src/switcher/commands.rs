use serde_json::Value;
use tauri::{AppHandle, Emitter, State};

use crate::switcher::models::{
    ConnectionTestResult, ExecuteSwitchInput, ModelDiscoveryResult, RestoreBackupInput,
    SaveProviderInput, SaveSettingsInput, SwitchPreview, SwitchResult, SwitcherBootstrap,
};
use crate::switcher::service::{
    build_bootstrap, execute_switch, fetch_models, get_runtime_status, list_backups,
    open_directory, preview_switch, read_logs, restore_backup, restore_official, test_provider,
};
use crate::switcher::state::SwitcherState;
use crate::switcher::store::SwitcherStore;
use crate::switcher::{ApiError, ApiResult};

#[tauri::command]
pub fn load_switcher_bootstrap(
    app: AppHandle,
    state: State<'_, SwitcherState>,
) -> ApiResult<SwitcherBootstrap> {
    let _guard = state
        .database_lock
        .lock()
        .map_err(|_| ApiError::new("database_lock_poisoned", "应用数据锁已损坏。"))?;
    build_bootstrap(&state.paths, &app.package_info().version.to_string())
}

#[tauri::command]
pub fn save_switcher_provider(
    app: AppHandle,
    state: State<'_, SwitcherState>,
    payload: SaveProviderInput,
) -> ApiResult<SwitcherBootstrap> {
    let _guard = state
        .database_lock
        .lock()
        .map_err(|_| ApiError::new("database_lock_poisoned", "应用数据锁已损坏。"))?;
    SwitcherStore::new(&state.paths).save_provider(payload)?;
    build_bootstrap(&state.paths, &app.package_info().version.to_string())
}

#[tauri::command]
pub fn duplicate_switcher_provider(
    app: AppHandle,
    state: State<'_, SwitcherState>,
    profile_id: String,
) -> ApiResult<SwitcherBootstrap> {
    let _guard = state
        .database_lock
        .lock()
        .map_err(|_| ApiError::new("database_lock_poisoned", "应用数据锁已损坏。"))?;
    SwitcherStore::new(&state.paths).duplicate_provider(&profile_id)?;
    build_bootstrap(&state.paths, &app.package_info().version.to_string())
}

#[tauri::command]
pub fn delete_switcher_provider(
    app: AppHandle,
    state: State<'_, SwitcherState>,
    profile_id: String,
) -> ApiResult<SwitcherBootstrap> {
    let _guard = state
        .database_lock
        .lock()
        .map_err(|_| ApiError::new("database_lock_poisoned", "应用数据锁已损坏。"))?;
    SwitcherStore::new(&state.paths).delete_provider(&profile_id)?;
    build_bootstrap(&state.paths, &app.package_info().version.to_string())
}

#[tauri::command]
pub fn save_switcher_settings(
    app: AppHandle,
    state: State<'_, SwitcherState>,
    payload: SaveSettingsInput,
) -> ApiResult<SwitcherBootstrap> {
    let _guard = state
        .database_lock
        .lock()
        .map_err(|_| ApiError::new("database_lock_poisoned", "应用数据锁已损坏。"))?;
    SwitcherStore::new(&state.paths).save_settings(payload)?;
    build_bootstrap(&state.paths, &app.package_info().version.to_string())
}

#[tauri::command]
pub fn preview_switcher(
    state: State<'_, SwitcherState>,
    profile_id: String,
) -> ApiResult<SwitchPreview> {
    preview_switch(&state.paths, &profile_id)
}

#[tauri::command]
pub async fn discover_switcher_models(
    state: State<'_, SwitcherState>,
    profile_id: String,
) -> ApiResult<ModelDiscoveryResult> {
    fetch_models(&state.paths, &profile_id).await
}

#[tauri::command]
pub async fn test_switcher_provider(
    state: State<'_, SwitcherState>,
    profile_id: String,
) -> ApiResult<ConnectionTestResult> {
    test_provider(&state.paths, &profile_id).await
}

#[tauri::command]
pub async fn execute_switcher(
    app: AppHandle,
    state: State<'_, SwitcherState>,
    payload: ExecuteSwitchInput,
) -> ApiResult<SwitchResult> {
    let _guard = state.switch_lock.lock().await;
    let _ = app.emit(
        "switcher://transaction",
        serde_json::json!({ "phase": "starting", "status": "running" }),
    );
    let result = execute_switch(
        &state.paths,
        payload,
        &app.package_info().version.to_string(),
    )
    .await;
    let _ = app.emit(
        "switcher://transaction",
        serde_json::json!({
            "phase": "complete",
            "status": if result.is_ok() { "success" } else { "failure" }
        }),
    );
    result
}

#[tauri::command]
pub async fn restore_switcher_official(
    app: AppHandle,
    state: State<'_, SwitcherState>,
    force_close: bool,
    restart_codex: bool,
) -> ApiResult<SwitchResult> {
    let _guard = state.switch_lock.lock().await;
    restore_official(
        &state.paths,
        force_close,
        restart_codex,
        &app.package_info().version.to_string(),
    )
    .await
}

#[tauri::command]
pub fn list_switcher_backups(
    state: State<'_, SwitcherState>,
) -> ApiResult<Vec<crate::switcher::models::BackupSummary>> {
    list_backups(&state.paths)
}

#[tauri::command]
pub fn restore_switcher_backup(
    app: AppHandle,
    state: State<'_, SwitcherState>,
    payload: RestoreBackupInput,
) -> ApiResult<SwitcherBootstrap> {
    let _switch_guard = state
        .switch_lock
        .try_lock()
        .map_err(|_| ApiError::new("switch_in_progress", "另一个切换或恢复操作正在进行。"))?;
    restore_backup(
        &state.paths,
        &payload.backup_id,
        payload.force,
        &app.package_info().version.to_string(),
    )
}

#[tauri::command]
pub fn get_switcher_runtime(
    state: State<'_, SwitcherState>,
) -> ApiResult<crate::switcher::models::CodexRuntimeStatus> {
    let database = SwitcherStore::new(&state.paths).load()?;
    get_runtime_status(&database)
}

#[tauri::command]
pub fn read_switcher_logs(
    state: State<'_, SwitcherState>,
    limit: Option<usize>,
) -> ApiResult<Vec<Value>> {
    read_logs(&state.paths, limit.unwrap_or(200))
}

#[tauri::command]
pub fn open_switcher_data_directory(state: State<'_, SwitcherState>) -> ApiResult<()> {
    open_directory(&state.paths.data_root)
}

#[tauri::command]
pub fn open_switcher_backup_directory(state: State<'_, SwitcherState>) -> ApiResult<()> {
    open_directory(&state.paths.backups_root)
}

#[tauri::command]
pub fn open_switcher_log_directory(state: State<'_, SwitcherState>) -> ApiResult<()> {
    open_directory(&state.paths.logs_root)
}
