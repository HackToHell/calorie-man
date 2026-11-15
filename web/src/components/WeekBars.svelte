<script lang="ts">
  import { addDays, kcal } from '../lib/format';
  import { navigate } from '../lib/state.svelte';
  import type { SummaryDay } from '../lib/types';

  /** Monday-to-Sunday calories with terrace caps and the goal line. */
  let { start, today, days, goal }: { start: string; today: string; days: SummaryDay[]; goal: number | null } = $props();

  const NAMES = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];
  const H = 150;
  const byDate = $derived(new Map(days.map((d) => [d.date, d])));
  const slots = $derived(
    NAMES.map((name, i) => {
      const date = addDays(start, i);
      const d = byDate.get(date);
      const v = d?.logged ? (d.nutrients.calories ?? 0) : null;
      const budget = d?.budget ?? goal;
      const state =
        date > today ? 'future' : date === today ? 'today' : v == null ? 'miss' : budget != null && v > budget + 0.5 ? 'over' : 'ok';
      return { date, name, v, state };
    }),
  );
  const max = $derived(Math.max(1, (goal ?? 0) * 1.24, ...slots.map((s) => (s.v ?? 0) * 1.08)));
  const colors: Record<string, string> = { ok: 'var(--sea)', over: 'var(--peach)', today: 'var(--butter)' };
</script>

<div class="wk" data-testid="week-bars">
  {#if goal}
    <div class="goal" style:bottom="{24 + (goal / max) * H}px"></div>
    <div class="glabel" style:bottom="{30 + (goal / max) * H}px">Goal {kcal(goal)}</div>
  {/if}
  <div class="bars">
    {#each slots as s (s.date)}
      <button
        class="col"
        disabled={s.state === 'future'}
        onclick={() => navigate(s.date === today ? 'log' : `log?date=${s.date}`)}
        aria-label="{s.name}: {s.v == null ? 'not logged' : `${kcal(s.v)} kcal`}"
        data-state={s.state}
      >
        {#if s.v != null && s.v > 0}
          {@const total = (s.v / max) * H}
          <span class="v num">{kcal(s.v)}</span>
          <i style="width:34%;height:5px" style:background={colors[s.state]}></i>
          <i style="width:65%;height:5px" style:background={colors[s.state]}></i>
          <i class="body" style:height="{Math.max(0, total - 16)}px" style:background={colors[s.state]}></i>
        {:else if s.state === 'miss'}
          <i class="miss"></i>
        {/if}
      </button>
    {/each}
  </div>
  <div class="names">
    {#each slots as s (s.date)}<span class:today={s.state === 'today'}>{s.name}</span>{/each}
  </div>
</div>

<style>
  .wk {
    position: relative;
    height: 200px;
  }
  .goal {
    position: absolute;
    left: 0;
    right: 0;
    border-top: 1.5px dashed oklch(0.91 0.08 95 / 0.8);
  }
  .glabel {
    position: absolute;
    right: 0;
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--butter-ink);
  }
  .bars {
    position: absolute;
    left: 0;
    right: 0;
    top: 0;
    bottom: 24px;
    display: flex;
    align-items: flex-end;
    justify-content: space-around;
  }
  .col {
    width: 13%;
    max-width: 46px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: flex-end;
    gap: 3px;
    height: 100%;
    padding: 0;
    border: 0;
    background: none;
    cursor: pointer;
    color: inherit;
  }
  .col:disabled {
    cursor: default;
  }
  .col:focus-visible {
    outline: 2px solid var(--focus);
  }
  .col i {
    display: block;
  }
  .body {
    width: 100%;
  }
  .v {
    font-size: 12px;
    font-weight: 600;
    color: var(--muted);
    padding-bottom: 3px;
    white-space: nowrap;
  }
  .miss {
    width: 100%;
    height: 14px;
    border: 1.5px dashed var(--muted);
    opacity: 0.5;
  }
  .names {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    display: flex;
    justify-content: space-around;
  }
  .names span {
    width: 13%;
    max-width: 46px;
    text-align: center;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--muted);
  }
  .names span.today {
    color: var(--ink);
  }
  @media (max-width: 480px) {
    .v {
      font-size: 10px;
    }
  }
</style>
