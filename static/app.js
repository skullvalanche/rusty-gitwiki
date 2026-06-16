const API_BASE = '/api';
let currentPage = null;
let currentGitHead = null;
let currentUser = null;
let authToken = localStorage.getItem('authToken') || null;
let currentPageContent = null;
let returnPageAfterCancel = null;
let pagePaths = new Set();

// Initialize
document.addEventListener('DOMContentLoaded', async () => {
    initTheme();
    document.getElementById('login-btn').addEventListener('click', () => handleLogin());
    document.getElementById('login-password').addEventListener('keypress', (e) => {
        if (e.key === 'Enter') handleLogin();
    });

    if (authToken) {
        setSidebarProfile(getFallbackProfile());
        await loadPageList();
        await loadUserProfile();
    } else {
        showLoginModal();
    }

    // Event listeners
    document.getElementById('search-input').addEventListener('input', e => {
        updateSearchClearButton();
        searchPages(e.target.value);
    });
    document.getElementById('search-clear-btn').addEventListener('click', () => clearSearch());
    updateSearchClearButton();
    document.getElementById('new-page-btn').addEventListener('click', () => newPage());
    document.getElementById('save-btn').addEventListener('click', () => savePage());
    document.getElementById('edit-btn').addEventListener('click', () => enterEditMode());
    document.getElementById('rename-btn').addEventListener('click', () => renameCurrentPage());
    document.getElementById('archive-btn').addEventListener('click', () => archiveCurrentPage());
    document.getElementById('resolve-btn').addEventListener('click', () => resolveConflict());
    document.getElementById('history-toggle').addEventListener('click', (e) => {
        e.preventDefault();
        toggleHistory();
    });
    document.getElementById('version-close-btn').addEventListener('click', () => {
        showOnly('viewer');
    });
    document.getElementById('restore-btn').addEventListener('click', () => restoreVersion());
    document.getElementById('content-input').addEventListener('input', () => updatePreview());
    document.getElementById('cancel-btn').addEventListener('click', () => cancelEdit());
    initEditorScrollSync();

    document.getElementById('logout-btn').addEventListener('click', () => handleLogout());
    document.getElementById('profile-btn').addEventListener('click', () => showProfileViewer());
    document.getElementById('user-management-btn').addEventListener('click', () => showUserManagementViewer());
    document.getElementById('archived-pages-btn').addEventListener('click', () => showArchiveViewer());
    document.getElementById('profile-save-btn').addEventListener('click', () => saveProfile());
    document.getElementById('profile-cancel-btn').addEventListener('click', () => cancelProfile());
    document.getElementById('create-user-form').addEventListener('submit', (e) => createAdminUser(e));
    document.getElementById('refresh-users-btn').addEventListener('click', () => loadAdminUsers());
    document.getElementById('reindex-search-btn').addEventListener('click', () => reindexSearch());
    document.getElementById('theme-toggle').addEventListener('change', (e) => setTheme(e.target.checked ? 'dark' : 'light'));

    document.querySelectorAll('.toolbar-btn').forEach(btn => {
        btn.addEventListener('click', (e) => handleToolbarClick(e));
    });
    document.getElementById('wiki-markup-select').addEventListener('change', (e) => handleWikiMarkupSelect(e));

    document.addEventListener('click', handleWikiLinkClick);
    window.addEventListener('popstate', () => openRouteFromUrl({ updateUrl: false }));

    if (authToken) {
        await openRouteFromUrl({ updateUrl: false });
    }
});

function initTheme() {
    const theme = localStorage.getItem('wiki-theme') || 'light';
    setTheme(theme, { persist: false });
}

function setTheme(theme, { persist = true } = {}) {
    const normalized = theme === 'dark' ? 'dark' : 'light';
    document.documentElement.dataset.theme = normalized;
    const toggle = document.getElementById('theme-toggle');
    if (toggle) {
        toggle.checked = normalized === 'dark';
    }
    if (persist) {
        localStorage.setItem('wiki-theme', normalized);
    }
}

function showLoginModal() {
    document.body.classList.add('is-authenticating');
    document.getElementById('login-modal').style.display = 'flex';
    document.getElementById('login-username').focus();
}

function hideLoginModal() {
    document.body.classList.remove('is-authenticating');
    document.getElementById('login-modal').style.display = 'none';
}

function showToast(message, tone = 'info') {
    const toast = document.getElementById('toast');
    toast.textContent = message;
    toast.className = `show ${tone}`;
    window.clearTimeout(showToast.timer);
    showToast.timer = window.setTimeout(() => {
        toast.className = '';
    }, 3200);
}

function showOnly(viewId) {
    ['viewer', 'editor', 'version-viewer', 'profile-viewer', 'archive-viewer', 'user-management-viewer', 'welcome'].forEach(id => {
        const view = document.getElementById(id);
        if (!view) return;
        view.style.display = id === viewId ? (id === 'welcome' ? 'block' : 'flex') : 'none';
    });
}

function getUsernameFromAuthToken() {
    if (!authToken) return '';
    try {
        const decoded = atob(authToken);
        return decoded.split(':')[0] || '';
    } catch (e) {
        return '';
    }
}

function getFallbackProfile() {
    const username = getUsernameFromAuthToken();
    return username ? { username, name: '', email: '', description: '', role: 'reader', can_edit: false } : null;
}

function setSidebarProfile(profile) {
    currentProfile = profile;
    document.getElementById('sidebar-username').textContent = profile?.username || '';
    document.getElementById('sidebar-email').textContent = profile?.email || 'No email set';
    document.getElementById('user-management-btn').style.display = isCurrentUserAdmin() ? 'block' : 'none';
    document.getElementById('archived-pages-btn').style.display = isCurrentUserAdmin() ? 'block' : 'none';
    const canEdit = canCurrentUserEdit();
    document.getElementById('new-page-btn').style.display = canEdit ? 'block' : 'none';
    document.getElementById('edit-btn').style.display = canEdit ? 'inline-block' : 'none';
}

function isCurrentUserAdmin() {
    return currentProfile?.role === 'admin';
}

function canCurrentUserEdit() {
    return Boolean(currentProfile?.can_edit || currentProfile?.role === 'admin' || currentProfile?.role === 'editor');
}

function getPageFromUrl() {
    const params = new URLSearchParams(window.location.search);
    return normalizePagePath(params.get('page') || '');
}

