<script lang="ts">
  /**
   * §123: the words of a song of a guest's own, written by the DJ's AI
   * provider, ready to copy into a music service.
   *
   * Only for a guest who agreed to a song being made for them and has sung.
   * What the model is told is `dj_app::guests::brief` — their name, the song,
   * where and when, their favourites and languages, never their contact
   * details — and what comes back is checked in Rust before it is kept.
   */
  import {
    guestMessage,
    guestRevealVoice,
    guestSong,
    openSuno,
    type Guest,
    type GuestBook,
  } from "./api";
  import { sunoFill, sunoLink, whatsappHere } from "./sunoSend";

  let { guest, onbook }: { guest: Guest; onbook: (book: GuestBook) => void } = $props();

  /** The song they sang last, and so the one the song is about. */
  const last = $derived(guest.sang[guest.sang.length - 1]);
  const draft = $derived(last?.song ?? null);

  let language = $state("");
  let keywords = $state("");
  let writing = $state(false);
  let error = $state("");
  let copied = $state("");
  /** What went wrong with Suno, the voice file or the message: shown beside them. */
  let failed = $state("");
  /** The song's address on Suno, pasted by the DJ once it is made. */
  let link = $state("");
  let sent = $state("");
  /**
   * Karaoke beta B5: the words to put in Suno's form, which language; what
   * the fill said; and whether the link came from Suno's window by itself.
   */
  let fillIn = $state("");
  let filled = $state("");
  let caught = $state(false);
  /** Suno's window is open: its address is read for the song's link. */
  let watching = $state(false);
  const fillLanguage = $derived(
    draft?.versions.some((v) => v.language === fillIn) ? fillIn : (draft?.versions[0]?.language ?? ""),
  );
  async function fill() {
    failed = "";
    filled = "";
    const words = draft?.versions.find((v) => v.language === fillLanguage);
    try {
      // On the clipboard too: a box Suno has renamed is then one paste away.
      if (words) await navigator.clipboard.writeText(words.lyrics).catch(() => undefined);
      const done = await sunoFill(guest.id, fillLanguage);
      watching = true;
      filled = `The ${done.language} words and the style went to Suno's window, and the words are on your clipboard. Suno's window says what it filled — check them there, then press Create. ${
        done.waits
          ? "Not signed in yet? Sign in there: the form fills itself once Create opens."
          : "Not signed in yet? Sign in there, open Create, then press this again."
      }`;
    } catch (e) {
      failed = String(e);
    }
  }
  $effect(() => {
    if (!watching || link.trim() !== "") return;
    let alive = true;
    const look = async () => {
      while (alive && link.trim() === "") {
        try {
          const found = await sunoLink();
          if (alive && found && link.trim() === "") {
            link = found;
            caught = true;
          }
        } catch {
          // No window, no link: the DJ can paste it.
        }
        await new Promise((r) => setTimeout(r, 2_000));
      }
    };
    void look();
    return () => {
      alive = false;
    };
  });
  async function sendHere() {
    failed = "";
    sent = "";
    try {
      sent = await whatsappHere(guest.id, link.trim());
    } catch (e) {
      failed = String(e);
    }
  }

  const voice = $derived(guest.take ?? [...guest.sang].reverse().find((song) => song.voice)?.voice ?? null);
  // A number or an address on the record is already consent: Rust clears
  // them on save unless the guest agreed to be contacted, and `compose`
  // checks again before anything opens.
  const canWhatsApp = $derived(guest.phone.trim() !== "");
  const canMail = $derived(guest.email.trim() !== "");

  async function act(what: () => Promise<unknown>) {
    failed = "";
    try {
      await what();
    } catch (e) {
      failed = String(e);
    }
  }

  async function send(reach: "whatsapp" | "mail") {
    failed = "";
    sent = "";
    try {
      sent = await guestMessage(guest.id, reach, link.trim());
    } catch (e) {
      failed = String(e);
    }
  }

  async function write() {
    if (!last) return;
    writing = true;
    error = "";
    const date = new Date(last.at * 1000).toLocaleDateString(undefined, {
      weekday: "long",
      day: "numeric",
      month: "long",
      year: "numeric",
    });
    try {
      onbook(await guestSong(guest.id, language.trim(), keywords.trim(), date));
    } catch (e) {
      error = String(e);
    } finally {
      writing = false;
    }
  }

  async function copy(what: string, text: string) {
    try {
      await navigator.clipboard.writeText(text);
      copied = what;
      setTimeout(() => {
        if (copied === what) copied = "";
      }, 2000);
    } catch (e) {
      error = `Could not copy: ${String(e)}`;
    }
  }
</script>

