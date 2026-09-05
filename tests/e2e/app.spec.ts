import { expect, test, type Page } from '@playwright/test';

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
      rootId: 'root',
      path: '/workspace/a-project-with-a-long-but-readable-name',
      project: 'a-project-with-a-long-but-readable-name',
      version: '0.153.4',
      supported: true,
      activity: field('running'),
      model: field('gpt-5'),
      effort: field('high'),
      usage: field({ input: 1200, cachedInput: 800, output: 300, total: 1500 }),
      lastUsage: field({ input: 200, cachedInput: 100, output: 40, total: 240 }),
      contextLimit: field(200000),
      contextUsed: { value: null, source: 'local', observedAt: null, quality: 'unsupported' },
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
  version: '0.153.4',
  updatedAt: now,
  error: null,
  localError: null,
  refreshing: false,
};
const settings = {
  schema: 1,
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
  muted: false,
  lowQuota: 10,
  position: null,
};

async function mock(page: Page) {
  await page.route('**/api/**', async (route) => {
    const name = new URL(route.request().url()).pathname.slice(5);
    if (route.request().method() === 'GET') {
      await route.fulfill({ json: name === 'snapshot' ? snapshot : settings });
      return;
    }
    await route.fulfill({ status: 204, body: '' });
  });
}

test.beforeEach(async ({ page }) => {
  await mock(page);
  await page.clock.install({ time: now });
  await page.goto('/', { waitUntil: 'networkidle' });
  await expect(page.getByText('a-project-with-a-long-but-readable-name')).toBeVisible();
});

test('summary, details, sessions and settings remain operable', async ({ page }) => {
  await expect(page.getByText('64%')).toBeVisible();
  await page.getByRole('button', { name: /a-project-with/ }).click();
  await expect(page.getByText('Token usage')).toBeVisible();
  await page.getByRole('button', { name: 'Session list' }).click();
  await expect(page.getByText('/workspace/a-project-with-a-long-but-readable-name')).toBeVisible();
  await page.getByRole('button', { name: 'Settings' }).click();
  await expect(page.getByLabel('Theme')).toBeVisible();
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

test('collapsed mode has stable controls and no horizontal overflow', async ({ page }) => {
  await page.getByRole('button', { name: 'Collapse' }).click();
  await expect(page.getByRole('button', { name: 'Expand' })).toBeVisible();
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth - innerWidth);
  expect(overflow).toBeLessThanOrEqual(0);
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
          latestAt: now - index * 60_000,
        })),
      },
    }),
  );
  await page.setViewportSize({ width: 360, height: 504 });
  await page.reload({ waitUntil: 'networkidle' });
  await page.getByRole('button', { name: 'Session list' }).click();

  const layout = await page.evaluate(() => ({
    cardHeight: document.querySelector('main')!.getBoundingClientRect().height,
    pageOverflow: document.documentElement.scrollHeight - innerHeight,
    listOverflow:
      document.querySelector('.scroll-view')!.scrollHeight -
      document.querySelector('.scroll-view')!.clientHeight,
  }));
  expect(layout.cardHeight).toBeLessThanOrEqual(480);
  expect(layout.pageOverflow).toBeLessThanOrEqual(0);
  expect(layout.listOverflow).toBeGreaterThan(0);
  await page.screenshot({
    path: `build/screenshots/sessions-${testInfo.project.name}.png`,
    fullPage: true,
  });

  await page.setViewportSize({ width: 360, height: 320 });
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

test('large text, themes and degraded states remain readable', async ({ page }, testInfo) => {
  await page.getByRole('button', { name: 'Settings' }).click();
  await page.getByLabel('Font size').fill('18');
  await page.getByLabel('Theme').selectOption('dark');
  await page.getByRole('button', { name: 'Save', exact: true }).click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await page.getByRole('button', { name: 'Close panel' }).click();
  await page.screenshot({
    path: `build/screenshots/dark-${testInfo.project.name}.png`,
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
  await page.getByRole('button', { name: /a-project-with/ }).click();
  await expect(page.getByText('Refresh timed out')).toBeVisible();
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
