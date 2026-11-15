<script lang="ts">
  import { api } from '../lib/api';
  import { app } from '../lib/state.svelte';
  import Avatar from './Avatar.svelte';
  import Dialog from './Dialog.svelte';

  /** `compact` shows only the avatar (phone header); the menu opens to the left. */
  let { compact = false }: { compact?: boolean } = $props();

  let open = $state(false);
  let adding = $state(false);
  let newName = $state('');
  let busy = $state(false);
  let root: HTMLElement | undefined = $state();

  function onWindowClick(e: MouseEvent) {
    if (open && root && !root.contains(e.target as Node)) open = false;
  }

  function choose(id: number) {
    app.selectUser(id);
    open = false;
    app.toast(`Switched to ${app.user?.name}`);
  }

  async function add(e: SubmitEvent) {
    e.preventDefault();
    if (!newName.trim()) return;
    busy = true;
    try {
      const u = await api.createUser({ name: newName.trim() });
      await app.reloadUsers();
      app.selectUser(u.id);
      adding = false;
      newName = '';
      app.toast(`Added ${u.name}. Set goals in Settings.`);
    } catch (err) {
      app.error(err);
    } finally {
      busy = false;
    }
  }
</script>

<svelte:window onclick={onWindowClick} />

<div class="switcher" class:compact bind:this={root}>
  <button
    class="current"
    type="button"
    aria-haspopup="menu"
    aria-expanded={open}
    aria-label={compact ? `Switch person (now ${app.user?.name})` : undefined}
    data-testid={compact ? 'user-switcher-compact' : 'user-switcher'}
    onclick={() => (open = !open)}
  >
    {#if app.user}
      <Avatar name={app.user.name} color={app.user.color} size={compact ? 32 : 28} />
      {#if !compact}<span class="name">{app.user.name}</span>{/if}
    {/if}
    {#if !compact}<span class="caret" aria-hidden="true">▾</span>{/if}
  </button>
  {#if open}
    <div class="menu" role="menu">
      {#each app.users as u (u.id)}
        <button
          class="opt"
          class:active={u.id === app.userId}
          role="menuitemradio"
          aria-checked={u.id === app.userId}
          data-testid="user-option"
          onclick={() => choose(u.id)}
        >
          <Avatar name={u.name} color={u.color} size={26} />
          <span>{u.name}</span>
          {#if u.id === app.userId}<span class="tick">✓</span>{/if}
        </button>
      {/each}
      <button
        class="opt add"
        role="menuitem"
        onclick={() => {
          open = false;
          adding = true;
        }}>+ Add person</button
      >
    </div>
  {/if}
</div>

<Dialog bind:open={adding} title="Add person" testid="add-user-dialog">
  <form id="add-user-form" onsubmit={add} class="stack">
    <label class="field"><span>Name</span><input class="input" bind:value={newName} maxlength="40" required /></label>
    <p class="muted small">Everyone shares the food library and recipes; diaries, goals and weights are separate.</p>
  </form>
  {#snippet footer()}
    <button class="btn" type="button" onclick={() => (adding = false)}>Cancel</button>
    <button class="btn primary" type="submit" form="add-user-form" disabled={busy}>Add</button>
  {/snippet}
</Dialog>

<style>
  .switcher {
    position: relative;
  }
  .current {
    --t: 5px;
    clip-path: var(--terrace);
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 6px 12px 6px 8px;
    border: 0;
    background: var(--surface);
    cursor: pointer;
    font-weight: 600;
    color: var(--ink);
  }
  .current:focus-visible {
    outline: none;
    box-shadow: inset 0 0 0 2px var(--focus);
  }
  .compact .current {
    clip-path: none;
    background: none;
    padding: 0;
    width: auto;
  }
  .compact .menu {
    left: auto;
    right: 0;
    width: 220px;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .name {
    flex: 1;
    text-align: left;
  }
  .caret {
    color: var(--muted);
    font-size: 0.8rem;
  }
  .menu {
    --t: 6px;
    clip-path: var(--terrace);
    position: absolute;
    left: 0;
    right: 0;
    top: calc(100% + 6px);
    background: var(--surface);
    padding: 8px;
    min-width: 196px;
    z-index: 50;
    filter: drop-shadow(0 10px 30px rgba(0, 0, 0, 0.25));
  }
  .opt {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 8px;
    border: 0;
    background: none;
    cursor: pointer;
    text-align: left;
    font-weight: 550;
    color: var(--ink);
  }
  .opt:hover,
  .opt.active {
    background: var(--card-2);
  }
  .tick {
    margin-left: auto;
    color: var(--peach-ink);
  }
  .add {
    color: var(--peach-ink);
    border-top: 1px solid var(--line);
    margin-top: 4px;
  }
</style>
