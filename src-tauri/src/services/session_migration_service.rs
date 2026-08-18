use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration as StdDuration;

use chrono::{Duration, Local};
use regex::Regex;
use rusqlite::Connection;
use serde::Serialize;
use uuid::Uuid;

use crate::errors::app_error::AppResult;
use crate::models::profile::ProviderCategory;
use crate::services::target_service::TargetContext;
use crate::services::wsl_service::WslService;

const WSL_SESSION_MIGRATION_PYTHON: &str = r#"import json
import os
import sqlite3
import sys
import time

payload_path = sys.argv[1]
codex_directory = sys.argv[2]

with open(payload_path, "r", encoding="utf-8") as handle:
    payload = json.load(handle)

session_ids = [session_id for session_id in payload.get("SessionIds", []) if session_id]
model_provider = payload.get("ModelProvider")

if not model_provider or not session_ids:
    sys.exit(0)

db_path = os.path.join(codex_directory, "state_5.sqlite")
if not os.path.exists(db_path):
    sys.exit(0)

for attempt in range(3):
    connection = None
    try:
        connection = sqlite3.connect(db_path, timeout=5)
        cursor = connection.cursor()
        cursor.execute("PRAGMA busy_timeout = 5000")
        cursor.execute("BEGIN IMMEDIATE")
        cursor.executemany(
            "UPDATE threads SET model_provider = ? WHERE id = ?",
            [(model_provider, session_id) for session_id in session_ids],
        )
        connection.commit()
        break
    except sqlite3.OperationalError as ex:
        if "locked" in str(ex).lower() and attempt < 2:
            time.sleep(0.5 * (attempt + 1))
            continue
        raise
    finally:
        if connection is not None:
            connection.close()
"#;

pub struct SessionMigrationService {
    model_provider_regex: Regex,
    session_filename_regex: Regex,
    wsl_service: WslService,
}

impl SessionMigrationService {
    pub fn new() -> Self {
        Self {
            model_provider_regex: Regex::new(
                r#"(?P<prefix>"model_provider"\s*:\s*")(?P<value>[^"\\]*(?:\\.[^"\\]*)*)(?P<suffix>")"#,
            )
            .expect("model_provider regex 初始化失败"),
            session_filename_regex: Regex::new(
                r#"(?P<id>[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})\.jsonl$"#,
            )
            .expect("session 文件名 regex 初始化失败"),
            wsl_service: WslService::new(),
        }
    }

    pub fn migrate(
        &self,
        target: &TargetContext,
        provider_category: ProviderCategory,
        api_key_provider_name: &str,
        session_migration_days: i32,
    ) -> AppResult<()> {
        if session_migration_days <= 0 {
            return Ok(());
        }

        let days = session_migration_days.clamp(1, 30);
        let sessions_root = target.codex_directory_path.join("sessions");
        if !sessions_root.exists() {
            return Ok(());
        }

        let target_provider = if provider_category == ProviderCategory::OpenAI {
            "openai".to_string()
        } else {
            api_key_provider_name.to_string()
        };
        let mut session_ids = HashSet::new();

        for offset in 0..days {
            let date = Local::now().date_naive() - Duration::days(offset as i64);
            let day_dir = sessions_root
                .join(date.format("%Y").to_string())
                .join(date.format("%m").to_string())
                .join(date.format("%d").to_string());

            if !day_dir.exists() {
                continue;
            }

            for entry in fs::read_dir(day_dir)? {
                let entry = entry?;
                if !entry.file_type()?.is_file() {
                    continue;
                }
                if entry.path().extension().and_then(|item| item.to_str()) != Some("jsonl") {
                    continue;
                }

                if let Some(session_id) =
                    self.rewrite_session_first_line(&entry.path(), &target_provider)?
                {
                    session_ids.insert(session_id);
                }
            }
        }

        if session_ids.is_empty() {
            return Ok(());
        }

        if target.wsl_environment.is_some() && target.codex_linux_path.is_some() {
            self.rewrite_wsl_sqlite(target, &target_provider, session_ids)?;
        } else {
            self.rewrite_local_sqlite(&target.codex_directory_path, &target_provider, session_ids)?;
        }

        Ok(())
    }

    fn rewrite_session_first_line(
        &self,
        file_path: &Path,
        target_provider: &str,
    ) -> AppResult<Option<String>> {
        let reader = BufReader::new(File::open(file_path)?);
        let lines: Vec<String> = reader.lines().collect::<Result<_, _>>()?;
        if lines.is_empty() {
            return Ok(None);
        }

        let first_line = &lines[0];
        let Some(captures) = self.model_provider_regex.captures(first_line) else {
            return Ok(None);
        };

        let escaped_target = serde_json::to_string(target_provider)
            .map(|item| item.trim_matches('"').to_string())?;
        let current = captures
            .name("value")
            .map(|item| item.as_str())
            .unwrap_or_default();
        let session_id = self.try_get_session_id(file_path, first_line);

        if current == escaped_target {
            return Ok(session_id);
        }

        let new_first_line = self.model_provider_regex.replace(
            first_line,
            format!(
                "{}{}{}",
                captures
                    .name("prefix")
                    .map(|item| item.as_str())
                    .unwrap_or_default(),
                escaped_target,
                captures
                    .name("suffix")
                    .map(|item| item.as_str())
                    .unwrap_or_default()
            ),
        );

        let temp_path = file_path.with_extension("jsonl.tmp");
        let mut writer = File::create(&temp_path)?;
        writeln!(writer, "{new_first_line}")?;
        for line in lines.iter().skip(1) {
            writeln!(writer, "{line}")?;
        }

        fs::copy(&temp_path, file_path)?;
        fs::remove_file(temp_path)?;
        Ok(session_id)
    }

