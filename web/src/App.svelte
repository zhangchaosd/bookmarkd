<script lang="ts">
  import { onMount } from 'svelte';
  import { request, passkey, setCSRF, csrf, ApiError } from './api';
  type Bookmark = {
    id: string;
    title: string;
    url_raw: string;
    notes: string;
    folder_id: string | null;
    tags: string[];
    pinned: boolean;
    version: number;
    created_at: number;
  };
  type Folder = {
    id: string;
    name: string;
    parent_id: string | null;
    version: number;
  };
  let ready = false,
    authenticated = false,
    busy = false,
    error = '',
    notice = '',
    remember = false;
  let setup = location.pathname === '/setup',
    token = '',
    verified = false;
  let view =
    location.pathname === '/trash'
      ? 'trash'
      : location.pathname.startsWith('/settings')
        ? 'settings'
        : 'library';
  let bookmarks: Bookmark[] = [],
    folders: Folder[] = [],
    trashedFolders: Folder[] = [],
    query = '',
    folder = 'all',
    tag = '',
    total = 0,
    revision = -1,
    offset = 0;
  let organize = false,
    selected: string[] = [],
    mobileNav = false,
    showEditor = false,
    editId = '',
    editVersion = 0,
    title = '',
    url = '',
    notes = '',
    tags = '',
    draftFolder = '',
    pinned = false;
  let editKey = '',
    conflicts: any = null,
    folderName = '',
    folderParent = '',
    folderEdit = '',
    folderVersion = 0;
  let importJob: any = null,
    importFormat = 'html',
    importMode = 'merge',
    keepDuplicates = false,
    importTarget = '',
    importContent = '';
  let sessions: any[] = [],
    credentials: any[] = [],
    preferences: any = {
      version: 1,
      theme: 'system',
      density: 'comfortable',
      new_tab: true,
    };
  let batchFolder = '',
    batchTags = '',
    sessionId = '',
    channel: BroadcastChannel | undefined;
  let searchTimer: ReturnType<typeof setTimeout>;
  $: document.documentElement.dataset.theme = preferences.theme;
  $: document.documentElement.dataset.density = preferences.density;
  $: bookmarklet = `javascript:(()=>{const u=new URL('/capture',${JSON.stringify(location.origin)});u.searchParams.set('url',location.href);u.searchParams.set('title',document.title);window.open(u,'_blank','noopener,noreferrer')})()`;
  function focusDialog(node: HTMLDialogElement) {
    node.showModal();
    const close = () => {
      showEditor = false;
    };
    node.addEventListener('cancel', close);
    return {
      destroy() {
        node.removeEventListener('cancel', close);
      },
    };
  }
  function report(e: unknown) {
    if (e instanceof DOMException && e.name === 'NotAllowedError')
      error = '已取消，请重新点击登录';
    else error = e instanceof Error ? e.message : String(e);
    if (e instanceof ApiError && e.code === 'UNAUTHENTICATED') clearPrivate();
    if (e instanceof ApiError && e.code === 'VERSION_CONFLICT')
      conflicts = e.details;
  }
  async function run(f: () => Promise<void>) {
    error = '';
    busy = true;
    try {
      await f();
    } catch (e) {
      report(e);
    } finally {
      busy = false;
    }
  }
  function clearPrivate() {
    authenticated = false;
    bookmarks = [];
    folders = [];
    trashedFolders = [];
    sessions = [];
    credentials = [];
    selected = [];
    importJob = null;
    setCSRF('');
  }
  async function check() {
    const s = await request('/api/v1/session');
    authenticated = s.authenticated;
    if (authenticated) {
      setCSRF(s.csrf_token);
      sessionId = s.session.id;
    } else clearPrivate();
    ready = true;
  }
  async function load() {
    if (!authenticated) return;
    folders = await request('/api/v1/folders');
    if (view === 'trash') {
      const t = await request('/api/v1/trash');
      bookmarks = t.bookmarks;
      trashedFolders = t.folders;
      total = bookmarks.length;
    } else if (view === 'settings') {
      [sessions, credentials, preferences] = await Promise.all([
        request('/api/v1/auth/sessions'),
        request('/api/v1/auth/credentials'),
        request('/api/v1/preferences'),
      ]);
    } else {
      const params = new URLSearchParams({
        q: query,
        limit: '100',
        offset: String(offset),
      });
      if (folder !== 'all' && folder !== 'pinned')
        params.set('folder_id', folder);
      if (folder === 'pinned') params.set('pinned', 'true');
      if (tag) params.set('tag', tag);
      const r = await request('/api/v1/bookmarks?' + params);
      bookmarks = r.items;
      total = r.total;
      revision = r.revision;
    }
  }
  async function nav(next: string, f = 'all') {
    view = next;
    folder = f;
    offset = 0;
    selected = [];
    mobileNav = false;
    history.replaceState(
      null,
      '',
      next === 'library' ? '/library' : '/' + next
    );
    await run(load);
  }
  function search() {
    clearTimeout(searchTimer);
    searchTimer = setTimeout(() => {
      offset = 0;
      run(load);
    }, 250);
  }
  function edit(b?: Bookmark) {
    showEditor = true;
    conflicts = null;
    editKey = crypto.randomUUID();
    editId = b?.id || '';
    editVersion = b?.version || 0;
    title = b?.title || '';
    url = b?.url_raw || '';
    notes = b?.notes || '';
    tags = b?.tags.join(', ') || '';
    draftFolder =
      b?.folder_id || (folder !== 'all' && folder !== 'pinned' ? folder : '');
    pinned = b?.pinned || false;
  }
  function capture() {
    const p = new URLSearchParams(location.search);
    if (location.pathname === '/capture') {
      const draft = sessionStorage.getItem('bookmarkd-capture');
      const d = draft
        ? JSON.parse(draft)
        : { url: p.get('url') || '', title: p.get('title') || '' };
      sessionStorage.setItem('bookmarkd-capture', JSON.stringify(d));
      if (authenticated) {
        edit();
        url = d.url;
        title = d.title;
      }
    }
  }
  async function save() {
    await request(
      '/api/v1/bookmarks' + (editId ? '/' + editId : ''),
      editId ? 'PATCH' : 'POST',
      {
        version: editVersion,
        title,
        url_raw: url,
        notes,
        folder_id: draftFolder || null,
        tags: tags
          .split(/[,，]/)
          .map((t) => t.trim())
          .filter(Boolean),
        pinned,
      },
      editKey
    );
    showEditor = false;
    sessionStorage.removeItem('bookmarkd-capture');
    notice = '已保存';
    await load();
  }
  async function login() {
    await passkey('login', remember);
    await check();
    await load();
    capture();
  }
  async function logout() {
    await request('/logout', 'POST', {});
    clearPrivate();
    sessionStorage.removeItem('bookmarkd-capture');
    showEditor = false;
    title = url = notes = tags = '';
    channel?.postMessage('logout');
    history.replaceState(null, '', '/login');
  }
  async function sensitive(f: () => Promise<void>) {
    try {
      await f();
    } catch (e) {
      if (e instanceof ApiError && e.code === 'RECENT_AUTH_REQUIRED') {
        await passkey('reauth');
        await f();
      } else throw e;
    }
  }
  async function change(b: Bookmark, action: 'delete' | 'restore' | 'pin') {
    if (action === 'delete')
      await request('/api/v1/bookmarks/' + b.id, 'DELETE', {
        version: b.version,
      });
    else if (action === 'restore')
      await request(`/api/v1/bookmarks/${b.id}/restore`, 'POST', {
        version: b.version,
        folder_id: folders.some((f) => f.id === b.folder_id)
          ? b.folder_id
          : null,
      });
    else
      await request('/api/v1/bookmarks/' + b.id, 'PATCH', {
        version: b.version,
        pinned: !b.pinned,
      });
    await load();
  }
  async function batch(action: string) {
    await request(
      '/api/v1/bookmarks/batch',
      'POST',
      {
        action,
        items: bookmarks
          .filter((b) => selected.includes(b.id))
          .map((b) => ({ id: b.id, version: b.version })),
        folder_id: batchFolder || null,
        tags: batchTags
          .split(/[,，]/)
          .map((s) => s.trim())
          .filter(Boolean),
      },
      crypto.randomUUID()
    );
    selected = [];
    await load();
  }
  async function moveUp(b: Bookmark) {
    const items = [...bookmarks];
    const n = items.findIndex((x) => x.id === b.id);
    if (n <= 0) return;
    [items[n], items[n - 1]] = [items[n - 1], items[n]];
    await request('/api/v1/bookmarks/reorder', 'POST', {
      scope: folder === 'pinned' ? 'pinned' : 'folder',
      items: items.map((b) => ({ id: b.id, version: b.version })),
    });
    await load();
  }
  function folderPath(f: Folder): string {
    const parent = folders.find((x) => x.id === f.parent_id);
    return parent ? folderPath(parent) + ' / ' + f.name : f.name;
  }
  async function saveFolder() {
    await request(
      '/api/v1/folders' + (folderEdit ? '/' + folderEdit : ''),
      folderEdit ? 'PATCH' : 'POST',
      {
        name: folderName,
        parent_id: folderParent || null,
        version: folderVersion,
      }
    );
    folderName = folderEdit = folderParent = '';
    await load();
  }
  async function deleteFolder(f: Folder, strategy: string) {
    if (
      !confirm(
        `删除目录“${f.name}”及其子目录？${strategy === 'trash' ? '内容移入回收站。' : '内容移至收件箱。'}`
      )
    )
      return;
    await request('/api/v1/folders/' + f.id, 'DELETE', {
      version: f.version,
      strategy,
      folder_id: null,
    });
    folder = 'all';
    await load();
  }
  async function importFile(e: Event) {
    const file = (e.currentTarget as HTMLInputElement).files?.[0];
    if (!file) return;
    if (file.size > 20 * 1024 * 1024) throw new Error('文件不得超过 20 MiB');
    importFormat = file.name.toLowerCase().endsWith('.json') ? 'json' : 'html';
    importContent = await file.text();
    importJob = await request('/api/v1/imports', 'POST', {
      format: importFormat,
      content: importContent,
    });
  }
  async function commitImport() {
    const result = await request(
      `/api/v1/imports/${importJob.id}/commit`,
      'POST',
      {
        folder_id: importTarget || null,
        keep_duplicates: keepDuplicates,
        mode: importMode,
      },
      importJob.id
    );
    notice = `导入完成：新增 ${result.added}，跳过 ${result.skipped}`;
    importJob = null;
    importContent = '';
    await load();
  }
  async function exportFile(format: string) {
    await sensitive(async () => {
      const response = await fetch('/api/v1/exports', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', 'X-CSRF-Token': csrf },
        body: JSON.stringify({ format, includes_trash: true }),
      });
      if (!response.ok) {
        const e = await response.json();
        throw new ApiError(e.error.code, e.error.message, e.error.details);
      }
      const href = URL.createObjectURL(await response.blob());
      const a = document.createElement('a');
      a.href = href;
      a.download = `bookmarkd.${format}`;
      a.click();
      setTimeout(() => URL.revokeObjectURL(href), 1000);
    });
  }
  async function savePrefs() {
    preferences = await request('/api/v1/preferences', 'PATCH', preferences);
    notice = '偏好已保存';
  }
  onMount(() => {
    channel = new BroadcastChannel('bookmarkd');
    channel.onmessage = () => {
      clearPrivate();
      showEditor = false;
      title = url = notes = tags = '';
    };
    run(async () => {
      await check();
      capture();
      if (authenticated) {
        preferences = await request('/api/v1/preferences');
        await load();
      }
    });
    const focus = () =>
      run(async () => {
        await check();
        if (authenticated) {
          const r = await request('/api/v1/library/revision');
          if (r.revision !== revision) await load();
        }
      });
    const pagehide = () => {
      ready = false;
    };
    const pageshow = () => {
      if (!ready) focus();
    };
    window.addEventListener('focus', focus);
    window.addEventListener('pagehide', pagehide);
    window.addEventListener('pageshow', pageshow);
    return () => {
      channel?.close();
      window.removeEventListener('focus', focus);
      window.removeEventListener('pagehide', pagehide);
      window.removeEventListener('pageshow', pageshow);
      clearTimeout(searchTimer);
    };
  });
