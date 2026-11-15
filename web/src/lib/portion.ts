import { MEASURE } from './format';
import type { Food, Nutrients } from './types';

export interface UnitOption {
  /** "s:<serving id>" or "u:<unit>" */
  value: string;
  label: string;
  servingId: number | null;
  unit: string | null;
  /** Basis units per 1 of this option. */
  per: number;
  /** For an entry's original unit that the food no longer has: nutrients per 1 as logged. */
  per1?: Nutrients;
}

/**
 * If an entry was logged with a unit the food no longer offers (a deleted serving, a changed basis),
 * return an option that keeps it "as logged" so editing the quantity scales what was eaten.
 */
export function legacyOption(food: Food, entry: { unit: string; serving_id: number | null; quantity: number; amount: number | null; nutrients: Nutrients }): UnitOption | null {
  const opts = unitOptions(food);
  const u = entry.unit.toLowerCase();
  const known =
    (entry.serving_id != null && opts.some((o) => o.servingId === entry.serving_id)) ||
    food.servings.some((s) => s.label.toLowerCase() === u) ||
    opts.some((o) => o.unit === entry.unit);
  if (known || entry.quantity <= 0) return null;
  const per1: Nutrients = {};
  for (const [k, v] of Object.entries(entry.nutrients)) per1[k] = v / entry.quantity;
  return { value: 'legacy', label: `${entry.unit} (as logged)`, servingId: null, unit: null, per: (entry.amount ?? 0) / entry.quantity, per1 };
}

export function unitOptions(food: Food): UnitOption[] {
  const out: UnitOption[] = [];
  for (const s of food.servings) {
    const label = food.basis_unit === 'serving' ? s.label : `${s.label} (${round(s.amount)} ${food.basis_unit})`;
    out.push({ value: `s:${s.id}`, label, servingId: s.id, unit: null, per: s.amount });
  }
  if (food.basis_unit === 'serving') {
    if (!food.servings.some((s) => s.label.toLowerCase() === 'serving')) {
      out.push({ value: 'u:serving', label: 'serving', servingId: null, unit: 'serving', per: 1 });
    }
  } else {
    for (const u of food.units) {
      const m = MEASURE[u];
      if (m) out.push({ value: `u:${u}`, label: u, servingId: null, unit: u, per: m.f });
    }
  }
  return out;
}

function round(v: number) {
  return Math.round(v * 10) / 10;
}

/** Pick the option matching a previous entry (serving id or unit label), else the default serving. */
export function pickOption(food: Food, opts: UnitOption[], servingId?: number | null, unit?: string | null): UnitOption {
  if (servingId != null) {
    const o = opts.find((o) => o.servingId === servingId);
    if (o) return o;
  }
  if (unit) {
    const byLabel = food.servings.find((s) => s.label.toLowerCase() === unit.toLowerCase());
    if (byLabel) return opts.find((o) => o.servingId === byLabel.id) ?? opts[0];
    const o = opts.find((o) => o.unit === unit);
    if (o) return o;
  }
  const def = food.servings.find((s) => s.is_default) ?? food.servings[0];
  if (def) return opts.find((o) => o.servingId === def.id) ?? opts[0];
  return opts.find((o) => o.unit === food.basis_unit) ?? opts[0];
}

export function scale(food: Food, amount: number): Nutrients {
  const f = amount / food.basis_amount;
  const out: Nutrients = {};
  for (const [k, v] of Object.entries(food.nutrients)) out[k] = v * f;
  return out;
}

export function sum(list: Nutrients[]): Nutrients {
  const out: Nutrients = {};
  for (const n of list) for (const [k, v] of Object.entries(n)) out[k] = (out[k] ?? 0) + v;
  return out;
}

export function portionBody(opt: UnitOption, quantity: number) {
  return opt.servingId != null ? { quantity, serving_id: opt.servingId } : { quantity, unit: opt.unit };
}
