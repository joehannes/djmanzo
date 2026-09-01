<script lang="ts">
  /**
   * What the room is asking for.
   *
   * # The order of this panel is the order of the night
   *
   * A DJ opens this twice: once at the start, to get the page running and put
   * the QR somewhere people can see it, and then repeatedly for the rest of
   * the night to read the list. So the list is first and the setup is second,
   * folded away once it has been used. A panel that keeps a QR code at the top
   * all night is a panel that makes the DJ scroll past the answer to reach the
   * question.
   *
   * # Why every row has "Find it"
   *
   * A request is a name, and a name is not a record. The gesture that matters
   * is read it, find it, load it — and without the middle step the DJ reads a
   * name off one panel and retypes it into another, in a dark room, with one
   * hand. So the row hands its text to the search box beside it.
   *
   * # Why played and passed are both here
   *
   * A list that only removes what was played grows all night with things that
   * are never going to happen — the wrong genre, the record nobody owns, the
   * fourth request for the same song from the same table. "Pass" is how the
   * list stays short enough to be read at a glance, and the ask stays in the
   * book rather than vanishing, so the room's evening can still be read back
   * afterwards.
   */
  import {
    audienceAll,
    audienceLanguages,
    audienceOpen,
    audienceSettings,
    audienceSettle,
    audienceSheet,
    audienceStart,
    audienceStatus,
    audienceStop,
    audienceWaiting,
    type Ask,
    type AudienceStatus,
  } from "./api";
  import IconButton from "./controls/IconButton.svelte";
  import { save } from "@tauri-apps/plugin-dialog";
  import { onMount } from "svelte";

  interface Props {
    enabled: boolean;
    /** Hand a request's text to the search box, which is the next gesture. */
    onFind?: (text: string) => void;
  }

  let { enabled, onFind }: Props = $props();

  /**
   * How often the list is re-read.
   *
   * Three seconds. Requests arrive at the speed of somebody typing on a phone,
   * and a DJ glancing at this wants to see the one that just came in — but the
   * list is also the thing being read while deciding what to play next, so a
   * list that reorders under the eyes every half second is unreadable.
   */
  const REFRESH_MS = 3000;

  let status = $state<AudienceStatus | null>(null);
  let asks = $state<Ask[]>([]);
  let languages = $state<[string, string][]>([]);
  let error = $state("");
  /** What just happened, when it is not an error — where a sheet went. */
  let note = $state("");
  /** Whether the setup section is unfolded. Open until the page is running. */
  let setup = $state(true);
  /** Show everything, including what has been played and passed. */
  let showSettled = $state(false);
  let headingDraft = $state<string | null>(null);

  async function load() {
    try {
      status = await audienceStatus();
      asks = showSettled ? await audienceAll() : await audienceWaiting();
      error = "";
    } catch (e) {
      error = String(e);
    }
  }

  onMount(async () => {
    if (!enabled) return;
    try {
      languages = await audienceLanguages();
    } catch (e) {
      error = String(e);
    }
  });

  $effect(() => {
    if (!enabled) return;
    void load();
    const timer = setInterval(() => void load(), REFRESH_MS);
    return () => clearInterval(timer);
  });

  async function act(work: () => Promise<unknown>) {
    try {
      await work();
      await load();
      error = "";
    } catch (e) {
      error = String(e);
    }
  }

  const start = () =>
    act(async () => {
      status = await audienceStart();
      // Folded away the moment it has done its job.
      setup = !status.running;
    });

  const stop = () =>
    act(async () => {
      status = await audienceStop();
      setup = true;
    });

  /**
   * Print a sheet of stickers.
   *
   * Saved where the DJ chose and then handed to the operating system, rather
   * than opened in a window from here: `window.open` inside this webview
   * returns an object and opens nothing at all, which is a button that looks
   * like it worked. The file is the deliverable; opening it is the
   * convenience, and the two are reported separately so a machine that cannot
   * open it still says where the sheet is.
   */
  async function printSheet(kind: string) {
    const picked = await save({
      defaultPath: "djmanzo-stickers.html",
      filters: [{ name: "Sticker sheet", extensions: ["html"] }],
    });
    if (typeof picked !== "string") return;
    try {
      const opened = await audienceSheet(kind, picked, 12);
      note = opened
        ? `Wrote ${picked} and opened it — print it from there.`
        : `Wrote ${picked}. Open it in a browser and print it.`;
      error = "";
    } catch (e) {
      error = String(e);
    }
  }

  /** Local time: a DJ reads a night against the night they played it. */
  function clock(unixSeconds: number): string {
    return new Date(unixSeconds * 1000).toLocaleTimeString(undefined, {
      hour: "2-digit",
      minute: "2-digit",
    });
  }

  let heading = $derived(headingDraft ?? status?.heading ?? "");
