use std::path::PathBuf;
use std::sync::Mutex;

use crate::errors::app_error::{AppError, AppResult};

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub data_root: PathBuf,
    pub profiles_root: PathBuf,
    pub database_path: PathBuf,
    pub templates_root: PathBuf,
    pub openai_template_path: PathBuf,
    pub apikey_template_path: PathBuf,
    pub backups_root: PathBuf,
    pub logs_root: PathBuf,
    pub structured_log_path: PathBuf,
}

impl AppPaths {
    pub fn from_system() -> AppResult<Self> {
        let local_data = dirs::data_local_dir().ok_or_else(|| {
            AppError::validation("local_app_data_missing", "无法识别 LOCALAPPDATA 目录。")
        })?;
        let user_home = dirs::home_dir()
            .ok_or_else(|| AppError::validation("home_dir_missing", "无法识别用户目录。"))?;

        let data_root = local_data.join("codex-switch");
        let profiles_root = data_root.join("profiles");
        let templates_root = data_root.join("templates");
        let logs_root = data_root.join("logs");

        Ok(Self {
            database_path: data_root.join("profiles.json"),
            openai_template_path: templates_root.join("config_openai.toml"),
            apikey_template_path: templates_root.join("config_apikey.toml"),
            backups_root: user_home.join("codex-switch-backups"),
            structured_log_path: logs_root.join("app.jsonl"),
            data_root,
            profiles_root,
            templates_root,
            logs_root,
        })
    }
}

#[derive(Debug)]
pub struct AppState {
    pub paths: AppPaths,
    pub database_lock: Mutex<()>,
    pub switch_lock: Mutex<()>,
}

impl AppState {
    pub fn new() -> AppResult<Self> {
        Ok(Self {
            paths: AppPaths::from_system()?,
            database_lock: Mutex::new(()),
            switch_lock: Mutex::new(()),
        })
    }
}
