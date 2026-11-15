import { expect, test } from '../fixtures';

test.use({ seedDays: 60 });

test('trends show budget, protein, weight and micronutrients for seeded history', async ({ page, consoleErrors }) => {
  await page.goto('/#/trends');
  const stats = page.getByTestId('trend-stats');
  await expect(stats).toBeVisible();
  await expect(page.getByTestId('avg-calories')).toHaveText(/^\d,\d{3} kcal$/);
  await expect(page.getByTestId('avg-protein')).toHaveText(/^\d{2,3} g\/day$/);
  await expect(page.getByTestId('protein-hit-rate')).toHaveText(/hit target \d+%/);
  await expect(page.getByTestId('weight-change')).toHaveText(/^[+−]?\d+\.\d kg$/);
  await expect(page.getByTestId('tdee')).toHaveText(/^\d,\d{3} kcal$/);

  // 30 days of bars (minus unlogged days), a budget line and a 7-day average.
  const bars = page.getByTestId('chart-calories').locator('rect.bar');
  expect(await bars.count()).toBeGreaterThan(20);
  expect(await bars.count()).toBeLessThanOrEqual(30);
  await expect(page.getByTestId('chart-calories').locator('path')).toHaveCount(2);
  await expect(page.getByTestId('chart-weight').locator('circle').first()).toBeVisible();
  await expect(page.getByTestId('chart-protein').locator('rect.bar').first()).toBeVisible();

  // Hover shows a tooltip.
  const svg = page.getByTestId('chart-calories').locator('svg');
  const box = (await svg.boundingBox())!;
  await page.mouse.move(box.x + box.width - 20, box.y + box.height / 2);
  await expect(page.getByRole('tooltip')).toContainText('kcal');

  await expect(page.getByTestId('micros-card')).toContainText('Vitamin C');
  // No broken numbers anywhere.
  await expect(page.locator('main')).not.toContainText('NaN');
  await expect(page.locator('main')).not.toContainText('undefined');

  await page.getByRole('tab', { name: '90 days' }).click();
  await expect(page).toHaveURL(/range=90/);
  await expect.poll(() => page.getByTestId('chart-calories').locator('rect.bar').count()).toBeGreaterThan(40);
  await page.getByRole('tab', { name: '7 days' }).click();
  await expect.poll(() => page.getByTestId('chart-calories').locator('rect.bar').count()).toBeLessThanOrEqual(7);
  expect(consoleErrors).toEqual([]);
});

test('trends for a brand-new user explain that there is no data yet', async ({ page, api }) => {
  const u = await api.user('Newbie');
  await page.addInitScript((id) => localStorage.setItem('calorie-man.user', String(id)), u.id);
  await page.goto('/#/trends');
  await expect(page.getByText('No completed days logged in this range yet')).toBeVisible();
  await expect(page.locator('main')).not.toContainText('NaN');
});

test('every page of the seeded app renders without console errors', async ({ page, consoleErrors }) => {
  for (const hash of [
    '#/today',
    '#/log',
    '#/trends',
    '#/weight',
    '#/foods',
    '#/recipes',
    '#/foods/1',
    '#/foods/new',
    '#/recipes/new',
    '#/foods/import',
    '#/goals',
    '#/profile',
  ]) {
    await page.goto(`/${hash}`);
    await page.waitForLoadState('networkidle');
    await expect(page.locator('main')).not.toContainText('NaN');
  }
  expect(consoleErrors).toEqual([]);
});
