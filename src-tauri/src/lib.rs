pub mod errors;
pub mod models;
pub mod services;
pub mod state;
pub mod switcher;
pub mod utils;

use switcher::commands::{
    delete_switcher_provider, discover_switcher_models, duplicate_switcher_provider,
    execute_switcher, get_switcher_runtime, list_switcher_backups, load_switcher_bootstrap,
    open_switcher_backup_directory, open_switcher_data_directory, open_switcher_log_directory,
    preview_switcher, read_switcher_logs, restore_switcher_backup, restore_switcher_official,
    save_switcher_provider, save_switcher_settings, test_switcher_provider,
};
use switcher::state::SwitcherState;
use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::default().build())
        .manage(SwitcherState::new().expect("初始化 Codex API Switcher 状态失败"))
        .setup(|app| {
            if let (Some(window), Some(icon)) = (
                app.get_webview_window("main"),
                app.default_window_icon().cloned(),
            ) {
                window.set_icon(icon)?;
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            load_switcher_bootstrap,
            save_switcher_provider,
            duplicate_switcher_provider,
            delete_switcher_provider,
            save_switcher_settings,
            preview_switcher,
            discover_switcher_models,
            test_switcher_provider,
            execute_switcher,
            restore_switcher_official,
            list_switcher_backups,
            restore_switcher_backup,
            get_switcher_runtime,
            read_switcher_logs,
            open_switcher_data_directory,
            open_switcher_backup_directory,
            open_switcher_log_directory
        ])
        .run(tauri::generate_context!())
        .expect("运行 Codex API Switcher 失败");
}
