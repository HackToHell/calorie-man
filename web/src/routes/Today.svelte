<script lang="ts">
  import EntryDialog from '../components/EntryDialog.svelte';
  import Gauge from '../components/Gauge.svelte';
  import MacroPyramid from '../components/MacroPyramid.svelte';
  import MealTiming from '../components/MealTiming.svelte';
  import MealsList from '../components/MealsList.svelte';
  import MicroRadar from '../components/MicroRadar.svelte';
  import MonthCalendar from '../components/MonthCalendar.svelte';
  import NextMeal from '../components/NextMeal.svelte';
  import PageHeader from '../components/PageHeader.svelte';
  import WeekBars from '../components/WeekBars.svelte';
  import WeightCard from '../components/WeightCard.svelte';
  import { api } from '../lib/api';
  import { addDays, fmtDay, kcal, parseDate, signed, todayStr } from '../lib/format';
  import { media } from '../lib/media.svelte';
  import { app } from '../lib/state.svelte';
  import { onDestroy } from 'svelte';
  import type { Entry, Summary } from '../lib/types';

  const today = todayStr();
  const eyebrow = fmtDay(today, { weekday: 'long', month: 'long', day: 'numeric' }).replace(',', ' ·');
  const weekStart = addDays(today, -((parseDate(today).getDay() + 6) % 7)); // Monday
  let now = $state(new Date());
  const tick = setInterval(() => (now = new Date()), 30_000);
  onDestroy(() => clearInterval(tick));
  const diary = $derived(app.today);
  let week = $state<Summary | null>(null);
  let editing = $state<Entry | null>(null);

  $effect(() => {
    void app.version;
    api
      .summary(app.userId!, weekStart, today)
      .then((s) => (week = s))
      .catch((e) => app.error(e));
  });

  const pctOfGoal = $derived(diary?.budget.budget ? Math.round((diary.budget.eaten / diary.budget.budget) * 100) : null);
  const streak = $derived(diary ? (diary.goal?.calories != null ? diary.streak.on_budget : diary.streak.logged) : null);
</script>

<PageHeader home title="Today" {eyebrow} />

{#if diary}
  <div class="dash">
    <div class="card c-gauge" data-testid="budget-card">
      {#if media.phone}
        <div class="eyebrow center">Calories &amp; macros</div>
      {:else}
        <div class="card-head">
          <span class="card-title">Calories &amp; macros</span>
          {#if pctOfGoal != null && diary.budget.budget}<span class="card-sub num">{pctOfGoal}% of {kcal(diary.budget.budget)}</span>{/if}
        </div>
      {/if}
      <Gauge {diary} layout={media.phone ? 'grid' : 'list'} />
      {#if diary.budget.budget == null}
        <p class="small muted">No calorie goal yet. <a href="#/goals">Set goals</a> to get a daily budget.</p>
      {/if}
    </div>

    <div class="card c-timing"><MealTiming {diary} live {now} /></div>

    {#if media.phone}
      <div class="c-next"><NextMeal {diary} /></div>
    {/if}

    <div class="card c-meals">
      <div class="card-head">
        <span class="card-title">Today's meals</span>
        <a class="card-sub" href="#/log">Open the full day →</a>
      </div>
      <MealsList {diary} compact onadd={(meal) => app.openAdd({ meal })} onedit={(e) => (editing = e)} />
    </div>

    <div class="card c-split">
      <div class="card-head"><span class="card-title">Macro split</span><span class="card-sub">share of kcal</span></div>
      <MacroPyramid totals={diary.totals} />
    </div>

    <div class="card c-micro">
      <div class="card-head"><span class="card-title">Micronutrients</span><span class="card-sub">% daily value</span></div>
      <MicroRadar totals={diary.totals} targets={diary.targets} coverage={diary.coverage} />
    </div>

    <div class="card c-week" data-testid="week-card">
      <div class="whead">
        <div class="wt">
          <span class="card-title">This week</span>
          <span class="card-sub">
            Daily calories against your goal{#if diary.week.weekly_budget != null && diary.week.logged_days > 0}{' · '}<b
                class:pos={diary.week.balance >= 0}
                class:neg={diary.week.balance < 0}>{signed(diary.week.balance)} kcal {diary.week.balance >= 0 ? 'banked' : 'over'}</b
              >{/if}
          </span>
        </div>
        {#if week?.stats.avg_calories != null}
          <div class="avg"><span class="display num">{kcal(week.stats.avg_calories)}</span><span class="card-sub">avg kcal</span></div>
        {/if}
      </div>
      {#if week}<WeekBars start={weekStart} {today} days={week.days} goal={diary.budget.goal} />{/if}
    </div>

    <div class="card c-weight"><WeightCard /></div>

    <div class="card c-cal"><MonthCalendar {streak} /></div>
  </div>
{:else}
  <p class="muted">Loading…</p>
{/if}

<EntryDialog bind:entry={editing} onchanged={() => app.touch()} />

<style>
  .dash {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 20px;
    grid-template-areas:
      'gauge split micro'
      'week week weight'
      'timing timing cal'
      'meals meals meals';
  }
  .c-timing {
    grid-area: timing;
  }
  .dash > :global(.card + .card) {
    margin-top: 0;
  }
  .c-gauge {
    grid-area: gauge;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .c-split {
    grid-area: split;
    display: flex;
    flex-direction: column;
  }
  .c-micro {
    grid-area: micro;
  }
  .c-week {
    grid-area: week;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .c-weight {
    grid-area: weight;
  }
  .c-meals {
    grid-area: meals;
  }
  .c-cal {
    grid-area: cal;
  }
  .center {
    text-align: center;
  }
  .whead {
    display: flex;
    align-items: flex-end;
    gap: 12px;
  }
  .wt {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .avg {
    display: flex;
    align-items: baseline;
    gap: 6px;
  }
  .avg .display {
    font-size: 32px;
  }
  .pos {
    color: var(--sea-ink);
  }
  .neg {
    color: var(--peach-ink);
  }
  a.card-sub {
    text-decoration: none;
  }
  a.card-sub:hover {
    color: var(--ink);
  }
  @media (max-width: 1199px) {
    .dash {
      grid-template-columns: repeat(2, minmax(0, 1fr));
      grid-template-areas:
        'gauge split'
        'micro weight'
        'week week'
        'timing timing'
        'meals cal';
    }
  }
  @media (max-width: 899px) {
    .dash {
      grid-template-columns: minmax(0, 1fr);
      gap: 12px;
      grid-template-areas:
        'gauge'
        'timing'
        'next'
        'meals'
        'week'
        'weight'
        'split'
        'micro'
        'cal';
    }
    .c-next {
      grid-area: next;
    }
    .c-gauge {
      padding: 18px 20px;
    }
  }
</style>
