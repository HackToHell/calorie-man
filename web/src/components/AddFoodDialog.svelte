<script lang="ts">
  import { api } from '../lib/api';
  import { MEAL_LABELS, defaultTime, kcal, relDay } from '../lib/format';
  import { pickOption, portionBody, unitOptions } from '../lib/portion';
  import { app } from '../lib/state.svelte';
  import type { Food, RecentFood } from '../lib/types';
  import BarcodeLookup from './BarcodeLookup.svelte';
  import Dialog from './Dialog.svelte';
  import FoodEditor from './FoodEditor.svelte';
  import FoodSearch from './FoodSearch.svelte';
  import OnlineSearch from './OnlineSearch.svelte';
  import PortionPicker from './PortionPicker.svelte';

  // Opened from anywhere through app.openAdd(); see lib/state.svelte.ts.
  const date = $derived(app.add.date);
  let meal = $state('breakfast');
  const onadded = () => app.touch();

  type Tab = 'library' | 'online' | 'barcode' | 'quick' | 'new';
  let tab = $state<Tab>('library');
  let query = $state('');
  let onlineQuery = $state('');
  let selected = $state<Food | null>(null);
  let quantity = $state<number | string>(1);
  let optValue = $state('');
  let note = $state('');
  let time = $state('');
  let timeTouched = $state(false);
  let busy = $state(false);
  let added = $state<string[]>([]);
  let newBarcode = $state('');

  // quick add
  let qName = $state('');
  let qCal = $state('');
  let qP = $state('');
  let qC = $state('');
  let qF = $state('');

  let lastSeq = 0;
  $effect(() => {
    const req = app.add;
    if (!req.open || req.seq === lastSeq) return;
    lastSeq = req.seq;
    tab = 'library';
    selected = null;
    query = req.query;
    added = [];
    meal = req.meal;
    newBarcode = '';
    timeTouched = false;
    if (req.food) {
      pick(req.food, req.quantity != null ? ({ last_quantity: req.quantity, last_serving_id: req.servingId, last_unit: req.unit ?? '' } as RecentFood) : undefined);
    }
  });

  // Until the user edits it, the time follows the meal: now for the current meal, else its usual time.
  $effect(() => {
    const [d, m, usual] = [date, meal, app.today?.timing.usual];
    if (!timeTouched) time = defaultTime(d, m, usual);
  });

  function pick(f: Food, r?: RecentFood) {
    selected = f;
    const opts = unitOptions(f);
    const o = r ? pickOption(f, opts, r.last_serving_id, r.last_unit) : pickOption(f, opts);
    optValue = o.value;
    quantity = r ? r.last_quantity : o.unit && o.unit !== 'serving' ? 100 : 1;
    note = '';
  }

  async function addSelected(e: SubmitEvent) {
    e.preventDefault();
    if (!selected || app.userId == null) return;
    const q = Number(quantity);
    if (!Number.isFinite(q) || q <= 0) return;
    const opts = unitOptions(selected);
    const o = opts.find((x) => x.value === optValue) ?? opts[0];
    busy = true;
    try {
      const entry = await api.addEntry(app.userId, {
        date,
        meal,
        food_id: selected.id,
        ...portionBody(o, q),
        note: note.trim() || null,
        time: time || null,
      });
      added = [...added, `${entry.name} (${kcal(entry.nutrients.calories)} kcal)`];
      selected = null;
      query = '';
      tab = 'library';
      onadded();
    } catch (err) {
      app.error(err);
    } finally {
      busy = false;
    }
  }

  async function quickAdd(e: SubmitEvent) {
    e.preventDefault();
    if (app.userId == null) return;
    const n: Record<string, number> = {};
    for (const [k, v] of [
      ['calories', qCal],
      ['protein', qP],
      ['carbs', qC],
      ['fat', qF],
    ] as const) {
      if (v.trim() !== '') n[k] = Number(v);
    }
    busy = true;
    try {
      const entry = await api.addEntry(app.userId, { date, meal, name: qName.trim() || 'Quick add', nutrients: n, time: time || null });
      added = [...added, `${entry.name} (${kcal(entry.nutrients.calories)} kcal)`];
      qName = qCal = qP = qC = qF = '';
      onadded();
    } catch (err) {
      app.error(err);
    } finally {
      busy = false;
    }
  }

  const tabs: [Tab, string][] = [
    ['library', 'Library'],
    ['online', 'Online'],
    ['barcode', 'Barcode'],
    ['quick', 'Quick add'],
    ['new', 'New food'],
  ];
</script>

