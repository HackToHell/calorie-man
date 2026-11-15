// Visit every screen in light and dark mode, screenshot it, and record console errors, failed
// requests, sideways scrolling, broken numbers and serious accessibility violations.
import AxeBuilder from '@axe-core/playwright';
import { mkdirSync } from 'node:fs';
import { expect, test } from '../fixtures';

test.use({ seedDays: 90 });

const out = process.env.BUGBASH_OUT ?? 'bugbash-output';

const openAdd = async (p: import('@playwright/test').Page) => {
  const fab = p.getByTestId('fab');
  await ((await fab.isVisible()) ? fab : p.getByTestId('add-food')).click();
};

const screens: { name: string; hash: string; act?: (p: import('@playwright/test').Page) => Promise<void> }[] = [
  { name: 'today', hash: '#/today' },
  { name: 'log', hash: '#/log' },
  { name: 'log-nutrients', hash: '#/log', act: async (p) => p.getByTestId('toggle-nutrients').click() },
  { name: 'log-yesterday', hash: '#/log', act: async (p) => p.getByTestId('prev-day').click() },
  { name: 'add-library', hash: '#/today', act: openAdd },
  {
    name: 'add-portion',
    hash: '#/today',
    act: async (p) => {
      await openAdd(p);
      await p.getByTestId('food-search').fill('milk');
      await p.getByTestId('food-result').first().click();
    },
  },
  {
    name: 'add-online',
    hash: '#/today',
    act: async (p) => {
      await openAdd(p);
      await p.getByTestId('tab-online').click();
      await p.getByTestId('online-query').fill('costco milk');
      await p.getByTestId('online-search').click();
      await p.getByTestId('online-result').first().waitFor();
    },
  },
  {
    name: 'add-quick',
    hash: '#/today',
    act: async (p) => {
      await openAdd(p);
      await p.getByTestId('tab-quick').click();
    },
  },
  {
    name: 'add-new-food',
    hash: '#/today',
    act: async (p) => {
      await openAdd(p);
      await p.getByTestId('tab-new').click();
    },
  },
  { name: 'entry-edit', hash: '#/today', act: async (p) => p.getByTestId('entry').first().click() },
  { name: 'weight-log', hash: '#/today', act: async (p) => p.getByTestId('log-weight').click() },
  { name: 'copy', hash: '#/log', act: async (p) => p.getByTestId('copy-open').click() },
  { name: 'trends-30', hash: '#/trends' },
  { name: 'trends-7', hash: '#/trends?range=7' },
  { name: 'trends-365', hash: '#/trends?range=365' },
  { name: 'weight', hash: '#/weight' },
  { name: 'foods', hash: '#/foods' },
  { name: 'recipes', hash: '#/recipes' },
  { name: 'food-edit', hash: '#/foods/2' },
  {
    name: 'recipe-edit',
    hash: '#/recipes',
    act: async (p) => {
      await p.getByTestId('food-row').filter({ hasText: 'Morning oats' }).click();
      await p.getByTestId('ingredient-row').first().waitFor();
    },
  },
  { name: 'new-recipe', hash: '#/recipes/new' },
  { name: 'import', hash: '#/foods/import' },
  { name: 'goals', hash: '#/goals' },
  { name: 'profile', hash: '#/profile' },
  {
    name: 'user-menu',
    hash: '#/today',
    act: async (p) => {
      const sw = p.getByTestId('user-switcher');
      await ((await sw.isVisible()) ? sw : p.getByTestId('user-switcher-compact')).click();
    },
  },
];

for (const scheme of ['light', 'dark'] as const) {
  test(`sweep all screens (${scheme})`, async ({ page }, info) => {
    test.setTimeout(240_000);
    await page.emulateMedia({ colorScheme: scheme });
    const dir = `${out}/screens/${info.project.name}`;
    mkdirSync(dir, { recursive: true });
    const problems: string[] = [];
    page.on('console', (m) => m.type() === 'error' && problems.push(`console: ${m.text()}`));
    page.on('pageerror', (e) => problems.push(`pageerror: ${e.message}`));
    page.on('response', (r) => r.status() >= 500 && problems.push(`HTTP ${r.status()} ${r.url()}`));

    for (const s of screens) {
      await test.step(s.name, async () => {
        const before = problems.length;
        await page.goto('about:blank');
        await page.goto(`/${s.hash}`);
        await page.waitForLoadState('networkidle');
        if (s.act) {
          try {
            await s.act(page);
          } catch (e) {
            problems.push(`${s.name}: interaction failed: ${(e as Error).message.split('\n')[0]}`);
          }
          await page.waitForTimeout(250);
        }
        const overflow = await page.evaluate(() => document.documentElement.scrollWidth - window.innerWidth);
        expect.soft(overflow, `${s.name}: sideways scroll`).toBeLessThanOrEqual(1);
        const text = await page.locator('body').innerText();
        expect.soft(text, `${s.name}: broken numbers`).not.toMatch(/\bNaN\b|undefined|Infinity|\[object Object\]/);
        // Clipped text: elements whose content is wider than their box (ignores intentional ellipsis).
        const clipped = await page.evaluate(() =>
          [...document.querySelectorAll('button, .chip, label, h1, h2, h3, td')]
            .filter((el) => {
              const cs = getComputedStyle(el);
              return el.scrollWidth > el.clientWidth + 2 && cs.textOverflow !== 'ellipsis' && cs.overflowX !== 'auto' && el.clientWidth > 0;
            })
            .slice(0, 5)
            .map((el) => `${el.tagName.toLowerCase()} "${(el.textContent ?? '').trim().slice(0, 40)}"`),
        );
        expect.soft(clipped, `${s.name}: clipped text`).toEqual([]);
        {
          const axe = await new AxeBuilder({ page }).withTags(['wcag2a', 'wcag2aa']).analyze();
          const serious = axe.violations
            .filter((v) => v.impact === 'serious' || v.impact === 'critical')
            .map((v) => `${v.id} (${v.nodes.length}): ${v.help} e.g. ${v.nodes[0]?.target.join(' ')}`);
          expect.soft(serious, `${s.name}: accessibility`).toEqual([]);
        }
        await page.screenshot({ path: `${dir}/${scheme}-${s.name}.png`, fullPage: true });
        expect.soft(problems.slice(before), `${s.name}: console/network`).toEqual([]);
      });
    }
  });
}
