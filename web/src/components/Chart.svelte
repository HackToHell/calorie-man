<script lang="ts">
  import { fmtDay } from '../lib/format';

  export interface Series {
    type: 'bar' | 'line' | 'step' | 'dots' | 'area';
    values: (number | null)[];
    color: string | ((i: number) => string);
    label: string;
    dash?: string;
    width?: number;
    /** Stack bars of this group on top of each other. */
    stack?: string;
    /** Draw lines straight across missing values instead of breaking them. */
    connect?: boolean;
  }

  let {
    dates,
    series,
    height = 220,
    yMin,
    yMax,
    zero = true,
    yFormat = (v: number) => String(Math.round(v)),
    tooltip,
    testid,
    label,
  }: {
    dates: string[];
    series: Series[];
    height?: number;
    yMin?: number;
    yMax?: number;
    zero?: boolean;
    yFormat?: (v: number) => string;
    tooltip?: (i: number) => string[];
    testid?: string;
    label: string;
  } = $props();

  let width = $state(600);
  let hover = $state<number | null>(null);
  const pad = { l: 44, r: 10, t: 10, b: 24 };

  const n = $derived(dates.length);
  const plotW = $derived(Math.max(10, width - pad.l - pad.r));
  const plotH = $derived(height - pad.t - pad.b);
  const band = $derived(n > 0 ? plotW / n : plotW);

  const stacks = $derived.by(() => {
    const totals = new Map<string, number[]>();
    for (const s of series) {
      if (s.type !== 'bar' || !s.stack) continue;
      const t = totals.get(s.stack) ?? new Array(n).fill(0);
      s.values.forEach((v, i) => (t[i] += v ?? 0));
      totals.set(s.stack, t);
    }
    return totals;
  });

  const range = $derived.by(() => {
    let lo = Infinity,
      hi = -Infinity;
    for (const s of series) for (const v of s.values) if (v != null && Number.isFinite(v)) ((lo = Math.min(lo, v)), (hi = Math.max(hi, v)));
    for (const t of stacks.values()) for (const v of t) hi = Math.max(hi, v);
    if (!Number.isFinite(lo)) ((lo = 0), (hi = 1));
    if (zero) lo = Math.min(0, lo);
    if (yMin != null) lo = yMin;
    if (yMax != null) hi = yMax;
    if (hi - lo < 1e-9) ((hi += 1), (lo -= zero ? 0 : 1));
    const span = hi - lo;
    if (!zero) lo -= span * 0.08;
    hi += span * 0.08;
    return niceRange(lo, hi);
  });

  function niceRange(lo: number, hi: number) {
    const raw = (hi - lo) / 4;
    const mag = 10 ** Math.floor(Math.log10(raw));
    const step = [1, 2, 2.5, 5, 10].map((m) => m * mag).find((s) => raw <= s) ?? raw;
    const a = Math.floor(lo / step) * step;
    const b = Math.ceil(hi / step) * step;
    const ticks = [];
    for (let v = a; v <= b + step / 2; v += step) ticks.push(Math.round(v * 1e6) / 1e6);
    return { lo: a, hi: b, ticks };
  }

  const y = (v: number) => pad.t + plotH - ((v - range.lo) / (range.hi - range.lo)) * plotH;
  const x = (i: number) => pad.l + band * i + band / 2;

  function colorOf(s: Series, i: number) {
    return typeof s.color === 'function' ? s.color(i) : s.color;
  }

  function linePath(values: (number | null)[], step = false, connect = false) {
    let d = '';
    let pen = false;
    values.forEach((v, i) => {
      if (v == null || !Number.isFinite(v)) {
        if (!connect) pen = false;
        return;
      }
      if (step) {
        const x0 = pad.l + band * i,
          x1 = x0 + band;
        d += pen ? `L${x0},${y(v)}L${x1},${y(v)}` : `M${x0},${y(v)}L${x1},${y(v)}`;
      } else {
        d += `${pen ? 'L' : 'M'}${x(i)},${y(v)}`;
      }
      pen = true;
    });
    return d;
  }

  const xLabels = $derived.by(() => {
    if (n === 0) return [];
    const every = Math.max(1, Math.ceil(n / Math.max(2, Math.floor(plotW / 70))));
    const out: number[] = [];
    for (let i = n - 1; i >= 0; i -= every) out.push(i);
    return out;
  });

  function move(e: PointerEvent) {
    const rect = (e.currentTarget as SVGElement).getBoundingClientRect();
    const px = ((e.clientX - rect.left) / rect.width) * width;
    const i = Math.floor((px - pad.l) / band);
    hover = i >= 0 && i < n ? i : null;
  }

  const barW = $derived(Math.max(1, Math.min(28, band * 0.72)));
  const tipLeft = $derived(hover == null ? 0 : Math.min(Math.max(x(hover), 80), width - 80));
