<script lang="ts">
  import Chart from '../components/Chart.svelte';
  import PageHeader from '../components/PageHeader.svelte';
  import Progress from '../components/Progress.svelte';
  import { api } from '../lib/api';
  import { addDays, fmtDay, kcal, num, signed, todayStr, toDisplayWeight, w1 } from '../lib/format';
  import { app, navigate } from '../lib/state.svelte';
  import type { Summary } from '../lib/types';

  const user = $derived(app.user!);
  const ranges: [string, string, number][] = [
    ['7', '7 days', 7],
    ['30', '30 days', 30],
    ['90', '90 days', 90],
    ['180', '6 months', 180],
    ['365', '1 year', 365],
  ];
  const range = $derived(ranges.find((r) => r[0] === app.route.params.get('range')) ?? ranges[1]);
  const to = $derived(todayStr());
  const from = $derived(addDays(to, -(range[2] - 1)));

  let data = $state<Summary | null>(null);
  let error = $state<string | null>(null);
  let seq = 0;

  $effect(() => {
    void app.version;
    const [uid, f, t] = [user.id, from, to];
    const my = ++seq;
    api
      .summary(uid, f, t)
      .then((s) => {
        if (my === seq) ((data = s), (error = null));
      })
      .catch((e) => my === seq && (error = (e as Error).message));
  });

  const days = $derived(data?.days ?? []);
  const dates = $derived(days.map((d) => d.date));
  const st = $derived(data?.stats);
  const wu = $derived(user.weight_unit);
  const cal = $derived(days.map((d) => (d.logged ? (d.nutrients.calories ?? 0) : null)));
  const avg7 = $derived(
    cal.map((_, i) => {
      const win = cal.slice(Math.max(0, i - 6), i + 1).filter((v): v is number => v != null);
      return win.length >= 3 ? win.reduce((a, b) => a + b, 0) / win.length : null;
    }),
  );
  const cumulative = $derived.by(() => {
    let acc = 0;
    return days.map((d) => {
      if (d.logged && d.budget != null && d.date !== to) acc += d.budget - (d.nutrients.calories ?? 0);
      return d.budget != null ? acc : null;
    });
  });
  const hasWeights = $derived(days.some((d) => d.weight_kg != null));
  const hasTargets = $derived(days.some((d) => d.protein_target != null));
  const micros = $derived((data?.micros ?? []).filter((m) => !['protein', 'carbs', 'fat', 'fiber'].includes(m.key)));
</script>

