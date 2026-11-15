<script lang="ts">
  import { MEAL_COLORS, MEAL_LABELS, fmtDuration, fmtTime, hhmmToMinutes, kcal } from '../lib/format';
  import { P, octagon, pol, q } from '../lib/geom';
  import type { Diary } from '../lib/types';

  /**
   * "Sun path" of the day: meals as diamonds placed by the time they were eaten along an arc from
   * 6 AM to 10 PM, sized by calories. For today it also marks "now" and the next meal's usual time.
   */
  let { diary, live = false, now = new Date() }: { diary: Diary; live?: boolean; now?: Date } = $props();

  let width = $state(600);
  const compact = $derived(width < 480);
  // Geometry from the design: phone artboard and desktop artboard.
  const g = $derived(
    compact
      ? { W: 322, H: 112, cx: 161, cy: 92, rx: 148, ry: 74, s: 0.75, lo: 7, kfs: 13 }
      : { W: 605, H: 184, cx: 302, cy: 158, rx: 282, ry: 130, s: 1, lo: 9, kfs: 18 },
  );

  const nowMin = $derived(now.getHours() * 60 + now.getMinutes());
  const occ = $derived(diary.timing.occasions);
  const ORDER = ['breakfast', 'lunch', 'dinner'];
  const ghost = $derived.by(() => {
    if (!live) return null;
    for (const meal of ORDER) {
      const u = hhmmToMinutes(diary.timing.usual[meal]);
      // Still shown for 90 minutes after the usual time: an overdue dinner is when the hint helps most.
      if (u != null && u > nowMin - 90 && !occ.some((o) => o.meal === meal)) return { meal, t: u, time: diary.timing.usual[meal] };
    }
    return null;
  });

  // Time domain in minutes; widened when meals or "now" fall outside 6 AM - 10 PM.
  const dom = $derived.by(() => {
    const pts = [...occ.map((o) => hhmmToMinutes(o.time) ?? 720), ...(live ? [nowMin] : [])];
    const lo = Math.min(360, ...pts.map((m) => Math.floor(m / 60) * 60));
    const hi = Math.max(1320, ...pts.map((m) => Math.ceil(m / 60) * 60));
    return { lo, hi };
  });
  const pt = (m: number) => pol(g.cx, g.cy, g.rx, P + ((m - dom.lo) / (dom.hi - dom.lo)) * P, g.ry);
  const size = (k: number) => q(g.s * Math.min(28, 6 + k / 50));
  const band = (f: number) => `M${q(g.cx - g.rx * f)} ${g.cy}A${q(g.rx * f)} ${q(g.ry * f)} 0 0 1 ${q(g.cx + g.rx * f)} ${g.cy}Z`;
  const arcTo = (a: number, b: number) => {
    const [x0, y0] = pt(a),
      [x1, y1] = pt(b);
    return `M${x0} ${y0}A${g.rx} ${g.ry} 0 0 1 ${x1} ${y1}`;
  };

  const marks = $derived.by(() => {
    let prevX = -999;
    return occ.map((o, i) => {
      const m = hhmmToMinutes(o.time) ?? 720;
      const [x, y] = pt(m);
      const sz = size(o.calories);
      const hd = q(sz * 0.71);
      const crowded = x - prevX < (compact ? 34 : 70);
      prevX = x;
      const frac = (m - dom.lo) / (dom.hi - dom.lo);
      const side: 'right' | 'left' | 'below' = frac < 0.12 ? 'right' : frac > 0.88 ? 'left' : 'below';
      return { ...o, i, x, y, sz, hd, crowded, side, color: MEAL_COLORS[o.meal] ?? 'var(--sea)' };
    });
  });
  const nowPt = $derived(live ? pt(Math.min(Math.max(nowMin, dom.lo), dom.hi)) : null);
  const ghostPt = $derived(ghost ? pt(ghost.t) : null);

  const windowText = $derived.by(() => {
    const f = diary.timing.first;
    if (!f) return live ? 'No meals with a time yet' : 'No times logged';
    const fm = hhmmToMinutes(f)!;
    if (live && nowMin >= fm) return `Eating window ${fmtTime(f)} – now · ${fmtDuration(nowMin - fm)}`;
    const l = diary.timing.last!;
    return `Eating window ${fmtTime(f)} – ${fmtTime(l)} · ${fmtDuration((hhmmToMinutes(l) ?? fm) - fm)}`;
  });
  const nowLabel = $derived(fmtTime(`${now.getHours()}:${now.getMinutes()}`, false));
  const axis = $derived(
    compact
      ? [dom.lo, dom.hi]
      : [dom.lo, (dom.lo + dom.hi) / 2, dom.hi].map((m) => Math.round(m / 60) * 60),
  );
  const axisLabel = (m: number) => {
    const h = Math.floor(m / 60) % 24;
    return fmtTime(`${h}:00`).replace(':00', '');
  };
  const summary = $derived(occ.map((o) => `${MEAL_LABELS[o.meal]} at ${fmtTime(o.time)}, ${kcal(o.calories)} kcal`).join('; ') || 'No meals with a time');
