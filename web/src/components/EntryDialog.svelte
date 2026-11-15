<script lang="ts">
  import { api } from '../lib/api';
  import { MEAL_LABELS, isValidDate, kcal, num } from '../lib/format';
  import { legacyOption, pickOption, portionBody, unitOptions, type UnitOption } from '../lib/portion';
  import { app } from '../lib/state.svelte';
  import type { Entry, Food } from '../lib/types';
  import Dialog from './Dialog.svelte';
  import PortionPicker from './PortionPicker.svelte';

  let { entry = $bindable(null), onchanged }: { entry?: Entry | null; onchanged: () => void } = $props();

  let open = $state(false);
  let food = $state<Food | null>(null);
  let legacy = $state<UnitOption[]>([]);
  let loadingFood = $state(false);
  let quantity = $state<number | string>(1);
  let optValue = $state('');
  let meal = $state('breakfast');
  let date = $state('');
  let note = $state('');
  let time = $state('');
  let name = $state('');
  let qn = $state<Record<string, string>>({});
  let busy = $state(false);

  $effect(() => {
    const e = entry;
    if (!e) {
      open = false;
      return;
    }
    open = true;
    meal = e.meal;
    date = e.date;
    note = e.note ?? '';
    time = e.time ?? '';
    name = e.name;
    quantity = e.quantity;
    food = null;
    qn = Object.fromEntries(
      ['calories', 'protein', 'carbs', 'fat'].map((k) => [k, e.nutrients[k] != null ? String(Math.round((e.nutrients[k] / e.quantity) * 10) / 10) : '']),
    );
    if (e.food_id != null) {
      loadingFood = true;
      api
        .food(e.food_id)
        .then((f) => {
          const l = legacyOption(f, e);
          legacy = l ? [l] : [];
          food = f;
          optValue = l ? l.value : pickOption(f, unitOptions(f), e.serving_id, e.unit).value;
        })
        .catch((err) => app.error(err))
        .finally(() => (loadingFood = false));
    }
  });

  function close() {
    entry = null;
  }

  async function save(ev: SubmitEvent) {
    ev.preventDefault();
    if (!entry) return;
    const q = Number(quantity);
    if (!Number.isFinite(q) || q <= 0) return app.toast('Quantity must be above 0', 'error');
    if (!isValidDate(date)) return app.toast('Pick a valid date', 'error');
    const body: Record<string, unknown> = { meal, date, note: note.trim() || null, quantity: q, time: time || null };
    if (food && optValue !== 'legacy') {
      const opts = unitOptions(food);
      const o = opts.find((x) => x.value === optValue) ?? opts[0];
      Object.assign(body, portionBody(o, q));
      if (o.servingId == null) body.serving_id = null;
    } else if (entry.food_id == null) {
      body.name = name.trim() || 'Quick add';
      const n: Record<string, number> = {};
      for (const [k, v] of Object.entries(qn)) if (v.trim() !== '') n[k] = Number(v);
      if (!Object.keys(n).length) return app.toast('Enter at least one nutrient', 'error');
      body.nutrients = n;
    }
    busy = true;
    try {
      await api.updateEntry(entry.id, body);
      app.toast('Entry updated');
      onchanged();
      close();
    } catch (e) {
      app.error(e);
    } finally {
      busy = false;
    }
  }

  async function remove() {
    if (!entry) return;
    busy = true;
    try {
      await api.deleteEntry(entry.id);
      app.toast(`Removed ${entry.name}`);
      onchanged();
      close();
    } catch (e) {
      app.error(e);
    } finally {
      busy = false;
    }
  }
</script>

<Dialog bind:open title="Edit entry" onclose={close} testid="entry-dialog">
  {#if entry}
    <form id="entry-form" class="stack" onsubmit={save}>
      <div>
        <b>{entry.name}</b>{#if entry.brand}<span class="muted">{' · '}{entry.brand}</span>{/if}
        <div class="tiny muted">Logged {entry.display_quantity} · {kcal(entry.nutrients.calories)} kcal · {num(entry.nutrients.protein, 1)} g protein</div>
      </div>
      {#if entry.food_id != null}
        {#if food}
          <PortionPicker {food} bind:quantity bind:optValue extra={legacy} />
        {:else if loadingFood}
          <p class="muted small">Loading food…</p>
        {/if}
      {:else}
        <label class="field"><span>Description</span><input class="input" bind:value={name} /></label>
        <div class="grid2">
          <label class="field"><span>Quantity</span><input class="input num" type="number" step="any" min="0" bind:value={quantity} /></label>
          <span></span>
          {#each [['calories', 'Calories (kcal)'], ['protein', 'Protein (g)'], ['carbs', 'Carbs (g)'], ['fat', 'Fat (g)']] as [k, l]}
            <label class="field"><span>{l} per 1</span><input class="input num" inputmode="decimal" bind:value={qn[k]} /></label>
          {/each}
        </div>
      {/if}
      <div class="grid2">
        <label class="field"
          ><span>Meal</span>
          <select class="input" bind:value={meal} data-testid="entry-meal">
            {#each Object.entries(MEAL_LABELS) as [k, l]}<option value={k}>{l}</option>{/each}
          </select>
        </label>
        <label class="field"><span>Date</span><input class="input" type="date" bind:value={date} required /></label>
        <label class="field"><span>Time eaten</span><input class="input" type="time" bind:value={time} data-testid="entry-time-edit" /></label>
      </div>
      <label class="field"><span>Note</span><input class="input" bind:value={note} maxlength="500" /></label>
    </form>
  {/if}
  {#snippet footer()}
    <button class="btn danger" type="button" onclick={remove} disabled={busy} data-testid="delete-entry">Delete</button>
    <span class="spacer"></span>
    <button class="btn" type="button" onclick={close}>Cancel</button>
    <button class="btn primary" type="submit" form="entry-form" disabled={busy || (entry?.food_id != null && !food)} data-testid="save-entry">Save</button>
  {/snippet}
</Dialog>
