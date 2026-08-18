use std::path::PathBuf;
use std::sync::Mutex;

use crate::switcher::{ApiError, ApiResult};

#[derive(Debug, Clone)]
pub struct SwitcherPaths {
    pub data_root: PathBuf,
    pub database_path: PathBuf,
    pub backups_root: PathBuf,
    pub logs_root: PathBuf,
    pub log_path: PathBuf,
    pub transaction_path: PathBuf,
}

impl SwitcherPaths {
    pub fn from_system() -> ApiResult<Self> {
        let local_data = dirs::data_local_dir().ok_or_else(|| {
            ApiError::new("local_app_data_missing", "无法识别 LOCALAPPDATA 目录。")
        })?;
        let data_root = local_data.join("CodexApiSwitcher");
        Ok(Self {
            database_path: data_root.join("switcher.json"),
            backups_root: data_root.join("backups"),
            logs_root: data_root.join("logs"),
            log_path: data_root.join("logs").join("app.jsonl"),
            transaction_path: data_root.join("active-transaction.json"),
            data_root,
        })
    }
}

#[derive(Debug)]
pub struct SwitcherState {
    pub paths: SwitcherPaths,
    pub database_lock: Mutex<()>,
    pub switch_lock: tokio::sync::Mutex<()>,
}

impl SwitcherState {
    pub fn new() -> ApiResult<Self> {
        Ok(Self {
            paths: SwitcherPaths::from_system()?,
            database_lock: Mutex::new(()),
            switch_lock: tokio::sync::Mutex::new(()),
        })
    }
}
