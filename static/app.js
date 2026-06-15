const API_BASE = '/api';
let currentPage = null;
let currentGitHead = null;
let currentUser = null;

// Initialize
document.addEventListener('DOMContentLoaded', async () => {
    await loadPageList();

    // Event listeners
    document.getElementById('search-input').addEventListener('input', e => searchPages(e.target.value));
    document.getElementById('new-page-btn').addEventListener('click', () => newPage());
    document.getElementById('save-btn').addEventListener('click', () => savePage());
    document.getElementById('edit-btn').addEventListener('click', () => enterEditMode());
    document.getElementById('resolve-btn').addEventListener('click', () => resolveConflict());
});

async function loadPageList() {
    try {
        const resp = await fetch(`${API_BASE}/pages`);
        if (!resp.ok) return;

        const pages = await resp.json();
        const list = document.getElementById('page-list');
        list.innerHTML = '';

        pages.forEach(page => {
            const div = document.createElement('div');
            div.className = 'page-item';
            div.textContent = page.path;
            div.onclick = () => viewPage(page.path);
            list.appendChild(div);
        });
    } catch (e) {
        console.error('Failed to load pages:', e);
    }
}

async function viewPage(pagePath) {
    try {
        const resp = await fetch(`${API_BASE}/pages/${encodeURIComponent(pagePath)}`);
        if (!resp.ok) {
            showViewer();
            return;
        }

        const page = await resp.json();
        currentPage = pagePath;
        currentGitHead = page.current_git_head;

        document.getElementById('page-title').textContent = pagePath;
        const contentDiv = document.getElementById('page-content');
        contentDiv.innerHTML = markdownToHtml(page.content);

        const historyList = document.getElementById('history-list');
        historyList.innerHTML = page.history.map(c =>
            `<div class="history-item"><strong>${c.author}</strong> - ${c.message}</div>`
        ).join('');

        showViewer();
    } catch (e) {
        console.error('Failed to load page:', e);
    }
}

function enterEditMode() {
    document.getElementById('editor').style.display = 'block';
    document.getElementById('viewer').style.display = 'none';
    document.getElementById('welcome').style.display = 'none';
    document.getElementById('conflict-ui').style.display = 'none';

    document.getElementById('page-path-input').value = currentPage || '';

    // Load current content
    const contentDiv = document.getElementById('page-content');
    const contentText = contentDiv.textContent;
    document.getElementById('content-input').value = contentText;
}

function newPage() {
    currentPage = '';
    currentGitHead = null;
    document.getElementById('content-input').value = '';
    document.getElementById('page-path-input').value = '';
    enterEditMode();
}

async function savePage() {
    const path = document.getElementById('page-path-input').value.trim();
    const content = document.getElementById('content-input').value;

    if (!path) {
        alert('Page path required');
        return;
    }

    try {
        const resp = await fetch(`${API_BASE}/pages/${encodeURIComponent(path)}`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                content,
                expected_git_head: currentGitHead || '',
            }),
        });

        const result = await resp.json();

        if (result.conflict) {
            showConflictUI(path, result.current_content, result.their_changes);
        } else {
            alert('Page saved!');
            currentPage = path;
            await loadPageList();
            await viewPage(path);
        }
    } catch (e) {
        console.error('Failed to save page:', e);
        alert('Save failed');
    }
}

function showConflictUI(path, current, their) {
    document.getElementById('conflict-ui').style.display = 'block';
    document.getElementById('current-content').textContent = current;
    document.getElementById('their-content').textContent = their;
    document.getElementById('resolve-input').value = current; // Default to current
}

async function resolveConflict() {
    const resolved = document.getElementById('resolve-input').value;
    const path = document.getElementById('page-path-input').value;

    try {
        const resp = await fetch(`${API_BASE}/resolve`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                path,
                resolved_content: resolved,
                conflict_commit_hash: currentGitHead,
            }),
        });

        if (resp.ok) {
            alert('Conflict resolved!');
            currentPage = path;
            await loadPageList();
            await viewPage(path);
        }
    } catch (e) {
        console.error('Failed to resolve conflict:', e);
    }
}

async function searchPages(query) {
    if (!query) {
        await loadPageList();
        return;
    }

    try {
        const resp = await fetch(`${API_BASE}/search?q=${encodeURIComponent(query)}`);
        if (!resp.ok) return;

        const results = await resp.json();
        const list = document.getElementById('page-list');
        list.innerHTML = '';

        results.forEach(result => {
            const div = document.createElement('div');
            div.className = 'page-item';
            div.innerHTML = `<strong>${result.path}</strong><br><small>${result.excerpt}</small>`;
            div.style.cursor = 'pointer';
            div.onclick = () => viewPage(result.path);
            list.appendChild(div);
        });
    } catch (e) {
        console.error('Search failed:', e);
    }
}

function showViewer() {
    document.getElementById('viewer').style.display = 'block';
    document.getElementById('editor').style.display = 'none';
    document.getElementById('welcome').style.display = 'none';
}

function markdownToHtml(markdown) {
    // Simple markdown to HTML (for MVP)
    let html = markdown
        .replace(/^### (.*?)$/gm, '<h3>$1</h3>')
        .replace(/^## (.*?)$/gm, '<h2>$1</h2>')
        .replace(/^# (.*?)$/gm, '<h1>$1</h1>')
        .replace(/\*\*(.*?)\*\*/g, '<strong>$1</strong>')
        .replace(/\*(.*?)\*/g, '<em>$1</em>')
        .replace(/\n\n/g, '</p><p>')
        .replace(/^/gm, '')
        .replace(/$/gm, '');
    return `<p>${html}</p>`;
}
