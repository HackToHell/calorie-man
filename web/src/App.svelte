<script lang="ts">
  import { onMount } from 'svelte';
  import AddFoodDialog from './components/AddFoodDialog.svelte';
  import DemoBanner from './components/DemoBanner.svelte';
  import Logo from './components/Logo.svelte';
  import Toasts from './components/Toasts.svelte';
  import UserSwitcher from './components/UserSwitcher.svelte';
  import { DEMO } from './lib/env';
  import { app, navigate } from './lib/state.svelte';
  import Foods from './routes/Foods.svelte';
  import Goals from './routes/Goals.svelte';
  import Log from './routes/Log.svelte';
  import Onboarding from './routes/Onboarding.svelte';
  import Profile from './routes/Profile.svelte';
  import Today from './routes/Today.svelte';
  import Trends from './routes/Trends.svelte';
  import WeightPage from './routes/Weight.svelte';

  onMount(() => app.init());

  const nav = [
    { page: 'today', label: 'Today' },
    { page: 'log', label: 'Log food' },
    { page: 'foods', label: 'Food bank' },
    { page: 'recipes', label: 'Recipes' },
    { page: 'trends', label: 'Trends' },
    { page: 'weight', label: 'Weight' },
    { page: 'goals', label: 'Goals' },
    { page: 'profile', label: 'Profile' },
  ];
  const aliases: Record<string, string> = { settings: 'profile' };
  const page = $derived.by(() => {
    const p = aliases[app.route.page] ?? app.route.page;
    return nav.some((n) => n.page === p) ? p : 'today';
  });
  const tabs = [
    { page: 'today', label: 'Today' },
    { page: 'recipes', label: 'Recipes' },
    { page: '+', label: 'Log food' },
    { page: 'trends', label: 'Trends' },
    { page: 'profile', label: 'Profile' },
  ];
  // Pages reachable from the phone's Profile tab count as "Profile" for the highlight.
  const tabFor: Record<string, string> = { foods: 'recipes', goals: 'profile', weight: 'trends', log: 'today' };
  const activeTab = $derived(tabFor[page] ?? page);

  // Today's diary feeds the dashboard and the streak, and is refreshed after every change.
  $effect(() => {
    void app.version;
    void app.userId;
    if (app.loaded) app.loadToday();
  });

  const streak = $derived(app.today ? (app.today.goal?.calories != null ? app.today.streak.on_budget : app.today.streak.logged) : null);
  const streakHint = $derived(
    app.today?.goal?.calories != null ? 'Days in a row within your calorie budget' : 'Days in a row with something logged',
  );

  function onKey(e: KeyboardEvent) {
    if (e.key !== '/' || e.metaKey || e.ctrlKey || e.altKey) return;
    const t = e.target as HTMLElement;
    if (t.closest('input, textarea, select, [contenteditable], dialog')) return;
    if (!app.user) return;
    e.preventDefault();
    app.openAdd();
  }
</script>

<svelte:window onkeydown={onKey} />

