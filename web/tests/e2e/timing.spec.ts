import { asUser, dayOffset, expect, pantry, test, today } from '../fixtures';

/** The browser thinks it is 4:20 PM today (the server keeps its real clock). */
async function at420(page: import('@playwright/test').Page) {
  const d = new Date();
  await page.clock.setFixedTime(new Date(d.getFullYear(), d.getMonth(), d.getDate(), 16, 20));
}

test('meal timing: diamonds by time, eating window, usual dinner time, and logging with a time', async ({ page, api, consoleErrors }) => {
  const u = await api.user('Jordan');
  const { oats, milk, banana } = await pantry(api);
  // Dinner around 7:30 PM on three earlier days teaches the usual time.
  for (const [d, t] of [
    [-3, '19:20'],
    [-2, '19:30'],
    [-1, '19:40'],
  ] as const) {
    await api.entry(u.id, { date: dayOffset(d), meal: 'dinner', food_id: milk.id, quantity: 1, time: t });
  }
  await api.entry(u.id, { date: today(), meal: 'breakfast', food_id: oats.id, quantity: 1, time: '07:40' });
  await api.entry(u.id, { date: today(), meal: 'breakfast', food_id: milk.id, quantity: 1, time: '07:45' });
  await api.entry(u.id, { date: today(), meal: 'lunch', food_id: banana.id, quantity: 2, time: '12:30' });
  await asUser(page, u.id);
  await at420(page);
  await page.goto('/');

  const timing = page.getByTestId('meal-timing');
  await expect(timing.getByTestId('timing-mark')).toHaveCount(2, { timeout: 5000 }); // breakfast is one sitting
  await expect(timing.getByTestId('eating-window')).toHaveText('Eating window 7:40 AM – now · 8 h 40 m');
  await expect(timing.getByTestId('usual-next')).toHaveText('Dinner · usually 7:30');
  await expect(timing).toContainText('300'); // 151.6 + 148.8 kcal at breakfast
  await expect(timing).toContainText('Breakfast · 7:40');
  await expect(page.getByTestId('meal-breakfast').getByTestId('entry-time').first()).toHaveText('7:40 AM');

  // Logging dinner now: the time defaults to now for the current meal and can be changed.
  await page.getByTestId('add-dinner').click();
  const dlg = page.getByTestId('add-food-dialog');
  await dlg.getByTestId('food-search').fill('milk');
  await dlg.getByTestId('food-result').click();
  await expect(dlg.getByTestId('entry-time-input')).toHaveValue('16:20');
  await dlg.getByTestId('entry-time-input').fill('19:05');
  await dlg.getByTestId('add-entry').click();
  await dlg.getByTestId('add-done').click();
  await expect(page.getByTestId('meal-dinner').getByTestId('entry-time')).toHaveText('7:05 PM');
  await expect(timing.getByTestId('timing-mark')).toHaveCount(3);
  await expect(timing.getByTestId('usual-next')).toHaveCount(0); // dinner is logged, nothing to predict

  // Clearing a time takes it off the chart and says so.
  await page.getByTestId('meal-dinner').getByTestId('entry').click();
  const ed = page.getByTestId('entry-dialog');
  await expect(ed.getByTestId('entry-time-edit')).toHaveValue('19:05');
  await ed.getByTestId('entry-time-edit').fill('');
  await ed.getByTestId('save-entry').click();
  await expect(timing.getByTestId('timing-mark')).toHaveCount(2);
  await expect(timing).toContainText('1 item has no time');
  const d = await api.diary(u.id, today());
  expect(d.timing.usual.dinner).toBe('19:30');
  expect(consoleErrors).toEqual([]);
});

test('logging an earlier meal suggests its usual time instead of now', async ({ page, api }) => {
  const u = await api.user('Alex');
  const { oats } = await pantry(api);
  for (const [d, t] of [
    [-3, '07:30'],
    [-2, '07:40'],
    [-1, '07:50'],
  ] as const) {
    await api.entry(u.id, { date: dayOffset(d), meal: 'breakfast', food_id: oats.id, quantity: 1, time: t });
  }
  await asUser(page, u.id);
  await at420(page);
  await page.goto('/');
  await page.getByTestId('add-breakfast').click();
  const dlg = page.getByTestId('add-food-dialog');
  await dlg.getByTestId('food-search').fill('oats');
  await dlg.getByTestId('food-result').click();
  await expect(dlg.getByTestId('entry-time-input')).toHaveValue('07:40');
  // Switching the meal to the current one moves the default to now.
  await dlg.getByTestId('meal-select').selectOption('dinner');
  await expect(dlg.getByTestId('entry-time-input')).toHaveValue('16:20');
});

test('the week chart and calendar start on Monday', async ({ page, api }) => {
  const u = await api.user('Alex');
  await asUser(page, u.id);
  await page.goto('/');
  await expect(page.getByTestId('week-bars').locator('.names span').first()).toHaveText('Mon');
  await expect(page.getByTestId('week-bars').locator('.names span').last()).toHaveText('Sun');
  await expect(page.getByTestId('month-calendar').locator('.dow').first()).toHaveText('M');
  const d = await api.diary(u.id, today());
  expect(new Date(`${d.week.start}T12:00`).getDay()).toBe(1);
});

test('an overdue meal keeps its usual-time marker for a while', async ({ page, api }) => {
  const u = await api.user('Alex');
  const { milk } = await pantry(api);
  for (const d of [-3, -2, -1]) await api.entry(u.id, { date: dayOffset(d), meal: 'dinner', food_id: milk.id, quantity: 1, time: '19:20' });
  await asUser(page, u.id);
  const now = new Date();
  await page.clock.setFixedTime(new Date(now.getFullYear(), now.getMonth(), now.getDate(), 19, 50));
  await page.goto('/');
  await expect(page.getByTestId('meal-timing').getByTestId('usual-next')).toHaveText('Dinner · usually 7:20');
});
