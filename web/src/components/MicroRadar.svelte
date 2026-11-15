<script lang="ts">
  import { P, octagon, pol, q } from '../lib/geom';
  import type { Nutrients, Target } from '../lib/types';

  /** Eight key micronutrients as % of target on an octagonal radar; the dashed ring is 100%. */
  let { totals, targets, coverage = {} }: { totals: Nutrients; targets: Target[]; coverage?: Record<string, number> } = $props();

  const KEYS: [string, string][] = [
    ['fiber', 'Fibre'],
    ['vitamin_c', 'Vit C'],
    ['iron', 'Iron'],
    ['calcium', 'Calcium'],
    ['potassium', 'Potassium'],
    ['vitamin_d', 'Vit D'],
    ['vitamin_b12', 'B12'],
    ['magnesium', 'Magnesium'],
  ];
  const cx = 135,
    cy = 120,
    R = 80;
  const rings = [0.25, 0.5, 0.75, 1, 1.2].map((f) => ({ pts: octagon(cx, cy, q((f / 1.2) * R)), full: f === 1 }));
  const spokes = Array.from({ length: 8 }, (_, k) => pol(cx, cy, R, -P / 2 + (k * P) / 4));

  const pts = $derived(
    KEYS.map(([key, name], k) => {
      const t = targets.find((x) => x.key === key)?.target;
      const v = totals[key];
      const pct = t && v != null ? (v / t) * 100 : 0;
      const a = -P / 2 + (k * P) / 4;
      const [x, y] = pol(cx, cy, (Math.min(pct, 120) / 120) * R, a);
      const [lx, ly] = pol(cx, cy, R + 18, a);
      const c = Math.cos(a);
      const anchor = Math.abs(c) < 0.2 ? 'middle' : c > 0 ? 'start' : 'end';
      return { key, name, pct: Math.round(pct), known: v != null, x, y, lx, ly, anchor };
    }),
  );
  const poly = $derived(pts.map((p) => `${p.x},${p.y}`).join(' '));
</script>

<svg viewBox="-20 -6 310 252" class="radar" role="img" aria-label="Micronutrients as percent of daily value" data-testid="micro-radar">
  {#each rings as r}
    <polygon points={r.pts} fill="none" stroke={r.full ? 'var(--peach)' : 'var(--track)'} stroke-width="1.25" stroke-dasharray={r.full ? '3 4' : undefined} />
  {/each}
  {#each spokes as [x, y]}
    <line x1={cx} y1={cy} x2={x} y2={y} stroke="var(--track)" stroke-width="1" />
  {/each}
  <polygon points={poly} fill="oklch(0.84 0.08 55 / 0.28)" stroke="var(--peach)" stroke-width="2" />
  {#each pts as p (p.key)}
    <rect x="-3.5" y="-3.5" width="7" height="7" fill="var(--butter)" transform="translate({p.x} {p.y}) rotate(45)" />
    <text x={p.lx} y={p.ly - 2} text-anchor={p.anchor} class="ln">{p.name}</text>
    <text x={p.lx} y={p.ly + 11} text-anchor={p.anchor} class="lv" data-key={p.key}>{p.known ? `${p.pct}%` : '–'}</text>
  {/each}
</svg>

<style>
  .radar {
    display: block;
    width: 100%;
    max-width: 300px;
    height: auto;
    margin: 0 auto;
    overflow: visible;
  }
  .ln {
    font-size: 10.5px;
    fill: var(--muted);
  }
  .lv {
    font-size: 11px;
    font-weight: 700;
    fill: var(--ink);
  }
</style>
