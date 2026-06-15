// Placeholder - to be implemented in Task 2
use axum::{
    middleware::Next,
    http::Request,
    response::Response,
    extract::State,
    body::Body,
};
use std::sync::Arc;

pub async fn basic_auth_middleware(
    State(_state): State<Arc<wiki_server::AppState>>,
    req: Request<Body>,
    next: Next,
) -> Response {
    // TODO: implement in a later task
    next.run(req).await
}
