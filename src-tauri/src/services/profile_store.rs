use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::Utc;
use uuid::Uuid;

use crate::errors::app_error::{AppError, AppResult};
use crate::models::profile::{
    CodexAuthMode, CodexProfile, ProfileDatabase, ProviderCategory, SaveProfileInput,
    CURRENT_MIGRATION_VERSION, DEFAULT_APIKEY_PROVIDER_NAME,
};
use crate::state::app_state::AppPaths;
use crate::utils::dpapi::protect_to_base64;
use crate::utils::fs_utils::{copy_directory, ensure_directory};
use crate::utils::path_utils::normalize_optional_value;

pub struct ProfileStore<'a> {
    paths: &'a AppPaths,
}

impl<'a> ProfileStore<'a> {
    pub fn new(paths: &'a AppPaths) -> Self {
        Self { paths }
    }

    pub fn load(&self) -> AppResult<ProfileDatabase> {
        ensure_directory(&self.paths.data_root)?;
        ensure_directory(&self.paths.profiles_root)?;

        if !self.paths.database_path.exists() {
            return Ok(ProfileDatabase::default());
        }

        let content = fs::read_to_string(&self.paths.database_path)?;
        let mut database = serde_json::from_str::<ProfileDatabase>(&content)?;
        let backed_up = self.backup_legacy_data_if_needed(&database)?;
        let changed = self.normalize_database(&mut database);
        if backed_up || changed {
            self.save(&database)?;
        }
        Ok(database)
    }

    pub fn save(&self, database: &ProfileDatabase) -> AppResult<()> {
        ensure_directory(&self.paths.data_root)?;
        ensure_directory(&self.paths.profiles_root)?;
        let content = serde_json::to_string_pretty(database)?;
        write_json_atomically(&self.paths.database_path, &content)?;
        Ok(())
    }

    pub fn save_profile(&self, input: &SaveProfileInput) -> AppResult<ProfileDatabase> {
        let mut database = self.load()?;
        if let Some(profile_id) = input.id {
            self.update_profile(&mut database, profile_id, input)?;
        } else {
            self.create_profile(&mut database, input)?;
        }
        self.save(&database)?;
        Ok(database)
    }

    pub fn delete_profile(&self, profile_id: Uuid) -> AppResult<ProfileDatabase> {
        let mut database = self.load()?;
        let original_profiles = database.profiles.clone();
        database.profiles.retain(|profile| profile.id != profile_id);
        if database.last_selected_profile_id == Some(profile_id) {
            database.last_selected_profile_id = None;
        }

        let profile_dir = self.profile_directory_path(profile_id);
        if profile_dir.exists() {
            fs::remove_dir_all(&profile_dir)?;
        }

        if original_profiles.len() != database.profiles.len() {
            self.save(&database)?;
        }

        Ok(database)
    }

    pub fn duplicate_profile(&self, profile_id: Uuid) -> AppResult<ProfileDatabase> {
        let mut database = self.load()?;
        let source = database
            .profiles
            .iter()
            .find(|profile| profile.id == profile_id)
            .cloned()
            .ok_or_else(|| AppError::validation("profile_not_found", "未找到要复制的组合。"))?;

        let new_id = Uuid::new_v4();
        let new_dir = self.profile_directory_path(new_id);
        ensure_directory(&new_dir)?;

        let stored_auth_json_path = source
            .stored_auth_json_path
            .as_ref()
            .map(|path| self.copy_existing_profile_asset(path, &new_dir, "auth.json"))
            .transpose()?
            .flatten();
        let stored_config_toml_path = source
            .stored_config_toml_path
            .as_ref()
            .map(|path| self.copy_existing_profile_asset(path, &new_dir, "config.toml"))
            .transpose()?
            .flatten();
        let now = Utc::now();

        database.profiles.push(CodexProfile {
            id: new_id,
            name: format!("{} 副本", source.name),
            base_url: source.base_url,
            test_model: source.test_model,
            auto_disabled: false,
            auto_disabled_reason: None,
            auto_disabled_at_utc: None,
            provider_category: source.provider_category,
            auth_mode: source.auth_mode,
            stored_auth_json_path,
            protected_api_key_base64: source.protected_api_key_base64,
            stored_config_toml_path,
            created_at_utc: now,
            updated_at_utc: now,
        });

        self.save(&database)?;
        Ok(database)
    }