</script>

<div class="mt" bind:clientWidth={width} data-testid="meal-timing">
  <div class="head">
    {#if compact}
      <span class="eyebrow">Meal timing</span>
      <span class="card-sub">{live ? `Now ${fmtTime(`${now.getHours()}:${now.getMinutes()}`)}` : windowText}</span>
    {:else}
      <span class="card-title">Meal timing</span>
      <span class="card-sub" data-testid="eating-window">{windowText}</span>
    {/if}
  </div>
  <svg viewBox="0 0 {g.W} {g.H}" role="img" aria-label="Meal timing: {summary}">
    {#each [1, 0.68, 0.36] as f, i}<path d={band(f)} class="band b{i}" />{/each}
    <line x1="0" y1={g.cy} x2={g.W} y2={g.cy} class="base" />
    {#if live && nowPt}
      <path d={arcTo(dom.lo, Math.min(Math.max(nowMin, dom.lo), dom.hi))} class="past" />
      <path d={arcTo(Math.min(Math.max(nowMin, dom.lo), dom.hi), dom.hi)} class="future" />
    {:else}
      <path d={arcTo(dom.lo, dom.hi)} class="past dim" />
    {/if}
    {#if ghost && ghostPt}
      {@const gs = q(g.s * 16)}
      <rect x={-gs / 2} y={-gs / 2} width={gs} height={gs} class="ghost" transform="translate({ghostPt[0]} {ghostPt[1]}) rotate(45)" />
      {#if !compact}
        <text x={ghostPt[0] - gs * 0.71 - 10} y={ghostPt[1] + 4} text-anchor="end" class="lbl" data-testid="usual-next"
          >{MEAL_LABELS[ghost.meal]} · usually {fmtTime(ghost.time, false)}</text
        >
      {/if}
    {/if}
    {#each marks as m (m.i)}
      <g class="meal" data-testid="timing-mark">
        <title>{MEAL_LABELS[m.meal]} · {fmtTime(m.time)} · {kcal(m.calories)} kcal · {m.entries} item{m.entries === 1 ? '' : 's'}</title>
        <rect x={-m.sz / 2} y={-m.sz / 2} width={m.sz} height={m.sz} fill={m.color} class="dia" transform="translate({m.x} {m.y}) rotate(45)" />
        <text x={m.x} y={m.y - m.hd - g.lo - (m.crowded ? g.kfs : 0)} text-anchor="middle" class="k" style:font-size="{g.kfs}px">{kcal(m.calories)}</text>
        {#if !compact && !m.crowded}
          {#if m.side === 'right'}
            <text x={m.x + m.hd + 8} y={m.y + 4} class="lbl">{MEAL_LABELS[m.meal]} · {fmtTime(m.time, false)}</text>
          {:else if m.side === 'left'}
            <text x={m.x - m.hd - 8} y={m.y + 4} text-anchor="end" class="lbl">{MEAL_LABELS[m.meal]} · {fmtTime(m.time, false)}</text>
          {:else}
            <text x={m.x} y={m.y + m.hd + 15} text-anchor="middle" class="lbl">{MEAL_LABELS[m.meal]} · {fmtTime(m.time, false)}</text>
          {/if}
        {/if}
      </g>
    {/each}
    {#if live && nowPt}
      <polygon points={octagon(nowPt[0], nowPt[1], q(g.s * 12), P / 8)} class="ring" />
      <polygon points={octagon(nowPt[0], nowPt[1], q(g.s * 6), P / 8)} class="now" />
      {#if !compact}<text x={nowPt[0]} y={nowPt[1] - g.s * 12 - 9} text-anchor="middle" class="nowl">Now {nowLabel}</text>{/if}
    {/if}
    {#each axis as m, i}
      <text
        x={i === 0 ? (compact ? 0 : 20) : i === axis.length - 1 ? (compact ? g.W : g.W - 20) : g.cx}
        y={g.H - 4}
        text-anchor={compact ? (i === 0 ? 'start' : 'end') : 'middle'}
        class="ax">{axisLabel(m)}</text
      >
    {/each}
  </svg>
  {#if compact}
    <div class="legend">
      {#each ['breakfast', 'lunch', 'dinner', 'snacks'].filter((k) => occ.some((o) => o.meal === k)) as k}
        <span><i class="diamond" style:background={MEAL_COLORS[k]}></i>{MEAL_LABELS[k]}</span>
      {/each}
      {#if ghost}<span data-testid="usual-next"><i class="diamond dash"></i>{MEAL_LABELS[ghost.meal]} · {fmtTime(ghost.time, false)}?</span>{/if}
    </div>
  {/if}
  {#if diary.timing.untimed > 0}
    <p class="tiny muted">{diary.timing.untimed} item{diary.timing.untimed === 1 ? ' has' : 's have'} no time; tap {diary.timing.untimed === 1 ? 'it' : 'one'} in the meal list to add it.</p>
  {/if}
</div>

<style>
  .mt {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 10px;
  }
  svg {
    display: block;
    width: 100%;
    max-width: 680px;
    height: auto;
    overflow: visible;
    margin: 0 auto;
  }
  .band.b0 {
    fill: var(--band-1);
  }
  .band.b1 {
    fill: var(--band-2);
  }
  .band.b2 {
    fill: var(--band-3);
  }
  .base {
    stroke: var(--track);
    stroke-width: 1;
  }
  .past {
    fill: none;
    stroke: var(--arc);
    stroke-width: 1.5;
  }
  .past.dim {
    opacity: 0.6;
  }
  .future {
    fill: none;
    stroke: var(--arc);
    stroke-width: 1.5;
    stroke-dasharray: 3 5;
    opacity: 0.6;
  }
  .dia {
    stroke: var(--surface);
    stroke-width: 2.5;
  }
  .ghost {
    fill: var(--surface);
    stroke: var(--arc);
    stroke-width: 1.5;
    stroke-dasharray: 3 3;
  }
  .k {
    font-family: var(--display);
    fill: var(--ink);
  }
  .lbl {
    font-size: 12px;
    fill: var(--muted);
  }
  .ring {
    fill: oklch(0.84 0.08 55 / 0.22);
  }
  .now {
    fill: var(--peach);
  }
  .nowl {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    fill: var(--peach-ink);
  }
  .ax {
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.1em;
    fill: var(--muted);
  }
  .legend {
    display: flex;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 6px 10px;
  }
  .legend span {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    font-weight: 600;
  }
  .dash {
    border: 1.25px dashed var(--ink);
    box-sizing: border-box;
  }
</style>
