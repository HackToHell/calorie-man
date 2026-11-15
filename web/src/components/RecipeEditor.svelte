<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { api } from '../lib/api';
  import { kcal, num, optNum } from '../lib/format';
  import { pickOption, portionBody, scale, sum, unitOptions } from '../lib/portion';
  import { app } from '../lib/state.svelte';
  import type { Food } from '../lib/types';
  import FoodSearch from './FoodSearch.svelte';

  let {
    recipe = null,
    onsaved,
    oncancel,
    ondeleted,
  }: { recipe?: Food | null; onsaved: (f: Food) => void; oncancel?: () => void; ondeleted?: () => void } = $props();

  type Row = { food: Food; quantity: string; optValue: string };

  const init = untrack(() => recipe);
  let name = $state(init?.name ?? '');
  let aliases = $state(init?.aliases ?? '');
  let notes = $state(init?.notes ?? '');
  let yieldN = $state(String(init?.recipe_yield ?? 1));
  let cooked = $state(init && init.basis_unit === 'g' ? String(init.basis_amount) : '');
  let rows = $state<Row[]>([]);
  let picking = $state(!init);
  let propagate = $state(true);
  let busy = $state(false);
  let loading = $state(!!init);
  let err = $state<string | null>(null);

  onMount(async () => {
    if (!init?.ingredients) return;
    try {
      const foods = await Promise.all(init.ingredients.map((i) => api.food(i.food_id)));
      rows = init.ingredients.map((i, k) => {
        const f = foods[k];
        const o = pickOption(f, unitOptions(f), i.serving_id, i.unit);
        return { food: f, quantity: String(i.quantity), optValue: o.value };
      });
    } catch (e) {
      app.error(e);
    } finally {
      loading = false;
    }
  });

  function rowNutrients(r: Row) {
    const opts = unitOptions(r.food);
    const o = opts.find((x) => x.value === r.optValue) ?? opts[0];
    const q = Number(r.quantity);
    return scale(r.food, Number.isFinite(q) && q > 0 ? q * (o?.per ?? 0) : 0);
  }

  const total = $derived(sum(rows.map(rowNutrients)));
  const y = $derived((optNum(yieldN) ?? 0) > 0 ? (optNum(yieldN) as number) : 1);

  function add(f: Food) {
    const opts = unitOptions(f);
    const o = pickOption(f, opts);
    rows = [...rows, { food: f, quantity: '1', optValue: o.value }];
    picking = false;
  }

  async function save(e: SubmitEvent) {
    e.preventDefault();
    err = null;
    if (!rows.length) {
      err = 'Add at least one ingredient';
      return;
    }
    const ingredients = [];
    for (const r of rows) {
      const q = Number(r.quantity);
      if (!Number.isFinite(q) || q <= 0) {
        err = `Quantity for ${r.food.name} must be above 0`;
        return;
      }
      const opts = unitOptions(r.food);
      const o = opts.find((x) => x.value === r.optValue) ?? opts[0];
      ingredients.push({ food_id: r.food.id, ...portionBody(o, q) });
    }
    const body = {
      name: name.trim(),
      kind: 'recipe',
      aliases: aliases.trim() || null,
      notes: notes.trim() || null,
      recipe_yield: y,
      cooked_weight_g: optNum(cooked),
      ingredients,
    };
    busy = true;
    try {
      const r = init ? await api.updateFood(init.id, body, propagate) : await api.createFood(body);
      for (const w of r.warnings) app.toast(w, 'warn');
      app.touch();
      onsaved(r.food);
    } catch (e) {
      err = (e as Error).message;
    } finally {
      busy = false;
    }
  }

  async function remove() {
    if (!init || !confirm(`Delete recipe “${init.name}”? If it was logged it will be archived instead.`)) return;
    try {
      const r = await api.deleteFood(init.id);
      app.toast(r.archived ? `Archived ${init.name}` : `Deleted ${init.name}`);
      app.touch();
      ondeleted?.();
    } catch (e) {
      app.error(e);
    }
  }
</script>

