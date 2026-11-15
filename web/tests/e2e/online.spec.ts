import { asUser, expect, openAdd, test } from '../fixtures';

test('"costco milk" is not in the library: find it online, import and log it', async ({ page, api, consoleErrors }) => {
  const u = await api.user('Jordan');
  await asUser(page, u.id);
  await page.goto('/');
  await page.getByTestId('add-breakfast').click();
  const dlg = page.getByTestId('add-food-dialog');
  await dlg.getByTestId('food-search').fill('costco milk');
  await expect(dlg.getByText('No library foods match')).toBeVisible();
  await dlg.getByRole('button', { name: 'Search online' }).click();
  await expect(dlg.getByTestId('online-query')).toHaveValue('costco milk');
  await dlg.getByTestId('online-search').click();
  const result = dlg.getByTestId('online-result').filter({ hasText: 'Whole Milk' }).filter({ hasText: 'Kirkland Signature' }).first();
  await expect(result).toBeVisible();
  await expect(result).toContainText('62 kcal');
  await expect(result).toContainText('cup = 240 ml');
  await result.getByTestId('import-result').click();

  // Straight to the portion step with the cup serving.
  await expect(dlg.getByTestId('portion-form')).toContainText('Kirkland Signature');
  await expect(dlg.getByTestId('portion-preview')).toContainText('149');
  await dlg.getByTestId('add-entry').click();
  await dlg.getByTestId('add-done').click();
  await expect(page.getByTestId('meal-breakfast')).toContainText('Whole Milk');
  await expect(page.getByTestId('calories-eaten')).toHaveText('149');

  // Now it is in the shared library and findable by what was typed.
  const hits = await api.json('GET', '/foods?q=costco%20milk');
  expect(hits).toHaveLength(1);
  expect(hits[0].source).toBe('usda');
  expect(hits[0].nutrients.vitamin_d).toBeCloseTo(1.25);

  // Searching again marks it as already in the library.
  await openAdd(page);
  await dlg.getByTestId('tab-online').click();
  await dlg.getByTestId('online-query').fill('kirkland milk');
  await dlg.getByTestId('online-search').click();
  await expect(dlg.getByTestId('online-result').filter({ hasText: 'in library' })).toHaveCount(1);
  expect(consoleErrors).toEqual([]);
});

test('barcode lookup imports from Open Food Facts; unknown codes offer to create the food', async ({ page, api }) => {
  const u = await api.user('Alex');
  await asUser(page, u.id);
  await page.goto('/');
  await openAdd(page);
  const dlg = page.getByTestId('add-food-dialog');
  await dlg.getByTestId('tab-barcode').click();
  await dlg.getByTestId('barcode-input').fill('3017624010701');
  await dlg.getByTestId('barcode-lookup').click();
  await expect(dlg.getByText('Nutella')).toBeVisible();
  await dlg.getByTestId('barcode-import').click();
  await dlg.getByTestId('qty').fill('15');
  await expect(dlg.getByTestId('portion-preview')).toContainText('81');
  await dlg.getByTestId('add-entry').click();

  await dlg.getByTestId('tab-barcode').click();
  await dlg.getByTestId('barcode-input').fill('000111222333');
  await dlg.getByTestId('barcode-lookup').click();
  await expect(dlg.getByText('No product found for 000111222333')).toBeVisible();
  await dlg.getByRole('button', { name: 'Create it from the label' }).click();
  await expect(dlg.getByTestId('food-editor')).toBeVisible();
});

test('rate limits from USDA show a helpful message', async ({ page, api }) => {
  const u = await api.user('Alex');
  await asUser(page, u.id);
  await page.goto('/#/foods/import');
  await page.getByTestId('online-query').fill('ratelimit test');
  await page.getByRole('combobox', { name: 'Source' }).selectOption('usda');
  await page.getByTestId('online-search').click();
  await expect(page.getByRole('alert')).toContainText('rate limit');
  await expect(page.getByRole('alert')).toContainText('Settings');
});
