import { defineConfig, devices } from '@playwright/test';

// Bug bash sweep: collects findings (soft assertions) across pages, viewports and color schemes
// instead of stopping at the first failure. See docs/BUGBASH.md.
const out = process.env.BUGBASH_OUT ?? 'bugbash-output';

export default defineConfig({
  testDir: './tests/bugbash',
  globalSetup: './tests/global-setup.ts',
  outputDir: `${out}/test-results`,
  timeout: 120_000,
  fullyParallel: true,
  workers: 4,
  retries: 0,
  reporter: [['list'], ['json', { outputFile: `${out}/results.json` }], ['html', { open: 'never', outputFolder: `${out}/html` }]],
  use: { trace: 'retain-on-failure', screenshot: 'only-on-failure', locale: 'en-US' },
  projects: [
    { name: 'desktop', use: { ...devices['Desktop Chrome'], viewport: { width: 1366, height: 900 } } },
    { name: 'tablet', use: { ...devices['iPad Mini'], browserName: 'chromium' }, testMatch: /sweep\.spec/ },
    { name: 'phone', use: { ...devices['Pixel 7'] }, testMatch: /sweep\.spec|edge\.spec/ },
  ],
});
