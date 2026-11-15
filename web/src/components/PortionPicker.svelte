<script lang="ts">
  import { kcal, num } from '../lib/format';
  import { scale, unitOptions, type UnitOption } from '../lib/portion';
  import type { Food } from '../lib/types';

  let {
    food,
    quantity = $bindable(1),
    optValue = $bindable(''),
    extra = [],
  }: { food: Food; quantity?: number | string; optValue?: string; extra?: UnitOption[] } = $props();

  const opts = $derived([...extra, ...unitOptions(food)]);
  const opt = $derived(opts.find((o) => o.value === optValue) ?? opts[0]);
  const qty = $derived(Number(quantity));
  const valid = $derived(Number.isFinite(qty) && qty > 0);
  const amount = $derived(valid ? qty * (opt?.per ?? 0) : 0);
  const n = $derived.by(() => {
    if (opt?.per1) {
      const out: Record<string, number> = {};
      for (const [k, v] of Object.entries(opt.per1)) out[k] = v * (valid ? qty : 0);
      return out;
    }
    return scale(food, amount);
  });
  const isMeasure = $derived(opt?.unit != null && opt.unit !== 'serving');
  const quick = $derived(isMeasure ? [25, 50, 100, 150, 200, 250] : [0.5, 1, 1.5, 2, 3]);

  $effect(() => {
    if (!opts.some((o) => o.value === optValue) && opts[0]) optValue = opts[0].value;
  });
</script>

<div class="stack">
  <div class="row">
    <input
      class="input num qty"
      type="number"
      inputmode="decimal"
      step="any"
      min="0"
      bind:value={quantity}
      aria-label="Quantity"
      data-testid="qty"
    />
    <select class="input" bind:value={optValue} aria-label="Unit" data-testid="unit">
      {#each opts as o (o.value)}
        <option value={o.value}>{o.label}</option>
      {/each}
    </select>
  </div>
  <div class="row wrap quick">
    {#each quick as qv}
      <button type="button" class="btn small" class:primary={qty === qv} onclick={() => (quantity = qv)}>{qv === 0.5 ? '½' : qv === 1.5 ? '1½' : qv}</button>
    {/each}
  </div>
  <div class="preview" data-testid="portion-preview">
    <div><b class="num">{valid ? kcal(n.calories) : '–'}</b><span>kcal</span></div>
    <div style:color="var(--protein-text)"><b class="num">{valid ? num(n.protein ?? null, 1) : '–'}</b><span>protein</span></div>
    <div style:color="var(--carbs-text)"><b class="num">{valid ? num(n.carbs ?? null, 1) : '–'}</b><span>carbs</span></div>
    <div style:color="var(--fat-text)"><b class="num">{valid ? num(n.fat ?? null, 1) : '–'}</b><span>fat</span></div>
  </div>
  {#if valid && food.basis_unit !== 'serving' && !isMeasure && !opt?.per1}
    <p class="tiny muted">= {num(amount)} {food.basis_unit}</p>
  {/if}
  {#if !valid}<p class="tiny" style:color="var(--danger)">Enter a quantity above 0.</p>{/if}
</div>

<style>
  .qty {
    width: 110px;
    flex: none;
  }
  .quick .btn {
    min-width: 44px;
  }
  .preview {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 6px;
    --t: 5px;
    clip-path: var(--terrace);
    background: var(--surface-2);
    padding: 10px;
    text-align: center;
  }
  .preview b {
    display: block;
    font-family: var(--display);
    font-weight: 400;
    font-size: 1.4rem;
  }
  .preview span {
    font-size: 0.72rem;
    color: var(--muted);
  }
</style>
