use serde::Serialize;
use thiserror::Error;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{message}")]
    Validation { code: String, message: String },
    #[error("{message}")]
    Io {
        code: String,
        message: String,
        #[source]
        source: std::io::Error,
    },
    #[error("{message}")]
    Json {
        code: String,
        message: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("{message}")]
    Sqlite {
        code: String,
        message: String,
        #[source]
        source: rusqlite::Error,
    },
    #[error("{message}")]
    Http {
        code: String,
        message: String,
        #[source]
        source: reqwest::Error,
    },
    #[error("{message}")]
    Internal {
        code: String,
        message: String,
        detail: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppErrorDto {
    pub code: String,
    pub message: String,
    pub detail: Option<String>,
}

impl AppError {
    pub fn validation(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Validation {
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn internal(
        code: impl Into<String>,
        message: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self::Internal {
            code: code.into(),
            message: message.into(),
            detail: Some(detail.into()),
        }
    }

    pub fn to_dto(&self) -> AppErrorDto {
        match self {
            Self::Validation { code, message } => AppErrorDto {
                code: code.clone(),
                message: message.clone(),
                detail: None,
            },
            Self::Io {
                code,
                message,
                source,
            } => AppErrorDto {
                code: code.clone(),
                message: message.clone(),
                detail: Some(source.to_string()),
            },
            Self::Json {
                code,
                message,
                source,
            } => AppErrorDto {
                code: code.clone(),
                message: message.clone(),
                detail: Some(source.to_string()),
            },
            Self::Sqlite {
                code,
                message,
                source,
            } => AppErrorDto {
                code: code.clone(),
                message: message.clone(),
                detail: Some(source.to_string()),
            },
            Self::Http {
                code,
                message,
                source,
            } => AppErrorDto {
                code: code.clone(),
                message: message.clone(),
                detail: Some(source.to_string()),
            },
            Self::Internal {
                code,
                message,
                detail,
            } => AppErrorDto {
                code: code.clone(),
                message: message.clone(),
                detail: detail.clone(),
            },
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(source: std::io::Error) -> Self {
        Self::Io {
            code: "io_error".into(),
            message: "文件系统操作失败。".into(),
            source,
        }
    }
}

impl From<serde_json::Error> for AppError {
    fn from(source: serde_json::Error) -> Self {
        Self::Json {
            code: "json_error".into(),
            message: "JSON 数据处理失败。".into(),
            source,
        }
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(source: rusqlite::Error) -> Self {
        Self::Sqlite {
            code: "sqlite_error".into(),
            message: "SQLite 操作失败。".into(),
            source,
        }
    }
}

impl From<reqwest::Error> for AppError {
    fn from(source: reqwest::Error) -> Self {
        Self::Http {
            code: "http_error".into(),
            message: "网络请求失败。".into(),
            source,
        }
    }
}
