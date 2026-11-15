<script lang="ts">
  import FoodEditor from '../components/FoodEditor.svelte';
  import OnlineSearch from '../components/OnlineSearch.svelte';
  import RecipeEditor from '../components/RecipeEditor.svelte';
  import BarcodeLookup from '../components/BarcodeLookup.svelte';
  import { api } from '../lib/api';
  import { kcal, num } from '../lib/format';
  import PageHeader from '../components/PageHeader.svelte';
  import { app, navigate } from '../lib/state.svelte';
  import type { Food } from '../lib/types';

  /** Food bank (kind=food) and Recipes (kind=recipe) share this page. */
  let { kind = 'food' }: { kind?: 'food' | 'recipe' } = $props();
  const base = $derived(kind === 'recipe' ? 'recipes' : 'foods');
  const sub = $derived(app.route.parts[0] ?? '');
  const editId = $derived(/^\d+$/.test(sub) ? Number(sub) : null);

  let q = $state('');
  let archived = $state(false);
  let list = $state<Food[]>([]);
  let loading = $state(false);
  let editing = $state<Food | null>(null);
  let editErr = $state<string | null>(null);
  let seq = 0;

  $effect(() => {
    void app.version;
    if (sub) return;
    const [query, k, arch] = [q.trim(), kind, archived];
    const my = ++seq;
    loading = true;
    const t = setTimeout(
      () =>
        api
          .foods({ q: query, kind: k, archived: arch, limit: 100, user_id: app.userId ?? undefined })
          .then((r) => {
            if (my === seq) list = arch ? r.filter((x) => x.archived) : r;
          })
          .catch((e) => app.error(e))
          .finally(() => my === seq && (loading = false)),
      query ? 150 : 0,
    );
    return () => clearTimeout(t);
  });

  $effect(() => {
    const id = editId;
    editing = null;
    editErr = null;
    if (id == null) return;
    api
      .food(id)
      .then((f) => (editing = f))
      .catch((e) => (editErr = (e as Error).message));
  });

  function done() {
    navigate(base);
  }

  async function restore(f: Food) {
    try {
      await api.restoreFood(f.id);
      archived = false; // back to the normal list, where the restored food now is
      app.toast(`Restored ${f.name}`);
      app.touch();
      editing = await api.food(f.id);
    } catch (e) {
      app.error(e);
    }
  }

  const sourceLabel: Record<string, string> = { usda: 'USDA', off: 'Open Food Facts', ai: 'AI', manual: 'manual', recipe: 'recipe' };
</script>

