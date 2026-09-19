import { test, expect } from '@playwright/test';
import { spawn, execFileSync, type ChildProcess } from 'node:child_process';
import { mkdtempSync, rmSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve, join } from 'node:path';
const binary = resolve(
  process.env.BOOKMARKD_BIN || '../target/debug/bookmarkd'
);
let data: string, config: string, server: ChildProcess, second: ChildProcess;
function cli(...args: string[]) {
  return execFileSync(binary, ['--config', config, ...args], {
    encoding: 'utf8',
  });
}
async function start(conf = config, port = 8765) {
  const proc = spawn(binary, ['--config', conf, 'serve'], { stdio: 'ignore' });
  await expect
    .poll(async () => {
      try {
        return (await fetch(`http://localhost:${port}/healthz`)).status;
      } catch {
        return 0;
      }
    })
    .toBe(200);
  return proc;
}
test.beforeAll(async () => {
  data = mkdtempSync(join(tmpdir(), 'bookmarkd-e2e-'));
  config = join(data, 'config.toml');
  execFileSync(binary, [
    'init',
    '--data-dir',
    data,
    '--public-url',
    'http://localhost:8765',
    '--rp-id',
    'localhost',
    '--allow-insecure-localhost',
  ]);
  server = await start();
});
test.afterAll(async () => {
  server?.kill();
  second?.kill();
  await new Promise((r) => setTimeout(r, 300));
  rmSync(data, { recursive: true, force: true });
});
test('Passkey, private library, conflicts, migration, shared host and recovery', async ({
  page,
  context,
}) => {
  const cdp = await context.newCDPSession(page);
  await cdp.send('WebAuthn.enable');
  const { authenticatorId } = await cdp.send(
    'WebAuthn.addVirtualAuthenticator',
    {
      options: {
        protocol: 'ctap2',
        transport: 'internal',
        hasResidentKey: true,
        hasUserVerification: true,
        isUserVerified: true,
        automaticPresenceSimulation: true,
      },
    }
  );
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/login');
  await expect(
    page.getByRole('button', { name: '使用 Passkey 登录', exact: true })
  ).toBeVisible();
  expect(await page.getByRole('button').count()).toBe(1);
  expect(await page.getByRole('checkbox').count()).toBe(1);
  expect((await context.request.get('/api/v1/bookmarks')).status()).toBe(401);
  const token = cli('auth', 'create-setup-token').trim().split('\n').at(-1)!;
  await page.goto('/setup');
  await page.getByLabel('Setup Token').fill(token);
  await page.getByRole('button', { name: '验证并创建 Passkey' }).click();
  await expect(page).toHaveURL(/\/login$/);
  await expect(
    page.getByRole('button', { name: '使用 Passkey 登录', exact: true })
  ).toBeVisible();
  await page
    .getByRole('button', { name: '使用 Passkey 登录', exact: true })
    .click();
  await expect(page.getByRole('button', { name: '＋ 添加收藏' })).toBeVisible();
  const cookies = await context.cookies();
  const session = cookies.find((c) => c.name === 'bookmarkd_session')!;
  expect(session.httpOnly).toBe(true);
  expect(session.sameSite).toBe('Strict');
  expect(session.expires).toBe(-1);
  expect((await context.request.get('/setup')).status()).toBe(404);
  await page.getByRole('button', { name: '＋ 添加收藏' }).click();
  await page
    .getByLabel('网址', { exact: false })
    .fill('https://example.com/?a=1&b=2');
  await page.getByLabel('标题', { exact: true }).fill('中文学习 Rust');
  await page.getByLabel('备注', { exact: true }).fill('100% _ useful');
  await page.getByLabel('标签', { exact: true }).fill('阅读, rust');
  await page.getByRole('button', { name: '保存收藏' }).click();
  await expect(page.getByRole('link', { name: '中文学习 Rust' })).toBeVisible();
  await page.getByRole('searchbox').fill('中 RUST');
  await expect(page.getByRole('link', { name: '中文学习 Rust' })).toBeVisible();
  const api = async (
    path: string,
    method = 'GET',
    body?: unknown,
    extra: Record<string, string> = {}
  ) =>
    page.evaluate(
      async ({ path, method, body, extra }) => {
        const s = await (await fetch('/api/v1/session')).json();
        const r = await fetch(path, {
          method,
          headers: {
            'Content-Type': 'application/json',
            'X-CSRF-Token': s.csrf_token,
            ...extra,
          },
          body: body === undefined ? undefined : JSON.stringify(body),
        });
        return { status: r.status, value: await r.json() };
      },
      { path, method, body, extra }
    );
  let listing = await api('/api/v1/bookmarks');
  const b = listing.value.items[0];
  expect(
    (
      await api('/api/v1/bookmarks/' + b.id, 'PATCH', {
        version: 0,
        title: 'wrong',
      })
    ).status
  ).toBe(409);
  expect(
    (await api('/api/v1/bookmarks', 'POST', { url_raw: 'javascript:alert(1)' }))
      .status
  ).toBe(400);
  const first = await api(
    '/api/v1/bookmarks',
    'POST',
    { url_raw: 'http://192.168.1.1', title: '内网' },
    { 'Idempotency-Key': 'repeat' }
  );
  const repeat = await api(
    '/api/v1/bookmarks',
    'POST',
    { url_raw: 'http://192.168.1.1', title: '内网' },
    { 'Idempotency-Key': 'repeat' }
  );
  expect(first.value.id).toBe(repeat.value.id);
  const f = (await api('/api/v1/folders', 'POST', { name: '学习' })).value;
  expect(
    (
      await api('/api/v1/folders/' + f.id, 'PATCH', {
        version: f.version,
        parent_id: f.id,
      })
    ).status
  ).toBe(400);
  expect(
    (
      await api('/api/v1/bookmarks/batch', 'POST', {
        action: 'delete',
        items: [
          { id: b.id, version: b.version },
          { id: 'missing', version: 1 },
        ],
      })
    ).status
  ).toBe(404);
  expect((await api('/api/v1/bookmarks/' + b.id)).value.deleted_at).toBeNull();
  expect(
    (await api('/api/v1/bookmarks/' + b.id, 'DELETE', { version: b.version }))
      .status
  ).toBe(200);
  expect(
    (
      await api('/api/v1/bookmarks/' + b.id + '/restore', 'POST', {
        version: b.version + 1,
      })
    ).status
  ).toBe(200);
  const preview = (
    await api('/api/v1/imports', 'POST', {
      format: 'html',
      content:
        '<DL><DT><H3>导入</H3><DL><DT><A HREF="https://rust-lang.org">Rust</A><DT><A HREF="javascript:alert(1)">unsafe</A></DL></DL>',
    })
  ).value;
  expect(preview.parsed).toBe(1);
  expect(preview.warnings).toHaveLength(1);
  expect(
    (await api(`/api/v1/imports/${preview.id}/commit`, 'POST', {})).value.added
  ).toBe(1);
  expect(
    (await api(`/api/v1/imports/${preview.id}/commit`, 'POST', {})).value.added
  ).toBe(1);
  const exported = (
    await api('/api/v1/exports', 'POST', {
      format: 'json',
      includes_trash: true,
    })
  ).value;
  expect(exported.bookmarks).toHaveLength(3);
  expect(exported).not.toHaveProperty('credentials');
  // A second application shares the same authoritative credentials but no sessions.
  const bdir = join(data, 'host-b');
  execFileSync(binary, [
    'init',
    '--data-dir',
    bdir,
    '--public-url',
    'http://localhost:8766',
    '--rp-id',
    'localhost',
    '--allow-insecure-localhost',
  ]);
  const bc = join(bdir, 'config.toml');
  let cfg = readFileSync(bc, 'utf8')
    .replace('127.0.0.1:8765', '127.0.0.1:8766')
    .replace('app_id = "bookmarkd"', 'app_id = "second"')
    .replace(
      'store = "auth.db"',
      `store = ${JSON.stringify(join(data, 'auth.db'))}`
    )
    .replace(
      'user_id_file = "user-id.txt"',
      `user_id_file = ${JSON.stringify(join(data, 'user-id.txt'))}`
    )
    .replace('setup_mode = "auto"', 'setup_mode = "disabled"');
  writeFileSync(bc, cfg);
  second = await start(bc, 8766);
  await context.addCookies([
    {
      name: 'second_session',
      value: session.value,
      url: 'http://localhost:8766',
    },
  ]);
  await page.goto('http://localhost:8766/login');
  await expect(
    page.getByRole('button', { name: '使用 Passkey 登录', exact: true })
  ).toBeVisible();
  await page.getByRole('checkbox').check();
  await page
    .getByRole('button', { name: '使用 Passkey 登录', exact: true })
    .click();
  await expect(page.getByRole('button', { name: '＋ 添加收藏' })).toBeVisible();
  const long = (await context.cookies()).find(
    (c) => c.name === 'second_session'
  )!;
  expect(long.expires - Date.now() / 1000).toBeGreaterThan(2591900);
  // One credential can authenticate at both hosts, while each business library is separate.
  expect((await api('/api/v1/bookmarks')).value.total).toBe(0);
  const imported = (
    await api('/api/v1/imports', 'POST', {
      format: 'json',
      content: JSON.stringify(exported),
    })
  ).value;
  await api(`/api/v1/imports/${imported.id}/commit`, 'POST', {
    mode: 'restore',
  });
  expect(
    (
      await api('/api/v1/exports', 'POST', {
        format: 'json',
        includes_trash: true,
      })
    ).value.bookmarks
  ).toEqual(exported.bookmarks);
  await page.goto('http://localhost:8765/library');
  await expect(page.getByRole('link', { name: '中文学习 Rust' })).toBeVisible();
  await page.setViewportSize({ width: 390, height: 844 });
  await page.screenshot({ path: 'test-results/mobile.png', fullPage: true });
  await page.setViewportSize({ width: 1440, height: 1000 });
  await page.screenshot({ path: 'test-results/desktop.png', fullPage: true });
  const backup = join(data, 'backup');
  cli('backup', '--include-auth', '--output', backup);
  expect(readFileSync(join(backup, 'manifest.json'), 'utf8')).toContain(
    'bookmarkd-backup'
  );
  // Reject CSRF and Origin even when a valid cookie is present.
  expect(
    (
      await context.request.post('/api/v1/bookmarks', {
        headers: {
          Origin: 'https://evil.example',
          'Content-Type': 'application/json',
        },
        data: { url_raw: 'https://evil.example' },
      })
    ).status()
  ).toBe(403);
  await page.getByRole('button', { name: '退出登录' }).click();
  await expect(
    page.getByRole('button', { name: '使用 Passkey 登录', exact: true })
  ).toBeVisible();
  expect((await context.request.get('/api/v1/bookmarks')).status()).toBe(401);
  const credential = (
    await cdp.send('WebAuthn.getCredentials', { authenticatorId })
  ).credentials[0];
  expect(Buffer.from(credential.userHandle, 'base64').length).toBe(32);
  cli('auth', 'revoke', cli('auth', 'list').split('\t')[0]);
  await page.goto('http://localhost:8766/library');
  await expect(
    page.getByRole('button', { name: '使用 Passkey 登录', exact: true })
  ).toBeVisible();
  // Restore a populated backup offline; never revive its old login sessions.
  await Promise.all(
    [server, second].map(
      (proc) =>
        new Promise<void>((resolve) => {
          proc.once('exit', () => resolve());
          proc.kill();
        })
    )
  );
  cli('restore', '--input', backup, '--include-auth', '--yes');
  server = await start();
  await context.addCookies([
    {
      name: 'bookmarkd_session',
      value: session.value,
      url: 'http://localhost:8765',
    },
  ]);
  await page.goto('http://localhost:8765/login');
  await expect(
    page.getByRole('button', { name: '使用 Passkey 登录', exact: true })
  ).toBeVisible();
  expect(
    (
      await context.request.get('http://localhost:8765/api/v1/bookmarks')
    ).status()
  ).toBe(401);
  // Explicit auth rollback restores the old credential, as documented; fresh login is required.
  await page
    .getByRole('button', { name: '使用 Passkey 登录', exact: true })
    .click();
  await expect(page.getByRole('link', { name: '中文学习 Rust' })).toBeVisible();
  expect((await api('/api/v1/bookmarks')).value.total).toBe(3);
  expect(errors).toEqual([]);
});
