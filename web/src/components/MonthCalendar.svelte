<script lang="ts">
  import { api } from '../lib/api';
  import { parseDate, todayStr } from '../lib/format';
  import { app, navigate } from '../lib/state.svelte';
  import type { Summary } from '../lib/types';

  /** This month as diamonds: on goal, over, not logged, today, future; plus the streak. */
  let { streak }: { streak: number | null } = $props();

  const today = todayStr();
  const first = `${today.slice(0, 8)}01`;
  const monthName = parseDate(today).toLocaleDateString(undefined, { month: 'long' });
  const dim = new Date(parseDate(today).getFullYear(), parseDate(today).getMonth() + 1, 0).getDate();
  const offset = (parseDate(first).getDay() + 6) % 7; // weeks start on Monday

  let data = $state<Summary | null>(null);
  $effect(() => {
    void app.version;
    api
      .summary(app.userId!, first, today)
      .then((s) => (data = s))
      .catch((e) => app.error(e));
  });

  const cells = $derived.by(() => {
    const byDate = new Map((data?.days ?? []).map((d) => [d.date, d]));
    return Array.from({ length: dim }, (_, i) => {
      const n = i + 1;
      const date = `${today.slice(0, 8)}${String(n).padStart(2, '0')}`;
      const d = byDate.get(date);
      let state: string;
      if (date > today) state = 'future';
      else if (date === today) state = 'today';
      else if (!d) state = 'none';
      else if (!d.logged) state = 'miss';
      else if (d.budget != null && (d.nutrients.calories ?? 0) > d.budget + 0.5) state = 'over';
      else state = 'ok';
      return { n, date, state };
    });
  });
  const labels: Record<string, string> = { ok: 'on goal', over: 'over budget', miss: 'not logged', today: 'today', future: '', none: 'before you started' };
</script>

<div class="mc" data-testid="month-calendar">
  <div class="card-head">
    <span class="card-title">{monthName}</span>
    <span class="row"><span class="display s">{streak ?? '–'}</span><span class="card-sub">day streak</span></span>
  </div>
  <div class="grid">
    {#each ['M', 'T', 'W', 'T', 'F', 'S', 'S'] as d}<div class="dow">{d}</div>{/each}
    {#each Array(offset) as _}<div></div>{/each}
    {#each cells as c (c.n)}
      <button
        class="cell {c.state}"
        disabled={c.state === 'future'}
        onclick={() => navigate(c.state === 'today' ? 'log' : `log?date=${c.date}`)}
        aria-label="{c.n}: {labels[c.state]}"
        data-state={c.state}
      >
        <i></i><span>{c.n}</span>
      </button>
    {/each}
  </div>
  <div class="legend">
    <span><i class="diamond" style="background:var(--sea)"></i>On goal</span>
    <span><i class="diamond" style="background:var(--peach)"></i>Over</span>
    <span><i class="diamond dash"></i>Not logged</span>
  </div>
</div>

<style>
  .mc {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .s {
    font-size: 24px;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(7, minmax(0, 1fr));
    row-gap: 2px;
    justify-items: center;
  }
  .dow {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.1em;
    color: var(--muted);
    padding-bottom: 4px;
  }
  .cell {
    position: relative;
    width: 36px;
    height: 34px;
    display: grid;
    place-items: center;
    border: 0;
    background: none;
    padding: 0;
    cursor: pointer;
    color: var(--muted);
  }
  .cell:disabled {
    cursor: default;
  }
  .cell:focus-visible {
    outline: 2px solid var(--focus);
  }
  .cell i {
    position: absolute;
    left: 7px;
    top: 6px;
    width: 22px;
    height: 22px;
    transform: rotate(45deg);
    box-sizing: border-box;
  }
  .cell span {
    position: relative;
    font-size: 11px;
    font-weight: 600;
  }
  .ok i {
    background: var(--sea);
  }
  .over i {
    background: var(--peach);
  }
  .ok span,
  .over span {
    color: var(--on-pastel);
  }
  .miss i {
    border: 1.5px dashed var(--muted);
    opacity: 0.55;
  }
  .today i {
    border: 2px solid var(--butter-ink);
  }
  .today span {
    color: var(--butter-ink);
  }
  .future i,
  .none i {
    background: var(--track);
  }
  .legend {
    display: flex;
    gap: 14px;
    flex-wrap: wrap;
    font-size: 11px;
    color: var(--muted);
    margin-top: 4px;
  }
  .legend span {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .dash {
    border: 1.5px dashed var(--muted);
    box-sizing: border-box;
  }
</style>
