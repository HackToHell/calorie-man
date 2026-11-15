<script lang="ts">
  let { name, color, size = 28 }: { name: string; color: string; size?: number } = $props();
  const initial = $derived((name.trim()[0] ?? '?').toUpperCase());
</script>

<span class="avatar" style:background={color} style:width="{size}px" style:height="{size}px" style:font-size="{size * 0.46}px" aria-hidden="true"
  >{initial}</span
>

<style>
  .avatar {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    clip-path: var(--octagon);
    color: #fff;
    font-weight: 700;
    flex: none;
  }
</style>
