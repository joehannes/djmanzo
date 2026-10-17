<!--
  §131: which edition this djmanzo is, in Settings.

  One download for every DJ, unlocked by the licence their sign-in fetches
  (docs/EDITIONS.md). Until the licence service is deployed there is nothing
  to sign in to, so this says so plainly and takes a licence pasted instead —
  which is also how a licence reaches a machine that has never been online.
-->
<script lang="ts">
  import { licenceInstall, licenceSignIn, licenceSignOut, licenceStatus, type LicenceStatus } from "./api";
  import { edition } from "./edition.svelte";

  let status = $state<LicenceStatus | null>(null);
  let pasted = $state("");
  let error = $state<string | null>(null);
  let busy = $state(false);
  /** The edition a new trial is of: Full unless the DJ picks another. */
  let trialOf = $state("full");

  const PROVIDERS = [
    ["google", "Google"],
    ["apple", "Apple"],
    ["github", "GitHub"],
    ["microsoft", "Microsoft"],
    ["discord", "Discord"],
  ];
  const EDITIONS = [
    ["full", "Full"],
    ["pro", "Pro DJ"],
    ["karaoke", "Karaoke"],
    ["apprentice", "Apprentice"],
    ["autopilot", "Auto-Pilot"],
    ["hobbyist", "Hobbyist"],
  ];

  async function signIn(provider: string, trial?: string) {
    busy = true;
    try {
      status = await licenceSignIn(provider, trial);
      edition.status = status;
      error = null;
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  $effect(() => {
    void licenceStatus()
      .then((s) => (status = s))
      .catch((e) => (error = String(e)));
  });

  async function install() {
    busy = true;
    try {
      status = await licenceInstall(pasted.trim());
      // A licence that gives more is worn at once, so the panels follow it.
      edition.status = status;
      pasted = "";
      error = null;
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function signOut() {
    busy = true;
    try {
      status = await licenceSignOut();
      edition.status = status;
      error = null;
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  /** The sentence at the top: where this djmanzo stands. */
  function standing(s: LicenceStatus): string {
    switch (s.state) {
      case "licensed":
        return `${s.product}${s.account ? `, signed in as ${s.account}` : ""}.`;
      case "trial": {
        const days = s.trial_days_left ?? 0;
        return `Trying ${s.product}: ${days} ${days === 1 ? "day" : "days"} left.`;
      }
      case "trial_over":
        return `The trial of ${s.product} has ended. Everything you made is kept.`;
      case "refused":
        return `The licence on this computer is not accepted: ${s.reason}.`;
      default:
        return "Not signed in.";
    }
  }
</script>

<div class="block licence" data-licence={status?.state ?? "loading"}>
  <h3>Licence</h3>
  {#if status}
    <p class="standing">{standing(status)}</p>
    {#if status.development}
      <p class="hint">A development licence: only a development build accepts it.</p>
    {/if}
    {#if status.waiting}
      <p class="hint">A change to your licence takes effect the next time djmanzo starts, so nothing you are using disappears in the middle of a set.</p>
    {/if}
    {#if status.renew_due}
      <p class="hint">Sign in when you are next online, to pick up any upgrade. djmanzo keeps working until then.</p>
    {/if}
  {/if}
  {#if !status || status.state === "signed_out" || status.state === "trial_over" || status.state === "refused"}
    <p class="hint">Sign in with an account you already have; djmanzo keeps no password.</p>
    <div class="row providers">
      {#each PROVIDERS as [id, name] (id)}
        <button disabled={busy} data-sign-in={id} onclick={() => signIn(id)}>{name}</button>
      {/each}
    </div>
    {#if !status || status.state === "signed_out"}
      <div class="row trial">
        <span class="hint">Or try an edition free for 7 days:</span>
        <select bind:value={trialOf} aria-label="Edition to try">
          {#each EDITIONS as [id, name] (id)}
            <option value={id}>{name}</option>
          {/each}
        </select>
        {#each PROVIDERS as [id, name] (id)}
          <button disabled={busy} data-trial-with={id} onclick={() => signIn(id, trialOf)}>with {name}</button>
        {/each}
      </div>
    {/if}
  {/if}
  <p class="hint">A licence you were sent can be pasted here.</p>
  <div class="row">
    <input
      class="paste mono"
      placeholder="djmanzo-licence-1.…"
      aria-label="Paste a licence"
      bind:value={pasted}
    />
    <button disabled={busy || !pasted.trim()} onclick={install}>Use this licence</button>
    {#if status && status.state !== "signed_out"}
      <button disabled={busy} onclick={signOut}>Sign out</button>
    {/if}
  </div>
  {#if error}
    <p class="error">{error}</p>
  {/if}
</div>

<style>
  .standing {
    margin: 0.2rem 0 0.4rem;
  }
  .providers,
  .trial {
    flex-wrap: wrap;
    gap: 0.3rem;
  }
  .paste {
    flex: 1;
    min-width: 12rem;
  }
</style>
