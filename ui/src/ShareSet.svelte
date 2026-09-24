<script lang="ts">
  /**
   * Handing one night to somebody else.
   *
   * Sits under the session chips in the history view rather than in a window
   * of its own. A modal would be a new kind of thing for one feature, and the
   * decision being made here -- which night, and to whom -- is small enough
   * that it does not deserve to cover the library up.
   *
   * # Why the message is shown before anything opens
   *
   * Because this is the last moment djmanzo controls. Once WhatsApp has the
   * text it belongs to a person and a chat window, and "wait, not that one"
   * is no longer something software can offer. So the DJ reads it here.
   *
   * It is also where a four-hour set announces that it does not fit in a link,
   * while there is still a file to choose instead.
   */
  import { untrack } from "svelte";
  import { save } from "@tauri-apps/plugin-dialog";
  import IconButton from "./controls/IconButton.svelte";
  import {
    exportSession,
    shareChannels,
    sharePreview,
    shareTo,
    type Share,
    type ShareChannel,
  } from "./api";

  interface Props {
    /** Which night. */
    session: string;
    /** Close and go back to the list. */
    onclose: () => void;
  }

  let { session, onclose }: Props = $props();

  /**
   * What goes above the tracklist.
   *
   * Starts as the session's own name, because that is already how the DJ
   * refers to this night, and stays editable because "2026-08-22" is not what
   * anybody wants to read in a group chat.
   */
  let heading = $state(untrack(() => session));

  /*
    A different night is a different heading.

    Svelte reuses this component when the chosen session changes, so without
    this the DJ picks Saturday, then Friday, and the sheet still says
    "Saturday" over Friday's tracklist. `untrack` above states that the
    initial value is a starting point rather than a binding; this is what
    keeps it right afterwards.
  */
  $effect(() => {
    heading = session;
  });
  let share = $state<Share | null>(null);
  /**
   * §108: where it goes. WhatsApp first, as it always was; X, Bluesky and
   * Threads beside it. Each holds a different amount — a post on X a handful
   * of records — so the preview is the chosen channel's, cut where it cuts.
   */
  let channels = $state<ShareChannel[]>([]);
  let channel = $state("whatsapp");
  let chosen = $derived(channels.find((c) => c.slug === channel) ?? null);
  let error = $state("");
  let note = $state("");
  let busy = $state(false);

  /*
    Re-read whenever the heading changes, because the heading is inside the
    message and inside the length budget with it -- a long title is a record
    that no longer fits, and the DJ should watch that happen rather than be
    told about it afterwards.
  */
  $effect(() => {
    void shareChannels()
      .then((found) => (channels = found))
      .catch(() => {});
  });

  $effect(() => {
    const wanted = heading;
    const forSession = session;
    const forChannel = channel;
    void (async () => {
      try {
        const next = await sharePreview(forSession, wanted, forChannel);
        // Ignore a reply that arrived after the DJ typed again.
        if (wanted === heading && forSession === session && forChannel === channel) {
          share = next;
          error = "";
        }
      } catch (e) {
        error = String(e);
        share = null;
      }
    })();
  });

  async function toChannel() {
    busy = true;
    note = "";
    try {
      const sent = await shareTo(session, heading, channel);
      share = sent;
      // Deliberately not "sent" or "posted". Nothing has been: the composer is
      // open with the message in it, and a person still has to press the
      // button. Saying otherwise would be a lie a DJ acts on.
      const name = chosen?.name ?? channel;
      note =
        channel === "whatsapp"
          ? "WhatsApp is open with the message ready — pick a chat and send."
          : `${name} is open with the post written — read it, and post it from there.`;
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function toFile() {
    const picked = await save({
      defaultPath: `${session}.txt`,
      filters: [{ name: "Set list", extensions: ["txt"] }],
    });
    if (typeof picked !== "string") return;
    busy = true;
    note = "";
    try {
      const count = await exportSession(session, picked);
      note = `Wrote all ${count} track${count === 1 ? "" : "s"} to ${picked}`;
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<section class="share">
  <header>
    <label>
      <span>Share</span>
      <input
        bind:value={heading}
        placeholder="what to call this night"
        aria-label="Heading for the shared set"
      />
    </label>
    <IconButton icon="fa-solid fa-xmark" title="Close" onClick={onclose} />
  </header>

  {#if error}
    <p class="error">{error}</p>
  {:else if share}
    <!--
      Read-only rather than editable. The tracklist is a record of what was
      played, and an editable one is a record of what the DJ would rather
      have played.
    -->
    {#if channels.length > 1}
      <div class="channels" role="radiogroup" aria-label="Where to share it">
        {#each channels as option (option.slug)}
          <button
            type="button"
            role="radio"
            aria-checked={channel === option.slug}
            class:chosen={channel === option.slug}
            data-channel={option.slug}
            onclick={() => (channel = option.slug)}
          >
            {option.name}
          </button>
        {/each}
      </div>
    {/if}
    <pre class="preview" data-preview>{share.message}</pre>

    <!--
      Warning and result sit above the buttons, not below.

      In a short panel everything under the destinations is off the bottom
      edge, and what lands there is precisely what the DJ needs after
      pressing: whether it worked. Above the buttons it is also where the eye
      already is.
    -->
    {#if share.dropped > 0}
      <p class="warning" data-dropped>
        {#if chosen?.limit}
          {share.dropped} of {share.total} won't fit in a {chosen.limit}-character
          {chosen.name} post.
        {:else}
          {share.dropped} of {share.total} won't fit in a {chosen?.name ?? "WhatsApp"} link.
        {/if}
        The message says so, and the file has all of them.
      </p>
    {/if}

    {#if note}
      <p class="note">{note}</p>
    {/if}

    <div class="destinations">
      <button type="button" disabled={busy} onclick={toChannel} data-open>
        Open {chosen?.name ?? "WhatsApp"}
      </button>
      <button type="button" disabled={busy} onclick={toFile}>
        Save as file{#if share.dropped > 0} (all {share.total}){/if}
      </button>
    </div>
  {:else}
    <p class="note">Reading the night…</p>
  {/if}
</section>

<style>
  /*
    The sheet never grows past the panel it sits in.

    Same rule as the track table above it: a browser panel is short, and a
    block that pushes its own buttons off the bottom is a feature the DJ
    cannot reach. Only the preview scrolls -- the heading and the two
    destinations stay put, because those are the parts that have to be
    reachable at any panel height.
  */
  .share {
    display: flex;
    flex-direction: column;
    flex: 1 1 auto;
    min-height: 0;
    margin-top: 0.5rem;
    padding: 0.6rem 0.7rem;
    border: 1px solid var(--line, #333);
    border-radius: 6px;
    background: color-mix(in srgb, var(--accent-2, #4a8) 6%, transparent);
  }

  header,
  .channels,
  .destinations,
  .warning,
  .note,
  .error {
    flex: none;
  }

  header {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  header label {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    flex: 1;
    font-size: 0.8em;
    color: var(--text-dim);
  }

  header input {
    flex: 1;
    min-width: 6rem;
  }

  /*
    Monospaced and scrollable. This is the message as it will arrive, so it is
    shown as text and not reflowed into the surrounding design -- a preview
    that looks nicer than the thing it previews is not a preview.
  */
  .preview {
    /*
      The preview is the only part that gives.

      `min-height: 0` rather than a floor, and deliberately: the destinations
      above and below it are pinned, so in a cramped panel the preview
      squeezes down to a couple of scrollable lines and every control stays
      reachable. A floor here would look tidier and would push a button off
      the bottom edge instead, which is the one outcome that makes the
      feature unusable rather than merely tight.
    */
    flex: 1 1 auto;
    min-height: 0;
    margin: 0.5rem 0 0;
    padding: 0.5rem 0.6rem;
    max-height: none;
    overflow: auto;
    font-size: 0.75em;
    line-height: 1.5;
    white-space: pre-wrap;
    word-break: break-word;
    background: var(--panel, #1a1a1a);
    border-radius: 4px;
    user-select: text;
    -webkit-user-select: text;
  }

  .destinations {
    display: flex;
    gap: 0.4rem;
    margin-top: 0.5rem;
  }

  /* §108: where it goes, one choice among four, the chosen one filled. */
  .channels {
    display: flex;
    flex-wrap: wrap;
    gap: 0.3rem;
    margin-top: 0.45rem;
  }

  .channels button {
    font: inherit;
    font-size: 0.78em;
    padding: 0.18rem 0.55rem;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: transparent;
    color: var(--text);
    cursor: pointer;
  }

  .channels button.chosen {
    border-color: var(--selected);
    background: var(--selected);
    color: var(--on-selected);
  }

  .warning,
  .note,
  .error {
    margin: 0.45rem 0 0;
    font-size: 0.78em;
    line-height: 1.5;
  }

  .warning {
    color: var(--warn, #d97706);
  }

  .note {
    color: var(--text-dim);
  }

  .error {
    color: var(--danger, #dc2626);
  }
</style>
