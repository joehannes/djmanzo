<!--
  The board by the box office window: what comparable software costs, where
  each figure comes from, how each edition's price was reached from it
  (docs/EDITIONS.md), and the rules for regions and students — so anyone can
  check the arithmetic.
-->
<script lang="ts">
  import { NAMED, PLANS, money, type Edition } from "../../shared/catalog";
  import { BAND_NAME, regional } from "../../shared/regions";
  import { catalog } from "../lib/catalog.svelte";

  const PLAN_SHORT = { monthly: "/mo", yearly: "/yr", perpetual: "once" };
  const usd = (n: number | undefined | null) => (n == null ? "—" : `$${n % 1 ? n.toFixed(2) : n}`);
  const EXAMPLES = ["United States, Austria, Japan", "Dominican Republic, Mexico, Brazil, Jamaica", "India, Nigeria, Haiti, the Philippines", "Ethiopia, Uganda, Madagascar"];
</script>

{#if catalog.data}
  {@const c = catalog.data}
  <section class="board">
    <h2>How these prices were set</h2>
    <p>
      The rule: <b>half of what the professional competitors charge on average</b>, and every edition cheaper than the same modules bought one by one. These are
      the list prices vendors published in October 2026; they change them, so follow the links.
    </p>
    <table>
      <thead><tr><th>Product</th><th>Once</th><th>Monthly</th><th>Yearly</th></tr></thead>
      <tbody>
        {#each c.market as r (r.product)}
          <tr>
            <td><a href={r.source} target="_blank" rel="noopener">{r.product}</a>{#if r.note}<br /><span class="small muted">{r.note}</span>{/if}</td>
            <td>{usd(r.perpetual)}</td>
            <td>{usd(r.monthly)}</td>
            <td>{usd(r.yearly)}</td>
          </tr>
        {/each}
      </tbody>
    </table>

    <h3>djmanzo, edition by edition</h3>
    <table>
      <thead><tr><th>Edition</th>{#each PLANS as p (p)}<th>{c.plans[p].name}</th>{/each}<th>How it was reached</th></tr></thead>
      <tbody>
        {#each NAMED as e (e)}
          <tr class="ours">
            <td><b>{c.editions[e].emoji} {c.editions[e].name}</b></td>
            {#each PLANS as p (p)}<td>{money(c.prices[e][p], "USD")} <span class="small">{PLAN_SHORT[p]}</span></td>{/each}
            <td class="small">{c.reasons[e]}</td>
          </tr>
        {/each}
        <tr>
          <td><b>{c.editions.custom.emoji} Custom</b></td>
          <td colspan="3">{money(c.foundation, "USD")} + modules</td>
          <td class="small">{c.reasons.custom}</td>
        </tr>
      </tbody>
    </table>
    <p class="small">
      <b>Monthly and yearly</b> are ours to propose, not yet the owner's word: Pro DJ's month is half the professional subscriptions' average (Serato DJ Pro,
      VirtualDJ Pro, rekordbox Creative: {money(Math.round(c.proMonth * 2), "USD")} → {money(Math.round(c.proMonth), "USD")}), every other edition's month in the same
      proportion to its one-time price, and a year is ten months. <b>Upgrading</b>, bought once, costs the difference between the two editions.
    </p>

    <h3>Where you live, and students</h3>
    <p class="small">
      By the World Bank's income groups. The price applies when your card and your billing address both show it. Students and teachers pay half, with a school
      e-mail address; the two discounts never stack — the larger one applies.
    </p>
    <ul class="bands">
      {#each [1, 2, 3, 4] as b (b)}
        <li><b>{BAND_NAME[b as 1]}</b> — e.g. {EXAMPLES[b - 1]}; Pro DJ bought once {money(regional("pro" as Edition, "perpetual", b as 1), "USD")}</li>
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
    overflow-x: auto;
  }
  table {
    margin: 8px 0;
    font-size: 14px;
  }
  .ours td {
    background: color-mix(in srgb, var(--primary) 12%, transparent);
  }
  .bands {
    padding-left: 18px;
    font-size: 14px;
  }
</style>
