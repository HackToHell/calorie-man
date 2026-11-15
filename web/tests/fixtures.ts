import { test as base, expect, type APIRequestContext, type Page } from '@playwright/test';
import { spawn, spawnSync, type ChildProcess } from 'node:child_process';
import { mkdtempSync, rmSync } from 'node:fs';
import { createServer } from 'node:net';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { BIN, MOCK_PORT } from './paths';

export interface Server {
  url: string;
  db: string;
  proc: ChildProcess;
}

async function freePort(): Promise<number> {
  return new Promise((resolve, reject) => {
    const s = createServer();
    s.listen(0, '127.0.0.1', () => {
      const a = s.address();
      s.close(() => (typeof a === 'object' && a ? resolve(a.port) : reject(new Error('no port'))));
    });
  });
}

export async function startServer(opts: { seedDays?: number; env?: Record<string, string> } = {}): Promise<Server> {
  const dir = mkdtempSync(join(tmpdir(), 'calorie-man-e2e-'));
  const db = join(dir, 'calorie-man.db');
  if (opts.seedDays) {
    const r = spawnSync(BIN, ['seed-demo', '--db', db, '--days', String(opts.seedDays)], { encoding: 'utf8' });
    if (r.status !== 0) throw new Error(`seed failed: ${r.stderr}`);
  }
  const port = await freePort();
  const mock = `http://127.0.0.1:${MOCK_PORT}`;
  const proc = spawn(BIN, ['serve', '--db', db, '--bind', `127.0.0.1:${port}`], {
    env: {
      ...process.env,
      CALORIE_MAN_USDA_BASE: `${mock}/fdc`,
      CALORIE_MAN_OFF_BASE: `${mock}/off`,
      CALORIE_MAN_OFF_SEARCH_BASE: `${mock}/offsearch`,
      USDA_API_KEY: 'test-key',
      RUST_LOG: 'warn',
      ...opts.env,
    },
    stdio: ['ignore', 'ignore', 'pipe'],
  });
  let stderr = '';
  proc.stderr?.on('data', (d) => (stderr += d));
  const url = `http://127.0.0.1:${port}`;
  for (let i = 0; i < 100; i++) {
    try {
      const r = await fetch(`${url}/api/health`);
      if (r.ok) return { url, db, proc };
    } catch {
      /* not up yet */
    }
    if (proc.exitCode != null) break;
    await new Promise((r) => setTimeout(r, 50));
  }
  proc.kill();
  throw new Error(`calorie-man did not start: ${stderr}`);
}

export function stopServer(s: Server) {
  s.proc.kill('SIGTERM');
  try {
    rmSync(join(s.db, '..'), { recursive: true, force: true });
  } catch {
    /* ignore */
  }
}

type Fixtures = {
  seedDays: number;
  server: Server;
  consoleErrors: string[];
  api: Api;
};

export const test = base.extend<Fixtures>({
  seedDays: [0, { option: true }],
  server: async ({ seedDays }, use) => {
    const s = await startServer({ seedDays });
    await use(s);
    stopServer(s);
  },
  baseURL: async ({ server }, use) => {
    await use(server.url);
  },
  consoleErrors: async ({ page }, use) => {
    const errors: string[] = [];
    page.on('console', (m) => {
      if (m.type() === 'error' && !/Failed to load resource: the server responded with a status of 4\d\d/.test(m.text())) errors.push(m.text());
    });
    page.on('pageerror', (e) => errors.push(e.message));
    await use(errors);
    expect(errors, 'browser console errors').toEqual([]);
  },
  api: async ({ request }, use) => {
    await use(new Api(request));
  },
});

export { expect };

/** Thin REST helper for arranging test data quickly. */
export class Api {
  constructor(private r: APIRequestContext) {}
  async json(method: string, path: string, data?: unknown) {
    const resp = await this.r.fetch(`/api${path}`, { method, data });
    const body = await resp.json().catch(() => null);
    if (!resp.ok()) throw new Error(`${method} ${path} -> ${resp.status()}: ${JSON.stringify(body)}`);
    return body;
  }
  user(name: string, extra: Record<string, unknown> = {}) {
    return this.json('POST', '/users', { name, ...extra });
  }
  goal(uid: number, g: Record<string, unknown>) {
    return this.json('POST', `/users/${uid}/goals`, g);
  }
  async food(f: Record<string, unknown>) {
    return (await this.json('POST', '/foods', f)).food;
  }
  entry(uid: number, e: Record<string, unknown>) {
    return this.json('POST', `/users/${uid}/entries`, e);
  }
  diary(uid: number, date: string) {
    return this.json('GET', `/users/${uid}/diary?date=${date}`);
  }
  weight(uid: number, w: Record<string, unknown>) {
    return this.json('POST', `/users/${uid}/weights`, w);
  }
  async mcp(tool: string, args: Record<string, unknown>, query = '') {
    const resp = await this.r.post(`/mcp${query}`, {
      data: { jsonrpc: '2.0', id: 1, method: 'tools/call', params: { name: tool, arguments: args } },
    });
    const body = await resp.json();
    const text = body.result?.content?.[0]?.text ?? '';
    if (body.result?.isError) throw new Error(text);
    return JSON.parse(text);
  }
}

export function today(): string {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
}

export function dayOffset(n: number): string {
  const d = new Date();
  d.setDate(d.getDate() + n);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
}

/** Standard pantry used by several specs. Values per 100 g / 100 ml unless noted. */
export async function pantry(api: Api) {
  const oats = await api.food({
    name: 'Rolled oats',
    nutrients: { calories: 379, protein: 13.2, carbs: 67.7, fat: 6.5, fiber: 10.1, iron: 4.3 },
    servings: [{ label: '1/2 cup', amount: 40 }],
  });
  const milk = await api.food({
    name: 'Whole milk',
    brand: 'Kirkland Signature',
    basis_unit: 'ml',
    nutrients: { calories: 62, protein: 3.33, carbs: 5, fat: 3.33, calcium: 117, vitamin_d: 1.25 },
    servings: [{ label: 'cup', amount: 240 }],
    aliases: 'costco milk',
  });
  const banana = await api.food({
    name: 'Banana',
    nutrients: { calories: 89, protein: 1.09, carbs: 22.8, fat: 0.33, potassium: 358, vitamin_c: 8.7 },
    servings: [{ label: 'medium', amount: 118 }],
  });
  const pb = await api.food({
    name: 'Peanut butter',
    nutrients: { calories: 588, protein: 25, carbs: 20, fat: 50 },
    servings: [{ label: 'tbsp', amount: 16 }],
  });
  return { oats, milk, banana, pb };
}

/** Select the user in localStorage before the app boots. */
export async function asUser(page: Page, id: number) {
  await page.addInitScript((uid) => localStorage.setItem('calorie-man.user', String(uid)), id);
}

/** Open the log-food dialog: the header button on desktop, the octagon + on phones. */
export async function openAdd(page: Page) {
  const fab = page.getByTestId('fab');
  if (await fab.isVisible()) await fab.click();
  else await page.getByTestId('add-food').click();
  await expect(page.getByTestId('add-food-dialog')).toBeVisible();
}

export async function noHorizontalOverflow(page: Page) {
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth - window.innerWidth);
  expect(overflow, 'page should not scroll sideways').toBeLessThanOrEqual(1);
}
