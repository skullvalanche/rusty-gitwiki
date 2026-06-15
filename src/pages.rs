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
        use std::time::{SystemTime, UNIX_EPOCH};
        use std::thread;
        use std::time::Duration;

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let tempdir = std::env::temp_dir().join(format!("wiki_pages_test_{}", timestamp));
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
}