    fn create_profile(
        &self,
        database: &mut ProfileDatabase,
        input: &SaveProfileInput,
    ) -> AppResult<()> {
        validate_profile_name(&input.name)?;
        let id = Uuid::new_v4();
        let profile_dir = self.profile_directory_path(id);
        ensure_directory(&profile_dir)?;
        let now = Utc::now();

        let mut profile = CodexProfile {
            id,
            name: input.name.trim().to_string(),
            base_url: normalize_optional_value(input.base_url.as_deref()).unwrap_or_default(),
            test_model: normalize_optional_value(input.test_model.as_deref()),
            auto_disabled: false,
            auto_disabled_reason: None,
            auto_disabled_at_utc: None,
            provider_category: input.provider_category,
            auth_mode: input.auth_mode,
            stored_auth_json_path: None,
            protected_api_key_base64: None,
            stored_config_toml_path: None,
            created_at_utc: now,
            updated_at_utc: now,
        };

        self.apply_profile_input(&mut profile, &profile_dir, input, None)?;
        database.profiles.push(profile);
        Ok(())
    }

    fn update_profile(
        &self,
        database: &mut ProfileDatabase,
        profile_id: Uuid,
        input: &SaveProfileInput,
    ) -> AppResult<()> {
        validate_profile_name(&input.name)?;
        let profile = database
            .profiles
            .iter_mut()
            .find(|item| item.id == profile_id)
            .ok_or_else(|| AppError::validation("profile_not_found", "未找到要编辑的组合。"))?;
        let previous = profile.clone();
        let profile_dir = self.profile_directory_path(profile_id);
        ensure_directory(&profile_dir)?;

        profile.name = input.name.trim().to_string();
        profile.provider_category = input.provider_category;
        profile.updated_at_utc = Utc::now();
        self.apply_profile_input(profile, &profile_dir, input, Some(&previous))?;
        Ok(())
    }

    pub fn set_profile_auto_disabled(
        &self,
        profile_id: Uuid,
        disabled: bool,
    ) -> AppResult<ProfileDatabase> {
        let mut database = self.load()?;
        let profile = database
            .profiles
            .iter_mut()
            .find(|item| item.id == profile_id)
            .ok_or_else(|| AppError::validation("profile_not_found", "未找到要更新的组合。"))?;

        profile.auto_disabled = disabled;
        profile.updated_at_utc = Utc::now();
        if disabled {
            profile.auto_disabled_reason = Some("手动标记为禁用。".into());
            profile.auto_disabled_at_utc = Some(Utc::now());
        } else {
            profile.auto_disabled_reason = None;
            profile.auto_disabled_at_utc = None;
        }

        self.save(&database)?;
        Ok(database)
    }

    pub fn mark_profiles_auto_disabled(
        &self,
        failures: &[(Uuid, String)],
    ) -> AppResult<ProfileDatabase> {
        if failures.is_empty() {
            return self.load();
        }

        let mut database = self.load()?;
        let now = Utc::now();
        let mut changed = false;

        for (profile_id, reason) in failures {
            let profile = database
                .profiles
                .iter_mut()
                .find(|item| item.id == *profile_id)
                .ok_or_else(|| AppError::validation("profile_not_found", "未找到要禁用的组合。"))?;

            if !profile.auto_disabled
                || profile.auto_disabled_reason.as_deref() != Some(reason.as_str())
            {
                profile.auto_disabled = true;
                profile.auto_disabled_reason = Some(reason.trim().to_string());
                profile.auto_disabled_at_utc = Some(now);
                profile.updated_at_utc = now;
                changed = true;
            }
        }

        if changed {
            self.save(&database)?;
        }

        Ok(database)
    }

