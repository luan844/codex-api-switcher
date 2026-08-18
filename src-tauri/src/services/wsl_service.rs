use std::process::{Command, Stdio};

use crate::errors::app_error::{AppError, AppResult};
use crate::models::app::WslEnvironmentInfo;
use crate::models::profile::ProfileDatabase;
use crate::utils::path_utils::{normalize_linux_path, normalize_optional_value};

pub struct WslService;

impl WslService {
    pub fn new() -> Self {
        Self
    }

    pub fn refresh_default_environment(&self) -> AppResult<WslEnvironmentInfo> {
        let distro_name = self.run_default_shell("printf '%s' \"$WSL_DISTRO_NAME\"")?;
        let user_name = self.run_default_shell("printf '%s' \"$USER\"")?;
        let home_directory = self.run_default_shell("printf '%s' \"$HOME\"")?;

        if distro_name.is_empty() {
            return Err(AppError::validation(
                "wsl_distro_missing",
                "未能识别默认 WSL 发行版。",
            ));
        }
        if user_name.is_empty() {
            return Err(AppError::validation(
                "wsl_user_missing",
                "未能识别默认 WSL 用户。",
            ));
        }
        if home_directory.is_empty() {
            return Err(AppError::validation(
                "wsl_home_missing",
                "未能识别 WSL 家目录。",
            ));
        }

        Ok(WslEnvironmentInfo {
            distro_name,
            user_name,
            home_directory: normalize_linux_path(&home_directory),
        })
    }

    pub fn cached_default_environment(
        &self,
        database: &ProfileDatabase,
    ) -> Option<WslEnvironmentInfo> {
        Some(WslEnvironmentInfo {
            distro_name: normalize_optional_value(
                database.cached_default_wsl_distro_name.as_deref(),
            )?,
            user_name: normalize_optional_value(database.cached_default_wsl_user_name.as_deref())?,
            home_directory: normalize_optional_value(
                database.cached_default_wsl_home_directory.as_deref(),
            )?,
        })
    }

    pub fn resolve_environment(&self, database: &ProfileDatabase) -> AppResult<WslEnvironmentInfo> {
        let configured_distro = normalize_optional_value(database.wsl_distro_name.as_deref());
        let configured_user = normalize_optional_value(database.wsl_user_name.as_deref());
        let cached_default = self.cached_default_environment(database);

        if let (Some(distro_name), Some(user_name)) =
            (configured_distro.clone(), configured_user.clone())
        {
            return Ok(WslEnvironmentInfo {
                distro_name,
                user_name: user_name.clone(),
                home_directory: format!("/home/{user_name}"),
            });
        }

        if configured_distro.is_none() && configured_user.is_none() {
            if let Some(info) = cached_default {
                return Ok(info);
            }
        }

        if let Some(cached) = self.cached_default_environment(database) {
            if configured_distro.is_some() || configured_user.is_some() {
                let distro_name = configured_distro.unwrap_or(cached.distro_name);
                let user_name = configured_user.unwrap_or(cached.user_name.clone());
                let home_directory = if database.wsl_user_name.as_deref().is_some() {
                    format!("/home/{user_name}")
                } else {
                    cached.home_directory
                };
                return Ok(WslEnvironmentInfo {
                    distro_name,
                    user_name,
                    home_directory,
                });
            }
        }

        let default_environment = self.refresh_default_environment()?;
        let distro_name = configured_distro.unwrap_or(default_environment.distro_name);
        let user_name = configured_user.unwrap_or(default_environment.user_name.clone());
        let home_directory = if database.wsl_user_name.as_deref().is_some() {
            format!("/home/{user_name}")
        } else {
            default_environment.home_directory
        };

        Ok(WslEnvironmentInfo {
            distro_name,
            user_name,
            home_directory: normalize_linux_path(&home_directory),
        })
    }

    pub fn to_windows_path(&self, distro_name: &str, linux_path: &str) -> String {
        let relative = normalize_linux_path(linux_path)
            .trim_start_matches('/')
            .replace('/', "\\");
        format!(r"\\wsl$\{}\{}", distro_name, relative)
    }

