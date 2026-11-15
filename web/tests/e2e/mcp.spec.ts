import { asUser, expect, test, today } from '../fixtures';

test('an AI client creates a food over MCP and the UI shows the logged entry', async ({ page, api, request }) => {
  const me = await api.user('Jordan');
  await api.user('Riley');

  const init = await request.post('/mcp', {
    data: { jsonrpc: '2.0', id: 1, method: 'initialize', params: { protocolVersion: '2025-06-18', capabilities: {}, clientInfo: { name: 'e2e', version: '1' } } },
  });
  expect((await init.json()).result.serverInfo.name).toBe('calorie-man');
  const list = await request.post('/mcp', { data: { jsonrpc: '2.0', id: 2, method: 'tools/list' } });
  expect((await list.json()).result.tools.length).toBeGreaterThan(15);

  const found = await api.mcp('search_foods', { query: 'costco rotisserie chicken' });
  expect(found.count).toBe(0);
  const created = await api.mcp('create_food', {
    name: 'Rotisserie chicken',
    brand: 'Kirkland Signature',
    basis_amount: 3,
    basis_unit: 'oz',
    nutrients: { calories: 140, protein: 19, fat: 7, carbs: 0, sodium: 460 },
    servings: [{ label: 'oz', amount: 1, unit: 'oz' }],
    aliases: 'costco rotisserie chicken',
    notes: 'From the Costco label',
  });
  const logged = await api.mcp(
    'log_food',
    { food_id: created.food.id, quantity: 6, unit: 'oz', meal: 'dinner' },
    `?user=${encodeURIComponent('Jordan')}`,
  );
  expect(logged.logged.nutrients.calories).toBe(280);
  expect(logged.day.date).toBe(today());

  await asUser(page, me.id);
  await page.goto('/');
  await expect(page.getByTestId('meal-dinner')).toContainText('Rotisserie chicken');
  await expect(page.getByTestId('meal-dinner')).toContainText('6 oz');
  await expect(page.getByTestId('calories-eaten')).toHaveText('280');

  // The food is shared: it shows in the library with its source.
  await page.goto('/#/foods');
  await expect(page.getByTestId('food-row').filter({ hasText: 'Rotisserie chicken' })).toContainText('AI');

  // Settings shows a ready-to-copy MCP URL for the current user.
  await page.goto('/#/settings');
  await expect(page.getByTestId('mcp-url')).toHaveValue(/\/mcp\?user=Jordan$/);
});
