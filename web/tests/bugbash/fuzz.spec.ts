// Throw hostile and odd input at every REST endpoint and MCP tool. Nothing may return a 5xx,
// errors must be JSON with a message, and the server must stay healthy.
import { expect, test } from '../fixtures';

test.use({ seedDays: 14 });

const weird: unknown[] = [
  null,
  -1,
  0,
  1e308,
  -1e-9,
  'NaN',
  'Infinity',
  '',
  ' ',
  '1e999',
  '٣',
  "'; DROP TABLE foods; --",
  '<img src=x onerror=alert(1)>',
  '🍌🍌🍌',
  'x'.repeat(10_000),
  [],
  {},
  true,
  '2024-02-30',
  '9999-99-99',
];

test('REST endpoints reject bad input without server errors', async ({ request }) => {
  const targets: [string, string, (v: unknown) => unknown][] = [
    ['POST', '/api/users', (v) => ({ name: v })],
    ['PUT', '/api/users/1', (v) => ({ height_cm: v })],
    ['PUT', '/api/users/1', (v) => ({ birth_date: v })],
    ['POST', '/api/users/1/goals', (v) => ({ calories: v })],
    ['POST', '/api/users/1/goals', (v) => ({ effective_from: v, protein: 100 })],
    ['POST', '/api/foods', (v) => ({ name: v, nutrients: { calories: 100 } })],
    ['POST', '/api/foods', (v) => ({ name: 'x', nutrients: { calories: v } })],
    ['POST', '/api/foods', (v) => ({ name: 'x', basis_amount: v, nutrients: {} })],
    ['POST', '/api/foods', (v) => ({ name: 'x', nutrients: {}, servings: [{ label: 'a', amount: v }] })],
    ['POST', '/api/foods', (v) => ({ name: 'r', kind: 'recipe', ingredients: [{ food_id: v, quantity: 1 }] })],
    ['POST', '/api/foods', (v) => ({ name: 'r', kind: 'recipe', ingredients: [{ food_id: 1, quantity: v }] })],
    ['POST', '/api/users/1/entries', (v) => ({ food_id: 1, quantity: v })],
    ['POST', '/api/users/1/entries', (v) => ({ food_id: 1, unit: v })],
    ['POST', '/api/users/1/entries', (v) => ({ food_id: 1, date: v })],
    ['POST', '/api/users/1/entries', (v) => ({ name: 'q', nutrients: v })],
    ['PUT', '/api/entries/1', (v) => ({ quantity: v })],
    ['POST', '/api/users/1/weights', (v) => ({ weight: v })],
    ['POST', '/api/users/1/exercises', (v) => ({ name: 'run', calories: v })],
    ['POST', '/api/users/1/copy', (v) => ({ from_date: v, to_date: '2024-01-01' })],
    ['GET', '/api/users/1/summary?from=', (v) => v],
    ['GET', '/api/foods?q=', (v) => v],
    ['GET', '/api/users/1/diary?date=', (v) => v],
    ['GET', '/api/external/barcode/', (v) => v],
    ['PUT', '/api/settings', (v) => ({ usda_api_key: v })],
  ];
  const failures: string[] = [];
  for (const [method, path, body] of targets) {
    for (const v of weird) {
      const isGet = method === 'GET';
      const url = isGet ? `${path}${encodeURIComponent(String(v)).slice(0, 2000)}` : path;
      const r = await request.fetch(url, { method, data: isGet ? undefined : body(v), failOnStatusCode: false });
      const status = r.status();
      if (status >= 500) failures.push(`${method} ${url.slice(0, 80)} ${JSON.stringify(v)?.slice(0, 40)} -> ${status}: ${(await r.text()).slice(0, 200)}`);
      if (status >= 400) {
        const ct = r.headers()['content-type'] ?? '';
        if (!ct.includes('json')) failures.push(`${method} ${path} ${JSON.stringify(v)?.slice(0, 40)} -> ${status} non-JSON error`);
      }
    }
  }
  // Malformed bodies.
  for (const raw of ['{', 'null', '[]', '"x"', '{"name": 1e999}']) {
    const r = await request.fetch('/api/foods', { method: 'POST', headers: { 'content-type': 'application/json' }, data: raw, failOnStatusCode: false });
    if (r.status() >= 500) failures.push(`POST /api/foods raw ${raw} -> ${r.status()}`);
  }
  expect.soft(failures).toEqual([]);
  expect((await request.get('/api/health')).ok()).toBe(true);
});

test('MCP tools return tool errors (not crashes) for bad arguments', async ({ request }) => {
  const list = await (await request.post('/mcp', { data: { jsonrpc: '2.0', id: 1, method: 'tools/list' } })).json();
  const tools: { name: string; inputSchema: { properties: Record<string, unknown> } }[] = list.result.tools;
  const failures: string[] = [];
  for (const t of tools) {
    const keys = Object.keys(t.inputSchema.properties);
    const cases: unknown[] = [{}, { bogus: 1 }, ...keys.flatMap((k) => weird.slice(0, 14).map((v) => ({ [k]: v })))];
    for (const args of cases) {
      const r = await request.post('/mcp?user=Alex', { data: { jsonrpc: '2.0', id: 7, method: 'tools/call', params: { name: t.name, arguments: args } }, failOnStatusCode: false });
      if (r.status() !== 200) {
        failures.push(`${t.name} ${JSON.stringify(args).slice(0, 60)} -> HTTP ${r.status()}`);
        continue;
      }
      const body = await r.json();
      if (body.error) failures.push(`${t.name} ${JSON.stringify(args).slice(0, 60)} -> JSON-RPC error ${body.error.message}`);
      const text: string = body.result?.content?.[0]?.text ?? '';
      if (/internal error/i.test(text)) failures.push(`${t.name} ${JSON.stringify(args).slice(0, 60)} -> ${text.slice(0, 160)}`);
    }
  }
  // Protocol-level oddities.
  for (const raw of ['', '{', '[]', '[1,2]', '{"jsonrpc":"2.0"}', '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":"x"}']) {
    const r = await request.fetch('/mcp', { method: 'POST', headers: { 'content-type': 'application/json' }, data: raw, failOnStatusCode: false });
    if (r.status() >= 500) failures.push(`raw ${raw} -> ${r.status()}`);
  }
  expect.soft(failures).toEqual([]);
  expect((await request.get('/api/health')).ok()).toBe(true);
});

test('concurrent writes from two devices do not corrupt data', async ({ request }) => {
  const writes = [];
  for (let i = 0; i < 60; i++) {
    writes.push(request.post(`/api/users/${(i % 2) + 1}/entries`, { data: { date: '2030-01-01', name: `c${i}`, nutrients: { calories: 10 } } }));
    writes.push(request.post(`/api/users/${(i % 2) + 1}/weights`, { data: { date: `2030-01-${String((i % 28) + 1).padStart(2, '0')}`, weight: 70 + (i % 5), unit: 'kg' } }));
  }
  const res = await Promise.all(writes);
  expect(res.filter((r) => r.status() >= 500).map((r) => r.status())).toEqual([]);
  const d1 = await (await request.get('/api/users/1/diary?date=2030-01-01')).json();
  const d2 = await (await request.get('/api/users/2/diary?date=2030-01-01')).json();
  expect(d1.entries.length + d2.entries.length).toBe(60);
  expect(d1.totals.calories + d2.totals.calories).toBe(600);
});
