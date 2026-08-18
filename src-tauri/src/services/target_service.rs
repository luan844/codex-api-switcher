use std::path::PathBuf;

use crate::errors::app_error::{AppError, AppResult};
use crate::models::app::WslEnvironmentInfo;
use crate::models::profile::ProfileDatabase;
use crate::services::wsl_service::WslService;
use crate::state::app_state::AppPaths;
use crate::utils::path_utils::{combine_linux_path, sanitize_directory_segment};

#[derive(Debug, Clone)]
pub struct TargetContext {
    pub target_key: String,
    pub display_name: String,
    pub codex_directory_path: PathBuf,
    pub backups_directory_path: PathBuf,
    pub codex_linux_path: Option<String>,
    pub wsl_environment: Option<WslEnvironmentInfo>,
}

pub struct TargetService<'a> {
    paths: &'a AppPaths,
    wsl_service: WslService,
}

impl<'a> TargetService<'a> {
    pub fn new(paths: &'a AppPaths) -> Self {
        Self {
            paths,
            wsl_service: WslService::new(),
        }
    }

    pub fn resolve_selected_targets(
        &self,
        database: &ProfileDatabase,
    ) -> AppResult<Vec<TargetContext>> {
        let mut targets = Vec::with_capacity(2);

        if database.replace_windows_target {
            targets.push(TargetContext {
                target_key: "windows".into(),
                display_name: "Windows".into(),
                codex_directory_path: dirs::home_dir()
                    .ok_or_else(|| AppError::validation("home_dir_missing", "无法识别用户目录。"))?
                    .join(".codex"),
                backups_directory_path: self.paths.backups_root.join("windows"),
                codex_linux_path: None,
                wsl_environment: None,
            });
        }

        if database.replace_wsl_target {
            let info = self.wsl_service.resolve_environment(database)?;
            let codex_linux_path = combine_linux_path(&info.home_directory, ".codex");
            let windows_path = PathBuf::from(
                self.wsl_service
                    .to_windows_path(&info.distro_name, &codex_linux_path),
            );
            let backup_leaf = format!(
                "{}-{}",
                sanitize_directory_segment(&info.distro_name),
                sanitize_directory_segment(&info.user_name)
            );

            targets.push(TargetContext {
                target_key: format!("wsl:{}:{}", info.distro_name, info.user_name),
                display_name: format!("WSL ({}/{})", info.distro_name, info.user_name),
                codex_directory_path: windows_path,
                backups_directory_path: self.paths.backups_root.join("wsl").join(backup_leaf),
                codex_linux_path: Some(codex_linux_path),
                wsl_environment: Some(info),
            });
        }

        if targets.is_empty() {
            return Err(AppError::validation(
                "switch_target_missing",
                "请至少选择一个替换目标。",
            ));
        }

        Ok(targets)
    }

    pub fn wsl_status_text(&self, database: &ProfileDatabase) -> String {
        self.wsl_service.build_status_text(database, false)
    }

    pub fn wsl_service(&self) -> &WslService {
        &self.wsl_service
    }
}
