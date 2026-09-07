import { expect, test, type Page } from '@playwright/test';
import { buildLayout } from '../../scripts/build/layout.mjs';

const now = Date.UTC(2026, 8, 5, 3, 0, 0);
const field = <T>(value: T) => ({
  value,
  source: 'local',
  observedAt: now,
  quality: 'fresh',
});
const snapshot = {
  revision: 4,
  sessions: [
    {
      id: 'root:session',
      path: '/workspace/a-project-with-a-long-but-readable-name',
      project: 'a-project-with-a-long-but-readable-name',
      activity: field('running'),
      model: field('gpt-5'),
      effort: field('high'),
      usage: field({ input: 1200, cachedInput: 800, output: 300, total: 1500 }),
      lastUsage: field({ input: 200, cachedInput: 100, output: 40, total: 240 }),
      contextLimit: field(200000),
      contextUsed: field(92_992),
      latestAt: now,
      turnStartedAt: now - 60_000,
      durationMs: null,
    },
  ],
  quotas: [
    {
      id: 'default',
      name: 'Default',
      windows: [
        { remaining: field(64), minutes: 300, resetsAt: now + 3_600_000 },
        { remaining: field(38), minutes: 10080, resetsAt: now + 86_400_000 },
      ],
      creditBalance: null,
      unlimitedCredits: false,
    },
  ],
  connection: 'connected',
  account: 'chatgpt',
  provider: null,
  updatedAt: now,
  error: null,
  localError: null,
  refreshing: false,
};
const settings = {
  roots: ['/data/root'],
  executable: 'codex',
  theme: 'system',
  fontSize: 13,
  alwaysOnTop: true,
  collapsed: false,
  autoFollow: true,
  pinnedSession: null,
  selectedBucket: null,
  notifications: true,
  approvalSound: 'bell',
  completionSound: 'bell',
  quotaSound: 'battery',
  muted: false,
  lowQuota: 10,
  position: null,
};

async function mock(page: Page, initialSnapshot = snapshot) {
  await page.route('**/api/**', async (route) => {
    const name = new URL(route.request().url()).pathname.slice(5);
    if (route.request().method() === 'GET') {
      await route.fulfill({ json: name === 'snapshot' ? initialSnapshot : settings });
      return;
    }
    await route.fulfill({ status: 204, body: '' });
  });
}

async function mockSound(page: Page) {
  await page.addInitScript(() => {
    Object.defineProperty(HTMLMediaElement.prototype, 'play', {
      configurable: true,
      value() {
        const target = window as typeof window & { __soundPlays?: number };
        target.__soundPlays = (target.__soundPlays || 0) + 1;
        return Promise.resolve();
      },
    });
  });
}

async function mockNative(page: Page) {
  await page.addInitScript(
    ({ snapshot, settings }) => {
      let callbackId = 0;
      const listeners = new Map<string, number[]>();
      type Invocation = { command: string; args: Record<string, unknown> };
      Object.defineProperty(globalThis, 'isTauri', { value: true, configurable: true });
      Object.assign(window, { __nativeInvocations: [] as Invocation[] });
      Object.defineProperty(window, '__TAURI_INTERNALS__', {
        configurable: true,
        value: {
          metadata: { currentWindow: { label: 'main' } },
          transformCallback(callback: (...args: unknown[]) => unknown) {
            const id = ++callbackId;
            Object.assign(window, { [`_${id}`]: callback });
            return id;
          },
          unregisterCallback(id: number) {
            delete (window as unknown as Record<string, unknown>)[`_${id}`];
          },
          async invoke(command: string, args: Record<string, unknown> = {}) {
            (
              window as typeof window & {
                __nativeInvocations: Invocation[];
              }
            ).__nativeInvocations.push({ command, args });
            if (command === 'snapshot') return snapshot;
            if (command === 'preferences') return settings;
            if (command === 'plugin:event|listen') {
              const event = String(args.event);
              listeners.set(event, [...(listeners.get(event) || []), Number(args.handler)]);
              return ++callbackId;
            }
          },
        },
      });
      Object.assign(window, {
        __emitNative(event: string, payload: unknown) {
          for (const id of listeners.get(event) || []) {
            const callback = (window as unknown as Record<string, (value: unknown) => void>)[
              `_${id}`
            ];
            callback?.({ event, id, payload });
          }
        },
      });
    },
    { snapshot, settings },
  );
}

