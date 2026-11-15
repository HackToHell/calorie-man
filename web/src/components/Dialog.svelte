<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    open = $bindable(false),
    title,
    wide = false,
    onclose,
    children,
    footer,
    testid,
  }: {
    open?: boolean;
    title: string;
    wide?: boolean;
    onclose?: () => void;
    children: Snippet;
    footer?: Snippet;
    testid?: string;
  } = $props();

  let el: HTMLDialogElement | undefined = $state();

  $effect(() => {
    if (!el) return;
    if (open && !el.open) el.showModal();
    if (!open && el.open) el.close();
  });

  function closed() {
    if (open) {
      open = false;
      onclose?.();
    }
  }

  function backdrop(e: MouseEvent) {
    if (e.target === el) closed();
  }
</script>

<dialog bind:this={el} class:wide onclose={closed} onclick={backdrop} data-testid={testid} aria-label={title}>
  {#if open}
    <div class="inner">
      <header>
        <h2>{title}</h2>
        <button class="btn ghost icon" type="button" aria-label="Close" onclick={closed}>✕</button>
      </header>
      <div class="body">
        {@render children()}
      </div>
      {#if footer}
        <footer>{@render footer()}</footer>
      {/if}
    </div>
  {/if}
</dialog>

<style>
  dialog {
    --t: 8px;
    clip-path: var(--terrace);
    border: 0;
    padding: 0;
    width: min(540px, calc(100vw - 20px));
    max-height: calc(100dvh - 40px);
    background: var(--surface);
    color: var(--ink);
  }
  dialog.wide {
    width: min(760px, calc(100vw - 20px));
  }
  dialog::backdrop {
    background: var(--scrim);
  }
  .inner {
    display: flex;
    flex-direction: column;
    max-height: calc(100dvh - 42px);
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 16px 10px 22px;
    border-bottom: 1px solid var(--line);
  }
  header h2 {
    font-family: var(--display);
    font-weight: 400;
    font-size: 22px;
  }
  .body {
    padding: 16px 22px;
    overflow-y: auto;
    flex: 1;
  }
  footer {
    display: flex;
    gap: 8px;
    justify-content: flex-end;
    padding: 12px 22px 16px;
    border-top: 1px solid var(--line);
    flex-wrap: wrap;
  }
  @media (max-width: 640px) {
    dialog {
      width: 100vw;
      max-width: 100vw;
      max-height: 100dvh;
      height: 100dvh;
      margin: 0;
      clip-path: none;
    }
    dialog.wide {
      width: 100vw;
    }
    .inner {
      max-height: 100dvh;
      height: 100%;
    }
  }
</style>
