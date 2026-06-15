// Placeholder - to be implemented in Task 7
use axum::extract::{State, Path};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use std::sync::Arc;
use wiki_server::{AppState, UserCreateRequest, UserCreateResponse, WikiError, PasswordSetRequest};

pub async fn create_user(
    _state: State<Arc<AppState>>,
    _req: Json<UserCreateRequest>,
) -> Result<Json<UserCreateResponse>, WikiError> {
    Err(WikiError::InternalError("not implemented".into()))
}

pub async fn delete_user(
    _state: State<Arc<AppState>>,
    _username: Path<String>,
) -> Result<StatusCode, WikiError> {
    Err(WikiError::InternalError("not implemented".into()))
}

pub async fn set_password(
    _state: State<Arc<AppState>>,
    _username: Path<String>,
    _req: Json<PasswordSetRequest>,
) -> Result<StatusCode, WikiError> {
    Err(WikiError::InternalError("not implemented".into()))
}
