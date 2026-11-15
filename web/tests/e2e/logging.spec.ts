import { asUser, dayOffset, expect, openAdd, pantry, test, today } from '../fixtures';

test.describe('logging food', () => {
  test('search the library, pick a portion, see totals and budget update', async ({ page, api, consoleErrors }) => {
    const u = await api.user('Jordan');
    await api.goal(u.id, { effective_from: dayOffset(-30), calories: 2000, protein: 150 });
    await pantry(api);
    await asUser(page, u.id);
    await page.goto('/');

    await page.getByTestId('add-breakfast').click();
    const dlg = page.getByTestId('add-food-dialog');
    await expect(dlg.getByRole('heading')).toContainText('Add to Breakfast');
    await dlg.getByTestId('food-search').fill('costco milk');
    await expect(dlg.getByTestId('food-result')).toHaveCount(1);
    await dlg.getByTestId('food-result').click();

    // Default serving is the cup; preview is live.
    await expect(dlg.getByTestId('unit')).toHaveValue(/^s:/);
    await expect(dlg.getByTestId('portion-preview')).toContainText('149');
    await dlg.getByTestId('qty').fill('2');
    await expect(dlg.getByTestId('portion-preview')).toContainText('298');
    await expect(dlg.getByTestId('portion-preview')).toContainText('16'); // 15.98 g protein
    await dlg.getByTestId('add-entry').click();
    await expect(dlg.getByTestId('added-banner')).toContainText('Whole milk (298 kcal)');

    // Add a second item in grams without leaving the dialog.
    await dlg.getByTestId('food-search').fill('oats');
    await dlg.getByTestId('food-result').first().click();
    await dlg.getByTestId('unit').selectOption({ label: 'g' });
    await dlg.getByTestId('qty').fill('50');
    await expect(dlg.getByTestId('portion-preview')).toContainText('190');
    await dlg.getByTestId('add-entry').click();
    await dlg.getByTestId('add-done').click();
    await expect(dlg).toBeHidden();

    const breakfast = page.getByTestId('meal-breakfast');
    await expect(breakfast.getByTestId('entry')).toHaveCount(2);
    await expect(breakfast).toContainText('2 cup (480 ml)');
    await expect(breakfast).toContainText('50 g');
    await expect(page.getByTestId('calories-eaten')).toHaveText('487');
    await expect(page.getByTestId('calories-remaining')).toHaveText('1,513');
    await expect(page.getByTestId('protein-eaten')).toHaveText('23');
    await expect(page.getByTestId('legend-protein')).toContainText('23 / 150 g');

    // Edit: 1 cup instead of 2.
    await breakfast.getByTestId('entry').filter({ hasText: 'Whole milk' }).click();
    const ed = page.getByTestId('entry-dialog');
    await expect(ed.getByTestId('qty')).toHaveValue('2');
    await ed.getByTestId('qty').fill('1');
    await ed.getByTestId('entry-meal').selectOption('snacks');
    await ed.getByTestId('save-entry').click();
    await expect(page.getByTestId('meal-snacks')).toContainText('1 cup (240 ml)');
    await expect(page.getByTestId('calories-eaten')).toHaveText('338');

    // Switch to ml.
    await page.getByTestId('meal-snacks').getByTestId('entry').click();
    await ed.getByTestId('unit').selectOption({ label: 'ml' });
    await ed.getByTestId('qty').fill('100');
    await ed.getByTestId('save-entry').click();
    await expect(page.getByTestId('meal-snacks')).toContainText('100 ml');
    await expect(page.getByTestId('calories-eaten')).toHaveText('252');

    // Delete.
    await page.getByTestId('meal-snacks').getByTestId('entry').click();
    await ed.getByTestId('delete-entry').click();
    await expect(page.getByTestId('meal-snacks').getByTestId('entry')).toHaveCount(0);
    await expect(page.getByTestId('calories-eaten')).toHaveText('190');

    const d = await api.diary(u.id, today());
    expect(d.entries).toHaveLength(1);
    expect(consoleErrors).toEqual([]);
  });

  test('recent foods re-log with the last portion', async ({ page, api }) => {
    const u = await api.user('Alex');
    const { banana } = await pantry(api);
    await api.entry(u.id, { date: dayOffset(-1), meal: 'snacks', food_id: banana.id, quantity: 1.5, unit: 'medium' });
    await asUser(page, u.id);
    await page.goto('/');
    await openAdd(page);
    const dlg = page.getByTestId('add-food-dialog');
    await dlg.getByTestId('recent-food').filter({ hasText: 'Banana' }).click();
    await expect(dlg.getByTestId('qty')).toHaveValue('1.5');
    await expect(dlg.getByTestId('unit').locator('option:checked')).toHaveText(/medium/);
    await dlg.getByTestId('meal-select').selectOption('lunch');
    await dlg.getByTestId('add-entry').click();
    await dlg.getByTestId('add-done').click();
    await expect(page.getByTestId('meal-lunch')).toContainText('1.5 medium (177 g)');
  });

  test('quick add without a library food', async ({ page, api }) => {
    const u = await api.user('Alex');
    await asUser(page, u.id);
    await page.goto('/');
    await page.getByTestId('add-dinner').click();
    const dlg = page.getByTestId('add-food-dialog');
    await dlg.getByTestId('tab-quick').click();
    await expect(dlg.getByTestId('quick-add')).toBeDisabled();
    await dlg.getByTestId('quick-name').fill('Pizza night');
    await dlg.getByTestId('quick-calories').fill('900');
    await dlg.getByTestId('quick-protein').fill('38');
    await dlg.getByTestId('quick-add').click();
    await expect(dlg.getByTestId('added-banner')).toContainText('Pizza night (900 kcal)');
    await dlg.getByTestId('add-done').click();
    await expect(page.getByTestId('meal-dinner')).toContainText('Pizza night');
    await expect(page.getByTestId('calories-eaten')).toHaveText('900');

    // Quick-add entries are edited by their numbers.
    await page.getByTestId('meal-dinner').getByTestId('entry').click();
    const ed = page.getByTestId('entry-dialog');
    await ed.getByLabel('Calories (kcal) per 1').fill('800');
    await ed.getByTestId('save-entry').click();
    await expect(page.getByTestId('calories-eaten')).toHaveText('800');
  });

  test('copy yesterday and copy from the other person', async ({ page, api }) => {
    const a = await api.user('Jordan');
    const b = await api.user('Riley');
    const { oats, milk } = await pantry(api);
    await api.entry(a.id, { date: dayOffset(-1), meal: 'breakfast', food_id: oats.id, quantity: 50, unit: 'g' });
    await api.entry(a.id, { date: dayOffset(-1), meal: 'dinner', food_id: milk.id, quantity: 1 });
    await api.entry(b.id, { date: today(), meal: 'dinner', name: 'Shared curry', nutrients: { calories: 650, protein: 30 } });
    await asUser(page, a.id);
    await page.goto('/#/log');

    await page.getByTestId('copy-open').click();
    const dlg = page.getByTestId('copy-dialog');
    await dlg.getByTestId('copy-confirm').click();
    await expect(page.getByTestId('toast').filter({ hasText: 'Copied 2 items' })).toBeVisible();
    await expect(page.getByTestId('meal-breakfast')).toContainText('Rolled oats');

    await page.getByRole('button', { name: 'Copy into Dinner' }).click();
    await dlg.getByRole('button', { name: 'Riley today' }).click();
    await dlg.getByTestId('copy-confirm').click();
    await expect(page.getByTestId('meal-dinner')).toContainText('Shared curry');
    await expect(page.getByTestId('meal-dinner').getByTestId('entry')).toHaveCount(2);
  });

  test('exercise adds to the budget only when the goal says so', async ({ page, api }) => {
    const u = await api.user('Alex');
    await api.goal(u.id, { effective_from: dayOffset(-3), calories: 2000 });
    await asUser(page, u.id);
    await page.goto('/#/log');
    await page.getByTestId('add-exercise').click();
    const dlg = page.getByTestId('exercise-dialog');
    await dlg.getByLabel('Activity').fill('Run');
    await dlg.getByLabel('Calories burned').fill('300');
    await dlg.getByRole('button', { name: 'Add' }).click();
    await expect(page.getByTestId('exercise-card')).toContainText('Run');
    await expect(page.getByTestId('calories-remaining')).toHaveText('2,000');
    await expect(page.getByTestId('day-budget')).toContainText('(not added)');

    await api.goal(u.id, { effective_from: dayOffset(-3), exercise_adds_to_budget: true });
    await page.reload();
    await expect(page.getByTestId('calories-remaining')).toHaveText('2,300');
  });

  test('day navigation, history and the weekly bank', async ({ page, api }) => {
    const u = await api.user('Alex');
    await api.goal(u.id, { effective_from: dayOffset(-40), calories: 2000 });
    await api.entry(u.id, { date: dayOffset(-1), name: 'Yesterday food', nutrients: { calories: 1500 } });
    await asUser(page, u.id);
    await page.goto('/#/log');
    await expect(page.getByTestId('day-label')).toHaveText('Today');
    await page.getByTestId('prev-day').click();
    await expect(page.getByTestId('day-label')).toHaveText('Yesterday');
    await expect(page).toHaveURL(new RegExp(`log\\?date=${dayOffset(-1)}`));
    await expect(page.getByTestId('calories-remaining')).toHaveText('500');
    await page.getByTestId('next-day').click();
    await expect(page.getByTestId('day-label')).toHaveText('Today');
    // Yesterday was 500 under budget: banked if it is in the same week.
    const sameWeek = new Date().getDay() !== 1; // Monday starts a new week
    if (sameWeek) {
      await expect(page.getByTestId('day-week')).toContainText('+500 kcal');
      await page.goto('/#/today');
      await expect(page.getByTestId('week-card')).toContainText('+500 kcal banked');
    }
    // A bad date in the URL falls back to today instead of breaking.
    await page.goto('/#/log?date=2024-02-31');
    await expect(page.getByTestId('day-label')).toHaveText('Today');
  });

  test('nutrients panel shows vitamins with coverage hints', async ({ page, api }) => {
    const u = await api.user('Alex');
    const { banana, oats } = await pantry(api);
    await api.entry(u.id, { date: today(), food_id: banana.id, quantity: 1 });
    await api.entry(u.id, { date: today(), food_id: oats.id, quantity: 40, unit: 'g' });
    await asUser(page, u.id);
    await page.goto('/#/log');
    await page.getByTestId('toggle-nutrients').click();
    const t = page.getByTestId('nutrient-table');
    await expect(t.locator('tr[data-key=vitamin_c]')).toContainText('10.3');
    await expect(t.locator('tr[data-key=vitamin_c]')).toContainText('50% data');
    await expect(t.locator('tr[data-key=potassium]')).toContainText('422');
  });
});

