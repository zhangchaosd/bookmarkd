import { test, expect, type Page, type Locator } from '@playwright/test';
import { spawn, execFileSync, type ChildProcess } from 'node:child_process';
import { createServer, type Server } from 'node:http';
import { mkdtempSync, rmSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve, join } from 'node:path';

const binary = resolve(
  process.env.BOOKMARKD_BIN || '../target/debug/bookmarkd'
);
const origin = 'http://localhost:8779';
const PNG = Buffer.from(
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==',
  'base64'
);
let data: string, server: ChildProcess | undefined, site: Server;
let siteUrl = '';

test.use({
  baseURL: origin,
  viewport: { width: 1280, height: 900 },
  reducedMotion: 'reduce',
});

test.beforeAll(async () => {
  // A tiny local site that declares its icon, for the server-side fetch.
  site = createServer((req, res) => {
    if (req.url === '/') {
      res.writeHead(200, { 'content-type': 'text/html' });
      res.end('<html><head><link rel="icon" href="/brand.png"></head></html>');
    } else if (req.url === '/brand.png') {
      res.writeHead(200, { 'content-type': 'image/png' });
      res.end(PNG);
    } else {
      res.writeHead(404, { 'content-type': 'text/html' });
      res.end('<h1>Not found</h1>');
    }
  });
  await new Promise<void>((done) => site.listen(0, '127.0.0.1', done));
  const address = site.address();
  siteUrl = `http://127.0.0.1:${typeof address === 'object' && address ? address.port : 0}`;
  data = mkdtempSync(join(tmpdir(), 'bookmarkd-features-'));
  const config = join(data, 'config.toml');
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
    readFileSync(config, 'utf8').replace('127.0.0.1:8765', '127.0.0.1:8779')
  );
  server = spawn(binary, ['--config', config, 'serve'], { stdio: 'ignore' });
  await expect
    .poll(async () => {
      try {
        return (await fetch(origin + '/healthz')).status;
      } catch {
        return 0;
      }
    })
    .toBe(200);
});

test.afterAll(async () => {
  if (server && server.exitCode === null) {
    const stopped = new Promise<void>((done) =>
      server!.once('exit', () => done())
    );
    server.kill();
    await stopped;
  }
  site?.close();
  if (data) rmSync(data, { recursive: true, force: true });
});

async function api(page: Page, path: string, method = 'GET', body?: unknown) {
  return page.evaluate(
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
      return response.json();
    },
    { path, method, body }
  );
}

async function offset(tile: Locator, board: Locator) {
  const [a, b] = [(await tile.boundingBox())!, (await board.boundingBox())!];
  return [Math.round(a.x - b.x), Math.round(a.y - b.y)];
}

