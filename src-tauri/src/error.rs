use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    Message(String),
    #[error("File system error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid data: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Network error: {0}")]
    Http(String),
    #[error("Archive error: {0}")]
    #[cfg(target_os = "macos")]
    Archive(String),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl From<reqwest::Error> for AppError {
    fn from(error: reqwest::Error) -> Self {
        AppError::Http(error.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;

pub fn message<T>(text: impl Into<String>) -> AppResult<T> {
    Err(AppError::Message(text.into()))
}