function updatePageUrl(pagePath, replace = false) {
    const normalized = normalizePagePath(pagePath);
    const url = new URL(window.location.href);

    if (normalized) {
        url.searchParams.set('page', normalized);
    } else {
        url.searchParams.delete('page');
    }

    if (url.href === window.location.href) return;
    const method = replace ? 'replaceState' : 'pushState';
    window.history[method]({ page: normalized || null }, '', url);
}

async function openRouteFromUrl({ updateUrl = false } = {}) {
    const pagePath = getPageFromUrl();
    if (!pagePath) {
        if (pagePaths.has('home')) {
            await viewPage('home', { updateUrl: false });
            return;
        }
        showOnly('welcome');
        return;
    }

    if (pagePaths.has(pagePath)) {
        await viewPage(pagePath, { updateUrl });
    } else {
        openMissingPage(pagePath, { updateUrl });
    }
}

function normalizePagePath(path) {
    return String(path || '')
        .trim()
        .replace(/\\/g, '/')
        .replace(/^\/+/, '')
        .replace(/\.md$/i, '')
        .replace(/\/+/g, '/');
}

async function handleLogin() {
    const username = document.getElementById('login-username').value.trim();
    const password = document.getElementById('login-password').value;
    document.getElementById('login-error').style.display = 'none';

    if (!username || !password) {
        showLoginError('Username and password required');
        return;
    }

    const credentials = btoa(`${username}:${password}`);

    try {
        const resp = await apiCall(`${API_BASE}/pages`, {
            headers: { 'Authorization': `Basic ${credentials}` },
            skipUnauthorized: true,
        });

        if (!resp.ok) {
            showLoginError('Invalid credentials');
            return;
        }

        authToken = credentials;
        localStorage.setItem('authToken', authToken);
        hideLoginModal();
        document.getElementById('login-username').value = '';
        document.getElementById('login-password').value = '';
        setSidebarProfile({ username, name: '', email: '', description: '', role: 'reader', can_edit: false });
        await loadPageList();
        await loadUserProfile();
        await openRouteFromUrl({ updateUrl: false });
        showToast('Signed in');
    } catch (e) {
        console.error('Login error:', e);
        showLoginError('Login failed');
    }
}

function showLoginError(msg) {
    const err = document.getElementById('login-error');
    err.textContent = msg;
    err.style.display = 'block';
}

function toggleHistory() {
    const toggle = document.getElementById('history-toggle');
    const list = document.getElementById('history-list');

    if (list.style.display === 'none') {
        list.style.display = 'block';
        toggle.textContent = '▼ History';
        toggle.setAttribute('aria-expanded', 'true');
    } else {
        list.style.display = 'none';
        toggle.textContent = '▶ History';
        toggle.setAttribute('aria-expanded', 'false');
    }
}

async function apiCall(url, options = {}) {
    const { skipUnauthorized = false, ...fetchOptions } = options;
    const headers = fetchOptions.headers || {};
    const hasAuthHeader = Boolean(headers.Authorization || headers.authorization);
    const opts = {
        ...fetchOptions,
        headers: {
            ...headers,
            ...(authToken && !hasAuthHeader ? { 'Authorization': `Basic ${authToken}` } : {})
        }
    };

    const resp = await fetch(url, opts);

    if (resp.status === 401 && !skipUnauthorized) {
        authToken = null;
        localStorage.removeItem('authToken');
        setSidebarProfile(null);
        showLoginModal();
        throw new Error('Unauthorized');
    }

    return resp;
}

async function loadPageList() {
    try {
        const resp = await apiCall(`${API_BASE}/pages`);
        if (!resp.ok) return;

        const pages = await resp.json();
        pagePaths = new Set(pages.map(p => p.path));
        const list = document.getElementById('page-list');
        list.innerHTML = '';
        if (pages.length === 0) {
            list.appendChild(createEmptyState('No pages yet.'));
            refreshWikiLinks();
            return;
        }
        list.appendChild(buildTree(pages.map(p => p.path)));
        refreshWikiLinks();
    } catch (e) {
        console.error('Failed to load pages:', e);
        showToast('Failed to load pages', 'error');
    }
}

function buildTree(paths) {
    const tree = {};
    for (const path of [...paths].sort((a, b) => a.localeCompare(b))) {
        const parts = path.split('/');
        let node = tree;
        for (let i = 0; i < parts.length; i++) {
            const part = parts[i];
            if (!node[part]) {
                node[part] = { _path: null, _children: {} };
            }
            if (i === parts.length - 1) {
                node[part]._path = path;
            }
            node = node[part]._children;
        }
    }
    return renderTreeNode(tree, 0, '');
}

function renderTreeNode(node, depth, parentPath) {
    const frag = document.createDocumentFragment();
    for (const [name, data] of Object.entries(node)) {
        const hasChildren = Object.keys(data._children).length > 0;
        const nodePath = parentPath ? `${parentPath}/${name}` : name;
        const wrapper = document.createElement('div');
        wrapper.className = 'tree-item';
        wrapper.style.paddingLeft = `${depth * 14}px`;

        if (hasChildren) {
            const toggle = document.createElement('button');
            const childContainer = document.createElement('div');
            const childId = `tree-group-${nodePath}`.replace(/[^a-zA-Z0-9_-]/g, '-');
            toggle.className = 'tree-toggle';
            toggle.textContent = '▶ ';
            toggle.type = 'button';
            toggle.setAttribute('aria-expanded', 'false');
            toggle.setAttribute('aria-controls', childId);
            toggle.setAttribute('aria-label', `Expand ${name}`);
            childContainer.id = childId;
            childContainer.className = 'tree-children';
            childContainer.style.display = 'none';
            childContainer.appendChild(renderTreeNode(data._children, depth + 1, nodePath));

            toggle.addEventListener('click', (e) => {
                e.stopPropagation();
                const collapsed = childContainer.style.display === 'none';
                childContainer.style.display = collapsed ? 'block' : 'none';
                toggle.textContent = collapsed ? '▼ ' : '▶ ';
                toggle.setAttribute('aria-expanded', String(collapsed));
                toggle.setAttribute('aria-label', `${collapsed ? 'Collapse' : 'Expand'} ${name}`);
            });

            wrapper.appendChild(toggle);

            const label = document.createElement('button');
            label.type = 'button';
            label.className = 'tree-label';
            label.textContent = name;
            if (data._path) {
                label.dataset.path = data._path;
                label.addEventListener('click', () => navigateToPage(data._path));
                if (data._path === currentPage) {
                    label.classList.add('active');
                    label.setAttribute('aria-current', 'page');
                }
            } else {
                label.disabled = true;
            }
            wrapper.appendChild(label);
            frag.appendChild(wrapper);
            frag.appendChild(childContainer);
        } else {
            const label = document.createElement('button');
            label.type = 'button';
            label.className = 'tree-label';
            label.textContent = name;
            if (data._path) {
                label.dataset.path = data._path;
                label.addEventListener('click', () => navigateToPage(data._path));
                if (data._path === currentPage) {
                    label.classList.add('active');
                    label.setAttribute('aria-current', 'page');
                }
            }
            wrapper.appendChild(label);
            frag.appendChild(wrapper);
        }
    }
    return frag;
}