    pub fn run_command(
        &self,
        environment: &WslEnvironmentInfo,
        executable: &str,
        arguments: &[&str],
    ) -> AppResult<String> {
        self.run_wsl_command(Some(environment), executable, arguments, false)
    }

    pub fn build_status_text(&self, database: &ProfileDatabase, refreshing: bool) -> String {
        if !database.replace_wsl_target {
            return "WSL 未启用。启用后会自动使用默认发行版和默认用户。".into();
        }

        match self.resolve_environment(database) {
            Ok(info) => format!("WSL 目标：{} / {}", info.distro_name, info.user_name),
            Err(error) => {
                if refreshing {
                    "WSL 默认值尚未缓存，正在后台读取。".into()
                } else {
                    error.to_dto().message
                }
            }
        }
    }

    fn run_default_shell(&self, command: &str) -> AppResult<String> {
        self.run_wsl_command(None, "sh", &["-lc", command], true)
    }

    fn run_wsl_command(
        &self,
        environment: Option<&WslEnvironmentInfo>,
        executable: &str,
        arguments: &[&str],
        require_output: bool,
    ) -> AppResult<String> {
        let mut command = Command::new("wsl.exe");
        command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null());

        if let Some(environment) = environment {
            command.arg("-d").arg(&environment.distro_name);
            command.arg("-u").arg(&environment.user_name);
        }

        command.arg("-e").arg(executable);
        for argument in arguments {
            command.arg(argument);
        }

        let output = command.output().map_err(|source| AppError::Io {
            code: "wsl_spawn_failed".into(),
            message: "未找到 wsl.exe，请先安装并初始化 WSL。".into(),
            source,
        })?;

        let stdout = cleanup_process_text(&String::from_utf8_lossy(&output.stdout));
        let stderr = cleanup_process_text(&String::from_utf8_lossy(&output.stderr));

        if !output.status.success() {
            return Err(AppError::validation(
                "wsl_command_failed",
                if stderr.is_empty() {
                    "wsl.exe 执行失败。".into()
                } else {
                    format!("wsl.exe 执行失败：{stderr}")
                },
            ));
        }

        if require_output && stdout.is_empty() {
            return Err(AppError::validation(
                "wsl_output_missing",
                if stderr.is_empty() {
                    "wsl.exe 没有返回可用结果。".into()
                } else {
                    format!("wsl.exe 没有返回可用结果：{stderr}")
                },
            ));
        }

        Ok(stdout)
    }
}

fn cleanup_process_text(value: &str) -> String {
    value
        .replace('\0', "")
        .replace('\u{FEFF}', "")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_environment_uses_cached_default_when_no_override() {
        let mut database = ProfileDatabase::default();
        database.replace_wsl_target = true;
        database.cached_default_wsl_distro_name = Some("Ubuntu-24.04".into());
        database.cached_default_wsl_user_name = Some("workspace".into());
        database.cached_default_wsl_home_directory = Some("/home/workspace".into());

        let service = WslService::new();
        let environment = service.resolve_environment(&database).unwrap();

        assert_eq!(environment.distro_name, "Ubuntu-24.04");
        assert_eq!(environment.user_name, "workspace");
        assert_eq!(environment.home_directory, "/home/workspace");
    }

    #[test]
    fn resolve_environment_merges_override_and_cached_values() {
        let mut database = ProfileDatabase::default();
        database.replace_wsl_target = true;
        database.wsl_user_name = Some("analyst".into());
        database.cached_default_wsl_distro_name = Some("Ubuntu-24.04".into());
        database.cached_default_wsl_user_name = Some("workspace".into());
        database.cached_default_wsl_home_directory = Some("/home/workspace".into());

        let service = WslService::new();
        let environment = service.resolve_environment(&database).unwrap();

        assert_eq!(environment.distro_name, "Ubuntu-24.04");
        assert_eq!(environment.user_name, "analyst");
        assert_eq!(environment.home_directory, "/home/analyst");
    }

    #[test]
    fn to_windows_path_converts_linux_home_path() {
        let service = WslService::new();
        let path = service.to_windows_path("Ubuntu-24.04", "/home/workspace/.codex");

        assert_eq!(path, r"\\wsl$\Ubuntu-24.04\home\workspace\.codex");
    }
}
