<script lang="ts">
  import { untrack } from 'svelte';
  import { api } from '../lib/api';
  import { num, optNum, txt } from '../lib/format';
  import { app } from '../lib/state.svelte';
  import type { Food } from '../lib/types';

  let {
    food = null,
    initialName = '',
    initialBarcode = '',
    onsaved,
    oncancel,
    ondeleted,
  }: {
    food?: Food | null;
    initialName?: string;
    initialBarcode?: string;
    onsaved: (f: Food) => void;
    oncancel?: () => void;
    ondeleted?: () => void;
  } = $props();

  type ServingRow = { id?: number; label: string; amount: string; is_default: boolean };

  // The editor edits a snapshot; the parent remounts it (with {#key}) to edit something else.
  const init = untrack(() => food);
  let name = $state(init?.name ?? untrack(() => initialName));
  let brand = $state(init?.brand ?? '');
  let basisAmount = $state(String(init?.basis_amount ?? 100));
  let basisUnit = $state<string>(init?.basis_unit ?? 'g');
  let barcode = $state(init?.barcode ?? untrack(() => initialBarcode));
  let aliases = $state(init?.aliases ?? '');
  let notes = $state(init?.notes ?? '');
  let propagate = $state(true);
  let busy = $state(false);
  let err = $state<string | null>(null);
  let values = $state<Record<string, string>>(
    Object.fromEntries(Object.entries(init?.nutrients ?? {}).map(([k, v]) => [k, String(Math.round(v * 10000) / 10000)])),
  );
  let servings = $state<ServingRow[]>(
    (init?.servings ?? []).map((s) => ({ id: s.id, label: s.label, amount: String(s.amount), is_default: s.is_default })),
  );

  const defs = $derived(app.catalog?.nutrients ?? []);
  const main = ['calories', 'protein', 'carbs', 'fat', 'fiber', 'sugar', 'added_sugar', 'saturated_fat', 'sodium'];
  const mainDefs = $derived(main.map((k) => defs.find((d) => d.key === k)).filter((d) => d != null));
  const restDefs = $derived(defs.filter((d) => !main.includes(d.key)));
  const restFilled = $derived(restDefs.filter((d) => txt(values[d.key]) !== '').length);

  const est = $derived.by(() => {
    const p = optNum(values.protein) ?? 0,
      c = optNum(values.carbs) ?? 0,
      f = optNum(values.fat) ?? 0,
      a = optNum(values.alcohol) ?? 0;
    return 4 * p + 4 * c + 9 * f + 7 * a;
  });
  const calMismatch = $derived.by(() => {
    const cal = optNum(values.calories);
    if (cal == null || !Number.isFinite(cal)) return false;
    if (!(values.protein || values.carbs || values.fat)) return false;
    return Math.abs(cal - est) > Math.max(10, 0.25 * Math.max(cal, est));
  });

  function addServing() {
    servings = [...servings, { label: '', amount: '', is_default: servings.length === 0 }];
  }

  function setDefault(i: number) {
    servings = servings.map((s, j) => ({ ...s, is_default: i === j }));
  }

  async function save(e: SubmitEvent) {
    e.preventDefault();
    err = null;
    const nutrients: Record<string, number | null> = {};
    for (const d of defs) {
      const v = optNum(values[d.key]);
      if (v == null) continue;
      if (!Number.isFinite(v) || v < 0) {
        err = `${d.label} must be a non-negative number`;
        return;
      }
      nutrients[d.key] = v;
    }
    const sv = [];
    for (const s of servings) {
      if (!txt(s.label) && !txt(s.amount)) continue;
      const a = optNum(s.amount) ?? NaN;
      if (!txt(s.label) || !Number.isFinite(a) || a <= 0) {
        err = 'Each serving needs a label and an amount above 0';
        return;
      }
      sv.push({ id: s.id, label: txt(s.label), amount: a, is_default: s.is_default });
    }
    const body = {
      name: name.trim(),
      brand: brand.trim() || null,
      basis_amount: optNum(basisAmount) ?? NaN,
      basis_unit: basisUnit,
      barcode: barcode.trim() || null,
      aliases: aliases.trim() || null,
      notes: notes.trim() || null,
      nutrients,
      servings: sv,
      source: init?.source,
    };
    busy = true;
    try {
      const r = init ? await api.updateFood(init.id, body, propagate) : await api.createFood(body);
      for (const w of r.warnings) app.toast(w, 'warn', 6000);
      if (init && r.entries_updated) app.toast(`Updated ${r.entries_updated} past diary entries`);
      app.touch();
      onsaved(r.food);
    } catch (e) {
      err = (e as Error).message;
    } finally {
      busy = false;
    }
  }

  async function remove() {
    if (!init || !confirm(`Delete “${init.name}”? If it was logged or used in a recipe it will be archived instead.`)) return;
    try {
      const r = await api.deleteFood(init.id);
      app.toast(r.archived ? `Archived ${init.name} (it is used in diaries or recipes)` : `Deleted ${init.name}`);
      app.touch();
      ondeleted?.();
    } catch (e) {
      app.error(e);
    }
  }
</script>

