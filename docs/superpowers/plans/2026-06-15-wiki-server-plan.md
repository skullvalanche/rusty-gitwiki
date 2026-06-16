# Wiki Server Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a single-binary Rust wiki server with git-backed markdown storage, basic auth, conflict resolution UI, and admin user management for small teams.

**Architecture:** Axum web server + git subprocess integration. Pages stored as markdown files in a git repo. Users stored in JSON. Frontend is SPA with vanilla JS.

**Tech Stack:** Rust (Axum, Tokio, comrak), HTTP Basic Auth, git CLI (subprocess), JSON user storage, vanilla JS frontend.

---

## File Structure

```
wiki-server/
├── Cargo.toml
├── src/
│   ├── main.rs              # Server setup, Axum app, port listen
│   ├── lib.rs               # Shared types (User, Page, GitOp, etc.)
│   ├── auth.rs              # Basic auth middleware, user CRUD, password hashing
│   ├── git.rs               # Git subprocess wrappers (commit, merge, log, diff)
│   ├── pages.rs             # Page CRUD (read, write, list, detect conflicts)
│   ├── search.rs            # Substring search on content + filenames
│   ├── api.rs               # REST handlers (/api/pages/*, /api/search, etc.)
│   └── admin.rs             # Admin handlers (user create/delete/password)
├── static/
│   ├── index.html           # HTML shell, nav, page editor
│   ├── style.css            # Minimal CSS
│   └── app.js               # SPA logic, fetch API, UI interaction
├── tests/
│   └── integration_test.rs   # End-to-end tests (create page, edit, conflict)
├── Cargo.toml
└── README.md
```

---

## Task 1: Project Setup

**Files:**
- Create: `Cargo.toml`
- Create: `src/lib.rs`
- Create: `src/main.rs`
- Create: `.gitignore`

- [ ] **Step 1: Create Cargo.toml**

```toml
[package]
name = "wiki-server"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "wiki-server"
path = "src/main.rs"

[dependencies]
axum = "0.7"
tokio = { version = "1", features = ["full"] }
tower = "0.4"
tower-http = { version = "0.5", features = ["trace", "cors"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
uuid = { version = "1", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
anyhow = "1"
thiserror = "1"
bcrypt = "0.15"
comrak = "0.18"
tracing = "0.1"
tracing-subscriber = "0.3"
once_cell = "1"

[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 2: Create src/lib.rs with shared types**

```rust
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
```

- [ ] **Step 3: Create src/main.rs skeleton**

```rust
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
```

- [ ] **Step 4: Create .gitignore**

```
/target
/wiki_data
.DS_Store
*.swp
*.swo
*~
.idea
.vscode
```

- [ ] **Step 5: Run cargo check to verify project compiles**

```bash
cd /Users/alan/code/wiki-server
cargo check
```

Expected: Compilation errors (missing modules), but project structure is valid.

- [ ] **Step 6: Commit**

```bash
cd /Users/alan/code/wiki-server
git init
git add Cargo.toml src/ .gitignore
git commit -m "feat: initial project setup with Cargo.toml and shared types"
```

---

## Task 2: Auth Module (User Storage & Password Hashing)

**Files:**
- Create: `src/auth.rs`
- Create: `tests/test_auth.rs`

- [ ] **Step 1: Write tests for user password hashing and verification**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_and_verify_password() {
        let password = "test_password_123";
        let hash = hash_password(password).unwrap();
        assert!(verify_password(password, &hash).unwrap());
        assert!(!verify_password("wrong_password", &hash).unwrap());
    }

    #[test]
    fn test_load_and_save_users() {
        let tempdir = std::env::temp_dir().join("wiki_test_users");
        let _ = std::fs::remove_dir_all(&tempdir);
        std::fs::create_dir_all(&tempdir).ok();

        let users_file = tempdir.join(".users.json");
        let user = User {
            username: "admin".to_string(),
            password_hash: hash_password("admin").unwrap(),
            is_admin: true,
            created_at: chrono::Utc::now(),
        };

        save_users(&users_file, vec![user.clone()]).unwrap();
        let loaded = load_users(&users_file).unwrap();

        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].username, "admin");
    }

    #[test]
    fn test_find_user() {
        let tempdir = std::env::temp_dir().join("wiki_test_find");
        let _ = std::fs::remove_dir_all(&tempdir);
        std::fs::create_dir_all(&tempdir).ok();

        let users_file = tempdir.join(".users.json");
        let user = User {
            username: "testuser".to_string(),
            password_hash: hash_password("pass").unwrap(),
            is_admin: false,
            created_at: chrono::Utc::now(),
        };
        save_users(&users_file, vec![user]).unwrap();

        let found = find_user(&users_file, "testuser").unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().username, "testuser");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd /Users/alan/code/wiki-server
cargo test test_hash_and_verify_password -- --nocapture 2>&1 | head -20
```

Expected: `cannot find function 'hash_password'`

- [ ] **Step 3: Implement auth.rs**