    fn apply_profile_input(
        &self,
        profile: &mut CodexProfile,
        profile_dir: &Path,
        input: &SaveProfileInput,
        previous: Option<&CodexProfile>,
    ) -> AppResult<()> {
        profile.test_model = if input.provider_category == ProviderCategory::OpenAI {
            None
        } else {
            normalize_optional_value(input.test_model.as_deref())
        };

        if input.provider_category == ProviderCategory::OpenAI {
            profile.provider_category = ProviderCategory::OpenAI;
            profile.auth_mode = CodexAuthMode::AuthJsonFile;
            profile.base_url.clear();
            profile.stored_config_toml_path = None;
            profile.protected_api_key_base64 = None;

            if let Some(auth_source) =
                normalize_optional_value(input.auth_json_source_path.as_deref())
            {
                profile.stored_auth_json_path = Some(self.copy_profile_asset_required(
                    &auth_source,
                    profile_dir,
                    "auth.json",
                )?);
            } else if previous
                .and_then(|item| item.stored_auth_json_path.as_deref())
                .filter(|path| Path::new(path).exists())
                .is_some()
            {
                profile.stored_auth_json_path =
                    previous.and_then(|item| item.stored_auth_json_path.clone());
            } else {
                return Err(AppError::validation(
                    "openai_auth_missing",
                    "OpenAI 提供商需要 auth.json 文件。",
                ));
            }

            return Ok(());
        }

        profile.provider_category = ProviderCategory::ApiKey;
        profile.base_url = normalize_optional_value(input.base_url.as_deref())
            .or_else(|| previous.map(|item| item.base_url.clone()))
            .unwrap_or_default();

        if input.import_config_toml {
            if let Some(config_source) =
                normalize_optional_value(input.config_toml_source_path.as_deref())
            {
                profile.stored_config_toml_path = Some(self.copy_profile_asset_required(
                    &config_source,
                    profile_dir,
                    "config.toml",
                )?);
            } else if previous
                .and_then(|item| item.stored_config_toml_path.as_deref())
                .filter(|path| Path::new(path).exists())
                .is_some()
            {
                profile.stored_config_toml_path =
                    previous.and_then(|item| item.stored_config_toml_path.clone());
            } else {
                return Err(AppError::validation(
                    "config_toml_missing",
                    "请选择 config.toml。",
                ));
            }
        } else {
            if profile.base_url.trim().is_empty() {
                return Err(AppError::validation(
                    "base_url_missing",
                    "请输入 Base URL。",
                ));
            }
            profile.stored_config_toml_path = None;
        }

        profile.auth_mode = input.auth_mode;
        match input.auth_mode {
            CodexAuthMode::ApiKey => {
                if let Some(api_key) = normalize_optional_value(input.api_key.as_deref()) {
                    profile.protected_api_key_base64 = Some(protect_to_base64(&api_key)?);
                } else if previous
                    .and_then(|item| item.protected_api_key_base64.as_deref())
                    .is_some()
                {
                    profile.protected_api_key_base64 =
                        previous.and_then(|item| item.protected_api_key_base64.clone());
                } else {
                    return Err(AppError::validation("api_key_missing", "请输入 API Key。"));
                }
                profile.stored_auth_json_path = None;
            }
            CodexAuthMode::AuthJsonFile => {
                if let Some(auth_source) =
                    normalize_optional_value(input.auth_json_source_path.as_deref())
                {
                    profile.stored_auth_json_path = Some(self.copy_profile_asset_required(
                        &auth_source,
                        profile_dir,
                        "auth.json",
                    )?);
                } else if previous
                    .and_then(|item| item.stored_auth_json_path.as_deref())
                    .filter(|path| Path::new(path).exists())
                    .is_some()
                {
                    profile.stored_auth_json_path =
                        previous.and_then(|item| item.stored_auth_json_path.clone());
                } else {
                    return Err(AppError::validation(
                        "auth_json_missing",
                        "请选择 auth.json。",
                    ));
                }
                profile.protected_api_key_base64 = None;
            }
        }

        Ok(())
    }

    fn copy_profile_asset_required(
        &self,
        source_path: &str,
        profile_dir: &Path,
        file_name: &str,
    ) -> AppResult<String> {
        let source = PathBuf::from(source_path);
        if !source.exists() {
            return Err(AppError::validation(
                "source_file_missing",
                format!("{file_name} 文件不存在。"),
            ));
        }

        let destination = profile_dir.join(file_name);
        fs::copy(&source, &destination)?;
        Ok(destination.to_string_lossy().to_string())
    }