    fn rewrite_local_sqlite(
        &self,
        codex_directory_path: &PathBuf,
        target_provider: &str,
        session_ids: HashSet<String>,
    ) -> AppResult<()> {
        let sqlite_path = codex_directory_path.join("state_5.sqlite");
        if !sqlite_path.exists() {
            return Ok(());
        }

        for attempt in 0..3 {
            match self.try_rewrite_local_sqlite(&sqlite_path, target_provider, &session_ids) {
                Ok(()) => return Ok(()),
                Err(error) => {
                    let detail = error.to_string().to_lowercase();
                    if detail.contains("locked") && attempt < 2 {
                        thread::sleep(StdDuration::from_millis(500 * (attempt + 1) as u64));
                        continue;
                    }
                    return Err(error.into());
                }
            }
        }

        Ok(())
    }

    fn try_rewrite_local_sqlite(
        &self,
        sqlite_path: &Path,
        target_provider: &str,
        session_ids: &HashSet<String>,
    ) -> rusqlite::Result<()> {
        let mut connection = Connection::open(sqlite_path)?;
        connection.busy_timeout(StdDuration::from_secs(5))?;
        let transaction = connection.transaction()?;
        {
            let mut statement =
                transaction.prepare("UPDATE threads SET model_provider = ?1 WHERE id = ?2")?;
            for session_id in session_ids {
                statement.execute((target_provider, session_id))?;
            }
        }
        transaction.commit()?;
        Ok(())
    }

    fn rewrite_wsl_sqlite(
        &self,
        target: &TargetContext,
        target_provider: &str,
        session_ids: HashSet<String>,
    ) -> AppResult<()> {
        let environment = target.wsl_environment.as_ref().expect("WSL 环境不存在");
        let codex_linux_path = target
            .codex_linux_path
            .as_ref()
            .expect("WSL Linux 路径不存在");
        let temp_dir = target.codex_directory_path.join("tmp");
        fs::create_dir_all(&temp_dir)?;

        let file_name = format!(
            "codex-switch-session-migration-{}.json",
            Uuid::new_v4().simple()
        );
        let windows_payload_path = temp_dir.join(&file_name);
        let linux_payload_path = format!(
            "{}/tmp/{}",
            codex_linux_path.trim_end_matches('/'),
            file_name
        );

        let payload = WslMigrationPayload {
            model_provider: target_provider.to_string(),
            session_ids: session_ids.into_iter().collect(),
        };
        fs::write(&windows_payload_path, serde_json::to_vec(&payload)?)?;

        let result = self
            .wsl_service
            .run_command(
                environment,
                "python3",
                &[
                    "-c",
                    WSL_SESSION_MIGRATION_PYTHON,
                    &linux_payload_path,
                    codex_linux_path,
                ],
            )
            .or_else(|_| {
                self.wsl_service.run_command(
                    environment,
                    "python",
                    &[
                        "-c",
                        WSL_SESSION_MIGRATION_PYTHON,
                        &linux_payload_path,
                        codex_linux_path,
                    ],
                )
            });

        let _ = fs::remove_file(windows_payload_path);
        result.map(|_| ())
    }

    fn try_get_session_id(&self, file_path: &Path, first_line: &str) -> Option<String> {
        if let Some(file_name) = file_path.file_name().and_then(|item| item.to_str()) {
            if let Some(captures) = self.session_filename_regex.captures(file_name) {
                return captures.name("id").map(|item| item.as_str().to_string());
            }
        }

        let value = serde_json::from_str::<serde_json::Value>(first_line).ok()?;
        value
            .get("payload")
            .and_then(|payload| payload.get("id"))
            .and_then(|item| item.as_str())
            .map(ToOwned::to_owned)
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct WslMigrationPayload {
    model_provider: String,
    session_ids: Vec<String>,
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn rewrite_session_first_line_updates_model_provider() {
        let workspace = tempdir().unwrap();
        let session_id = Uuid::new_v4();
        let file_path = workspace.path().join(format!("{session_id}.jsonl"));
        fs::write(
            &file_path,
            concat!(
                "{\"model_provider\":\"legacy\",\"payload\":{\"id\":\"placeholder\"}}\n",
                "{\"type\":\"message\"}\n"
            ),
        )
        .unwrap();

        let service = SessionMigrationService::new();
        let returned_id = service
            .rewrite_session_first_line(&file_path, "openai")
            .unwrap();
        let updated = fs::read_to_string(&file_path).unwrap();
        let expected_session_id = session_id.to_string();

        assert_eq!(returned_id.as_deref(), Some(expected_session_id.as_str()));
        assert!(updated
            .lines()
            .next()
            .unwrap()
            .contains(r#""model_provider":"openai""#));
        assert!(updated
            .lines()
            .nth(1)
            .unwrap()
            .contains(r#""type":"message""#));
    }

    #[test]
    fn try_get_session_id_falls_back_to_payload_id() {
        let workspace = tempdir().unwrap();
        let file_path = workspace.path().join("session.jsonl");
        let first_line = format!(
            "{{\"model_provider\":\"legacy\",\"payload\":{{\"id\":\"{}\"}}}}",
            Uuid::new_v4()
        );

        let service = SessionMigrationService::new();
        let session_id = service.try_get_session_id(&file_path, &first_line);

        assert!(session_id.is_some());
    }
}
