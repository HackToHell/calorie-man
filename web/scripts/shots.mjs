// Screenshot every page at desktop and phone sizes and report console errors / failed requests.
// Usage: node scripts/shots.mjs [baseUrl] [outDir]
import { chromium } from '@playwright/test';
import { mkdirSync, writeFileSync } from 'node:fs';

const base = process.argv[2] ?? 'http://127.0.0.1:18484';
const out = process.argv[3] ?? 'bugbash-output/shots';
mkdirSync(out, { recursive: true });

const openAdd = async (p) => ((await p.getByTestId('fab').isVisible()) ? p.getByTestId('fab') : p.getByTestId('add-food')).click();
const pages = [
  ['today', '#/today'],
  ['log', '#/log'],
  ['log-nutrients', '#/log', async (p) => p.getByTestId('toggle-nutrients').click()],
  ['add-food', '#/today', openAdd],
  ['add-food-portion', '#/today', async (p) => {
    await openAdd(p);
    await p.getByTestId('food-search').fill('oats');
    await p.getByTestId('food-result').first().click();
  }],
  ['trends', '#/trends'],
  ['trends-90', '#/trends?range=90'],
  ['weight', '#/weight'],
  ['foods', '#/foods'],
  ['recipes', '#/recipes'],
  ['food-edit', '#/foods/2'],
  ['goals', '#/goals'],
  ['profile', '#/profile'],
];
const viewports = [
  ['desktop', { width: 1280, height: 900 }],
  ['phone', { width: 390, height: 844 }],
];
const schemes = (process.env.SCHEMES ?? 'light').split(',');

const browser = await chromium.launch();
const problems = [];
for (const scheme of schemes) {
  for (const [vp, size] of viewports) {
    const ctx = await browser.newContext({ viewport: size, colorScheme: scheme, deviceScaleFactor: vp === 'phone' ? 2 : 1 });
    const page = await ctx.newPage();
    page.on('console', (m) => m.type() === 'error' && problems.push(`[${vp}] console: ${m.text()}`));
    page.on('pageerror', (e) => problems.push(`[${vp}] pageerror: ${e.message}`));
    page.on('response', (r) => r.status() >= 500 && problems.push(`[${vp}] HTTP ${r.status()} ${r.url()}`));
    for (const [name, hash, action] of pages) {
      // A hash-only change does not reload the page, so start each shot from a clean document.
      await page.goto('about:blank');
      await page.goto(`${base}/${hash}`);
      await page.waitForLoadState('networkidle');
      await page.waitForTimeout(250);
      if (action) {
        try {
          await action(page);
          await page.waitForTimeout(350);
        } catch (e) {
          problems.push(`[${vp}] ${name}: action failed: ${e.message.split('\n')[0]}`);
        }
      }
      const overflow = await page.evaluate(() => document.documentElement.scrollWidth - window.innerWidth);
      if (overflow > 1) problems.push(`[${vp}] ${name}: horizontal overflow ${overflow}px`);
      await page.screenshot({ path: `${out}/${scheme}-${vp}-${name}.png`, fullPage: true });
    }
    await ctx.close();
  }
}
await browser.close();
writeFileSync(`${out}/problems.txt`, problems.join('\n') + '\n');
console.log(problems.length ? problems.join('\n') : 'no console errors, failed requests or overflow');
