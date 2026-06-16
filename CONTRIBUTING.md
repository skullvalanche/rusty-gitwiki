# Contributing

This document captures the architecture and development notes for Rusty Gitwiki.

## Architecture

Rusty Gitwiki is a small Rust/Axum application with a vanilla JavaScript single-page frontend.

- **Backend**: Rust with Axum routes in `src/main.rs`, request handlers in `src/api.rs` and `src/admin.rs`.
- **Frontend**: Static files in `static/`, served by the Rust app fallback.
- **Page storage**: Markdown files in the wiki data directory.
- **Versioning**: The wiki data directory is a git repository. Page writes, renames, archives, restores, and conflict resolutions are committed through git subprocesses.
- **Users**: Stored in `.users.json` in the wiki data directory with bcrypt-hashed passwords.
- **Search**: Tantivy index files in `.search-index/`, generated from markdown and ignored by git.

## Data Directory

The app defaults to `./wiki_data`, or `WIKI_DATA_DIR` / `--data-dir`.

Important files and directories inside the data directory:

- `*.md`: live wiki pages.
- `.archive/`: archived pages, preserving page paths under the archive root.
- `.users.json`: user accounts, password hashes, profiles, and roles.
- `.search-index/`: generated Tantivy index. This is disposable derived data.
- `.gitignore`: should include `.search-index/`.
- `.git/`: git history for page and user-storage commits.

The markdown files and `.users.json` are source-of-truth data. The search index can be deleted and rebuilt.

## Roles And Permissions

Users have one role:

- `admin`: manage users, change roles, reset passwords, view/restore archived pages, and edit pages.
- `editor`: create, edit, rename, archive, restore page versions, and resolve conflicts.
- `reader`: view pages, search, and browse history only.

Permission checks must be enforced server-side. The frontend hides unavailable actions, but that is only UX.

Use `CurrentUser::is_admin()` for admin-only endpoints and `CurrentUser::can_edit()` for page-mutating endpoints.

## Page Lifecycle

Live pages are markdown files addressed by normalized wiki paths without `.md`.

Examples:

- `home` maps to `home.md`
- `docs/setup` maps to `docs/setup.md`

Path handling should go through `pages::path_to_file` so traversal and unsafe path segments are rejected consistently.

Page actions:

- **Save**: writes markdown, commits the file, rebuilds search.
- **Rename**: `git mv` to the new path, commits, rebuilds search.
- **Archive**: `git mv` to `.archive/<path>.md`, commits, rebuilds search.
- **Restore archive**: `git mv` from `.archive/<path>.md` back to live path, commits, rebuilds search.
- **Restore version**: reads content from a historical git commit, writes it as the current page, commits, rebuilds search.

## Search Index

Search is backed by Tantivy in `src/search.rs`.

The index schema stores:

- `path`
- `title`
- `content`

The index is rebuilt:

- On application startup.
- After page save, rename, archive, restore, conflict resolve, and version restore.
- When an admin calls `POST /api/admin/search/reindex`.

The index lives in `.search-index/` and must remain ignored by git.

## Conflict Resolution

The editor sends `expected_git_head` when saving. The backend compares the target page against that expected head:

1. First user saves successfully.
2. Second user's save detects the page changed since their expected head.
3. API returns conflict details instead of committing.
4. UI shows current server content and the user's changes.
5. User submits resolved content to `/api/resolve`.
6. Backend commits the resolved page and rebuilds search.

Do not rely on frontend conflict checks alone; keep conflict detection in the backend.

## API Shape

All routes require HTTP Basic Auth.

Keep response shapes stable when possible because the vanilla JS frontend is coupled directly to the API.

Important route groups:

- `/api/pages`: list, read, save.
- `/api/pages/:path/archive`: archive live pages.
- `/api/pages/:path/rename`: rename live pages.
- `/api/archive`: list archived pages, admin only.
- `/api/archive/:path/restore`: restore archived page, admin only.
- `/api/search`: search pages.
- `/api/admin/users`: admin user management.
- `/api/admin/search/reindex`: admin search reindex.
- `/api/profile`: current user's profile and role capabilities.

## Frontend Notes

The frontend is intentionally dependency-free vanilla JavaScript.

Primary state lives in `static/app.js`:

- current page path and git head
- current profile and role capabilities
- local Basic Auth token
- known page paths for wiki links and page tree behavior

The app uses:

- `localStorage.authToken` for Basic Auth credentials.
- `localStorage.wiki-theme` for light/dark theme.
- Query param `?page=<path>` for page routing.

When adding UI actions, update both:

- frontend visibility/disabled states for UX
- backend authorization checks for enforcement

## Testing

Run:

```bash
cargo check
cargo test
node --check static/app.js
git diff --check
```

Search and page tests use temporary git repositories. Keep test directories unique to avoid git lock contention during parallel test runs.

## Development Guidelines

- Keep markdown files and `.users.json` as the source of truth.
- Treat `.search-index/` as rebuildable cache data.
- Do not commit generated search index files.
- Prefer existing modules and patterns over new abstractions.
- Keep admin-only behavior enforced in backend handlers.
- Keep reader/editor/admin behavior reflected in the frontend, but never rely on frontend checks for security.
- Use git subprocess helpers in `src/git.rs` for repository mutations.
- Use page helpers in `src/pages.rs` for path normalization and page file operations.
