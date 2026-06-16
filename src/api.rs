use axum::{
    extract::{State, Path, Query},
    response::{IntoResponse, Html},
    http::{StatusCode, Request},
    body::Body,
    Json,
};
use serde::Deserialize;
use std::sync::Arc;
use wiki_server::{AppState, ListPageResponse, PageResponse, SaveResponse, SearchResult, WikiError};
use crate::{pages, search, git, auth};
use comrak::{markdown_to_html, ComrakOptions};

#[derive(Deserialize)]
pub struct SavePageRequest {
    pub content: String,
    pub expected_git_head: String,
}

#[derive(Deserialize)]
pub struct RenamePageRequest {
    pub new_path: String,
}

#[derive(Deserialize)]
pub struct ResolveConflictRequest {
    pub path: String,
    pub resolved_content: String,
    pub conflict_commit_hash: String,
}

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: String,
}

pub async fn list_pages(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<ListPageResponse>>, WikiError> {
    pages::list_pages(&state.wiki_data_dir)
        .map(Json)
        .map_err(|e| WikiError::InternalError(e.to_string()))
}

pub async fn get_page(
    State(state): State<Arc<AppState>>,
    Path(page_path): Path<String>,
) -> Result<Json<PageResponse>, WikiError> {
    match pages::read_page(&state.wiki_data_dir, &page_path) {
        Ok(content) => {
            let current_head = git::get_current_head(&state.wiki_data_dir)
                .map_err(|e| WikiError::GitError(e.to_string()))?;

            // Get git history
            let file_path = pages::path_to_file(&state.wiki_data_dir, &page_path)
                .map_err(|e| WikiError::InternalError(e.to_string()))?;

            let history_raw = git::get_git_log(&state.wiki_data_dir, &file_path, 10)
                .unwrap_or_default();

            let history = history_raw
                .into_iter()
                .map(|(hash, author, message)| wiki_server::CommitInfo {
                    commit_hash: hash,
                    author,
                    message,
                    date: chrono::Utc::now(), // TODO: parse from git log
                })
                .collect();

            let rendered = markdown_to_html(&content, &ComrakOptions::default());

            Ok(Json(PageResponse {
                path: page_path,
                content: rendered,
                raw: content,
                history,
                current_git_head: current_head,
            }))
        }
        Err(_) => Err(WikiError::NotFound),
    }
}

pub async fn save_page(
    State(state): State<Arc<AppState>>,
    Path(page_path): Path<String>,
    axum::extract::Extension(current_user): axum::extract::Extension<crate::auth::CurrentUser>,
    Json(req): Json<SavePageRequest>,
) -> Result<Json<SaveResponse>, WikiError> {
    if !current_user.can_edit() {
        return Err(WikiError::Unauthorized);
    }

    let file_path = pages::path_to_file(&state.wiki_data_dir, &page_path)
        .map_err(|e| WikiError::InternalError(e.to_string()))?;

    // New pages have no conflict
    let file_changed = if !file_path.exists() {
        false
    } else {
        git::file_changed_since_head(&state.wiki_data_dir, &file_path, &req.expected_git_head)
            .map_err(|e| WikiError::GitError(e.to_string()))?
    };

    if file_changed {
        // Conflict detected
        let current_content = pages::read_page(&state.wiki_data_dir, &page_path)
            .unwrap_or_default();

        return Ok(Json(SaveResponse {
            conflict: Some(true),
            current_content: Some(current_content),
            their_changes: Some(req.content),
            commit_hash: None,
            author: None,
            message: None,
            base: None,
        }));
    }

    // No conflict, write and commit
    pages::write_page(&state.wiki_data_dir, &page_path, &req.content, &current_user.username)
        .map_err(|e| WikiError::InternalError(e.to_string()))?;
    search::rebuild_index(&state.wiki_data_dir)
        .map_err(|e| WikiError::InternalError(e.to_string()))?;

    let commit_hash = git::get_current_head(&state.wiki_data_dir)
        .map_err(|e| WikiError::GitError(e.to_string()))?;

    Ok(Json(SaveResponse {
        commit_hash: Some(commit_hash),
        author: Some(current_user.username.clone()),
        message: Some(format!("Update {}", page_path)),
        conflict: Some(false),
        current_content: None,
        their_changes: None,
        base: None,
    }))
}

pub async fn archive_page(
    State(state): State<Arc<AppState>>,
    Path(page_path): Path<String>,
    axum::extract::Extension(current_user): axum::extract::Extension<crate::auth::CurrentUser>,
) -> Result<Json<serde_json::Value>, WikiError> {
    if !current_user.can_edit() {
        return Err(WikiError::Unauthorized);
    }

    let archived_path = pages::archive_page(&state.wiki_data_dir, &page_path, &current_user.username)
        .map_err(|e| {
            let message = e.to_string();
            if message.contains("not found") {
                WikiError::NotFound
            } else if message.contains("already exists") {
                WikiError::Conflict(message)
            } else {
                WikiError::InternalError(message)
            }
        })?;
    search::rebuild_index(&state.wiki_data_dir)
        .map_err(|e| WikiError::InternalError(e.to_string()))?;

    Ok(Json(serde_json::json!({ "path": archived_path })))
}

pub async fn rename_page(
    State(state): State<Arc<AppState>>,
    Path(page_path): Path<String>,
    axum::extract::Extension(current_user): axum::extract::Extension<crate::auth::CurrentUser>,
    Json(req): Json<RenamePageRequest>,
) -> Result<Json<SaveResponse>, WikiError> {
    if !current_user.can_edit() {
        return Err(WikiError::Unauthorized);
    }

    let new_path = req.new_path.trim();
    if new_path.is_empty() {
        return Err(WikiError::InternalError("New page path required".to_string()));
    }

    pages::rename_page(&state.wiki_data_dir, &page_path, new_path, &current_user.username)
        .map_err(|e| {
            let message = e.to_string();
            if message.contains("not found") {
                WikiError::NotFound
            } else if message.contains("already exists") {
                WikiError::Conflict(message)
            } else {
                WikiError::InternalError(message)
            }
        })?;
    search::rebuild_index(&state.wiki_data_dir)
        .map_err(|e| WikiError::InternalError(e.to_string()))?;

    let commit_hash = git::get_current_head(&state.wiki_data_dir)
        .map_err(|e| WikiError::GitError(e.to_string()))?;

    Ok(Json(SaveResponse {
        commit_hash: Some(commit_hash),
        author: Some(current_user.username.clone()),
        message: Some(format!("Rename {} to {}", page_path, new_path)),
        conflict: Some(false),
        current_content: None,
        their_changes: None,
        base: None,
    }))
}

pub async fn list_archived_pages(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(current_user): axum::extract::Extension<crate::auth::CurrentUser>,
) -> Result<Json<Vec<wiki_server::ArchivedPageResponse>>, WikiError> {
    if !current_user.is_admin() {
        return Err(WikiError::Unauthorized);
    }

    pages::list_archived_pages(&state.wiki_data_dir)
        .map(Json)
        .map_err(|e| WikiError::InternalError(e.to_string()))
}

pub async fn restore_archived_page(
    State(state): State<Arc<AppState>>,
    Path(page_path): Path<String>,
    axum::extract::Extension(current_user): axum::extract::Extension<crate::auth::CurrentUser>,
) -> Result<Json<SaveResponse>, WikiError> {
    if !current_user.is_admin() {
        return Err(WikiError::Unauthorized);
    }

    pages::restore_archived_page(&state.wiki_data_dir, &page_path, &current_user.username)
        .map_err(|e| {
            let message = e.to_string();
            if message.contains("not found") {
                WikiError::NotFound
            } else if message.contains("already exists") {
                WikiError::Conflict(message)
            } else {
                WikiError::InternalError(message)
            }
        })?;
    search::rebuild_index(&state.wiki_data_dir)
        .map_err(|e| WikiError::InternalError(e.to_string()))?;

    let commit_hash = git::get_current_head(&state.wiki_data_dir)
        .map_err(|e| WikiError::GitError(e.to_string()))?;

    Ok(Json(SaveResponse {
        commit_hash: Some(commit_hash),
        author: Some(current_user.username.clone()),
        message: Some(format!("Restore archived {}", page_path)),
        conflict: Some(false),
        current_content: None,
        their_changes: None,
        base: None,
    }))
}

pub async fn resolve_conflict(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(current_user): axum::extract::Extension<crate::auth::CurrentUser>,
    Json(req): Json<ResolveConflictRequest>,
) -> Result<Json<SaveResponse>, WikiError> {
    if !current_user.can_edit() {
        return Err(WikiError::Unauthorized);
    }

    pages::write_page(&state.wiki_data_dir, &req.path, &req.resolved_content, &current_user.username)
        .map_err(|e| WikiError::InternalError(e.to_string()))?;
    search::rebuild_index(&state.wiki_data_dir)
        .map_err(|e| WikiError::InternalError(e.to_string()))?;

    let commit_hash = git::get_current_head(&state.wiki_data_dir)
        .map_err(|e| WikiError::GitError(e.to_string()))?;
    let conflict_ref = req.conflict_commit_hash.chars().take(8).collect::<String>();
    let message = if conflict_ref.is_empty() {
        format!("Resolve conflict in {}", req.path)
    } else {
        format!("Resolve conflict in {} from {}", req.path, conflict_ref)
    };

    Ok(Json(SaveResponse {
        commit_hash: Some(commit_hash),
        author: Some(current_user.username.clone()),
        message: Some(message),
        conflict: Some(false),
        current_content: None,
        their_changes: None,
        base: None,
    }))
}

pub async fn get_page_at_version(
    State(state): State<Arc<AppState>>,
    axum::extract::Path((page_path, commit_hash)): axum::extract::Path<(String, String)>,
) -> Result<Json<serde_json::Value>, WikiError> {
    let file_path = pages::path_to_file(&state.wiki_data_dir, &page_path)
        .map_err(|e| WikiError::InternalError(e.to_string()))?;

    let raw = git::get_file_at_commit(&state.wiki_data_dir, &file_path, &commit_hash)
        .map_err(|_| WikiError::NotFound)?;

    let rendered = markdown_to_html(&raw, &ComrakOptions::default());

    let diff = git::get_diff_to_current(&state.wiki_data_dir, &file_path, &commit_hash)
        .unwrap_or_default();

    Ok(Json(serde_json::json!({
        "path": page_path,
        "commit_hash": commit_hash,
        "content": rendered,
        "raw": raw,
        "diff": diff,
    })))
}

pub async fn restore_page_version(
    State(state): State<Arc<AppState>>,
    axum::extract::Path((page_path, commit_hash)): axum::extract::Path<(String, String)>,
    axum::extract::Extension(current_user): axum::extract::Extension<crate::auth::CurrentUser>,
) -> Result<Json<SaveResponse>, WikiError> {
    if !current_user.can_edit() {
        return Err(WikiError::Unauthorized);
    }

    let file_path = pages::path_to_file(&state.wiki_data_dir, &page_path)
        .map_err(|e| WikiError::InternalError(e.to_string()))?;

    let content = git::get_file_at_commit(&state.wiki_data_dir, &file_path, &commit_hash)
        .map_err(|_| WikiError::NotFound)?;

    pages::write_page(&state.wiki_data_dir, &page_path, &content, &current_user.username)
        .map_err(|e| WikiError::InternalError(e.to_string()))?;
    search::rebuild_index(&state.wiki_data_dir)
        .map_err(|e| WikiError::InternalError(e.to_string()))?;

    let new_hash = git::get_current_head(&state.wiki_data_dir)
        .map_err(|e| WikiError::GitError(e.to_string()))?;

    Ok(Json(SaveResponse {
        commit_hash: Some(new_hash),
        author: Some(current_user.username.clone()),
        message: Some(format!("Restore {} to {}", page_path, &commit_hash[..8])),
        conflict: Some(false),
        current_content: None,
        their_changes: None,
        base: None,
    }))
}

pub async fn render_markdown(
    Json(req): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, WikiError> {
    let content = req["content"].as_str().unwrap_or("");
    let html = markdown_to_html(content, &ComrakOptions::default());
    Ok(Json(serde_json::json!({ "html": html })))
}

pub async fn search_pages(
    State(state): State<Arc<AppState>>,
    Query(qs): Query<SearchQuery>,
) -> Result<Json<Vec<SearchResult>>, WikiError> {
    search::search(&state.wiki_data_dir, &qs.q)
        .map(Json)
        .map_err(|e| WikiError::InternalError(e.to_string()))
}

pub async fn rebuild_search_index(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(current_user): axum::extract::Extension<crate::auth::CurrentUser>,
) -> Result<Json<serde_json::Value>, WikiError> {
    if !current_user.is_admin() {
        return Err(WikiError::Unauthorized);
    }

    let indexed_pages = search::rebuild_index(&state.wiki_data_dir)
        .map_err(|e| WikiError::InternalError(e.to_string()))?;

    Ok(Json(serde_json::json!({ "indexed_pages": indexed_pages })))
}

pub async fn get_profile(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(current_user): axum::extract::Extension<crate::auth::CurrentUser>,
) -> Result<Json<wiki_server::UserProfileResponse>, WikiError> {
    let users_file = state.wiki_data_dir.join(".users.json");
    let user = auth::find_user(&users_file, &current_user.username)
        .map_err(|e| WikiError::InternalError(e.to_string()))?
        .ok_or(WikiError::NotFound)?;

    Ok(Json(wiki_server::UserProfileResponse {
        username: user.username,
        name: user.name,
        email: user.email,
        description: user.description,
        can_edit: user.role.can_edit(),
        role: user.role,
    }))
}

pub async fn update_profile(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(current_user): axum::extract::Extension<crate::auth::CurrentUser>,
    Json(req): Json<wiki_server::UserProfileUpdateRequest>,
) -> Result<Json<wiki_server::UserProfileResponse>, WikiError> {
    let users_file = state.wiki_data_dir.join(".users.json");
    let user = auth::update_user_profile(
        &users_file,
        &current_user.username,
        &req.name,
        &req.email,
        &req.description,
    ).map_err(|e| WikiError::InternalError(e.to_string()))?;

    Ok(Json(wiki_server::UserProfileResponse {
        username: user.username,
        name: user.name,
        email: user.email,
        description: user.description,
        can_edit: user.role.can_edit(),
        role: user.role,
    }))
}

pub async fn serve_static(req: Request<Body>) -> impl IntoResponse {
    match req.uri().path() {
        "/" | "/index.html" => {
            match tokio::fs::read_to_string("static/index.html").await {
                Ok(content) => (StatusCode::OK, Html(content)).into_response(),
                Err(_) => (StatusCode::NOT_FOUND, "Not found").into_response(),
            }
        }
        path if path.ends_with(".css") => {
            match tokio::fs::read_to_string(format!("static{}", path)).await {
                Ok(content) => (StatusCode::OK, [("content-type", "text/css")], content).into_response(),
                Err(_) => (StatusCode::NOT_FOUND, "Not found").into_response(),
            }
        }
        path if path.ends_with(".js") => {
            match tokio::fs::read_to_string(format!("static{}", path)).await {
                Ok(content) => (StatusCode::OK, [("content-type", "application/javascript")], content).into_response(),
                Err(_) => (StatusCode::NOT_FOUND, "Not found").into_response(),
            }
        }
        _ => (StatusCode::NOT_FOUND, "Not found").into_response(),
    }
}
