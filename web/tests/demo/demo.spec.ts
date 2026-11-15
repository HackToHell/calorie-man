import { readFileSync } from 'node:fs';
import AxeBuilder from '@axe-core/playwright';
import { expect, test, type Page } from '@playwright/test';

const fixture = (name: string) => readFileSync(new URL(`../../../tests/fixtures/${name}`, import.meta.url), 'utf8');

/** Console errors, failed responses and any request that reached the network for /api or /mcp. */
function watch(page: Page) {
  const problems: string[] = [];
  page.on('console', (m) => m.type() === 'error' && problems.push(`console: ${m.text()}`));
  page.on('pageerror', (e) => problems.push(`pageerror: ${e.message}`));
  page.on('request', (r) => {
    const path = new URL(r.url()).pathname;
    if (/^\/(api\/|mcp$)/.test(path)) problems.push(`hit the network: ${r.url()}`);
  });
  page.on('response', (r) => r.status() >= 400 && problems.push(`HTTP ${r.status()} ${r.url()}`));
  return problems;
}

async function open(page: Page) {
  await page.goto('./');
  await expect(page.getByTestId('budget-card')).toBeVisible();
}

const eaten = async (page: Page) => Number((await page.getByTestId('calories-eaten').innerText()).replace(/,/g, ''));

test('the demo says up front that nothing is saved', async ({ page }) => {
  await open(page);
  const banner = page.getByTestId('demo-banner');
  await expect(banner).toContainText('Live demo');
  await expect(banner).toContainText('Nothing is saved: reloading the page starts over.');
  await expect(banner.getByRole('link', { name: /Self-host it/ })).toHaveAttribute('href', 'https://github.com/HackToHell/calorie-man');
  // The sticky sidebar starts below the banner, then moves up when the banner is hidden.
  const sidebarTop = () => page.locator('aside.sidebar').evaluate((el) => el.getBoundingClientRect().top);
  expect(await sidebarTop()).toBeGreaterThanOrEqual((await banner.boundingBox())!.height - 1);
  await banner.getByRole('button', { name: 'Hide the demo notice' }).click();
  await expect(banner).toHaveCount(0);
  expect(await sidebarTop()).toBe(0);
  // Back on every load.
  await page.reload();
  await expect(page.getByTestId('demo-banner')).toBeVisible();
});

test('the real backend runs in the page: sample data, logging, switching people, and a reload starts over', async ({ page }) => {
  const problems = watch(page);
  await open(page);
  await expect(page.getByTestId('user-switcher')).toContainText('Alex');
  const before = await eaten(page);

  await page.getByTestId('add-snacks').click();
  const dlg = page.getByTestId('add-food-dialog');
  await dlg.getByTestId('food-search').fill('banana');
  await dlg.getByTestId('food-result').filter({ hasText: 'Banana' }).first().click();
  await dlg.getByTestId('add-entry').click();
  await dlg.getByTestId('add-done').click();
  await expect(page.getByTestId('meal-snacks')).toContainText('Banana');
  await expect.poll(() => eaten(page)).toBe(before + 105);

  // Sam has a separate diary in the same in-page database.
  await page.getByTestId('user-switcher').click();
  await page.getByTestId('user-option').filter({ hasText: 'Sam' }).click();
  await expect(page.getByTestId('user-switcher')).toContainText('Sam');
  await expect(page.getByTestId('budget-card')).toBeVisible();
  await page.getByTestId('user-switcher').click();
  await page.getByTestId('user-option').filter({ hasText: 'Alex' }).click();
  await expect(page.getByTestId('meal-snacks')).toContainText('Banana');

  // A reload throws the database away and seeds a fresh one.
  await page.reload();
  await expect(page.getByTestId('budget-card')).toBeVisible();
  await expect(page.getByTestId('user-switcher')).toContainText('Alex');
  await expect.poll(() => eaten(page)).toBe(before);
  expect(problems).toEqual([]);
});

test('online lookup goes straight from the browser to USDA; Open Food Facts text search is skipped', async ({ page }) => {
  const problems = watch(page);
  const upstream: string[] = [];
  await page.route('https://api.nal.usda.gov/**', (route) => {
    const url = new URL(route.request().url());
    upstream.push(url.pathname);
    const id = /\/food\/(\d+)/.exec(url.pathname)?.[1];
    const body = id ? fixture(`usda_food_${id}.json`) : fixture('usda_search_kirkland_milk.json');
    return route.fulfill({ status: 200, contentType: 'application/json', headers: { 'access-control-allow-origin': '*' }, body });
  });
  await page.route('https://search.openfoodfacts.org/**', (route) => {
    upstream.push('off-search');
    return route.abort();
  });
  await open(page);
  const before = await eaten(page);
  await page.getByTestId('add-breakfast').click();
  const dlg = page.getByTestId('add-food-dialog');
  await dlg.getByTestId('tab-online').click();
  await dlg.getByTestId('online-query').fill('costco milk');
  await dlg.getByTestId('online-search').click();
  const result = dlg.getByTestId('online-result').filter({ hasText: 'Whole Milk' }).filter({ hasText: 'Kirkland Signature' }).first();
  await expect(result).toBeVisible();
  await result.getByTestId('import-result').click();
  await expect(dlg.getByTestId('portion-preview')).toContainText('149');
  await dlg.getByTestId('add-entry').click();
  await dlg.getByTestId('add-done').click();
  await expect(page.getByTestId('meal-breakfast')).toContainText('Whole Milk');
  await expect.poll(() => eaten(page)).toBe(before + 149);
  expect(upstream.some((p) => p.endsWith('/foods/search'))).toBe(true);
  expect(upstream).not.toContain('off-search');
  expect(problems).toEqual([]);
});

test('exports download from the in-page backend; backups and MCP explain what needs a server', async ({ page }) => {
  await open(page);
  await page.goto('./#/profile');
  const download = page.waitForEvent('download');
  await page.getByRole('link', { name: 'Export everything (JSON)' }).click();
  const file = await download;
  expect(file.suggestedFilename()).toBe('calorie-man-export.json');
  const data = JSON.parse(readFileSync((await file.path())!, 'utf8'));
  expect(data.users.map((u: { user: { name: string } }) => u.user.name)).toEqual(['Alex', 'Sam']);
  expect(data.users[0].entries.length).toBeGreaterThan(50);

  await expect(page.getByRole('link', { name: 'Download database backup' })).toHaveCount(0);
  await expect(page.getByText('Database backups come with the self-hosted app')).toBeVisible();
  await expect(page.getByTestId('mcp-demo-note')).toContainText('no server for an AI to connect to');
  await expect(page.getByTestId('mcp-url')).toHaveValue('http://192.168.1.20:8484/mcp?user=Alex');
});

for (const scheme of ['light', 'dark'] as const) {
  test(`the demo notice passes axe in the ${scheme} theme`, async ({ page }) => {
    await page.emulateMedia({ colorScheme: scheme });
    await open(page);
    const r = await new AxeBuilder({ page }).include('[data-testid="demo-banner"]').withTags(['wcag2a', 'wcag2aa']).analyze();
    expect(r.violations.map((v) => `${v.id}: ${v.nodes.map((n) => n.target).join(', ')}`)).toEqual([]);
  });
}