<Dialog bind:open={app.add.open} title="Add to {MEAL_LABELS[meal] ?? meal} · {relDay(date)}" wide testid="add-food-dialog">
  {#if added.length}
    <div class="banner ok small added" data-testid="added-banner">Added: {added.join(', ')}</div>
  {/if}

  {#if selected}
    <form class="stack" onsubmit={addSelected} data-testid="portion-form">
      <div class="row">
        <button type="button" class="btn ghost small" onclick={() => (selected = null)}>← Back</button>
        <div class="grow">
          <div><b>{selected.name}</b>{#if selected.brand}<span class="muted">{' · '}{selected.brand}</span>{/if}</div>
          {#if selected.kind === 'recipe'}<div class="tiny muted">Recipe · {selected.ingredients?.length ?? 0} ingredients</div>{/if}
        </div>
      </div>
      <PortionPicker food={selected} bind:quantity bind:optValue />
      <div class="mtn">
        <label class="field"
          ><span>Meal</span>
          <select class="input" bind:value={meal} data-testid="meal-select">
            {#each Object.entries(MEAL_LABELS) as [k, l]}<option value={k}>{l}</option>{/each}
          </select>
        </label>
        <label class="field"
          ><span>Time</span>
          <input class="input" type="time" bind:value={time} oninput={() => (timeTouched = true)} data-testid="entry-time-input" />
        </label>
        <label class="field note"><span>Note</span><input class="input" bind:value={note} maxlength="500" placeholder="optional" /></label>
      </div>
      <button class="btn primary" disabled={busy || !(Number(quantity) > 0)} data-testid="add-entry">Add to diary</button>
    </form>
  {:else}
    <div class="stack">
      <div class="tabs" role="tablist">
        {#each tabs as [k, label]}
          <button
            type="button"
            class="tab"
            class:active={tab === k}
            role="tab"
            aria-selected={tab === k}
            onclick={() => {
              if (k === 'online' && !onlineQuery) onlineQuery = query;
              tab = k;
            }}
            data-testid="tab-{k}">{label}</button
          >
        {/each}
      </div>

      {#if tab === 'library'}
        <FoodSearch onpick={pick} bind:query />
        {#if query.trim()}
          <div class="row wrap small">
            <span class="muted">Not here?</span>
            <button
              type="button"
              class="btn small"
              onclick={() => {
                onlineQuery = query;
                tab = 'online';
              }}>Search online</button
            >
            <button type="button" class="btn small" onclick={() => (tab = 'new')}>Create “{query.trim()}”</button>
          </div>
        {/if}
      {:else if tab === 'online'}
        <OnlineSearch onpick={(f) => pick(f)} bind:query={onlineQuery} />
      {:else if tab === 'barcode'}
        <BarcodeLookup
          onpick={(f) => pick(f)}
          oncreate={(b) => {
            newBarcode = b;
            tab = 'new';
          }}
        />
      {:else if tab === 'quick'}
        <form class="stack" onsubmit={quickAdd} data-testid="quick-add-form">
          <p class="small muted">For meals you can’t break down. Only this diary entry is saved; nothing is added to the library.</p>
          <label class="field"><span>Description</span><input class="input" bind:value={qName} placeholder="e.g. Dinner at Mom's" data-testid="quick-name" /></label>
          <label class="field qt"
            ><span>Time</span><input class="input" type="time" bind:value={time} oninput={() => (timeTouched = true)} /></label
          >
          <div class="grid2">
            <label class="field"><span>Calories (kcal)</span><input class="input num" inputmode="decimal" bind:value={qCal} data-testid="quick-calories" /></label>
            <label class="field"><span>Protein (g)</span><input class="input num" inputmode="decimal" bind:value={qP} data-testid="quick-protein" /></label>
            <label class="field"><span>Carbs (g)</span><input class="input num" inputmode="decimal" bind:value={qC} /></label>
            <label class="field"><span>Fat (g)</span><input class="input num" inputmode="decimal" bind:value={qF} /></label>
          </div>
          <button class="btn primary" disabled={busy || (!qCal.trim() && !qP.trim())} data-testid="quick-add">Add</button>
        </form>
      {:else if tab === 'new'}
        <FoodEditor initialName={query.trim()} initialBarcode={newBarcode} onsaved={(f) => pick(f)} oncancel={() => (tab = 'library')} />
      {/if}
    </div>
  {/if}

  {#snippet footer()}
    <span class="small muted foot">{added.length ? `${added.length} added` : ''}</span>
    <button class="btn" type="button" onclick={() => (app.add.open = false)} data-testid="add-done">Done</button>
  {/snippet}
</Dialog>

<style>
  .grow {
    flex: 1;
    min-width: 0;
  }
  .added {
    margin-bottom: 10px;
  }
  .foot {
    margin-right: auto;
    align-self: center;
  }
  .mtn {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 0.8fr) minmax(0, 1.4fr);
    gap: 10px;
  }
  @media (max-width: 520px) {
    .mtn {
      grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    }
    .mtn .note {
      grid-column: 1 / -1;
    }
  }
  .qt {
    max-width: 180px;
  }
</style>