test.beforeEach(async ({ page }) => {
  await mock(page);
  await page.clock.install({ time: now });
  await page.goto('/', { waitUntil: 'networkidle' });
  await expect(page.getByText('a-project-with-a-long-but-readable-name')).toBeVisible();
});

test('summary, details, sessions and settings remain operable', async ({ page }) => {
  const brand = page.locator('header .brand-icon');
  await expect(brand).toBeVisible();
  await expect(brand).toHaveAttribute('src', /icon(?:-[\w-]+)?\.svg/);
  await expect(page.locator('.session-button svg')).toHaveCount(0);
  await expect(page.getByText('64%')).toBeVisible();
  await expect(page.locator('meter').nth(0)).toHaveAttribute('data-level', 'high');
  await expect(page.locator('meter').nth(1)).toHaveAttribute('data-level', 'medium');
  await expect(page.locator('.text-tool span')).toHaveText('1 session');
  await page.getByRole('button', { name: /a-project-with/ }).click();
  await expect(page.locator('.view-title')).toContainText('Session details');
  await expect(page.getByText('Token usage')).toBeVisible();
  const details = page.getByRole('region', { name: 'Session details' });
  await expect(details.getByText('92,992')).toBeVisible();
  await expect(details.getByRole('heading', { name: 'Account quota' })).toHaveCount(0);
  await expect(details.getByRole('heading', { name: 'Connection' })).toHaveCount(0);
  await page.getByRole('button', { name: 'Session list' }).click();
  await expect(page.getByText('/workspace/a-project-with-a-long-but-readable-name')).toBeVisible();
  await page.getByRole('button', { name: 'Settings' }).click();
  await expect(page.getByLabel('Theme')).toBeVisible();
});

test('expanded, collapsed, dark and narrow cards keep rounded transparent corners', async ({
  page,
}, testInfo) => {
  const card = page.locator('main');
  const corners = async () =>
    page.evaluate(() => ({
      root: getComputedStyle(document.documentElement).backgroundColor,
      body: getComputedStyle(document.body).backgroundColor,
      surface: getComputedStyle(document.querySelector('main')!).backgroundColor,
      radius: getComputedStyle(document.querySelector('main')!).borderTopLeftRadius,
      overflow: getComputedStyle(document.querySelector('main')!).overflow,
      width: document.querySelector('main')!.getBoundingClientRect().width,
    }));

  await expect(card).toHaveCSS('border-top-left-radius', '10px');
  expect(await corners()).toMatchObject({
    root: 'rgba(0, 0, 0, 0)',
    body: 'rgba(0, 0, 0, 0)',
    radius: '10px',
    overflow: 'hidden',
  });
  if (testInfo.project.name === 'narrow') expect((await corners()).width).toBe(240);

  await page.getByRole('button', { name: 'Collapse' }).click();
  await expect(card).toHaveClass(/collapsed/);
  await expect(card).toHaveCSS('border-top-left-radius', '10px');

  await page.getByRole('button', { name: 'Expand' }).click();
  await page.getByRole('button', { name: 'Settings' }).click();
  await page.getByLabel('Theme').selectOption('dark');
  await page.getByRole('button', { name: 'Save', exact: true }).click();
  const dark = await corners();
  expect(dark.surface).not.toBe('rgba(0, 0, 0, 0)');
  expect(dark).toMatchObject({
    root: 'rgba(0, 0, 0, 0)',
    body: 'rgba(0, 0, 0, 0)',
    radius: '10px',
    overflow: 'hidden',
  });
});

test('the session heading has no pin control', async ({ page }) => {
  await expect(page.getByRole('button', { name: /^(Unpin|Pin) session$/ })).toHaveCount(0);
  await expect(page.locator('.session-heading svg')).toHaveCount(0);
});

