# Wiki Server

A simple, git-backed wiki server for small teams. Single binary, markdown storage, built in Rust.

## Features

- Markdown-based pages with folder hierarchy
- Git version control and history
- Conflict resolution UI for concurrent edits
- Basic HTTP auth
- Admin user management with admin/editor/reader roles
- Full-text search with a generated Tantivy index
- Dark mode
- Single binary runtime

## Quick Start

### Build

```bash
cargo build --release
```

### Run

```bash
./target/release/wiki-server --port 3000 --data-dir /var/wiki
```

First run creates a default admin user: `admin` / `admin`. Change that password immediately.

### Access

Open http://localhost:3000 in your browser.

## Configuration

CLI flags:

- `--port PORT` — HTTP port (default: 3000)
- `--data-dir PATH` — Directory for wiki data + git repo (default: ./wiki_data)

Environment variable fallbacks:

- `WIKI_PORT`
- `WIKI_DATA_DIR`

## API

### Pages

- `GET /api/pages` — List all pages
- `GET /api/pages/:path` — Read page + history
- `POST /api/pages/:path` — Save page (detects conflicts)
- `POST /api/resolve` — Resolve conflict

### Search

- `GET /api/search?q=...` — Search pages

### Admin

- `GET /api/admin/users` — List users
- `POST /api/admin/users` — Create user
- `DELETE /api/admin/users/:user` — Delete user
- `PUT /api/admin/users/:user/password` — Set password
- `PUT /api/admin/users/:user/role` — Set role
- `POST /api/admin/search/reindex` — Rebuild the generated search index

All routes require HTTP Basic Auth.

## Production

Run Wiki Server behind a reverse proxy that handles TLS, public routing, request limits, and network exposure. Bind the app to an internal interface or private network port, and keep the wiki data directory on persistent storage.

## License

MIT
