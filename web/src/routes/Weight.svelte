<script lang="ts">
  import Chart from '../components/Chart.svelte';
  import PageHeader from '../components/PageHeader.svelte';
  import Dialog from '../components/Dialog.svelte';
  import { api } from '../lib/api';
  import { addDays, daysBetween, fmtDay, isValidDate, num, optNum, relDay, signed, todayStr, toDisplayWeight, txt, w1 } from '../lib/format';
  import { app } from '../lib/state.svelte';
  import type { Weight } from '../lib/types';

  const user = $derived(app.user!);
  const wu = $derived(user.weight_unit);

  let all = $state<Weight[]>([]);
  let loaded = $state(false);
  let date = $state(todayStr());
  let weight = $state('');
  let bf = $state('');
  let note = $state('');
  let busy = $state(false);
  let span = $state(90);
  let editing = $state<Weight | null>(null);
  let editOpen = $state(false);
  let eDate = $state('');
  let eWeight = $state('');
  let eBf = $state('');
  let eNote = $state('');

  $effect(() => {
    void app.version;
    api
      .weights(user.id)
      .then((w) => ((all = w), (loaded = true)))
      .catch((e) => app.error(e));
  });

  const latest = $derived(all.at(-1) ?? null);
  const trendAt = (daysAgo: number) => {
    const cutoff = addDays(todayStr(), -daysAgo);
    const w = [...all].reverse().find((x) => x.date <= cutoff);
    return w?.trend_kg ?? null;
  };
  const change7 = $derived(latest?.trend_kg != null && trendAt(7) != null ? latest.trend_kg - trendAt(7)! : null);
  const change30 = $derived(latest?.trend_kg != null && trendAt(30) != null ? latest.trend_kg - trendAt(30)! : null);
  const rate = $derived.by(() => {
    if (!latest?.trend_kg) return null;
    const past = [...all].reverse().find((x) => daysBetween(x.date, latest.date) >= 21);
    if (!past?.trend_kg) return null;
    return ((latest.trend_kg - past.trend_kg) / daysBetween(past.date, latest.date)) * 7;
  });
  const eta = $derived.by(() => {
    const g = user.goal_weight_kg;
    if (g == null || rate == null || latest?.trend_kg == null) return null;
    const diff = g - latest.trend_kg;
    if (Math.abs(diff) < 0.2) return 'at goal';
    if (Math.sign(diff) !== Math.sign(rate) || Math.abs(rate) < 0.01) return 'not heading there yet';
    const weeks = diff / rate;
    if (weeks > 260) return 'more than 5 years away';
    return `around ${fmtDay(addDays(todayStr(), Math.round(weeks * 7)), { month: 'short', day: 'numeric', year: 'numeric' })}`;
  });

  const chartDays = $derived.by(() => {
    if (!all.length) return [] as string[];
    const end = todayStr();
    const rangeStart = addDays(end, -(span - 1));
    const start = span === 0 || all[0].date > rangeStart ? all[0].date : rangeStart;
    const out: string[] = [];
    for (let d = start; d <= end; d = addDays(d, 1)) out.push(d);
    return out;
  });
  const byDate = $derived(new Map(all.map((w) => [w.date, w])));
  // The trend is defined at each weigh-in; the chart joins those points.
  const trendSeries = $derived(chartDays.map((d) => toDisplayWeight(byDate.get(d)?.trend_kg, wu)));

  const sameYear = (d: string) => d.slice(0, 4) === todayStr().slice(0, 4);

  async function save(e: SubmitEvent) {
    e.preventDefault();
    const w = Number(weight);
    if (!Number.isFinite(w) || w <= 0) return app.toast('Enter a weight', 'error');
    busy = true;
    try {
      await api.addWeight(user.id, { date, weight: w, unit: wu, body_fat_pct: optNum(bf), note: txt(note) || null });
      app.toast(`Logged ${w1(w)} ${wu} for ${relDay(date).toLowerCase()}`);
      weight = bf = note = '';
      app.touch();
    } catch (err) {
      app.error(err);
    } finally {
      busy = false;
    }
  }

  function openEdit(w: Weight) {
    editing = w;
    eDate = w.date;
    eWeight = String(Math.round((toDisplayWeight(w.weight_kg, wu) ?? 0) * 10) / 10);
    eBf = w.body_fat_pct != null ? String(w.body_fat_pct) : '';
    eNote = w.note ?? '';
    editOpen = true;
  }

  async function saveEdit(e: SubmitEvent) {
    e.preventDefault();
    if (!editing) return;
    if (!isValidDate(eDate)) return app.toast('Pick a valid date', 'error');
    try {
      await api.updateWeight(editing.id, { date: eDate, weight: Number(eWeight), unit: wu, body_fat_pct: optNum(eBf), note: txt(eNote) || null });
      editOpen = false;
      app.touch();
    } catch (err) {
      app.error(err);
    }
  }

  async function remove() {
    if (!editing) return;
    try {
      await api.deleteWeight(editing.id);
      editOpen = false;
      app.toast('Weigh-in deleted');
      app.touch();
    } catch (err) {
      app.error(err);
    }
  }
