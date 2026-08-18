use std::fs;
use std::path::Path;

use walkdir::WalkDir;

use crate::errors::app_error::{AppError, AppResult};

pub fn ensure_directory(path: &Path) -> AppResult<()> {
    fs::create_dir_all(path)?;
    Ok(())
}

pub fn copy_directory(source: &Path, destination: &Path) -> AppResult<()> {
    ensure_directory(destination)?;

    for entry in WalkDir::new(source) {
        let entry = entry.map_err(|error| {
            AppError::internal(
                "copy_directory_walk_failed",
                "复制目录时遍历失败。",
                error.to_string(),
            )
        })?;
        let relative = entry.path().strip_prefix(source).map_err(|error| {
            AppError::internal(
                "copy_directory_strip_prefix_failed",
                "复制目录时计算相对路径失败。",
                error.to_string(),
            )
        })?;
        let target = destination.join(relative);

        if entry.file_type().is_dir() {
            ensure_directory(&target)?;
            continue;
        }

        if let Some(parent) = target.parent() {
            ensure_directory(parent)?;
        }
        fs::copy(entry.path(), &target)?;
    }

    Ok(())
}