{#if DEMO}<DemoBanner />{/if}

{#if app.loadError}
  <div class="center">
    <div class="card stack" style="max-width: 420px">
      <h2>Can't load Calorie Man</h2>
      <p class="muted">{app.loadError}</p>
      <button class="btn primary" onclick={() => location.reload()}>Retry</button>
    </div>
  </div>
{:else if !app.loaded}
  <div class="center muted">Loading…</div>
{:else if app.users.length === 0 || !app.user}
  <Onboarding />
{:else}
  <div class="shell">
    <aside class="sidebar">
      <a href="#/today" class="brand" aria-label="Calorie Man home"><Logo /></a>
      <UserSwitcher />
      <nav aria-label="Main">
        {#each nav as n}
          <a href="#/{n.page}" class="terrace-sm" class:active={page === n.page} aria-current={page === n.page ? 'page' : undefined}>{n.label}</a>
        {/each}
      </nav>
      <div class="streak card" title={streakHint} data-testid="sidebar-streak">
        <div class="display n">{streak ?? '–'}</div>
        <div class="eyebrow">Day streak</div>
      </div>
    </aside>

    <main>
      {#key app.userId}
        {#if page === 'today'}
          <Today />
        {:else if page === 'log'}
          <Log />
        {:else if page === 'foods' || page === 'recipes'}
          <Foods kind={page === 'recipes' ? 'recipe' : 'food'} />
        {:else if page === 'trends'}
          <Trends />
        {:else if page === 'weight'}
          <WeightPage />
        {:else if page === 'goals'}
          <Goals />
        {:else if page === 'profile'}
          <Profile />
        {/if}
      {/key}
    </main>
  </div>

  <nav class="tabbar" aria-label="Main">
    {#each tabs as t}
      {#if t.page === '+'}
        <div class="fab-wrap">
          <button class="fab oct" aria-label="Log food" data-testid="fab" onclick={() => app.openAdd()}>+</button>
        </div>
      {:else}
        <button class:active={activeTab === t.page} onclick={() => navigate(t.page)} aria-current={activeTab === t.page ? 'page' : undefined}>
          <span class="glyph" aria-hidden="true">
            {#if activeTab === t.page}<i style="width:6px"></i><i style="width:12px"></i><i style="width:18px"></i>{/if}
          </span>
          <span>{t.label}</span>
        </button>
      {/if}
    {/each}
  </nav>

  <AddFoodDialog />
{/if}

<Toasts />

<style>
  .center {
    min-height: calc(100dvh - var(--banner-h, 0px));
    display: grid;
    place-items: center;
    padding: 20px;
  }
  .shell {
    display: grid;
    grid-template-columns: 232px minmax(0, 1fr);
    min-height: calc(100dvh - var(--banner-h, 0px));
  }
  .sidebar {
    position: sticky;
    top: var(--banner-h, 0px);
    height: calc(100dvh - var(--banner-h, 0px));
    padding: 30px 18px 24px;
    border-right: 1px solid var(--line);
    background: var(--sidebar);
    display: flex;
    flex-direction: column;
    gap: 22px;
    overflow-y: auto;
  }
  .brand {
    color: var(--ink);
    text-decoration: none;
    display: flex;
    justify-content: center;
  }
  .sidebar nav {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 15px;
  }
  .sidebar nav a {
    height: 44px;
    display: flex;
    align-items: center;
    padding: 0 18px;
    color: var(--muted);
    text-decoration: none;
  }
  .sidebar nav a:hover {
    background: var(--hover);
    color: var(--ink);
  }
  .sidebar nav a.active {
    background: var(--nav-active-bg);
    color: var(--nav-active-ink);
    font-weight: 600;
  }
  .sidebar nav a:focus-visible {
    outline: none;
    box-shadow: inset 0 0 0 2px var(--focus);
  }
  .streak {
    margin-top: auto;
    text-align: center;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    background: var(--surface);
  }
  .streak .n {
    font-size: 40px;
  }
  main {
    padding: 30px 32px 40px;
    min-width: 0;
    max-width: 1400px;
  }
  .tabbar {
    display: none;
  }
  @media (max-width: 899px) {
    .shell {
      display: block;
    }
    .sidebar {
      display: none;
    }
    main {
      padding: 14px 14px calc(110px + env(safe-area-inset-bottom));
    }
    .tabbar {
      display: grid;
      grid-template-columns: repeat(5, minmax(0, 1fr));
      align-items: center;
      position: fixed;
      left: 0;
      right: 0;
      bottom: 0;
      z-index: 30;
      background: var(--surface);
      border-top: 1px solid var(--line);
      padding: 8px 6px calc(10px + env(safe-area-inset-bottom));
    }
    .tabbar button {
      display: flex;
      flex-direction: column;
      align-items: center;
      gap: 6px;
      border: 0;
      background: none;
      color: var(--muted);
      font-size: 10px;
      font-weight: 600;
      letter-spacing: 0.12em;
      text-transform: uppercase;
      cursor: pointer;
      padding: 4px 0;
    }
    .tabbar button.active {
      color: var(--ink);
    }
    .glyph {
      height: 12px;
      display: flex;
      flex-direction: column;
      align-items: center;
      gap: 1.5px;
    }
    .glyph i {
      display: block;
      height: 3px;
      background: var(--peach);
    }
    .fab-wrap {
      display: grid;
      place-items: center;
    }
    .tabbar .fab {
      display: grid;
      place-items: center;
      width: 52px;
      height: 52px;
      padding: 0;
      border: 0;
      background: var(--solid);
      color: var(--solid-ink);
      font-size: 28px;
      font-weight: 300;
      letter-spacing: 0;
      line-height: 1;
      cursor: pointer;
    }
    .tabbar .fab:focus-visible {
      outline: none;
      background: var(--peach);
      color: var(--on-pastel);
    }
  }
</style>
