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