function createEmptyState(message) {
    const div = document.createElement('div');
    div.className = 'empty-state';
    div.textContent = message;
    return div;
}

async function navigateToPage(pagePath) {
    const normalized = normalizePagePath(pagePath);
    if (!normalized) return;

    if (pagePaths.has(normalized)) {
        await viewPage(normalized, { updateUrl: true });
    } else {
        openMissingPage(normalized, { updateUrl: true });
    }
}

async function viewPage(pagePath, { updateUrl = true } = {}) {
    pagePath = normalizePagePath(pagePath);
    try {
        const resp = await apiCall(`${API_BASE}/pages/${encodeURIComponent(pagePath)}`);
        if (!resp.ok) {
            if (resp.status === 404) {
                openMissingPage(pagePath, { updateUrl });
            } else {
                showToast('Failed to load page', 'error');
            }
            return;
        }

        const page = await resp.json();
        currentPage = pagePath;
        currentGitHead = page.current_git_head;
        currentPageContent = page.raw;
        returnPageAfterCancel = null;

        document.getElementById('page-title').textContent = pagePath;
        const contentDiv = document.getElementById('page-content');
        contentDiv.innerHTML = page.content;
        renderWikiLinks(contentDiv);
        contentDiv.scrollTop = 0;

        const historyList = document.getElementById('history-list');
        historyList.innerHTML = '';
        historyList.style.display = 'none';
        const historyToggle = document.getElementById('history-toggle');
        historyToggle.textContent = '▶ History';
        historyToggle.setAttribute('aria-expanded', 'false');

        if (page.history && page.history.length > 0) {
            page.history.forEach(c => {
                const item = document.createElement('button');
                item.type = 'button';
                item.className = 'history-item';
                item.dataset.commit = c.commit_hash;

                const summary = document.createElement('span');
                summary.className = 'history-summary';

                const author = document.createElement('strong');
                author.textContent = c.author || 'unknown';
                summary.appendChild(author);
                summary.appendChild(document.createTextNode(` - ${c.message || 'Update'}`));

                const hash = document.createElement('span');
                hash.className = 'history-hash';
                hash.textContent = c.commit_hash.slice(0, 7);

                item.appendChild(summary);
                item.appendChild(hash);
                item.addEventListener('click', () => viewVersion(c.commit_hash));
                historyList.appendChild(item);
            });
        } else {
            historyList.appendChild(createEmptyState('No history yet.'));
        }

        setActivePageInTree(pagePath);
        if (updateUrl) updatePageUrl(pagePath);
        showViewer();
    } catch (e) {
        console.error('Failed to load page:', e);
        showToast('Failed to load page', 'error');
    }
}

function setActivePageInTree(pagePath) {
    document.querySelectorAll('.tree-label.active').forEach(label => {
        label.classList.remove('active');
        label.removeAttribute('aria-current');
    });

    document.querySelectorAll('.tree-label').forEach(label => {
        if (label.dataset.path === pagePath) {
            label.classList.add('active');
            label.setAttribute('aria-current', 'page');
        }
    });
}

function parseWikiLink(value) {
    const raw = String(value || '').trim();
    if (!raw) return null;

    const parts = raw.split('|');
    let label;
    let target;

    if (parts.length >= 2) {
        label = parts[0].trim();
        target = parts.slice(1).join('|').trim();
    } else {
        target = raw;
        label = raw;
    }

    target = normalizePagePath(target);
    if (!target) return null;

    return {
        label: label || target,
        target,
    };
}

function renderWikiLinks(container) {
    if (!container) return;

    renderWikiDirectives(container);

    const walker = document.createTreeWalker(container, NodeFilter.SHOW_TEXT, {
        acceptNode(node) {
            if (!node.nodeValue || !node.nodeValue.includes('[[')) {
                return NodeFilter.FILTER_REJECT;
            }

            const parent = node.parentElement;
            if (!parent || parent.closest('a, code, pre, textarea, .diff-view')) {
                return NodeFilter.FILTER_REJECT;
            }

            return NodeFilter.FILTER_ACCEPT;
        },
    });

    const nodes = [];
    while (walker.nextNode()) nodes.push(walker.currentNode);

    nodes.forEach(node => {
        const text = node.nodeValue;
        const frag = document.createDocumentFragment();
        const pattern = /\[\[([^\[\]\n]+)\]\]/g;
        let lastIndex = 0;
        let match;

        while ((match = pattern.exec(text)) !== null) {
            const link = parseWikiLink(match[1]);
            if (!link) continue;

            frag.appendChild(document.createTextNode(text.slice(lastIndex, match.index)));
            frag.appendChild(createWikiLink(link.label, link.target));
            lastIndex = match.index + match[0].length;
        }

        if (lastIndex === 0) return;

        frag.appendChild(document.createTextNode(text.slice(lastIndex)));
        node.parentNode.replaceChild(frag, node);
    });

    markWikiLinks(container);
}

function renderWikiDirectives(container) {
    const walker = document.createTreeWalker(container, NodeFilter.SHOW_TEXT, {
        acceptNode(node) {
            if (!node.nodeValue || !node.nodeValue.includes('[[')) {
                return NodeFilter.FILTER_REJECT;
            }

            const parent = node.parentElement;
            if (!parent || parent.closest('a, code, pre, textarea, .diff-view')) {
                return NodeFilter.FILTER_REJECT;
            }

            return NodeFilter.FILTER_ACCEPT;
        },
    });

    const nodes = [];
    while (walker.nextNode()) nodes.push(walker.currentNode);

    nodes.forEach(node => {
        const text = node.nodeValue;
        const frag = document.createDocumentFragment();
        const pattern = /\[\[(toc(?::[^\]\n]+)?|children(?::[^\]\n]+)?|tree(?::[^\]\n]+)?)\]\]/gi;
        let lastIndex = 0;
        let match;

        while ((match = pattern.exec(text)) !== null) {
            const directive = createDirective(match[1], container);
            if (!directive) continue;

            frag.appendChild(document.createTextNode(text.slice(lastIndex, match.index)));
            frag.appendChild(directive);
            lastIndex = match.index + match[0].length;
        }

        if (lastIndex === 0) return;

        frag.appendChild(document.createTextNode(text.slice(lastIndex)));
        const parent = node.parentElement;
        const isStandaloneDirective = parent?.tagName === 'P' && parent.textContent.trim() === text.trim();
        if (isStandaloneDirective) {
            parent.replaceWith(frag);
        } else {
            node.parentNode.replaceChild(frag, node);
        }
    });
}