</script>

<PageHeader title="Weight" eyebrow={latest ? `Trend ${w1(toDisplayWeight(latest.trend_kg, wu))} ${wu}` : 'Weigh-ins'} />

<div class="top">
  <form class="card stack" onsubmit={save} data-testid="weight-form">
    <h2>Log a weigh-in</h2>
    <div class="grid2">
      <label class="field"><span>Date</span><input class="input" type="date" bind:value={date} max={todayStr()} required /></label>
      <label class="field"
        ><span>Weight ({wu})</span><input
          class="input num"
          type="number"
          step="0.1"
          min="0"
          inputmode="decimal"
          bind:value={weight}
          required
          data-testid="weight-input"
        /></label
      >
      <label class="field"><span>Body fat % (optional)</span><input class="input num" type="number" step="0.1" min="0" bind:value={bf} /></label>
      <label class="field"><span>Note</span><input class="input" bind:value={note} maxlength="200" /></label>
    </div>
    <button class="btn primary" disabled={busy} data-testid="weight-save">Save</button>
    <p class="tiny muted">One weigh-in per day; saving again for the same day replaces it. Weigh at the same time each day for a cleaner trend.</p>
  </form>

  <div class="card stats" data-testid="weight-stats">
    <div>
      <div class="sl">Trend weight</div>
      <div class="sv num" data-testid="trend-weight">{latest?.trend_kg != null ? w1(toDisplayWeight(latest.trend_kg, wu)) : '–'} <small>{wu}</small></div>
      {#if latest}<div class="ss">last weigh-in {w1(toDisplayWeight(latest.weight_kg, wu))} {wu}, {relDay(latest.date).toLowerCase()}</div>{/if}
    </div>
    <div class="grid2">
      <div>
        <div class="sl">7 days</div>
        <div class="sv2 num">{change7 != null ? `${signed(toDisplayWeight(change7, wu) ?? 0, 1)} ${wu}` : '–'}</div>
      </div>
      <div>
        <div class="sl">30 days</div>
        <div class="sv2 num">{change30 != null ? `${signed(toDisplayWeight(change30, wu) ?? 0, 1)} ${wu}` : '–'}</div>
      </div>
      <div>
        <div class="sl">Rate</div>
        <div class="sv2 num">{rate != null ? `${signed(toDisplayWeight(rate, wu) ?? 0, 2)} ${wu}/wk` : '–'}</div>
      </div>
      <div>
        <div class="sl">Goal</div>
        <div class="sv2 num">{user.goal_weight_kg != null ? `${w1(toDisplayWeight(user.goal_weight_kg, wu))} ${wu}` : '–'}</div>
        {#if eta}<div class="ss">{eta}</div>{:else if user.goal_weight_kg == null}<div class="ss"><a href="#/settings">set a goal</a></div>{/if}
      </div>
    </div>
  </div>
</div>

{#if all.length}
  <div class="card">
    <div class="card-head">
      <h2>History</h2>
      <div class="tabs">
        {#each [[30, '30d'], [90, '90d'], [365, '1y'], [0, 'All']] as [s, l]}
          <button class="tab" class:active={span === s} onclick={() => (span = s as number)}>{l}</button>
        {/each}
      </div>
    </div>
    <Chart
      label="Weight history with trend"
      testid="chart-weight-history"
      dates={chartDays}
      zero={false}
      height={240}
      yFormat={(v) => v.toFixed(1)}
      series={[
        { type: 'dots', values: chartDays.map((d) => toDisplayWeight(byDate.get(d)?.weight_kg, wu)), label: 'Weigh-in', color: 'var(--weight)' },
        { type: 'line', values: trendSeries, label: 'Trend', color: 'var(--peach-ink)', width: 2.5, connect: true },
        ...(user.goal_weight_kg ? [{ type: 'line' as const, values: chartDays.map(() => toDisplayWeight(user.goal_weight_kg, wu)), label: 'Goal', color: 'var(--sea-ink)', dash: '5 4', width: 1.5 }] : []),
      ]}
      tooltip={(i) => {
        const w = byDate.get(chartDays[i]);
        return [w ? `Weighed ${w1(toDisplayWeight(w.weight_kg, wu))} ${wu}` : 'No weigh-in', trendSeries[i] != null ? `Trend ${w1(trendSeries[i])} ${wu}` : ''].filter(Boolean);
      }}
    />
  </div>

  <div class="card">
    <table class="table" data-testid="weight-table">
      <thead><tr><th>Date</th><th class="r">Weight</th><th class="r">Trend</th><th class="r hide-sm">Body fat</th><th class="hide-sm">Note</th><th></th></tr></thead>
      <tbody>
        {#each [...all].reverse().slice(0, 120) as w (w.id)}
          <tr>
            <td class="nowrap">{fmtDay(w.date, sameYear(w.date) ? { weekday: 'short', month: 'short', day: 'numeric' } : { year: 'numeric', month: 'short', day: 'numeric' })}</td>
            <td class="r num nowrap"><b>{w1(toDisplayWeight(w.weight_kg, wu))}</b> {wu}</td>
            <td class="r num muted">{w1(toDisplayWeight(w.trend_kg, wu))}</td>
            <td class="r num hide-sm">{w.body_fat_pct != null ? `${w.body_fat_pct}%` : ''}</td>
            <td class="small muted hide-sm">{w.note ?? ''}</td>
            <td class="r"><button class="btn ghost small" onclick={() => openEdit(w)}>Edit</button></td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
{:else if loaded}
  <div class="card empty">No weigh-ins yet. Log one above; the trend line smooths out day-to-day water swings.</div>
{/if}

<Dialog bind:open={editOpen} title="Edit weigh-in" testid="weight-edit-dialog">
  <form id="wedit" class="stack" onsubmit={saveEdit}>
    <div class="grid2">
      <label class="field"><span>Date</span><input class="input" type="date" bind:value={eDate} required /></label>
      <label class="field"><span>Weight ({wu})</span><input class="input num" type="number" step="0.1" bind:value={eWeight} required /></label>
      <label class="field"><span>Body fat %</span><input class="input num" type="number" step="0.1" bind:value={eBf} /></label>
      <label class="field"><span>Note</span><input class="input" bind:value={eNote} /></label>
    </div>
  </form>
  {#snippet footer()}
    <button class="btn danger" type="button" onclick={remove}>Delete</button>
    <span class="spacer"></span>
    <button class="btn" type="button" onclick={() => (editOpen = false)}>Cancel</button>
    <button class="btn primary" type="submit" form="wedit">Save</button>
  {/snippet}
</Dialog>

<style>
  .top {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    gap: 14px;
    margin-bottom: 14px;
  }
  .top .card + .card {
    margin-top: 0;
  }
  @media (max-width: 760px) {
    .top {
      grid-template-columns: minmax(0, 1fr);
    }
  }
  .stats {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .sl {
    font-size: 0.78rem;
    color: var(--muted);
    font-weight: 600;
  }
  .sv {
    font-family: var(--display);
    font-size: 2.4rem;
    line-height: 1.1;
  }
  .sv small {
    font-family: var(--font);
    font-size: 0.9rem;
    color: var(--muted);
  }
  .sv2 {
    font-size: 1.1rem;
    font-weight: 700;
  }
  .ss {
    font-size: 0.78rem;
    color: var(--muted);
  }
</style>
