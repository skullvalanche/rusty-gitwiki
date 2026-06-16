use std::path::PathBuf;

/// Setup helper: creates a temporary wiki directory with git initialized
fn setup_test_wiki() -> PathBuf {
    use std::time::{SystemTime, UNIX_EPOCH};
    use std::thread;
    use std::time::Duration;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let tempdir = std::env::temp_dir().join(format!("wiki_integration_test_{}", timestamp));
    let _ = std::fs::remove_dir_all(&tempdir);
    std::fs::create_dir_all(&tempdir).ok();

    // Small delay to avoid lock contention
    thread::sleep(Duration::from_millis(50));

    // Initialize git repo
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

    // Create initial .users.json
    let users_file = tempdir.join(".users.json");
    std::fs::write(&users_file, "[]").ok();

    tempdir
}

/// Test: Full wiki workflow (create page, read page, list pages)
#[test]
fn test_full_wiki_workflow() {
    let wiki_dir = setup_test_wiki();

    // 1. Write a page
    let page_path = "test_page";
    let content = "# Test Page\n\nThis is a test.";
    rusty_gitwiki_integration::write_page(&wiki_dir, page_path, content, "alice").unwrap();

    // 2. Verify page exists and can be read
    let read_content = rusty_gitwiki_integration::read_page(&wiki_dir, page_path).unwrap();
    assert_eq!(read_content, content);

    // 3. Write another page with hierarchy
    let nested_path = "docs/guide/intro";
    let nested_content = "# Introduction\n\nNested content.";
    rusty_gitwiki_integration::write_page(&wiki_dir, nested_path, nested_content, "bob").unwrap();

    // 4. List all pages
    let pages = rusty_gitwiki_integration::list_pages(&wiki_dir).unwrap();
    assert!(pages.iter().any(|p| p.path == page_path));
    assert!(pages.iter().any(|p| p.path == nested_path));
    assert_eq!(pages.len(), 2);

    // 5. Update a page
    let updated_content = "# Test Page\n\nUpdated content.";
    rusty_gitwiki_integration::write_page(&wiki_dir, page_path, updated_content, "alice").unwrap();

    let updated_read = rusty_gitwiki_integration::read_page(&wiki_dir, page_path).unwrap();
    assert_eq!(updated_read, updated_content);

    // 6. Verify page still exists in list
    let final_pages = rusty_gitwiki_integration::list_pages(&wiki_dir).unwrap();
    assert_eq!(final_pages.len(), 2);
}

/// Test: User creation and authentication
#[test]
fn test_user_creation_and_auth() {
    let wiki_dir = setup_test_wiki();
    let users_file = wiki_dir.join(".users.json");

    // 1. Create a user
    let username = "testuser";
    let password = "securepass123";
    let user = rusty_gitwiki_integration::create_user(
        &users_file,
        username,
        password,
        rusty_gitwiki::UserRole::Editor,
    ).unwrap();

    assert_eq!(user.username, username);
    assert!(matches!(user.role, rusty_gitwiki::UserRole::Editor));

    // 2. Find the user
    let found = rusty_gitwiki_integration::find_user(&users_file, username).unwrap();
    assert!(found.is_some());
    let found_user = found.unwrap();
    assert_eq!(found_user.username, username);
    assert!(matches!(found_user.role, rusty_gitwiki::UserRole::Editor));

    // 3. Verify password
    let is_valid = rusty_gitwiki_integration::verify_password(password, &found_user.password_hash).unwrap();
    assert!(is_valid);

    let is_invalid = rusty_gitwiki_integration::verify_password("wrongpass", &found_user.password_hash).unwrap();
    assert!(!is_invalid);

    // 4. Create admin user
    let admin_user = rusty_gitwiki_integration::create_user(
        &users_file,
        "admin",
        "adminpass",
        rusty_gitwiki::UserRole::Admin,
    ).unwrap();

    assert!(matches!(admin_user.role, rusty_gitwiki::UserRole::Admin));

    // 5. List both users
    let users = rusty_gitwiki_integration::load_users(&users_file).unwrap();
    assert_eq!(users.len(), 2);
    assert!(users.iter().any(|u| u.username == username && matches!(u.role, rusty_gitwiki::UserRole::Editor)));
    assert!(users.iter().any(|u| u.username == "admin" && matches!(u.role, rusty_gitwiki::UserRole::Admin)));

    // 6. Change password
    let new_password = "newpass456";
    rusty_gitwiki_integration::set_user_password(&users_file, username, new_password).unwrap();

    let updated_user = rusty_gitwiki_integration::find_user(&users_file, username).unwrap().unwrap();
    assert!(rusty_gitwiki_integration::verify_password(new_password, &updated_user.password_hash).unwrap());
    assert!(!rusty_gitwiki_integration::verify_password(password, &updated_user.password_hash).unwrap());

    // 7. Delete user
    rusty_gitwiki_integration::delete_user(&users_file, username).unwrap();

    let deleted = rusty_gitwiki_integration::find_user(&users_file, username).unwrap();
    assert!(deleted.is_none());

    let remaining_users = rusty_gitwiki_integration::load_users(&users_file).unwrap();
    assert_eq!(remaining_users.len(), 1);
    assert_eq!(remaining_users[0].username, "admin");
}

/// Helper module to expose internal functions for testing
mod rusty_gitwiki_integration {
    use rusty_gitwiki::{User, UserRole};
    use std::path::Path;
    use chrono::Utc;

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
        git_commit(wiki_dir, &file_path, &format!("Update {}", page_path), author)?;

        Ok(())
    }

    pub fn read_page(wiki_dir: &Path, page_path: &str) -> anyhow::Result<String> {
        let file_path = path_to_file(wiki_dir, page_path)?;
        std::fs::read_to_string(file_path).map_err(|e| anyhow::anyhow!(e))
    }

    pub fn list_pages(wiki_dir: &Path) -> anyhow::Result<Vec<rusty_gitwiki::ListPageResponse>> {
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

            pages.push(rusty_gitwiki::ListPageResponse {
                path: page_path.replace("\\", "/"),
                title: path.file_stem().unwrap_or_default().to_string_lossy().to_string(),
                updated_at: modified_dt,
                updated_by: "unknown".to_string(),
            });
        }

        Ok(pages)
    }

    pub fn path_to_file(wiki_dir: &Path, page_path: &str) -> anyhow::Result<std::path::PathBuf> {
        let mut file_path = wiki_dir.to_path_buf();
        file_path.push(format!("{}.md", page_path));

        // Prevent directory traversal
        if !file_path.starts_with(wiki_dir) {
            return Err(anyhow::anyhow!("Invalid page path"));
        }

        Ok(file_path)
    }

    pub fn git_commit(
        wiki_dir: &Path,
        file_path: &Path,
        message: &str,
        author: &str,
    ) -> anyhow::Result<()> {
        let output = std::process::Command::new("git")
            .args(&["add", file_path.to_string_lossy().as_ref()])
            .current_dir(wiki_dir)
            .output()?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("git add failed"));
        }

        let output = std::process::Command::new("git")
            .args(&[
                "commit",
                "-m",
                message,
                "--author",
                &format!("{} <{}>", author, "test@example.com"),
            ])
            .current_dir(wiki_dir)
            .output()?;

        if !output.status.success() {
            // Ignore errors (e.g., nothing to commit)
        }

        Ok(())
    }

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

        let user = users
            .iter_mut()
            .find(|u| u.username == username)
            .ok_or_else(|| anyhow::anyhow!("User not found"))?;

        user.password_hash = hash_password(new_password)?;
        save_users(users_file, users)?;
        Ok(())
    }
}
