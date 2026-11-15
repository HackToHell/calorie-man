import { expect, test } from '../fixtures';

test('first run: create a person, set goals, see the budget, add a second person and switch', async ({ page, consoleErrors }) => {
  await page.goto('/');
  const form = page.getByTestId('onboarding');
  await expect(form).toBeVisible();
  await form.getByLabel('Your name').fill('Jordan');
  await form.getByRole('button', { name: 'Get started' }).click();

  // Lands on settings with a setup hint.
  await expect(page.getByText('Welcome, Jordan!')).toBeVisible();
  await page.getByTestId('goal-calories').fill('2100');
  await page.getByTestId('goal-protein').fill('150');
  await page.getByRole('button', { name: '30% fat' }).click();
  await expect(page.getByTestId('goal-fat')).toHaveValue('70');
  await expect(page.getByTestId('goal-carbs')).toHaveValue('218');
  await page.getByTestId('save-goals').click();
  await expect(page.getByTestId('toast').filter({ hasText: 'Goals saved' })).toBeVisible();

  await page.getByRole('link', { name: 'Today' }).first().click();
  await expect(page.getByTestId('calories-remaining')).toHaveText('2,100');
  await expect(page.getByTestId('legend-protein')).toContainText('0 / 150 g');

  // Second person through the switcher.
  await page.getByTestId('user-switcher').click();
  await page.getByRole('menuitem', { name: '+ Add person' }).click();
  await page.getByTestId('add-user-dialog').getByLabel('Name').fill('Riley');
  await page.getByTestId('add-user-dialog').getByRole('button', { name: 'Add' }).click();
  await expect(page.getByTestId('user-switcher')).toContainText('Riley');
  // Riley has no goals yet.
  await expect(page.getByTestId('budget-card')).toContainText('No calorie goal yet');

  await page.getByTestId('user-switcher').click();
  await page.getByTestId('user-option').filter({ hasText: 'Jordan' }).click();
  await expect(page.getByTestId('calories-remaining')).toHaveText('2,100');

  // The choice survives a reload (stored per device).
  await page.reload();
  await expect(page.getByTestId('user-switcher')).toContainText('Jordan');
  expect(consoleErrors).toEqual([]);
});

test('profile validation errors are shown and duplicates rejected', async ({ page, api }) => {
  await api.user('Alex');
  await api.user('Sam');
  await page.goto('/#/settings');
  await page.getByTestId('profile-name').fill('sam');
  await page.getByTestId('save-profile').click();
  await expect(page.getByTestId('toast').filter({ hasText: "already exists" })).toBeVisible();
});

test('deleting a person requires typing their name and keeps shared foods', async ({ page, api }) => {
  const a = await api.user('Alex');
  await api.user('Sam');
  await api.food({ name: 'Shared bread', nutrients: { calories: 250 } });
  await page.addInitScript((id) => localStorage.setItem('calorie-man.user', String(id)), a.id);
  await page.goto('/#/settings');
  await page.getByRole('button', { name: 'Delete Alex…' }).click();
  const dlg = page.getByTestId('delete-user-dialog');
  const del = dlg.getByRole('button', { name: 'Delete', exact: true });
  await expect(del).toBeDisabled();
  await dlg.getByRole('textbox').fill('alex');
  await del.click();
  await expect(page.getByTestId('user-switcher')).toContainText('Sam');
  await page.goto('/#/foods');
  await expect(page.getByTestId('food-row')).toContainText('Shared bread');
});

test('devices keep their chosen person and theme from before the rename to Calorie Man', async ({ page, api }) => {
  await api.user('Jordan');
  const priya = await api.user('Riley');
  // A phone that used the app before stored its preferences under the old "eat.*" keys.
  await page.addInitScript((id) => {
    if (!sessionStorage.getItem('seeded')) {
      localStorage.setItem('eat.user', String(id));
      localStorage.setItem('eat.theme', 'dark');
      sessionStorage.setItem('seeded', '1');
    }
  }, priya.id);
  await page.goto('/');
  await expect(page.getByTestId('user-switcher')).toContainText('Riley');
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  const keys = await page.evaluate(() => Object.keys(localStorage).sort());
  expect(keys).toEqual(['calorie-man.theme', 'calorie-man.user']);
  await page.reload();
  await expect(page.getByTestId('user-switcher')).toContainText('Riley');
});
