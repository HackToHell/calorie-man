import { asUser, expect, pantry, test, today } from '../fixtures';

test('build "Morning oats" from 4 ingredients and both people log it', async ({ page, api, consoleErrors }) => {
  const me = await api.user('Jordan');
  const wife = await api.user('Riley');
  await pantry(api);
  await asUser(page, me.id);
  await page.goto('/#/foods');
  await page.getByTestId('new-recipe').click();
  const ed = page.getByTestId('recipe-editor');
  await ed.getByTestId('recipe-name').fill('Morning oats');

  const addIngredient = async (q: string, qty: string, unitLabel?: string) => {
    if (await ed.getByTestId('add-ingredient').isVisible()) await ed.getByTestId('add-ingredient').click();
    await ed.getByTestId('food-search').fill(q);
    await ed.getByTestId('food-result').first().click();
    const row = ed.getByTestId('ingredient-row').last();
    await row.getByTestId('ingredient-qty').fill(qty);
    if (unitLabel) await row.getByRole('combobox').selectOption({ label: unitLabel });
  };
  await addIngredient('rolled oats', '50', 'g');
  await addIngredient('whole milk', '1', 'cup (240 ml)');
  await addIngredient('banana', '1', 'medium (118 g)');
  await addIngredient('peanut', '1', 'tbsp (16 g)');
  await expect(ed.getByTestId('ingredient-row')).toHaveCount(4);
  // 189.5 + 148.8 + 105.02 + 94.08 = 537.4
  await expect(ed.getByTestId('recipe-totals')).toContainText('537 kcal');
  await ed.getByTestId('save-recipe').click();
  await expect(page.getByTestId('toast').filter({ hasText: 'Saved recipe Morning oats' })).toBeVisible();

  // Log for me.
  await page.goto('/');
  await page.getByTestId('add-breakfast').click();
  const dlg = page.getByTestId('add-food-dialog');
  await dlg.getByTestId('food-search').fill('morning');
  await expect(dlg.getByTestId('food-result')).toContainText('recipe');
  await dlg.getByTestId('food-result').click();
  await dlg.getByTestId('add-entry').click();
  await dlg.getByTestId('add-done').click();
  await expect(page.getByTestId('calories-eaten')).toHaveText('537');

  // Switch to my wife; the shared recipe is there for her too.
  await page.getByTestId('user-switcher').click();
  await page.getByTestId('user-option').filter({ hasText: 'Riley' }).click();
  await expect(page.getByTestId('calories-eaten')).toHaveText('0');
  await page.getByTestId('add-breakfast').click();
  await dlg.getByTestId('food-search').fill('oats');
  await dlg.getByTestId('food-result').filter({ hasText: 'Morning oats' }).click();
  await dlg.getByRole('button', { name: '½', exact: true }).click();
  await dlg.getByTestId('add-entry').click();
  await dlg.getByTestId('add-done').click();
  await expect(page.getByTestId('calories-eaten')).toHaveText('269');

  const mine = await api.diary(me.id, today());
  const hers = await api.diary(wife.id, today());
  expect(hers.totals.protein).toBeCloseTo(mine.totals.protein / 2, 2);
  expect(consoleErrors).toEqual([]);
});

test('editing a recipe ingredient updates logged entries', async ({ page, api }) => {
  const u = await api.user('Alex');
  const { oats, milk } = await pantry(api);
  const r = await api.food({
    name: 'Overnight oats',
    kind: 'recipe',
    recipe_yield: 2,
    ingredients: [
      { food_id: oats.id, quantity: 100, unit: 'g' },
      { food_id: milk.id, quantity: 2, unit: 'cup' },
    ],
  });
  await api.entry(u.id, { date: today(), food_id: r.id, quantity: 1 });
  await asUser(page, u.id);
  await page.goto('/');
  // (379 + 297.6) / 2 = 338.3
  await expect(page.getByTestId('calories-eaten')).toHaveText('338');
  await page.goto(`/#/foods/${r.id}`);
  const ed = page.getByTestId('recipe-editor');
  await expect(ed.getByTestId('ingredient-row')).toHaveCount(2);
  await ed.getByTestId('ingredient-row').first().getByTestId('ingredient-qty').fill('50');
  await expect(ed.getByTestId('recipe-totals')).toContainText('244 kcal'); // (189.5 + 297.6) / 2 = 243.55
  await ed.getByTestId('save-recipe').click();
  await page.goto('/');
  await expect(page.getByTestId('calories-eaten')).toHaveText('244');
});

test('a recipe cannot include itself', async ({ page, api }) => {
  const u = await api.user('Alex');
  const { oats } = await pantry(api);
  const r = await api.food({ name: 'Base mix', kind: 'recipe', ingredients: [{ food_id: oats.id, quantity: 50, unit: 'g' }] });
  await asUser(page, u.id);
  await page.goto(`/#/foods/${r.id}`);
  const ed = page.getByTestId('recipe-editor');
  await ed.getByTestId('add-ingredient').click();
  await ed.getByTestId('food-search').fill('base mix');
  // The recipe itself is excluded from the picker.
  await expect(ed.getByTestId('food-result')).toHaveCount(0);
});
