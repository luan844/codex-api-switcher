use std::fs;
use std::path::{Path, PathBuf};

use chrono::{Local, NaiveDateTime, TimeZone};

use crate::errors::app_error::AppResult;
use crate::models::app::BackupOverview;
use crate::models::backup::BackupItem;
use crate::models::profile::ProfileDatabase;
use crate::services::target_service::{TargetContext, TargetService};
use crate::state::app_state::AppPaths;
use crate::utils::fs_utils::ensure_directory;

pub struct BackupService<'a> {
    paths: &'a AppPaths,
}

impl<'a> BackupService<'a> {
    pub fn new(paths: &'a AppPaths) -> Self {
        Self { paths }
    }

    pub fn create_backup_if_needed(&self, target: &TargetContext) -> AppResult<Option<PathBuf>> {
        let auth_path = target.codex_directory_path.join("auth.json");
        let config_path = target.codex_directory_path.join("config.toml");

        if !auth_path.exists() && !config_path.exists() {
            return Ok(None);
        }

        ensure_directory(&target.backups_directory_path)?;
        let stamp = Local::now().format("%Y%m%d-%H%M%S-%3f").to_string();
        let backup_dir = target.backups_directory_path.join(stamp);
        ensure_directory(&backup_dir)?;

        if auth_path.exists() {
            fs::copy(auth_path, backup_dir.join("auth.json"))?;
        }
        if config_path.exists() {
            fs::copy(config_path, backup_dir.join("config.toml"))?;
        }

        Ok(Some(backup_dir))
    }

    pub fn restore_latest_for_selected_targets(
        &self,
        database: &ProfileDatabase,
    ) -> AppResult<Vec<String>> {
        let target_service = TargetService::new(self.paths);
        let targets = target_service.resolve_selected_targets(database)?;
        let mut restored = Vec::new();

        for target in targets {
            if let Some(backup_dir) =
                self.latest_backup_directory(&target.backups_directory_path)?
            {
                let restored_any = self.restore_snapshot(&target, Some(backup_dir.as_path()))?;
                if restored_any {
                    restored.push(target.display_name);
                }
            }
        }

        Ok(restored)
    }

    pub fn restore_snapshot(
        &self,
        target: &TargetContext,
        backup_dir: Option<&Path>,
    ) -> AppResult<bool> {
        ensure_directory(&target.codex_directory_path)?;

        let auth_changed = restore_backup_file(
            backup_dir.map(|item| item.join("auth.json")),
            &target.codex_directory_path.join("auth.json"),
        )?;
        let config_changed = restore_backup_file(
            backup_dir.map(|item| item.join("config.toml")),
            &target.codex_directory_path.join("config.toml"),
        )?;

        Ok(auth_changed || config_changed)
    }

    pub fn list_all_backups(&self) -> AppResult<Vec<BackupItem>> {
        let mut items = Vec::new();
        let windows_root = self.paths.backups_root.join("windows");
        items.extend(self.collect_backup_items("windows", "Windows", &windows_root)?);

        let wsl_root = self.paths.backups_root.join("wsl");
        if wsl_root.exists() {
            for entry in fs::read_dir(wsl_root)? {
                let entry = entry?;
                if !entry.file_type()?.is_dir() {
                    continue;
                }

                let label = entry.file_name().to_string_lossy().to_string();
                items.extend(self.collect_backup_items(
                    &format!("wsl:{label}"),
                    &format!("WSL ({label})"),
                    &entry.path(),
                )?);
            }
        }

        items.sort_by(|left, right| right.directory_name.cmp(&left.directory_name));
        Ok(items)
    }

    pub fn build_overview(&self, database: &ProfileDatabase) -> AppResult<Vec<BackupOverview>> {
        let target_service = TargetService::new(self.paths);
        let targets = target_service.resolve_selected_targets(database)?;
        let mut overview = Vec::new();

        for target in targets {
            let has_recent_backup = self
                .latest_backup_directory(&target.backups_directory_path)?
                .is_some();
            overview.push(BackupOverview {
                target_key: target.target_key,
                display_name: target.display_name,
                has_recent_backup,
                count: self.count_directories(&target.backups_directory_path)?,
            });
        }

        Ok(overview)
    }

    fn collect_backup_items(
        &self,
        target_key: &str,
        display_name: &str,
        root: &Path,
    ) -> AppResult<Vec<BackupItem>> {
        if !root.exists() {
            return Ok(Vec::new());
        }

        let mut items = Vec::new();
        for entry in fs::read_dir(root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }

            let name = entry.file_name().to_string_lossy().to_string();
            items.push(BackupItem {
                target_key: target_key.to_string(),
                display_name: display_name.to_string(),
                directory_name: name.clone(),
                created_at_label: format_backup_label(&name),
                has_auth_json: entry.path().join("auth.json").exists(),
                has_config_toml: entry.path().join("config.toml").exists(),
            });
        }

