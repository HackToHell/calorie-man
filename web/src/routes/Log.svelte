<script lang="ts">
  import Dialog from '../components/Dialog.svelte';
  import EntryDialog from '../components/EntryDialog.svelte';
  import Gauge from '../components/Gauge.svelte';
  import MealTiming from '../components/MealTiming.svelte';
  import MealsList from '../components/MealsList.svelte';
  import NutrientTable from '../components/NutrientTable.svelte';
  import PageHeader from '../components/PageHeader.svelte';
  import { api } from '../lib/api';
  import { MEAL_LABELS, addDays, fmtDay, isValidDate, kcal, num, optNum, relDay, signed, todayStr, txt } from '../lib/format';
  import { media } from '../lib/media.svelte';
  import { app, navigate } from '../lib/state.svelte';
  import type { Diary, Entry } from '../lib/types';

  const user = $derived(app.user!);
  const date = $derived.by(() => {
    const d = app.route.params.get('date');
    return isValidDate(d) ? d : todayStr();
  });
  const isToday = $derived(date === todayStr());

  let diary = $state<Diary | null>(null);
  let error = $state<string | null>(null);
  let editing = $state<Entry | null>(null);
  let showNutrients = $state(false);
  let seq = 0;

  $effect(() => {
    void app.version;
    const [uid, d] = [user.id, date];
    const my = ++seq;
    api
      .diary(uid, d)
      .then((x) => my === seq && ((diary = x), (error = null)))
      .catch((e) => my === seq && (error = (e as Error).message));
  });

  function go(d: string) {
    navigate(d === todayStr() ? 'log' : `log?date=${d}`);
  }

  // exercise
  let exOpen = $state(false);
  let exName = $state('');
  let exCal = $state('');
  let exMin = $state('');
  async function addExercise(e: SubmitEvent) {
    e.preventDefault();
    try {
      await api.addExercise(user.id, { date, name: txt(exName), calories: optNum(exCal) ?? NaN, duration_min: optNum(exMin) });
      exOpen = false;
      exName = exCal = exMin = '';
      app.touch();
    } catch (err) {
      app.error(err);
    }
  }
  async function delExercise(id: number) {
    try {
      await api.deleteExercise(id);
      app.touch();
    } catch (err) {
      app.error(err);
    }
  }

  // copy
  let copyOpen = $state(false);
  let copyFromUser = $state<number>(0);
  let copyFromDate = $state('');
  let copyMeal = $state('');
  function openCopy(meal = '', fromUser?: number) {
    copyFromUser = fromUser ?? user.id;
    copyFromDate = fromUser && fromUser !== user.id ? date : addDays(date, -1);
    copyMeal = meal;
    copyOpen = true;
  }
  async function doCopy(e: SubmitEvent) {
    e.preventDefault();
    try {
      const r = await api.copy(user.id, {
        from_date: copyFromDate,
        to_date: date,
        meal: copyMeal || undefined,
        from_user_id: copyFromUser === user.id ? undefined : copyFromUser,
      });
      app.toast(r.length ? `Copied ${r.length} item${r.length === 1 ? '' : 's'}` : 'Nothing to copy from that day', r.length ? 'ok' : 'warn');
      copyOpen = false;
      app.touch();
    } catch (err) {
      app.error(err);
    }
  }
  const others = $derived(app.users.filter((u) => u.id !== user.id));
  const eyebrow = $derived(fmtDay(date, { weekday: 'long', month: 'long', day: 'numeric' }).replace(',', ' ·'));
</script>

<PageHeader title="Log food" {eyebrow} />