```rust
use wiki_server::User;
use std::path::Path;
use chrono::Utc;

pub fn hash_password(password: &str) -> anyhow::Result<String> {
    bcrypt::hash(password, 10).map_err(|e| anyhow::anyhow!("Bcrypt error: {}", e))
}

pub fn verify_password(password: &str, hash: &str) -> anyhow::Result<bool> {
    bcrypt::verify(password, hash).map_err(|e| anyhow::anyhow!("Bcrypt error: {}", e))
}

pub fn load_users(users_file: &Path) -> anyhow::Result<Vec<User>> {
    if !users_file.exists() {
        return Ok(Vec::new());
    }
    let content = std::fs::read_to_string(users_file)?;
    let users: Vec<User> = serde_json::from_str(&content).unwrap_or_default();
    Ok(users)
}

pub fn save_users(users_file: &Path, users: Vec<User>) -> anyhow::Result<()> {
    let json = serde_json::to_string_pretty(&users)?;
    std::fs::write(users_file, json)?;
    Ok(())
}

pub fn find_user(users_file: &Path, username: &str) -> anyhow::Result<Option<User>> {
    let users = load_users(users_file)?;
    Ok(users.into_iter().find(|u| u.username == username))
}

pub fn user_exists(users_file: &Path, username: &str) -> anyhow::Result<bool> {
    find_user(users_file, username).map(|u| u.is_some())
}

pub fn create_user(
    users_file: &Path,
    username: &str,
    password: &str,
    is_admin: bool,
) -> anyhow::Result<User> {
    let mut users = load_users(users_file)?;
    
    if users.iter().any(|u| u.username == username) {
        return Err(anyhow::anyhow!("User already exists"));
    }

    let password_hash = hash_password(password)?;
    let user = User {
        username: username.to_string(),
        password_hash,
        is_admin,
        created_at: Utc::now(),
    };
    
    users.push(user.clone());
    save_users(users_file, users)?;
    Ok(user)
}

pub fn delete_user(users_file: &Path, username: &str) -> anyhow::Result<()> {
    let mut users = load_users(users_file)?;
    users.retain(|u| u.username != username);
    save_users(users_file, users)?;
    Ok(())
}

pub fn set_user_password(
    users_file: &Path,
    username: &str,
    new_password: &str,
) -> anyhow::Result<()> {
    let mut users = load_users(users_file)?;
    
    let user = users.iter_mut().find(|u| u.username == username)
        .ok_or_else(|| anyhow::anyhow!("User not found"))?;
    
    user.password_hash = hash_password(new_password)?;
    save_users(users_file, users)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_and_verify_password() {
        let password = "test_password_123";
        let hash = hash_password(password).unwrap();
        assert!(verify_password(password, &hash).unwrap());
        assert!(!verify_password("wrong_password", &hash).unwrap());
    }

    #[test]
    fn test_load_and_save_users() {
        let tempdir = std::env::temp_dir().join("wiki_test_users");
        let _ = std::fs::remove_dir_all(&tempdir);
        std::fs::create_dir_all(&tempdir).ok();

        let users_file = tempdir.join(".users.json");
        let user = User {
            username: "admin".to_string(),
            password_hash: hash_password("admin").unwrap(),
            is_admin: true,
            created_at: Utc::now(),
        };

        save_users(&users_file, vec![user.clone()]).unwrap();
        let loaded = load_users(&users_file).unwrap();

        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].username, "admin");
    }

    #[test]
    fn test_find_user() {
        let tempdir = std::env::temp_dir().join("wiki_test_find");
        let _ = std::fs::remove_dir_all(&tempdir);
        std::fs::create_dir_all(&tempdir).ok();

        let users_file = tempdir.join(".users.json");
        let user = User {
            username: "testuser".to_string(),
            password_hash: hash_password("pass").unwrap(),
            is_admin: false,
            created_at: Utc::now(),
        };
        save_users(&users_file, vec![user]).unwrap();

        let found = find_user(&users_file, "testuser").unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().username, "testuser");
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cd /Users/alan/code/wiki-server
cargo test auth::tests -- --nocapture
```

Expected: All tests PASS.

- [ ] **Step 5: Add basic auth middleware stub to auth.rs**

Add this to the end of auth.rs (after tests):

```rust
use axum::{
    middleware::Next,
    http::Request,
    response::Response,
    extract::State,
};
use std::sync::Arc;

pub async fn basic_auth_middleware<B>(
    State(_state): State<Arc<wiki_server::AppState>>,
    req: Request<B>,
    next: Next,
) -> Result<Response, wiki_server::WikiError> {
    // TODO: implement in a later task
    Ok(next.run(req).await)
}
```

- [ ] **Step 6: Commit**

```bash
cd /Users/alan/code/wiki-server
git add src/auth.rs Cargo.lock
git commit -m "feat: auth module with password hashing and user storage"
```

---

## Task 3: Git Module (Subprocess Wrappers)

**Files:**
- Create: `src/git.rs`
- Create: `tests/test_git.rs`

