import { api, setActingUser } from './api';
import { mealForNow, todayStr } from './format';
import type { Catalog, Diary, Food, NutrientDef, User } from './types';

export type Theme = 'system' | 'light' | 'dark';

// Per-device preferences. Keys were "eat.*" before the rename to Calorie Man; carry them over once.
const KEY_USER = 'calorie-man.user';
const KEY_THEME = 'calorie-man.theme';
for (const [old, key] of [
  ['eat.user', KEY_USER],
  ['eat.theme', KEY_THEME],
]) {
  const v = localStorage.getItem(old);
  if (v != null && localStorage.getItem(key) == null) localStorage.setItem(key, v);
  localStorage.removeItem(old);
}

export interface AddRequest {
  open: boolean;
  date: string;
  meal: string;
  query: string;
  /** Jump straight to the portion step with this food (and optionally a remembered portion). */
  food: Food | null;
  quantity: number | null;
  servingId: number | null;
  unit: string | null;
  seq: number;
}

export interface Route {
  page: string;
  parts: string[];
  params: URLSearchParams;
}

function parseHash(): Route {
  const raw = location.hash.replace(/^#\/?/, '');
  const [path, query] = raw.split('?');
  const parts = (path || 'today').split('/').filter(Boolean);
  return { page: parts[0] || 'today', parts: parts.slice(1), params: new URLSearchParams(query ?? '') };
}

export function navigate(path: string) {
  location.hash = path.startsWith('#') ? path : `#/${path.replace(/^\//, '')}`;
}

export interface Toast {
  id: number;
  kind: 'ok' | 'error' | 'warn';
  text: string;
}

class AppState {
  users = $state<User[]>([]);
  userId = $state<number | null>(null);
  catalog = $state<Catalog | null>(null);
  route = $state<Route>(parseHash());
  toasts = $state<Toast[]>([]);
  loaded = $state(false);
  loadError = $state<string | null>(null);
  /** Bumped whenever data changes in a way other views should notice. */
  version = $state(0);
  /** Today's diary for the current user (dashboard, sidebar streak). */
  today = $state<Diary | null>(null);
  theme = $state<Theme>((localStorage.getItem(KEY_THEME) as Theme) || 'system');
  add = $state<AddRequest>({ open: false, date: todayStr(), meal: 'breakfast', query: '', food: null, quantity: null, servingId: null, unit: null, seq: 0 });
  private toastSeq = 0;
  private todaySeq = 0;

  user = $derived(this.users.find((u) => u.id === this.userId) ?? null);
  nutrientMap = $derived(new Map<string, NutrientDef>((this.catalog?.nutrients ?? []).map((n) => [n.key, n])));

  constructor() {
    window.addEventListener('hashchange', () => (this.route = parseHash()));
    this.applyTheme();
  }

  setTheme(t: Theme) {
    this.theme = t;
    localStorage.setItem(KEY_THEME, t);
    this.applyTheme();
  }

  private applyTheme() {
    if (this.theme === 'system') delete document.documentElement.dataset.theme;
    else document.documentElement.dataset.theme = this.theme;
  }

  /** Open the log-food dialog from anywhere (header search, "/" key, the phone + button, suggestions). */
  openAdd(o: Partial<Omit<AddRequest, 'open' | 'seq'>> = {}) {
    this.add = {
      open: true,
      date: o.date ?? todayStr(),
      meal: o.meal ?? mealForNow(),
      query: o.query ?? '',
      food: o.food ?? null,
      quantity: o.quantity ?? null,
      servingId: o.servingId ?? null,
      unit: o.unit ?? null,
      seq: this.add.seq + 1,
    };
  }

  async loadToday() {
    const uid = this.userId;
    if (uid == null) return;
    const my = ++this.todaySeq;
    try {
      const d = await api.diary(uid, todayStr());
      if (my === this.todaySeq && uid === this.userId) this.today = d;
    } catch {
      /* the page that needs it shows its own error */
    }
  }

  async init() {
    try {
      const [users, catalog] = await Promise.all([api.users(), api.catalog()]);
      this.catalog = catalog;
      this.users = users;
      const saved = Number(localStorage.getItem(KEY_USER));
      const pick = users.find((u) => u.id === saved) ?? users[0] ?? null;
      this.selectUser(pick?.id ?? null);
      this.loaded = true;
    } catch (e) {
      this.loadError = (e as Error).message;
    }
  }

  selectUser(id: number | null) {
    if (id !== this.userId) this.today = null;
    this.userId = id;
    setActingUser(id);
    if (id != null) localStorage.setItem(KEY_USER, String(id));
    else localStorage.removeItem(KEY_USER);
  }

  async reloadUsers() {
    this.users = await api.users();
    if (!this.users.some((u) => u.id === this.userId)) this.selectUser(this.users[0]?.id ?? null);
  }

  touch() {
    this.version++;
  }

  toast(text: string, kind: Toast['kind'] = 'ok', ms = 3500) {
    const id = ++this.toastSeq;
    this.toasts = [...this.toasts, { id, kind, text }];
    setTimeout(() => (this.toasts = this.toasts.filter((t) => t.id !== id)), kind === 'error' ? ms * 2 : ms);
  }

  error(e: unknown) {
    this.toast(e instanceof Error ? e.message : String(e), 'error');
  }
}

export const app = new AppState();
