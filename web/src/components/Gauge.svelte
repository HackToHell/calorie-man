<script lang="ts">
  import { kcal, num } from '../lib/format';
  import { P, arc, clamp01 } from '../lib/geom';
  import type { Diary } from '../lib/types';

  /** Concentric half-ring gauge: calories outside, then protein, carbs, fat and fibre. */
  let { diary, layout = 'list' }: { diary: Diary; layout?: 'list' | 'grid' } = $props();

  const tgt = (k: string) => diary.targets.find((t) => t.key === k)?.target ?? null;
  const b = $derived(diary.budget);
  const eaten = $derived(b.eaten);
  const budget = $derived(b.budget);
  const over = $derived(budget != null && eaten > budget + 0.5);

  const rows = $derived([
    { key: 'calories', label: 'Calories', color: 'var(--peach)', v: eaten, t: budget, unit: '' },
    { key: 'protein', label: 'Protein', color: 'var(--lilac)', v: diary.totals.protein ?? 0, t: tgt('protein'), unit: ' g' },
    { key: 'carbs', label: 'Carbs', color: 'var(--butter)', v: diary.totals.carbs ?? 0, t: tgt('carbs'), unit: ' g' },
    { key: 'fat', label: 'Fat', color: 'var(--sea)', v: diary.totals.fat ?? 0, t: tgt('fat'), unit: ' g' },
    { key: 'fiber', label: 'Fibre', color: 'var(--sky)', v: diary.totals.fiber ?? 0, t: tgt('fiber'), unit: ' g' },
  ]);
  // Geometry from the phone artboard: centre (161,166), outer band 20 wide, inner bands 12 wide.
  const spec: [number, number][] = [
    [148, 20],
    [124, 12],
    [108, 12],
    [92, 12],
    [76, 12],
  ];
  const bands = $derived(
    rows.map((r, i) => {
      const [rad, sw] = spec[i];
      const p = r.t ? clamp01(r.v / r.t) : 0;
      return { ...r, sw, track: arc(161, 166, rad, P, 2 * P), prog: p > 0.002 ? arc(161, 166, rad, P, P + p * P) : '' };
    }),
  );
  const fmt = (r: (typeof rows)[number], v: number) => (r.key === 'calories' ? kcal(v) : num(v, 0));
</script>

<div class="gauge" data-testid="gauge">
  <div class="dial">
    <svg viewBox="0 0 322 172" role="img" aria-label="Calories and macros against today's targets">
      {#each bands as band (band.key)}
        <path d={band.track} fill="none" stroke="var(--track)" stroke-width={band.sw} />
        {#if band.prog}<path d={band.prog} fill="none" stroke={band.key === 'calories' && over ? 'var(--over)' : band.color} stroke-width={band.sw} />{/if}
      {/each}
    </svg>
    <div class="center">
      {#if budget != null}
        <div class="display big num" style:--chars={kcal(Math.abs(b.remaining ?? 0)).length} data-testid="calories-remaining">{kcal(Math.abs(b.remaining ?? 0))}</div>
        <div class="eyebrow cap">{over ? 'kcal over' : 'kcal left'}</div>
      {:else}
        <div class="display big num" style:--chars={kcal(eaten).length} data-testid="calories-eaten-center">{kcal(eaten)}</div>
        <div class="eyebrow cap">kcal eaten</div>
      {/if}
    </div>
  </div>
  <div class="legend {layout}">
    {#each rows as r (r.key)}
      <div class="li" data-testid="legend-{r.key}">
        <span class="swatch" style:background={r.color}></span>
        <span class="ln">{r.label}</span>
        <span class="lv num"
          ><span data-testid={r.key === 'calories' ? 'calories-eaten' : r.key === 'protein' ? 'protein-eaten' : undefined}>{fmt(r, r.v)}</span
          >{r.t != null ? ` / ${fmt(r, r.t)}` : ''}{r.unit}</span
        >
      </div>
    {/each}
  </div>
</div>

<style>
  .gauge {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .dial {
    position: relative;
    width: 100%;
    max-width: 322px;
    align-self: center;
    container-type: inline-size;
  }
  svg {
    display: block;
    width: 100%;
    height: auto;
  }
  .center {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
  }
  /* The number has to fit inside the innermost ring: ~12% of the dial for 3 digits, smaller for more. */
  .big {
    font-size: min(40px, calc(36cqw / max(3, var(--chars, 3))));
  }
  .cap {
    font-size: 10px;
    letter-spacing: 0.18em;
  }
  .legend {
    border-top: 1px solid var(--line);
    padding-top: 12px;
  }
  .legend.list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .legend.list .li {
    display: flex;
    align-items: center;
    gap: 9px;
    font-size: 13px;
    white-space: nowrap;
  }
  .legend.list .ln {
    flex: 1;
    font-weight: 600;
  }
  .legend.list .lv {
    color: var(--muted);
  }
  .legend.grid {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 8px 12px;
  }
  .legend.grid .li {
    display: grid;
    grid-template-columns: 10px 1fr;
    column-gap: 8px;
    align-items: center;
  }
  .legend.grid .ln {
    font-size: 13px;
    font-weight: 600;
  }
  .legend.grid .lv {
    grid-column: 2;
    font-size: 12px;
    color: var(--muted);
  }
</style>
