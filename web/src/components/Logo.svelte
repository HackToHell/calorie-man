<script lang="ts">
  let { size = 26, stacked = true }: { size?: number; stacked?: boolean } = $props();
</script>

<div class="logo" class:stacked>
  <div class="mark" aria-hidden="true">
    <i style="width:10px;background:var(--lilac)"></i>
    <i style="width:20px;background:var(--peach)"></i>
    <i style="width:30px;background:var(--sea)"></i>
  </div>
  <div class="word display" style:font-size="{size}px">Calorie Man</div>
</div>

<style>
  .logo {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .logo.stacked {
    flex-direction: column;
    gap: 8px;
  }
  .mark {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
  }
  .mark i {
    display: block;
    height: 4px;
  }
  .word {
    white-space: nowrap;
  }
</style>