function createDirective(rawDirective, container) {
    const [namePart, argPart = ''] = rawDirective.split(':');
    const name = namePart.trim().toLowerCase();
    const arg = argPart.trim();

    if (name === 'toc') {
        return createTocDirective(container, arg);
    }

    if (name === 'children') {
        return createPageListDirective(arg || currentPage, false);
    }

    if (name === 'tree') {
        return createPageListDirective(arg || currentPage, true);
    }

    return null;
}

function createTocDirective(container, rangeArg) {
    const { min, max } = parseTocRange(rangeArg);
    const headings = Array.from(container.querySelectorAll('h1, h2, h3, h4, h5, h6'))
        .filter(heading => !heading.closest('.wiki-directive'))
        .map(heading => ({
            element: heading,
            level: Number(heading.tagName.slice(1)),
            text: heading.textContent.trim(),
        }))
        .filter(item => item.text && item.level >= min && item.level <= max);

    const wrapper = document.createElement('nav');
    wrapper.className = 'wiki-directive wiki-toc';
    wrapper.setAttribute('aria-label', 'Table of contents');

    const title = document.createElement('div');
    title.className = 'wiki-directive-title';
    title.textContent = 'Contents';
    wrapper.appendChild(title);

    if (headings.length === 0) {
        wrapper.appendChild(createDirectiveEmpty('No headings on this page.'));
        return wrapper;
    }

    const list = document.createElement('ol');
    headings.forEach(item => {
        item.element.id = item.element.id || uniqueHeadingId(item.text, container);

        const li = document.createElement('li');
        li.style.paddingLeft = `${Math.max(0, item.level - min) * 14}px`;

        const link = document.createElement('a');
        link.href = `#${item.element.id}`;
        link.textContent = item.text;

        li.appendChild(link);
        list.appendChild(li);
    });
    wrapper.appendChild(list);
    return wrapper;
}

function parseTocRange(rangeArg) {
    const trimmed = String(rangeArg || '').trim();
    if (!trimmed) return { min: 2, max: 6 };

    const range = trimmed.match(/^([1-6])\s*-\s*([1-6])$/);
    if (range) {
        const first = Number(range[1]);
        const second = Number(range[2]);
        return {
            min: Math.min(first, second),
            max: Math.max(first, second),
        };
    }

    const single = trimmed.match(/^[1-6]$/);
    if (single) {
        return { min: Number(trimmed), max: 6 };
    }

    return { min: 2, max: 6 };
}

function uniqueHeadingId(text, container) {
    const base = text
        .toLowerCase()
        .replace(/[^a-z0-9]+/g, '-')
        .replace(/^-+|-+$/g, '') || 'heading';
    let id = base;
    let count = 2;

    while (container.querySelector(`#${CSS.escape(id)}`)) {
        id = `${base}-${count}`;
        count += 1;
    }

    return id;
}

function createPageListDirective(rawBasePath, recursive) {
    const basePath = normalizePagePath(rawBasePath || '');
    const pages = getSubPages(basePath, recursive);
    const wrapper = document.createElement('section');
    wrapper.className = `wiki-directive ${recursive ? 'wiki-page-tree' : 'wiki-page-list'}`;

    const title = document.createElement('div');
    title.className = 'wiki-directive-title';
    title.textContent = recursive ? 'Sub-page tree' : 'Sub-pages';
    wrapper.appendChild(title);

    if (pages.length === 0) {
        wrapper.appendChild(createDirectiveEmpty('No sub-pages.'));
        return wrapper;
    }

    wrapper.appendChild(recursive ? buildDirectiveTree(pages, basePath) : buildDirectiveList(pages));
    return wrapper;
}

function getSubPages(basePath, recursive) {
    const prefix = basePath ? `${basePath}/` : '';
    return [...pagePaths]
        .filter(path => path.startsWith(prefix) && path !== basePath)
        .filter(path => recursive || !path.slice(prefix.length).includes('/'))
        .sort((a, b) => a.localeCompare(b));
}

function buildDirectiveList(paths) {
    const list = document.createElement('ul');
    paths.forEach(path => {
        const li = document.createElement('li');
        li.appendChild(createWikiLink(path, path));
        list.appendChild(li);
    });
    return list;
}

function buildDirectiveTree(paths, basePath) {
    const prefix = basePath ? `${basePath}/` : '';
    const root = {};

    paths.forEach(path => {
        const parts = path.slice(prefix.length).split('/').filter(Boolean);
        let node = root;
        parts.forEach((part, index) => {
            node[part] ||= { path: null, children: {} };
            if (index === parts.length - 1) {
                node[part].path = path;
            }
            node = node[part].children;
        });
    });

    return renderDirectiveTreeNode(root);
}

function renderDirectiveTreeNode(node) {
    const list = document.createElement('ul');

    Object.entries(node).forEach(([name, data]) => {
        const li = document.createElement('li');
        if (data.path) {
            li.appendChild(createWikiLink(name, data.path));
        } else {
            const label = document.createElement('span');
            label.className = 'wiki-tree-folder';
            label.textContent = name;
            li.appendChild(label);
        }

        if (Object.keys(data.children).length > 0) {
            li.appendChild(renderDirectiveTreeNode(data.children));
        }

        list.appendChild(li);
    });

    return list;
}

function createDirectiveEmpty(message) {
    const empty = document.createElement('div');
    empty.className = 'wiki-directive-empty';
    empty.textContent = message;
    return empty;
}

function createWikiLink(label, target) {
    const link = document.createElement('a');
    link.href = `?page=${encodeURIComponent(target)}`;
    link.className = 'wiki-link';
    link.dataset.page = target;
    link.textContent = label;
    return link;
}

function markWikiLinks(scope = document) {
    scope.querySelectorAll?.('.wiki-link').forEach(link => {
        const exists = pagePaths.has(link.dataset.page);
        link.classList.toggle('missing', !exists);
        link.title = exists ? `Open ${link.dataset.page}` : `Create ${link.dataset.page}`;
        link.setAttribute('aria-label', exists ? `Open ${link.textContent}` : `Create ${link.textContent}`);
    });
}

