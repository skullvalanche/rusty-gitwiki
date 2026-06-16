# Wiki Server Design

**Date:** 2026-06-15  
**Scope:** Simple, team-friendly wiki with git-backed storage and markdown files.

---

## Overview

Single-binary Rust wiki server. Stores pages as markdown files in a git repository. Designed for small teams (5-20 people) who want:
- Portable, version-controlled wiki (easy to migrate, backup, fork)
- Simple collaboration with conflict resolution UI
- No database—just filesystem + git
- Easy deployment (one binary)

---

## Architecture

```
Browser
  ↓
Web Server (Axum + Tokio)
  ├── REST API (/api/*)
  ├── Static assets (HTML/CSS/JS)
  └── Basic auth middleware
       ↓
       ├── Page logic (read/write markdown)
       ├── Git subprocess (merge, commit, history)
       ├── Admin (user mgmt)
       └── Search (substring on filenames/content)
            ↓
            wiki_data/ (local git repo with .md files)
```

Request flow for page edit:
1. User edits page, clicks save
2. Server checks if git HEAD changed since page load
3. If no conflict: write file, `git add`, `git commit`
4. If conflict: run `git merge`, return conflict UI with both versions
5. User picks sections, submits resolution
6. Server commits resolved version

---

## Components

### 1. Web Server (Axum)
- Serves REST API + bundled static assets
- Basic auth on all routes (username/password in Authorization header)
- CORS: none needed (same-origin SPA)

### 2. Frontend (Vanilla JS or HTMX)
- Single-page app: page list + editor
- Sidebar: folder tree (expandable), recent pages, search
- Page editor: textarea + live markdown preview (split pane)
- Conflict UI: left panel = current git version, right = user changes, radio buttons to pick sections
- Admin panel (for admin users): user create/delete, password reset

### 3. Page Storage
- Location: `wiki_data/` directory (git repo)
- Structure: folders match hierarchy (e.g., `docs/guides/setup.md` → page at path `docs/guides/setup`)
- Filenames: sanitized page names + `.md` extension

### 4. Git Integration
- Spawn `git` CLI for: commit, merge, log (history), diff
- Store user identity per commit: basic auth user → commit author
- Merge strategy: automatic (git merge), user resolves conflicts via UI
- No force-push, no rebase—simple linear history

### 5. Admin Module
- First startup: create admin user (prompt on stdin or env var)
- `/admin` interface: create/delete users, set passwords
- Users stored: flat file (JSON) or embedded SQLite (TBD, keep simple)
- Passwords: bcrypt hashed

### 6. Search
- Naive substring search: grep filenames + content
- Future: full-text search (tantivy) when needed

---

## Data Model

### User
```
{
  username: String,
  password_hash: String,    // bcrypt
  is_admin: bool,
  created_at: DateTime,
}
```

Users stored in `wiki_data/.users.json` (or embedded DB).

### Page (implicit from filesystem)
```
Path: docs/guides/setup
File: wiki_data/docs/guides/setup.md
Content: markdown
History: git log wiki_data/docs/guides/setup.md
```

---

## API Endpoints

### Pages
- `GET /api/pages` — list all pages (recursively from wiki_data/)
  - Returns: `[{path, title, updated_at, updated_by}, ...]`
- `GET /api/pages/:path` — read page + history
  - Returns: `{path, content, history: [{commit_hash, author, message, date}, ...], current_git_head: String}`
- `POST /api/pages/:path` — save page
  - Input: `{content, expected_git_head}`
  - Returns on success: `{commit_hash, author, message}`
  - Returns on conflict: `{conflict: true, current_content: String, their_changes: String, base: String}`
- `POST /api/resolve` — submit conflict resolution
  - Input: `{path, resolved_content, conflict_commit_hash}`
  - Returns: `{commit_hash, author, message}`

### Search
- `GET /api/search?q=...` — search pages
  - Returns: `[{path, excerpt, highlight_positions}, ...]`

### Admin
- `POST /api/admin/users` — create user
  - Input: `{username, password, is_admin}`
  - Returns: `{username, created_at}`
- `DELETE /api/admin/users/:user` — delete user
- `PUT /api/admin/users/:user/password` — set password
  - Input: `{password}`

### Auth
- `POST /api/auth/login` — exchange credentials for token (or use basic auth throughout)
  - Simpler: stick with basic auth, no session tokens needed

---

## User Workflows

### View Page
1. User navigates to page via sidebar tree or search
2. GET `/api/pages/:path`
3. Display content + history sidebar

