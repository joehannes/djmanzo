<!--
  The box office at the harbour: four editions, three ways to pay for them,
  a price that follows where you are (once you can show it), promo codes,
  vouchers — and, on the board by the window, how the prices were set.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { EDITIONS, PLANS, money, type Edition, type Plan } from "../../shared/catalog";
  import { BAND_NAME, band as bandOf, currencyFor, regional } from "../../shared/regions";
  import { catalog } from "../lib/catalog.svelte";
  import { session } from "../lib/session.svelte";
  import { profile, recommend } from "../lib/profile.svelte";
  import { countries, flag } from "../lib/countries";
  import { post } from "../lib/api";
  import type { PlaceId } from "../lib/places";
  import EditionSleeve from "../shop/EditionSleeve.svelte";
  import SignIn from "../shop/SignIn.svelte";
  import Checkout from "../shop/Checkout.svelte";
  import PriceBoard from "../shop/PriceBoard.svelte";

  let { go }: { go: (id: PlaceId) => void; linkCode?: string | null } = $props();

  const rec = $derived(recommend(profile.p));
  let edition = $state<Edition>(recommend(profile.p).edition);
  let plan = $state<Plan>("yearly");
  let country = $state("US");
  let promo = $state("");
  let quote = $state<any>(null);
  let quoteError = $state<string | null>(null);
  let done = $state<any>(null);
  let voucher = $state("");
  let voucherMsg = $state<string | null>(null);
  const list = countries();

  onMount(async () => {
    await catalog.load();
    if (catalog.data?.visitor.country) country = catalog.data.visitor.country;
  });

  const currency = $derived(currencyFor(country));
  const b = $derived(bandOf(country));
  const PER = { monthly: "a month", yearly: "a year", perpetual: "once, for version 1" };

  // Ask the Worker for the real quote whenever the choice changes; the page's own arithmetic is only a preview.
  let timer: ReturnType<typeof setTimeout>;
  $effect(() => {
    const body = { edition, plan, country, promo: promo.trim() || null };
    clearTimeout(timer);
    timer = setTimeout(async () => {
      try {
        quote = await post("/api/quote", body);
        quoteError = null;
      } catch (e) {
        quoteError = (e as Error).message;
      }
    }, 250);
  });

  async function redeem() {
    voucherMsg = null;
    try {
      done = await post("/api/redeem", { code: voucher });
    } catch (e) {
      voucherMsg = (e as Error).message;
    }
  }

  function expires(ms: number | null): string {
    return ms === null ? "never — version 1 is yours" : new Date(ms).toLocaleDateString(undefined, { day: "numeric", month: "long", year: "numeric" });
  }
</script>

