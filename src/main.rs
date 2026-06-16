mod auth;
mod git;
mod pages;
mod search;
mod api;
mod admin;

use axum::{
    routing::{get, post, delete, put},
    Router,
    middleware,
};
use std::sync::Arc;
use tower_http::trace::TraceLayer;
use std::path::PathBuf;
use wiki_server::{AppState, UserRole};
use tracing_subscriber;

struct Config {
    data_dir: PathBuf,
    port: u16,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let config = Config::from_args(std::env::args().skip(1))?;
    let wiki_data_dir = config.data_dir;

    // Ensure wiki_data exists and is a git repo
    if !wiki_data_dir.exists() {
        init_wiki_repo(&wiki_data_dir).await?;
    }
    search::ensure_search_index_ignored(&wiki_data_dir)?;
    search::rebuild_index(&wiki_data_dir)?;

    let state = Arc::new(AppState {
        wiki_data_dir: wiki_data_dir.clone(),
    });

    let app = Router::new()
        .route("/api/pages", get(api::list_pages))
        .route("/api/pages/:path", get(api::get_page).post(api::save_page))
        .route("/api/pages/:path/archive", post(api::archive_page))
        .route("/api/pages/:path/rename", post(api::rename_page))
        .route("/api/archive", get(api::list_archived_pages))
        .route("/api/archive/:path/restore", post(api::restore_archived_page))
        .route("/api/pages/:path/version/:commit", get(api::get_page_at_version))
        .route("/api/pages/:path/restore/:commit", post(api::restore_page_version))
        .route("/api/resolve", post(api::resolve_conflict))
        .route("/api/search", get(api::search_pages))
        .route("/api/render", post(api::render_markdown))
        .route("/api/profile", get(api::get_profile).put(api::update_profile))
        .route("/api/admin/users", get(admin::list_users).post(admin::create_user))
        .route("/api/admin/users/:user", delete(admin::delete_user))
        .route("/api/admin/users/:user/password", put(admin::set_password))
        .route("/api/admin/users/:user/role", put(admin::set_role))
        .route("/api/admin/search/reindex", post(api::rebuild_search_index))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::basic_auth_middleware,
        ))
        .layer(TraceLayer::new_for_http())
        .fallback(api::serve_static)
        .with_state(state)
        .into_make_service();

    let bind_addr = format!("0.0.0.0:{}", config.port);
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    println!("Wiki server running on http://{}", bind_addr);
    axum::serve(listener, app).await?;

    Ok(())
}

impl Config {
    fn from_args(args: impl IntoIterator<Item = String>) -> anyhow::Result<Self> {
        let mut data_dir = std::env::var("WIKI_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("./wiki_data"));
        let mut port = match std::env::var("WIKI_PORT") {
            Ok(value) => parse_port(&value)?,
            Err(_) => 3000,
        };

        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            if arg == "--help" || arg == "-h" {
                print_usage();
                std::process::exit(0);
            }

            if let Some(value) = arg.strip_prefix("--data-dir=") {
                data_dir = PathBuf::from(value);
                continue;
            }

            if let Some(value) = arg.strip_prefix("--port=") {
                port = parse_port(value)?;
                continue;
            }

            match arg.as_str() {
                "--data-dir" => {
                    let value = args.next()
                        .ok_or_else(|| anyhow::anyhow!("--data-dir requires a value"))?;
                    data_dir = PathBuf::from(value);
                }
                "--port" => {
                    let value = args.next()
                        .ok_or_else(|| anyhow::anyhow!("--port requires a value"))?;
                    port = parse_port(&value)?;
                }
                _ => {
                    return Err(anyhow::anyhow!(
                        "Unknown argument: {}\nRun with --help for usage.",
                        arg
                    ));
                }
            }
        }

        Ok(Self { data_dir, port })
    }
}

fn parse_port(value: &str) -> anyhow::Result<u16> {
    value
        .parse::<u16>()
        .map_err(|_| anyhow::anyhow!("Invalid port: {}", value))
}

