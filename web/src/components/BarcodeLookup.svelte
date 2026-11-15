<script lang="ts">
  import { api } from '../lib/api';
  import { kcal, num } from '../lib/format';
  import { app } from '../lib/state.svelte';
  import type { Candidate, Food } from '../lib/types';

  let { onpick, oncreate }: { onpick: (f: Food) => void; oncreate?: (barcode: string) => void } = $props();

  let code = $state('');
  let loading = $state(false);
  let result = $state<{ barcode: string; food: Food | null; candidate: Candidate | null; errors: string[] } | null>(null);
  let err = $state<string | null>(null);

  async function lookup(e: SubmitEvent) {
    e.preventDefault();
    loading = true;
    err = null;
    result = null;
    try {
      result = await api.barcode(code.trim());
    } catch (e) {
      err = (e as Error).message;
    } finally {
      loading = false;
    }
  }

  async function importIt(c: Candidate) {
    try {
      const r = await api.importFood(c.source, c.source_id);
      app.toast(`Imported ${r.food.name}`);
      app.touch();
      onpick(r.food);
    } catch (e) {
      app.error(e);
    }
  }
</script>

<div class="stack">
  <form class="row" onsubmit={lookup}>
    <input class="input" inputmode="numeric" placeholder="UPC / EAN digits" bind:value={code} aria-label="Barcode" data-testid="barcode-input" />
    <button class="btn primary" disabled={loading || code.replace(/\D/g, '').length < 6} data-testid="barcode-lookup">{loading ? '…' : 'Look up'}</button>
  </form>
  <p class="tiny muted">Type the digits under the barcode. Camera scanning needs HTTPS, which a plain local server does not have.</p>
  {#if err}<div class="banner error">{err}</div>{/if}
  {#if result}
    {#each result.errors as e}<div class="banner small">{e}</div>{/each}
    {#if result.food}
      <div class="card hit">
        <div><b>{result.food.name}</b>{#if result.food.brand}<span class="muted">{' · '}{result.food.brand}</span>{/if}</div>
        <div class="small muted">Already in your library</div>
        <button class="btn primary" onclick={() => onpick(result!.food!)} data-testid="barcode-use">Use</button>
      </div>
    {:else if result.candidate}
      {@const c = result.candidate}
      <div class="card hit">
        <div><b>{c.name}</b>{#if c.brand}<span class="muted">{' · '}{c.brand}</span>{/if}</div>
        <div class="small muted">
          {kcal(c.nutrients.calories)} kcal · {num(c.nutrients.protein, 1)} g protein per {num(c.basis_amount)} {c.basis_unit} · {c.source === 'usda'
            ? 'USDA'
            : 'Open Food Facts'}
        </div>
        <button class="btn primary" onclick={() => importIt(c)} data-testid="barcode-import">Add to library</button>
      </div>
    {:else}
      <div class="banner">
        No product found for {result.barcode}.
        {#if oncreate}<button class="btn small" onclick={() => oncreate(result!.barcode)}>Create it from the label</button>{/if}
      </div>
    {/if}
  {/if}
</div>

<style>
  .hit {
    display: flex;
    flex-direction: column;
    gap: 6px;
    align-items: flex-start;
  }
</style>
