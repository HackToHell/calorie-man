<script lang="ts">
  import { api } from '../lib/api';
  import { addDays, fmtDay, optNum, signed, todayStr, toDisplayWeight, w1 } from '../lib/format';
  import { q } from '../lib/geom';
  import { app } from '../lib/state.svelte';
  import type { Weight } from '../lib/types';
  import Dialog from './Dialog.svelte';

  /** Latest weigh-in, 30-day change and a stepped trend of the last 30 days. */
  const user = $derived(app.user!);
  const wu = $derived(user.weight_unit);
  let list = $state<Weight[]>([]);
  let loaded = $state(false);
  let open = $state(false);
  let value = $state<number | string>('');

  $effect(() => {
    void app.version;
    api
      .weights(user.id, addDays(todayStr(), -30))
      .then((w) => ((list = w), (loaded = true)))
      .catch((e) => app.error(e));
  });

  const latest = $derived(list.at(-1) ?? null);
  const change = $derived(list.length >= 2 && latest?.trend_kg != null && list[0].trend_kg != null ? latest.trend_kg - list[0].trend_kg : null);
  const toward = $derived.by(() => {
    if (change == null || Math.abs(change) < 0.05) return 'flat';
    const g = user.goal_weight_kg;
    if (g == null || latest == null) return 'flat';
    return Math.sign(g - latest.weight_kg) === Math.sign(change) ? 'good' : 'bad';
  });

  // Drawn at the card's real width so text stays 10px; the right strip holds the axis labels (as in the design).
  let boxW = $state(270);
  const W = $derived(Math.max(200, boxW));
  const LW = $derived(W - 34);
  const Hh = 120;
  const chart = $derived.by(() => {
    if (list.length < 2) return null;
    const vals = list.map((w) => toDisplayWeight(w.weight_kg, wu)!);
    let lo = Math.floor(Math.min(...vals) - 0.3),
      hi = Math.ceil(Math.max(...vals) + 0.3);
    if (hi - lo < 2) hi = lo + 2;
    const t0 = new Date(list[0].date).getTime(),
      t1 = Math.max(new Date(todayStr()).getTime(), new Date(list.at(-1)!.date).getTime());
    const X = (d: string) => q(((new Date(d).getTime() - t0) / Math.max(1, t1 - t0)) * LW);
    const Y = (v: number) => q(8 + ((hi - v) / (hi - lo)) * (Hh - 16));
    let d = `M0 ${Y(vals[0])}`;
    for (let i = 1; i < list.length; i++) d += `H${X(list[i].date)}V${Y(vals[i])}`;
    const step = hi - lo > 6 ? Math.ceil((hi - lo) / 3) : 1;
    const grid = [];
    for (let v = hi; v >= lo && grid.length < 4; v -= step) grid.push({ v, y: Y(v) });
    return { line: d, area: `${d}V${Hh}H0Z`, ex: X(list.at(-1)!.date), ey: Y(vals.at(-1)!), grid };
  });

  async function save(e: SubmitEvent) {
    e.preventDefault();
    const v = optNum(value);
    if (v == null || !(v > 0)) return app.toast('Enter a weight', 'error');
    try {
      await api.addWeight(user.id, { date: todayStr(), weight: v, unit: wu });
      app.toast(`Logged ${w1(v)} ${wu}`);
      value = '';
      open = false;
      app.touch();
    } catch (err) {
      app.error(err);
    }
  }
</script>

<div class="wc" data-testid="weight-card" bind:clientWidth={boxW}>
  <div class="card-head">
    <a class="card-title plain" href="#/weight">Weight</a>
    {#if change != null}
      <span class="chg num" class:good={toward === 'good'} class:bad={toward === 'bad'}>{signed(toDisplayWeight(change, wu) ?? 0, 1)} {wu} in 30 days</span>
    {/if}
  </div>
  <div class="row big">
    {#if latest}
      <span class="display n num" data-testid="today-weight">{w1(toDisplayWeight(latest.weight_kg, wu))} <small>{wu}</small></span>
      {#if latest.date !== todayStr()}<span class="tiny muted">{fmtDay(latest.date, { month: 'short', day: 'numeric' })}</span>{/if}
    {:else if loaded}
      <span class="muted small">No weigh-ins in the last 30 days.</span>
    {/if}
    <span class="spacer"></span>
    <button class="btn small" onclick={() => (open = true)} data-testid="log-weight">+ Log</button>
  </div>
  {#if chart}
    <svg width={W} height={Hh} viewBox="0 0 {W} {Hh}" class="spark" role="img" aria-label="Weight over the last 30 days">
      {#each chart.grid as g}
        <line x1="0" x2={W} y1={g.y} y2={g.y} stroke="var(--track)" />
        <text x={W} y={g.y - 4} text-anchor="end" class="gl">{g.v} {wu}</text>
      {/each}
      <path d={chart.area} fill="oklch(0.8 0.08 305 / 0.2)" />
      <path d={chart.line} fill="none" stroke="var(--lilac)" stroke-width="2" />
      <rect x="-5" y="-5" width="10" height="10" fill="var(--lilac)" transform="translate({chart.ex} {chart.ey}) rotate(45)" />
    </svg>
    <div class="xl small muted"><span>{fmtDay(list[0].date, { month: 'short', day: 'numeric' })}</span><span>Today</span></div>
  {/if}
</div>

<Dialog bind:open title="Log weight" testid="weight-dialog">
  <form id="qw" class="stack" onsubmit={save}>
    <label class="field"
      ><span>Today's weight ({wu})</span>
      <input class="input num" type="number" step="0.1" min="0" inputmode="decimal" bind:value aria-label="Weight in {wu}" data-testid="quick-weight" /></label
    >
  </form>
  {#snippet footer()}
    <button class="btn" type="button" onclick={() => (open = false)}>Cancel</button>
    <button class="btn primary" type="submit" form="qw" data-testid="quick-weight-save">Save</button>
  {/snippet}
</Dialog>

<style>
  .wc {
    display: flex;
    flex-direction: column;
    gap: 8px;
    height: 100%;
  }
  .plain {
    color: inherit;
    text-decoration: none;
  }
  .chg {
    font-size: 13px;
    font-weight: 600;
    color: var(--muted);
  }
  .chg.good {
    color: var(--sea-ink);
  }
  .chg.bad {
    color: var(--peach-ink);
  }
  .big {
    align-items: baseline;
  }
  .n {
    font-size: 38px;
  }
  .n small {
    font-family: var(--font);
    font-size: 14px;
    color: var(--muted);
  }
  .spark {
    display: block;
    overflow: visible;
    margin-top: 6px;
  }
  .gl {
    font-size: 10px;
    fill: var(--muted);
  }
  .xl {
    display: flex;
    justify-content: space-between;
  }
</style>
