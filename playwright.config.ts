import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: 'tests/e2e',
  outputDir: 'build/test-results',
  reporter: 'line',
  workers: 1,
  use: {
    baseURL: 'http://127.0.0.1:1420',
    trace: 'retain-on-failure',
  },
  projects: [
    { name: 'chromium', use: { ...devices['Desktop Chrome'] } },
    { name: 'narrow', use: { browserName: 'chromium', viewport: { width: 240, height: 720 } } },
  ],
  webServer: {
    command: 'node scripts/build/test-server.mjs',
    url: 'http://127.0.0.1:1420',
    reuseExistingServer: false,
  },
});
