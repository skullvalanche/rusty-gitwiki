use wiki_server::{ArchivedPageResponse, ListPageResponse};
use std::path::{Component, Path, PathBuf};
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

pub fn archive_page(
    wiki_dir: &Path,
    page_path: &str,
    author: &str,
) -> anyhow::Result<String> {
    let file_path = path_to_file(wiki_dir, page_path)?;
    let archive_file = archived_path_to_file(wiki_dir, page_path)?;

    if !file_path.exists() {
        return Err(anyhow::anyhow!("Page not found"));
    }

    if archive_file.exists() {
        return Err(anyhow::anyhow!("Archived page already exists"));
    }

    if let Some(parent) = archive_file.parent() {
        std::fs::create_dir_all(parent)?;
    }

    git::git_rename(
        wiki_dir,
        &file_path,
        &archive_file,
        &format!("Archive {}", page_path),
        author,
    )?;
    remove_empty_parent_dirs(wiki_dir, &file_path)?;

    Ok(page_path.to_string())
}

pub fn rename_page(
    wiki_dir: &Path,
    old_path: &str,
    new_path: &str,
    author: &str,
) -> anyhow::Result<()> {
    let old_file = path_to_file(wiki_dir, old_path)?;
    let new_file = path_to_file(wiki_dir, new_path)?;

    if !old_file.exists() {
        return Err(anyhow::anyhow!("Page not found"));
    }

    if new_file.exists() {
        return Err(anyhow::anyhow!("Target page already exists"));
    }

    if let Some(parent) = new_file.parent() {
        std::fs::create_dir_all(parent)?;
    }

    git::git_rename(
        wiki_dir,
        &old_file,
        &new_file,
        &format!("Rename {} to {}", old_path, new_path),
        author,
    )?;
    remove_empty_parent_dirs(wiki_dir, &old_file)?;

    Ok(())
}

pub fn read_page(wiki_dir: &Path, page_path: &str) -> anyhow::Result<String> {
    let file_path = path_to_file(wiki_dir, page_path)?;
    std::fs::read_to_string(file_path).map_err(|e| anyhow::anyhow!(e))
}

pub fn list_archived_pages(wiki_dir: &Path) -> anyhow::Result<Vec<ArchivedPageResponse>> {
    let archive_dir = wiki_dir.join(".archive");
    let mut pages = Vec::new();

    if !archive_dir.exists() {
        return Ok(pages);
    }

    for entry in walkdir::WalkDir::new(&archive_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|ext| ext == "md").unwrap_or(false))
    {
        let path = entry.path();
        let relative = path.strip_prefix(&archive_dir)?;
        let page_path = relative.with_extension("").to_string_lossy().to_string();

        let metadata = std::fs::metadata(path)?;
        let modified = metadata.modified()?;
        let modified_dt = chrono::DateTime::<Utc>::from(modified);

        pages.push(ArchivedPageResponse {
            path: page_path.replace("\\", "/"),
            archived_at: modified_dt,
        });
    }

    pages.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(pages)
}

pub fn restore_archived_page(
    wiki_dir: &Path,
    page_path: &str,
    author: &str,
) -> anyhow::Result<()> {
    let archive_file = archived_path_to_file(wiki_dir, page_path)?;
    let restored_file = path_to_file(wiki_dir, page_path)?;

    if !archive_file.exists() {
        return Err(anyhow::anyhow!("Archived page not found"));
    }

    if restored_file.exists() {
        return Err(anyhow::anyhow!("Target page already exists"));
    }

    if let Some(parent) = restored_file.parent() {
        std::fs::create_dir_all(parent)?;
    }

    git::git_rename(
        wiki_dir,
        &archive_file,
        &restored_file,
        &format!("Restore archived {}", page_path),
        author,
    )?;
    remove_empty_parent_dirs(wiki_dir, &archive_file)?;

    Ok(())
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
    let page_path = page_path.trim();
    if page_path.is_empty() {
        return Err(anyhow::anyhow!("Page path required"));
    }

    let relative = Path::new(page_path);
    if relative.is_absolute() || relative.components().any(|component| {
        matches!(component, Component::ParentDir | Component::RootDir | Component::Prefix(_))
    }) {
        return Err(anyhow::anyhow!("Invalid page path"));
    }

    let mut file_path = wiki_dir.to_path_buf();
    file_path.push(format!("{}.md", page_path));

    // Prevent directory traversal
    if !file_path.starts_with(wiki_dir) {
        return Err(anyhow::anyhow!("Invalid page path"));
    }

    Ok(file_path)
}