        Ok(items)
    }

    fn latest_backup_directory(&self, root: &Path) -> AppResult<Option<PathBuf>> {
        if !root.exists() {
            return Ok(None);
        }

        let mut directories = Vec::new();
        for entry in fs::read_dir(root)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                directories.push(entry.path());
            }
        }

        directories.sort_by(|left, right| {
            right
                .file_name()
                .and_then(|item| item.to_str())
                .cmp(&left.file_name().and_then(|item| item.to_str()))
        });

        Ok(directories.into_iter().next())
    }

    fn count_directories(&self, root: &Path) -> AppResult<usize> {
        if !root.exists() {
            return Ok(0);
        }

        let mut count = 0;
        for entry in fs::read_dir(root)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                count += 1;
            }
        }
        Ok(count)
    }
}

fn format_backup_label(name: &str) -> String {
    if let Ok(parsed) = NaiveDateTime::parse_from_str(name, "%Y%m%d-%H%M%S-%3f") {
        if let Some(local) = Local.from_local_datetime(&parsed).single() {
            return local.format("%Y-%m-%d %H:%M:%S").to_string();
        }
    }

    name.to_string()
}

fn restore_backup_file(backup_path: Option<PathBuf>, target_path: &Path) -> AppResult<bool> {
    if let Some(source_path) = backup_path.filter(|item| item.exists()) {
        fs::copy(source_path, target_path)?;
        return Ok(true);
    }

    if target_path.exists() {
        fs::remove_file(target_path)?;
        return Ok(true);
    }

    Ok(false)
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;
    use crate::state::app_state::AppPaths;

    fn build_test_paths(root: &Path) -> AppPaths {
        let data_root = root.join("data");
        let profiles_root = data_root.join("profiles");
        let templates_root = data_root.join("templates");
        let logs_root = data_root.join("logs");

        AppPaths {
            database_path: data_root.join("profiles.json"),
            openai_template_path: templates_root.join("config_openai.toml"),
            apikey_template_path: templates_root.join("config_apikey.toml"),
            backups_root: root.join("backups"),
            structured_log_path: logs_root.join("app.jsonl"),
            data_root,
            profiles_root,
            templates_root,
            logs_root,
        }
    }

    #[test]
    fn create_backup_if_needed_copies_auth_and_config_files() {
        let workspace = tempdir().unwrap();
        let paths = build_test_paths(workspace.path());
        let codex_dir = workspace.path().join(".codex");
        fs::create_dir_all(&codex_dir).unwrap();
        fs::write(codex_dir.join("auth.json"), "{\"token\":\"demo\"}").unwrap();
        fs::write(codex_dir.join("config.toml"), "model = \"gpt-5.4-mini\"").unwrap();

        let target = TargetContext {
            target_key: "windows".into(),
            display_name: "Windows".into(),
            codex_directory_path: codex_dir,
            backups_directory_path: workspace.path().join("backups").join("windows"),
            codex_linux_path: None,
            wsl_environment: None,
        };

        let service = BackupService::new(&paths);
        let backup_dir = service.create_backup_if_needed(&target).unwrap().unwrap();

        assert!(backup_dir.join("auth.json").exists());
        assert!(backup_dir.join("config.toml").exists());
    }

    #[test]
    fn format_backup_label_formats_timestamp_directory_name() {
        let label = format_backup_label("20260327-120102-123");

        assert!(label.starts_with("2026-03-27 12:01:02"));
    }

    #[test]
    fn restore_snapshot_removes_files_absent_in_backup() {
        let workspace = tempdir().unwrap();
        let paths = build_test_paths(workspace.path());
        let codex_dir = workspace.path().join(".codex");
        let backup_dir = workspace
            .path()
            .join("backups")
            .join("windows")
            .join("latest");
        fs::create_dir_all(&codex_dir).unwrap();
        fs::create_dir_all(&backup_dir).unwrap();
        fs::write(codex_dir.join("auth.json"), "{\"token\":\"new\"}").unwrap();
        fs::write(codex_dir.join("config.toml"), "model = \"new\"").unwrap();
        fs::write(backup_dir.join("config.toml"), "model = \"old\"").unwrap();

        let target = TargetContext {
            target_key: "windows".into(),
            display_name: "Windows".into(),
            codex_directory_path: codex_dir.clone(),
            backups_directory_path: workspace.path().join("backups").join("windows"),
            codex_linux_path: None,
            wsl_environment: None,
        };

        let service = BackupService::new(&paths);
        let changed = service
            .restore_snapshot(&target, Some(backup_dir.as_path()))
            .unwrap();

        assert!(changed);
        assert!(!codex_dir.join("auth.json").exists());
        assert_eq!(
            fs::read_to_string(codex_dir.join("config.toml")).unwrap(),
            "model = \"old\""
        );
    }
}
