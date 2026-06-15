mod auth;
mod git;
mod pages;
mod search;
mod api;
mod admin;

use axum::{
    routing::{get, post, delete, put},
    Router,
    extract::State,
    middleware,
};
use std::sync::Arc;
use tower_http::trace::TraceLayer;
use std::path::PathBuf;
use wiki_server::AppState;
use tracing_subscriber;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let wiki_data_dir = std::env::var("WIKI_DATA_DIR")
        .unwrap_or_else(|_| "./wiki_data".to_string());
    let wiki_data_dir = PathBuf::from(&wiki_data_dir);

    // Ensure wiki_data exists and is a git repo
    if !wiki_data_dir.exists() {
        init_wiki_repo(&wiki_data_dir).await?;
    }

    let state = Arc::new(AppState {
        wiki_data_dir: wiki_data_dir.clone(),
    });

    let app = Router::new()
        .route("/api/pages", get(api::list_pages))
        .route("/api/pages/:path", get(api::get_page).post(api::save_page))
        .route("/api/resolve", post(api::resolve_conflict))
        .route("/api/search", get(api::search_pages))
        .route("/api/admin/users", post(admin::create_user))
        .route("/api/admin/users/:user", delete(admin::delete_user))
        .route("/api/admin/users/:user/password", put(admin::set_password))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::basic_auth_middleware,
        ))
        .layer(TraceLayer::new_for_http())
        .fallback(api::serve_static)
        .with_state(state)
        .into_make_service();

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    println!("Wiki server running on http://0.0.0.0:3000");
    axum::serve(listener, app).await?;

    Ok(())
}

async fn init_wiki_repo(wiki_data_dir: &PathBuf) -> anyhow::Result<()> {
    tokio::fs::create_dir_all(wiki_data_dir).await?;
    let status = tokio::process::Command::new("git")
        .arg("init")
        .current_dir(wiki_data_dir)
        .output()
        .await?;
    if !status.status.success() {
        return Err(anyhow::anyhow!("Failed to init git repo"));
    }

    // Create initial .users.json
    let users_file = wiki_data_dir.join(".users.json");
    tokio::fs::write(&users_file, "[]").await?;

    Ok(())
}
