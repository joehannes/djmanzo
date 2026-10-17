<!--
  The board by the box office window: what comparable software costs, where
  each figure comes from, and djmanzo's price beside it — half the typical,
  computed rather than chosen, so anyone can check it.
-->
<script lang="ts">
  import { EDITIONS, PLANS, money, type Edition } from "../../shared/catalog";
  import { BAND_NAME, regional } from "../../shared/regions";
  import { catalog } from "../lib/catalog.svelte";

  const PLAN_SHORT = { monthly: "/mo", yearly: "/yr", perpetual: "once" };
  const usd = (n: number | undefined | null) => (n == null ? "—" : `$${n % 1 ? n.toFixed(2) : n}`);
</script>

{#if catalog.data}
  {@const c = catalog.data}
  <section class="board">
    <h2>How these prices were set</h2>
    <p>
      We looked up what comparable DJ and karaoke software costs, took the typical (mean) price for each kind of edition and plan, and halved it. A yearly plan is
      capped at about ten months, so it is always worth having. Prices were checked in October 2026; vendors change them, so follow the links.
    </p>
    {#each EDITIONS as e (e)}
      {@const refs = c.market[e as Edition]}
      <details open={e === "pro"}>
        <summary><b>{c.editions[e].emoji} {c.editions[e].name}</b> — compared with {refs.map((r) => r.product).join(", ")}</summary>
        <table>
          <thead><tr><th>Product</th><th>Monthly</th><th>Yearly</th><th>Once</th></tr></thead>
          <tbody>
            {#each refs as r (r.product)}
              <tr>
                <td><a href={r.source} target="_blank" rel="noopener">{r.product}</a>{#if r.note}<br /><span class="small muted">{r.note}</span>{/if}</td>
                <td>{usd(r.monthly)}</td>
                <td>{usd(r.yearly)}</td>
                <td>{usd(r.perpetual)}</td>
              </tr>
            {/each}
            <tr class="typ">
              <td>Typical</td>
              {#each PLANS as p (p)}<td>{usd(c.typical[e][p] ? Math.round(c.typical[e][p]! * 100) / 100 : null)}</td>{/each}
            </tr>
            <tr class="ours">
              <td>djmanzo {c.editions[e].name}</td>
              {#each PLANS as p (p)}<td><b>{money(c.base[e][p], "USD")}</b> <span class="small">{PLAN_SHORT[p]}</span></td>{/each}
            </tr>
          </tbody>
        </table>
        {#if e === "karaoke"}
          <p class="small muted">KaraFun's price includes a song catalogue and a commercial licence; djmanzo comes with no songs, so it is compared with software-only karaoke and KaraFun is shown for honesty, not used.</p>
        {/if}
      </details>
    {/each}
    <h3>By region</h3>
    <p class="small">Four bands by what things cost where you live. The band applies when your card and your billing address both show it.</p>
    <ul class="bands">
      {#each [1, 2, 3, 4] as b (b)}
        <li><b>{BAND_NAME[b as 1]}</b> — e.g. {["United States, Germany, Japan", "Spain, Poland, Chile", "Dominican Republic, Mexico, Brazil, Jamaica", "India, Nigeria, Haiti, Philippines"][b - 1]}; Pro yearly {money(regional("pro", "yearly", b as 1), "USD")}</li>
      {/each}
    </ul>
  </section>
{/if}

<style>
  .board {
    margin-top: 28px;
    padding: 18px;
    border-radius: 14px;
    background: var(--surface-2);
    border: 2px dashed var(--line);
  }
  details {
    margin: 8px 0;
  }
  summary {
    cursor: pointer;
  }
  table {
    margin: 8px 0;
    font-size: 14px;
  }
  .typ td {
    color: var(--ink-soft);
    font-style: italic;
  }
  .ours td {
    background: color-mix(in srgb, var(--primary) 18%, transparent);
  }
  .bands {
    padding-left: 18px;
    font-size: 14px;
  }
</style>
