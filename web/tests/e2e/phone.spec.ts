import { asUser, dayOffset, expect, noHorizontalOverflow, openAdd, pantry, test } from '../fixtures';

test('phone: every page fits the screen and logging works with the bottom bar', async ({ page, api, consoleErrors }) => {
  const u = await api.user('Jordan');
  await api.goal(u.id, { calories: 2000, protein: 140 });
  await pantry(api);
  await asUser(page, u.id);
  for (const hash of ['#/today', '#/log', '#/trends', '#/weight', '#/foods', '#/recipes', '#/foods/new', '#/goals', '#/profile']) {
    await page.goto(`/${hash}`);
    await page.waitForLoadState('networkidle');
    await noHorizontalOverflow(page);
  }
  await page.goto('/');
  const tabbar = page.getByRole('navigation', { name: 'Main' }).last();
  await expect(tabbar).toBeVisible();
  await tabbar.getByRole('button', { name: 'Trends' }).click();
  await expect(page.getByRole('heading', { name: 'Trends' })).toBeVisible();
  await tabbar.getByRole('button', { name: 'Today' }).click();

  await openAdd(page);
  const dlg = page.getByTestId('add-food-dialog');
  // Full-screen sheet on phones.
  const box = (await dlg.boundingBox())!;
  expect(box.width).toBeGreaterThan(page.viewportSize()!.width - 2);
  await dlg.getByTestId('food-search').fill('banana');
  await dlg.getByTestId('food-result').click();
  await dlg.getByTestId('add-entry').click();
  await dlg.getByTestId('add-done').click();
  await expect(page.getByTestId('calories-eaten')).toHaveText('105');
  await noHorizontalOverflow(page);
  expect(consoleErrors).toEqual([]);
});

test('phone: Today cards use the full width (BUG-4)', async ({ page, api }) => {
  const u = await api.user('Alex');
  await api.goal(u.id, { calories: 2000, protein: 140 });
  await api.entry(u.id, { meal: 'lunch', name: 'Short', nutrients: { calories: 100 } });
  await asUser(page, u.id);
  await page.goto('/');
  const full = (await page.getByTestId('budget-card').boundingBox())!.width;
  const cards = page.locator('.dash > .card, .dash > .c-next');
  expect(await cards.count()).toBeGreaterThan(6);
  for (const box of await cards.evaluateAll((els) => els.map((e) => [e.className, e.getBoundingClientRect().width] as const))) {
    expect(Math.abs(box[1] - full), `${box[0]} width`).toBeLessThan(2);
  }
});

test('phone: the + button logs food and the next-meal card offers yesterday and the usual dish', async ({ page, api }) => {
  const u = await api.user('Alex');
  const { oats, milk } = await pantry(api);
  const r = await api.food({ name: 'Turkey chili', kind: 'recipe', recipe_yield: 4, ingredients: [{ food_id: oats.id, quantity: 400, unit: 'g' }] });
  // Every meal yesterday, and the chili at each meal on earlier days.
  for (const meal of ['breakfast', 'lunch', 'dinner', 'snacks']) {
    await api.entry(u.id, { date: dayOffset(-1), meal, food_id: milk.id, quantity: 1 });
    for (const d of [-2, -3, -4]) await api.entry(u.id, { date: dayOffset(d), meal, food_id: r.id, quantity: 1 });
  }
  await asUser(page, u.id);
  await page.goto('/');
  const card = page.getByTestId('next-meal');
  await expect(card).toContainText('Nothing logged yet');
  await expect(card.getByTestId('copy-yesterday')).toContainText('Whole milk · 149');
  await expect(card.getByTestId('suggestion')).toContainText('Turkey chili');
  await card.getByTestId('copy-yesterday').click();
  await expect(page.getByTestId('toast').filter({ hasText: 'Copied 1 item' })).toBeVisible();
  await expect(page.getByTestId('calories-eaten')).toHaveText('149');
  // The suggestion opens the portion step with the usual portion.
  await page.getByTestId('suggestion').click();
  const dlg = page.getByTestId('add-food-dialog');
  await expect(dlg.getByTestId('portion-form')).toContainText('Turkey chili');
  await expect(dlg.getByTestId('qty')).toHaveValue('1');
});
