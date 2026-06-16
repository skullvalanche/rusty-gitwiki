use axum::{
    extract::{State, Path},
    http::StatusCode,
    Json,
};
use std::sync::Arc;
use wiki_server::{
    AppState, PasswordSetRequest, UserAdminResponse, UserCreateRequest, UserCreateResponse,
    UserRoleSetRequest, WikiError,
};
use crate::auth;

pub async fn list_users(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(current_user): axum::extract::Extension<crate::auth::CurrentUser>,
) -> Result<Json<Vec<UserAdminResponse>>, WikiError> {
    if !current_user.is_admin() {
        return Err(WikiError::Unauthorized);
    }

    let users_file = state.wiki_data_dir.join(".users.json");
    let mut users = auth::load_users(&users_file)
        .map_err(|e| WikiError::InternalError(e.to_string()))?
        .into_iter()
        .map(|user| UserAdminResponse {
            role: user.role,
            username: user.username,
            created_at: user.created_at,
            name: user.name,
            email: user.email,
            description: user.description,
        })
        .collect::<Vec<_>>();

    users.sort_by(|a, b| a.username.cmp(&b.username));
    Ok(Json(users))
}

pub async fn create_user(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(current_user): axum::extract::Extension<crate::auth::CurrentUser>,
    Json(req): Json<UserCreateRequest>,
) -> Result<Json<UserCreateResponse>, WikiError> {
    if !current_user.is_admin() {
        return Err(WikiError::Unauthorized);
    }

    let users_file = state.wiki_data_dir.join(".users.json");

    let user = auth::create_user(&users_file, &req.username, &req.password, req.role)
        .map_err(|e| WikiError::InternalError(e.to_string()))?;

    Ok(Json(UserCreateResponse {
        username: user.username,
        created_at: user.created_at,
        role: user.role,
    }))
}

pub async fn set_role(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(current_user): axum::extract::Extension<crate::auth::CurrentUser>,
    Path(username): Path<String>,
    Json(req): Json<UserRoleSetRequest>,
) -> Result<Json<UserAdminResponse>, WikiError> {
    if !current_user.is_admin() {
        return Err(WikiError::Unauthorized);
    }

    if username == current_user.username && !req.role.is_admin() {
        return Err(WikiError::Conflict("Cannot remove your own admin role".to_string()));
    }

    let users_file = state.wiki_data_dir.join(".users.json");
    let user = auth::set_user_role(&users_file, &username, req.role)
        .map_err(|e| WikiError::InternalError(e.to_string()))?;

    Ok(Json(UserAdminResponse {
        role: user.role,
        username: user.username,
        created_at: user.created_at,
        name: user.name,
        email: user.email,
        description: user.description,
    }))
}

pub async fn delete_user(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(current_user): axum::extract::Extension<crate::auth::CurrentUser>,
    Path(username): Path<String>,
) -> Result<StatusCode, WikiError> {
    if !current_user.is_admin() {
        return Err(WikiError::Unauthorized);
    }

    if username == current_user.username {
        return Err(WikiError::Conflict("Cannot remove your own account".to_string()));
    }

    let users_file = state.wiki_data_dir.join(".users.json");
    auth::delete_user(&users_file, &username)
        .map_err(|e| WikiError::InternalError(e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn set_password(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(current_user): axum::extract::Extension<crate::auth::CurrentUser>,
    Path(username): Path<String>,
    Json(req): Json<PasswordSetRequest>,
) -> Result<StatusCode, WikiError> {
    if !current_user.is_admin() {
        return Err(WikiError::Unauthorized);
    }

    let users_file = state.wiki_data_dir.join(".users.json");
    auth::set_user_password(&users_file, &username, &req.password)
        .map_err(|e| WikiError::InternalError(e.to_string()))?;

    Ok(StatusCode::OK)
}
