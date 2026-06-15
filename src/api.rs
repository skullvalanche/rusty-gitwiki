// Placeholder - to be implemented in Task 6
use axum::extract::{State, Path, Query};
use axum::response::{IntoResponse, Html};
use axum::http::{StatusCode, Request};
use axum::body::Body;
use axum::Json;
use serde::Deserialize;
use std::sync::Arc;
use wiki_server::{AppState, ListPageResponse, PageResponse, SaveResponse, SearchResult, WikiError};

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
    _state: State<Arc<AppState>>,
) -> Result<Json<Vec<ListPageResponse>>, WikiError> {
    Err(WikiError::InternalError("not implemented".into()))
}

pub async fn get_page(
    _state: State<Arc<AppState>>,
    _path: Path<String>,
) -> Result<Json<PageResponse>, WikiError> {
    Err(WikiError::InternalError("not implemented".into()))
}

pub async fn save_page(
    _state: State<Arc<AppState>>,
    _path: Path<String>,
    _req: Json<SavePageRequest>,
) -> Result<Json<SaveResponse>, WikiError> {
    Err(WikiError::InternalError("not implemented".into()))
}

pub async fn resolve_conflict(
    _state: State<Arc<AppState>>,
    _req: Json<ResolveConflictRequest>,
) -> Result<Json<SaveResponse>, WikiError> {
    Err(WikiError::InternalError("not implemented".into()))
}

pub async fn search_pages(
    _state: State<Arc<AppState>>,
    _qs: Query<SearchQuery>,
) -> Result<Json<Vec<SearchResult>>, WikiError> {
    Err(WikiError::InternalError("not implemented".into()))
}

pub async fn serve_static(_req: Request<Body>) -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "Not found")
}