<form class="stack" onsubmit={save} data-testid="food-editor">
  <div class="grid2">
    <label class="field"><span>Name *</span><input class="input" bind:value={name} required maxlength="200" data-testid="food-name" /></label>
    <label class="field"><span>Brand</span><input class="input" bind:value={brand} maxlength="100" data-testid="food-brand" /></label>
  </div>

  <div class="basis">
    <span class="small muted">Nutrients per</span>
    <input class="input num amt" type="number" step="any" min="0" bind:value={basisAmount} aria-label="Basis amount" data-testid="basis-amount" />
    <select class="input unit" bind:value={basisUnit} aria-label="Basis unit" data-testid="basis-unit">
      <option value="g">g</option>
      <option value="ml">ml</option>
      <option value="serving">serving(s)</option>
    </select>
  </div>

  <div class="ngrid">
    {#each mainDefs as d (d.key)}
      <label class="field">
        <span>{d.label} <span class="faint">({d.unit})</span></span>
        <input class="input num" inputmode="decimal" bind:value={values[d.key]} data-testid="n-{d.key}" placeholder="–" />
      </label>
    {/each}
  </div>
  {#if calMismatch}
    <div class="banner small">Calories don’t match the macros (about {num(est, 0)} kcal from 4/4/9). Double-check the label.</div>
  {/if}

  <details>
    <summary>More nutrients: fats, vitamins & minerals {#if restFilled}<span class="chip accent">{restFilled} filled</span>{/if}</summary>
    <div class="ngrid more">
      {#each restDefs as d (d.key)}
        <label class="field">
          <span>{d.label} <span class="faint">({d.unit})</span></span>
          <input class="input num" inputmode="decimal" bind:value={values[d.key]} data-testid="n-{d.key}" placeholder="–" />
        </label>
      {/each}
    </div>
    <p class="tiny muted">Leave unknown values empty; empty means “unknown”, not zero.</p>
  </details>

  <div class="stack">
    <div class="row">
      <h3>Servings</h3>
      <span class="spacer"></span>
      <button type="button" class="btn small" onclick={addServing} data-testid="add-serving">+ Serving</button>
    </div>
    {#if servings.length === 0}
      <p class="small muted">Add servings like “cup = 240 ml” or “bar = 1 serving” so you can log “2 cups”.</p>
    {/if}
    {#each servings as s, i}
      <div class="srow" data-testid="serving-row">
        <input class="input" placeholder="label, e.g. cup" bind:value={s.label} aria-label="Serving label" />
        <span class="small muted">=</span>
        <input class="input num" type="number" step="any" min="0" placeholder="amount" bind:value={s.amount} aria-label="Serving amount" />
        <span class="small muted u">{basisUnit === 'serving' ? 'serv.' : basisUnit}</span>
        <label class="check tiny" title="Default serving"><input type="radio" checked={s.is_default} onchange={() => setDefault(i)} /> default</label>
        <button type="button" class="btn ghost icon" aria-label="Remove serving" onclick={() => (servings = servings.filter((_, j) => j !== i))}>✕</button>
      </div>
    {/each}
  </div>

  <details>
    <summary>Barcode, search words & notes</summary>
    <div class="stack" style="margin-top: 8px">
      <label class="field"><span>Barcode (UPC/EAN)</span><input class="input" inputmode="numeric" bind:value={barcode} /></label>
      <label class="field"
        ><span>Search words</span><input class="input" bind:value={aliases} placeholder="e.g. costco milk" data-testid="food-aliases" /></label
      >
      <label class="field"><span>Notes</span><textarea class="input" bind:value={notes}></textarea></label>
    </div>
  </details>

  {#if init}
    <label class="check small"
      ><input type="checkbox" bind:checked={propagate} /> Update past diary entries that use this food (and recipes that contain it)</label
    >
    <p class="tiny muted">Source: {init.source}{init.source_id ? ` #${init.source_id}` : ''}</p>
  {/if}

  {#if err}<div class="banner error" role="alert">{err}</div>{/if}

  <div class="row actions">
    {#if init}
      <button type="button" class="btn danger" onclick={remove}>Delete</button>
    {/if}
    <span class="spacer"></span>
    {#if oncancel}<button type="button" class="btn" onclick={oncancel}>Cancel</button>{/if}
    <button class="btn primary" disabled={busy} data-testid="save-food">{init ? 'Save changes' : 'Save food'}</button>
  </div>
</form>

<style>
  .basis {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }
  .amt {
    width: 100px;
  }
  .unit {
    width: 140px;
  }
  .ngrid {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 8px 10px;
  }
  .ngrid.more {
    margin-top: 10px;
  }
  @media (max-width: 560px) {
    .ngrid {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }
  .srow {
    display: grid;
    grid-template-columns: minmax(0, 1.6fr) auto minmax(0, 1fr) auto auto auto;
    gap: 6px;
    align-items: center;
  }
  @media (max-width: 520px) {
    .srow {
      grid-template-columns: auto minmax(0, 1fr) auto auto auto;
      padding-bottom: 8px;
      border-bottom: 1px solid var(--line);
    }
    .srow > :first-child {
      grid-column: 1 / -1;
    }
  }
  .u {
    min-width: 34px;
  }
  .actions {
    position: sticky;
    bottom: -14px;
    background: var(--card);
    padding: 8px 0;
    border-top: 1px solid var(--line);
    z-index: 2;
  }
  /* On a page (not in a dialog) keep the buttons above the phone tab bar. */
  @media (max-width: 760px) {
    :global(main) .actions {
      bottom: calc(56px + env(safe-area-inset-bottom));
    }
    :global(dialog) .actions {
      bottom: -14px;
    }
  }
</style>