</script>

{#if !ready}<main class="auth"><p>正在检查会话…</p></main>
{:else if setup}
  <main class="auth">
    <div class="brandmark">◆</div>
    <h1>初始化收藏中心</h1>
    <p class="muted">使用服务器管理终端提供的一次性授权。</p>
    <form
      on:submit|preventDefault={() =>
        run(async () => {
          if (!verified) {
            await request('/auth/setup/verify', 'POST', { token });
            token = '';
            verified = true;
          }
          await passkey('register');
          location.href = '/login';
        })}
    >
      <label
        >Setup Token<input
          type="password"
          bind:value={token}
          required={!verified}
          autocomplete="off"
          disabled={verified}
        /></label
      ><button class="primary" disabled={busy}
        >{verified ? '创建 Passkey' : '验证并创建 Passkey'}</button
      >
    </form>
    <p role="alert" class="error">{error}</p>
  </main>
{:else if !authenticated}
  <main class="auth">
    <div class="brandmark" aria-hidden="true">◆</div>
    <h1>私人收藏中心</h1>
    <p class="muted">好内容，随时找得到。</p>
    <button class="primary login" disabled={busy} on:click={() => run(login)}
      >{busy ? '等待 Passkey 验证…' : '使用 Passkey 登录'}</button
    ><label class="remember"
      ><input type="checkbox" bind:checked={remember} /> 在此设备保持登录 30 天</label
    >
    <p role="alert" class="error">{error}</p>
  </main>
{:else}
  <div class="shell">
    <aside class:open={mobileNav}>
      <a class="brand" href="/"
        >◆ <strong>收藏中心</strong><small>BOOKMARKD</small></a
      >
      <nav aria-label="收藏导航">
        <button
          class:active={view === 'library' && folder === 'all'}
          on:click={() => nav('library')}>▦ 全部收藏</button
        ><button
          class:active={view === 'library' && folder === 'pinned'}
          on:click={() => nav('library', 'pinned')}>☆ 常用置顶</button
        ><button
          class:active={view === 'library' && folder === ''}
          on:click={() => nav('library', '')}>▣ 收件箱</button
        >
        <div class="nav-label">文件夹</div>
        {#each folders as f}<button
            class:active={folder === f.id}
            title={folderPath(f)}
            on:click={() => nav('library', f.id)}>▱ {folderPath(f)}</button
          >{/each}
        <div class="nav-label">管理</div>
        <button class:active={view === 'trash'} on:click={() => nav('trash')}
          >♲ 回收站</button
        ><button
          class:active={view === 'settings'}
          on:click={() => nav('settings')}>⚙ 设置与迁移</button
        >
      </nav>
      <div class="aside-footer">
        <span class="status-dot"></span> 私有 · 自托管<button
          on:click={() => run(logout)}>退出登录</button
        >
      </div>
    </aside>
    <main class="workspace">
      <header>
        <button
          class="mobile-toggle"
          aria-label="打开导航"
          on:click={() => (mobileNav = !mobileNav)}>☰</button
        >
        <div>
          <p class="eyebrow">YOUR PERSONAL LIBRARY</p>
          <h1>
            {view === 'trash'
              ? '回收站'
              : view === 'settings'
                ? '设置与迁移'
                : folder === 'pinned'
                  ? '常用置顶'
                  : folder === ''
                    ? '收件箱'
                    : folders.find((f) => f.id === folder)?.name || '全部收藏'}
          </h1>
        </div>
        {#if view === 'library'}<button class="primary" on:click={() => edit()}
            >＋ 添加收藏</button
          >{/if}
      </header>
      {#if error}<div class="banner error" role="alert">
          {error}
        </div>{/if}{#if notice}<div class="banner" role="status">
          {notice}<button aria-label="关闭提示" on:click={() => (notice = '')}
            >×</button
          >
        </div>{/if}
      {#if view === 'settings'}<div class="settings-grid">
          <section class="panel">
            <h2>浏览偏好</h2>
            <label
              >主题<select bind:value={preferences.theme}
                ><option value="system">跟随系统</option><option value="light"
                  >浅色</option
                ><option value="dark">深色</option></select
              ></label
            ><label
              >显示密度<select bind:value={preferences.density}
                ><option value="comfortable">舒适</option><option
                  value="compact">紧凑</option
                ></select
              ></label
            ><label class="inline"
              ><input type="checkbox" bind:checked={preferences.new_tab} /> 新标签页打开链接</label
            ><button on:click={() => run(savePrefs)}>保存偏好</button>
          </section>
          <section class="panel">
            <h2>文件夹管理</h2>
            <form on:submit|preventDefault={() => run(saveFolder)}>
              <label
                >名称<input
                  bind:value={folderName}
                  required
                  maxlength="128"
                /></label
              ><label
                >上级目录<select bind:value={folderParent}
                  ><option value="">根目录</option
                  >{#each folders.filter((f) => f.id !== folderEdit) as f}<option
                      value={f.id}>{folderPath(f)}</option
                    >{/each}</select
                ></label
              ><button disabled={busy}
                >{folderEdit ? '保存目录' : '创建目录'}</button
              >{#if folderEdit}<button
                  type="button"
                  on:click={() => {
                    folderEdit = folderName = folderParent = '';
                  }}>取消</button
                >{/if}
            </form>
            {#each folders as f}<div class="setting-row">
                <span>{folderPath(f)}</span><button
                  on:click={() => {
                    folderEdit = f.id;
                    folderName = f.name;
                    folderParent = f.parent_id || '';
                    folderVersion = f.version;
                  }}>编辑</button
                ><button on:click={() => run(() => deleteFolder(f, 'move'))}
                  >移至收件箱</button
                ><button
                  class="danger"
                  on:click={() => run(() => deleteFolder(f, 'trash'))}
                  >删除</button
                >
              </div>{/each}
          </section>
          <section class="panel">
            <h2>导入收藏</h2>
            <p class="muted">
              支持浏览器 HTML 与完整业务 JSON。先预览，确认后写入。
            </p>
            <label
              >选择 UTF-8 文件<input
                type="file"
                accept=".html,.htm,.json"
                on:change={(e) => run(() => importFile(e))}
              /></label
            >{#if importJob}<p>
                识别 {importJob.parsed} 条收藏、{importJob.folders} 个目录
              </p>
              {#each importJob.warnings as warning}<p class="error">
                  {warning}
                </p>{/each}<label
                >目标目录<select bind:value={importTarget}
                  ><option value="">根目录 / 收件箱</option
                  >{#each folders as f}<option value={f.id}
                      >{folderPath(f)}</option
                    >{/each}</select
                ></label
              ><label
                >导入方式<select bind:value={importMode}
                  ><option value="merge">合并（重新映射 ID）</option><option
                    value="restore">完整恢复（仅空库，保留 ID 和偏好）</option
                  ></select
                ></label
              ><label class="inline"
                ><input type="checkbox" bind:checked={keepDuplicates} /> 保留完全重复条目</label
              ><button
                class="primary"
                disabled={busy}
                on:click={() => run(commitImport)}>确认导入</button
              ><button
                on:click={() =>
                  run(async () => {
                    await request(
                      '/api/v1/imports/' + importJob.id,
                      'DELETE',
                      {}
                    );
                    importJob = null;
                  })}>取消</button
              >{/if}
          </section>
          <section class="panel">
            <h2>导出与快速收藏</h2>
            <p class="muted">
              JSON 包含完整业务字段和回收站；HTML 用于浏览器迁移。
            </p>
            <div class="actions">
              <button on:click={() => run(() => exportFile('json'))}
                >导出 JSON</button
              ><button on:click={() => run(() => exportFile('html'))}
                >导出 HTML</button
              >
            </div>
            <h3>Bookmarklet</h3>
            <p class="muted">
              将以下代码保存为浏览器书签的网址。在网页点击该书签后，确认保存。
            </p>
            <textarea
              aria-label="Bookmarklet 代码"
              readonly
              value={bookmarklet}
              rows="4"></textarea><button
              on:click={() =>
                run(async () => {
                  await navigator.clipboard.writeText(bookmarklet);
                  notice = 'Bookmarklet 已复制';
                })}>复制 Bookmarklet</button
            >
          </section>
          <section class="panel">
            <h2>Passkey 与登录设备</h2>
            <p class="muted">
              添加或撤销凭据请使用服务器管理终端。敏感操作可能要求再次验证。
            </p>
            {#each credentials as c}<div class="setting-row">
                <span>⌘ {c.name}</span><button
                  on:click={() =>
                    run(async () => {
                      const name = prompt('凭据显示名称', c.name);
                      if (name)
                        await sensitive(async () => {
                          await request(
                            '/api/v1/auth/credentials/' + c.id,
                            'PATCH',
                            { name }
                          );
                        });
                      await load();
                    })}>重命名</button
                >
              </div>{/each}{#each sessions as s}<div class="setting-row">
                <span
                  >{s.id === sessionId ? '当前设备' : '其他会话'}<small
                    >到期 {new Date(
                      s.expires_at * 1000
                    ).toLocaleString()}</small
                  ></span
                >{#if s.id !== sessionId}<button
                    on:click={() =>
                      run(() =>
                        sensitive(async () => {
                          await request(
                            '/api/v1/auth/sessions/' + s.id,
                            'DELETE',
                            {}
                          );
                          await load();
                        })
                      )}>撤销</button
                  >{/if}
              </div>{/each}<button
              on:click={() =>
                run(() =>
                  sensitive(async () => {
                    await request(
                      '/api/v1/auth/sessions/revoke-others',
                      'POST',
                      {}
                    );
                    await load();
                  })
                )}>撤销其他会话</button
            >
          </section>
        </div>
      {:else}<div class="toolbar">
          {#if view === 'library'}<div class="search">
              <span aria-hidden="true">⌕</span><input
                type="search"
                aria-label="搜索收藏"
                placeholder="搜索标题、网址、备注或标签…"
                bind:value={query}
                on:input={search}
              /><kbd>搜索</kbd>
            </div>
            <button
              class:active={organize}
              on:click={() => {
                organize = !organize;
                selected = [];
              }}>{organize ? '完成整理' : '整理'}</button
            >{:else}<p class="muted">
              删除的收藏会保留在这里，直到你主动清空。
            </p>
            <button
              class="danger"
              on:click={() =>
                run(async () => {
                  if (confirm('永久清空回收站？此操作不可撤销。'))
                    await sensitive(async () => {
                      await request('/api/v1/trash/purge', 'POST', {
                        confirm: true,
                      });
                      await load();
                    });
                })}>清空回收站</button
            >{/if}
        </div>
        {#if tag}<button
            on:click={() => {
              tag = '';
              run(load);
            }}>标签：{tag} ×</button
          >{/if}
        {#if organize}<div class="batch">
            <label class="inline"
              ><input
                type="checkbox"
                checked={selected.length === bookmarks.length &&
                  bookmarks.length > 0}
                on:change={(e) =>
                  (selected = e.currentTarget.checked
                    ? bookmarks.map((b) => b.id)
                    : [])}
              /> 本页全选</label
            ><span>{selected.length} 项</span><select
              aria-label="批量目标目录"
              bind:value={batchFolder}
              ><option value="">收件箱</option>{#each folders as f}<option
                  value={f.id}>{folderPath(f)}</option
                >{/each}</select
            ><button
              disabled={!selected.length}
              on:click={() => run(() => batch('move'))}>移动</button
            ><input
              aria-label="批量标签"
              placeholder="标签，以逗号分隔"
              bind:value={batchTags}
            /><button
              disabled={!selected.length}
              on:click={() => run(() => batch('add-tags'))}>加标签</button
            ><button
              disabled={!selected.length}
              on:click={() => run(() => batch('remove-tags'))}>减标签</button
            ><button
              disabled={!selected.length}
              class="danger"
              on:click={() => run(() => batch('delete'))}>删除</button
            >
          </div>{/if}
        <div class="list-heading">
          <span>{total} 条收藏</span><span
            >{folder === 'all'
              ? '全部目录'
              : folder === 'pinned'
                ? '手动排序'
                : '当前目录'}</span
          >
        </div>
        <div class="bookmark-list">
          {#each bookmarks as b (b.id)}<article class="bookmark">
              {#if organize}<input
                  aria-label={'选择 ' + b.title}
                  type="checkbox"
                  value={b.id}
                  bind:group={selected}
                />{/if}
              <div class="site-icon" aria-hidden="true">
                {b.title.slice(0, 1).toUpperCase()}
              </div>
              <div class="bookmark-content">
                <a
                  href={b.url_raw}
                  target={preferences.new_tab ? '_blank' : '_self'}
                  rel="noopener noreferrer"
                  >{b.title}{#if b.pinned}<span class="pin" aria-label="已置顶">
                      ★</span
                    >{/if}</a
                >
                <p class="url">{b.url_raw}</p>
                {#if b.notes}<p class="notes">{b.notes}</p>{/if}
                <div class="tags">
                  {#each b.tags as t}<button
                      on:click={() => {
                        tag = t;
                        run(load);
                      }}>{t}</button
                    >{/each}{#if b.folder_id}<span class="folder-caption"
                      >{folders.find((f) => f.id === b.folder_id)?.name ||
                        '原目录已删除'}</span
                    >{/if}
                </div>
              </div>
              <div class="item-actions">
                {#if view === 'trash'}<button
                    on:click={() => run(() => change(b, 'restore'))}
                    >恢复</button
                  >{:else}<button
                    title="复制网址"
                    aria-label={'复制 ' + b.title}
                    on:click={() =>
                      run(async () => {
                        await navigator.clipboard.writeText(b.url_raw);
                        notice = '网址已复制';
                      })}>复制</button
                  ><button title="编辑收藏" on:click={() => edit(b)}
                    >编辑</button
                  >{#if organize}<button
                      on:click={() => run(() => change(b, 'pin'))}
                      >{b.pinned ? '取消置顶' : '置顶'}</button
                    ><button on:click={() => run(() => moveUp(b))}>上移</button
                    ><button
                      class="danger"
                      on:click={() => run(() => change(b, 'delete'))}
                      >删除</button
                    >{/if}{/if}
              </div>
            </article>{/each}
        </div>
        {#if bookmarks.length === 0}<div class="empty">
            <span>◇</span>
            <h2>
              {query
                ? '没有找到匹配的收藏'
                : view === 'trash'
                  ? '回收站为空'
                  : '给好内容留个位置'}
            </h2>
            <p>
              {query
                ? '试试更短的关键词，或切换目录。'
                : '保存网址，整理想法，在任何设备继续探索。'}
            </p>
            {#if view === 'library'}<button
                class="primary"
                on:click={() => edit()}>添加第一条收藏</button
              >{/if}
          </div>{/if}
        {#if view === 'trash'}{#each trashedFolders as f}<div
              class="setting-row"
            >
              <span>▱ {f.name}</span><button
                on:click={() =>
                  run(async () => {
                    await request(`/api/v1/folders/${f.id}/restore`, 'POST', {
                      version: f.version,
                      parent_id: null,
                    });
                    await load();
                  })}>恢复目录及本次删除内容</button
              >
            </div>{/each}{/if}
        {#if total > 100 && view === 'library'}<div class="pagination">
            <button
              disabled={offset === 0}
              on:click={() => {
                offset -= 100;
                run(load);
              }}>上一页</button
            ><span>{offset + 1}–{Math.min(offset + 100, total)}</span><button
              disabled={offset + 100 >= total}
              on:click={() => {
                offset += 100;
                run(load);
              }}>下一页</button
            >
          </div>{/if}{/if}
      <footer>自己的收藏，自己的数据。</footer>
    </main>
  </div>
  {#if showEditor}<div class="overlay">
      <dialog use:focusDialog aria-labelledby="editor-title" class="editor">
        <div class="dialog-heading">
          <h2 id="editor-title">{editId ? '编辑收藏' : '添加收藏'}</h2>
          <button aria-label="关闭编辑器" on:click={() => (showEditor = false)}
            >×</button
          >
        </div>
        <form on:submit|preventDefault={() => run(save)}>
          <label
            >网址 <span class="required">*</span><input
              type="url"
              bind:value={url}
              required
              maxlength="16384"
              placeholder="https://example.com"
            /></label
          ><label
            >标题<input
              bind:value={title}
              maxlength="512"
              placeholder="留空时使用域名"
            /></label
          >
          <div class="form-grid">
            <label
              >文件夹<select bind:value={draftFolder}
                ><option value="">收件箱</option>{#each folders as f}<option
                    value={f.id}>{folderPath(f)}</option
                  >{/each}</select
              ></label
            ><label
              >标签<input
                bind:value={tags}
                placeholder="阅读, 工具, 灵感"
              /></label
            >
          </div>
          <label
            >备注<textarea
              bind:value={notes}
              rows="4"
              maxlength="16384"
              placeholder="为什么值得收藏？"></textarea></label
          ><label class="inline"
            ><input type="checkbox" bind:checked={pinned} /> 置顶到常用收藏</label
          >{#if error}<p class="error" role="alert">
              {error}
            </p>{/if}{#if conflicts}<p>
              你的输入已保留。可复制当前内容后重新打开最新版本。
            </p>
            <button
              type="button"
              on:click={() => {
                editVersion = conflicts.version;
                conflicts = null;
                error = '';
              }}>以最新版本为基础重试当前输入</button
            >{/if}
          <div class="dialog-footer">
            <button type="button" on:click={() => (showEditor = false)}
              >取消</button
            ><button class="primary" disabled={busy}>保存收藏</button>
          </div>
        </form>
      </dialog>
    </div>{/if}
{/if}