</script>

<div class="chart" bind:clientWidth={width} data-testid={testid}>
  <svg
    {width}
    {height}
    viewBox="0 0 {width} {height}"
    role="img"
    aria-label={label}
    onpointermove={move}
    onpointerleave={() => (hover = null)}
  >
    {#each range.ticks as t}
      <line x1={pad.l} x2={width - pad.r} y1={y(t)} y2={y(t)} class="grid" />
      <text x={pad.l - 6} y={y(t)} class="ytick" text-anchor="end" dominant-baseline="middle">{yFormat(t)}</text>
    {/each}
    {#if hover != null}
      <rect x={pad.l + band * hover} y={pad.t} width={band} height={plotH} class="hover" />
    {/if}
    {#each series as s}
      {#if s.type === 'bar'}
        {#each s.values as v, i}
          {#if v != null && v > 0}
            {@const base = s.stack ? series.filter((o) => o.stack === s.stack && series.indexOf(o) < series.indexOf(s)).reduce((a, o) => a + (o.values[i] ?? 0), 0) : 0}
            <rect
              x={x(i) - barW / 2}
              y={y(base + v)}
              width={barW}
              height={Math.max(0, y(base) - y(base + v))}
              fill={colorOf(s, i)}
              class="bar"
            />
          {/if}
        {/each}
      {:else if s.type === 'line' || s.type === 'step'}
        <path d={linePath(s.values, s.type === 'step', s.connect)} fill="none" stroke={colorOf(s, 0)} stroke-width={s.width ?? 2} stroke-dasharray={s.dash} />
      {:else if s.type === 'area'}
        <path d={linePath(s.values)} fill="none" stroke={colorOf(s, 0)} stroke-width={s.width ?? 2} />
      {:else if s.type === 'dots'}
        {#each s.values as v, i}
          {#if v != null}<circle cx={x(i)} cy={y(v)} r={n > 120 ? 1.6 : 2.6} fill={colorOf(s, i)} opacity="0.75" />{/if}
        {/each}
      {/if}
    {/each}
    {#each xLabels as i}
      <text x={x(i)} y={height - 6} class="xtick" text-anchor="middle">{fmtDay(dates[i], { month: 'short', day: 'numeric' })}</text>
    {/each}
  </svg>
  {#if hover != null && tooltip}
    <div class="tip" style:left="{tipLeft}px" role="tooltip">
      <div class="td">{fmtDay(dates[hover], { weekday: 'short', month: 'short', day: 'numeric' })}</div>
      {#each tooltip(hover) as line}<div>{line}</div>{/each}
    </div>
  {/if}
  <div class="legend">
    {#each series as s}
      <span><i style:background={colorOf(s, n - 1)} class:dash={!!s.dash}></i>{s.label}</span>
    {/each}
  </div>
</div>

<style>
  .chart {
    position: relative;
    width: 100%;
    user-select: none;
    touch-action: pan-y;
  }
  svg {
    display: block;
    overflow: visible;
  }
  .grid {
    stroke: var(--line);
    stroke-width: 1;
  }
  .ytick,
  .xtick {
    font-size: 10.5px;
    font-weight: 500;
    fill: var(--muted);
    font-variant-numeric: tabular-nums;
  }
  .hover {
    fill: var(--text);
    opacity: 0.05;
  }
  .tip {
    position: absolute;
    top: 0;
    transform: translateX(-50%);
    background: var(--solid);
    color: var(--solid-ink);
    padding: 6px 9px;
    font-size: 0.78rem;
    pointer-events: none;
    white-space: nowrap;
    z-index: 5;
  }
  .td {
    font-weight: 700;
    margin-bottom: 2px;
  }
  .legend {
    display: flex;
    gap: 12px;
    flex-wrap: wrap;
    font-size: 0.75rem;
    color: var(--muted);
    padding-left: 44px;
  }
  .legend i {
    display: inline-block;
    width: 10px;
    height: 10px;
    margin-right: 4px;
    vertical-align: -1px;
  }
</style>
