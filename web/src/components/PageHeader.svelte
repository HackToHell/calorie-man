<script lang="ts">
  import type { Snippet } from 'svelte';
  import { app } from '../lib/state.svelte';
  import Logo from './Logo.svelte';
  import UserSwitcher from './UserSwitcher.svelte';

  let {
    eyebrow,
    title,
    home = false,
    actions,
    search = true,
  }: { eyebrow?: string; title: string; home?: boolean; actions?: Snippet; search?: boolean } = $props();
</script>

<header class="ph" class:home>
  <div class="phone-top">
    {#if home}
      <div class="home-brand">
        <Logo size={30} />
        {#if eyebrow}<div class="eyebrow">{eyebrow}</div>{/if}
      </div>
    {:else}
      <h1 class="ptitle">{title}</h1>
    {/if}
    {#if app.user}
      <div class="me"><UserSwitcher compact /></div>
    {/if}
  </div>
  <div class="desk">
    <div class="titles">
      {#if eyebrow}<div class="eyebrow e" data-testid="page-eyebrow">{eyebrow}</div>{/if}
      <h1>{title}</h1>
    </div>
    {#if search}
      <button class="search terrace-sm" onclick={() => app.openAdd()} data-testid="header-search">
        <span class="mag" aria-hidden="true"></span>
        <span class="grow">Search foods &amp; recipes</span>
        <kbd>/</kbd>
      </button>
      <button class="btn primary log" onclick={() => app.openAdd()} data-testid="add-food">+ Log food</button>
    {/if}
  </div>
  {#if actions}<div class="actions">{@render actions()}</div>{/if}
</header>

<style>
  .ph {
    margin-bottom: 22px;
  }
  .desk {
    display: flex;
    align-items: flex-end;
    gap: 14px;
  }
  .titles {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }
  .e {
    font-size: 12px;
    letter-spacing: 0.18em;
  }
  h1 {
    font-size: 40px;
    line-height: 1.1;
  }
  .search {
    width: 340px;
    height: 48px;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 0 14px 0 18px;
    border: 0;
    background: var(--surface);
    color: var(--muted);
    font-size: 14px;
    cursor: text;
    text-align: left;
  }
  .search:focus-visible {
    outline: none;
    box-shadow: inset 0 0 0 2px var(--focus);
  }
  .grow {
    flex: 1;
  }
  kbd {
    font: inherit;
    font-size: 12px;
  }
  .mag {
    position: relative;
    width: 16px;
    height: 16px;
    flex: none;
  }
  .mag::before {
    content: '';
    position: absolute;
    left: 0;
    top: 0;
    width: 12px;
    height: 12px;
    border: 1.75px solid currentColor;
    border-radius: 50%;
    box-sizing: border-box;
  }
  .mag::after {
    content: '';
    position: absolute;
    left: 10px;
    top: 11px;
    width: 6px;
    height: 1.75px;
    background: currentColor;
    transform: rotate(45deg);
    transform-origin: left center;
  }
  .log {
    height: 48px;
    padding: 0 24px;
  }
  .actions {
    margin-top: 14px;
  }
  .phone-top {
    display: none;
  }
  @media (max-width: 1100px) {
    .search {
      width: 220px;
    }
  }
  @media (max-width: 899px) {
    .desk {
      display: none;
    }
    .phone-top {
      display: flex;
      align-items: center;
      gap: 10px;
      position: relative;
    }
    .home .phone-top {
      justify-content: center;
      padding-top: 6px;
    }
    .home-brand {
      display: flex;
      flex-direction: column;
      align-items: center;
      gap: 6px;
    }
    .ptitle {
      flex: 1;
      font-size: 30px;
    }
    .me {
      margin-left: auto;
      display: flex;
    }
    .home .me {
      position: absolute;
      right: 0;
      top: 4px;
    }
    .ph {
      margin-bottom: 14px;
    }
  }
</style>
