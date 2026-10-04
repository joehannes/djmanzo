<script lang="ts">
  /**
   * §119: YouTube's live chat, read into the crowd as it arrives.
   *
   * The reading is Rust's (`dj_app::youtube`): one thread signs in with what
   * Google gave, finds the DJ's broadcast that is live and reads its chat at
   * YouTube's own pace. This keeps the switch and the Google client, starts
   * the sign-in in the DJ's own browser, and says what the reading is doing.
   * What Google gives on signing in goes to the keychain and is never shown.
   */
  import {
    youtubeSettings,
    youtubeSettingsSet,
    youtubeSignIn,
    youtubeSignOut,
    youtubeStatus,
    type YouTubeStatus,
  } from "./api";

  let on = $state(false);
  let clientId = $state("");
  let clientSecret = $state("");
  let status = $state<YouTubeStatus | null>(null);
  let error = $state("");
  let signingIn = $state(false);

  $effect(() => {
    void youtubeSettings()
      .then((kept) => {
        on = kept.on;
        clientId = kept.client_id;
        clientSecret = kept.client_secret;
      })
      .catch((e) => (error = String(e)));
  });

  $effect(() => {
    const look = () =>
      void youtubeStatus()
        .then((now) => (status = now))
        .catch(() => {});
    look();
    const poll = setInterval(look, 2_000);
    return () => clearInterval(poll);
  });

  async function keep() {
    error = "";
    try {
      status = await youtubeSettingsSet(on, clientId.trim(), clientSecret.trim());
    } catch (e) {
      error = String(e);
    }
  }

  async function signIn() {
    signingIn = true;
    error = "";
    try {
      status = await youtubeSignIn();
    } catch (e) {
      error = String(e);
    } finally {
      signingIn = false;
    }
  }

  async function signOut() {
    error = "";
    try {
      status = await youtubeSignOut();
    } catch (e) {
      error = String(e);
    }
  }
</script>

<div class="youtube" data-youtube>
  <label class="switch">
    <input
      type="checkbox"
      bind:checked={on}
      onchange={() => void keep()}
    />
    Read my YouTube live chat into the crowd
  </label>

  <div class="client">
    <label>
      Google client id
      <input
        class="mono"
        aria-label="Google client id"
        placeholder="…apps.googleusercontent.com"
        bind:value={clientId}
        onchange={() => void keep()}
      />
    </label>
    <label>
      Client secret <span class="dim">(if Google asks for one)</span>
      <input class="mono" aria-label="Google client secret" bind:value={clientSecret} onchange={() => void keep()} />
    </label>
  </div>

  {#if status}
    {#if !status.client}
      <p class="how" data-youtube-how>
        djmanzo signs in as a Google Cloud app, and reads with it. Until djmanzo's
        own is published, use yours: in the Google Cloud console make a project,
        enable the <em>YouTube Data API v3</em>, set up the <em>OAuth consent
        screen</em> with the <span class="mono">youtube.readonly</span> scope, and
        under <em>Credentials</em> create an <em>OAuth client ID</em> of type
        <em>Desktop app</em>. Paste its id here.
      </p>
    {:else if !status.signed_in}
      <div class="row">
        <button class="primary" disabled={signingIn} onclick={() => void signIn()}>
          {signingIn ? "Finish signing in in your browser…" : "Sign in with Google"}
        </button>
        <span class="dim">Read only: djmanzo cannot post, delete or moderate anything.</span>
      </div>
    {:else}
      <div class="row">
        <span data-youtube-state={status.reading ? "reading" : "waiting"}>
          {#if !status.on}
            Signed in. Switch it on to read your chat.
          {:else if status.reading}
            Reading the chat of <strong>{status.reading}</strong> · {status.read} brought in
          {:else}
            Signed in.
          {/if}
        </span>
        <button onclick={() => void signOut()}>Sign out</button>
      </div>
    {/if}
    {#if status.note}
      <p class="note" data-youtube-note>{status.note}</p>
    {/if}
  {/if}
  {#if error}
    <p class="error" role="alert">{error}</p>
  {/if}
</div>

<style>
  .youtube {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .switch {
    display: flex;
    gap: 0.4rem;
    align-items: baseline;
  }

  .client {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
  }

  .client label {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    flex: 1 1 14rem;
    color: var(--text-dim);
    font-size: 0.85em;
  }

  .client input {
    font: inherit;
    font-size: 1.1em;
    padding: 0.2rem 0.35rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-s, 4px);
    background: var(--sunken);
    color: var(--text);
  }

  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.5rem;
  }

  .dim,
  .how,
  .note {
    color: var(--text-dim);
  }

  .how,
  .note {
    margin: 0;
    font-size: 0.85em;
    line-height: 1.4;
  }

  .error {
    margin: 0;
    color: var(--danger);
  }
</style>