{#if last}
  <div class="song" role="group" aria-label="A song for {guest.name}">
    <h4>A song for {guest.name}</h4>
    <p class="hint">
      Words about tonight — {last.title}{last.event ? `, ${last.event}` : ""}{last.place ? `, ${last.place}` : ""} —
      written by your AI provider. It is told their name, the song, where and when, their favourites and languages;
      not how to reach them.
    </p>
    <div class="ask">
      <label
        >Sung in <input
          bind:value={language}
          size="8"
          placeholder="its own"
          title="The language of the song they sang — or empty, for the AI to know it from the song"
        /></label
      >
      <label class="wide"
        >Your ideas <input bind:value={keywords} placeholder="first time on stage, the whole table sang along" /></label
      >
      <button type="button" disabled={writing} onclick={() => void write()}>
        {writing ? "Writing…" : draft ? "Write them again" : "Write the words"}
      </button>
    </div>
    {#if error}
      <p class="error" role="alert">{error}</p>
    {/if}

    {#if draft}
      <div class="style">
        <span class="label">Style</span>
        <span class="text" data-style>{draft.style}</span>
        <button type="button" onclick={() => void copy("style", draft.style)}
          >{copied === "style" ? "Copied" : "Copy the style"}</button
        >
      </div>
      {#each draft.versions as version (version.language)}
        <section class="version" aria-label="Words in {version.language}">
          <div class="head">
            <h5>{version.language}</h5>
            <button type="button" onclick={() => void copy(version.language, version.lyrics)}
              >{copied === version.language ? "Copied" : `Copy the ${version.language} words`}</button
            >
          </div>
          <pre>{version.lyrics}</pre>
        </section>
      {/each}

      <div class="make" role="group" aria-label="Make it on Suno">
        <!-- B5: an autofill and no more — the DJ presses Create. -->
        <button type="button" class="primary" onclick={() => void fill()}>Fill in Suno's form</button>
        {#if draft.versions.length > 1}
          <select
            aria-label="Words to fill in"
            value={fillLanguage}
            onchange={(e) => (fillIn = e.currentTarget.value)}
          >
            {#each draft.versions as version (version.language)}
              <option value={version.language}>{version.language}</option>
            {/each}
          </select>
        {/if}
        <button
          type="button"
          onclick={() =>
            void act(async () => {
              await openSuno();
              watching = true;
            })}>Open Suno</button
        >
        {#if voice}
          <button type="button" onclick={() => void act(() => guestRevealVoice(guest.id))}>Show their voice file</button>
        {/if}
        {#if filled}
          <p class="filled" role="status">{filled}</p>
        {/if}
        <p class="hint">
          Sign in to Suno once in its window. <em>Fill in Suno's form</em> puts in the words and the style{voice
            ? "; upload their voice yourself"
            : ""}; you check them and press Create — Suno's terms do not allow djmanzo to make the song for you.
          Open the finished song there and its link comes here by itself.
        </p>
      </div>

      {#if canWhatsApp || canMail}
        <div class="send" role="group" aria-label="Send it to {guest.name}">
          <label class="wide"
            >The song's link <input
              bind:value={link}
              oninput={() => (caught = false)}
              placeholder="https://suno.com/s/…"
            /></label
          >
          {#if canWhatsApp}
            <!-- B5: WhatsApp Web in a window of djmanzo's own; the app beside it. -->
            <button
              type="button"
              class="primary"
              title="WhatsApp Web, in a window of djmanzo's own — sign in once with your phone"
              onclick={() => void sendHere()}>In djmanzo's window</button
            >
            <button type="button" onclick={() => void send("whatsapp")}>WhatsApp</button>
          {/if}
          {#if canMail}
            <button type="button" onclick={() => void send("mail")}>Mail</button>
          {/if}
          {#if caught}
            <p class="caught" data-caught>The link came from Suno's window.</p>
          {/if}
          <p class="hint">
            Opens with the message written and {guest.name} as the recipient; you press send.{canWhatsApp
              ? " In djmanzo's window is WhatsApp Web; WhatsApp is the app on this computer."
              : ""}
          </p>
          {#if sent}
            <p class="sent" role="status">{sent}</p>
          {/if}
        </div>
      {:else}
        <p class="hint">
          To send it from here, {guest.name} needs to agree to be contacted, with a number or an address on their
          record.
        </p>
      {/if}
      {#if failed}
        <p class="error" role="alert">{failed}</p>
      {/if}
    {/if}
  </div>
{/if}

<style>
  .song {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    border-top: 1px solid var(--border);
    padding-top: 0.5rem;
  }

  h4,
  h5 {
    margin: 0;
    font-size: 0.95em;
  }

  .hint {
    margin: 0;
    color: var(--text-dim);
    font-size: 0.85em;
  }

  .error {
    margin: 0;
    color: var(--danger);
  }
  .filled,
  .caught {
    margin: 0;
    font-size: 0.85em;
    color: var(--success);
  }

  .ask {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
    align-items: center;
  }

  label {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    font-size: 0.85em;
    color: var(--text-dim);
  }

  label.wide {
    flex: 1 1 14rem;
  }

  label.wide input {
    flex: 1;
  }

  input {
    font: inherit;
    background: var(--panel-raised);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 0.2rem 0.35rem;
    min-width: 0;
  }

  .style {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
    align-items: baseline;
  }

  .label {
    font-size: 0.8em;
    color: var(--text-dim);
  }

  .text {
    flex: 1 1 12rem;
  }

  .make,
  .send {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
    align-items: center;
  }

  .make .hint,
  .send .hint,
  .sent {
    flex-basis: 100%;
  }

  .sent {
    margin: 0;
    font-size: 0.85em;
  }

  .version {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
  }

  .head {
    display: flex;
    gap: 0.5rem;
    align-items: center;
    justify-content: space-between;
  }

  pre {
    margin: 0;
    white-space: pre-wrap;
    font-family: inherit;
    font-size: 0.9em;
    background: var(--panel-raised);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 0.4rem 0.5rem;
    max-height: 14rem;
    overflow: auto;
  }
</style>
