import { defineConfig, devices } from '@playwright/test';

// The GitHub Pages demo: `make demo` builds dist-demo, served here under /calorie-man/ like Pages does.
const port = 18710;

export default defineConfig({
  testDir: './tests/demo',
  timeout: 30_000,
  expect: { timeout: 5_000 },
  fullyParallel: true,
  workers: 4,
  reporter: [['list']],
  use: {
    baseURL: `http://127.0.0.1:${port}/calorie-man/`,
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
    locale: 'en-US',
  },
  webServer: {
    command: `node scripts/serve-demo.mjs --port ${port}`,
    url: `http://127.0.0.1:${port}/calorie-man/`,
    reuseExistingServer: false,
  },
  projects: [
    { name: 'desktop', use: { ...devices['Desktop Chrome'], viewport: { width: 1280, height: 900 } }, testIgnore: /phone\.spec/ },
    { name: 'phone', use: { ...devices['Pixel 7'] }, testMatch: /phone\.spec/ },
  ],
});
