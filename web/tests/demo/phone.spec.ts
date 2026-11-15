import { expect, test } from '@playwright/test';

test('on a phone the notice is one short line and the app still fits', async ({ page }) => {
  await page.goto('./');
  await expect(page.getByTestId('budget-card')).toBeVisible();
  const banner = page.getByTestId('demo-banner');
  await expect(banner).toContainText('Nothing is saved: reloading starts over.');
  await expect(banner.getByText('Everything runs in your browser')).toBeHidden();
  // One line: the self-host link is left to the Profile page on phones.
  expect((await banner.boundingBox())!.height).toBeLessThanOrEqual(44);
  await expect(banner.getByRole('link')).toBeHidden();
  expect(await page.evaluate(() => document.documentElement.scrollWidth - innerWidth)).toBeLessThanOrEqual(0);
  await page.getByTestId('fab').click();
  await expect(page.getByTestId('add-food-dialog')).toBeVisible();
});
