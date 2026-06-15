use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub username: String,
    pub password_hash: String,
    pub is_admin: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page {
    pub path: String,
    pub content: String,
    pub updated_at: DateTime<Utc>,
    pub updated_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitInfo {
    pub commit_hash: String,
    pub author: String,
    pub message: String,
    pub date: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictResolution {
    pub path: String,
    pub resolved_content: String,
    pub conflict_commit_hash: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PageResponse {
    pub path: String,
    pub content: String,
    pub history: Vec<CommitInfo>,
    pub current_git_head: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SaveResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conflict: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub their_changes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListPageResponse {
    pub path: String,
    pub title: String,
    pub updated_at: DateTime<Utc>,
    pub updated_by: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResult {
    pub path: String,
    pub excerpt: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserCreateRequest {
    pub username: String,
    pub password: String,
    pub is_admin: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserCreateResponse {
    pub username: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PasswordSetRequest {
    pub password: String,
}

#[derive(Debug)]
pub struct AppState {
    pub wiki_data_dir: PathBuf,
}

#[derive(Debug, thiserror::Error)]
pub enum WikiError {
    #[error("Unauthorized")]
    Unauthorized,
    #[error("Not found")]
    NotFound,
    #[error("Conflict")]
    Conflict(String),
    #[error("Git error: {0}")]
    GitError(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("Internal error: {0}")]
    InternalError(String),
}

impl axum::response::IntoResponse for WikiError {
    fn into_response(self) -> axum::response::Response {
        use axum::http::StatusCode;

        let (status, message) = match self {
            WikiError::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized".to_string()),
            WikiError::NotFound => (StatusCode::NOT_FOUND, "Not found".to_string()),
            WikiError::Conflict(msg) => (StatusCode::CONFLICT, msg),
            WikiError::GitError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            WikiError::IoError(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            WikiError::JsonError(e) => (StatusCode::BAD_REQUEST, e.to_string()),
            WikiError::InternalError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };

        (status, message).into_response()
    }
}