<form class="stack" onsubmit={save} data-testid="recipe-editor">
  <label class="field"><span>Recipe name *</span><input class="input" bind:value={name} required maxlength="200" data-testid="recipe-name" /></label>
  <div class="grid2">
    <label class="field"
      ><span>Makes (servings)</span><input class="input num" type="number" step="any" min="0" bind:value={yieldN} data-testid="recipe-yield" /></label
    >
    <label class="field"
      ><span>Cooked weight (g, optional)</span><input
        class="input num"
        type="number"
        step="any"
        min="0"
        bind:value={cooked}
        placeholder="log portions by weight"
      /></label
    >
  </div>

  <div class="stack">
    <div class="row"><h3>Ingredients</h3></div>
    {#if loading}<p class="muted small">Loading ingredients…</p>{/if}
    {#each rows as r, i}
      {@const n = rowNutrients(r)}
      {@const opts = unitOptions(r.food)}
      <div class="irow" data-testid="ingredient-row">
        <div class="iname">
          <div class="ellipsis"><b>{r.food.name}</b>{#if r.food.brand}<span class="muted small">{' · '}{r.food.brand}</span>{/if}</div>
          <div class="tiny muted">{kcal(n.calories)} kcal · {num(n.protein, 1)} g protein</div>
        </div>
        <input class="input num" type="number" step="any" min="0" bind:value={r.quantity} aria-label="Quantity of {r.food.name}" data-testid="ingredient-qty" />
        <select class="input" bind:value={r.optValue} aria-label="Unit of {r.food.name}">
          {#each opts as o (o.value)}<option value={o.value}>{o.label}</option>{/each}
        </select>
        <button type="button" class="btn ghost icon" aria-label="Remove {r.food.name}" onclick={() => (rows = rows.filter((_, j) => j !== i))}>✕</button>
      </div>
    {/each}
    {#if picking}
      <div class="picker">
        <FoodSearch onpick={add} showRecent={true} excludeId={init?.id} placeholder="Find an ingredient in the library" />
        <p class="tiny muted">Ingredients must be in the food library. Add missing ones from the Foods page or the online search first.</p>
      </div>
    {:else}
      <button type="button" class="btn" onclick={() => (picking = true)} data-testid="add-ingredient">+ Add ingredient</button>
    {/if}
  </div>

  <div class="totals" data-testid="recipe-totals">
    <div>
      <div class="tiny muted">Per serving</div>
      <b class="num">{kcal((total.calories ?? 0) / y)} kcal</b>
      <span class="small" style:color="var(--protein-text)">{num((total.protein ?? 0) / y, 1)} g P</span>
      <span class="small" style:color="var(--carbs-text)">{num((total.carbs ?? 0) / y, 1)} g C</span>
      <span class="small" style:color="var(--fat-text)">{num((total.fat ?? 0) / y, 1)} g F</span>
    </div>
    <div class="small muted">Whole recipe: {kcal(total.calories)} kcal · {num(total.protein, 1)} g protein</div>
  </div>

  <details>
    <summary>Search words & notes</summary>
    <div class="stack" style="margin-top: 8px">
      <label class="field"><span>Search words</span><input class="input" bind:value={aliases} placeholder="e.g. oatmeal breakfast" /></label>
      <label class="field"><span>Notes</span><textarea class="input" bind:value={notes}></textarea></label>
    </div>
  </details>

  {#if init}
    <label class="check small"><input type="checkbox" bind:checked={propagate} /> Update past diary entries of this recipe</label>
  {/if}
  {#if err}<div class="banner error" role="alert">{err}</div>{/if}

  <div class="row actions">
    {#if init}<button type="button" class="btn danger" onclick={remove}>Delete</button>{/if}
    <span class="spacer"></span>
    {#if oncancel}<button type="button" class="btn" onclick={oncancel}>Cancel</button>{/if}
    <button class="btn primary" disabled={busy || loading} data-testid="save-recipe">{init ? 'Save recipe' : 'Create recipe'}</button>
  </div>
</form>

<style>
  .irow {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 80px minmax(0, 150px) auto;
    gap: 6px;
    align-items: center;
    padding: 6px 0;
    border-bottom: 1px solid var(--line);
  }
  .iname {
    min-width: 0;
  }
  @media (max-width: 520px) {
    .irow {
      grid-template-columns: 70px minmax(0, 1fr) auto;
    }
    .iname {
      grid-column: 1 / -1;
    }
  }
  .picker {
    border: 1px dashed var(--line);
    padding: 10px;
  }
  .totals {
    --t: 5px;
    clip-path: var(--terrace);
    background: var(--surface-2);
    padding: 10px 12px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .totals b {
    font-size: 1.1rem;
    margin-right: 8px;
  }
  .totals span {
    margin-right: 6px;
  }
  .actions {
    position: sticky;
    bottom: -14px;
    background: var(--card);
    padding: 8px 0;
    border-top: 1px solid var(--line);
    z-index: 2;
  }
  @media (max-width: 760px) {
    :global(main) .actions {
      bottom: calc(56px + env(safe-area-inset-bottom));
    }
    :global(dialog) .actions {
      bottom: -14px;
    }
  }
</style>