test('refresh remains usable while an account query is already running', async ({ page }) => {
  await page.unroute('**/api/**');
  await mock(page, { ...snapshot, revision: snapshot.revision + 1, refreshing: true });
  await page.reload({ waitUntil: 'networkidle' });

  const refresh = page.getByRole('button', { name: 'Refresh' });
  await expect(refresh).toBeEnabled();
  const request = page.waitForRequest(
    (request) => request.url().endsWith('/api/refresh') && request.method() === 'POST',
  );
  await refresh.click();
  await request;
});

test('the card surface is draggable without a dedicated control or cursor override', async ({
  page,
}) => {
  await expect(page.getByRole('button', { name: 'Move window' })).toHaveCount(0);
  await expect(page.locator('main')).toHaveCSS('cursor', 'auto');
});

test('the expanded card exposes a draggable bottom resize edge', async ({ page }) => {
  const edge = page.getByRole('separator', { name: 'Resize height' });
  await expect(edge).toBeVisible();
  await expect(edge).toHaveCSS('cursor', 'ns-resize');
  await expect(edge).toHaveAttribute('data-no-drag', '');
});

test('native views request their own widths and bottom dragging changes only height', async ({
  page,
}) => {
  await mockNative(page);
  await page.setViewportSize({ width: 300, height: 400 });
  await page.reload({ waitUntil: 'networkidle' });

  await page.getByRole('button', { name: 'Session list' }).click();
  await expect
    .poll(() =>
      page.evaluate(() => {
        const calls = (
          window as typeof window & {
            __nativeInvocations: Array<{
              command: string;
              args: Record<string, unknown>;
            }>;
          }
        ).__nativeInvocations;
        return calls
          .slice()
          .reverse()
          .find((call) => call.command === 'resize')?.args.width;
      }),
    )
    .toBe(380);

  await page.getByRole('button', { name: 'Settings' }).click();
  await expect
    .poll(() =>
      page.evaluate(() => {
        const calls = (
          window as typeof window & {
            __nativeInvocations: Array<{
              command: string;
              args: Record<string, unknown>;
            }>;
          }
        ).__nativeInvocations;
        return calls
          .slice()
          .reverse()
          .find((call) => call.command === 'resize')?.args.width;
      }),
    )
    .toBe(400);

  await page.getByRole('button', { name: 'Close panel' }).click();
  const before = await page.evaluate(
    () =>
      (
        window as typeof window & {
          __nativeInvocations: unknown[];
        }
      ).__nativeInvocations.length,
  );
  const edge = page.getByRole('separator', { name: 'Resize height' });
  const box = await edge.boundingBox();
  expect(box).not.toBeNull();
  await page.mouse.move(box!.x + box!.width / 2, box!.y + box!.height / 2);
  await page.mouse.down();
  await page.mouse.move(box!.x + box!.width / 2, box!.y + box!.height / 2 + 40);
  await page.mouse.up();

  await expect
    .poll(() =>
      page.evaluate((before) => {
        const calls = (
          window as typeof window & {
            __nativeInvocations: Array<{
              command: string;
              args: Record<string, unknown>;
            }>;
          }
        ).__nativeInvocations.slice(before);
        return calls.some(
          (call) =>
            call.command === 'resize' &&
            call.args.width === innerWidth &&
            Number(call.args.height) > innerHeight,
        );
      }, before),
    )
    .toBe(true);
  const nativeCalls = await page.evaluate(() =>
    (
      window as typeof window & {
        __nativeInvocations: Array<{ command: string }>;
      }
    ).__nativeInvocations.map((call) => call.command),
  );
  expect(nativeCalls).not.toContain('plugin:window|start_resize_dragging');
});

test('an expanded panel fills a window resized from its native edge', async ({
  page,
}, testInfo) => {
  await mockNative(page);
  await page.setViewportSize({ width: 360, height: 480 });
  await page.reload({ waitUntil: 'networkidle' });
  await page.getByRole('button', { name: /a-project-with/ }).click();

  await page.setViewportSize({ width: 360, height: 720 });

  await expect
    .poll(() =>
      page.evaluate(
        () => document.querySelector('main')!.getBoundingClientRect().height === innerHeight,
      ),
    )
    .toBe(true);
  const layout = await page.evaluate(() => {
    const card = document.querySelector('main')!.getBoundingClientRect();
    const panel = document.querySelector('.scroll-view')!.getBoundingClientRect();
    return {
      cardHeight: card.height,
      viewportHeight: innerHeight,
      bottomGap: card.bottom - panel.bottom,
    };
  });
  expect(layout.cardHeight).toBe(layout.viewportHeight);
  expect(layout.bottomGap).toBeLessThanOrEqual(8);
  await page.screenshot({
    path: `${buildLayout.testScreenshots}/resized-${testInfo.project.name}.png`,
    fullPage: true,
  });
});

