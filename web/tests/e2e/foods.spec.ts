import { asUser, dayOffset, expect, pantry, test, today } from '../fixtures';

test('create a food with servings and vitamins, then log it by serving', async ({ page, api, consoleErrors }) => {
  const u = await api.user('Alex');
  await asUser(page, u.id);
  await page.goto('/#/foods');
  await page.getByTestId('new-food').click();
  const ed = page.getByTestId('food-editor');
  await ed.getByTestId('food-name').fill('Protein bar');
  await ed.getByTestId('food-brand').fill('Kirkland Signature');
  await ed.getByTestId('basis-amount').fill('1');
  await ed.getByTestId('basis-unit').selectOption('serving');
  await ed.getByTestId('n-calories').fill('190');
  await ed.getByTestId('n-protein').fill('21');
  await ed.getByTestId('n-carbs').fill('22');
  await ed.getByTestId('n-fat').fill('7');
  await ed.getByTestId('n-fiber').fill('15');
  await ed.getByText('More nutrients: fats, vitamins & minerals').click();
  await ed.getByTestId('n-calcium').fill('130');
  await ed.getByTestId('n-iron').fill('1.8');
  await ed.getByTestId('add-serving').click();
  const row = ed.getByTestId('serving-row');
  await row.getByLabel('Serving label').fill('bar');
  await row.getByLabel('Serving amount').fill('1');
  await ed.getByText('Barcode, search words & notes').click();
  await ed.getByTestId('food-aliases').fill('costco protein bar');
  await ed.getByTestId('save-food').click();
  await expect(page.getByTestId('toast').filter({ hasText: 'Saved Protein bar' })).toBeVisible();

  // Findable via the alias and the costco->kirkland synonym.
  await page.goto('/#/foods');
  await page.getByTestId('foods-search').fill('costco bar');
  await expect(page.getByTestId('food-row')).toHaveCount(1);
  await expect(page.getByTestId('food-row')).toContainText('190 kcal · 21 g protein per bar');

  await page.goto('/');
  await page.getByTestId('add-snacks').click();
  const dlg = page.getByTestId('add-food-dialog');
  await dlg.getByTestId('food-search').fill('protein bar');
  await dlg.getByTestId('food-result').click();
  await dlg.getByRole('button', { name: '2', exact: true }).click();
  await dlg.getByTestId('add-entry').click();
  await dlg.getByTestId('add-done').click();
  await expect(page.getByTestId('meal-snacks')).toContainText('2 bar');
  await expect(page.getByTestId('calories-eaten')).toHaveText('380');
  expect(consoleErrors).toEqual([]);
});

test('editor validates input and warns about inconsistent labels', async ({ page, api }) => {
  const u = await api.user('Alex');
  await asUser(page, u.id);
  await page.goto('/#/foods/new');
  const ed = page.getByTestId('food-editor');
  await ed.getByTestId('food-name').fill('Mystery');
  await ed.getByTestId('n-calories').fill('500');
  await ed.getByTestId('n-protein').fill('1');
  await expect(ed.getByText('Calories don’t match the macros')).toBeVisible();
  await ed.getByTestId('n-protein').fill('80');
  await ed.getByTestId('n-carbs').fill('80');
  await ed.getByTestId('save-food').click();
  await expect(ed.getByRole('alert')).toContainText('impossible');
  await ed.getByTestId('n-carbs').fill('-3');
  await ed.getByTestId('save-food').click();
  await expect(ed.getByRole('alert')).toContainText('non-negative');
});

test('editing a food updates past diary entries and recipes', async ({ page, api }) => {
  const u = await api.user('Alex');
  const { milk } = await pantry(api);
  await api.entry(u.id, { date: dayOffset(-2), food_id: milk.id, quantity: 1 });
  await asUser(page, u.id);
  await page.goto(`/#/foods/${milk.id}`);
  const ed = page.getByTestId('food-editor');
  await ed.getByTestId('n-calories').fill('42');
  await ed.getByTestId('save-food').click();
  await expect(page.getByTestId('toast').filter({ hasText: 'Updated 1 past diary entries' })).toBeVisible();
  const d = await api.diary(u.id, dayOffset(-2));
  expect(d.totals.calories).toBeCloseTo(100.8, 1);
});

test('deleting a logged food archives it and it can be restored', async ({ page, api }) => {
  const u = await api.user('Alex');
  const { banana } = await pantry(api);
  await api.entry(u.id, { date: today(), meal: 'snacks', food_id: banana.id, quantity: 1 });
  await asUser(page, u.id);
  page.on('dialog', (d) => d.accept());
  await page.goto(`/#/foods/${banana.id}`);
  await page.getByTestId('food-editor').getByRole('button', { name: 'Delete' }).click();
  await expect(page.getByTestId('toast').filter({ hasText: 'Archived Banana' })).toBeVisible();
  await page.getByTestId('foods-search').fill('banana');
  await expect(page.getByTestId('food-row')).toHaveCount(0);
  // History keeps the entry.
  await page.goto('/');
  await expect(page.getByTestId('meal-snacks')).toContainText('Banana');
  await page.goto('/#/foods');
  await page.getByLabel('Archived').check();
  await page.getByTestId('food-row').filter({ hasText: 'Banana' }).click();
  await page.getByRole('button', { name: 'Restore' }).click();
  await page.goto('/#/foods');
  await page.getByTestId('foods-search').fill('banana');
  await expect(page.getByTestId('food-row')).toHaveCount(1);
});
