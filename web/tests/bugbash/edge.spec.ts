// UI edge cases a household will eventually hit.
import { asUser, expect, noHorizontalOverflow, openAdd, test, today } from '../fixtures';

test('long, unicode and hostile names render safely', async ({ page, api }) => {
  const u = await api.user('Ünïcødé 名前 👩‍🍳');
  const f = await api.food({
    name: 'Grandma’s extra-super-long-named slow-cooked lentil & vegetable soup with garam masala and coriander 🍲',
    brand: '<script>alert(1)</script>',
    nutrients: { calories: 120, protein: 7 },
    servings: [{ label: 'bowl (very large, the blue one from the cupboard)', amount: 450 }],
  });
  await api.entry(u.id, { date: today(), meal: 'lunch', food_id: f.id, quantity: 1, note: 'ate with naan 🫓 '.repeat(20) });
  await asUser(page, u.id);
  let dialogs = 0;
  page.on('dialog', (d) => (dialogs++, d.dismiss()));
  for (const hash of ['#/today', '#/log', '#/foods', `#/foods/${f.id}`, '#/profile']) {
    await page.goto(`/${hash}`);
    await page.waitForLoadState('networkidle');
    await noHorizontalOverflow(page);
  }
  expect(dialogs, 'no script injection').toBe(0);
  await page.goto('/');
  await expect(page.locator('body')).toContainText('Ünïcødé');
});

test('huge and tiny quantities', async ({ page, api }) => {
  const u = await api.user('Alex');
  const f = await api.food({ name: 'Salt', nutrients: { sodium: 38758, calories: 0 } });
  await api.entry(u.id, { date: today(), meal: 'snacks', food_id: f.id, quantity: 0.001, unit: 'g' });
  await api.entry(u.id, { date: today(), meal: 'snacks', name: 'Feast', nutrients: { calories: 99999, protein: 9999 } });
  await asUser(page, u.id);
  await page.goto('/');
  await expect(page.getByTestId('calories-eaten')).toHaveText('99,999');
  await noHorizontalOverflow(page);
  await page.goto('/#/log');
  await page.getByTestId('toggle-nutrients').click();
  await noHorizontalOverflow(page);
});

test('rapid user switching never shows the wrong diary', async ({ page, api }) => {
  const a = await api.user('Alex');
  const b = await api.user('Sam');
  await api.entry(a.id, { date: today(), meal: 'lunch', name: 'Alex lunch', nutrients: { calories: 500 } });
  await api.entry(b.id, { date: today(), meal: 'lunch', name: 'Sam lunch', nutrients: { calories: 700 } });
  await asUser(page, a.id);
  await page.goto('/');
  const phone = !(await page.getByTestId('user-switcher').isVisible());
  const switcher = page.getByTestId(phone ? 'user-switcher-compact' : 'user-switcher');
  for (let i = 0; i < 6; i++) {
    await switcher.click();
    await page.getByTestId('user-option').filter({ hasText: i % 2 ? 'Alex' : 'Sam' }).click();
  }
  if (!phone) await expect(page.getByTestId('user-switcher')).toContainText('Alex');
  await expect(page.getByTestId('meal-lunch')).toContainText('Alex lunch');
  await expect(page.getByTestId('meal-lunch')).not.toContainText('Sam lunch');
});

test('double-clicking "Add to diary" logs once', async ({ page, api }) => {
  const u = await api.user('Alex');
  await api.food({ name: 'Apple', nutrients: { calories: 52 }, servings: [{ label: 'medium', amount: 182 }] });
  await asUser(page, u.id);
  await page.goto('/');
  await openAdd(page);
  await page.getByTestId('food-search').fill('apple');
  await page.getByTestId('food-result').click();
  await page.getByTestId('add-entry').dblclick();
  await page.getByTestId('add-done').click();
  const d = await api.diary(u.id, today());
  expect(d.entries).toHaveLength(1);
});

test('escape closes dialogs and the browser back button works', async ({ page, api }) => {
  const u = await api.user('Alex');
  await asUser(page, u.id);
  await page.goto('/#/today');
  await openAdd(page);
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('add-food-dialog')).toBeHidden();
  await page.goto('/#/trends');
  await expect(page.getByRole('heading', { name: 'Trends' })).toBeVisible();
  await page.goBack();
  await expect(page.getByTestId('budget-card')).toBeVisible();
});

test('server restart while the page is open recovers on the next action', async ({ page, api, server }) => {
  const u = await api.user('Alex');
  await asUser(page, u.id);
  await page.goto('/#/log');
  server.proc.kill('SIGSTOP');
  await page.getByTestId('next-day').click();
  await page.waitForTimeout(500);
  server.proc.kill('SIGCONT');
  await page.getByTestId('prev-day').click();
  await expect(page.getByTestId('day-label')).toHaveText('Today');
  await expect(page.getByTestId('day-budget')).toBeVisible();
});
