import type {
  Candidate,
  Catalog,
  Diary,
  Energy,
  Entry,
  Exercise,
  Food,
  Goal,
  RecentFood,
  Settings,
  Summary,
  User,
  Weight,
} from './types';

export class ApiError extends Error {
  constructor(
    public status: number,
    message: string,
  ) {
    super(message);
  }
}

let actingUser: number | null = null;
export function setActingUser(id: number | null) {
  actingUser = id;
}

async function call<T>(method: string, path: string, body?: unknown): Promise<T> {
  const headers: Record<string, string> = {};
  if (body !== undefined) headers['content-type'] = 'application/json';
  if (actingUser != null) headers['x-calorie-man-user'] = String(actingUser);
  let resp: Response;
  try {
    resp = await fetch(`/api${path}`, { method, headers, body: body === undefined ? undefined : JSON.stringify(body) });
  } catch {
    throw new ApiError(0, 'Cannot reach the server. Is it running?');
  }
  const text = await resp.text();
  let data: any = null;
  try {
    data = text ? JSON.parse(text) : null;
  } catch {
    data = text;
  }
  if (!resp.ok) {
    const msg = (data && typeof data === 'object' && data.error) || (typeof data === 'string' && data) || resp.statusText;
    throw new ApiError(resp.status, msg);
  }
  return data as T;
}

const q = (params: Record<string, string | number | boolean | null | undefined>) => {
  const s = new URLSearchParams();
  for (const [k, v] of Object.entries(params)) if (v !== undefined && v !== null && v !== '') s.set(k, String(v));
  const str = s.toString();
  return str ? `?${str}` : '';
};

export interface Saved {
  food: Food;
  warnings: string[];
  entries_updated?: number;
}

export const api = {
  catalog: () => call<Catalog>('GET', '/nutrients'),
  users: () => call<User[]>('GET', '/users'),
  createUser: (u: Partial<User>) => call<User>('POST', '/users', u),
  updateUser: (id: number, u: Partial<User>) => call<User>('PUT', `/users/${id}`, u),
  deleteUser: (id: number) => call<unknown>('DELETE', `/users/${id}`),

  goals: (uid: number) => call<Goal[]>('GET', `/users/${uid}/goals`),
  setGoal: (uid: number, g: Partial<Goal>) => call<Goal>('POST', `/users/${uid}/goals`, g),
  deleteGoal: (id: number) => call<unknown>('DELETE', `/goals/${id}`),
  energy: (uid: number) => call<Energy>('GET', `/users/${uid}/energy`),

  foods: (params: { q?: string; kind?: string; limit?: number; user_id?: number; archived?: boolean }) =>
    call<Food[]>('GET', `/foods${q(params)}`),
  food: (id: number) => call<Food>('GET', `/foods/${id}`),
  createFood: (f: unknown) => call<Saved>('POST', '/foods', f),
  updateFood: (id: number, f: unknown, propagate = true) => call<Saved>('PUT', `/foods/${id}${q({ propagate })}`, f),
  deleteFood: (id: number) => call<{ archived: boolean; deleted: boolean }>('DELETE', `/foods/${id}`),
  restoreFood: (id: number) => call<Food>('POST', `/foods/${id}/restore`),
  recentFoods: (uid: number, limit = 20) => call<RecentFood[]>('GET', `/users/${uid}/recent-foods${q({ limit })}`),

  externalSearch: (query: string, source: string, usdaKind = 'all') =>
    call<{ results: Candidate[]; errors: string[] }>('GET', `/external/search${q({ q: query, source, usda_kind: usdaKind, limit: 15 })}`),
  barcode: (code: string) =>
    call<{ barcode: string; food: Food | null; candidate: Candidate | null; errors: string[] }>('GET', `/external/barcode/${encodeURIComponent(code)}`),
  importFood: (source: string, id: string, aliases?: string) =>
    call<{ food: Food; created: boolean; warnings: string[] }>('POST', '/external/import', { source, id, aliases }),

  diary: (uid: number, date: string) => call<Diary>('GET', `/users/${uid}/diary${q({ date })}`),
  addEntry: (uid: number, e: unknown) => call<Entry>('POST', `/users/${uid}/entries`, e),
  updateEntry: (id: number, e: unknown) => call<Entry>('PUT', `/entries/${id}`, e),
  deleteEntry: (id: number) => call<Entry>('DELETE', `/entries/${id}`),
  copy: (uid: number, body: { from_date: string; to_date: string; meal?: string; to_meal?: string; from_user_id?: number }) =>
    call<Entry[]>('POST', `/users/${uid}/copy`, body),
  addExercise: (uid: number, e: { date: string; name: string; calories: number; duration_min?: number | null }) =>
    call<Exercise>('POST', `/users/${uid}/exercises`, e),
  deleteExercise: (id: number) => call<unknown>('DELETE', `/exercises/${id}`),

  weights: (uid: number, from?: string, to?: string) => call<Weight[]>('GET', `/users/${uid}/weights${q({ from, to })}`),
  addWeight: (uid: number, w: { date: string; weight: number; unit: string; body_fat_pct?: number | null; note?: string | null }) =>
    call<Weight>('POST', `/users/${uid}/weights`, w),
  updateWeight: (id: number, w: unknown) => call<Weight>('PUT', `/weights/${id}`, w),
  deleteWeight: (id: number) => call<Weight>('DELETE', `/weights/${id}`),

  summary: (uid: number, from: string, to: string) => call<Summary>('GET', `/users/${uid}/summary${q({ from, to })}`),
  settings: () => call<Settings>('GET', '/settings'),
  saveSettings: (s: { usda_api_key: string | null }) => call<Settings>('PUT', '/settings', s),
};
