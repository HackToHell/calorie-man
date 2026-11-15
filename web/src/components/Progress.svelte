<script lang="ts">
  let {
    value,
    max,
    color = 'var(--peach)',
    limit = false,
    height = 8,
    label,
  }: { value: number; max: number | null | undefined; color?: string; limit?: boolean; height?: number; label?: string } = $props();

  const pct = $derived(max && max > 0 ? Math.max(0, (value / max) * 100) : 0);
  const over = $derived(pct > 100.5);
  const fill = $derived(over && limit ? 'var(--over)' : color);
</script>

<div
  class="bar"
  style:height="{height}px"
  role="progressbar"
  aria-label={label}
  aria-valuemin="0"
  aria-valuemax={max ?? 0}
  aria-valuenow={Math.round(value)}
>
  <div class="fill" style:width="{Math.min(pct, 100)}%" style:background={fill}></div>
  {#if over && !limit}
    <div class="over" style:background={color}></div>
  {/if}
</div>

<style>
  .bar {
    position: relative;
    width: 100%;
    background: var(--track);
    overflow: hidden;
  }
  .fill {
    height: 100%;
    transition: width 0.3s ease;
  }
  .over {
    position: absolute;
    right: 0;
    top: 0;
    bottom: 0;
    width: 4px;
    opacity: 0.5;
  }
</style>