fn print_usage() {
    println!(
        "Usage: wiki-server [--data-dir PATH] [--port PORT]\n\n\
Options:\n  \
--data-dir PATH  Wiki data directory (default: ./wiki_data or WIKI_DATA_DIR)\n  \
--port PORT      HTTP port (default: 3000 or WIKI_PORT)\n  \
-h, --help       Show this help"
    );
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

    // Configure git user for commits
    tokio::process::Command::new("git")
        .args(&["config", "user.email", "wiki@localhost"])
        .current_dir(wiki_data_dir)
        .output()
        .await?;
    tokio::process::Command::new("git")
        .args(&["config", "user.name", "Wiki"])
        .current_dir(wiki_data_dir)
        .output()
        .await?;

    // Create default admin user
    let users_file = wiki_data_dir.join(".users.json");
    let admin_user = auth::create_user(&users_file, "admin", "admin", UserRole::Admin)
        .map_err(|e| anyhow::anyhow!("Failed to create admin user: {}", e))?;
    println!("Created default admin user: admin / admin (change password immediately!)");

    drop(admin_user); // avoid unused warning

    // Create default home page
    let home_page = r#"# Welcome to Wiki

This is your wiki's home page. Welcome!

## Getting Started

### Creating Pages

To create a new page, use the **+ New Page** button in the sidebar or the top input field. Pages are organized by **path**, similar to folders:

- `getting-started` — creates a top-level page
- `docs/guide/setup` — creates nested pages in folders
- `tutorials/python/hello-world` — creates deeply nested pages

The path becomes the page's location and name.

### Editing Pages

Click **Edit** on any page to modify its content. This wiki uses **Markdown** formatting:

- `# Heading 1`
- `## Heading 2`
- `**bold** and *italic*`
- Lists, code blocks, links — standard Markdown syntax

Use **Rename** in the editor header to move a page to a new path. Use **Archive** to remove a page from the active wiki without destroying it. Admins can review archived pages and restore them later.

### Linking Pages

Use wiki links to connect pages:

- `[[home]]` — links to a page by path
- `[[Setup Guide|docs/guide/setup]]` — shows a label while linking to a page path
- Missing page links are shown with a `+`; click one to create that page.

### Page Listings

Use page-list directives to keep index pages current:

- `[[children]]` — lists direct sub-pages of the current page
- `[[children:docs]]` — lists direct sub-pages under `docs`
- `[[tree]]` — shows a nested sub-page tree under the current page
- `[[tree:docs]]` — shows a nested tree under `docs`

### Table of Contents

Use TOC directives to link to headings within a page:

- `[[toc]]` — includes headings from `##` down
- `[[toc:2]]` — includes `##` and deeper headings
- `[[toc:2-4]]` — includes only `##` through `####`

### Saving Changes

When you save, your changes are automatically:
1. Committed to git (preserving history)
2. Attributed to your username
3. Stored as markdown files

You can view the commit history in the **History** section below each page.

### Handling Conflicts

If two people edit the same page simultaneously:
1. The second person's save will detect a conflict
2. A conflict resolution UI will appear
3. Select which sections to keep
4. Submit the resolved version

Git handles merging automatically — no manual conflict resolution needed.

### Searching

Use the search box in the sidebar to find pages by:
- **Filename** — search for page names/paths
- **Content** — search inside page content

Results show matching pages with excerpts.

### User Administration

Admins can open **Users** from the sidebar profile area to:
- Create admins, editors, and readers
- Change user roles
- Reset user passwords
- Remove users who no longer need access

Admins can also open **Archived Pages** to restore archived content.

Roles:
- **Admin** — manage users and archived pages, and edit wiki pages
- **Editor** — create, edit, rename, archive, and restore page versions
- **Reader** — view, search, and browse history without editing

## Tips

- Use consistent naming for related pages (e.g., `docs/api/`, `docs/cli/`)
- Start with an overview page, then add detail pages
- Archive old pages instead of deleting their files manually
- View full commit history: `git log` in the wiki_data directory

## What's Next?

- Click **Edit** on this page to customize it
- Use **+ New Page** to start adding content
- Invite teammates and start collaborating

Happy wiki-ing! 📝
"#;

    let gitignore_file = wiki_data_dir.join(".gitignore");
    tokio::fs::write(&gitignore_file, ".search-index/\n").await?;

    let home_file = wiki_data_dir.join("home.md");
    tokio::fs::write(&home_file, home_page).await?;

    // Commit the home page
    tokio::process::Command::new("git")
        .args(&["add", "home.md", ".users.json", ".gitignore"])
        .current_dir(wiki_data_dir)
        .output()
        .await?;
    tokio::process::Command::new("git")
        .args(&["commit", "-m", "Initial commit: home page and user storage"])
        .current_dir(wiki_data_dir)
        .output()
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cli_flags() {
        let config = Config::from_args(vec![
            "--data-dir".to_string(),
            "/tmp/wiki".to_string(),
            "--port".to_string(),
            "4000".to_string(),
        ]).unwrap();

        assert_eq!(config.data_dir, PathBuf::from("/tmp/wiki"));
        assert_eq!(config.port, 4000);
    }

    #[test]
    fn parses_equals_cli_flags() {
        let config = Config::from_args(vec![
            "--data-dir=/tmp/wiki".to_string(),
            "--port=4001".to_string(),
        ]).unwrap();

        assert_eq!(config.data_dir, PathBuf::from("/tmp/wiki"));
        assert_eq!(config.port, 4001);
    }

    #[test]
    fn rejects_invalid_port() {
        let result = Config::from_args(vec!["--port".to_string(), "not-a-port".to_string()]);

        assert!(result.is_err());
    }
}
