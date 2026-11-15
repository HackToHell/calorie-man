<script lang="ts">
  import PageHeader from '../components/PageHeader.svelte';
  import { api } from '../lib/api';
  import { KG_PER_LB, fmtDay, isValidDate, kcal, num, optNum, todayStr, txt } from '../lib/format';
  import { app } from '../lib/state.svelte';
  import type { Energy, Goal, Settings } from '../lib/types';

  const user = $derived(app.user!);
  const setup = $derived(app.route.params.get('setup') === '1');
  const unit = $derived(user.weight_unit);

  // ---- goals
  let goals = $state<Goal[]>([]);
  let energy = $state<Energy | null>(null);
  let gFrom = $state(todayStr());
  let gCal = $state('');
  let gP = $state('');
  let gC = $state('');
  let gF = $state('');
  let gFiber = $state('');
  let gEx = $state(false);
  let micros = $state<Record<string, string>>({});
  let savingGoal = $state(false);

  async function loadGoals() {
    try {
      const [g, e] = await Promise.all([api.goals(user.id), api.energy(user.id)]);
      goals = g;
      energy = e;
      const today = todayStr();
      const cur = [...g].reverse().find((x) => x.effective_from <= today) ?? g[0];
      const s = (v: number | null | undefined) => (v != null ? String(Math.round(v)) : '');
      gCal = s(cur?.calories);
      gP = s(cur?.protein);
      gC = s(cur?.carbs);
      gF = s(cur?.fat);
      gFiber = s(cur?.fiber);
      gEx = cur?.exercise_adds_to_budget ?? false;
      micros = Object.fromEntries(Object.entries(cur?.micros ?? {}).map(([k, v]) => [k, String(v)]));
    } catch (err) {
      app.error(err);
    }
  }
  $effect(() => {
    void user.id;
    void app.version;
    loadGoals();
  });

  const refWeight = $derived(user.goal_weight_kg ?? energy?.weight_kg ?? null);

  function fillMacros(fatPct = 30) {
    const c = optNum(gCal) ?? 0;
    const p = optNum(gP) ?? 0;
    if (!c) return app.toast('Set calories first', 'warn');
    const fat = (c * fatPct) / 100 / 9;
    const carbs = Math.max(0, (c - p * 4 - fat * 9) / 4);
    gF = String(Math.round(fat));
    gC = String(Math.round(carbs));
  }

  async function saveGoal(e: SubmitEvent) {
    e.preventDefault();
    if (!isValidDate(gFrom)) return app.toast('Pick a valid start date', 'error');
    const n = optNum;
    const m: Record<string, number | null> = {};
    for (const d of app.catalog?.nutrients ?? []) {
      if (['calories', 'protein', 'carbs', 'fat', 'fiber'].includes(d.key)) continue;
      m[d.key] = optNum(micros[d.key]);
    }
    savingGoal = true;
    try {
      await api.setGoal(user.id, {
        effective_from: gFrom,
        calories: n(gCal),
        protein: n(gP),
        carbs: n(gC),
        fat: n(gF),
        fiber: n(gFiber),
        exercise_adds_to_budget: gEx,
        micros: m as any,
      });
      app.toast(gFrom === todayStr() ? 'Goals saved' : `Goals saved from ${fmtDay(gFrom)}`);
      app.touch();
    } catch (err) {
      app.error(err);
    } finally {
      savingGoal = false;
    }
  }

  async function deleteGoal(g: Goal) {
    if (!confirm(`Delete the goal set starting ${g.effective_from}?`)) return;
    try {
      await api.deleteGoal(g.id);
      app.touch();
    } catch (err) {
      app.error(err);
    }
  }

</script>

