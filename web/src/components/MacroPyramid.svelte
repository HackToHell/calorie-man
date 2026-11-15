<script lang="ts">
  import { num } from '../lib/format';
  import type { Nutrients } from '../lib/types';

  /** Share of calories from protein, fat and carbs, stacked smallest-on-top like a terrace. */
  let { totals }: { totals: Nutrients } = $props();

  const tiers = $derived.by(() => {
    const p = (totals.protein ?? 0) * 4,
      c = (totals.carbs ?? 0) * 4,
      f = (totals.fat ?? 0) * 9;
    const sum = p + c + f;
    if (sum <= 0) return [];
    const list = [
      { key: 'protein', label: 'Protein', g: totals.protein ?? 0, share: p / sum, color: 'var(--lilac)' },
      { key: 'fat', label: 'Fat', g: totals.fat ?? 0, share: f / sum, color: 'var(--sea)' },
      { key: 'carbs', label: 'Carbs', g: totals.carbs ?? 0, share: c / sum, color: 'var(--butter)' },
    ].sort((a, b) => a.share - b.share);
    // Rounded shares that still add up to 100.
    const pct = list.map((t) => Math.round(t.share * 100));
    pct[2] += 100 - pct.reduce((a, b) => a + b, 0);
    return list.map((t, i) => ({ ...t, pct: pct[i], width: [50, 75, 100][i], height: Math.round(40 + t.share * 80) }));
  });
</script>

<div class="pyr" data-testid="macro-pyramid">
  {#if tiers.length}
    {#each tiers as t (t.key)}
      <div class="tier" style:width="{t.width}%" style:height="{t.height}px" style:background={t.color}>
        <div class="display p num">{t.pct}%</div>
        <div class="lbl">{t.label} · {num(t.g, 0)} g</div>
      </div>
    {/each}
  {:else}
    <p class="muted small empty">Log food to see where today's calories come from.</p>
  {/if}
</div>

<style>
  .pyr {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: flex-end;
    gap: 4px;
    color: var(--on-pastel);
    min-height: 200px;
  }
  .tier {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 3px;
    min-width: 120px;
  }
  .p {
    font-size: 22px;
  }
  .lbl {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.14em;
    text-transform: uppercase;
  }
  .empty {
    margin: auto 0;
    text-align: center;
  }
</style>
