<script lang="ts">
  import { num } from '../lib/format';
  import { app } from '../lib/state.svelte';
  import type { Nutrients, Target } from '../lib/types';
  import Progress from './Progress.svelte';

  let {
    totals,
    targets,
    coverage = {},
    groups = ['macros', 'fats', 'minerals', 'vitamins', 'other'],
  }: { totals: Nutrients; targets: Target[]; coverage?: Record<string, number>; groups?: string[] } = $props();

  const labels: Record<string, string> = { macros: 'Macros', fats: 'Fats', minerals: 'Minerals', vitamins: 'Vitamins', other: 'Other' };
  const byKey = $derived(new Map(targets.map((t) => [t.key, t])));
  const color = (pct: number, limit: boolean) =>
    limit ? (pct > 100 ? 'var(--over)' : 'var(--sea)') : pct >= 100 ? 'var(--sea)' : pct >= 50 ? 'var(--butter)' : 'var(--muted)';
</script>

<div class="nt" data-testid="nutrient-table">
  {#each groups as g}
    {@const defs = (app.catalog?.nutrients ?? []).filter((n) => n.group === g)}
    <h4>{labels[g]}</h4>
    <table class="table">
      <tbody>
        {#each defs as d (d.key)}
          {@const v = totals[d.key]}
          {@const t = byKey.get(d.key)}
          {@const pct = t && v != null ? (v / t.target) * 100 : null}
          {@const cov = coverage[d.key]}
          <tr data-key={d.key}>
            <td class="name">
              {d.label}
              {#if cov != null && cov < 1 && v != null}<span class="chip warn" title="Some entries have no data for this nutrient, so the total is a minimum"
                  >{Math.round(cov * 100)}% data</span
                >{/if}
            </td>
            <td class="r num amount">{v != null ? num(v) : '–'} <span class="faint">{d.unit}</span></td>
            <td class="bar">
              {#if t && pct != null}
                <Progress value={v ?? 0} max={t.target} limit={t.limit} color={color(pct, t.limit)} height={6} label={d.label} />
              {/if}
            </td>
            <td class="r num pct">
              {#if t && pct != null}{Math.round(pct)}%{:else if t}<span class="faint">0%</span>{/if}
              {#if t}<div class="tiny faint">{t.limit ? '≤ ' : ''}{num(t.target)}{d.unit}</div>{/if}
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/each}
  <p class="tiny muted">Targets are your goals, or FDA daily values (2,000 kcal reference) when no goal is set. “≤” marks limits.</p>
</div>

<style>
  h4 {
    margin: 14px 0 4px;
    font-size: 0.8rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--muted);
  }
  h4:first-child {
    margin-top: 0;
  }
  .name {
    width: 40%;
  }
  .amount {
    width: 22%;
    white-space: nowrap;
  }
  .bar {
    width: 22%;
  }
  .pct {
    width: 16%;
    white-space: nowrap;
  }
  .chip {
    margin-left: 4px;
  }
</style>
