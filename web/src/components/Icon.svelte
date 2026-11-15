<script lang="ts">
  let { name, size = 20 }: { name: string; size?: number } = $props();
  const paths: Record<string, string> = {
    today: 'M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18Zm0 4a5 5 0 1 1 0 10 5 5 0 0 1 0-10Z',
    trends: 'M4 19V5M4 19h16M8 15l4-5 3 3 5-6',
    weight: 'M5 7h14l-1.5 12h-11ZM9 7a3 3 0 0 1 6 0M12 11v3',
    foods: 'M6 4h10a2 2 0 0 1 2 2v14H8a2 2 0 0 1-2-2ZM6 18a2 2 0 0 1 2-2h10M10 8h5',
    settings: 'M12 9a3 3 0 1 0 0 6 3 3 0 0 0 0-6ZM19 12l2-1-1-3-2 .2-1.3-1.4.3-2-3-1-1 2h-2l-1-2-3 1 .3 2L6 7.2 4 7 3 10l2 1v2l-2 1 1 3 2-.2 1.3 1.4-.3 2 3 1 1-2h2l1 2 3-1-.3-2 1.3-1.4 2 .2 1-3-2-1Z',
    plus: 'M12 5v14M5 12h14',
    left: 'M15 6l-6 6 6 6',
    right: 'M9 6l6 6-6 6',
    copy: 'M8 8h11v11H8zM5 16V5h11',
    search: 'M11 4a7 7 0 1 0 0 14 7 7 0 0 0 0-14ZM20 20l-4-4',
  };
</script>

<svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
  <path d={paths[name] ?? ''} />
</svg>