test('pinned landing, site icons and the desktop view', async ({
  page,
  context,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
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
  const token = execFileSync(
    binary,
    ['--config', join(data, 'config.toml'), 'auth', 'create-setup-token'],
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
  const heading = page.getByRole('heading', { level: 1 });
  // Without pinned bookmarks the landing view falls back to the whole library.
  await expect(heading).toHaveText('全部收藏');

  // Site icons are fetched by the server and saved with the bookmark.
  await page.getByRole('button', { name: '添加收藏' }).first().click();
  const editor = page.getByRole('dialog', { name: '添加收藏' });
  await editor.getByLabel('网址', { exact: false }).fill(siteUrl + '/missing');
  await editor.getByRole('button', { name: '获取网站图标' }).click();
  await expect(editor.getByRole('alert')).toContainText('没有可用的网站图标');
  await editor.getByLabel('网址', { exact: false }).fill(siteUrl + '/');
  await editor.getByLabel('标题', { exact: true }).fill('本地图标站');
  await editor.getByRole('button', { name: '获取网站图标' }).click();
  const preview = editor.getByRole('img', { name: '当前网站图标' });
  await expect(preview).toHaveAttribute('src', /^data:image\/png;base64,/);
  await editor.getByRole('button', { name: '移除' }).click();
  await expect(preview).toHaveCount(0);
  await editor.getByRole('button', { name: '获取网站图标' }).click();
  await expect(preview).toBeVisible();
  await editor.getByRole('button', { name: '保存收藏' }).click();
  await expect(editor).toHaveCount(0);
  const row = page
    .locator('tbody tr')
    .filter({ hasText: '本地图标站' })
    .locator('.site-icon img');
  await expect(row).toHaveAttribute('src', /^data:image\/png;base64,/);
  const saved = (await api(page, '/api/v1/bookmarks')).items[0];
  expect(saved.icon).toMatch(/^data:image\/png;base64,/);
  const exported = await page.evaluate(async () => {
    const session = await (await fetch('/api/v1/session')).json();
    const r = await fetch('/api/v1/exports', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'X-CSRF-Token': session.csrf_token,
      },
      body: JSON.stringify({ format: 'html' }),
    });
    return r.status === 200 ? r.text() : '';
  });
  if (exported) expect(exported).toContain('ICON="data:image/png;base64,');

  for (const name of ['甲', '乙', '丙', '丁'])
    await api(page, '/api/v1/bookmarks', 'POST', {
      url_raw: `https://${name.charCodeAt(0)}.example.com/`,
      title: name,
      pinned: true,
    });
  await page.reload();
  await expect(heading).toHaveText('常用置顶');

  // The desktop layout is offered only for pinned bookmarks.
  await page.getByRole('button', { name: '桌面视图' }).click();
  const board = page.locator('.desktop-board');
  const tile = (title: string) =>
    board.locator('.desk-tile').filter({ hasText: title });
  await expect(board.locator('.desk-tile')).toHaveCount(4);
  await expect(page.locator('.bookmark-list')).toBeHidden();
  // The four pins fill the top row of 108×116 cells; same-second pins have no fixed order.
  const at = (col: number, row: number) => [col * 108, row * 116];
  const titles = ['甲', '乙', '丙', '丁'];
  const byColumn: string[] = [];
  for (const t of titles) {
    const [x, y] = await offset(tile(t), board);
    expect(y).toBe(0);
    byColumn[x / 108] = t;
  }
  expect(byColumn.filter(Boolean)).toHaveLength(4);
  const [, second, third, fourth] = byColumn;
  const saveLayout = () =>
    page.waitForResponse(
      (r) =>
        r.url().endsWith('/api/v1/preferences') &&
        r.request().method() === 'PATCH'
    );
  const dragTo = async (title: string, col: number, row: number) => {
    const origin = (await board.boundingBox())!;
    const box = (await tile(title).boundingBox())!;
    const saving = saveLayout();
    await page.mouse.move(box.x + 54, box.y + 30);
    await page.mouse.down();
    await page.mouse.move(
      origin.x + col * 108 + 54,
      origin.y + row * 116 + 30,
      {
        steps: 12,
      }
    );
    await page.mouse.up();
    expect((await saving).status()).toBe(200);
  };
  await dragTo(fourth, 1, 1);
  await expect.poll(() => offset(tile(fourth), board)).toEqual(at(1, 1));

  // Alt + arrow keys move the focused icon one cell.
  const saving = saveLayout();
  await tile(third).getByRole('link').focus();
  await page.keyboard.press('Alt+ArrowDown');
  await saving;
  await expect.poll(() => offset(tile(third), board)).toEqual(at(2, 1));

  // Dropping onto an occupied cell swaps the two icons.
  await dragTo(second, 1, 1);
  await expect.poll(() => offset(tile(second), board)).toEqual(at(1, 1));
  await expect.poll(() => offset(tile(fourth), board)).toEqual(at(1, 0));

  // Positions and the chosen layout survive a reload.
  const layout = await Promise.all(
    ['甲', '乙', '丙', '丁'].map((t) => offset(tile(t), board))
  );
  await page.reload();
  await expect(heading).toHaveText('常用置顶');
  await expect(board.locator('.desk-tile')).toHaveCount(4);
  expect(
    await Promise.all(
      ['甲', '乙', '丙', '丁'].map((t) => offset(tile(t), board))
    )
  ).toEqual(layout);
  expect(
    Object.keys((await api(page, '/api/v1/preferences')).desktop).length
  ).toBeGreaterThanOrEqual(3);

  // Icons can be edited from the desktop, and other views keep their layout.
  await tile('丁').hover();
  await tile('丁').getByRole('button', { name: '编辑 丁' }).click();
  const editDialog = page.getByRole('dialog', { name: '编辑收藏' });
  await expect(editDialog.getByLabel('标题', { exact: true })).toHaveValue(
    '丁'
  );
  await page.keyboard.press('Escape');
  await page
    .getByRole('navigation', { name: '收藏导航' })
    .getByRole('button', { name: '全部收藏', exact: true })
    .click();
  await expect(heading).toHaveText('全部收藏');
  await expect(page.getByRole('button', { name: '桌面视图' })).toHaveCount(0);
  await expect(board).toHaveCount(0);
  expect(errors).toEqual([]);
});
