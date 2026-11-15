export const KG_PER_LB = 0.45359237;

/** Form values: Svelte binds `type="number"` inputs as numbers (or undefined when empty), text inputs as strings. */
export function txt(v: unknown): string {
  return v == null ? '' : String(v).trim();
}

/** Parse an optional numeric form value: '' or undefined -> null. */
export function optNum(v: unknown): number | null {
  const t = txt(v).replace(',', '.');
  return t === '' ? null : Number(t);
}

export function todayStr(d = new Date()): string {
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, '0');
  const day = String(d.getDate()).padStart(2, '0');
  return `${y}-${m}-${day}`;
}

export function parseDate(s: string): Date {
  const [y, m, d] = s.split('-').map(Number);
  return new Date(y, m - 1, d);
}

export function addDays(s: string, n: number): string {
  const d = parseDate(s);
  d.setDate(d.getDate() + n);
  return todayStr(d);
}

export function daysBetween(a: string, b: string): number {
  return Math.round((parseDate(b).getTime() - parseDate(a).getTime()) / 86400000);
}

export function isValidDate(s: string | null | undefined): s is string {
  if (!s || !/^\d{4}-\d{2}-\d{2}$/.test(s)) return false;
  const d = parseDate(s);
  return todayStr(d) === s;
}

export function fmtDay(s: string, opts: Intl.DateTimeFormatOptions = { weekday: 'short', month: 'short', day: 'numeric' }): string {
  return parseDate(s).toLocaleDateString(undefined, opts);
}

export function relDay(s: string): string {
  const diff = daysBetween(todayStr(), s);
  if (diff === 0) return 'Today';
  if (diff === -1) return 'Yesterday';
  if (diff === 1) return 'Tomorrow';
  const sameYear = parseDate(s).getFullYear() === new Date().getFullYear();
  return fmtDay(s, sameYear ? { weekday: 'short', month: 'short', day: 'numeric' } : { year: 'numeric', month: 'short', day: 'numeric' });
}

const nf0 = new Intl.NumberFormat(undefined, { maximumFractionDigits: 0 });
const nf1 = new Intl.NumberFormat(undefined, { maximumFractionDigits: 1 });
const nf2 = new Intl.NumberFormat(undefined, { maximumFractionDigits: 2 });

/** Whole numbers for kcal, one decimal for grams, more precision for tiny amounts. */
export function num(v: number | null | undefined, digits?: number): string {
  if (v == null || !Number.isFinite(v)) return '–';
  if (digits === 0) return nf0.format(v);
  if (digits === 1) return nf1.format(v);
  if (digits === 2) return nf2.format(v);
  const a = Math.abs(v);
  if (a >= 100) return nf0.format(v);
  if (a >= 1) return nf1.format(v);
  return nf2.format(v);
}

export function kcal(v: number | null | undefined): string {
  return v == null ? '–' : nf0.format(Math.round(v));
}

export function signed(v: number, digits = 0): string {
  const s = digits === 0 ? nf0.format(Math.round(Math.abs(v))) : Math.abs(v).toFixed(digits);
  return `${v > 0 ? '+' : v < 0 ? '−' : ''}${s}`;
}

/** Weights always show one decimal: 82.0, not 82. */
export function w1(v: number | null | undefined): string {
  return v == null || !Number.isFinite(v) ? '–' : v.toFixed(1);
}

export function toDisplayWeight(kg: number | null | undefined, unit: string): number | null {
  if (kg == null) return null;
  return unit === 'lb' ? kg / KG_PER_LB : kg;
}

export function fmtWeight(kg: number | null | undefined, unit: string, digits = 1): string {
  const v = toDisplayWeight(kg, unit);
  return v == null ? '–' : `${v.toFixed(digits)} ${unit}`;
}

export function round(v: number, places = 1): number {
  const p = 10 ** places;
  return Math.round(v * p) / p;
}

export const MEAL_LABELS: Record<string, string> = {
  breakfast: 'Breakfast',
  lunch: 'Lunch',
  dinner: 'Dinner',
  snacks: 'Snacks',
};

export function mealForNow(): string {
  const h = new Date().getHours();
  if (h >= 4 && h <= 10) return 'breakfast';
  if (h >= 11 && h <= 15) return 'lunch';
  if (h >= 16 && h <= 21) return 'dinner';
  return 'snacks';
}

export function basisLabel(amount: number, unit: string): string {
  if (unit === 'serving') return amount === 1 ? '1 serving' : `${num(amount)} servings`;
  return `${num(amount)} ${unit}`;
}

export function servingText(label: string, amount: number, unit: string): string {
  if (unit === 'serving') return label;
  return `${label} (${num(amount)} ${unit})`;
}

export const MEASURE: Record<string, { dim: 'g' | 'ml'; f: number }> = {
  g: { dim: 'g', f: 1 },
  kg: { dim: 'g', f: 1000 },
  oz: { dim: 'g', f: 28.349523125 },
  lb: { dim: 'g', f: 453.59237 },
  ml: { dim: 'ml', f: 1 },
  l: { dim: 'ml', f: 1000 },
  dl: { dim: 'ml', f: 100 },
  tsp: { dim: 'ml', f: 4.92892159375 },
  tbsp: { dim: 'ml', f: 14.78676478125 },
  'fl oz': { dim: 'ml', f: 29.5735295625 },
  cup: { dim: 'ml', f: 236.5882365 },
  pint: { dim: 'ml', f: 473.176473 },
  quart: { dim: 'ml', f: 946.352946 },
  gallon: { dim: 'ml', f: 3785.411784 },
};

// ---- times of day ("HH:MM" strings)

const uses12h = (() => {
  try {
    const hc = new Intl.DateTimeFormat(undefined, { hour: 'numeric' }).resolvedOptions().hourCycle;
    return hc === 'h12' || hc === 'h11';
  } catch {
    return true;
  }
})();

export function hhmmToMinutes(t: string | null | undefined): number | null {
  if (!t) return null;
  const [h, m] = t.split(':').map(Number);
  return Number.isFinite(h) && Number.isFinite(m) ? h * 60 + m : null;
}

export function nowHHMM(d = new Date()): string {
  return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`;
}

/** "7:40" / "7:40 PM" in 12-hour locales, "19:40" in 24-hour ones. */
export function fmtTime(t: string | null | undefined, period = true): string {
  const m = hhmmToMinutes(t);
  if (m == null) return '';
  const h = Math.floor(m / 60),
    mm = String(m % 60).padStart(2, '0');
  if (!uses12h) return `${String(h).padStart(2, '0')}:${mm}`;
  const h12 = h % 12 === 0 ? 12 : h % 12;
  return period ? `${h12}:${mm} ${h < 12 ? 'AM' : 'PM'}` : `${h12}:${mm}`;
}

/** "8 h 40 m" */
export function fmtDuration(minutes: number): string {
  const m = Math.max(0, Math.round(minutes));
  const h = Math.floor(m / 60);
  return h ? `${h} h ${m % 60} m` : `${m} m`;
}

export const MEAL_COLORS: Record<string, string> = {
  breakfast: 'var(--butter)',
  lunch: 'var(--peach)',
  dinner: 'var(--lilac)',
  snacks: 'var(--sea)',
};

/**
 * Default time for a new entry: now when logging today's current meal; otherwise the meal's usual
 * time if known (visible and editable), else empty (unknown).
 */
export function defaultTime(date: string, meal: string, usual: Record<string, string> | undefined): string {
  if (date === todayStr() && meal === mealForNow()) return nowHHMM();
  return usual?.[meal] ?? '';
}
