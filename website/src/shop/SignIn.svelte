<!--
  Sign in with an account you already have. The site never sees a password;
  the provider says who you are, and the licence you buy is tied to that
  account — which is how djmanzo finds it later.
-->
<script lang="ts">
  import { session } from "../lib/session.svelte";
  import { profile } from "../lib/profile.svelte";

  let { why = "Sign in to buy — your licence is tied to your account, and djmanzo picks it up from there." }: { why?: string } = $props();
  let name = $state(profile.p.name);
  let busy = $state(false);
  let error = $state<string | null>(null);

  const ICON: Record<string, string> = { google: "G", github: "", discord: "", facebook: "f", microsoft: "⊞" };

  async function demo() {
    busy = true;
    error = null;
    try {
      await session.demoSignIn(name || "Guest DJ");
    } catch (e) {
      error = (e as Error).message;
    } finally {
      busy = false;
    }
  }
</script>

<div class="signin">
  <p>{why}</p>
  {#if session.providers.length}
    <div class="providers">
      {#each session.providers as p (p.id)}
        <a class="btn provider {p.id}" href={session.signInUrl(p.id)}><span class="i" aria-hidden="true">{ICON[p.id] ?? "→"}</span>Continue with {p.name}</a>
      {/each}
    </div>
  {/if}
  {#if !session.providers.length || session.demo}
    <div class="demo notice">
      <p><b>Demo sign-in.</b> {session.providers.length ? "The shop is in demo mode" : "No sign-in provider is connected to this site yet"}: you can walk through everything with a name, no password, and nothing is charged. A demo account's licences are marked as demo in djmanzo.</p>
      <form onsubmit={(e) => { e.preventDefault(); void demo(); }}>
        <input type="text" bind:value={name} placeholder="Your DJ name" aria-label="Your DJ name" maxlength="40" />
        <button class="btn primary" disabled={busy}>{busy ? "…" : "Sign in for the demo"}</button>
      </form>
    </div>
  {/if}
  {#if error}<p class="notice" role="alert">{error}</p>{/if}
</div>

<style>
  .providers {
    display: grid;
    gap: 8px;
    margin-bottom: 12px;
  }
  .provider {
    justify-content: flex-start;
    background: #fff;
    color: #1d1d1f;
    border-color: #ddd;
  }
  .provider .i {
    width: 22px;
    font-weight: 900;
  }
  .github {
    background: #24292f;
    color: #fff;
  }
  .discord {
    background: #5865f2;
    color: #fff;
  }
  .facebook {
    background: #1877f2;
    color: #fff;
  }
  form {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  form input {
    flex: 1;
    min-width: 180px;
  }
</style>
