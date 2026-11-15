import { asUser, dayOffset, expect, test, today } from '../fixtures';

test('log, replace, edit and delete weigh-ins; trend and stats update', async ({ page, api, consoleErrors }) => {
  const u = await api.user('Alex', { goal_weight_kg: 75 });
  for (let i = 30; i >= 1; i -= 3) await api.weight(u.id, { date: dayOffset(-i), weight: 80 + i * 0.05, unit: 'kg' });
  await asUser(page, u.id);
  await page.goto('/#/weight');
  await expect(page.getByTestId('chart-weight-history').locator('circle')).toHaveCount(10);

  await page.getByTestId('weight-input').fill('79.6');
  await page.getByTestId('weight-save').click();
  await expect(page.getByTestId('toast').filter({ hasText: 'Logged 79.6 kg for today' })).toBeVisible();
  await expect(page.getByTestId('weight-table').locator('tbody tr')).toHaveCount(11);
  await expect(page.getByTestId('weight-table').locator('tbody tr').first()).toContainText('79.6 kg');

  // Same day again replaces it.
  await page.getByTestId('weight-input').fill('79.4');
  await page.getByTestId('weight-save').click();
  await expect(page.getByTestId('weight-table').locator('tbody tr')).toHaveCount(11);
  await expect(page.getByTestId('weight-table').locator('tbody tr').first()).toContainText('79.4 kg');
  await expect(page.getByTestId('weight-stats')).toContainText('75.0');
  await expect(page.getByTestId('weight-stats')).toContainText('/wk');

  // Edit then delete.
  await page.getByTestId('weight-table').locator('tbody tr').first().getByRole('button', { name: 'Edit' }).click();
  const dlg = page.getByTestId('weight-edit-dialog');
  await dlg.getByLabel('Weight (kg)').fill('79.9');
  await dlg.getByRole('button', { name: 'Save' }).click();
  await expect(page.getByTestId('weight-table').locator('tbody tr').first()).toContainText('79.9 kg');
  await page.getByTestId('weight-table').locator('tbody tr').first().getByRole('button', { name: 'Edit' }).click();
  await dlg.getByRole('button', { name: 'Delete' }).click();
  await expect(page.getByTestId('weight-table').locator('tbody tr')).toHaveCount(10);

  // Out-of-range values are rejected by the server with a readable message.
  await page.getByTestId('weight-input').fill('5');
  await page.getByTestId('weight-save').click();
  await expect(page.getByTestId('toast').filter({ hasText: 'plausible range' })).toBeVisible();
  expect(consoleErrors).toEqual([]);
});

test('pound users enter and see pounds; storage is kg', async ({ page, api }) => {
  const u = await api.user('Sam', { weight_unit: 'lb' });
  await asUser(page, u.id);
  await page.goto('/');
  await page.getByTestId('log-weight').click();
  await page.getByTestId('quick-weight').fill('150');
  await page.getByTestId('quick-weight-save').click();
  await expect(page.getByTestId('today-weight')).toHaveText('150.0 lb');
  const ws = await api.json('GET', `/users/${u.id}/weights`);
  expect(ws[0].weight_kg).toBeCloseTo(68.039, 2);
  await page.goto('/#/weight');
  await expect(page.getByTestId('trend-weight')).toContainText('150.0');

  // Switching units in the profile converts the display.
  await page.goto('/#/settings');
  await page.getByTestId('profile-unit').selectOption('kg');
  await page.getByTestId('save-profile').click();
  await expect(page.getByTestId('toast').filter({ hasText: 'Profile saved' })).toBeVisible();
  await page.goto('/#/weight');
  await expect(page.getByTestId('trend-weight')).toContainText('68.0');
  expect(today()).toBeTruthy();
});