test('collapsed mode has stable controls and no horizontal overflow', async ({
  page,
}, testInfo) => {
  await page.getByRole('button', { name: 'Collapse' }).click();
  await expect(page.locator('.collapsed-row img')).toHaveCount(0);
  const status = page.locator('.collapsed-status');
  await expect(status).toHaveText('Running');
  await expect(status).toHaveAttribute('data-status', 'running');
  const expand = page.getByRole('button', { name: 'Expand' });
  await expect(expand).toBeVisible();
  await expect(expand.locator('svg')).toHaveCount(1);
  await expect(page.locator('.collapsed-row > strong')).toHaveCSS('padding-left', '4px');
  const order = await page.evaluate(() => {
    const status = document.querySelector('.collapsed-status')!.getBoundingClientRect();
    const quota = document.querySelector('.collapsed-row .numeric')!.getBoundingClientRect();
    return status.right <= quota.left;
  });
  expect(order).toBe(true);
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth - innerWidth);
  expect(overflow).toBeLessThanOrEqual(0);
  await page.screenshot({
    path: `${buildLayout.testScreenshots}/collapsed-${testInfo.project.name}.png`,
  });
});

test('session history stays compact and scrolls inside the card', async ({ page }, testInfo) => {
  await page.unroute('**/api/**');
  await mock(page);
  await page.route('**/api/preferences', (route) =>
    route.fulfill({ json: { ...settings, fontSize: 15 } }),
  );
  await page.route('**/api/snapshot', (route) =>
    route.fulfill({
      json: {
        ...snapshot,
        revision: 5,
        sessions: Array.from({ length: 16 }, (_, index) => ({
          ...snapshot.sessions[0],
          id: `root:session-${index}`,
          project: `project-${index}`,
          path: `/workspace/project-${index}`,
          activity: field(index % 2 === 0 ? 'running' : 'waitingApproval'),
          latestAt: now - index * 60_000,
        })),
      },
    }),
  );
  const width = testInfo.project.name === 'narrow' ? 240 : 380;
  await page.setViewportSize({ width, height: 504 });
  await page.reload({ waitUntil: 'networkidle' });
  await page.getByRole('button', { name: 'Session list' }).click();

  const layout = await page.evaluate(() => ({
    cardHeight: document.querySelector('main')!.getBoundingClientRect().height,
    pageOverflow: document.documentElement.scrollHeight - innerHeight,
    pageWidthOverflow: document.documentElement.scrollWidth - innerWidth,
    listOverflow:
      document.querySelector('.scroll-view')!.scrollHeight -
      document.querySelector('.scroll-view')!.clientHeight,
    statusRights: [...document.querySelectorAll('.session-status')].map(
      (status) => status.getBoundingClientRect().right,
    ),
  }));
  expect(layout.cardHeight).toBeLessThanOrEqual(480);
  expect(layout.pageOverflow).toBeLessThanOrEqual(0);
  expect(layout.pageWidthOverflow).toBeLessThanOrEqual(0);
  expect(layout.listOverflow).toBeGreaterThan(0);
  expect(Math.max(...layout.statusRights) - Math.min(...layout.statusRights)).toBeLessThanOrEqual(
    1,
  );
  await expect(page.locator('.session-item svg')).toHaveCount(0);
  await page.screenshot({
    path: `${buildLayout.testScreenshots}/sessions-${testInfo.project.name}.png`,
    fullPage: true,
  });

  await page.setViewportSize({ width, height: 320 });
  const compact = await page.evaluate(() => ({
    cardHeight: document.querySelector('main')!.getBoundingClientRect().height,
    pageOverflow: document.documentElement.scrollHeight - innerHeight,
    listOverflow:
      document.querySelector('.scroll-view')!.scrollHeight -
      document.querySelector('.scroll-view')!.clientHeight,
  }));
  expect(compact.cardHeight).toBeLessThanOrEqual(320);
  expect(compact.pageOverflow).toBeLessThanOrEqual(0);
  expect(compact.listOverflow).toBeGreaterThan(0);
});

