<script lang="ts">
  import { api } from '../lib/api';
  import Logo from '../components/Logo.svelte';
  import { app, navigate } from '../lib/state.svelte';

  let name = $state('');
  let unit = $state<'kg' | 'lb'>('kg');
  let busy = $state(false);
  let err = $state<string | null>(null);

  async function start(e: SubmitEvent) {
    e.preventDefault();
    busy = true;
    err = null;
    try {
      const u = await api.createUser({ name: name.trim(), weight_unit: unit });
      await app.reloadUsers();
      app.selectUser(u.id);
      navigate('goals?setup=1');
    } catch (e) {
      err = (e as Error).message;
    } finally {
      busy = false;
    }
  }
</script>

<div class="wrap">
  <form class="card stack" onsubmit={start} data-testid="onboarding">
    <Logo size={34} />
    <h1 class="welcome">Welcome</h1>
    <p class="muted">
      Track calories, protein and nutrients for your household. The food library and recipes are shared; each person has their own
      diary, goals and weight log. Add more people later from the menu in the top right.
    </p>
    <label class="field"><span>Your name</span><input class="input" bind:value={name} maxlength="40" required autocomplete="given-name" /></label>
    <fieldset class="row">
      <legend class="small muted">Weight unit</legend>
      <label class="check"><input type="radio" bind:group={unit} value="kg" /> kg</label>
      <label class="check"><input type="radio" bind:group={unit} value="lb" /> lb</label>
    </fieldset>
    {#if err}<div class="banner error">{err}</div>{/if}
    <button class="btn primary" disabled={busy || !name.trim()}>Get started</button>
  </form>
</div>

<style>
  .wrap {
    min-height: 100dvh;
    display: grid;
    place-items: center;
    padding: 16px;
  }
  form {
    max-width: 440px;
    width: 100%;
  }
  .welcome {
    text-align: center;
    font-size: 28px;
  }
  fieldset {
    border: 0;
    padding: 0;
    margin: 0;
    gap: 16px;
  }
  legend {
    margin-bottom: 4px;
  }
</style>