### Edit Page
1. User clicks edit, textarea opens with current content
2. User types changes
3. Click save
4. POST `/api/pages/:path` with new content + current_git_head
5. If conflict: show conflict UI
   - User picks sections (radio buttons)
   - Submit resolution → POST `/api/resolve`
6. On success: update display, show confirmation

### Create Page
1. User enters path (e.g., `docs/new-guide`) in sidebar
2. Creates file → first save uses POST `/api/pages/docs/new-guide`
3. File + directories created, committed

### Manage Users (Admin)
1. Admin visits `/admin`
2. List current users
3. Create user: form → POST `/api/admin/users`
4. Delete user: click delete → DELETE `/api/admin/users/:user`

---

## Tech Stack

| Layer | Choice | Rationale |
|-------|--------|-----------|
| Language | Rust | Single binary, no runtime deps, compile-time safety |
| Web Framework | Axum | Minimal, composable, good ecosystem |
| Async Runtime | Tokio | Industry standard, proven |
| Markdown | comrak | Simple, renders to HTML |
| Git | spawn `git` CLI | Avoid reimplementing merge logic |
| Users | JSON file or SQLite | TBD in implementation (JSON simpler to start) |
| Frontend | Vanilla JS or HTMX | Keep it simple, avoid npm bloat |
| CSS | Minimal (classless) or Tailwind | TBD, keep lightweight |
| Auth | HTTP Basic Auth | Simple, sufficient for internal teams |

---

## File Structure

```
wiki-server/
├── src/
│   ├── main.rs              # server setup, port listen
│   ├── api.rs               # REST handlers
│   ├── auth.rs              # basic auth middleware
│   ├── git.rs               # git subprocess wrappers
│   ├── pages.rs             # page CRUD (read/write/list)
│   ├── search.rs            # search impl
│   ├── admin.rs             # user management
│   └── lib.rs               # shared types
├── static/
│   ├── index.html           # SPA shell
│   ├── style.css            # styles
│   └── app.js               # frontend JS
├── wiki_data/               # git repo (created at first run)
│   ├── .git/
│   ├── .users.json          # user list (or .users.db)
│   └── *.md                 # wiki pages
├── Cargo.toml
└── README.md
```

---

## Deployment

### Build
```bash
cargo build --release
```

Outputs: `target/release/wiki-server` (single binary, ~10-20 MB)

### Run
```bash
WIKI_PORT=3000 WIKI_DATA_DIR=/var/wiki ./wiki-server
```

First run: prompts for admin username/password, initializes `wiki_data/` as git repo.

### Systemd Service
```ini
[Unit]
Description=Wiki Server
After=network.target

[Service]
Type=simple
ExecStart=/usr/local/bin/wiki-server
Environment="WIKI_PORT=3000"
Environment="WIKI_DATA_DIR=/var/wiki"
Restart=on-failure
User=wiki
Group=wiki

[Install]
WantedBy=multi-user.target
```

### Docker
```dockerfile
FROM rust:latest as builder
WORKDIR /build
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=builder /build/target/release/wiki-server /app/wiki-server
COPY --from=builder /build/static /app/static
WORKDIR /app
ENV WIKI_PORT=3000 WIKI_DATA_DIR=/data
EXPOSE 3000
CMD ["./wiki-server"]
```

---

## Error Handling

- **Auth fails:** 401 Unauthorized (basic auth prompt)
- **Page not found:** 404 (return empty page option to create)
- **Conflict on save:** Return conflict details (not an error, expected workflow)
- **Git command fails:** 500 Internal Server Error + log details
- **Disk full/permission denied:** 500 Internal Server Error + log details

---

## Testing

- Unit tests: page CRUD, git wrapper, search
- Integration tests: full workflows (create page, edit, conflict, resolve)
- Manual: test admin UI, conflict resolution, git history accuracy

---

## Future Enhancements (Post-MVP)

- Drag-drop page reorganization
- Full-text search (tantivy)
- Backlinks / knowledge graph
- Markdown plugins (tables, math, code highlighting)
- User roles (read-only vs read-write)
- Page comments/annotations
- Export to static HTML/PDF
- Dark mode
- Rich markdown editor (Monaco, CodeMirror)

---

## Open Questions / TBD

- User storage: JSON file vs SQLite? (JSON simpler, SQLite scales better)
- Frontend framework: Vanilla JS vs HTMX vs Lit? (start vanilla, migrate if needed)
- CSS framework: none vs Tailwind vs classless? (keep minimal)
- Initial git commit message format? (e.g., "Created by admin")
