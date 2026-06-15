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
