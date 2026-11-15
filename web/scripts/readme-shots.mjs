// Screenshots for the README, taken from the demo build (same sample data as the live demo).
// Usage: make demo && node scripts/readme-shots.mjs   → docs/screenshots/*.webp
import { spawn } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
import { chromium, devices } from 'playwright';

const port = 18720;
const base = `http://127.0.0.1:${port}/calorie-man/`;
const out = new URL('../../docs/screenshots/', import.meta.url);
mkdirSync(out, { recursive: true });

const server = spawn('node', ['scripts/serve-demo.mjs', '--port', String(port)], { stdio: 'ignore' });
await new Promise((r) => setTimeout(r, 600));
const browser = await chromium.launch();

// Early evening: breakfast and lunch are logged and dinner is coming up on the meal-timing arc.
const now = new Date();
const clock = new Date(now.getFullYear(), now.getMonth(), now.getDate(), 18, 40);

async function context(opts, scheme) {
  const ctx = await browser.newContext({ ...opts, colorScheme: scheme, locale: 'en-US' });
  await ctx.addInitScript(() => localStorage.setItem('calorie-man.theme', 'system'));
  return ctx;
}

async function open(ctx, hash = '') {
  const page = await ctx.newPage();
  await page.clock.setFixedTime(clock);
  await page.goto(base + hash);
  await page.getByTestId('demo-banner').waitFor();
  // The README shows the app itself; the banner is for visitors of the live demo.
  await page.getByRole('button', { name: 'Hide the demo notice' }).click();
  await page.waitForTimeout(400);
  return page;
}

/** PNG → WebP with Chromium's encoder, which keeps UI text crisp at a fraction of the size. */
async function save(name, png, quality = 0.9) {
  const page = await browser.newPage();
  const dataUrl = await page.evaluate(
    async ({ b64, quality }) => {
      const img = new Image();
      img.src = `data:image/png;base64,${b64}`;
      await img.decode();
      const c = Object.assign(document.createElement('canvas'), { width: img.width, height: img.height });
      c.getContext('2d').drawImage(img, 0, 0);
      return c.toDataURL('image/webp', quality);
    },
    { b64: png.toString('base64'), quality },
  );
  await page.close();
  const buf = Buffer.from(dataUrl.split(',')[1], 'base64');
  writeFileSync(new URL(name, out), buf);
  console.log(`${name}  ${(buf.length / 1024).toFixed(0)} KB`);
}

const desktop = { viewport: { width: 1440, height: 1000 }, deviceScaleFactor: 1.5 };

// 1. Today, light: budget gauge, macro pyramid, micronutrients, week, weight.
{
  const ctx = await context(desktop, 'light');
  const page = await open(ctx, '#/today');
  await page.getByTestId('week-card').waitFor();
  await save('today-light.webp', await page.screenshot());
  await ctx.close();
}

// 2. Today, dark, scrolled to meal timing, the month calendar and today's meals.
{
  const ctx = await context(desktop, 'dark');
  const page = await open(ctx, '#/today');
  await page.getByTestId('meal-timing').scrollIntoViewIfNeeded();
  await page.evaluate(() => {
    const el = document.querySelector('[data-testid="meal-timing"]');
    window.scrollTo(0, el.getBoundingClientRect().top + window.scrollY - 24);
  });
  await page.waitForTimeout(300);
  await save('today-dark.webp', await page.screenshot());
  await ctx.close();
}

// 3. The day log with every nutrient.
{
  const ctx = await context(desktop, 'light');
  const page = await open(ctx, '#/log');
  await page.getByTestId('toggle-nutrients').click();
  await page.waitForTimeout(300);
  await save('log-light.webp', await page.screenshot());
  await ctx.close();
}

// 4. Trends, dark.
{
  const ctx = await context(desktop, 'dark');
  const page = await open(ctx, '#/trends');
  await page.getByTestId('trend-stats').waitFor();
  await page.waitForTimeout(300);
  await save('trends-dark.webp', await page.screenshot());
  await ctx.close();
}

// 5. A shared recipe.
{
  const ctx = await context(desktop, 'light');
  const page = await open(ctx, '#/recipes');
  await page.getByTestId('food-row').filter({ hasText: 'Morning oats' }).click();
  await page.waitForTimeout(500);
  await save('recipe-light.webp', await page.screenshot());
  await ctx.close();
}

// 6. Phones: dashboard, logging a portion, weight.
const phone = { ...devices['Pixel 7'], deviceScaleFactor: 2 };
const phoneShots = [];
{
  const ctx = await context(phone, 'light');
  const page = await open(ctx, '#/today');
  phoneShots.push(await page.screenshot());
  await ctx.close();
}
{
  const ctx = await context(phone, 'dark');
  const page = await open(ctx, '#/today');
  await page.getByTestId('fab').click();
  const dlg = page.getByTestId('add-food-dialog');
  await dlg.getByTestId('food-search').fill('greek yogurt');
  await dlg.getByTestId('food-result').first().click();
  await dlg.getByTestId('portion-preview').waitFor();
  await page.waitForTimeout(400);
  phoneShots.push(await page.screenshot());
  await ctx.close();
}
{
  const ctx = await context(phone, 'light');
  const page = await open(ctx, '#/weight');
  await page.waitForTimeout(500);
  phoneShots.push(await page.screenshot());
  await ctx.close();
}
{
  const page = await browser.newPage({ viewport: { width: 1500, height: 1080 }, deviceScaleFactor: 1.5 });
  const imgs = phoneShots.map((b) => `<img src="data:image/png;base64,${b.toString('base64')}">`).join('');
  await page.setContent(`<!doctype html><style>
    body { margin: 0; background: transparent; }
    .row { display: flex; gap: 48px; justify-content: center; padding: 40px; }
    img { width: 412px; border-radius: 28px; box-shadow: 0 18px 50px rgba(40, 20, 50, 0.22), 0 0 0 1px rgba(40, 20, 50, 0.08); }
  </style><div class="row">${imgs}</div>`);
  await page.waitForTimeout(200);
  await save('phones.webp', await page.locator('.row').screenshot({ omitBackground: true }), 0.92);
  await page.close();
}

await browser.close();
server.kill();
