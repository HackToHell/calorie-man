<script lang="ts">
  import Avatar from '../components/Avatar.svelte';
  import Dialog from '../components/Dialog.svelte';
  import PageHeader from '../components/PageHeader.svelte';
  import { api } from '../lib/api';
  import { KG_PER_LB, fmtDay, isValidDate, kcal, num, optNum, todayStr, txt } from '../lib/format';
  import { app, type Theme } from '../lib/state.svelte';
  import type { Energy, Goal, Settings } from '../lib/types';

  const user = $derived(app.user!);
  const palette = ['#7a4a8c', '#c0663a', '#2f8a7e', '#5466b0', '#a8507a', '#7d7330', '#3d7ea6', '#9a5b3a'];

  // ---- profile
  let name = $state('');
  let color = $state('#2f855a');
  let sex = $state('');
  let birth = $state('');
  let heightCm = $state('');
  let heightFt = $state('');
  let heightIn = $state('');
  let activity = $state('');
  let unit = $state<'kg' | 'lb'>('kg');
  let tz = $state('');
  let goalWeight = $state('');
  let savingProfile = $state(false);

  function loadProfile() {
    const u = user;
    name = u.name;
    color = u.color;
    sex = u.sex ?? '';
    birth = u.birth_date ?? '';
    unit = u.weight_unit;
    heightCm = u.height_cm != null ? String(Math.round(u.height_cm * 10) / 10) : '';
    if (u.height_cm != null) {
      const inches = u.height_cm / 2.54;
      heightFt = String(Math.floor(inches / 12));
      heightIn = String(Math.round(inches % 12));
    } else heightFt = heightIn = '';
    activity = u.activity_level != null ? String(u.activity_level) : '';
    tz = u.timezone ?? '';
    goalWeight = u.goal_weight_kg != null ? String(Math.round((unit === 'lb' ? u.goal_weight_kg / KG_PER_LB : u.goal_weight_kg) * 10) / 10) : '';
  }
  $effect(() => {
    void user.id;
    loadProfile();
  });

  const zones = (() => {
    try {
      return (Intl as any).supportedValuesOf('timeZone') as string[];
    } catch {
      return [] as string[];
    }
  })();
  const browserTz = Intl.DateTimeFormat().resolvedOptions().timeZone;

  async function saveProfile(e: SubmitEvent) {
    e.preventDefault();
    let h: number | null = null;
    if (unit === 'lb') {
      if (txt(heightFt) || txt(heightIn)) h = ((optNum(heightFt) ?? 0) * 12 + (optNum(heightIn) ?? 0)) * 2.54;
    } else h = optNum(heightCm);
    const gwv = optNum(goalWeight);
    const gw = gwv != null ? gwv * (unit === 'lb' ? KG_PER_LB : 1) : null;
    savingProfile = true;
    try {
      await api.updateUser(user.id, {
        name: name.trim(),
        color,
        sex: (sex || null) as any,
        birth_date: birth || null,
        height_cm: h != null ? Math.round(h * 10) / 10 : null,
        activity_level: optNum(activity),
        weight_unit: unit,
        timezone: tz || null,
        goal_weight_kg: gw != null ? Math.round(gw * 100) / 100 : null,
      });
      await app.reloadUsers();
      app.toast('Profile saved');
      app.touch();
    } catch (err) {
      app.error(err);
    } finally {
      savingProfile = false;
    }
  }

  // ---- people
  let delOpen = $state(false);
  let delConfirm = $state('');
  let newPerson = $state('');
  async function addPerson(e: SubmitEvent) {
    e.preventDefault();
    try {
      const u = await api.createUser({ name: newPerson.trim() });
      newPerson = '';
      await app.reloadUsers();
      app.toast(`Added ${u.name}`);
    } catch (err) {
      app.error(err);
    }
  }
  async function deletePerson() {
    try {
      const gone = user.name;
      await api.deleteUser(user.id);
      delOpen = false;
      delConfirm = '';
      await app.reloadUsers();
      app.toast(`Deleted ${gone} and their diary`);
    } catch (err) {
      app.error(err);
    }
  }

  // ---- settings
  let settings = $state<Settings | null>(null);
  let usdaKey = $state('');
  $effect(() => {
    api
      .settings()
      .then((s) => (settings = s))
      .catch(() => {});
  });
  async function saveKey(clear = false) {
    try {
      settings = await api.saveSettings({ usda_api_key: clear ? null : usdaKey.trim() || null });
      usdaKey = '';
      app.toast(clear ? 'USDA key removed' : 'USDA key saved');
    } catch (err) {
      app.error(err);
    }
  }

  const origin = location.origin;
  const mcpUrl = $derived(`${origin}/mcp?user=${encodeURIComponent(user.name)}`);
  function copy(text: string) {
    navigator.clipboard?.writeText(text).then(
      () => app.toast('Copied'),
      () => app.toast('Copy failed; select the text instead', 'warn'),
    );
  }