function refreshWikiLinks() {
    ['page-content', 'preview', 'version-content'].forEach(id => {
        markWikiLinks(document.getElementById(id));
    });
}

function handleWikiLinkClick(e) {
    const link = e.target.closest?.('.wiki-link');
    if (!link) return;

    e.preventDefault();
    navigateToPage(link.dataset.page);
}

function openMissingPage(pagePath, { updateUrl = true } = {}) {
    if (!canCurrentUserEdit()) {
        showToast('Read-only users cannot create pages', 'error');
        return;
    }

    const normalized = normalizePagePath(pagePath);
    if (!normalized) return;

    returnPageAfterCancel = currentPage || returnPageAfterCancel;
    currentPage = '';
    currentGitHead = null;
    currentPageContent = `# ${titleFromPagePath(normalized)}\n\n`;

    showOnly('editor');
    document.getElementById('conflict-ui').style.display = 'none';
    document.getElementById('page-path-input').value = normalized;
    document.getElementById('content-input').value = currentPageContent;
    updateEditorPageActions();
    updatePreview();
    if (updateUrl) updatePageUrl(normalized);
    showToast(`Create ${normalized}`, 'warning');

    window.setTimeout(() => {
        document.getElementById('content-input').focus();
    }, 0);
}

function titleFromPagePath(pagePath) {
    const lastSegment = normalizePagePath(pagePath).split('/').filter(Boolean).pop() || pagePath;
    return lastSegment
        .replace(/[-_]+/g, ' ')
        .replace(/\b\w/g, char => char.toUpperCase());
}

function enterEditMode() {
    if (!canCurrentUserEdit()) {
        showToast('Read-only users cannot edit pages', 'error');
        return;
    }

    showOnly('editor');
    document.getElementById('conflict-ui').style.display = 'none';

    document.getElementById('page-path-input').value = currentPage || '';
    document.getElementById('content-input').value = currentPageContent || '';
    updateEditorPageActions();
    updatePreview();
    window.setTimeout(() => {
        const field = currentPage ? document.getElementById('content-input') : document.getElementById('page-path-input');
        field.focus();
    }, 0);
}

function newPage() {
    if (!canCurrentUserEdit()) {
        showToast('Read-only users cannot create pages', 'error');
        return;
    }

    returnPageAfterCancel = currentPage || returnPageAfterCancel;
    currentPage = '';
    currentGitHead = null;
    currentPageContent = null;
    document.getElementById('page-path-input').value = '';
    enterEditMode();
}

function updateEditorPageActions() {
    const hasExistingPage = Boolean(currentPage);
    document.getElementById('rename-btn').disabled = !hasExistingPage;
    document.getElementById('archive-btn').disabled = !hasExistingPage;
}

function cancelEdit() {
    if (currentPage) {
        viewPage(currentPage);
    } else if (returnPageAfterCancel) {
        const pageToRestore = returnPageAfterCancel;
        returnPageAfterCancel = null;
        viewPage(pageToRestore);
    } else {
        updatePageUrl('', true);
        showOnly('welcome');
    }
}

async function savePage() {
    if (!canCurrentUserEdit()) {
        showToast('Read-only users cannot save pages', 'error');
        return;
    }

    const path = document.getElementById('page-path-input').value.trim();
    const content = document.getElementById('content-input').value;
    const saveButton = document.getElementById('save-btn');

    if (!path) {
        showToast('Page path required', 'error');
        document.getElementById('page-path-input').focus();
        return;
    }

    try {
        saveButton.disabled = true;
        saveButton.textContent = 'Saving...';
        const resp = await apiCall(`${API_BASE}/pages/${encodeURIComponent(path)}`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                content,
                expected_git_head: currentGitHead || '',
            }),
        });

        if (!resp.ok) {
            showToast('Save failed', 'error');
            return;
        }

        const result = await resp.json();

        if (result.conflict) {
            showConflictUI(path, result.current_content, result.their_changes);
        } else {
            currentPage = path;
            returnPageAfterCancel = null;
            await loadPageList();
            await viewPage(path);
            showToast('Page saved');
        }
    } catch (e) {
        console.error('Failed to save page:', e);
        showToast('Save failed', 'error');
    } finally {
        saveButton.disabled = false;
        saveButton.textContent = 'Save';
    }
}

async function renameCurrentPage() {
    if (!canCurrentUserEdit()) {
        showToast('Read-only users cannot rename pages', 'error');
        return;
    }

    if (!currentPage) return;

    const proposedPath = window.prompt('Rename page to:', currentPage);
    if (proposedPath === null) return;

    const newPath = normalizePagePath(proposedPath);
    if (!newPath) {
        showToast('New page path required', 'error');
        return;
    }

    if (newPath === currentPage) {
        showToast('Page path unchanged');
        return;
    }

    if (pagePaths.has(newPath)) {
        showToast('A page already exists at that path', 'error');
        return;
    }

    try {
        const resp = await apiCall(`${API_BASE}/pages/${encodeURIComponent(currentPage)}/rename`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ new_path: newPath }),
        });

        if (!resp.ok) {
            const message = resp.status === 409
                ? 'A page already exists at that path'
                : 'Rename failed';
            showToast(message, 'error');
            return;
        }

        currentPage = newPath;
        await loadPageList();
        await viewPage(newPath, { updateUrl: true });
        showToast('Page renamed');
    } catch (e) {
        console.error('Failed to rename page:', e);
        showToast('Rename failed', 'error');
    }
}

async function archiveCurrentPage() {
    if (!canCurrentUserEdit()) {
        showToast('Read-only users cannot archive pages', 'error');
        return;
    }

    if (!currentPage) return;

    const confirmed = window.confirm(`Archive "${currentPage}"? Admins can restore it later.`);
    if (!confirmed) return;

    const pageToArchive = currentPage;

    try {
        const resp = await apiCall(`${API_BASE}/pages/${encodeURIComponent(pageToArchive)}/archive`, {
            method: 'POST',
        });

        if (!resp.ok) {
            const message = resp.status === 409
                ? 'This page is already archived'
                : 'Archive failed';
            showToast(message, 'error');
            return;
        }

        currentPage = null;
        currentGitHead = null;
        currentPageContent = null;
        await loadPageList();
        updatePageUrl('', true);
        showOnly('welcome');
        showToast('Page archived');
    } catch (e) {
        console.error('Failed to archive page:', e);
        showToast('Archive failed', 'error');
    }
}

