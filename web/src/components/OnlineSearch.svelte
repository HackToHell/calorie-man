<script lang="ts">
  import { api } from '../lib/api';
  import { kcal, num } from '../lib/format';
  import { app } from '../lib/state.svelte';
  import type { Candidate, Food } from '../lib/types';

  let { onpick, query = $bindable('') }: { onpick: (f: Food) => void; query?: string } = $props();

  let source = $state('all');
  let kind = $state('all');
  let results = $state<Candidate[]>([]);
  let errors = $state<string[]>([]);
  let loading = $state(false);
  let searched = $state(false);
  let importing = $state<string | null>(null);
  let expanded = $state<string | null>(null);

  async function search(e?: SubmitEvent) {
    e?.preventDefault();
    if (!query.trim()) return;
    loading = true;
    errors = [];
    try {
      const r = await api.externalSearch(query.trim(), source, kind);
      results = r.results;
      errors = r.errors;
      searched = true;
    } catch (err) {
      results = [];
      errors = [(err as Error).message];
      searched = true;
    } finally {
      loading = false;
    }
  }

  async function use(c: Candidate) {
    importing = `${c.source}:${c.source_id}`;
    try {
      if (c.in_library) {
        onpick(await api.food(c.in_library));
        return;
      }
      const alias = query.trim().toLowerCase();
      const nameL = `${c.name} ${c.brand ?? ''}`.toLowerCase();
      const r = await api.importFood(c.source, c.source_id, nameL.includes(alias) ? undefined : alias);
      for (const w of r.warnings) app.toast(w, 'warn');
      app.toast(r.created ? `Imported ${r.food.name} into the shared library` : `${r.food.name} is already in the library`);
      app.touch();
      onpick(r.food);
    } catch (err) {
      app.error(err);
    } finally {
      importing = null;
    }
  }

  const per = (c: Candidate) => (c.basis_unit === 'serving' ? 'serving' : `${num(c.basis_amount)} ${c.basis_unit}`);
</script>

<div class="stack">
  <form class="row" onsubmit={search}>
    <input
      class="input"
      type="search"
      bind:value={query}
      placeholder="e.g. costco milk, banana, clif bar"
      aria-label="Search online databases"
      data-testid="online-query"
    />
    <button class="btn primary" disabled={loading || !query.trim()} data-testid="online-search">{loading ? 'Searching…' : 'Search'}</button>
  </form>
  <div class="row wrap small">
    <select class="input sel" bind:value={source} aria-label="Source">
      <option value="all">USDA + Open Food Facts</option>
      <option value="usda">USDA FoodData Central</option>
      <option value="off">Open Food Facts</option>
    </select>
    {#if source !== 'off'}
      <select class="input sel" bind:value={kind} aria-label="USDA type">
        <option value="all">All USDA foods</option>
        <option value="generic">Whole foods (full vitamins)</option>
        <option value="branded">Packaged / branded</option>
      </select>
    {/if}
  </div>
  {#each errors as e}<div class="banner error small" role="alert">{e}</div>{/each}
  {#if results.length}
    <ul class="list" data-testid="online-results">
      {#each results as c (`${c.source}:${c.source_id}`)}
        {@const key = `${c.source}:${c.source_id}`}
        <li class="res" data-testid="online-result">
          <div class="grow">
            <div>
              <b>{c.name}</b>{#if c.brand}<span class="muted">{' · '}{c.brand}</span>{/if}
            </div>
            <div class="small muted">
              {kcal(c.nutrients.calories)} kcal · {num(c.nutrients.protein, 1)} P · {num(c.nutrients.carbs, 1)} C · {num(c.nutrients.fat, 1)} F per {per(c)}
            </div>
            <div class="row wrap tags">
              <span class="chip">{c.source === 'usda' ? 'USDA' : 'Open Food Facts'}</span>
              {#if c.data_type && c.source === 'usda'}<span class="chip">{c.data_type}</span>{/if}
              <span class="chip">{c.nutrient_count} nutrients</span>
              {#if c.servings[0]}<span class="chip">{c.servings[0].label} = {num(c.servings[0].amount)} {c.basis_unit}</span>{/if}
              {#if c.in_library}<span class="chip accent">in library</span>{/if}
              <button type="button" class="linkish tiny" onclick={() => (expanded = expanded === key ? null : key)}>details</button>
            </div>
            {#if expanded === key}
              <div class="tiny muted details">
                {#if c.barcode}Barcode {c.barcode} · {/if}{#if c.category}{c.category} · {/if}
                {#if c.url}<a href={c.url} target="_blank" rel="noreferrer">source page</a>{/if}
                <div>
                  {#each Object.entries(c.nutrients).slice(0, 40) as [k, v]}
                    <span class="nv">{app.nutrientMap.get(k)?.label ?? k} {num(v)}{app.nutrientMap.get(k)?.unit}</span>
                  {/each}
                </div>
              </div>
            {/if}
          </div>
          <button class="btn small" class:primary={!c.in_library} disabled={importing != null} onclick={() => use(c)} data-testid="import-result">
            {importing === key ? '…' : c.in_library ? 'Use' : 'Add'}
          </button>
        </li>
      {/each}
    </ul>
  {:else if searched && !loading && !errors.length}
    <p class="empty small">Nothing found online. Try fewer words, or create the food yourself from the label.</p>
  {:else if !searched}
    <p class="small muted">
      Searches USDA FoodData Central (US products incl. Costco/Kirkland, plus whole foods with full vitamin data) and Open Food Facts. Adding a
      result copies it into the shared library.
    </p>
  {/if}
</div>

<style>
  .sel {
    width: auto;
    min-height: 34px;
    padding: 4px 8px;
  }
  .res {
    display: flex;
    gap: 10px;
    align-items: center;
    padding: 10px 2px;
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .tags {
    gap: 4px;
    margin-top: 4px;
  }
  .linkish {
    background: none;
    border: 0;
    color: var(--accent-text);
    cursor: pointer;
    padding: 0 4px;
  }
  .details {
    margin-top: 6px;
  }
  .nv {
    display: inline-block;
    margin-right: 10px;
  }
</style>