<PageHeader title="Goals" eyebrow="Daily targets" />
{#if setup}
  <div class="banner ok intro">Welcome, {user.name}! Set a calorie and protein goal to get a daily budget. Add your sex, birth date and height in <a href="#/profile">Profile</a> for a maintenance estimate.</div>
{/if}

<div class="cols">
  <form class="card stack" onsubmit={saveGoal} data-testid="goals-form">
    {#if energy}
      <div class="calc small">
        {#if energy.tdee}
          <div>
            Estimated maintenance: <b class="num">{kcal(energy.tdee)} kcal</b>
            <span class="muted">(BMR {kcal(energy.bmr)} × activity {energy.activity_level ?? 1.2})</span>
          </div>
        {/if}
        {#if energy.adaptive_tdee}
          <div>From your logs & weight: <b class="num">{kcal(energy.adaptive_tdee)} kcal</b> <span class="muted">{energy.adaptive_note}</span></div>
        {/if}
        {#if energy.missing.length && !energy.adaptive_tdee}
          <div class="muted">Add {energy.missing.join(', ').replace('_', ' ')} to estimate maintenance calories.</div>
        {/if}
        {#if energy.suggestions.length}
          <div class="row wrap rates">
            {#each energy.suggestions as s}
              <button
                type="button"
                class="btn small"
                title={s.warning ?? ''}
                class:warnb={!!s.warning}
                onclick={() => (gCal = String(s.calories))}
                >{s.rate_kg_per_week === 0
                  ? 'Maintain'
                  : `${s.rate_kg_per_week > 0 ? '+' : '−'}${unit === 'lb' ? num(Math.abs(s.rate_kg_per_week) / KG_PER_LB, 1) + ' lb' : Math.abs(s.rate_kg_per_week) + ' kg'}/wk`}
                · {kcal(s.calories)}</button
              >
            {/each}
          </div>
        {/if}
      </div>
    {/if}
    <div class="grid2">
      <label class="field"><span>Calories (kcal)</span><input class="input num" type="number" min="0" bind:value={gCal} data-testid="goal-calories" /></label>
      <label class="field"><span>Protein (g)</span><input class="input num" type="number" min="0" bind:value={gP} data-testid="goal-protein" /></label>
    </div>
    {#if refWeight}
      <div class="row wrap small">
        <span class="muted">Protein per kg of {user.goal_weight_kg ? 'goal' : 'current'} weight:</span>
        {#each [1.2, 1.6, 2.0, 2.2] as f}
          <button type="button" class="btn small" onclick={() => (gP = String(Math.round(refWeight * f)))}>{f} g/kg · {Math.round(refWeight * f)} g</button>
        {/each}
      </div>
    {/if}
    <div class="grid3">
      <label class="field"><span>Carbs (g)</span><input class="input num" type="number" min="0" bind:value={gC} data-testid="goal-carbs" /></label>
      <label class="field"><span>Fat (g)</span><input class="input num" type="number" min="0" bind:value={gF} data-testid="goal-fat" /></label>
      <label class="field"><span>Fiber (g)</span><input class="input num" type="number" min="0" bind:value={gFiber} /></label>
    </div>
    <div class="row wrap small">
      <span class="muted">Fill carbs & fat from calories and protein:</span>
      {#each [25, 30, 35] as p}<button type="button" class="btn small" onclick={() => fillMacros(p)}>{p}% fat</button>{/each}
    </div>
    <label class="check small"><input type="checkbox" bind:checked={gEx} /> Add exercise calories to the daily budget</label>
    <details>
      <summary class="small">Vitamin & mineral targets</summary>
      <p class="tiny muted">Leave empty to use the FDA daily value (shown greyed).</p>
      <div class="grid3 micro">
        {#each (app.catalog?.nutrients ?? []).filter((d) => !['calories', 'protein', 'carbs', 'fat', 'fiber'].includes(d.key)) as d (d.key)}
          <label class="field"
            ><span>{d.label} ({d.unit}){d.limit ? ' max' : ''}</span><input
              class="input num"
              type="number"
              min="0"
              step="any"
              bind:value={micros[d.key]}
              placeholder={d.dv != null ? String(d.dv) : ''}
            /></label
          >
        {/each}
      </div>
    </details>
    <label class="field"
      ><span>Starting</span><input class="input" type="date" bind:value={gFrom} required data-testid="goal-from" /></label
    >
    <p class="tiny muted">Past days keep the goals that applied then. Pick an earlier date to change history.</p>
    <button class="btn primary" disabled={savingGoal} data-testid="save-goals">Save goals</button>
    {#if goals.length}
      <details>
        <summary class="small">Goal history ({goals.length})</summary>
        <table class="table">
          <thead><tr><th>From</th><th class="r">kcal</th><th class="r">Protein</th><th class="r">C / F</th><th></th></tr></thead>
          <tbody>
            {#each [...goals].reverse() as g (g.id)}
              <tr>
                <td>{fmtDay(g.effective_from, { year: 'numeric', month: 'short', day: 'numeric' })}</td>
                <td class="r num">{kcal(g.calories)}</td>
                <td class="r num">{num(g.protein, 0)} g</td>
                <td class="r num">{num(g.carbs, 0)} / {num(g.fat, 0)}</td>
                <td class="r"><button type="button" class="btn ghost small" onclick={() => deleteGoal(g)} aria-label="Delete goal">✕</button></td>
              </tr>
            {/each}
          </tbody>
        </table>
      </details>
    {/if}
  </form>
  <div class="card stack help">
    <span class="card-title">How budgets work</span>
    <p class="small muted">Your calorie goal is the daily budget. Goals apply from their start date, so past days keep the goal that applied then.</p>
    <p class="small muted">With “add exercise calories”, burned calories raise that day's budget. The weekly bank on Today adds up how far under or over budget you were on earlier days this week.</p>
    <p class="small muted">Vitamin and mineral targets default to FDA daily values; set your own below the macros if a clinician gave you different numbers.</p>
  </div>
</div>

<style>
  .intro {
    margin-bottom: 16px;
  }
  .cols {
    display: grid;
    grid-template-columns: minmax(0, 1.4fr) minmax(0, 1fr);
    gap: 20px;
    align-items: start;
  }
  .cols :global(.card + .card) {
    margin-top: 0;
  }
  @media (max-width: 1000px) {
    .cols {
      grid-template-columns: minmax(0, 1fr);
    }
  }
  .calc {
    --t: 5px;
    clip-path: var(--terrace);
    background: var(--surface-2);
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .rates {
    gap: 4px;
  }
  .warnb {
    color: var(--peach-ink);
  }
  .micro {
    margin-top: 6px;
  }
</style>
