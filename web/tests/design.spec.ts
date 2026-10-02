import { test, expect, type Page } from '@playwright/test';
import { spawn, execFileSync, type ChildProcess } from 'node:child_process';
import { mkdtempSync, rmSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve, join } from 'node:path';

const binary = resolve(
  process.env.BOOKMARKD_BIN || '../target/debug/bookmarkd'
);
const origin = 'http://localhost:8777';
let data: string, config: string, server: ChildProcess | undefined;

test.use({
  baseURL: origin,
  viewport: { width: 1440, height: 1000 },
  colorScheme: 'light',
  reducedMotion: 'reduce',
  actionTimeout: 15000,
});

test.beforeAll(async () => {
  data = mkdtempSync(join(tmpdir(), 'bookmarkd-design-'));
  config = join(data, 'config.toml');
  execFileSync(binary, [
    'init',
    '--data-dir',
    data,
    '--public-url',
    origin,
    '--rp-id',
    'localhost',
    '--allow-insecure-localhost',
  ]);
  writeFileSync(
    config,
    readFileSync(config, 'utf8').replace('127.0.0.1:8765', '127.0.0.1:8777')
  );
  server = spawn(binary, ['--config', config, 'serve'], { stdio: 'ignore' });
  await expect
    .poll(
      async () => {
        if (server?.exitCode !== null) return 0;
        try {
          return (await fetch(origin + '/healthz')).status;
        } catch {
          return 0;
        }
      },
      { message: 'The isolated design-test server should become healthy' }
    )
    .toBe(200);
});

test.afterAll(async () => {
  if (server && server.exitCode === null) {
    const stopped = new Promise<void>((resolve) =>
      server!.once('exit', () => resolve())
    );
    server.kill();
    await stopped;
  }
  if (data) rmSync(data, { recursive: true, force: true });
});

async function api(page: Page, path: string, method = 'GET', body?: unknown) {
  const result = await page.evaluate(
    async ({ path, method, body }) => {
      const session = await (await fetch('/api/v1/session')).json();
      const response = await fetch(path, {
        method,
        headers: {
          'Content-Type': 'application/json',
          'X-CSRF-Token': session.csrf_token,
        },
        body: body === undefined ? undefined : JSON.stringify(body),
      });
      return { status: response.status, value: await response.json() };
    },
    { path, method, body }
  );
  expect(
    result.status,
    `${method} ${path}: ${JSON.stringify(result.value)}`
  ).toBeGreaterThanOrEqual(200);
  expect(
    result.status,
    `${method} ${path}: ${JSON.stringify(result.value)}`
  ).toBeLessThan(300);
  return result.value;
}

async function noHorizontalOverflow(page: Page, description: string) {
  await expect
    .poll(
      () =>
        page
          .evaluate(() => ({
            content: Math.max(
              document.documentElement.scrollWidth,
              document.body.scrollWidth
            ),
            viewport: window.innerWidth,
          }))
          .then(({ content, viewport }) => content - viewport),
      {
        message: `${description} must fit the viewport without document-level horizontal scrolling`,
      }
    )
    .toBeLessThanOrEqual(1);
}

