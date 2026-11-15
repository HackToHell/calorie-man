<script lang="ts">
  import { REPO_URL } from '../lib/env';

  // Shown on every load of the GitHub Pages demo, so nobody expects their entries to stick around.
  let hidden = $state(false);
  let height = $state(0);
  // The sticky sidebar sits below the banner.
  $effect(() => {
    document.documentElement.style.setProperty('--banner-h', `${hidden ? 0 : height}px`);
  });
</script>

{#if !hidden}
  <div class="demo-banner" role="region" aria-label="Demo notice" data-testid="demo-banner" bind:clientHeight={height}>
    <span class="tag">Live demo</span>
    <p>
      <span class="long">Everything runs in your browser on sample data for Alex and Sam. <b>Nothing is saved</b>: reloading the page starts over.</span>
      <span class="short"><b>Nothing is saved</b>: reloading starts over.</span>
    </p>
    <a href={REPO_URL} target="_blank" rel="noreferrer">Self-host it <span aria-hidden="true">→</span></a>
    <button type="button" aria-label="Hide the demo notice" title="Hide until the next reload" onclick={() => (hidden = true)}>×</button>
  </div>
{/if}

<style>
  .demo-banner {
    position: sticky;
    top: 0;
    z-index: 40;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 12px 8px 16px;
    background: var(--butter);
    color: var(--on-pastel);
    font-size: 14px;
    line-height: 1.35;
    border-bottom: 1px solid oklch(0.22 0.04 315 / 0.12);
  }
  .tag {
    flex: none;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    padding: 3px 8px;
    background: var(--on-pastel);
    color: var(--butter);
    clip-path: var(--terrace);
    --t: 3px;
  }
  p {
    flex: 1;
    min-width: 0;
    margin: 0;
  }
  .short {
    display: none;
  }
  a {
    flex: none;
    color: inherit;
    font-weight: 600;
    text-underline-offset: 3px;
  }
  button {
    flex: none;
    width: 28px;
    height: 28px;
    border: 0;
    background: transparent;
    color: inherit;
    font-size: 20px;
    line-height: 1;
    cursor: pointer;
  }
  button:hover {
    background: oklch(0.22 0.04 315 / 0.08);
  }
  @media (max-width: 760px) {
    .demo-banner {
      gap: 8px;
      padding: 6px 6px 6px 10px;
      font-size: 13px;
    }
    .long {
      display: none;
    }
    .short {
      display: inline;
    }
    /* One line on phones; Profile links to the project too. */
    a {
      display: none;
    }
  }
</style>