function showConflictUI(path, current, their) {
    document.getElementById('conflict-ui').style.display = 'block';
    document.getElementById('current-content').textContent = current;
    document.getElementById('their-content').textContent = their;
    document.getElementById('resolve-input').value = current;
    showToast(`Resolve changes before saving ${path}`, 'warning');
}

async function resolveConflict() {
    const resolved = document.getElementById('resolve-input').value;
    const path = document.getElementById('page-path-input').value;

    try {
        const resp = await apiCall(`${API_BASE}/resolve`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                path,
                resolved_content: resolved,
                conflict_commit_hash: currentGitHead,
            }),
        });

        if (resp.ok) {
            showToast('Conflict resolved');
            currentPage = path;
            await loadPageList();
            await viewPage(path);
        } else {
            showToast('Failed to resolve conflict', 'error');
        }
    } catch (e) {
        console.error('Failed to resolve conflict:', e);
        showToast('Failed to resolve conflict', 'error');
    }
}

async function searchPages(query) {
    const trimmedQuery = query.trim();
    if (!trimmedQuery) {
        await loadPageList();
        return;
    }

    try {
        const resp = await apiCall(`${API_BASE}/search?q=${encodeURIComponent(trimmedQuery)}`);
        if (!resp.ok) return;

        const results = await resp.json();
        const list = document.getElementById('page-list');
        list.innerHTML = '';

        if (results.length === 0) {
            list.appendChild(createEmptyState(`No results for "${trimmedQuery}".`));
            return;
        }

        results.forEach(result => list.appendChild(createSearchResult(result)));
    } catch (e) {
        console.error('Search failed:', e);
        showToast('Search failed', 'error');
    }
}

function clearSearch() {
    const input = document.getElementById('search-input');
    if (!input.value) return;

    input.value = '';
    updateSearchClearButton();
    loadPageList();
    input.focus();
}

function updateSearchClearButton() {
    const input = document.getElementById('search-input');
    const button = document.getElementById('search-clear-btn');
    const hasQuery = input.value.trim().length > 0;
    button.disabled = !hasQuery;
    button.classList.toggle('visible', hasQuery);
}

function createSearchResult(result) {
    const button = document.createElement('button');
    button.type = 'button';
    button.className = 'page-item';
    button.addEventListener('click', () => navigateToPage(result.path));

    const title = document.createElement('strong');
    title.textContent = result.path;
    const excerpt = document.createElement('small');
    excerpt.textContent = result.excerpt || 'Matching page';

    button.appendChild(title);
    button.appendChild(excerpt);
    return button;
}

let previewTimer = null;
let isSyncingEditorScroll = false;

function initEditorScrollSync() {
    const textarea = document.getElementById('content-input');
    const preview = document.getElementById('preview');

    textarea.addEventListener('scroll', () => syncEditorScroll(textarea, preview));
    preview.addEventListener('scroll', () => syncEditorScroll(preview, textarea));
}

function syncEditorScroll(source, target) {
    if (isSyncingEditorScroll) return;

    const sourceMax = source.scrollHeight - source.clientHeight;
    const targetMax = target.scrollHeight - target.clientHeight;
    if (sourceMax <= 0 || targetMax <= 0) return;

    isSyncingEditorScroll = true;
    const ratio = source.scrollTop / sourceMax;
    target.scrollTop = ratio * targetMax;
    requestAnimationFrame(() => {
        isSyncingEditorScroll = false;
    });
}

function syncPreviewToEditor() {
    syncEditorScroll(
        document.getElementById('content-input'),
        document.getElementById('preview')
    );
}

function updatePreview() {
    clearTimeout(previewTimer);
    previewTimer = setTimeout(async () => {
        const content = document.getElementById('content-input').value;
        try {
            const resp = await apiCall(`${API_BASE}/render`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ content }),
            });
            if (resp.ok) {
                const data = await resp.json();
                const preview = document.getElementById('preview');
                preview.innerHTML = data.html;
                renderWikiLinks(preview);
                syncPreviewToEditor();
            }
        } catch (e) {
            // silent fail on preview
        }
    }, 300);
}

function showViewer() {
    showOnly('viewer');
}

let currentVersionCommit = null;

async function viewVersion(commitHash) {
    try {
        const resp = await apiCall(`${API_BASE}/pages/${encodeURIComponent(currentPage)}/version/${commitHash}`);
        if (!resp.ok) return;

        const data = await resp.json();
        currentVersionCommit = commitHash;

        document.getElementById('version-label').textContent =
            `Version ${commitHash.slice(0, 7)} — restore to revert current page to this version`;
        const versionContent = document.getElementById('version-content');
        versionContent.innerHTML = data.content;
        renderWikiLinks(versionContent);
        document.getElementById('version-diff').innerHTML = renderDiff(data.diff);
        showOnly('version-viewer');
    } catch (e) {
        console.error('Failed to load version:', e);
        showToast('Failed to load version', 'error');
    }
}

function renderDiff(diff) {
    if (!diff || diff.trim() === '') {
        return '<span class="diff-meta">No changes — this is the current version.</span>';
    }

    return diff.split('\n').map(line => {
        const escaped = line.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
        if (line.startsWith('+') && !line.startsWith('+++')) {
            return `<span class="diff-added">${escaped}</span>`;
        } else if (line.startsWith('-') && !line.startsWith('---')) {
            return `<span class="diff-removed">${escaped}</span>`;
        } else if (line.startsWith('@@') || line.startsWith('diff') || line.startsWith('index') || line.startsWith('---') || line.startsWith('+++')) {
            return `<span class="diff-meta">${escaped}</span>`;
        }
        return escaped;
    }).join('\n');
}

async function restoreVersion() {
    if (!currentVersionCommit || !currentPage) return;

    try {
        const resp = await apiCall(
            `${API_BASE}/pages/${encodeURIComponent(currentPage)}/restore/${currentVersionCommit}`,
            { method: 'POST' }
        );

        if (resp.ok) {
            currentVersionCommit = null;
            await loadPageList();
            await viewPage(currentPage);
            showToast('Version restored');
        } else {
            showToast('Failed to restore version', 'error');
        }
    } catch (e) {
        console.error('Failed to restore version:', e);
        showToast('Failed to restore version', 'error');
    }
}

let currentProfile = null;

async function loadUserProfile() {
    try {
        const resp = await apiCall(`${API_BASE}/profile`);
        if (!resp.ok) {
            setSidebarProfile(getFallbackProfile());
            return;
        }
        const profile = await resp.json();
        const fallback = getFallbackProfile() || {};
        setSidebarProfile({ ...fallback, ...profile });

    } catch (e) {
        console.error('Failed to load user profile:', e);
        setSidebarProfile(getFallbackProfile());
    }
}