<PageHeader title="Trends" eyebrow={data ? `${fmtDay(data.from, { month: 'short', day: 'numeric' })} – today` : 'Over time'} />
<div class="tabs range" role="tablist" aria-label="Range">
  {#each ranges as r}
    <button class="tab" class:active={r === range} role="tab" aria-selected={r === range} onclick={() => navigate(`trends?range=${r[0]}`)}>{r[1]}</button>
  {/each}
</div>

{#if error}<div class="banner error">{error}</div>{/if}

{#if data && st}
  {#if st.logged_days === 0}
    <div class="card empty">No completed days logged in this range yet. Log some food on the Today page and come back.</div>
  {/if}
  <div class="stats" data-testid="trend-stats">
    <div class="card stat">
      <div class="sl">Average intake</div>
      <div class="sv num" data-testid="avg-calories">{kcal(st.avg_calories)}<small>{' kcal'}</small></div>
      <div class="ss">
        {#if st.avg_budget != null}budget {kcal(st.avg_budget)} · within on {st.within_budget_days}/{st.budget_days} days{:else}{st.logged_days} of {st.days} days logged{/if}
      </div>
    </div>
    <div class="card stat">
      <div class="sl">Calorie balance</div>
      <div class="sv num" class:pos={st.balance >= 0} class:neg={st.balance < 0} data-testid="balance">{signed(st.balance)}<small>{' kcal'}</small></div>
      <div class="ss">{st.balance >= 0 ? 'under' : 'over'} budget overall ≈ {num(Math.abs(st.balance) / 7700, 2)} kg of body fat</div>
    </div>
    <div class="card stat">
      <div class="sl">Protein</div>
      <div class="sv num" style:color="var(--protein-text)" data-testid="avg-protein">{num(st.avg_protein, 0)}<small>{' g/day'}</small></div>
      <div class="ss">
        {#if st.protein_hit_rate != null}<span data-testid="protein-hit-rate">hit target {Math.round(st.protein_hit_rate * 100)}%</span> ({st.protein_hit_days}/{st.protein_target_days} days)
          · streak {st.protein_streak}{:else}no protein target set{/if}
        {#if st.avg_protein_per_kg}· {num(st.avg_protein_per_kg, 1)} g/kg{/if}
      </div>
    </div>
    <div class="card stat">
      <div class="sl">Weight trend</div>
      {#if st.weight_change_kg != null}
        <div class="sv num" data-testid="weight-change">{signed(toDisplayWeight(st.weight_change_kg, wu) ?? 0, 1)}<small>{' '}{wu}</small></div>
        <div class="ss">
          {w1(toDisplayWeight(st.weight_end_kg, wu))} {wu} now{#if st.weight_rate_kg_per_week != null}
            · {signed(toDisplayWeight(st.weight_rate_kg_per_week, wu) ?? 0, 2)} {wu}/week{/if}
        </div>
      {:else}
        <div class="sv muted">–</div>
        <div class="ss">no weigh-ins in range</div>
      {/if}
    </div>
    <div class="card stat">
      <div class="sl">Estimated maintenance</div>
      <div class="sv num" data-testid="tdee">{st.estimated_tdee != null ? kcal(st.estimated_tdee) : '–'}<small>{' kcal'}</small></div>
      <div class="ss">{st.estimated_tdee_note ?? 'Needs 10+ logged days and weigh-ins at least 2 weeks apart.'}</div>
    </div>
    <div class="card stat">
      <div class="sl">Macro split (of calories)</div>
      {#if st.protein_pct_kcal != null}
        <div class="split">
          <span style:width="{st.protein_pct_kcal}%" style:background="var(--protein)"></span>
          <span style:width="{st.carbs_pct_kcal}%" style:background="var(--carbs)"></span>
          <span style:width="{st.fat_pct_kcal}%" style:background="var(--fat)"></span>
        </div>
        <div class="ss">
          <b style:color="var(--protein-text)">P {Math.round(st.protein_pct_kcal)}%</b> ·
          <b style:color="var(--carbs-text)">C {Math.round(st.carbs_pct_kcal ?? 0)}%</b> ·
          <b style:color="var(--fat-text)">F {Math.round(st.fat_pct_kcal ?? 0)}%</b>
        </div>
      {:else}<div class="sv muted">–</div>{/if}
    </div>
  </div>

  <div class="card">
    <div class="card-head"><h2>Calories vs budget</h2></div>
    <Chart
      label="Calories per day compared to the budget"
      testid="chart-calories"
      {dates}
      series={[
        {
          type: 'bar',
          values: cal,
          label: 'Eaten',
          color: (i) => (days[i]?.budget != null && (cal[i] ?? 0) > (days[i].budget ?? 0) ? 'var(--peach)' : 'var(--sea)'),
        },
        { type: 'step', values: days.map((d) => d.budget), label: 'Budget', color: 'var(--butter-ink)', dash: '4 3', width: 1.5 },
        { type: 'line', values: avg7, label: '7-day average', color: 'var(--lilac-ink)', width: 2 },
      ]}
      tooltip={(i) => [
        days[i].logged ? `Eaten ${kcal(cal[i])} kcal` : 'Not logged',
        days[i].budget != null ? `Budget ${kcal(days[i].budget)}` : '',
        days[i].exercise ? `Exercise ${kcal(days[i].exercise)}` : '',
        avg7[i] != null ? `7-day avg ${kcal(avg7[i])}` : '',
      ].filter(Boolean)}
    />
  </div>

  <div class="card">
    <div class="card-head"><h2>Protein</h2></div>
    <Chart
      label="Protein per day compared to the target"
      testid="chart-protein"
      {dates}
      series={[
        {
          type: 'bar',
          values: days.map((d) => (d.logged ? (d.nutrients.protein ?? 0) : null)),
          label: 'Protein (g)',
          color: (i) => (days[i]?.protein_target != null && (days[i].nutrients.protein ?? 0) + 0.5 >= (days[i].protein_target ?? 0) ? 'var(--lilac)' : 'color-mix(in srgb, var(--lilac) 40%, transparent)'),
        },
        ...(hasTargets ? [{ type: 'step' as const, values: days.map((d) => d.protein_target), label: 'Target', color: 'var(--butter-ink)', dash: '4 3', width: 1.5 }] : []),
      ]}
      tooltip={(i) => [
        days[i].logged ? `${num(days[i].nutrients.protein ?? 0, 0)} g protein` : 'Not logged',
        days[i].protein_target != null ? `Target ${num(days[i].protein_target, 0)} g` : '',
      ].filter(Boolean)}
    />
  </div>

  {#if hasWeights}
    <div class="card">
      <div class="card-head"><h2>Weight</h2><a class="small" href="#/weight">Weight log →</a></div>
      <Chart
        label="Weight and trend"
        testid="chart-weight"
        {dates}
        zero={false}
        yFormat={(v) => v.toFixed(1)}
        series={[
          { type: 'dots', values: days.map((d) => toDisplayWeight(d.weight_kg, wu)), label: 'Weigh-in', color: 'var(--weight)' },
          { type: 'line', values: days.map((d) => (d.weight_kg != null ? toDisplayWeight(d.trend_kg, wu) : null)), label: 'Trend', color: 'var(--trend)', width: 2.5, connect: true },
          ...(user.goal_weight_kg
            ? [{ type: 'line' as const, values: days.map(() => toDisplayWeight(user.goal_weight_kg, wu)), label: 'Goal', color: 'var(--sea-ink)', dash: '5 4', width: 1.5 }]
            : []),
        ]}
        tooltip={(i) => [
          days[i].weight_kg != null ? `Weighed ${w1(toDisplayWeight(days[i].weight_kg, wu))} ${wu}` : '',
          days[i].trend_kg != null ? `Trend ${w1(toDisplayWeight(days[i].trend_kg, wu))} ${wu}` : '',
        ].filter(Boolean)}
      />
    </div>
  {/if}

  <div class="grid2 two">
    <div class="card">
      <div class="card-head"><h2>Calories from macros</h2></div>
      <Chart
        label="Calories from protein, carbs and fat per day"
        testid="chart-macros"
        height={190}
        {dates}
        series={[
          { type: 'bar', stack: 'm', values: days.map((d) => (d.logged ? (d.nutrients.protein ?? 0) * 4 : null)), label: 'Protein', color: 'var(--protein)' },
          { type: 'bar', stack: 'm', values: days.map((d) => (d.logged ? (d.nutrients.carbs ?? 0) * 4 : null)), label: 'Carbs', color: 'var(--carbs)' },
          { type: 'bar', stack: 'm', values: days.map((d) => (d.logged ? (d.nutrients.fat ?? 0) * 9 : null)), label: 'Fat', color: 'var(--fat)' },
        ]}
        tooltip={(i) =>
          days[i].logged
            ? [`Protein ${num(days[i].nutrients.protein ?? 0, 0)} g`, `Carbs ${num(days[i].nutrients.carbs ?? 0, 0)} g`, `Fat ${num(days[i].nutrients.fat ?? 0, 0)} g`]
            : ['Not logged']}
      />
    </div>
    <div class="card">
      <div class="card-head"><h2>Running balance</h2></div>
      <Chart
        label="Cumulative calories under or over budget"
        testid="chart-balance"
        height={190}
        {dates}
        series={[{ type: 'line', values: cumulative, label: 'Under (+) / over (−) budget, cumulative', color: 'var(--sea-ink)', width: 2.5 }]}
        tooltip={(i) => (cumulative[i] != null ? [`${signed(cumulative[i] ?? 0)} kcal so far`] : [])}
      />
    </div>
  </div>

  <div class="card" data-testid="micros-card">
    <div class="card-head">
      <h2>Vitamins & minerals</h2>
      <span class="small muted">daily average over logged days</span>
    </div>
    <table class="table">
      <thead><tr><th>Nutrient</th><th class="r">Avg/day</th><th>of target</th><th class="r">%</th><th class="r">Days with data</th></tr></thead>
      <tbody>
        {#each micros as m (m.key)}
          <tr>
            <td>{m.label}{#if m.limit}<span class="faint small">{' (limit)'}</span>{/if}</td>
            <td class="r num">{m.avg != null ? `${num(m.avg)} ${m.unit}` : '–'}</td>
            <td style="width: 28%">
              {#if m.pct != null && m.target}<Progress
                  value={m.avg ?? 0}
                  max={m.target}
                  limit={m.limit}
                  color={m.pct >= 100 ? 'var(--sea)' : m.pct >= 50 ? 'var(--butter)' : 'var(--muted)'}
                  height={6}
                  label={m.label}
                />{/if}
            </td>
            <td class="r num">{m.pct != null ? `${Math.round(m.pct)}%` : ''}</td>
            <td class="r num muted">{m.days_with_data}/{st.logged_days}</td>
          </tr>
        {/each}
      </tbody>
    </table>
    <p class="tiny muted">
      Averages only count foods that report the nutrient; when few days have data the real intake is likely higher. Whole foods imported from USDA
      have the most complete vitamin data.
    </p>
  </div>
{:else if !error}
  <p class="muted">Loading…</p>
{/if}

<style>
  .range {
    max-width: 100%;
    margin-bottom: 16px;
    display: inline-flex;
  }
  .stats {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 12px;
    margin-bottom: 14px;
  }
  .stats .card + .card {
    margin-top: 0;
  }
  .stats {
    gap: 16px;
  }
  @media (max-width: 760px) {
    .stats {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }
  .stat {
    padding: 12px 14px;
  }
  .sl {
    font-size: 0.78rem;
    color: var(--muted);
    font-weight: 600;
  }
  .sv {
    font-family: var(--display);
    font-size: 1.9rem;
    line-height: 1.1;
  }
  .sv small {
    font-family: var(--font);
    font-size: 0.8rem;
    font-weight: 600;
    color: var(--muted);
  }
  .ss {
    font-size: 0.78rem;
    color: var(--muted);
  }
  .pos {
    color: var(--sea-ink);
  }
  .neg {
    color: var(--peach-ink);
  }
  .split {
    display: flex;
    height: 12px;
    overflow: hidden;
    margin: 10px 0 6px;
  }
  .two {
    margin: 14px 0;
    gap: 14px;
  }
  .two .card + .card {
    margin-top: 0;
  }
  @media (max-width: 760px) {
    .two {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
