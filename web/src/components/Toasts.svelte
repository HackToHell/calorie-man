<script lang="ts">
  import { app } from '../lib/state.svelte';
</script>

<div class="toasts" aria-live="polite">
  {#each app.toasts as t (t.id)}
    <div class="toast {t.kind}" role={t.kind === 'error' ? 'alert' : 'status'} data-testid="toast">{t.text}</div>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    left: 50%;
    bottom: calc(100px + env(safe-area-inset-bottom));
    transform: translateX(-50%);
    display: flex;
    flex-direction: column;
    gap: 8px;
    z-index: 1000;
    width: min(460px, calc(100vw - 24px));
    pointer-events: none;
  }
  .toast {
    --t: 5px;
    clip-path: var(--terrace);
    pointer-events: auto;
    background: var(--solid);
    color: var(--solid-ink);
    padding: 11px 16px;
    font-size: 0.9rem;
    font-weight: 500;
  }
  .toast.error {
    background: var(--danger);
    color: #fff;
  }
  .toast.warn {
    background: var(--peach);
    color: var(--on-pastel);
  }
  @media (min-width: 900px) {
    .toasts {
      bottom: 24px;
    }
  }
</style>