function showProfileViewer() {
    if (!currentProfile) {
        setSidebarProfile(getFallbackProfile());
    }
    if (!currentProfile) {
        showToast('Profile unavailable', 'error');
        return;
    }

    showOnly('profile-viewer');

    document.getElementById('profile-username').value = currentProfile.username;
    document.getElementById('profile-name').value = currentProfile.name || '';
    document.getElementById('profile-email').value = currentProfile.email || '';
    document.getElementById('profile-description').value = currentProfile.description || '';
}

async function saveProfile() {
    const name = document.getElementById('profile-name').value.trim();
    const email = document.getElementById('profile-email').value.trim();
    const description = document.getElementById('profile-description').value.trim();

    try {
        const resp = await apiCall(`${API_BASE}/profile`, {
            method: 'PUT',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ name, email, description }),
        });

        if (resp.ok) {
            await loadUserProfile();
            cancelProfile();
            showToast('Profile saved');
        } else {
            showToast('Failed to save profile', 'error');
        }
    } catch (e) {
        console.error('Error saving profile:', e);
        showToast('Error saving profile', 'error');
    }
}

function cancelProfile() {
    if (currentPage) {
        showOnly('viewer');
    } else {
        showOnly('welcome');
    }
}

async function showArchiveViewer() {
    if (!isCurrentUserAdmin()) {
        showToast('Admin access required', 'error');
        return;
    }

    showOnly('archive-viewer');
    await loadArchivedPages();
}

async function showUserManagementViewer() {
    if (!isCurrentUserAdmin()) {
        showToast('Admin access required', 'error');
        return;
    }

    showOnly('user-management-viewer');
    await loadAdminUsers();
}

async function loadAdminUsers() {
    const list = document.getElementById('user-management-list');
    list.innerHTML = '';
    list.appendChild(createEmptyState('Loading users...'));

    try {
        const resp = await apiCall(`${API_BASE}/admin/users`);
        if (!resp.ok) {
            showToast('Failed to load users', 'error');
            return;
        }

        const users = await resp.json();
        list.innerHTML = '';

        if (users.length === 0) {
            list.appendChild(createEmptyState('No users.'));
            return;
        }

        users.forEach(user => {
            const item = document.createElement('div');
            item.className = 'admin-list-item';

            const details = document.createElement('div');
            details.className = 'admin-list-details';

            const titleRow = document.createElement('div');
            titleRow.className = 'admin-user-title';

            const username = document.createElement('strong');
            username.textContent = user.username;
            titleRow.appendChild(username);

            const badge = document.createElement('span');
            badge.className = `admin-badge role-${user.role}`;
            badge.textContent = roleLabel(user.role);
            titleRow.appendChild(badge);

            const meta = document.createElement('small');
            const created = user.created_at ? new Date(user.created_at).toLocaleString() : 'Unknown';
            meta.textContent = `Created ${created}`;

            const profile = document.createElement('small');
            profile.textContent = [user.name, user.email].filter(Boolean).join(' - ') || 'No profile details';

            details.appendChild(titleRow);
            details.appendChild(meta);
            details.appendChild(profile);

            const actions = document.createElement('div');
            actions.className = 'admin-list-actions';

            const roleSelect = document.createElement('select');
            roleSelect.className = 'role-select';
            ['admin', 'editor', 'reader'].forEach(role => {
                const option = document.createElement('option');
                option.value = role;
                option.textContent = roleLabel(role);
                option.selected = user.role === role;
                roleSelect.appendChild(option);
            });
            roleSelect.disabled = user.username === currentProfile?.username;
            roleSelect.title = roleSelect.disabled ? 'You cannot change your own role' : 'Change role';
            roleSelect.addEventListener('change', () => setAdminUserRole(user.username, roleSelect.value));

            const passwordButton = document.createElement('button');
            passwordButton.type = 'button';
            passwordButton.className = 'secondary-action-btn';
            passwordButton.textContent = 'Set Password';
            passwordButton.addEventListener('click', () => setAdminUserPassword(user.username));

            const removeButton = document.createElement('button');
            removeButton.type = 'button';
            removeButton.className = 'danger-action-btn';
            removeButton.textContent = 'Remove';
            removeButton.disabled = user.username === currentProfile?.username;
            removeButton.title = removeButton.disabled ? 'You cannot remove your own account' : '';
            removeButton.addEventListener('click', () => removeAdminUser(user.username));

            actions.appendChild(roleSelect);
            actions.appendChild(passwordButton);
            actions.appendChild(removeButton);

            item.appendChild(details);
            item.appendChild(actions);
            list.appendChild(item);
        });
    } catch (e) {
        console.error('Failed to load users:', e);
        showToast('Failed to load users', 'error');
    }
}

async function reindexSearch() {
    const button = document.getElementById('reindex-search-btn');

    try {
        button.disabled = true;
        button.textContent = 'Reindexing...';
        const resp = await apiCall(`${API_BASE}/admin/search/reindex`, {
            method: 'POST',
        });

        if (!resp.ok) {
            showToast('Failed to reindex search', 'error');
            return;
        }

        const result = await resp.json();
        showToast(`Reindexed ${result.indexed_pages} pages`);
    } catch (e) {
        console.error('Failed to reindex search:', e);
        showToast('Failed to reindex search', 'error');
    } finally {
        button.disabled = false;
        button.textContent = 'Reindex Search';
    }
}

function roleLabel(role) {
    if (role === 'admin') return 'Admin';
    if (role === 'reader') return 'Reader';
    return 'Editor';
}

async function createAdminUser(e) {
    e.preventDefault();

    const usernameInput = document.getElementById('new-user-username');
    const passwordInput = document.getElementById('new-user-password');
    const roleInput = document.getElementById('new-user-role');
    const createButton = document.getElementById('create-user-btn');
    const username = usernameInput.value.trim();
    const password = passwordInput.value;

    if (!username || !password) {
        showToast('Username and password required', 'error');
        return;
    }

    try {
        createButton.disabled = true;
        createButton.textContent = 'Creating...';
        const resp = await apiCall(`${API_BASE}/admin/users`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                username,
                password,
                role: roleInput.value,
            }),
        });

        if (!resp.ok) {
            const message = resp.status === 500 ? 'User may already exist' : 'Failed to create user';
            showToast(message, 'error');
            return;
        }

        usernameInput.value = '';
        passwordInput.value = '';
        roleInput.value = 'editor';
        await loadAdminUsers();
        showToast('User created');
    } catch (error) {
        console.error('Failed to create user:', error);
        showToast('Failed to create user', 'error');
    } finally {
        createButton.disabled = false;
        createButton.textContent = 'Create User';
    }
}