    fn copy_existing_profile_asset(
        &self,
        source_path: &str,
        profile_dir: &Path,
        file_name: &str,
    ) -> AppResult<Option<String>> {
        let source = PathBuf::from(source_path);
        if !source.exists() {
            return Ok(None);
        }

        let destination = profile_dir.join(file_name);
        fs::copy(&source, &destination)?;
        Ok(Some(destination.to_string_lossy().to_string()))
    }

    fn profile_directory_path(&self, profile_id: Uuid) -> PathBuf {
        self.paths
            .profiles_root
            .join(profile_id.simple().to_string())
    }

    fn backup_legacy_data_if_needed(&self, database: &ProfileDatabase) -> AppResult<bool> {
        if database.migration_version >= CURRENT_MIGRATION_VERSION {
            return Ok(false);
        }

        if !self.paths.database_path.exists()
            && !self.paths.profiles_root.exists()
            && !self.paths.templates_root.exists()
        {
            return Ok(false);
        }

        let backup_root = self
            .paths
            .data_root
            .join("migration-backups")
            .join(Utc::now().format("%Y%m%d-%H%M%S").to_string());
        ensure_directory(&backup_root)?;

        if self.paths.database_path.exists() {
            fs::copy(&self.paths.database_path, backup_root.join("profiles.json"))?;
        }
        if self.paths.profiles_root.exists() {
            copy_directory(&self.paths.profiles_root, &backup_root.join("profiles"))?;
        }
        if self.paths.templates_root.exists() {
            copy_directory(&self.paths.templates_root, &backup_root.join("templates"))?;
        }

        Ok(true)
    }

    fn normalize_database(&self, database: &mut ProfileDatabase) -> bool {
        let mut changed = false;

        if !database.replace_windows_target && !database.replace_wsl_target {
            database.replace_windows_target = true;
            changed = true;
        }

        let normalized_provider_name =
            normalize_optional_value(Some(&database.api_key_provider_name))
                .unwrap_or_else(|| DEFAULT_APIKEY_PROVIDER_NAME.to_string());
        if database.api_key_provider_name != normalized_provider_name {
            database.api_key_provider_name = normalized_provider_name;
            changed = true;
        }

        if database.migration_version != CURRENT_MIGRATION_VERSION {
            database.migration_version = CURRENT_MIGRATION_VERSION;
            changed = true;
        }

        for profile in &mut database.profiles {
            let normalized_reason =
                normalize_optional_value(profile.auto_disabled_reason.as_deref());
            if profile.auto_disabled_reason != normalized_reason {
                profile.auto_disabled_reason = normalized_reason;
                changed = true;
            }

            if !profile.auto_disabled
                && (profile.auto_disabled_reason.is_some()
                    || profile.auto_disabled_at_utc.is_some())
            {
                profile.auto_disabled_reason = None;
                profile.auto_disabled_at_utc = None;
                changed = true;
            }
        }

        changed
    }
}

fn write_json_atomically(path: &Path, content: &str) -> AppResult<()> {
    let temp_path = path.with_extension(format!("{}.tmp", Uuid::new_v4().simple()));
    let mut file = File::create(&temp_path)?;
    file.write_all(content.as_bytes())?;
    file.flush()?;
    file.sync_all()?;
    replace_file_atomically(&temp_path, path)?;
    Ok(())
}

