use rusty_gitwiki::{User, UserRole};
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

pub fn create_user(
    users_file: &Path,
    username: &str,
    password: &str,
    role: UserRole,
) -> anyhow::Result<User> {
    let mut users = load_users(users_file)?;

    if users.iter().any(|u| u.username == username) {
        return Err(anyhow::anyhow!("User already exists"));
    }

    let password_hash = hash_password(password)?;
    let user = User {
        username: username.to_string(),
        password_hash,
        role,
        created_at: Utc::now(),
        name: "".to_string(),
        email: "".to_string(),
        description: "".to_string(),
    };

    users.push(user.clone());
    save_users(users_file, users)?;
    Ok(user)
}

pub fn set_user_role(
    users_file: &Path,
    username: &str,
    role: UserRole,
) -> anyhow::Result<User> {
    let mut users = load_users(users_file)?;

    let user = users.iter_mut().find(|u| u.username == username)
        .ok_or_else(|| anyhow::anyhow!("User not found"))?;

    user.role = role;

    let cloned = user.clone();
    save_users(users_file, users)?;
    Ok(cloned)
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

pub fn update_user_profile(
    users_file: &Path,
    username: &str,
    name: &str,
    email: &str,
    description: &str,
) -> anyhow::Result<User> {
    let mut users = load_users(users_file)?;
    let user = users.iter_mut().find(|u| u.username == username)
        .ok_or_else(|| anyhow::anyhow!("User not found"))?;

    user.name = name.to_string();
    user.email = email.to_string();
    user.description = description.to_string();

    let cloned = user.clone();
    save_users(users_file, users)?;
    Ok(cloned)
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
            role: UserRole::Admin,
            created_at: Utc::now(),
            name: "".to_string(),
            email: "".to_string(),
            description: "".to_string(),
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
            role: UserRole::Editor,
            created_at: Utc::now(),
            name: "".to_string(),
            email: "".to_string(),
            description: "".to_string(),
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
};
use std::sync::Arc;

pub async fn basic_auth_middleware(
    State(state): State<Arc<rusty_gitwiki::AppState>>,
    req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, rusty_gitwiki::WikiError> {
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
                                role: user.role,
                            });
                            return Ok(next.run(req).await);
                        }
                    }
                }
            }
        }
    }

    // No valid auth, return 401
    Err(rusty_gitwiki::WikiError::Unauthorized)
}

fn base64_decode(s: &str) -> Result<String, anyhow::Error> {
    let engine = base64::engine::general_purpose::STANDARD;
    use base64::Engine as _;
    let decoded = engine.decode(s)?;
    Ok(String::from_utf8(decoded)?)
}

#[derive(Debug, Clone)]
pub struct CurrentUser {
    pub username: String,
    pub role: UserRole,
}

impl CurrentUser {
    pub fn is_admin(&self) -> bool {
        self.role.is_admin()
    }

    pub fn can_edit(&self) -> bool {
        self.role.can_edit()
    }
}