async function setAdminUserRole(username, role) {
    if (username === currentProfile?.username) {
        showToast('You cannot change your own role', 'error');
        await loadAdminUsers();
        return;
    }

    try {
        const resp = await apiCall(`${API_BASE}/admin/users/${encodeURIComponent(username)}/role`, {
            method: 'PUT',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ role }),
        });

        if (!resp.ok) {
            showToast('Failed to update role', 'error');
            await loadAdminUsers();
            return;
        }

        await loadAdminUsers();
        showToast('Role updated');
    } catch (e) {
        console.error('Failed to update role:', e);
        showToast('Failed to update role', 'error');
        await loadAdminUsers();
    }
}

async function setAdminUserPassword(username) {
    const password = window.prompt(`New password for "${username}":`);
    if (password === null) return;
    if (!password) {
        showToast('Password required', 'error');
        return;
    }

    try {
        const resp = await apiCall(`${API_BASE}/admin/users/${encodeURIComponent(username)}/password`, {
            method: 'PUT',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ password }),
        });

        if (!resp.ok) {
            showToast('Failed to set password', 'error');
            return;
        }

        showToast('Password updated');
    } catch (e) {
        console.error('Failed to set password:', e);
        showToast('Failed to set password', 'error');
    }
}

async function removeAdminUser(username) {
    if (username === currentProfile?.username) {
        showToast('You cannot remove your own account', 'error');
        return;
    }

    if (!window.confirm(`Remove user "${username}"?`)) return;

    try {
        const resp = await apiCall(`${API_BASE}/admin/users/${encodeURIComponent(username)}`, {
            method: 'DELETE',
        });

        if (!resp.ok) {
            showToast('Failed to remove user', 'error');
            return;
        }

        await loadAdminUsers();
        showToast('User removed');
    } catch (e) {
        console.error('Failed to remove user:', e);
        showToast('Failed to remove user', 'error');
    }
}

async function loadArchivedPages() {
    const list = document.getElementById('archive-list');
    list.innerHTML = '';
    list.appendChild(createEmptyState('Loading archived pages...'));

    try {
        const resp = await apiCall(`${API_BASE}/archive`);
        if (!resp.ok) {
            showToast('Failed to load archived pages', 'error');
            return;
        }

        const pages = await resp.json();
        list.innerHTML = '';

        if (pages.length === 0) {
            list.appendChild(createEmptyState('No archived pages.'));
            return;
        }

        pages.forEach(page => {
            const item = document.createElement('div');
            item.className = 'archive-item';

            const details = document.createElement('div');
            details.className = 'archive-item-details';

            const title = document.createElement('strong');
            title.textContent = page.path;

            const meta = document.createElement('small');
            meta.textContent = page.archived_at ? `Archived ${new Date(page.archived_at).toLocaleString()}` : 'Archived';

            details.appendChild(title);
            details.appendChild(meta);

            const restore = document.createElement('button');
            restore.type = 'button';
            restore.className = 'secondary-action-btn';
            restore.textContent = 'Restore';
            restore.addEventListener('click', () => restoreArchivedPage(page.path));

            item.appendChild(details);
            item.appendChild(restore);
            list.appendChild(item);
        });
    } catch (e) {
        console.error('Failed to load archived pages:', e);
        showToast('Failed to load archived pages', 'error');
    }
}

async function restoreArchivedPage(pagePath) {
    if (!window.confirm(`Restore "${pagePath}"?`)) return;

    try {
        const resp = await apiCall(`${API_BASE}/archive/${encodeURIComponent(pagePath)}/restore`, {
            method: 'POST',
        });

        if (!resp.ok) {
            const message = resp.status === 409
                ? 'A live page already exists at that path'
                : 'Restore failed';
            showToast(message, 'error');
            return;
        }

        await loadPageList();
        await loadArchivedPages();
        showToast('Page restored');
    } catch (e) {
        console.error('Failed to restore archived page:', e);
        showToast('Restore failed', 'error');
    }
}

function handleLogout() {
    authToken = null;
    localStorage.removeItem('authToken');
    currentPage = null;
    currentGitHead = null;
    currentPageContent = null;
    setSidebarProfile(null);
    showOnly('welcome');
    showLoginModal();
}

function handleToolbarClick(e) {
    e.preventDefault();
    insertEditorMarkup(e.currentTarget.dataset.action);
}

function handleWikiMarkupSelect(e) {
    const action = e.target.value;
    if (!action) return;

    insertEditorMarkup(action);
    e.target.value = '';
}

function insertEditorMarkup(action) {
    const textarea = document.getElementById('content-input');
    const start = textarea.selectionStart;
    const end = textarea.selectionEnd;
    const text = textarea.value;
    const selected = text.substring(start, end);
    let replacement = '';

    switch (action) {
        case 'bold':
            replacement = `**${selected || 'bold text'}**`;
            break;
        case 'italic':
            replacement = `*${selected || 'italic text'}*`;
            break;
        case 'heading':
            replacement = `\n## ${selected || 'Heading'}`;
            break;
        case 'code':
            if (selected.includes('\n')) {
                replacement = `\n\`\`\`\n${selected}\n\`\`\`\n`;
            } else {
                replacement = `\`${selected || 'code'}\``;
            }
            break;
        case 'wiki-link':
            replacement = `[[${selected || 'page/path'}]]`;
            break;
        case 'wiki-alias-link':
            replacement = selected ? `[[${selected}|page/path]]` : '[[Link label|page/path]]';
            break;
        case 'children':
            replacement = '\n[[children]]\n';
            break;
        case 'children-prefix':
            replacement = '\n[[children:docs]]\n';
            break;
        case 'tree':
            replacement = '\n[[tree]]\n';
            break;
        case 'tree-prefix':
            replacement = '\n[[tree:docs]]\n';
            break;
        case 'toc':
            replacement = '\n[[toc]]\n';
            break;
        case 'toc-range':
            replacement = '\n[[toc:2-4]]\n';
            break;
        case 'link':
            replacement = `[${selected || 'link text'}](https://example.com)`;
            break;
        default:
            return;
    }

    textarea.value = text.substring(0, start) + replacement + text.substring(end);
    textarea.focus();
    textarea.selectionStart = start;
    textarea.selectionEnd = start + replacement.length;
    updatePreview();
}
