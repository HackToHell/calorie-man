<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '../lib/api';
  import { kcal, num } from '../lib/format';
  import { app } from '../lib/state.svelte';
  import type { Food, RecentFood } from '../lib/types';

  let {
    onpick,
    showRecent = true,
    kind,
    excludeId,
    placeholder = 'Search foods & recipes',
    query = $bindable(''),
  }: {
    onpick: (f: Food, recent?: RecentFood) => void;
    showRecent?: boolean;
    kind?: string;
    excludeId?: number;
    placeholder?: string;
    query?: string;
  } = $props();

  let results = $state<Food[]>([]);
  let recent = $state<RecentFood[]>([]);
  let loading = $state(false);
  let searched = $state(false);
  let seq = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let inputEl: HTMLInputElement | undefined = $state();

  onMount(async () => {
    inputEl?.focus();
    if (showRecent && app.userId != null) {
      try {
        recent = await api.recentFoods(app.userId, 15);
      } catch (e) {
        app.error(e);
      }
    }
    if (query) run();
  });

  function run() {
    clearTimeout(timer);
    const q = query.trim();
    if (!q) {
      results = [];
      searched = false;
      return;
    }
    timer = setTimeout(async () => {
      const my = ++seq;
      loading = true;
      try {
        const r = await api.foods({ q, kind, limit: 25, user_id: app.userId ?? undefined });
        if (my === seq) {
          results = r.filter((f) => f.id !== excludeId);
          searched = true;
        }
      } catch (e) {
        app.error(e);
      } finally {
        if (my === seq) loading = false;
      }
    }, 140);
  }

  function line(f: Food) {
    const p = f.per_serving;
    if (p) return `${kcal(p.calories)} kcal · ${num(p.protein, 1)} g protein per ${p.label}`;
    return `${kcal(f.nutrients.calories)} kcal · ${num(f.nutrients.protein, 1)} g protein per ${num(f.basis_amount)} ${f.basis_unit}`;
  }
</script>

<div class="stack">
  <div class="searchbox">
    <input
      bind:this={inputEl}
      class="input"
      type="search"
      {placeholder}
      bind:value={query}
      oninput={run}
      aria-label="Search the food library"
      data-testid="food-search"
      autocomplete="off"
    />
    {#if loading}<span class="spin tiny muted">…</span>{/if}
  </div>

  {#if query.trim()}
    {#if results.length}
      <ul class="list" data-testid="search-results">
        {#each results as f (f.id)}
          <li>
            <button class="item" type="button" onclick={() => onpick(f)} data-testid="food-result">
              <div class="grow">
                <div class="ellipsis">
                  <b>{f.name}</b>{#if f.brand}<span class="muted">{' · '}{f.brand}</span>{/if}
                  {#if f.kind === 'recipe'}<span class="chip accent">recipe</span>{/if}
                </div>
                <div class="small muted ellipsis">{line(f)}</div>
              </div>
            </button>
          </li>
        {/each}
      </ul>
    {:else if searched && !loading}
      <p class="empty small">No library foods match “{query.trim()}”.</p>
    {/if}
  {:else if showRecent && recent.length}
    <h3 class="small muted">Recent</h3>
    <ul class="list" data-testid="recent-foods">
      {#each recent.filter((r) => r.food.id !== excludeId && (!kind || r.food.kind === kind)) as r (r.food.id)}
        <li>
          <button class="item" type="button" onclick={() => onpick(r.food, r)} data-testid="recent-food">
            <div class="grow">
              <div class="ellipsis">
                <b>{r.food.name}</b>{#if r.food.brand}<span class="muted">{' · '}{r.food.brand}</span>{/if}
                {#if r.food.kind === 'recipe'}<span class="chip accent">recipe</span>{/if}
              </div>
              <div class="small muted ellipsis">Last: {num(r.last_quantity)} {r.last_unit} · {line(r.food)}</div>
            </div>
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .searchbox {
    position: relative;
  }
  .spin {
    position: absolute;
    right: 34px;
    top: 50%;
    transform: translateY(-50%);
  }
</style>
