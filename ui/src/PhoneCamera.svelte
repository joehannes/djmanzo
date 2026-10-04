<script lang="ts">
  /**
   * §119: a phone as a camera, from djmanzo's own page.
   *
   * The serving is Rust's (`dj_app::phone_camera`, over `dj_net::phone`): a
   * page for the phone over HTTPS on this network, a page for OBS on this
   * computer, and the introduction between them. The picture goes from the
   * phone straight to OBS; djmanzo never holds a frame of it. This is the
   * switch, the code for the phone, the address for OBS, and what each of
   * them is doing.
   */
  import { phoneCameraNewLink, phoneCameraSet, phoneCameraStatus, type PhoneCameraStatus } from "./api";

  let status = $state<PhoneCameraStatus | null>(null);
  let showCode = $state(false);
  let copied = $state(false);
  let error = $state("");

  function take(now: PhoneCameraStatus) {
    // The code goes away once a phone is sending: a screen in a room is
    // photographed, and the code is the way in. It can be shown again.
    if (now.phone === "sending" && status?.phone !== "sending") showCode = false;
    status = now;
  }

  $effect(() => {
    const look = () =>
      void phoneCameraStatus()
        .then(take)
        .catch((e) => (error = String(e)));
    look();
    const poll = setInterval(look, 2_000);
    return () => clearInterval(poll);
  });

  async function turn(on: boolean) {
    error = "";
    try {
      take(await phoneCameraSet(on));
      showCode = on;
    } catch (e) {
      error = String(e);
    }
  }

  async function newLink() {
    error = "";
    try {
      take(await phoneCameraNewLink());
      showCode = true;
    } catch (e) {
      error = String(e);
    }
  }

  async function copy(what: string) {
    try {
      await navigator.clipboard.writeText(what);
      copied = true;
      setTimeout(() => (copied = false), 1_500);
    } catch {
      // No clipboard: the address is selectable, which is the fallback.
    }
  }

  const phoneSays = {
    away: "No phone has the page open.",
    open: "A phone has the page open.",
    sending: "A phone is sending its camera.",
  } as const;
  const obsSays = {
    away: "OBS's page is not open.",
    open: "OBS's page is open, waiting for a phone.",
    showing: "OBS is showing the phone.",
  } as const;
</script>

<div class="phone" data-phone-camera>
  <label class="switch">
    <input
      type="checkbox"
      checked={status?.on ?? false}
      onchange={(e) => void turn(e.currentTarget.checked)}
    />
    Let a phone be a camera
  </label>

  {#if status && !status.on}
    <p class="dim">
      The phone opens a page djmanzo serves on this network and sends its camera
      to OBS, straight from the phone, without leaving the network. Nothing else
      to install; the phone only has to be on the same wifi as this computer.
    </p>
  {:else if status}
    <section aria-label="On the phone">
      <h3>On the phone</h3>
      {#if status.phone_url}
        <div class="row">
          <button onclick={() => (showCode = !showCode)}>
            {showCode ? "Hide the code" : "Show the code"}
          </button>
          <button
            onclick={() => void newLink()}
            title="The old code stops working, and a phone using it is cut off. OBS's address stays."
          >
            New phone link
          </button>
        </div>
        {#if showCode}
          <div class="code" data-phone-code>
            {#if status.qr}
              <!--
                eslint-disable-next-line svelte/no-at-html-tags -- built by
                dj_net::sticker from an address djmanzo composed, never from input.
              -->
              <div class="qr">{@html status.qr}</div>
            {/if}
            <div class="about">
              <p class="url mono">{status.phone_url}</p>
              <p class="dim">
                The phone's browser warns once that the connection is not private:
                djmanzo vouches for itself. Tap <em>Advanced → Proceed</em> (Chrome)
                or <em>Show Details → visit this website</em> (Safari). The page
                offers djmanzo's certificate, to install once and not be warned again.
              </p>
            </div>
          </div>
        {/if}
      {/if}
    </section>

    <section aria-label="In OBS">
      <h3>In OBS</h3>
      {#if status.obs_url}
        <p class="dim">Add a <em>Browser</em> source with this address, 1280 × 720:</p>
        <div class="row">
          <input class="mono address" readonly value={status.obs_url} aria-label="Address for OBS" />
          <button onclick={() => void copy(status!.obs_url!)}>{copied ? "Copied" : "Copy"}</button>
        </div>
      {/if}
    </section>

    <ul class="now" aria-label="What each is doing">
      <li data-phone-now={status.phone}>{phoneSays[status.phone]}</li>
      <li data-obs-now={status.viewer}>{obsSays[status.viewer]}</li>
    </ul>

    {#if status.fingerprint}
      <details class="dim">
        <summary>djmanzo's certificate</summary>
        <p>Its fingerprint, as the phone shows it under the certificate's details:</p>
        <p class="mono print">{status.fingerprint}</p>
      </details>
    {/if}
  {/if}

  {#if status?.note}
    <p class="note" role="status">{status.note}</p>
  {/if}
  {#if error}
    <p class="error" role="alert">{error}</p>
  {/if}
</div>

<style>
  .phone {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    overflow: auto;
    min-height: 0;
  }

  .switch {
    display: flex;
    gap: 0.4rem;
    align-items: baseline;
    font-weight: 600;
  }

  h3 {
    margin: 0 0 0.3rem;
    font-size: 0.95em;
  }

  section {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
    align-items: center;
  }

  .code {
    display: flex;
    flex-wrap: wrap;
    gap: 0.8rem;
    align-items: flex-start;
  }

  .qr {
    flex: none;
    width: 11rem;
    border-radius: 4px;
    overflow: hidden;
    line-height: 0;
  }

  .qr :global(svg) {
    width: 100%;
    height: auto;
    display: block;
  }

  .about {
    flex: 1 1 14rem;
    min-width: 0;
  }

  .about p {
    margin: 0 0 0.4rem;
  }

  .url,
  .print {
    word-break: break-all;
  }

  .address {
    flex: 1 1 16rem;
    min-width: 0;
    font: inherit;
    padding: 0.2rem 0.35rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-s, 4px);
    background: var(--sunken);
    color: var(--text);
  }

  .now {
    margin: 0;
    padding-left: 1.1rem;
  }

  [data-phone-now="sending"],
  [data-obs-now="showing"] {
    color: var(--success);
  }

  .dim,
  .note {
    color: var(--text-dim);
    margin: 0;
    font-size: 0.9em;
    line-height: 1.4;
  }

  .error {
    margin: 0;
    color: var(--danger);
  }
</style>