</script>

<PageHeader title="Profile" eyebrow={user.name} />

<nav class="more card" aria-label="More pages">
  <a href="#/log" class="btn">Log food</a>
  <a href="#/foods" class="btn">Food bank</a>
  <a href="#/weight" class="btn">Weight</a>
  <a href="#/goals" class="btn">Goals</a>
</nav>

<div class="cols">
  <form class="card stack" onsubmit={saveProfile} data-testid="profile-form">
    <div class="row"><Avatar name={name || user.name} {color} size={34} /><h2>Profile</h2></div>
    <label class="field"><span>Name</span><input class="input" bind:value={name} maxlength="40" required data-testid="profile-name" /></label>
    <div class="row wrap swatches" role="radiogroup" aria-label="Color">
      {#each palette as c}
        <button type="button" class="sw" class:on={color === c} style:background={c} aria-label="Color {c}" role="radio" aria-checked={color === c} onclick={() => (color = c)}></button>
      {/each}
    </div>
    <div class="grid2">
      <label class="field"
        ><span>Sex (for BMR)</span>
        <select class="input" bind:value={sex}>
          <option value="">–</option><option value="male">Male</option><option value="female">Female</option>
        </select>
      </label>
      <label class="field"><span>Birth date</span><input class="input" type="date" bind:value={birth} max={todayStr()} /></label>
      <label class="field"
        ><span>Units</span>
        <select class="input" bind:value={unit} data-testid="profile-unit">
          <option value="kg">kg, cm</option><option value="lb">lb, ft/in</option>
        </select>
      </label>
      {#if unit === 'lb'}
        <div class="field">
          <span class="small muted"><b>Height</b></span>
          <div class="row">
            <input class="input num" type="number" min="0" bind:value={heightFt} aria-label="Feet" placeholder="ft" />
            <input class="input num" type="number" min="0" bind:value={heightIn} aria-label="Inches" placeholder="in" />
          </div>
        </div>
      {:else}
        <label class="field"><span>Height (cm)</span><input class="input num" type="number" min="0" step="0.1" bind:value={heightCm} /></label>
      {/if}
      <label class="field"
        ><span>Activity</span>
        <select class="input" bind:value={activity}>
          <option value="">–</option>
          {#each app.catalog?.activity_levels ?? [] as [v, l]}<option value={String(v)}>{l}</option>{/each}
        </select>
      </label>
      <label class="field"><span>Goal weight ({unit})</span><input class="input num" type="number" step="0.1" min="0" bind:value={goalWeight} /></label>
    </div>
    <label class="field"
      ><span>Time zone (for “today” in AI tools)</span>
      <select class="input" bind:value={tz}>
        <option value="">Server default</option>
        {#if browserTz && !zones.includes(browserTz)}<option value={browserTz}>{browserTz}</option>{/if}
        {#each zones as z}<option value={z}>{z}</option>{/each}
      </select>
    </label>
    <button class="btn primary" disabled={savingProfile} data-testid="save-profile">Save profile</button>
  </form>

  <div class="side">
  <div class="card stack" data-testid="theme-card">
    <h2>Appearance</h2>
    <div class="tabs" role="radiogroup" aria-label="Theme">
      {#each [['system', 'System'], ['light', 'Light'], ['dark', 'Dark']] as [k, l]}
        <button class="tab" class:active={app.theme === k} role="radio" aria-checked={app.theme === k} onclick={() => app.setTheme(k as Theme)}>{l}</button>
      {/each}
    </div>
    <p class="tiny muted">Saved on this device. “System” follows the phone or computer's light/dark setting.</p>
  </div>

  <div class="card stack">
    <h2>People</h2>
    <ul class="list">
      {#each app.users as u (u.id)}
        <li class="row person">
          <Avatar name={u.name} color={u.color} size={26} />
          <span class="grow row">{u.name}{#if u.id === user.id}<span class="chip accent">you</span>{/if}</span>
          {#if u.id !== user.id}<button class="btn small" onclick={() => app.selectUser(u.id)}>Switch</button>{/if}
        </li>
      {/each}
    </ul>
    <form class="row" onsubmit={addPerson}>
      <input class="input" bind:value={newPerson} placeholder="Name" maxlength="40" aria-label="New person's name" />
      <button class="btn" disabled={!txt(newPerson)}>Add person</button>
    </form>
    <button class="btn danger small" style="align-self: flex-start" onclick={() => (delOpen = true)}>Delete {user.name}…</button>
  </div>
  </div>
</div>

<div class="cols">
  <div class="card stack">
    <h2>Online food lookup</h2>
    <p class="small muted">
      USDA FoodData Central works without setup using the shared <code>DEMO_KEY</code> (about 30 searches an hour). A free personal key raises that to 1,000
      an hour: <a href="https://fdc.nal.usda.gov/api-key-signup" target="_blank" rel="noreferrer">get one here</a>. Open Food Facts needs no key.
    </p>
    {#if settings}
      <div class="small">
        Current key: <b>{settings.usda_key_source === 'settings' ? `saved (${settings.usda_api_key_hint})` : settings.usda_key_source === 'environment' ? 'from server environment' : 'DEMO_KEY'}</b>
        {#if settings.offline}<span class="chip warn">offline mode</span>{/if}
      </div>
    {/if}
    <div class="row">
      <input class="input" type="password" bind:value={usdaKey} placeholder="Paste USDA API key" autocomplete="off" aria-label="USDA API key" />
      <button class="btn" disabled={!usdaKey.trim()} onclick={() => saveKey()}>Save</button>
      {#if settings?.usda_api_key_set}<button class="btn ghost" onclick={() => saveKey(true)}>Remove</button>{/if}
    </div>
  </div>

<div class="card stack">
    <h2>Data</h2>
    <div class="row wrap">
      <a class="btn" href="/api/users/{user.id}/export.csv" download>Export {user.name}'s diary (CSV)</a>
      <a class="btn" href="/api/export.json" download>Export everything (JSON)</a>
      <a class="btn" href="/api/backup" download>Download database backup</a>
    </div>
    <p class="tiny muted">The backup is a complete SQLite copy; replace the server’s database file with it to restore.</p>
  </div>
</div>

<div class="card stack" data-testid="mcp-card">
  <h2>AI assistants (MCP)</h2>
  <p class="small muted">
    Connect an MCP client so an AI can look up products, create foods and recipes, and log meals for you. The URL below defaults tools to
    <b>{user.name}</b>; leave off <code>?user=</code> and say whose diary to use instead.
  </p>
  <div class="row">
    <input class="input mono" readonly value={mcpUrl} aria-label="MCP URL" data-testid="mcp-url" />
    <button class="btn" onclick={() => copy(mcpUrl)}>Copy</button>
  </div>
  <details>
    <summary class="small">Client setup examples</summary>
    <p class="small"><b>Claude Code</b></p>
    <pre>claude mcp add --transport http calorie-man "{mcpUrl}"</pre>
    <p class="small"><b>Claude Desktop</b> (stdio bridge; run on a machine with the calorie-man binary)</p>
    <pre>{`{
  "mcpServers": {
    "calorie-man": {
      "command": "/path/to/calorie-man",
      "args": ["mcp-stdio", "--url", "${mcpUrl}"]
    }
  }
}`}</pre>
    <p class="small"><b>Any Streamable HTTP client</b> (Cursor, VS Code, LM Studio, …)</p>
    <pre>{`{ "url": "${mcpUrl}" }`}</pre>
  </details>
</div>

<Dialog bind:open={delOpen} title="Delete {user.name}?" testid="delete-user-dialog">
  <div class="stack">
    <p>This permanently deletes {user.name}'s diary, goals, weights and exercise. Shared foods and recipes stay.</p>
    <label class="field"><span>Type <b>{user.name}</b> to confirm</span><input class="input" bind:value={delConfirm} /></label>
  </div>
  {#snippet footer()}
    <button class="btn" onclick={() => (delOpen = false)}>Cancel</button>
    <button class="btn danger solid" disabled={delConfirm.trim().toLowerCase() !== user.name.toLowerCase()} onclick={deletePerson}>Delete</button>
  {/snippet}
</Dialog>

<style>
  .more {
    display: none;
  }
  @media (max-width: 899px) {
    .more {
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: 8px;
      margin-bottom: 12px;
    }
  }
  .cols {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    gap: 14px;
    margin-bottom: 14px;
    align-items: start;
  }
  .cols .card + .card {
    margin-top: 0;
  }
  .side {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  @media (max-width: 860px) {
    .cols {
      grid-template-columns: minmax(0, 1fr);
    }
  }
  .swatches {
    gap: 6px;
  }
  .sw {
    width: 28px;
    height: 28px;
    clip-path: var(--octagon);
    border: 0;
    cursor: pointer;
  }
  .sw.on {
    box-shadow: inset 0 0 0 3px var(--surface), inset 0 0 0 5px var(--ink);
  }
  .person {
    padding: 8px 0;
  }
  .grow {
    flex: 1;
  }
  .mono {
    font-family: var(--mono);
    font-size: 0.85rem;
  }
</style>