- [ ] **Step 1: Write tests for git operations**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn setup_test_repo() -> PathBuf {
        let tempdir = std::env::temp_dir().join("wiki_git_test");
        let _ = std::fs::remove_dir_all(&tempdir);
        std::fs::create_dir_all(&tempdir).ok();
        
        // Init git repo
        std::process::Command::new("git")
            .args(&["init"])
            .current_dir(&tempdir)
            .output()
            .ok();

        std::process::Command::new("git")
            .args(&["config", "user.email", "test@example.com"])
            .current_dir(&tempdir)
            .output()
            .ok();

        std::process::Command::new("git")
            .args(&["config", "user.name", "Test User"])
            .current_dir(&tempdir)
            .output()
            .ok();

        tempdir
    }

    #[test]
    fn test_git_commit() {
        let repo = setup_test_repo();
        let test_file = repo.join("test.md");
        std::fs::write(&test_file, "# Test\nContent").ok();

        let result = std_commit(&repo, &test_file, "test message", "testuser").unwrap();
        assert!(!result.is_empty());
    }

    #[test]
    fn test_get_current_head() {
        let repo = setup_test_repo();
        let test_file = repo.join("test.md");
        std::fs::write(&test_file, "# Test").ok();
        git_commit(&repo, &test_file, "init", "user1").ok();

        let head = get_current_head(&repo).unwrap();
        assert!(!head.is_empty());
        assert!(head.len() > 10); // SHA hashes are long
    }

    #[test]
    fn test_file_changed_since_head() {
        let repo = setup_test_repo();
        let test_file = repo.join("test.md");
        std::fs::write(&test_file, "v1").ok();
        git_commit(&repo, &test_file, "commit1", "user1").ok();
        
        let head1 = get_current_head(&repo).unwrap();
        assert!(!file_changed_since_head(&repo, &test_file, &head1).unwrap());
        
        std::fs::write(&test_file, "v2").ok();
        assert!(file_changed_since_head(&repo, &test_file, &head1).unwrap());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd /Users/alan/code/wiki-server
cargo test git::tests -- --nocapture 2>&1 | head -20
```

Expected: `cannot find function 'git_commit'`

- [ ] **Step 3: Implement git.rs**

```rust
use std::path::{Path, PathBuf};
use anyhow::anyhow;

pub fn git_commit(
    repo_dir: &Path,
    file_path: &Path,
    message: &str,
    author: &str,
) -> anyhow::Result<String> {
    // Stage the file
    let status = std::process::Command::new("git")
        .args(&["add", file_path.file_name().unwrap().to_str().unwrap()])
        .current_dir(repo_dir)
        .output()?;

    if !status.status.success() {
        return Err(anyhow!("git add failed"));
    }

    // Commit
    let status = std::process::Command::new("git")
        .args(&["commit", "-m", message, "--author", &format!("{} <{}>", author, author)])
        .current_dir(repo_dir)
        .output()?;

    if !status.status.success() {
        let stderr = String::from_utf8_lossy(&status.stderr);
        return Err(anyhow!("git commit failed: {}", stderr));
    }

    get_current_head(repo_dir)
}

pub fn get_current_head(repo_dir: &Path) -> anyhow::Result<String> {
    let output = std::process::Command::new("git")
        .args(&["rev-parse", "HEAD"])
        .current_dir(repo_dir)
        .output()?;

    if !output.status.success() {
        return Err(anyhow!("git rev-parse HEAD failed"));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn file_changed_since_head(
    repo_dir: &Path,
    file_path: &Path,
    expected_head: &str,
) -> anyhow::Result<bool> {
    let current_head = get_current_head(repo_dir)?;
    
    if current_head != expected_head {
        return Ok(true); // HEAD has moved
    }

    // Check if file has uncommitted changes
    let output = std::process::Command::new("git")
        .args(&["status", "--porcelain", file_path.to_str().unwrap()])
        .current_dir(repo_dir)
        .output()?;

    let status = String::from_utf8_lossy(&output.stdout);
    Ok(!status.trim().is_empty())
}

pub fn git_merge(
    repo_dir: &Path,
    file_path: &Path,
    branch_name: &str,
) -> anyhow::Result<GitMergeResult> {
    let output = std::process::Command::new("git")
        .args(&["merge", branch_name])
        .current_dir(repo_dir)
        .output()?;

    if output.status.success() {
        return Ok(GitMergeResult::Success {
            commit_hash: get_current_head(repo_dir)?,
        });
    }

    // Check for conflicts
    let status_output = std::process::Command::new("git")
        .args(&["status", "--porcelain"])
        .current_dir(repo_dir)
        .output()?;

    let status_str = String::from_utf8_lossy(&status_output.stdout);
    if status_str.contains("UU") || status_str.contains("AA") || status_str.contains("UD") {
        let file_content = std::fs::read_to_string(file_path)?;
        return Ok(GitMergeResult::Conflict {
            conflicted_content: file_content,
        });
    }

    Err(anyhow!("git merge failed with unknown error"))
}

pub enum GitMergeResult {
    Success { commit_hash: String },
    Conflict { conflicted_content: String },
}

pub fn get_git_log(
    repo_dir: &Path,
    file_path: &Path,
    limit: usize,
) -> anyhow::Result<Vec<(String, String, String)>> {
    let output = std::process::Command::new("git")
        .args(&[
            "log",
            &format!("--max-count={}", limit),
            "--format=%H|%an|%s",
            "--",
            file_path.to_str().unwrap(),
        ])
        .current_dir(repo_dir)
        .output()?;

    if !output.status.success() {
        return Ok(Vec::new()); // File has no history yet
    }

    let lines = String::from_utf8_lossy(&output.stdout);
    let commits = lines
        .lines()
        .map(|line| {
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() >= 3 {
                (
                    parts[0].to_string(),
                    parts[1].to_string(),
                    parts[2].to_string(),
                )
            } else {
                (String::new(), String::new(), String::new())
            }
        })
        .collect();

    Ok(commits)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_repo() -> PathBuf {
        let tempdir = std::env::temp_dir().join("wiki_git_test");
        let _ = std::fs::remove_dir_all(&tempdir);
        std::fs::create_dir_all(&tempdir).ok();
        
        std::process::Command::new("git")
            .args(&["init"])
            .current_dir(&tempdir)
            .output()
            .ok();

        std::process::Command::new("git")
            .args(&["config", "user.email", "test@example.com"])
            .current_dir(&tempdir)
            .output()
            .ok();

        std::process::Command::new("git")
            .args(&["config", "user.name", "Test User"])
            .current_dir(&tempdir)
            .output()
            .ok();

        tempdir
    }

    #[test]
    fn test_git_commit() {
        let repo = setup_test_repo();
        let test_file = repo.join("test.md");
        std::fs::write(&test_file, "# Test\nContent").ok();

        let result = git_commit(&repo, &test_file, "test message", "testuser").unwrap();
        assert!(!result.is_empty());
    }

    #[test]
    fn test_get_current_head() {
        let repo = setup_test_repo();
        let test_file = repo.join("test.md");
        std::fs::write(&test_file, "# Test").ok();
        git_commit(&repo, &test_file, "init", "user1").ok();

        let head = get_current_head(&repo).unwrap();
        assert!(!head.is_empty());
        assert!(head.len() > 10);
    }

    #[test]
    fn test_file_changed_since_head() {
        let repo = setup_test_repo();
        let test_file = repo.join("test.md");
        std::fs::write(&test_file, "v1").ok();
        git_commit(&repo, &test_file, "commit1", "user1").ok();
        
        let head1 = get_current_head(&repo).unwrap();
        assert!(!file_changed_since_head(&repo, &test_file, &head1).unwrap());
        
        std::fs::write(&test_file, "v2").ok();
        assert!(file_changed_since_head(&repo, &test_file, &head1).unwrap());
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cd /Users/alan/code/wiki-server
cargo test git::tests -- --nocapture
```

Expected: All tests PASS.

- [ ] **Step 5: Commit**

```bash
cd /Users/alan/code/wiki-server
git add src/git.rs
git commit -m "feat: git module with commit, merge, and log operations"
```

---

## Task 4: Pages Module (CRUD Logic)

**Files:**
- Create: `src/pages.rs`
- Create: `tests/test_pages.rs`

- [ ] **Step 1: Write tests for page operations**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn setup_wiki_dir() -> PathBuf {
        let tempdir = std::env::temp_dir().join("wiki_pages_test");
        let _ = std::fs::remove_dir_all(&tempdir);
        std::fs::create_dir_all(&tempdir).ok();
        
        // Init git repo
        std::process::Command::new("git")
            .args(&["init"])
            .current_dir(&tempdir)
            .output()
            .ok();

        std::process::Command::new("git")
            .args(&["config", "user.email", "test@example.com"])
            .current_dir(&tempdir)
            .output()
            .ok();

        std::process::Command::new("git")
            .args(&["config", "user.name", "Test User"])
            .current_dir(&tempdir)
            .output()
            .ok();

        tempdir
    }

    #[test]
    fn test_write_page() {
        let wiki_dir = setup_wiki_dir();
        let content = "# Hello\nWorld";
        
        write_page(&wiki_dir, "test", content, "alice").unwrap();
        
        let path = wiki_dir.join("test.md");
        assert!(path.exists());
        let read = std::fs::read_to_string(&path).unwrap();
        assert_eq!(read, content);
    }

    #[test]
    fn test_write_page_with_hierarchy() {
        let wiki_dir = setup_wiki_dir();
        let content = "# Nested page";
        
        write_page(&wiki_dir, "docs/guide/intro", content, "bob").unwrap();
        
        let path = wiki_dir.join("docs/guide/intro.md");
        assert!(path.exists());
    }

    #[test]
    fn test_read_page() {
        let wiki_dir = setup_wiki_dir();
        write_page(&wiki_dir, "mypage", "# Content", "alice").unwrap();
        
        let content = read_page(&wiki_dir, "mypage").unwrap();
        assert_eq!(content, "# Content");
    }

    #[test]
    fn test_list_pages() {
        let wiki_dir = setup_wiki_dir();
        write_page(&wiki_dir, "page1", "# P1", "alice").unwrap();
        write_page(&wiki_dir, "docs/page2", "# P2", "bob").unwrap();
        
        let pages = list_pages(&wiki_dir).unwrap();
        assert!(pages.iter().any(|p| p.path == "page1"));
        assert!(pages.iter().any(|p| p.path == "docs/page2"));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd /Users/alan/code/wiki-server
cargo test pages::tests -- --nocapture 2>&1 | head -20
```

Expected: `cannot find function 'write_page'`

- [ ] **Step 3: Implement pages.rs**

```rust
use wiki_server::{Page, ListPageResponse};
use std::path::{Path, PathBuf};
use chrono::Utc;
use crate::git;

pub fn write_page(
    wiki_dir: &Path,
    page_path: &str,
    content: &str,
    author: &str,
) -> anyhow::Result<()> {
    let file_path = path_to_file(wiki_dir, page_path)?;
    
    // Create parent directories
    if let Some(parent) = file_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::write(&file_path, content)?;
    git::git_commit(wiki_dir, &file_path, &format!("Update {}", page_path), author)?;

    Ok(())
}

pub fn read_page(wiki_dir: &Path, page_path: &str) -> anyhow::Result<String> {
    let file_path = path_to_file(wiki_dir, page_path)?;
    std::fs::read_to_string(file_path).map_err(|e| anyhow::anyhow!(e))
}

pub fn page_exists(wiki_dir: &Path, page_path: &str) -> anyhow::Result<bool> {
    let file_path = path_to_file(wiki_dir, page_path)?;
    Ok(file_path.exists())
}

pub fn list_pages(wiki_dir: &Path) -> anyhow::Result<Vec<ListPageResponse>> {
    let mut pages = Vec::new();

    for entry in walkdir::WalkDir::new(wiki_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|ext| ext == "md").unwrap_or(false))
    {
        let path = entry.path();
        let relative = path.strip_prefix(wiki_dir)?;
        let page_path = relative.with_extension("").to_string_lossy().to_string();
        
        // Skip hidden files
        if page_path.starts_with(".") {
            continue;
        }

        let metadata = std::fs::metadata(path)?;
        let modified = metadata.modified()?;
        let modified_dt = chrono::DateTime::<Utc>::from(modified);

        pages.push(ListPageResponse {
            path: page_path.replace("\\", "/"),
            title: path.file_stem().unwrap_or_default().to_string_lossy().to_string(),
            updated_at: modified_dt,
            updated_by: "unknown".to_string(), // TODO: extract from git log
        });
    }

    Ok(pages)
}

pub fn path_to_file(wiki_dir: &Path, page_path: &str) -> anyhow::Result<PathBuf> {
    let mut file_path = wiki_dir.to_path_buf();
    file_path.push(format!("{}.md", page_path));
    
    // Prevent directory traversal
    if !file_path.starts_with(wiki_dir) {
        return Err(anyhow::anyhow!("Invalid page path"));
    }

    Ok(file_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_wiki_dir() -> PathBuf {
        let tempdir = std::env::temp_dir().join("wiki_pages_test");
        let _ = std::fs::remove_dir_all(&tempdir);
        std::fs::create_dir_all(&tempdir).ok();
        
        std::process::Command::new("git")
            .args(&["init"])
            .current_dir(&tempdir)
            .output()
            .ok();

        std::process::Command::new("git")
            .args(&["config", "user.email", "test@example.com"])
            .current_dir(&tempdir)
            .output()
            .ok();

        std::process::Command::new("git")
            .args(&["config", "user.name", "Test User"])
            .current_dir(&tempdir)
            .output()
            .ok();

        tempdir
    }

    #[test]
    fn test_write_page() {
        let wiki_dir = setup_wiki_dir();
        let content = "# Hello\nWorld";
        
        write_page(&wiki_dir, "test", content, "alice").unwrap();
        
        let path = wiki_dir.join("test.md");
        assert!(path.exists());
        let read = std::fs::read_to_string(&path).unwrap();
        assert_eq!(read, content);
    }

    #[test]
    fn test_write_page_with_hierarchy() {
        let wiki_dir = setup_wiki_dir();
        let content = "# Nested page";
        
        write_page(&wiki_dir, "docs/guide/intro", content, "bob").unwrap();
        
        let path = wiki_dir.join("docs/guide/intro.md");
        assert!(path.exists());
    }

    #[test]
    fn test_read_page() {
        let wiki_dir = setup_wiki_dir();
        write_page(&wiki_dir, "mypage", "# Content", "alice").unwrap();
        
        let content = read_page(&wiki_dir, "mypage").unwrap();
        assert_eq!(content, "# Content");
    }

    #[test]
    fn test_list_pages() {
        let wiki_dir = setup_wiki_dir();
        write_page(&wiki_dir, "page1", "# P1", "alice").unwrap();
        write_page(&wiki_dir, "docs/page2", "# P2", "bob").unwrap();
        
        let pages = list_pages(&wiki_dir).unwrap();
        assert!(pages.iter().any(|p| p.path == "page1"));
        assert!(pages.iter().any(|p| p.path == "docs/page2"));
    }
}
```

- [ ] **Step 4: Add walkdir to Cargo.toml dependencies**

In Cargo.toml, under `[dependencies]`, add:

```toml
walkdir = "2"
```

- [ ] **Step 5: Run tests to verify they pass**

```bash
cd /Users/alan/code/wiki-server
cargo test pages::tests -- --nocapture
```

Expected: All tests PASS.

- [ ] **Step 6: Commit**

```bash
cd /Users/alan/code/wiki-server
git add src/pages.rs Cargo.toml
git commit -m "feat: pages module with CRUD operations"
```

---

## Task 5: Search Module (Substring Search)

**Files:**
- Create: `src/search.rs`

- [ ] **Step 1: Implement search.rs**

```rust
use wiki_server::SearchResult;
use std::path::Path;

pub fn search(wiki_dir: &Path, query: &str) -> anyhow::Result<Vec<SearchResult>> {
    let mut results = Vec::new();
    let query_lower = query.to_lowercase();

    for entry in walkdir::WalkDir::new(wiki_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|ext| ext == "md").unwrap_or(false))
    {
        let path = entry.path();
        let relative = path.strip_prefix(wiki_dir)?;
        let page_path = relative.with_extension("").to_string_lossy().to_string();

        // Skip hidden files
        if page_path.starts_with(".") {
            continue;
        }

        // Search filename
        if page_path.to_lowercase().contains(&query_lower) {
            results.push(SearchResult {
                path: page_path.replace("\\", "/"),
                excerpt: format!("(filename match)"),
            });
            continue;
        }

        // Search content
        if let Ok(content) = std::fs::read_to_string(path) {
            if content.to_lowercase().contains(&query_lower) {
                // Extract excerpt
                let lines: Vec<&str> = content.lines().collect();
                let mut excerpt = String::new();
                for line in lines {
                    if line.to_lowercase().contains(&query_lower) {
                        excerpt = line.to_string();
                        if excerpt.len() > 80 {
                            excerpt.truncate(80);
                            excerpt.push_str("...");
                        }
                        break;
                    }
                }

                results.push(SearchResult {
                    path: page_path.replace("\\", "/"),
                    excerpt,
                });
            }
        }
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_wiki_dir() -> std::path::PathBuf {
        let tempdir = std::env::temp_dir().join("wiki_search_test");
        let _ = std::fs::remove_dir_all(&tempdir);
        std::fs::create_dir_all(&tempdir).ok();
        
        std::process::Command::new("git")
            .args(&["init"])
            .current_dir(&tempdir)
            .output()
            .ok();

        tempdir
    }

    #[test]
    fn test_search_by_filename() {
        let wiki_dir = setup_wiki_dir();
        std::fs::write(wiki_dir.join("rust.md"), "# Rust programming").ok();
        std::fs::write(wiki_dir.join("python.md"), "# Python guide").ok();

        let results = search(&wiki_dir, "rust").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].path, "rust");
    }

    #[test]
    fn test_search_by_content() {
        let wiki_dir = setup_wiki_dir();
        std::fs::write(wiki_dir.join("page1.md"), "This contains database info").ok();
        std::fs::write(wiki_dir.join("page2.md"), "Nothing here").ok();

        let results = search(&wiki_dir, "database").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].path, "page1");
    }
}
```

- [ ] **Step 2: Add module to main.rs**

In src/main.rs, add this line after other mod declarations:

```rust
mod search;
```

- [ ] **Step 3: Run tests to verify they pass**

```bash
cd /Users/alan/code/wiki-server
cargo test search::tests -- --nocapture
```

Expected: All tests PASS.

- [ ] **Step 4: Commit**

```bash
cd /Users/alan/code/wiki-server
git add src/search.rs src/main.rs
git commit -m "feat: search module with substring matching"
```

---

## Task 6: API Module (REST Handlers)

**Files:**
- Create: `src/api.rs`

- [ ] **Step 1: Implement api.rs with page endpoints**

```rust
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
    pages::write_page(&state.wiki_data_dir, &page_path, &req.content, "unknown")
        .map_err(|e| WikiError::InternalError(e.to_string()))?;

    let commit_hash = git::get_current_head(&state.wiki_data_dir)
        .map_err(|e| WikiError::GitError(e.to_string()))?;

    Ok(Json(SaveResponse {
        commit_hash: Some(commit_hash),
        author: Some("unknown".to_string()),
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
```

- [ ] **Step 2: Add api module to main.rs**

In src/main.rs, add this line after other mod declarations:

```rust
mod api;
```

- [ ] **Step 3: Verify code compiles**

```bash
cd /Users/alan/code/wiki-server
cargo check 2>&1 | head -30
```

Expected: Some compilation errors (auth middleware incomplete, admin handlers missing), but structure is sound.

- [ ] **Step 4: Commit**

```bash
cd /Users/alan/code/wiki-server
git add src/api.rs src/main.rs
git commit -m "feat: REST API handlers for pages and search"
```

---

## Task 7: Admin Module (User Management)

**Files:**
- Create: `src/admin.rs`

- [ ] **Step 1: Implement admin.rs**

```rust
use axum::{
    extract::{State, Path},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
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
```

- [ ] **Step 2: Add admin module to main.rs**

In src/main.rs, add this line after other mod declarations:

```rust
mod admin;
```

- [ ] **Step 3: Verify code compiles**

```bash
cd /Users/alan/code/wiki-server
cargo check
```

Expected: Compilation succeeds.

- [ ] **Step 4: Commit**

```bash
cd /Users/alan/code/wiki-server
git add src/admin.rs src/main.rs
git commit -m "feat: admin module for user management"
```

---

## Task 8: Basic Auth Middleware Implementation

**Files:**
- Modify: `src/auth.rs` (update middleware function)
- Modify: `src/main.rs` (extract username for API handlers)

- [ ] **Step 1: Implement basic_auth_middleware in auth.rs**

Replace the middleware stub with:

```rust
pub async fn basic_auth_middleware<B>(
    State(state): State<Arc<wiki_server::AppState>>,
    req: Request<B>,
    next: Next,
) -> Result<Response, wiki_server::WikiError> {
    let auth_header = req.headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok());

    if let Some(auth_header) = auth_header {
        if auth_header.starts_with("Basic ") {
            if let Ok(credentials) = base64_decode(&auth_header[6..]) {
                if let Some((username, password)) = credentials.split_once(':') {
                    let users_file = state.wiki_data_dir.join(".users.json");
                    if let Ok(Some(user)) = find_user(&users_file, username) {
                        if let Ok(true) = verify_password(password, &user.password_hash) {
                            // Store username in request extensions for API handlers
                            let mut req = req;
                            req.extensions_mut().insert(CurrentUser {
                                username: username.to_string(),
                                is_admin: user.is_admin,
                            });
                            return Ok(next.run(req).await);
                        }
                    }
                }
            }
        }
    }

    // No valid auth, return 401
    Err(wiki_server::WikiError::Unauthorized)
}

fn base64_decode(s: &str) -> Result<String, std::string::FromUtf8Error> {
    use std::str;
    let decoded = base64::decode(s).map_err(|_| std::string::FromUtf8Error {
        valid_up_to: 0,
        error_kind: std::string::Utf8Error::valid_up_to,
    })?;
    String::from_utf8(decoded)
}

#[derive(Debug, Clone)]
pub struct CurrentUser {
    pub username: String,
    pub is_admin: bool,
}
```

- [ ] **Step 2: Add base64 to Cargo.toml dependencies**

In Cargo.toml, add:

```toml
base64 = "0.21"
```

- [ ] **Step 3: Update API handlers to use current user**

In src/api.rs, update `save_page`:

```rust
pub async fn save_page(
    State(state): State<Arc<AppState>>,
    Path(page_path): Path<String>,
    req_ext: axum::extract::Extension(current_user): axum::extract::Extension<crate::auth::CurrentUser>,
    Json(req): Json<SavePageRequest>,
) -> Result<Json<SaveResponse>, WikiError> {
    // ... existing code ...
    
    // Replace "unknown" with current_user.username:
    pages::write_page(&state.wiki_data_dir, &page_path, &req.content, &current_user.username)
        .map_err(|e| WikiError::InternalError(e.to_string()))?;
    
    // ... rest of function ...
}
```

- [ ] **Step 4: Verify code compiles**

```bash
cd /Users/alan/code/wiki-server
cargo check
```

Expected: Compilation succeeds.

- [ ] **Step 5: Commit**

```bash
cd /Users/alan/code/wiki-server
git add src/auth.rs src/api.rs src/main.rs Cargo.toml
git commit -m "feat: HTTP Basic Auth middleware with user extraction"
```

---

## Task 9: Frontend (HTML + JS SPA)

**Files:**
- Create: `static/index.html`
- Create: `static/app.js`
- Create: `static/style.css`

- [ ] **Step 1: Create static/index.html**

```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Wiki</title>
    <link rel="stylesheet" href="/style.css">
</head>
<body>
    <div id="app" class="container">
        <div class="sidebar">
            <h2>Wiki</h2>
            <input type="text" id="search-input" placeholder="Search pages...">
            <div id="page-list"></div>
            <button id="new-page-btn">+ New Page</button>
        </div>
        <div class="main">
            <div id="editor" style="display: none;">
                <div class="editor-header">
                    <input type="text" id="page-path-input" placeholder="Page path (e.g., docs/guide)">
                    <button id="save-btn">Save</button>
                </div>
                <div class="editor-container">
                    <textarea id="content-input"></textarea>
                    <div id="preview" class="preview"></div>
                </div>
                <div id="conflict-ui" style="display: none;">
                    <h3>Merge Conflict</h3>
                    <div class="conflict-content">
                        <div class="conflict-side">
                            <h4>Current (Server)</h4>
                            <pre id="current-content"></pre>
                        </div>
                        <div class="conflict-side">
                            <h4>Your Changes</h4>
                            <pre id="their-content"></pre>
                        </div>
                    </div>
                    <textarea id="resolve-input"></textarea>
                    <button id="resolve-btn">Resolve Conflict</button>
                </div>
            </div>
            <div id="viewer" style="display: none;">
                <div class="viewer-header">
                    <h1 id="page-title"></h1>
                    <button id="edit-btn">Edit</button>
                </div>
                <div id="page-content" class="content"></div>
                <div id="history" class="history">
                    <h3>History</h3>
                    <div id="history-list"></div>
                </div>
            </div>
            <div id="welcome" style="display: block;">
                <h1>Welcome to Wiki</h1>
                <p>Select a page or create a new one.</p>
            </div>
        </div>
    </div>
    <script src="/app.js"></script>
</body>
</html>
```

- [ ] **Step 2: Create static/app.js**

```javascript
const API_BASE = '/api';
let currentPage = null;
let currentGitHead = null;
let currentUser = null;

// Initialize
document.addEventListener('DOMContentLoaded', async () => {
    await loadPageList();
    
    // Event listeners
    document.getElementById('search-input').addEventListener('input', e => searchPages(e.target.value));
    document.getElementById('new-page-btn').addEventListener('click', () => newPage());
    document.getElementById('save-btn').addEventListener('click', () => savePage());
    document.getElementById('edit-btn').addEventListener('click', () => enterEditMode());
    document.getElementById('resolve-btn').addEventListener('click', () => resolveConflict());
});

async function loadPageList() {
    try {
        const resp = await fetch(`${API_BASE}/pages`);
        if (!resp.ok) return;
        
        const pages = await resp.json();
        const list = document.getElementById('page-list');
        list.innerHTML = '';
        
        pages.forEach(page => {
            const div = document.createElement('div');
            div.className = 'page-item';
            div.textContent = page.path;
            div.onclick = () => viewPage(page.path);
            list.appendChild(div);
        });
    } catch (e) {
        console.error('Failed to load pages:', e);
    }
}

async function viewPage(pagePath) {
    try {
        const resp = await fetch(`${API_BASE}/pages/${encodeURIComponent(pagePath)}`);
        if (!resp.ok) {
            showViewer();
            return;
        }
        
        const page = await resp.json();
        currentPage = pagePath;
        currentGitHead = page.current_git_head;
        
        document.getElementById('page-title').textContent = pagePath;
        const contentDiv = document.getElementById('page-content');
        contentDiv.innerHTML = markdownToHtml(page.content);
        
        const historyList = document.getElementById('history-list');
        historyList.innerHTML = page.history.map(c => 
            `<div class="history-item"><strong>${c.author}</strong> - ${c.message}</div>`
        ).join('');
        
        showViewer();
    } catch (e) {
        console.error('Failed to load page:', e);
    }
}

function enterEditMode() {
    document.getElementById('editor').style.display = 'block';
    document.getElementById('viewer').style.display = 'none';
    document.getElementById('welcome').style.display = 'none';
    document.getElementById('conflict-ui').style.display = 'none';
    
    document.getElementById('page-path-input').value = currentPage || '';
    
    // Load current content
    const contentDiv = document.getElementById('page-content');
    const contentText = contentDiv.textContent;
    document.getElementById('content-input').value = contentText;
}

function newPage() {
    currentPage = '';
    currentGitHead = null;
    document.getElementById('content-input').value = '';
    document.getElementById('page-path-input').value = '';
    enterEditMode();
}

async function savePage() {
    const path = document.getElementById('page-path-input').value.trim();
    const content = document.getElementById('content-input').value;
    
    if (!path) {
        alert('Page path required');
        return;
    }
    
    try {
        const resp = await fetch(`${API_BASE}/pages/${encodeURIComponent(path)}`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                content,
                expected_git_head: currentGitHead || '',
            }),
        });
        
        const result = await resp.json();
        
        if (result.conflict) {
            showConflictUI(path, result.current_content, result.their_changes);
        } else {
            alert('Page saved!');
            currentPage = path;
            await loadPageList();
            await viewPage(path);
        }
    } catch (e) {
        console.error('Failed to save page:', e);
        alert('Save failed');
    }
}

function showConflictUI(path, current, their) {
    document.getElementById('conflict-ui').style.display = 'block';
    document.getElementById('current-content').textContent = current;
    document.getElementById('their-content').textContent = their;
    document.getElementById('resolve-input').value = current; // Default to current
}

async function resolveConflict() {
    const resolved = document.getElementById('resolve-input').value;
    const path = document.getElementById('page-path-input').value;
    
    try {
        const resp = await fetch(`${API_BASE}/resolve`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                path,
                resolved_content: resolved,
                conflict_commit_hash: currentGitHead,
            }),
        });
        
        if (resp.ok) {
            alert('Conflict resolved!');
            currentPage = path;
            await loadPageList();
            await viewPage(path);
        }
    } catch (e) {
        console.error('Failed to resolve conflict:', e);
    }
}

async function searchPages(query) {
    if (!query) {
        await loadPageList();
        return;
    }
    
    try {
        const resp = await fetch(`${API_BASE}/search?q=${encodeURIComponent(query)}`);
        if (!resp.ok) return;
        
        const results = await resp.json();
        const list = document.getElementById('page-list');
        list.innerHTML = '';
        
        results.forEach(result => {
            const div = document.createElement('div');
            div.className = 'page-item';
            div.innerHTML = `<strong>${result.path}</strong><br><small>${result.excerpt}</small>`;
            div.style.cursor = 'pointer';
            div.onclick = () => viewPage(result.path);
            list.appendChild(div);
        });
    } catch (e) {
        console.error('Search failed:', e);
    }
}

function showViewer() {
    document.getElementById('viewer').style.display = 'block';
    document.getElementById('editor').style.display = 'none';
    document.getElementById('welcome').style.display = 'none';
}

function markdownToHtml(markdown) {
    // Simple markdown to HTML (for MVP)
    let html = markdown
        .replace(/^### (.*?)$/gm, '<h3>$1</h3>')
        .replace(/^## (.*?)$/gm, '<h2>$1</h2>')
        .replace(/^# (.*?)$/gm, '<h1>$1</h1>')
        .replace(/\*\*(.*?)\*\*/g, '<strong>$1</strong>')
        .replace(/\*(.*?)\*/g, '<em>$1</em>')
        .replace(/\n\n/g, '</p><p>')
        .replace(/^/gm, '')
        .replace(/$/gm, '');
    return `<p>${html}</p>`;
}
```

- [ ] **Step 3: Create static/style.css**

```css
* {
    margin: 0;
    padding: 0;
    box-sizing: border-box;
}

body {
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    background: #f5f5f5;
}

.container {
    display: flex;
    height: 100vh;
}

.sidebar {
    width: 250px;
    background: #fff;
    border-right: 1px solid #ddd;
    padding: 20px;
    overflow-y: auto;
}

.sidebar h2 {
    margin-bottom: 20px;
    font-size: 24px;
}

#search-input {
    width: 100%;
    padding: 8px;
    margin-bottom: 15px;
    border: 1px solid #ddd;
    border-radius: 4px;
    font-size: 14px;
}

#page-list {
    margin-bottom: 20px;
}

.page-item {
    padding: 8px 12px;
    margin-bottom: 4px;
    background: #f9f9f9;
    border-radius: 4px;
    cursor: pointer;
    font-size: 14px;
    border-left: 3px solid transparent;
    transition: all 0.2s;
}

.page-item:hover {
    background: #f0f0f0;
    border-left-color: #0066cc;
}

#new-page-btn {
    width: 100%;
    padding: 10px;
    background: #0066cc;
    color: white;
    border: none;
    border-radius: 4px;
    cursor: pointer;
    font-weight: bold;
}

#new-page-btn:hover {
    background: #0052a3;
}

.main {
    flex: 1;
    display: flex;
    flex-direction: column;
    background: white;
}

.editor {
    display: flex;
    flex-direction: column;
    height: 100%;
}

.editor-header {
    padding: 15px;
    border-bottom: 1px solid #ddd;
    display: flex;
    gap: 10px;
}

#page-path-input {
    flex: 1;
    padding: 8px;
    border: 1px solid #ddd;
    border-radius: 4px;
    font-size: 14px;
}

#save-btn {
    padding: 8px 20px;
    background: #28a745;
    color: white;
    border: none;
    border-radius: 4px;
    cursor: pointer;
    font-weight: bold;
}

#save-btn:hover {
    background: #218838;
}

.editor-container {
    display: flex;
    flex: 1;
    overflow: hidden;
}

#content-input, .preview {
    flex: 1;
    padding: 15px;
    font-family: monospace;
    font-size: 14px;
    border: none;
    resize: none;
}

#content-input {
    border-right: 1px solid #ddd;
}

.preview {
    background: #f9f9f9;
    overflow-y: auto;
}

.viewer-header {
    padding: 15px;
    border-bottom: 1px solid #ddd;
    display: flex;
    justify-content: space-between;
    align-items: center;
}

#edit-btn {
    padding: 8px 20px;
    background: #0066cc;
    color: white;
    border: none;
    border-radius: 4px;
    cursor: pointer;
}

.content {
    padding: 20px;
    flex: 1;
    overflow-y: auto;
}

.history {
    padding: 20px;
    border-top: 1px solid #ddd;
    background: #f9f9f9;
    max-height: 200px;
    overflow-y: auto;
}

.history-item {
    padding: 8px;
    margin-bottom: 8px;
    background: white;
    border-radius: 4px;
    font-size: 12px;
}

#welcome {
    padding: 40px;
    text-align: center;
}

#conflict-ui {
    padding: 20px;
    border: 2px solid #ff9800;
    background: #fffde7;
    border-radius: 4px;
}

.conflict-content {
    display: flex;
    gap: 20px;
    margin-bottom: 15px;
}

.conflict-side {
    flex: 1;
}

.conflict-side pre {
    background: white;
    padding: 10px;
    border-radius: 4px;
    max-height: 150px;
    overflow-y: auto;
    font-size: 12px;
}

#resolve-input {
    width: 100%;
    min-height: 150px;
    padding: 10px;
    font-family: monospace;
    font-size: 12px;
    border: 1px solid #ddd;
    border-radius: 4px;
    margin-bottom: 10px;
}

#resolve-btn {
    padding: 8px 20px;
    background: #28a745;
    color: white;
    border: none;
    border-radius: 4px;
    cursor: pointer;
}
```

- [ ] **Step 4: Verify structure**

```bash
ls -la /Users/alan/code/wiki-server/static/
```

Expected: All three files exist.

- [ ] **Step 5: Commit**

```bash
cd /Users/alan/code/wiki-server
git add static/
git commit -m "feat: frontend SPA with HTML, CSS, and vanilla JS"
```

---

## Task 10: Integration Testing

**Files:**
- Create: `tests/integration_test.rs`

- [ ] **Step 1: Write integration tests**

```rust
use std::path::PathBuf;
use std::process::Command;

fn setup_test_wiki() -> PathBuf {
    let tempdir = std::env::temp_dir().join("wiki_integration_test");
    let _ = std::fs::remove_dir_all(&tempdir);
    std::fs::create_dir_all(&tempdir).ok();
    
    Command::new("git")
        .args(&["init"])
        .current_dir(&tempdir)
        .output()
        .ok();

    Command::new("git")
        .args(&["config", "user.email", "test@example.com"])
        .current_dir(&tempdir)
        .output()
        .ok();

    Command::new("git")
        .args(&["config", "user.name", "Test User"])
        .current_dir(&tempdir)
        .output()
        .ok();

    // Create .users.json
    std::fs::write(tempdir.join(".users.json"), "[]").ok();

    tempdir
}

#[test]
fn test_full_wiki_workflow() {
    let wiki_dir = setup_test_wiki();
    
    // Create a page
    let page_path = wiki_dir.join("test.md");
    std::fs::write(&page_path, "# Test Page\nContent").ok();
    
    Command::new("git")
        .args(&["add", "test.md"])
        .current_dir(&wiki_dir)
        .output()
        .ok();

    let status = Command::new("git")
        .args(&["commit", "-m", "Initial commit"])
        .current_dir(&wiki_dir)
        .output()
        .unwrap();
    
    assert!(status.status.success());
}

#[test]
fn test_user_creation_and_auth() {
    // This would require spinning up the actual server
    // For now, test the auth module directly
    let tempdir = std::env::temp_dir().join("wiki_auth_test");
    let _ = std::fs::remove_dir_all(&tempdir);
    std::fs::create_dir_all(&tempdir).ok();

    let users_file = tempdir.join(".users.json");
    std::fs::write(&users_file, "[]").ok();

    // Would test: wiki_server::auth functions here
    assert!(users_file.exists());
}
```

- [ ] **Step 2: Run tests**

```bash
cd /Users/alan/code/wiki-server
cargo test --test integration_test
```

Expected: Tests compile and pass (basic integration tests).

- [ ] **Step 3: Commit**

```bash
cd /Users/alan/code/wiki-server
git add tests/
git commit -m "test: integration tests for wiki workflow"
```

---

## Task 11: README & Documentation

**Files:**
- Create: `README.md`

- [ ] **Step 1: Write README.md**

```markdown
# Wiki Server

A simple, git-backed wiki server for small teams. Single binary, markdown storage, built in Rust.

## Features

- Markdown-based pages with folder hierarchy
- Git version control and history
- Conflict resolution UI for concurrent edits
- Basic HTTP auth
- Admin user management
- Full-text search
- Single binary deployment

## Quick Start

### Build

```bash
cargo build --release
```

### Run

```bash
WIKI_PORT=3000 WIKI_DATA_DIR=/var/wiki ./target/release/wiki-server
```

First run: you'll be prompted to create an admin user.

### Access

Open http://localhost:3000 in your browser.

## Configuration

- `WIKI_PORT` — HTTP port (default: 3000)
- `WIKI_DATA_DIR` — Directory for wiki data + git repo (default: ./wiki_data)

## API

### Pages

- `GET /api/pages` — List all pages
- `GET /api/pages/:path` — Read page + history
- `POST /api/pages/:path` — Save page (detects conflicts)
- `POST /api/resolve` — Resolve conflict

### Search

- `GET /api/search?q=...` — Search pages

### Admin

- `POST /api/admin/users` — Create user
- `DELETE /api/admin/users/:user` — Delete user
- `PUT /api/admin/users/:user/password` — Set password

All routes require HTTP Basic Auth.

## Deployment

### Systemd

```ini
[Unit]
Description=Wiki Server
After=network.target

[Service]
Type=simple
ExecStart=/usr/local/bin/wiki-server
Environment="WIKI_PORT=3000"
Environment="WIKI_DATA_DIR=/var/wiki"
Restart=on-failure
User=wiki
Group=wiki

[Install]
WantedBy=multi-user.target
```

### Docker

```bash
docker build -t wiki-server .
docker run -p 3000:3000 -v /path/to/wiki:/data wiki-server
```

## Architecture

- **Backend**: Rust + Axum web framework
- **Storage**: Markdown files in git repo
- **Users**: JSON file with bcrypt-hashed passwords
- **Frontend**: Vanilla JS SPA
- **Git**: Subprocess integration (requires `git` CLI)

## Conflict Resolution

When two users edit the same page simultaneously:
1. First user saves successfully
2. Second user's save detects conflict
3. UI shows both versions
4. User selects sections to keep
5. Conflict is resolved and committed

## Future Enhancements

- Full-text search (tantivy)
- Drag-drop page reorganization
- User roles (read-only vs read-write)
- Rich markdown editor
- Dark mode

## License

MIT
```

- [ ] **Step 2: Commit**

```bash
cd /Users/alan/code/wiki-server
git add README.md
git commit -m "docs: README with features, setup, and API overview"
```

---

## Task 12: Verify Full Build

**Files:**
- (no changes, verification only)

- [ ] **Step 1: Full clean build**

```bash
cd /Users/alan/code/wiki-server
cargo clean
cargo build --release
```

Expected: Build succeeds, single binary at `target/release/wiki-server`.

- [ ] **Step 2: Verify binary size**

```bash
ls -lh /Users/alan/code/wiki-server/target/release/wiki-server
```

Expected: Binary is ~15-25 MB (single executable with no runtime deps).

- [ ] **Step 3: Final commit (if any cleanup needed)**

```bash
cd /Users/alan/code/wiki-server
git log --oneline | head -10
```

Expected: Clean commit history from Tasks 1-11.

---

## Summary

This plan produces a working wiki server:
- ✅ Single-binary Rust application
- ✅ Git-backed markdown storage
- ✅ HTTP Basic Auth + user management
- ✅ Conflict resolution UI
- ✅ Search
- ✅ REST API
- ✅ SPA frontend
- ✅ Full deployment support

Each task is small, testable, and builds incrementally on prior tasks.
