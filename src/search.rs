use std::path::{Path, PathBuf};
use tantivy::{
    collector::TopDocs,
    doc,
    query::QueryParser,
    schema::{Field, Schema, Value, STORED, TEXT},
    Index, TantivyDocument,
};
use wiki_server::SearchResult;

const INDEX_DIR: &str = ".search-index";
const RESULT_LIMIT: usize = 25;

struct SearchFields {
    path: Field,
    title: Field,
    content: Field,
}

fn search_schema() -> (Schema, SearchFields) {
    let mut builder = Schema::builder();
    let path = builder.add_text_field("path", TEXT | STORED);
    let title = builder.add_text_field("title", TEXT | STORED);
    let content = builder.add_text_field("content", TEXT | STORED);
    let schema = builder.build();
    (schema, SearchFields { path, title, content })
}

fn index_path(wiki_dir: &Path) -> PathBuf {
    wiki_dir.join(INDEX_DIR)
}

pub fn ensure_search_index_ignored(wiki_dir: &Path) -> anyhow::Result<()> {
    let gitignore = wiki_dir.join(".gitignore");
    let entry = format!("{}/", INDEX_DIR);
    let existing = std::fs::read_to_string(&gitignore).unwrap_or_default();

    if existing.lines().any(|line| line.trim() == entry) {
        return Ok(());
    }

    let mut next = existing;
    if !next.is_empty() && !next.ends_with('\n') {
        next.push('\n');
    }
    next.push_str(&entry);
    next.push('\n');
    std::fs::write(gitignore, next)?;
    Ok(())
}

pub fn rebuild_index(wiki_dir: &Path) -> anyhow::Result<usize> {
    ensure_search_index_ignored(wiki_dir)?;

    let index_dir = index_path(wiki_dir);
    if index_dir.exists() {
        std::fs::remove_dir_all(&index_dir)?;
    }
    std::fs::create_dir_all(&index_dir)?;

    let (schema, fields) = search_schema();
    let index = Index::create_in_dir(&index_dir, schema)?;
    let mut writer = index.writer(50_000_000)?;
    let mut count = 0;

    for page in markdown_pages(wiki_dir) {
        let page = page?;
        let path = page_path_from_file(wiki_dir, &page)?;
        let content = std::fs::read_to_string(&page)?;
        let title = title_from_content(&path, &content);

        writer.add_document(doc!(
            fields.path => path,
            fields.title => title,
            fields.content => content,
        ))?;
        count += 1;
    }

    writer.commit()?;
    Ok(count)
}

pub fn search(wiki_dir: &Path, query: &str) -> anyhow::Result<Vec<SearchResult>> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    let index_dir = index_path(wiki_dir);
    if !index_dir.exists() {
        rebuild_index(wiki_dir)?;
    }

    let (schema, fields) = search_schema();
    let index = Index::open_or_create(tantivy::directory::MmapDirectory::open(&index_dir)?, schema)?;
    let reader = index.reader()?;
    let searcher = reader.searcher();
    let parser = QueryParser::for_index(&index, vec![fields.path, fields.title, fields.content]);
    let query = parser.parse_query(trimmed)
        .or_else(|_| parser.parse_query(&format!("\"{}\"", trimmed)))?;

    let hits = searcher.search(&query, &TopDocs::with_limit(RESULT_LIMIT).order_by_score())?;
    let mut results = Vec::new();

    for (_score, address) in hits {
        let doc = searcher.doc::<TantivyDocument>(address)?;
        let Some(path) = text_field(&doc, fields.path) else {
            continue;
        };
        let content = text_field(&doc, fields.content).unwrap_or_default();

        results.push(SearchResult {
            path,
            excerpt: excerpt_for(trimmed, &content),
        });
    }

    Ok(results)
}

fn markdown_pages(wiki_dir: &Path) -> impl Iterator<Item = anyhow::Result<PathBuf>> + '_ {
    walkdir::WalkDir::new(wiki_dir)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().extension().map(|ext| ext == "md").unwrap_or(false))
        .filter(move |entry| {
            entry.path()
                .strip_prefix(wiki_dir)
                .ok()
                .and_then(|path| path.components().next())
                .map(|component| !component.as_os_str().to_string_lossy().starts_with('.'))
                .unwrap_or(false)
        })
        .map(|entry| Ok(entry.into_path()))
}

fn page_path_from_file(wiki_dir: &Path, file_path: &Path) -> anyhow::Result<String> {
    let relative = file_path.strip_prefix(wiki_dir)?;
    Ok(relative.with_extension("").to_string_lossy().replace('\\', "/"))
}

fn title_from_content(path: &str, content: &str) -> String {
    content
        .lines()
        .find_map(|line| line.trim().strip_prefix("# ").map(str::trim))
        .filter(|title| !title.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| path.rsplit('/').next().unwrap_or(path).to_string())
}

fn text_field(doc: &TantivyDocument, field: Field) -> Option<String> {
    doc.get_first(field)
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

fn excerpt_for(query: &str, content: &str) -> String {
    let query_lower = query.to_lowercase();
    let query_terms = query_lower
        .split_whitespace()
        .filter(|term| !term.is_empty())
        .collect::<Vec<_>>();

    let line = content.lines().find(|line| {
        let line_lower = line.to_lowercase();
        query_terms.iter().any(|term| line_lower.contains(term))
    }).or_else(|| content.lines().find(|line| !line.trim().is_empty()));

    let mut excerpt = line.unwrap_or("Matching page").trim().to_string();
    if excerpt.len() > 140 {
        excerpt.truncate(140);
        excerpt.push_str("...");
    }
    excerpt
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn setup_wiki_dir() -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let tempdir = std::env::temp_dir().join(format!(
            "wiki_search_test_{}_{}",
            std::process::id(),
            nanos
        ));
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

        rebuild_index(&wiki_dir).unwrap();
        let results = search(&wiki_dir, "rust").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].path, "rust");
    }

    #[test]
    fn test_search_by_content() {
        let wiki_dir = setup_wiki_dir();
        std::fs::write(wiki_dir.join("page1.md"), "This contains database info").ok();
        std::fs::write(wiki_dir.join("page2.md"), "Nothing here").ok();

        rebuild_index(&wiki_dir).unwrap();
        let results = search(&wiki_dir, "database").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].path, "page1");
    }

    #[test]
    fn test_ignores_archived_pages() {
        let wiki_dir = setup_wiki_dir();
        std::fs::create_dir_all(wiki_dir.join(".archive")).ok();
        std::fs::write(wiki_dir.join(".archive/old.md"), "archived database").ok();

        rebuild_index(&wiki_dir).unwrap();
        let results = search(&wiki_dir, "database").unwrap();
        assert!(results.is_empty());
    }
}