test('settings controls align without text overlap', async ({ page }, testInfo) => {
  const width = testInfo.project.name === 'narrow' ? 240 : 400;
  await page.setViewportSize({ width, height: 620 });
  await page.getByRole('button', { name: 'Settings' }).click();

  const layout = await page.evaluate(() => {
    const rows = [...document.querySelectorAll('.setting')].map((row) => {
      const label = row.querySelector('.setting-label')!.getBoundingClientRect();
      const control = row.querySelector('input, select')!.getBoundingClientRect();
      return { labelRight: label.right, controlLeft: control.left, controlRight: control.right };
    });
    const rights = rows.map((row) => row.controlRight);
    return {
      overflow: document.documentElement.scrollWidth - innerWidth,
      rightDrift: Math.max(...rights) - Math.min(...rights),
      overlaps: rows.filter((row) => row.labelRight > row.controlLeft).length,
    };
  });
  expect(layout.overflow).toBeLessThanOrEqual(0);
  expect(layout.rightDrift).toBeLessThanOrEqual(1);
  expect(layout.overlaps).toBe(0);
  await page.screenshot({
    path: `${buildLayout.testScreenshots}/settings-${testInfo.project.name}.png`,
    fullPage: true,
  });
});

test('cli executable whitespace is removed before saving', async ({ page }) => {
  await page.getByRole('button', { name: 'Settings' }).click();
  await page.getByLabel('CLI executable').fill('  tool  ');
  const request = page.waitForRequest(
    (request) => request.url().endsWith('/api/save_preferences') && request.method() === 'POST',
  );

  await page.getByRole('button', { name: 'Save', exact: true }).click();

  expect((await request).postDataJSON().settings.executable).toBe('tool');
});

test('command approval sound can be previewed and disabled', async ({ page }) => {
  await mockSound(page);
  await page.reload({ waitUntil: 'networkidle' });
  await page.getByRole('button', { name: 'Settings' }).click();
  const sound = page.getByRole('combobox', { name: 'Command approval sound' });
  await expect(sound).toHaveValue('bell');
  await page.getByRole('button', { name: 'Preview command approval sound' }).click();
  await expect
    .poll(() =>
      page.evaluate(() => (window as typeof window & { __soundPlays?: number }).__soundPlays || 0),
    )
    .toBe(1);
  await sound.selectOption('off');
  await expect(page.getByRole('button', { name: 'Preview command approval sound' })).toBeDisabled();
  const request = page.waitForRequest(
    (request) => request.url().endsWith('/api/save_preferences') && request.method() === 'POST',
  );
  await page.getByRole('button', { name: 'Save', exact: true }).click();
  expect((await request).postDataJSON().settings.approvalSound).toBe('off');
});

test('task completion sound is selectable and can be disabled', async ({ page }) => {
  await page.getByRole('button', { name: 'Settings' }).click();
  const sound = page.getByRole('combobox', { name: 'Task completion sound' });
  await expect(sound).toHaveValue('bell');
  await sound.selectOption('ding');
  await sound.selectOption('off');
  await expect(page.getByRole('button', { name: 'Preview task completion sound' })).toBeDisabled();
  const request = page.waitForRequest(
    (request) => request.url().endsWith('/api/save_preferences') && request.method() === 'POST',
  );
  await page.getByRole('button', { name: 'Save', exact: true }).click();
  expect((await request).postDataJSON().settings.completionSound).toBe('off');
});

test('task completion sound can be previewed from settings', async ({ page }) => {
  await mockSound(page);
  await page.reload({ waitUntil: 'networkidle' });
  await page.getByRole('button', { name: 'Settings' }).click();
  await page.getByRole('button', { name: 'Preview task completion sound' }).click();
  await expect
    .poll(() =>
      page.evaluate(() => (window as typeof window & { __soundPlays?: number }).__soundPlays || 0),
    )
    .toBe(1);
});

