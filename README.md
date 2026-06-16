# Wiki Server

A simple, git-backed wiki server for small teams. Single binary, markdown storage, built in Rust.

## Features

- Markdown-based pages with folder hierarchy
- Git version control and history
- Conflict resolution UI for concurrent edits
- Basic HTTP auth
- Admin user management
- Full-text search
- Single binary deployment

## Quick Start

### Build

```bash
cargo build --release
```

### Run

```bash
WIKI_PORT=3000 WIKI_DATA_DIR=/var/wiki ./target/release/wiki-server
```

First run: you'll be prompted to create an admin user.

### Access

Open http://localhost:3000 in your browser.

## Configuration

- `WIKI_PORT` — HTTP port (default: 3000)
- `WIKI_DATA_DIR` — Directory for wiki data + git repo (default: ./wiki_data)

## API

### Pages

- `GET /api/pages` — List all pages
- `GET /api/pages/:path` — Read page + history
- `POST /api/pages/:path` — Save page (detects conflicts)
- `POST /api/resolve` — Resolve conflict

### Search

- `GET /api/search?q=...` — Search pages

### Admin

- `POST /api/admin/users` — Create user
- `DELETE /api/admin/users/:user` — Delete user
- `PUT /api/admin/users/:user/password` — Set password

All routes require HTTP Basic Auth.

## Deployment

### Systemd

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

```bash
docker build -t wiki-server .
docker run -p 3000:3000 -v /path/to/wiki:/data wiki-server
```

## Architecture

- **Backend**: Rust + Axum web framework
- **Storage**: Markdown files in git repo
- **Users**: JSON file with bcrypt-hashed passwords
- **Frontend**: Vanilla JS SPA
- **Git**: Subprocess integration (requires `git` CLI)

## Conflict Resolution

When two users edit the same page simultaneously:
1. First user saves successfully
2. Second user's save detects conflict
3. UI shows both versions
4. User selects sections to keep
5. Conflict is resolved and committed

## Future Enhancements

- Full-text search (tantivy)
- Drag-drop page reorganization
- User roles (read-only vs read-write)
- Rich markdown editor
- Dark mode

## License

MIT