fn archived_path_to_file(wiki_dir: &Path, page_path: &str) -> anyhow::Result<PathBuf> {
    path_to_file(wiki_dir, &format!(".archive/{}", page_path))
}

fn remove_empty_parent_dirs(wiki_dir: &Path, file_path: &Path) -> anyhow::Result<()> {
    let mut current = file_path.parent();

    while let Some(dir) = current {
        if dir == wiki_dir {
            break;
        }

        match std::fs::remove_dir(dir) {
            Ok(()) => current = dir.parent(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => current = dir.parent(),
            Err(e) if e.kind() == std::io::ErrorKind::DirectoryNotEmpty => break,
            Err(e) => return Err(e.into()),
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_wiki_dir() -> PathBuf {
        use std::time::{SystemTime, UNIX_EPOCH};
        use std::thread;
        use std::time::Duration;

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let tempdir = std::env::temp_dir().join(format!(
            "wiki_pages_test_{}_{}",
            timestamp,
            uuid::Uuid::new_v4()
        ));
        let _ = std::fs::remove_dir_all(&tempdir);
        std::fs::create_dir_all(&tempdir).ok();

        // Small delay to avoid lock contention
        thread::sleep(Duration::from_millis(50));

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

    #[test]
    fn test_archive_and_restore_page() {
        let wiki_dir = setup_wiki_dir();
        write_page(&wiki_dir, "docs/archive-me", "# Archive me", "alice").unwrap();

        archive_page(&wiki_dir, "docs/archive-me", "alice").unwrap();

        assert!(!wiki_dir.join("docs/archive-me.md").exists());
        assert!(wiki_dir.join(".archive/docs/archive-me.md").exists());

        let archived = list_archived_pages(&wiki_dir).unwrap();
        assert!(archived.iter().any(|p| p.path == "docs/archive-me"));

        restore_archived_page(&wiki_dir, "docs/archive-me", "alice").unwrap();

        assert!(wiki_dir.join("docs/archive-me.md").exists());
        assert!(!wiki_dir.join(".archive/docs/archive-me.md").exists());
        assert_eq!(read_page(&wiki_dir, "docs/archive-me").unwrap(), "# Archive me");
    }

    #[test]
    fn test_rename_page() {
        let wiki_dir = setup_wiki_dir();
        write_page(&wiki_dir, "docs/old-name", "# Rename me", "alice").unwrap();

        rename_page(&wiki_dir, "docs/old-name", "docs/new-name", "alice").unwrap();

        assert!(!wiki_dir.join("docs/old-name.md").exists());
        assert_eq!(read_page(&wiki_dir, "docs/new-name").unwrap(), "# Rename me");
    }

    #[test]
    fn test_rename_page_rejects_existing_target() {
        let wiki_dir = setup_wiki_dir();
        write_page(&wiki_dir, "docs/old-name", "# Old", "alice").unwrap();
        write_page(&wiki_dir, "docs/new-name", "# New", "alice").unwrap();

        let result = rename_page(&wiki_dir, "docs/old-name", "docs/new-name", "alice");

        assert!(result.is_err());
        assert_eq!(read_page(&wiki_dir, "docs/old-name").unwrap(), "# Old");
        assert_eq!(read_page(&wiki_dir, "docs/new-name").unwrap(), "# New");
    }
}
