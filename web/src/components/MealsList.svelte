<script lang="ts">
  import { MEAL_COLORS, MEAL_LABELS, fmtTime, kcal, num } from '../lib/format';
  import type { Diary, Entry } from '../lib/types';

  /** Entries grouped by meal, with add / copy actions per meal. */
  let {
    diary,
    onadd,
    onedit,
    oncopy,
    compact = false,
  }: { diary: Diary; onadd: (meal: string) => void; onedit: (e: Entry) => void; oncopy?: (meal: string) => void; compact?: boolean } = $props();

  const meals = $derived(
    Object.keys(MEAL_LABELS).map((m) => ({ meal: m, entries: diary.entries.filter((e) => e.meal === m), totals: diary.meals[m] ?? {} })),
  );
  const colors = MEAL_COLORS;
</script>

<div class="wrapq"><div class="meals" class:compact>
  {#each meals as m (m.meal)}
    <section class="meal" data-testid="meal-{m.meal}">
      <div class="mhead">
        <i class="diamond" style:background={colors[m.meal]}></i>
        <h3>{MEAL_LABELS[m.meal]}</h3>
        {#if m.entries.length}<span class="small muted num">{kcal(m.totals.calories ?? 0)} kcal · {num(m.totals.protein ?? 0, 0)} g P</span>{/if}
        <span class="spacer"></span>
        {#if oncopy}
          <button class="btn small ghost" onclick={() => oncopy(m.meal)} aria-label="Copy into {MEAL_LABELS[m.meal]}" title="Copy into {MEAL_LABELS[m.meal]}">Copy</button>
        {/if}
        <button class="btn small" onclick={() => onadd(m.meal)} data-testid="add-{m.meal}">+ Add</button>
      </div>
      {#if m.entries.length}
        <ul class="list">
          {#each m.entries as e (e.id)}
            <li>
              <button class="item" onclick={() => onedit(e)} data-testid="entry">
                <div class="grow">
                  <div class="ellipsis"><span class="ename">{e.name}</span>{#if e.brand}<span class="muted small">{' · '}{e.brand}</span>{/if}</div>
                  <div class="tiny muted ellipsis">
                    {#if e.time}<span class="time" data-testid="entry-time">{fmtTime(e.time)}</span>{' · '}{/if}{e.display_quantity || 'quick add'}{#if e.note}{' · '}{e.note}{/if}
                  </div>
                </div>
                <div class="r">
                  <div class="num"><b>{kcal(e.nutrients.calories)}</b><span class="tiny muted">{' kcal'}</span></div>
                  <div class="tiny num" style:color="var(--protein-text)">{num(e.nutrients.protein ?? null, 1)} g P</div>
                </div>
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    </section>
  {/each}
</div></div>

<style>
  .wrapq {
    container-type: inline-size;
  }
  .meals {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .meal + .meal {
    border-top: 1px solid var(--line);
    padding-top: 12px;
  }
  /* A wide card shows the four meals in two columns. */
  @container (min-width: 760px) {
    .meals {
      display: grid;
      grid-template-columns: repeat(2, minmax(0, 1fr));
      gap: 14px 36px;
    }
    .meal + .meal {
      border-top: 0;
      padding-top: 0;
    }
    .meal:nth-child(n + 3) {
      border-top: 1px solid var(--line);
      padding-top: 12px;
    }
  }
  .time {
    font-weight: 600;
    color: var(--ink);
  }
  .mhead {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
    min-height: 32px;
  }
  .mhead h3 {
    font-size: 15px;
  }
  .list {
    margin-top: 4px;
  }
  .ename {
    font-weight: 600;
  }
  .r {
    text-align: right;
    flex: none;
  }
  .compact .item {
    padding: 7px 4px;
  }
</style>