</script>

<div class="requests">
  {#if error}
    <p class="error">{error}</p>
  {:else if note}
    <p class="note">{note}</p>
  {/if}

  <!--
    One strip, always visible: whether the room can reach the page, whether it
    is being listened to, and how many are waiting. Everything a glance is for.
  -->
  <div class="strip">
    {#if status?.running}
      <span class="live" title="The page is reachable from the room">Live</span>
      <button
        class="toggle"
        class:shut={!status.open}
        onclick={() => act(() => audienceOpen(!status!.open))}
        title={status.open
          ? "Stop taking requests. The list stays visible to the room."
          : "Take requests again."}
      >
        {status.open ? "Taking requests" : "Requests closed"}
      </button>
      <span class="count">{status.waiting} waiting</span>
      <button class="quiet" onclick={stop}>Turn off</button>
    {:else}
      <button class="go" onclick={start}>Take requests from the room</button>
      {#if status?.error}<span class="why">{status.error}</span>{/if}
    {/if}
  </div>

  {#if status?.running}
    <div class="lens">
      <label>
        <input
          type="checkbox"
          bind:checked={showSettled}
          onchange={() => void load()}
        />
        Show played and passed
      </label>
    </div>
  {/if}

  {#if asks.length === 0}
    <p class="empty">
      {#if !status?.running}
        Nothing is running. Turning this on opens a page the room can reach from
        a QR code, where people type what they want to hear. Nothing they send
        can touch a deck — the page takes a song's name and nothing else.
      {:else}
        Nobody has asked for anything yet. Put the QR code where people can see
        it: on a table, at the bar, or on a screen.
      {/if}
    </p>
  {:else}
    <ul class="asks">
      {#each asks as ask (ask.id)}
        <li class:settled={ask.standing !== "waiting"}>
          <span class="voices" title="How many people asked for this"
            >{ask.voices}</span
          >
          <span class="what" title={ask.text}>{ask.text}</span>
          <span class="mono when">{clock(ask.first_asked)}</span>
          {#if ask.standing === "waiting"}
            <button
              class="find"
              onclick={() => onFind?.(ask.text)}
              title="Search the library for this"
            >
              Find it
            </button>
            <IconButton
              icon="fa-solid fa-check"
              title="Played — take it off the list"
              onClick={() => act(() => audienceSettle(ask.id, "played"))}
            />
            <IconButton
              icon="fa-solid fa-xmark"
              title="Not happening — take it off the list"
              onClick={() => act(() => audienceSettle(ask.id, "passed"))}
            />
          {:else}
            <span class="standing">{ask.standing}</span>
            <IconButton
              icon="fa-solid fa-rotate-left"
              title="Back onto the list"
              onClick={() => act(() => audienceSettle(ask.id, "waiting"))}
            />
          {/if}
        </li>
      {/each}
    </ul>
  {/if}

  <!--
    Setup, folded once it has been used. It is needed at the start of a night
    and then not again, which is exactly the shape of a details element.
  -->
  <details class="setup" bind:open={setup}>
    <summary>The way in, and what the room sees</summary>

    {#if status?.running && status.ways_in.length > 0}
      <div class="ways">
        {#each status.ways_in as way (way.kind)}
          <div class="way">
            {#if way.qr}
              <!--
                The QR comes from Rust as an SVG rather than being drawn here:
                the same square goes onto the printed sheet, and two
                implementations of one code is one of them being subtly wrong.
                eslint-disable-next-line svelte/no-at-html-tags -- built by
                dj_net::sticker from a URL djmanzo composed, never from input.
              -->
              <div class="qr">{@html way.qr}</div>
            {/if}
            <div class="about">
              <p class="url mono">{way.url}</p>
              <p class="caveat">{way.caveat}</p>
              <button onclick={() => printSheet(way.kind)}>
                Print a sheet of stickers
              </button>
            </div>
          </div>
        {/each}
      </div>
      {#if status.announce_error}
        <p class="note">
          The printable address is not being answered for: {status.announce_error}.
          The address below still works on this network — show it on a screen,
          or print it here at the venue.
        </p>
      {/if}
    {:else if status?.running}
      <p class="note">
        This machine has no address on a local network, so there is nothing for
        a phone to reach. Join the venue's wifi and turn this off and on again.
      </p>
    {/if}

    <div class="fields">
      <label>
        What the page is called
        <input
          value={heading}
          placeholder="Tonight"
          oninput={(e) => (headingDraft = e.currentTarget.value)}
          onblur={() =>
            act(async () => {
              status = await audienceSettings({ heading });
              headingDraft = null;
            })}
        />
      </label>
      <label>
        The room's language
        <select
          value={status?.language ?? "en"}
          onchange={(e) =>
            act(async () => {
              status = await audienceSettings({
                language: e.currentTarget.value,
              });
            })}
        >
          {#each languages as [tag, name] (tag)}
            <option value={tag}>{name}</option>
          {/each}
        </select>
      </label>
      <label class="check">
        <input
          type="checkbox"
          checked={status?.show_playing ?? true}
          onchange={(e) =>
            act(async () => {
              status = await audienceSettings({
                showPlaying: e.currentTarget.checked,
              });
            })}
        />
        Tell the room what is playing
      </label>
    </div>
    <!--
      Said plainly because it is a real trade and not an obvious one: the track
      name on the page is how a request stops being "play that one song" and
      starts being useful, and it is also how a set gets written down by
      somebody at the bar.
    -->
    <p class="note">
      Showing what is playing stops people asking for the record that is already
      on. It also means anyone in the room can read your set as you play it.
    </p>
  </details>
</div>

<style>
  .requests {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    flex: 1;
    min-height: 0;
  }

  .strip {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex: none;
    flex-wrap: wrap;
  }

  .live {
    font-size: 0.72em;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--accent);
    border: 1px solid var(--accent);
    border-radius: 999px;
    padding: 0.1rem 0.45rem;
  }

  .toggle.shut {
    border-color: var(--text-dim);
    color: var(--text-dim);
  }

  .count {
    font-size: 0.8em;
    color: var(--text-dim);
    margin-left: auto;
  }

  .why {
    font-size: 0.78em;
    color: var(--danger, #dc2626);
  }

  .lens {
    flex: none;
    font-size: 0.78em;
    color: var(--text-dim);
  }

  .lens label {
    display: flex;
    align-items: center;
    gap: 0.35rem;
  }

  /*
    The list scrolls, not the panel: the strip above must stay put.

    `min-height` is the load-bearing part. Without it the list is a flex child
    free to shrink to nothing, and unfolding the setup section below took the
    whole list away while the strip above still said "1 waiting" -- the panel
    hiding the one thing it exists to show, at the moment somebody was setting
    it up.
  */
  .asks {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    flex: 1 1 auto;
    min-height: 4.5rem;
    overflow: auto;
  }

  .asks li {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.35rem 0.5rem;
    border: 1px solid var(--border);
    border-radius: 6px;
  }

  .asks li.settled {
    opacity: 0.55;
    border-style: dashed;
  }

  /*
    The count first and in the accent colour, because it is what the list is
    sorted by and the reason one row matters more than the next.
  */
  .voices {
    flex: none;
    min-width: 1.6rem;
    text-align: center;
    font-variant-numeric: tabular-nums;
    color: var(--accent);
    font-weight: 600;
  }

  .what {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .when,
  .standing {
    flex: none;
    font-size: 0.75em;
    color: var(--text-dim);
  }

  .find {
    flex: none;
    font-size: 0.78em;
    padding: 0.1rem 0.4rem;
  }

  /*
    Allowed to shrink and scroll rather than push the list out. It is used once
    a night and then folded away, so it gives up room to the list instead of
    taking it.
  */
  .setup {
    flex: 0 1 auto;
    min-height: 0;
    overflow: auto;
    border-top: 1px solid var(--border);
    padding-top: 0.4rem;
  }

  /*
    Unfolded, it takes a share of the room instead of a sliver of it. A DJ who
    opens this is looking at the QR code and the print button, and the browser
    panel is short enough that without this they got a two-line window to
    scroll them through. The list keeps its floor above, so neither disappears.
  */
  .setup[open] {
    flex: 1 1 auto;
  }

  .setup summary {
    font-size: 0.8em;
    color: var(--text-dim);
    cursor: pointer;
  }

  .ways {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    margin: 0.5rem 0;
  }

  .way {
    display: flex;
    gap: 0.6rem;
    align-items: flex-start;
  }

  /*
    No background and no padding: the SVG carries its own white ground and its
    own quiet zone, because the square is read by a camera and a QR without a
    margin of light around it does not scan. Painting a second one here would
    be the same decision made twice, in two places, in two units.
  */
  .qr {
    flex: none;
    width: 6.5rem;
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
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }

  .url {
    margin: 0;
    font-size: 0.85em;
    overflow-wrap: anywhere;
  }

  .caveat,
  .note,
  .empty,
  .error {
    margin: 0;
    font-size: 0.78em;
    line-height: 1.5;
    color: var(--text-dim);
  }

  .error {
    color: var(--danger, #dc2626);
  }

  .fields {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    margin: 0.5rem 0;
  }

  .fields label {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    font-size: 0.78em;
    color: var(--text-dim);
  }

  .fields label.check {
    flex-direction: row;
    align-items: center;
    gap: 0.35rem;
  }
</style>
