import { defineConfig, devices } from '@playwright/test';

const port = 1421;

export default defineConfig({
  testDir: 'tests/e2e',
  outputDir: 'build/test-results',
  reporter: 'line',
  workers: 1,
  use: {
    baseURL: `http://127.0.0.1:${port}`,
    trace: 'retain-on-failure',
  },
  projects: [
    { name: 'chromium', use: { ...devices['Desktop Chrome'] } },
    { name: 'narrow', use: { browserName: 'chromium', viewport: { width: 240, height: 720 } } },
  ],
  webServer: {
    command: `node scripts/build/test-server.mjs ${port}`,
    url: `http://127.0.0.1:${port}`,
    reuseExistingServer: false,
  },
});
