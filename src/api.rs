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

#[derive(Deserialize)]
pub struct SavePageRequest {
    pub content: String,
    pub expected_git_head: String,
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

            Ok(Json(PageResponse {
                path: page_path,
                content,
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
    let file_path = pages::path_to_file(&state.wiki_data_dir, &page_path)
        .map_err(|e| WikiError::InternalError(e.to_string()))?;

    // Check if file changed since expected_git_head
    let file_changed = git::file_changed_since_head(&state.wiki_data_dir, &file_path, &req.expected_git_head)
        .map_err(|e| WikiError::GitError(e.to_string()))?;

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

pub async fn resolve_conflict(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ResolveConflictRequest>,
) -> Result<Json<SaveResponse>, WikiError> {
    pages::write_page(&state.wiki_data_dir, &req.path, &req.resolved_content, "unknown")
        .map_err(|e| WikiError::InternalError(e.to_string()))?;

    let commit_hash = git::get_current_head(&state.wiki_data_dir)
        .map_err(|e| WikiError::GitError(e.to_string()))?;

    Ok(Json(SaveResponse {
        commit_hash: Some(commit_hash),
        author: Some("unknown".to_string()),
        message: Some(format!("Resolve conflict in {}", req.path)),
        conflict: Some(false),
        current_content: None,
        their_changes: None,
        base: None,
    }))
}

pub async fn search_pages(
    State(state): State<Arc<AppState>>,
    Query(qs): Query<SearchQuery>,
) -> Result<Json<Vec<SearchResult>>, WikiError> {
    search::search(&state.wiki_data_dir, &qs.q)
        .map(Json)
        .map_err(|e| WikiError::InternalError(e.to_string()))
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