test('archive design: layouts, responsive navigation, keyboard, themes and editing', async ({
  page,
  context,
}) => {
  test.setTimeout(90000);
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  const cdp = await context.newCDPSession(page);
  await cdp.send('WebAuthn.enable');
  await cdp.send('WebAuthn.addVirtualAuthenticator', {
    options: {
      protocol: 'ctap2',
      transport: 'internal',
      hasResidentKey: true,
      hasUserVerification: true,
      isUserVerified: true,
      automaticPresenceSimulation: true,
    },
  });

  await page.goto('/login');
  await expect(
    page.getByRole('button', { name: '使用 Passkey 登录', exact: true })
  ).toBeVisible();
  await page.screenshot({
    path: 'test-results/design-login.png',
    fullPage: true,
    animations: 'disabled',
  });
  for (const width of [320, 390]) {
    await page.setViewportSize({ width, height: 844 });
    await noHorizontalOverflow(page, `${width}px login page`);
    await expect(
      page.getByRole('button', { name: '使用 Passkey 登录', exact: true })
    ).toBeVisible();
  }
  await page.screenshot({
    path: 'test-results/design-login-mobile.png',
    fullPage: true,
    animations: 'disabled',
  });
  await page.setViewportSize({ width: 1440, height: 1000 });
  const token = execFileSync(
    binary,
    ['--config', config, 'auth', 'create-setup-token'],
    { encoding: 'utf8' }
  )
    .trim()
    .split('\n')
    .at(-1)!;
  await page.goto('/setup');
  await page.getByLabel('Setup Token').fill(token);
  await page.getByRole('button', { name: '验证并创建 Passkey' }).click();
  await expect(page).toHaveURL(/\/login$/);
  await page
    .getByRole('button', { name: '使用 Passkey 登录', exact: true })
    .click();
  await expect(
    page.getByRole('heading', { level: 1, name: '全部收藏', exact: true })
  ).toBeVisible();

  const folderNames = ['设计灵感', '工程与工具', '阅读与摄影'];
  const folderIds: string[] = [];
  for (const name of folderNames) {
    folderIds.push((await api(page, '/api/v1/folders', 'POST', { name })).id);
  }
  const longTitle =
    '在日常里寻找微小的秩序：关于阅读、摄影与长期主义的一份私人观察笔记 — A Field Guide to Everyday Wonder';
  const entries = [
    {
      title: 'The Creative Independent',
      url: 'https://thecreativeindependent.com',
      tags: ['灵感', '阅读'],
      notes: '关于创作的耐心、方法与日常。',
      folder: 0,
    },
    {
      title: 'Aperture · 看见摄影的另一面',
      url: 'https://aperture.org',
      tags: ['摄影', '视觉'],
      notes: '用影像记录时间，用观看理解世界。',
      folder: 2,
    },
    {
      title: 'MDN Web Docs',
      url: 'https://developer.mozilla.org',
      tags: ['工程', '参考'],
      notes: '开放 Web 的知识地图。',
      folder: 1,
    },
    {
      title: '字体、留白与阅读节奏',
      url: 'https://typography.example.com/notes',
      tags: ['灵感', '排版', '设计'],
      notes: '收集那些经得起细看的设计细节。',
      folder: 0,
    },
    {
      title: 'Svelte · Build with less',
      url: 'https://svelte.dev/docs',
      tags: ['工程', '前端'],
      notes: '让交互保持轻盈。',
      folder: 1,
    },
    {
      title: 'Rust 语言圣经',
      url: 'https://course.rs',
      tags: ['工程', 'Rust', '阅读'],
      notes: '可靠软件，从清晰的抽象开始。',
      folder: 1,
    },
    {
      title: 'Magnum Photos — Stories',
      url: 'https://www.magnumphotos.com',
      tags: ['摄影', '纪实'],
      notes: '在真实世界中寻找故事。',
      folder: 2,
    },
    {
      title: 'A List Apart',
      url: 'https://alistapart.com',
      tags: ['设计', '阅读'],
      notes: '设计、开发，以及那些重要的交汇点。',
      folder: 0,
    },
    {
      title: longTitle,
      url: 'https://fieldnotes.example.com/everyday-wonder',
      tags: ['摄影', '阅读', '长期主义', '生活观察', '特别值得保存的长标签'],
      notes: '一条足够长的收藏，检验文字换行、截断和标签边界。',
      folder: 2,
    },
    {
      title: 'The Paris Review',
      url: 'https://www.theparisreview.org',
      tags: ['阅读', '访谈'],
      notes: '作家谈写作，也谈生活。',
      folder: 2,
    },
    {
      title: 'Are.na — A space to connect ideas',
      url: 'https://www.are.na',
      tags: ['灵感', '策展'],
      notes: '想法在连接中生长。',
      folder: 0,
    },
    {
      title: 'SQLite · Small. Fast. Reliable.',
      url: 'https://sqlite.org',
      tags: ['工程', '数据库'],
      notes: '为小而美的软件保存记忆。',
      folder: 1,
    },
  ];
  for (const [index, entry] of entries.entries()) {
    await api(page, '/api/v1/bookmarks', 'POST', {
      title: entry.title,
      url_raw: entry.url,
      tags: entry.tags,
      notes: entry.notes,
      folder_id: folderIds[entry.folder],
      pinned: index < 3,
    });
  }

  const rows = page.locator('.bookmark-table tbody tr.bookmark');
  // The first three entries are pinned; the shelf shows them instead of the list.
  const shelved = entries.slice(0, 3).map((entry) => entry.title);
  const listed = entries.length - shelved.length;
  const rowFor = (title: string) =>
    rows.filter({ has: page.locator('.bookmark-title', { hasText: title }) });
  const waitForLibrary = async () => {
    await expect(
      page.getByRole('heading', { level: 1, name: '全部收藏', exact: true })
    ).toBeVisible();
    await expect(rows).toHaveCount(listed);
    await expect(page.locator('.bookmark-list')).toHaveAttribute(
      'aria-busy',
      'false'
    );
  };
  await page.reload();
  await waitForLibrary();
  await expect(
    page.getByRole('region', { name: '精选置顶' }).getByRole('link')
  ).toHaveCount(3);
  await expect(
    page.getByRole('button', { name: '列表视图', exact: true })
  ).toHaveAttribute('aria-pressed', 'true');
  await expect(rowFor(longTitle)).toHaveCount(1);
  await expect(rowFor(longTitle).locator('.tags button')).toHaveCount(5);
  for (const title of shelved) await expect(rowFor(title)).toHaveCount(0);
  const shelf = page.getByRole('region', { name: '精选置顶' });
  await shelf.getByRole('button', { name: '编辑 ' + shelved[0] }).click();
  const shelfEditor = page.getByRole('dialog', { name: '编辑收藏' });
  await expect(shelfEditor.getByLabel('标题', { exact: true })).toHaveValue(
    shelved[0]
  );
  await page.keyboard.press('Escape');
  await expect(shelfEditor).toHaveCount(0);
  await page.getByRole('button', { name: '批量操作' }).click();
  await expect(shelf).toHaveCount(0);
  await expect(rows).toHaveCount(entries.length);
  await page.getByRole('button', { name: '完成' }).click();
  await waitForLibrary();
  await page.screenshot({
    path: 'test-results/design-desktop.png',
    fullPage: false,
    animations: 'disabled',
  });

  await page.getByRole('button', { name: '卡片视图', exact: true }).click();
  await expect(page.locator('.bookmark-list')).toHaveClass(/gallery/);
  await page.reload();
  await waitForLibrary();
  await expect(
    page.getByRole('button', { name: '卡片视图', exact: true })
  ).toHaveAttribute('aria-pressed', 'true');
  await expect(page.locator('.bookmark-list')).toHaveClass(/gallery/);
  // Cards sit side by side, so dropping on a card's left/right half orders before/after.
  {
    const source = rows.nth(0);
    const target = rows.nth(1);
    await source.hover();
    const from = (await source.locator('.drag-handle').boundingBox())!;
    const to = (await target.boundingBox())!;
    await page.mouse.move(from.x + from.width / 2, from.y + from.height / 2);
    await page.mouse.down();
    await page.mouse.move(
      from.x + from.width / 2 + 12,
      from.y + from.height / 2 + 3
    );
    for (const [fraction, placement] of [
      [0.8, 'after'],
      [0.2, 'before'],
    ] as const) {
      const x = to.x + to.width * fraction,
        y = to.y + to.height * 0.5;
      await page.mouse.move(x, y, { steps: 8 });
      await page.mouse.move(x + 1, y + 1);
      await expect(target).toHaveAttribute('data-drop', placement);
    }
    // Release outside any drop target so the order is left unchanged.
    await page.mouse.move(5, 5, { steps: 8 });
    await page.mouse.up();
    await expect(page.locator('[data-dragging]')).toHaveCount(0);
  }
  await page.screenshot({
    path: 'test-results/design-grid.png',
    fullPage: false,
    animations: 'disabled',
  });

  for (const width of [1440, 1024, 768, 390, 320]) {
    await page.setViewportSize({ width, height: 1000 });
    await noHorizontalOverflow(page, `${width}px card layout`);
    await page.getByRole('button', { name: '列表视图', exact: true }).click();
    await noHorizontalOverflow(page, `${width}px list layout`);
    await page.getByRole('button', { name: '卡片视图', exact: true }).click();
  }

  await page.setViewportSize({ width: 1440, height: 1000 });
  await page.getByRole('button', { name: '列表视图', exact: true }).click();
  await page.reload();
  await waitForLibrary();
  await expect(
    page.getByRole('button', { name: '列表视图', exact: true })
  ).toHaveAttribute('aria-pressed', 'true');
  await page
    .getByRole('heading', { level: 1, name: '全部收藏', exact: true })
    .click();
  await page.keyboard.press('/');
  const search = page.getByRole('searchbox', { name: '搜索收藏' });
  await expect(search).toBeFocused();
  await search.fill('RUST');
  await expect(rows).toHaveCount(1);
  await expect(rowFor('Rust 语言圣经')).toBeVisible();
  await search.press('Escape');
  await expect(search).toHaveValue('');
  await waitForLibrary();
  await search.fill('no-matching-archive-2026');
  await expect(rows).toHaveCount(0);
  await expect(
    page.getByRole('heading', { name: '没有找到匹配的收藏', exact: true })
  ).toBeVisible();
  await page.getByRole('button', { name: '清除筛选', exact: true }).click();
  await expect(search).toHaveValue('');
  await waitForLibrary();

  await page.setViewportSize({ width: 390, height: 844 });
  const mobileToggle = page.locator('.mobile-toggle');
  await mobileToggle.click();
  const navigation = page.getByRole('navigation', { name: '收藏导航' });
  const sidebar = page.locator('aside');
  await expect(mobileToggle).toHaveAttribute('aria-expanded', 'true');
  await expect(page.locator('#workspace')).toHaveJSProperty('inert', true);
  await expect(
    navigation.getByRole('button', { name: '全部收藏', exact: true })
  ).toBeFocused();
  const folderMore = sidebar.getByRole('button', {
    name: '更多操作：设计灵感',
    exact: true,
  });
  await folderMore.click();
  await expect(
    page.getByRole('menuitem', { name: '新建子文件夹', exact: true })
  ).toBeFocused();
  await page.keyboard.press('ArrowDown');
  await expect(
    page.getByRole('menuitem', { name: '重命名', exact: true })
  ).toBeFocused();
  await page.keyboard.press('Tab');
  await expect(
    page.getByRole('menuitem', { name: '删除…', exact: true })
  ).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(page.getByRole('menu')).toHaveCount(0);
  await expect(folderMore).toBeFocused();
  await expect(sidebar).toHaveClass(/open/);
  for (let index = 0; index < 8; index++) {
    await page.keyboard.press('Tab');
    await expect
      .poll(() =>
        sidebar.evaluate((node) => node.contains(document.activeElement))
      )
      .toBe(true);
  }
  const sidebarFirst = sidebar.getByRole('link', {
    name: 'bookmarkd 收藏中心',
    exact: true,
  });
  const sidebarLast = sidebar.getByRole('button', {
    name: '退出登录',
    exact: true,
  });
  await sidebarLast.focus();
  await page.keyboard.press('Tab');
  await expect(sidebarFirst).toBeFocused();
  await page.keyboard.press('Shift+Tab');
  await expect(sidebarLast).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(sidebar).not.toHaveClass(/open/);
  await expect(mobileToggle).toHaveAttribute('aria-expanded', 'false');
  await expect(page.locator('#workspace')).toHaveJSProperty('inert', false);
  await expect(mobileToggle).toBeFocused();
  await mobileToggle.click();
  await navigation
    .getByRole('button', { name: '设计灵感', exact: true })
    .click();
  await expect(
    page.getByRole('heading', { level: 1, name: '设计灵感', exact: true })
  ).toBeVisible();
  await expect(rows).toHaveCount(4);
  await expect(page.locator('aside')).not.toHaveClass(/open/);
  await noHorizontalOverflow(page, 'mobile folder selection');
  await page.getByRole('button', { name: '打开导航', exact: true }).click();
  await navigation
    .getByRole('button', { name: '全部收藏', exact: true })
    .click();
  await waitForLibrary();
  await rowFor(longTitle)
    .getByRole('button', { name: '摄影', exact: true })
    .click();
  await expect(
    page.getByRole('button', { name: '清除标签筛选 摄影', exact: true })
  ).toBeVisible();
  await expect(rows).toHaveCount(3);
  await expect(rowFor('Magnum Photos — Stories')).toBeVisible();
  await noHorizontalOverflow(page, 'mobile tag filter');
  await page
    .getByRole('button', { name: '清除标签筛选 摄影', exact: true })
    .click();
  await waitForLibrary();
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.locator('.shelf-grid').evaluate((node) => node.scrollTo(0, 0));
  await page.mouse.move(0, 0);
  await page.screenshot({
    path: 'test-results/design-mobile.png',
    fullPage: false,
    animations: 'disabled',
  });

  await page.setViewportSize({ width: 1440, height: 1000 });
  await navigation
    .getByRole('button', { name: '设置与迁移', exact: true })
    .click();
  await expect(
    page.getByRole('heading', { level: 1, name: '设置与迁移', exact: true })
  ).toBeVisible();
  const theme = page.getByLabel('明暗模式');
  await expect(theme).toBeEnabled();
  for (const [id, name] of [
    ['terracotta', '陶土橙'],
    ['forest', '森林绿'],
    ['ocean', '海洋蓝'],
    ['violet', '鸢尾紫'],
    ['amber', '暖琥珀'],
    ['slate', '石墨灰'],
  ]) {
    await page.getByRole('radio', { name, exact: true }).check();
    await expect(page.locator('html')).toHaveAttribute('data-palette', id);
    for (const mode of ['light', 'dark']) {
      await theme.selectOption(mode);
      await expect(page.locator('html')).toHaveCSS('color-scheme', mode);
      for (const width of [1440, 1024, 768, 390, 320]) {
        await page.setViewportSize({ width, height: 1000 });
        await noHorizontalOverflow(page, `${width}px ${name} ${mode} settings`);
      }
    }
  }
  await page.setViewportSize({ width: 1440, height: 1000 });
  await page.getByRole('radio', { name: '陶土橙', exact: true }).check();
  await theme.selectOption('light');
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.mouse.move(0, 0);
  await page.screenshot({
    path: 'test-results/design-settings.png',
    fullPage: true,
    animations: 'disabled',
  });
  await theme.selectOption('dark');
  await expect(page.locator('html')).toHaveCSS('color-scheme', 'dark');
  await page.getByRole('button', { name: '保存偏好', exact: true }).click();
  await expect(
    page.getByRole('status').filter({ hasText: '偏好已保存' })
  ).toBeVisible();
  await page.reload();
  await expect(theme).toHaveValue('dark');
  await navigation
    .getByRole('button', { name: '全部收藏', exact: true })
    .click();
  await waitForLibrary();
  await expect(page.locator('html')).toHaveCSS('color-scheme', 'dark');
  await page.screenshot({
    path: 'test-results/design-dark.png',
    fullPage: false,
    animations: 'disabled',
  });
  await navigation
    .getByRole('button', { name: '设置与迁移', exact: true })
    .click();
  await expect(theme).toBeEnabled();
  await theme.selectOption('system');
  await page.emulateMedia({ colorScheme: 'light' });
  await expect(page.locator('html')).toHaveCSS('color-scheme', 'light');
  await page.emulateMedia({ colorScheme: 'dark' });
  await expect(page.locator('html')).toHaveCSS('color-scheme', 'dark');
  await page.emulateMedia({ colorScheme: 'light' });
  await page.getByRole('button', { name: '保存偏好', exact: true }).click();
  await expect(
    page.getByRole('status').filter({ hasText: '偏好已保存' })
  ).toBeVisible();
  await navigation
    .getByRole('button', { name: '全部收藏', exact: true })
    .click();
  await waitForLibrary();

  await page.getByRole('button', { name: '添加收藏', exact: true }).click();
  let editor = page.getByRole('dialog', { name: '添加收藏', exact: true });
  await expect(editor).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(editor).toHaveCount(0);
  await page.getByRole('button', { name: '添加收藏', exact: true }).click();
  editor = page.getByRole('dialog', { name: '添加收藏', exact: true });
  await editor
    .getByLabel('网址', { exact: false })
    .fill('https://notes.example.com/slow-looking');
  await editor.getByLabel('标题', { exact: true }).fill('慢慢看，慢慢收藏');
  await editor.getByLabel('文件夹').selectOption(folderIds[2]);
  await editor.getByLabel('标签', { exact: true }).fill('摄影, 灵感');
  await editor
    .getByLabel('备注', { exact: true })
    .fill('为值得重访的内容留一个位置。');
  await page.screenshot({
    path: 'test-results/design-editor.png',
    fullPage: false,
    animations: 'disabled',
  });
  await page.setViewportSize({ width: 320, height: 844 });
  await noHorizontalOverflow(page, '320px bookmark editor');
  await editor.getByRole('button', { name: '保存收藏', exact: true }).click();
  await expect(editor).toHaveCount(0);
  await expect(rows).toHaveCount(listed + 1);
  await expect(rowFor('慢慢看，慢慢收藏')).toBeVisible();
  await rowFor('慢慢看，慢慢收藏')
    .getByRole('button', { name: '编辑 慢慢看，慢慢收藏', exact: true })
    .click();
  const editDialog = page.getByRole('dialog', {
    name: '编辑收藏',
    exact: true,
  });
  await expect(editDialog.getByLabel('备注', { exact: true })).toHaveValue(
    '为值得重访的内容留一个位置。'
  );
  await editDialog
    .getByLabel('标题', { exact: true })
    .fill('慢慢看，慢慢收藏 · 已整理');
  await editDialog
    .getByRole('button', { name: '保存收藏', exact: true })
    .click();
  await expect(editDialog).toHaveCount(0);
  await page.reload();
  await expect(rowFor('慢慢看，慢慢收藏 · 已整理')).toBeVisible();
  await expect(rows).toHaveCount(listed + 1);
  // Logging out from the open mobile sidebar must release its focus trap.
  await page.getByRole('button', { name: '打开导航', exact: true }).click();
  await page.getByRole('button', { name: '退出登录', exact: true }).click();
  await expect(
    page.getByRole('button', { name: '使用 Passkey 登录', exact: true })
  ).toBeVisible();
  await page.keyboard.press('Tab');
  await page.keyboard.press('Tab');
  expect(errors).toEqual([]);
});