test('quota warning sound can be selected and previewed', async ({ page }) => {
  await mockSound(page);
  await page.reload({ waitUntil: 'networkidle' });
  await page.getByRole('button', { name: 'Settings' }).click();
  const sound = page.getByRole('combobox', { name: 'Quota warning sound' });
  await expect(sound).toHaveValue('battery');
  await sound.selectOption('alert');
  await page.getByRole('button', { name: 'Preview quota warning sound' }).click();
  await expect
    .poll(() =>
      page.evaluate(() => (window as typeof window & { __soundPlays?: number }).__soundPlays || 0),
    )
    .toBe(1);
  const request = page.waitForRequest(
    (request) => request.url().endsWith('/api/save_preferences') && request.method() === 'POST',
  );
  await page.getByRole('button', { name: 'Save', exact: true }).click();
  expect((await request).postDataJSON().settings.quotaSound).toBe('alert');
});

test('desktop approval and completion events play their configured sounds', async ({ page }) => {
  await mockSound(page);
  await mockNative(page);
  await page.reload({ waitUntil: 'networkidle' });
  await expect
    .poll(() =>
      page.evaluate(() =>
        (
          window as typeof window & {
            __nativeInvocations: Array<{ command: string; args: Record<string, unknown> }>;
          }
        ).__nativeInvocations.some(
          ({ command, args }) => command === 'plugin:event|listen' && args.event === 'play-sound',
        ),
      ),
    )
    .toBe(true);
  await page.evaluate(() =>
    (
      window as typeof window & {
        __emitNative: (event: string, payload: unknown) => void;
      }
    ).__emitNative('play-sound', 'approvalBell'),
  );
  await expect
    .poll(() =>
      page.evaluate(() => (window as typeof window & { __soundPlays?: number }).__soundPlays || 0),
    )
    .toBe(1);
  await page.evaluate(() =>
    (
      window as typeof window & {
        __emitNative: (event: string, payload: unknown) => void;
      }
    ).__emitNative('play-sound', 'completionDing'),
  );
  await expect
    .poll(() =>
      page.evaluate(() => (window as typeof window & { __soundPlays?: number }).__soundPlays || 0),
    )
    .toBe(2);
});

test('large text, themes and degraded states remain readable', async ({ page }, testInfo) => {
  await page.getByRole('button', { name: 'Settings' }).click();
  await page.getByLabel('Font size').fill('18');
  await page.getByLabel('Theme').selectOption('dark');
  await page.getByRole('button', { name: 'Save', exact: true }).click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await page.getByRole('button', { name: 'Close panel' }).click();
  await page.screenshot({
    path: `${buildLayout.testScreenshots}/dark-${testInfo.project.name}.png`,
    fullPage: true,
  });
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth - innerWidth);
  expect(overflow).toBeLessThanOrEqual(0);

  await page.unroute('**/api/**');
  await mock(page);
  await page.route('**/api/snapshot', (route) =>
    route.fulfill({
      json: {
        ...snapshot,
        revision: 5,
        quotas: [],
        connection: 'offline',
        error: 'query-timeout',
      },
    }),
  );
  await page.reload({ waitUntil: 'networkidle' });
  await expect(page.locator('footer').getByText('Offline')).toBeVisible();
  await expect(page.locator('main > .source-error')).toHaveText('Refresh timed out');
  await page.getByRole('button', { name: /a-project-with/ }).click();
  await expect(page.getByRole('region', { name: 'Session details' }).locator('.error')).toHaveCount(
    0,
  );
});

test('external provider uses its configured name', async ({ page }) => {
  const provider = `provider-${'x'.repeat(119)}`;
  await page.unroute('**/api/**');
  await mock(page);
  await page.route('**/api/snapshot', (route) =>
    route.fulfill({
      json: {
        ...snapshot,
        revision: 5,
        quotas: [],
        account: 'externalProvider',
        connection: 'externalProvider',
        provider,
      },
    }),
  );
  await page.reload({ waitUntil: 'networkidle' });

  const label = page.locator('footer .connection');
  await expect(label).toHaveAttribute('title', provider);
  await expect(label.getByText(provider, { exact: true })).toBeVisible();
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth - innerWidth);
  expect(overflow).toBeLessThanOrEqual(0);
});