#[cfg(windows)]
fn replace_file_atomically(source: &Path, destination: &Path) -> AppResult<()> {
    use std::os::windows::ffi::OsStrExt;

    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };

    let source_wide: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination_wide: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();

    let result = unsafe {
        MoveFileExW(
            PCWSTR(source_wide.as_ptr()),
            PCWSTR(destination_wide.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };

    if result.is_err() {
        let _ = fs::remove_file(source);
        return Err(std::io::Error::last_os_error().into());
    }

    Ok(())
}

#[cfg(not(windows))]
fn replace_file_atomically(source: &Path, destination: &Path) -> AppResult<()> {
    fs::rename(source, destination)?;
    Ok(())
}

fn validate_profile_name(name: &str) -> AppResult<()> {
    if name.trim().is_empty() {
        return Err(AppError::validation("profile_name_missing", "请输入名称。"));
    }
    Ok(())
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
    fn load_will_normalize_legacy_database_and_create_migration_backup() {
        let workspace = tempdir().unwrap();
        let paths = build_test_paths(workspace.path());
        fs::create_dir_all(&paths.data_root).unwrap();

        let mut database = ProfileDatabase::default();
        database.replace_windows_target = false;
        database.replace_wsl_target = false;
        database.api_key_provider_name = "   ".into();
        database.migration_version = 0;

        fs::write(
            &paths.database_path,
            serde_json::to_string_pretty(&database).unwrap(),
        )
        .unwrap();

        let store = ProfileStore::new(&paths);
        let loaded = store.load().unwrap();

        assert!(loaded.replace_windows_target);
        assert!(!loaded.replace_wsl_target);
        assert_eq!(loaded.api_key_provider_name, DEFAULT_APIKEY_PROVIDER_NAME);
        assert_eq!(loaded.migration_version, CURRENT_MIGRATION_VERSION);
        assert!(paths.data_root.join("migration-backups").exists());
    }

    #[test]
    fn save_writes_database_without_leaving_temp_files() {
        let workspace = tempdir().unwrap();
        let paths = build_test_paths(workspace.path());
        let store = ProfileStore::new(&paths);
        let mut database = ProfileDatabase::default();
        database.api_key_provider_name = "openai".into();

        store.save(&database).unwrap();

        let saved = fs::read_to_string(&paths.database_path).unwrap();
        assert!(saved.contains("\"apiKeyProviderName\": \"openai\""));
        let temp_count = fs::read_dir(&paths.data_root)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .and_then(|item| item.to_str())
                    .map(|item| item == "tmp")
                    .unwrap_or(false)
            })
            .count();
        assert_eq!(temp_count, 0);
    }

    #[test]
    fn mark_profiles_auto_disabled_writes_reason_and_timestamp() {
        let workspace = tempdir().unwrap();
        let paths = build_test_paths(workspace.path());
        let store = ProfileStore::new(&paths);
        let profile = CodexProfile {
            name: "测试 APIKEY".into(),
            base_url: "https://example.com/v1".into(),
            ..CodexProfile::default()
        };
        let profile_id = profile.id;
        store
            .save(&ProfileDatabase {
                profiles: vec![profile],
                ..ProfileDatabase::default()
            })
            .unwrap();

        let database = store
            .mark_profiles_auto_disabled(&[(profile_id, "连接失败：超时。".into())])
            .unwrap();

        let profile = database
            .profiles
            .iter()
            .find(|item| item.id == profile_id)
            .unwrap();
        assert!(profile.auto_disabled);
        assert_eq!(
            profile.auto_disabled_reason.as_deref(),
            Some("连接失败：超时。")
        );
        assert!(profile.auto_disabled_at_utc.is_some());
    }

    #[test]
    fn set_profile_auto_disabled_false_clears_disable_metadata() {
        let workspace = tempdir().unwrap();
        let paths = build_test_paths(workspace.path());
        let store = ProfileStore::new(&paths);
        let profile = CodexProfile {
            name: "测试 APIKEY".into(),
            base_url: "https://example.com/v1".into(),
            auto_disabled: true,
            auto_disabled_reason: Some("连接失败：认证没有通过。".into()),
            auto_disabled_at_utc: Some(Utc::now()),
            ..CodexProfile::default()
        };
        let profile_id = profile.id;
        store
            .save(&ProfileDatabase {
                profiles: vec![profile],
                ..ProfileDatabase::default()
            })
            .unwrap();

        let database = store.set_profile_auto_disabled(profile_id, false).unwrap();
        let profile = database
            .profiles
            .iter()
            .find(|item| item.id == profile_id)
            .unwrap();
        assert!(!profile.auto_disabled);
        assert!(profile.auto_disabled_reason.is_none());
        assert!(profile.auto_disabled_at_utc.is_none());
    }
}
