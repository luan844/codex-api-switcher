use std::collections::VecDeque;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::sync::OnceLock;

use chrono::Utc;
use regex::Regex;
use serde_json::json;

use crate::errors::app_error::{AppError, AppResult};
use crate::models::app::LogEntry;
use crate::state::app_state::AppPaths;
use crate::utils::fs_utils::ensure_directory;

pub struct LogService<'a> {
    paths: &'a AppPaths,
}

impl<'a> LogService<'a> {
    pub fn new(paths: &'a AppPaths) -> Self {
        Self { paths }
    }

    pub fn append(
        &self,
        level: &str,
        action: &str,
        message: &str,
        context: serde_json::Value,
    ) -> AppResult<()> {
        ensure_directory(&self.paths.logs_root)?;

        let entry = sanitize_log_entry(LogEntry {
            timestamp: Utc::now().to_rfc3339(),
            level: level.to_string(),
            action: action.to_string(),
            message: message.to_string(),
            context,
        });

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.paths.structured_log_path)?;
        writeln!(file, "{}", serde_json::to_string(&entry)?)?;
        Ok(())
    }

    pub fn append_success(
        &self,
        action: &str,
        message: &str,
        context: serde_json::Value,
    ) -> AppResult<()> {
        self.append("info", action, message, context)
    }

    pub fn append_failure(
        &self,
        action: &str,
        message: &str,
        detail: impl Into<String>,
    ) -> AppResult<()> {
        self.append("error", action, message, json!({ "detail": detail.into() }))
    }

    pub fn read_recent(&self, limit: usize) -> AppResult<Vec<LogEntry>> {
        if !self.paths.structured_log_path.exists() {
            return Ok(Vec::new());
        }

        if limit == 0 {
            return Ok(Vec::new());
        }

        let file = fs::File::open(&self.paths.structured_log_path)?;
        let reader = BufReader::new(file);
        let mut entries = VecDeque::with_capacity(limit);
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }

            let entry =
                serde_json::from_str::<LogEntry>(&line).map_err(|source| AppError::Json {
                    code: "log_entry_parse_failed".into(),
                    message: "日志文件格式损坏。".into(),
                    source,
                })?;
            if entries.len() == limit {
                entries.pop_front();
            }
            entries.push_back(sanitize_log_entry(entry));
        }

        Ok(entries.into_iter().rev().collect())
    }
}

fn sanitize_log_entry(entry: LogEntry) -> LogEntry {
    LogEntry {
        timestamp: entry.timestamp,
        level: entry.level,
        action: entry.action,
        message: sanitize_log_string(None, &entry.message),
        context: sanitize_json_value(None, entry.context),
    }
}

fn sanitize_json_value(key: Option<&str>, value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.into_iter()
                .map(|(entry_key, entry_value)| {
                    let sanitized_value = sanitize_json_value(Some(&entry_key), entry_value);
                    (entry_key, sanitized_value)
                })
                .collect(),
        ),
        serde_json::Value::Array(items) => serde_json::Value::Array(
            items
                .into_iter()
                .map(|item| sanitize_json_value(key, item))
                .collect(),
        ),
        serde_json::Value::String(text) => {
            serde_json::Value::String(sanitize_log_string(key, &text))
        }
        other => other,
    }
}

fn sanitize_log_string(key: Option<&str>, value: &str) -> String {
    if key.is_some_and(is_sensitive_key) {
        return "[已隐藏敏感值]".into();
    }

    let secrets_masked = secret_regex().replace_all(value, "[已隐藏敏感值]");
    path_regex()
        .replace_all(secrets_masked.as_ref(), "[已隐藏路径]")
        .to_string()
}

fn is_sensitive_key(key: &str) -> bool {
    let normalized = key.to_ascii_lowercase();
    normalized.contains("api_key")
        || normalized.contains("apikey")
        || normalized.contains("token")
        || normalized.contains("secret")
        || normalized.contains("password")
        || normalized.contains("authorization")
}

fn path_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r#"(?i)(?:[A-Z]:[\\/]|\\\\wsl\$\\|/mnt/[a-z]/|/home/|/Users/)[^\s\"']+"#)
            .expect("日志路径脱敏正则初始化失败")
    })
}

fn secret_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r#"(?i)(sk-[A-Za-z0-9_-]{10,}|ghp_[A-Za-z0-9]{10,}|github_pat_[A-Za-z0-9_]{20,}|bearer\s+[A-Za-z0-9._-]{10,}|AIza[0-9A-Za-z\-_]{10,}|xox[baprs]-[A-Za-z0-9-]{10,})"#,
        )
        .expect("日志密钥脱敏正则初始化失败")
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use serde_json::json;
    use tempfile::tempdir;

    use super::{sanitize_json_value, sanitize_log_string};
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
    fn sanitize_log_string_hides_paths_and_tokens() {
        let token = ["sk", "1234567890abcdef"].join("-");
        let windows_path = ["X:", "masked", "private", "auth.json"].join("/");
        let wsl_mounted_path = "/mnt/c/Users/demo-user/private/auth.json";
        let text = format!("Bearer {token} {windows_path} {wsl_mounted_path}");
        let sanitized = sanitize_log_string(None, text.as_str());

        assert!(!sanitized.contains(token.as_str()));
        assert!(!sanitized.contains(windows_path.as_str()));
        assert!(!sanitized.contains(wsl_mounted_path));
        assert!(sanitized.contains("[已隐藏敏感值]"));
        assert!(sanitized.contains("[已隐藏路径]"));
    }

    #[test]
    fn sanitize_json_value_hides_sensitive_keys() {
        let api_key = ["sk", "live-secret"].join("-");
        let authorization = ["Bearer", "abcdefghijklmnop"].join(" ");
        let value = json!({
            "apiKey": api_key,
            "detail": "D:/workspace/private/result.json",
            "nested": {
                "authorization": authorization
            }
        });

        let sanitized = sanitize_json_value(None, value);
        let serialized = sanitized.to_string();

        assert!(!serialized.contains("live-secret"));
        assert!(!serialized.contains("D:/workspace/private/result.json"));
        assert!(!serialized.contains("abcdefghijklmnop"));
    }

    #[test]
    fn read_recent_returns_latest_entries_in_reverse_order() {
        let workspace = tempdir().unwrap();
        let paths = build_test_paths(workspace.path());
        fs::create_dir_all(&paths.logs_root).unwrap();

        let lines = (1..=4)
            .map(|index| {
                serde_json::to_string(&json!({
                    "timestamp": format!("2026-03-28T10:00:0{index}Z"),
                    "level": "info",
                    "action": format!("step-{index}"),
                    "message": format!("message-{index}"),
                    "context": { "index": index }
                }))
                .unwrap()
            })
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(&paths.structured_log_path, lines).unwrap();

        let service = super::LogService::new(&paths);
        let recent = service.read_recent(2).unwrap();

        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].action, "step-4");
        assert_eq!(recent[1].action, "step-3");
    }
}