test('BUG-7: an entry whose serving was deleted keeps its unit and can still be edited', async ({ page, api }) => {
  const u = await api.user('Alex');
  const juice = await api.food({
    name: 'Juice',
    basis_unit: 'ml',
    nutrients: { calories: 45 },
    servings: [
      { label: 'cup', amount: 240 },
      { label: 'glass', amount: 300 },
    ],
  });
  await api.entry(u.id, { date: today(), meal: 'snacks', food_id: juice.id, quantity: 1, unit: 'glass' });
  await api.json('PUT', `/foods/${juice.id}`, { name: 'Juice', basis_unit: 'ml', nutrients: { calories: 45 }, servings: [{ label: 'cup', amount: 240 }] });
  await asUser(page, u.id);
  await page.goto('/');
  await expect(page.getByTestId('calories-eaten')).toHaveText('135');
  await page.getByTestId('meal-snacks').getByTestId('entry').click();
  const ed = page.getByTestId('entry-dialog');
  await expect(ed.getByTestId('unit').locator('option:checked')).toHaveText('glass (as logged)');
  await ed.getByTestId('qty').fill('2');
  await expect(ed.getByTestId('portion-preview')).toContainText('270');
  await ed.getByTestId('save-entry').click();
  await expect(page.getByTestId('calories-eaten')).toHaveText('270');
  await expect(page.getByTestId('meal-snacks')).toContainText('2 glass');
});