{#if sub === 'new' && kind === 'food'}
  <div class="card">
    <div class="card-head"><h1>New food</h1><button class="btn ghost" onclick={done}>Close</button></div>
    <FoodEditor onsaved={(f) => (app.toast(`Saved ${f.name}`), navigate(`foods/${f.id}`))} oncancel={done} />
  </div>
{:else if (sub === 'new' && kind === 'recipe') || sub === 'new-recipe'}
  <div class="card">
    <div class="card-head"><h1>New recipe</h1><button class="btn ghost" onclick={done}>Close</button></div>
    <RecipeEditor onsaved={(f) => (app.toast(`Saved recipe ${f.name}`), done())} oncancel={done} />
  </div>
{:else if sub === 'import'}
  <div class="card">
    <div class="card-head"><h1>Add from online databases</h1><button class="btn ghost" onclick={done}>Close</button></div>
    <OnlineSearch onpick={(f) => navigate(`foods/${f.id}`)} />
  </div>
  <div class="card">
    <h2>Barcode</h2>
    <BarcodeLookup onpick={(f) => navigate(`foods/${f.id}`)} />
  </div>
{:else if editId != null}
  <div class="card">
    <div class="card-head">
      <h1>{editing?.kind === 'recipe' ? 'Edit recipe' : 'Edit food'}</h1>
      <button class="btn ghost" onclick={done}>Close</button>
    </div>
    {#if editErr}<div class="banner error">{editErr}</div>{/if}
    {#if editing}
      {#if editing.archived}
        <div class="banner row">This food is archived: it is hidden from search but kept for history. <button class="btn small" onclick={() => restore(editing!)}>Restore</button></div>
      {/if}
      {#key editing.id}
        {#if editing.kind === 'recipe'}
          <RecipeEditor recipe={editing} onsaved={(f) => (app.toast(`Saved ${f.name}`), done())} oncancel={done} ondeleted={done} />
        {:else}
          <FoodEditor food={editing} onsaved={(f) => (app.toast(`Saved ${f.name}`), done())} oncancel={done} ondeleted={done} />
        {/if}
      {/key}
    {/if}
  </div>
{:else}
  <PageHeader title={kind === 'recipe' ? 'Recipes' : 'Food bank'} eyebrow="Shared by everyone" />
  <div class="row wrap toolbar">
    <div class="tabs kinds">
      <a class="tab" class:active={kind === 'food'} href="#/foods">Food bank</a>
      <a class="tab" class:active={kind === 'recipe'} href="#/recipes">Recipes</a>
    </div>
    <span class="spacer"></span>
    <a class="btn" href="#/foods/import" data-testid="foods-import">Online / barcode</a>
    <a class="btn" class:primary={kind === 'recipe'} href="#/recipes/new" data-testid="new-recipe">New recipe</a>
    <a class="btn" class:primary={kind === 'food'} href="#/foods/new" data-testid="new-food">New food</a>
  </div>
  <div class="card">
    <div class="row wrap filters">
      <input class="input search" type="search" placeholder="Search name, brand, search words, barcode" bind:value={q} data-testid="foods-search" />
      <label class="check small"><input type="checkbox" bind:checked={archived} /> Archived</label>
    </div>
    {#if list.length}
      <ul class="list" data-testid="foods-list">
        {#each list as f (f.id)}
          <li>
            <a class="item" href="#/{base}/{f.id}" data-testid="food-row">
              <div class="grow">
                <div class="ellipsis">
                  <b>{f.name}</b>{#if f.brand}<span class="muted">{' · '}{f.brand}</span>{/if}
                </div>
                <div class="small muted ellipsis">
                  {#if f.per_serving}{kcal(f.per_serving.calories)} kcal · {num(f.per_serving.protein, 1)} g protein per {f.per_serving.label}{:else}{kcal(
                      f.nutrients.calories,
                    )} kcal · {num(f.nutrients.protein, 1)} g protein per {num(f.basis_amount)} {f.basis_unit}{/if}
                </div>
              </div>
              <div class="tags">
                {#if f.kind === 'recipe'}<span class="chip accent">recipe</span>{:else}<span class="chip">{sourceLabel[f.source] ?? f.source}</span>{/if}
                {#if f.uses}<span class="chip">{f.uses}×</span>{/if}
              </div>
            </a>
          </li>
        {/each}
      </ul>
    {:else if !loading}
      <div class="empty">
        {#if q.trim()}No foods match “{q.trim()}”. <a href="#/foods/import">Search online</a> or <a href="#/{base}/new">create it</a>.{:else if archived}Nothing archived.{:else if kind === 'recipe'}No recipes yet. <a href="#/recipes/new">Build one</a> from foods in the food bank,
          like oats + milk + banana.{:else}The food bank is empty. Add foods from the online databases, or create your own.{/if}
      </div>
    {/if}
  </div>
{/if}

<style>
  .toolbar {
    margin-bottom: 16px;
  }
  .kinds .tab {
    text-decoration: none;
  }
  .filters {
    margin-bottom: 8px;
  }
  .search {
    flex: 1;
    min-width: 200px;
  }
  .item {
    color: inherit;
    text-decoration: none;
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .tags {
    display: flex;
    gap: 4px;
    flex: none;
  }
</style>