{#if done}
  <div class="done">
    <span class="big" aria-hidden="true">🎉</span>
    <h2>It's yours.</h2>
    <p>
      <b>djmanzo {catalog.data?.editions[done.licence.edition as Edition]?.name ?? done.licence.edition}</b>, {done.licence.plan === "perpetual" ? "bought once" : done.licence.plan}.
      Valid until {expires(done.licence.expires)}.{#if done.licence.demo}<span class="demo-badge">demo licence</span>{/if}
    </p>
    <h3>Get it into djmanzo</h3>
    <ol class="steps">
      <li>Download djmanzo for your computer: <a href={catalog.data?.downloads} target="_blank" rel="noopener">the latest release</a>.</li>
      <li>In djmanzo, open <b>Settings → Account</b> and press <b>Sign in</b>. It shows a short code and opens this site.</li>
      <li>Sign in here with the same account, check the code, and press <b>Link</b>. djmanzo picks up the licence by itself and keeps working offline.</li>
    </ol>
    <div class="row">
      <button class="btn primary" onclick={() => go("pase")}>🪪 See it in your pass</button>
      <button class="btn" onclick={() => (done = null)}>Back to the box office</button>
    </div>
  </div>
{:else}
  <h2 class="step">Pick your edition</h2>
  <div class="editions">
    {#if catalog.data}
      {#each EDITIONS as e (e)}
        <EditionSleeve
          info={catalog.data.editions[e]}
          features={catalog.data.features}
          price={money(regional(e, plan, b), currency)}
          per={PER[plan]}
          seats={catalog.data.seats[e]}
          chosen={edition === e}
          pick={profile.p.asked.length > 0 && rec.edition === e}
          onchoose={(x) => (edition = x)}
        />
      {/each}
    {:else if catalog.error}
      <p class="notice">{catalog.error}</p>
    {:else}
      <p class="muted">Opening the box office…</p>
    {/if}
  </div>
  {#if profile.p.asked.length}
    <p class="small muted">The MC's pick: {rec.why} <button class="link" onclick={() => go("gigs")}>Compare by kind of night</button>.</p>
  {/if}

  <h2 class="step">How you'd like to pay for it</h2>
  <div class="plans" role="radiogroup" aria-label="Plan">
    {#each PLANS as p (p)}
      <button class="plan" role="radio" aria-checked={plan === p} onclick={() => (plan = p)}>
        <b>{catalog.data?.plans[p].name ?? p}</b>
        <span>{money(regional(edition, p, b), currency)} {PER[p]}</span>
        <small>{catalog.data?.plans[p].about}</small>
      </button>
    {/each}
  </div>

  <h2 class="step">Where you are</h2>
  <div class="where">
    <label class="field">
      <span>Country</span>
      <select bind:value={country}>
        {#each list as c (c.code)}<option value={c.code}>{flag(c.code)} {c.name}</option>{/each}
      </select>
    </label>
    <p class="small">
      {#if b === 1}
        Full price here. {country === catalog.data?.visitor.country ? "(We guessed from your connection.)" : ""}
      {:else}
        <b>{BAND_NAME[b]}</b> in {flag(country)} — prices follow what things cost where you live. To get it, your <b>card must be issued in</b> and your
        <b>billing address must be in</b> a country priced the same or lower. Otherwise the checkout tells you the price you can show, and asks before charging it.
      {/if}
    </p>
  </div>

  <h2 class="step">A promo code?</h2>
  <div class="promo">
    <input type="text" bind:value={promo} placeholder="e.g. MANZO-ABCD-EFGH" aria-label="Promo code" autocomplete="off" spellcheck="false" />
    {#if quote?.promo}
      <span class={quote.promo.ok ? "good" : "bad"} role="status">{quote.promo.ok ? "✓ " : ""}{quote.promo.reason}</span>
    {/if}
  </div>

  <div class="ticket" aria-live="polite">
    {#if quote}
      <div class="lines">
        <span>djmanzo {catalog.data?.editions[edition].name}, {catalog.data?.plans[plan].name.toLowerCase()}</span>
        {#if quote.regional !== quote.base}
          <span class="r"><s>{money(quote.base, quote.currency)}</s> {money(quote.regional, quote.currency)} <small>{BAND_NAME[quote.band as 1]}</small></span>
        {:else}
          <span class="r">{money(quote.regional, quote.currency)}</span>
        {/if}
        {#if quote.discount}
          <span>Promo {quote.promo.code}</span><span class="r">− {money(quote.discount, quote.currency)}</span>
        {/if}
        <span class="total">Total</span><span class="r total">{money(quote.total, quote.currency)}</span>
      </div>
      {#if quote.renews && quote.discount}<p class="small muted">The promo applies to the first payment; renewals are {money(quote.regional, quote.currency)}.</p>{/if}
    {:else if quoteError}
      <p class="notice">{quoteError}</p>
    {:else}
      <p class="muted">Working out the price…</p>
    {/if}
  </div>

  <h2 class="step">Pay</h2>
  {#if !session.user}
    <SignIn />
  {:else}
    <p class="small">Signed in as <b>{session.user.name}</b>{session.user.email ? ` (${session.user.email})` : ""}. The licence goes to this account.</p>
    {#if quote}
      <Checkout {edition} {plan} {country} {promo} total={quote.total} display={money(quote.total, quote.currency)} ondone={(r) => (done = r)} />
    {/if}
  {/if}

  <details class="voucher">
    <summary>Got a voucher instead?</summary>
    {#if session.user}
      <form onsubmit={(e) => { e.preventDefault(); void redeem(); }}>
        <input type="text" bind:value={voucher} placeholder="GIFT-XXXX-XXXX" aria-label="Voucher code" />
        <button class="btn">Redeem</button>
      </form>
      {#if voucherMsg}<p class="notice">{voucherMsg}</p>{/if}
    {:else}
      <p class="small">Sign in above first, so the licence has somewhere to go.</p>
    {/if}
  </details>

  <PriceBoard />
{/if}

<style>
  .step {
    margin-top: 22px;
    font-size: 1.3rem;
  }
  .editions {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
    gap: 14px;
  }
  .plans {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 10px;
  }
  .plan {
    display: flex;
    flex-direction: column;
    gap: 2px;
    text-align: left;
    padding: 12px;
    border-radius: 12px;
    border: 2px solid var(--line);
    background: var(--surface-2);
    cursor: pointer;
  }
  .plan[aria-checked="true"] {
    border-color: var(--primary);
    box-shadow: inset 0 0 0 2px var(--primary);
  }
  .plan small {
    color: var(--ink-soft);
  }
  .where {
    display: grid;
    grid-template-columns: minmax(200px, 280px) 1fr;
    gap: 14px;
    align-items: start;
  }
  .promo {
    display: flex;
    gap: 10px;
    align-items: center;
    flex-wrap: wrap;
  }
  .promo input {
    max-width: 280px;
    text-transform: uppercase;
  }
  .good {
    color: #3ddc84;
    font-weight: 700;
  }
  .bad {
    color: var(--secondary);
    font-weight: 700;
  }
  .ticket {
    margin-top: 18px;
    padding: 16px 18px;
    border-radius: 6px;
    background: #fbf3dc;
    color: #1d1a14;
    border-left: 8px dotted #d9c79d;
    max-width: 520px;
    --ink-soft: #6b5a44;
  }
  .lines {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 6px 14px;
  }
  .r {
    text-align: right;
  }
  .total {
    font-weight: 800;
    font-size: 1.25rem;
    border-top: 2px dashed #c9b88a;
    padding-top: 6px;
  }
  small {
    font-size: 12px;
  }
  .voucher {
    margin: 18px 0;
  }
  .voucher summary {
    cursor: pointer;
    font-weight: 700;
  }
  .voucher form {
    display: flex;
    gap: 8px;
    margin-top: 8px;
    max-width: 420px;
  }
  .link {
    background: none;
    border: 0;
    color: var(--primary);
    text-decoration: underline;
    cursor: pointer;
    padding: 0;
    font: inherit;
  }
  .done {
    text-align: left;
  }
  .big {
    font-size: 52px;
  }
  .steps li {
    margin-bottom: 8px;
  }
  .row {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  .demo-badge {
    margin-left: 8px;
    padding: 2px 8px;
    border-radius: 999px;
    background: var(--secondary);
    color: var(--on-secondary);
    font-size: 12px;
    font-weight: 800;
  }
  @media (max-width: 640px) {
    .plans,
    .where {
      grid-template-columns: 1fr;
    }
  }
</style>