<div class="daybar">
  <button class="btn icon" aria-label="Previous day" onclick={() => go(addDays(date, -1))} data-testid="prev-day">‹</button>
  <label class="datepick terrace-sm">
    <span class="dlabel" data-testid="day-label">{relDay(date)}</span>
    <input type="date" value={date} onchange={(e) => isValidDate(e.currentTarget.value) && go(e.currentTarget.value)} aria-label="Pick a date" />
  </label>
  <button class="btn icon" aria-label="Next day" onclick={() => go(addDays(date, 1))} data-testid="next-day">›</button>
  {#if !isToday}<button class="btn small" onclick={() => go(todayStr())}>Today</button>{/if}
  <span class="spacer"></span>
  <button class="btn small" onclick={() => openCopy()} data-testid="copy-open">Copy day…</button>
  {#if media.phone}<button class="btn small primary" onclick={() => app.openAdd({ date })} data-testid="add-food-day">+ Add</button>{/if}
</div>

{#if error}<div class="banner error">{error}</div>{/if}

{#if diary}
  <div class="layout">
    <aside class="side">
      <div class="card" data-testid="day-budget">
        <div class="card-head">
          <span class="card-title">{isToday ? 'Today' : relDay(date)}</span>
          {#if diary.budget.budget != null}<span class="card-sub num">budget {kcal(diary.budget.budget)}</span>{/if}
        </div>
        <Gauge {diary} layout="list" />
        {#if diary.budget.exercise > 0}
          <p class="tiny muted">Exercise {kcal(diary.budget.exercise)} kcal{diary.budget.exercise_adds_to_budget ? ' added to the budget' : ' (not added)'}</p>
        {/if}
        {#if diary.week.weekly_budget != null && diary.week.logged_days > 0}
          <p class="small" data-testid="day-week">
            <b class:pos={diary.week.balance >= 0} class:neg={diary.week.balance < 0}>{signed(diary.week.balance)} kcal</b>
            {diary.week.balance >= 0 ? 'banked' : 'over'} earlier this week
          </p>
        {/if}
      </div>
      <div class="card"><MealTiming {diary} live={isToday} /></div>
    </aside>

    <section class="main">
      <div class="card">
        <MealsList {diary} onadd={(meal) => app.openAdd({ meal, date })} onedit={(e) => (editing = e)} oncopy={(m) => openCopy(m)} />
      </div>

      <div class="card" data-testid="exercise-card">
        <div class="card-head">
          <span class="card-title">Exercise</span>
          {#if diary.exercises.length}<span class="card-sub num">{kcal(diary.budget.exercise)} kcal</span>{/if}
          <span class="spacer"></span>
          <button class="btn small" onclick={() => (exOpen = true)} data-testid="add-exercise">+ Add</button>
        </div>
        {#if diary.exercises.length}
          <ul class="list">
            {#each diary.exercises as x (x.id)}
              <li class="row exrow">
                <span class="grow">{x.name}{#if x.duration_min}<span class="muted small">{' · '}{num(x.duration_min, 0)} min</span>{/if}</span>
                <span class="num">{kcal(x.calories)} kcal</span>
                <button class="btn ghost small" aria-label="Remove {x.name}" onclick={() => delExercise(x.id)}>✕</button>
              </li>
            {/each}
          </ul>
          {#if !diary.budget.exercise_adds_to_budget && diary.budget.budget != null}
            <p class="tiny muted">Exercise isn’t added to your budget. Change this in Goals.</p>
          {/if}
        {:else}
          <p class="small muted">Nothing logged. Burned calories count towards the budget only if your goal says so.</p>
        {/if}
      </div>

      <div class="card">
        <button class="toggle" onclick={() => (showNutrients = !showNutrients)} aria-expanded={showNutrients} data-testid="toggle-nutrients">
          <span class="card-title">Nutrients</span>
          <span class="card-sub">vitamins, minerals & more</span>
          <span class="spacer"></span>
          <i class="diamond" style:background={showNutrients ? 'var(--peach)' : 'var(--muted)'}></i>
        </button>
        {#if showNutrients}
          <div class="nt"><NutrientTable totals={diary.totals} targets={diary.targets} coverage={diary.coverage} /></div>
        {/if}
      </div>
    </section>
  </div>
{/if}

<EntryDialog bind:entry={editing} onchanged={() => app.touch()} />

<Dialog bind:open={exOpen} title="Add exercise" testid="exercise-dialog">
  <form id="ex-form" class="stack" onsubmit={addExercise}>
    <label class="field"><span>Activity</span><input class="input" bind:value={exName} required maxlength="100" placeholder="e.g. Run" /></label>
    <div class="grid2">
      <label class="field"><span>Calories burned</span><input class="input num" type="number" min="0" step="1" bind:value={exCal} required /></label>
      <label class="field"><span>Minutes</span><input class="input num" type="number" min="0" step="1" bind:value={exMin} /></label>
    </div>
  </form>
  {#snippet footer()}
    <button class="btn" type="button" onclick={() => (exOpen = false)}>Cancel</button>
    <button class="btn primary" type="submit" form="ex-form">Add</button>
  {/snippet}
</Dialog>

<Dialog bind:open={copyOpen} title="Copy food into {relDay(date)}" testid="copy-dialog">
  <form id="copy-form" class="stack" onsubmit={doCopy}>
    <div class="grid2">
      <label class="field"
        ><span>From</span>
        <select class="input" bind:value={copyFromUser}>
          <option value={user.id}>My diary</option>
          {#each others as o}<option value={o.id}>{o.name}'s diary</option>{/each}
        </select>
      </label>
      <label class="field"><span>Day</span><input class="input" type="date" bind:value={copyFromDate} required /></label>
    </div>
    <label class="field"
      ><span>Meal</span>
      <select class="input" bind:value={copyMeal}>
        <option value="">All meals</option>
        {#each Object.entries(MEAL_LABELS) as [k, l]}<option value={k}>{l}</option>{/each}
      </select>
    </label>
    <div class="row wrap">
      <button type="button" class="btn small" onclick={() => ((copyFromUser = user.id), (copyFromDate = addDays(date, -1)))}>Yesterday</button>
      {#each others as o}
        <button type="button" class="btn small" onclick={() => ((copyFromUser = o.id), (copyFromDate = date))}>{o.name} today</button>
      {/each}
    </div>
  </form>
  {#snippet footer()}
    <button class="btn" type="button" onclick={() => (copyOpen = false)}>Cancel</button>
    <button class="btn primary" type="submit" form="copy-form" data-testid="copy-confirm">Copy</button>
  {/snippet}
</Dialog>

<style>
  .daybar {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 16px;
    flex-wrap: wrap;
  }
  .daybar .btn.icon {
    font-size: 22px;
    line-height: 1;
  }
  .datepick {
    position: relative;
    display: flex;
    align-items: center;
    height: 40px;
    padding: 0 16px;
    background: var(--surface);
    cursor: pointer;
  }
  .datepick input {
    position: absolute;
    inset: 0;
    opacity: 0;
    cursor: pointer;
    width: 100%;
  }
  .dlabel {
    font-family: var(--display);
    font-size: 20px;
  }
  .layout {
    display: grid;
    grid-template-columns: minmax(0, 330px) minmax(0, 1fr);
    gap: 20px;
    align-items: start;
  }
  .side {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .side :global(.card + .card) {
    margin-top: 0;
  }
  .main {
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-width: 0;
  }
  .main :global(.card + .card) {
    margin-top: 0;
  }
  .pos {
    color: var(--sea-ink);
  }
  .neg {
    color: var(--peach-ink);
  }
  .exrow {
    padding: 8px 2px;
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .toggle {
    display: flex;
    align-items: baseline;
    gap: 10px;
    width: 100%;
    background: none;
    border: 0;
    padding: 0;
    cursor: pointer;
    color: inherit;
    text-align: left;
  }
  .nt {
    margin-top: 14px;
  }
  @media (max-width: 899px) {
    .layout {
      grid-template-columns: minmax(0, 1fr);
      gap: 12px;
    }
    .side {
      position: static;
    }
    .main {
      gap: 12px;
    }
  }
</style>
