<script lang="ts">
  import { api } from '../lib/api';
  import { MEAL_LABELS, addDays, kcal, num, todayStr } from '../lib/format';
  import { app } from '../lib/state.svelte';
  import type { Diary, RecentFood } from '../lib/types';

  /** The next meal to log, with one-tap "copy yesterday" and the food usually eaten at this meal. */
  let { diary }: { diary: Diary } = $props();

  const ORDER = ['breakfast', 'lunch', 'dinner'];
  const meal = $derived.by(() => {
    const h = new Date().getHours() + new Date().getMinutes() / 60;
    const byTime = h < 10.5 ? 0 : h < 14.5 ? 1 : h < 22 ? 2 : 3;
    for (let i = byTime; i < ORDER.length; i++) if (!diary.entries.some((e) => e.meal === ORDER[i])) return ORDER[i];
    return byTime < 3 ? ORDER[Math.min(byTime, 2)] : 'snacks';
  });
  const logged = $derived(diary.entries.filter((e) => e.meal === meal));
  const loggedKcal = $derived(logged.reduce((a, e) => a + (e.nutrients.calories ?? 0), 0));

  let yesterday = $state<Diary | null>(null);
  let recent = $state<RecentFood[]>([]);
  let busy = $state(false);
  $effect(() => {
    void app.version;
    const uid = app.userId!;
    Promise.all([api.diary(uid, addDays(todayStr(), -1)), api.recentFoods(uid, 40)])
      .then(([y, r]) => ((yesterday = y), (recent = r)))
      .catch(() => {});
  });

  const yEntries = $derived(yesterday?.entries.filter((e) => e.meal === meal) ?? []);
  // "Salmon +2" keeps the calories visible on a narrow button.
  const yName = $derived(yEntries.length ? `${yEntries[0].name.split(',')[0]}${yEntries.length > 1 ? ` +${yEntries.length - 1}` : ''}` : '');
  const yKcal = $derived(yEntries.reduce((a, e) => a + (e.nutrients.calories ?? 0), 0));
  /** Calories of the portion last logged. */
  function portionKcal(r: RecentFood): number | null {
    const f = r.food;
    const s = f.servings.find((x) => x.id === r.last_serving_id);
    if (s) return ((f.nutrients.calories ?? 0) * s.amount * r.last_quantity) / f.basis_amount;
    if (f.basis_unit === r.last_unit) return ((f.nutrients.calories ?? 0) * r.last_quantity) / f.basis_amount;
    return f.per_serving?.calories != null ? f.per_serving.calories * r.last_quantity : null;
  }
  // The usual main dish for this meal: recipes and real portions beat side items like spinach.
  const suggestion = $derived.by(() => {
    const ids = new Set(yEntries.map((e) => e.food_id));
    const n = (r: RecentFood) => r.meal_uses?.[meal] ?? 0;
    const score = (r: RecentFood) => n(r) * (r.food.kind === 'recipe' ? 3 : 1) * ((portionKcal(r) ?? 0) >= 150 ? 1 : 0.3);
    return recent.filter((r) => n(r) > 0 && !ids.has(r.food.id)).sort((a, b) => score(b) - score(a))[0] ?? null;
  });
  const sugKcal = $derived(suggestion ? portionKcal(suggestion) : null);

  async function copyYesterday() {
    busy = true;
    try {
      const r = await api.copy(app.userId!, { from_date: addDays(todayStr(), -1), to_date: todayStr(), meal });
      app.toast(`Copied ${r.length} item${r.length === 1 ? '' : 's'} from yesterday's ${MEAL_LABELS[meal].toLowerCase()}`);
      app.touch();
    } catch (e) {
      app.error(e);
    } finally {
      busy = false;
    }
  }
</script>

<div class="card solid nm" data-testid="next-meal">
  <div class="head">
    <div class="display t">{MEAL_LABELS[meal]}</div>
    <div class="s">{logged.length ? `${kcal(loggedKcal)} kcal logged` : 'Nothing logged yet'}</div>
  </div>
  <div class="opts">
    {#if yEntries.length}
      <button class="opt terrace-sm peach" disabled={busy} onclick={copyYesterday} data-testid="copy-yesterday">
        <b>Copy yesterday</b><span class="ellipsis">{yName} · {kcal(yKcal)}</span>
      </button>
    {/if}
    {#if suggestion}
      <button
        class="opt terrace-sm ghost"
        onclick={() =>
          app.openAdd({ meal, food: suggestion.food, quantity: suggestion.last_quantity, servingId: suggestion.last_serving_id, unit: suggestion.last_unit })}
        data-testid="suggestion"
      >
        <b class="ellipsis">{suggestion.food.name}</b>
        <span class="ellipsis"
          >{suggestion.food.kind === 'recipe' ? 'Recipe · ' : ''}{num(suggestion.last_quantity)} {suggestion.last_unit}{sugKcal != null ? ` · ${kcal(sugKcal)}` : ''}</span
        >
      </button>
    {/if}
    {#if !yEntries.length || !suggestion}
      <button class="opt terrace-sm ghost" onclick={() => app.openAdd({ meal })}>
        <b>Search foods</b><span>Library, online or barcode</span>
      </button>
    {/if}
  </div>
</div>

<style>
  .nm {
    padding: 12px 20px 16px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
  }
  .t {
    font-size: 22px;
  }
  .s {
    font-size: 12px;
    color: var(--solid-muted);
  }
  .opts {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }
  .opt {
    min-height: 44px;
    padding: 6px 10px;
    border: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    min-width: 0;
    color: inherit;
  }
  .opt b {
    font-size: 14px;
    max-width: 100%;
  }
  .opt span {
    font-size: 11px;
    max-width: 100%;
  }
  .opt.peach {
    background: var(--peach);
    color: var(--on-pastel);
  }
  .opt.ghost {
    background: var(--solid-ghost);
  }
  .opt.ghost span {
    color: var(--solid-muted);
  }
  .opt:focus-visible {
    outline: none;
    box-shadow: inset 0 0 0 2px var(--solid-ink);
  }
</style>
