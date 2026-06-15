// Placeholder - to be implemented in Task 4
use std::path::Path;
use wiki_server::ListPageResponse;

pub fn write_page(
    _wiki_dir: &Path,
    _page_path: &str,
    _content: &str,
    _author: &str,
) -> anyhow::Result<()> {
    todo!()
}

pub fn read_page(_wiki_dir: &Path, _page_path: &str) -> anyhow::Result<String> {
    todo!()
}

pub fn page_exists(_wiki_dir: &Path, _page_path: &str) -> anyhow::Result<bool> {
    todo!()
}

pub fn list_pages(_wiki_dir: &Path) -> anyhow::Result<Vec<ListPageResponse>> {
    todo!()
}

pub fn path_to_file(_wiki_dir: &Path, _page_path: &str) -> anyhow::Result<std::path::PathBuf> {
    todo!()
}
