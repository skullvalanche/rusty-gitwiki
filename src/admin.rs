use axum::{
    extract::{State, Path},
    http::StatusCode,
    Json,
};
use std::sync::Arc;
use wiki_server::{AppState, UserCreateRequest, UserCreateResponse, WikiError, PasswordSetRequest};
use crate::auth;

pub async fn create_user(
    State(state): State<Arc<AppState>>,
    Json(req): Json<UserCreateRequest>,
) -> Result<Json<UserCreateResponse>, WikiError> {
    let users_file = state.wiki_data_dir.join(".users.json");

    // Check if user is admin (TODO: extract from request context)
    // For now, allow creation

    let user = auth::create_user(&users_file, &req.username, &req.password, req.is_admin)
        .map_err(|e| WikiError::InternalError(e.to_string()))?;

    Ok(Json(UserCreateResponse {
        username: user.username,
        created_at: user.created_at,
    }))
}

pub async fn delete_user(
    State(state): State<Arc<AppState>>,
    Path(username): Path<String>,
) -> Result<StatusCode, WikiError> {
    let users_file = state.wiki_data_dir.join(".users.json");

    // Check if user is admin (TODO: extract from request context)
    // For now, allow deletion

    auth::delete_user(&users_file, &username)
        .map_err(|e| WikiError::InternalError(e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn set_password(
    State(state): State<Arc<AppState>>,
    Path(username): Path<String>,
    Json(req): Json<PasswordSetRequest>,
) -> Result<StatusCode, WikiError> {
    let users_file = state.wiki_data_dir.join(".users.json");

    // Check if user is admin (TODO: extract from request context)
    // For now, allow password changes

    auth::set_user_password(&users_file, &username, &req.password)
        .map_err(|e| WikiError::InternalError(e.to_string()))?;

    Ok(StatusCode::OK)
}
