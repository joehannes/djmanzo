<script lang="ts">
  /**
   * §123: the karaoke journal — the guests who sang, and what they agreed to.
   *
   * The rule is `dj_app::guests`: this draws it. A singer the rotation marks
   * as having sung is already here, with the song, agreeing to nothing; the
   * host opens them, asks the three questions exactly as Rust words them, and
   * fills in what the guest is happy to give. Contact details cannot be
   * typed until the guest agrees to them, and Rust clears them anyway if the
   * form sends them without it.
   */
  import { save as saveDialog } from "@tauri-apps/plugin-dialog";
  import {
    guestBook,
    guestExport,
    guestForget,
    guestSave,
    type Guest,
    type GuestBook,
  } from "./api";

  let { rev = 0 }: { rev?: number } = $props();

  let book = $state<GuestBook | null>(null);
  let editing = $state<Guest | null>(null);
  let error = $state("");
  let forgetting = $state(false);

  function blank(): Guest {
    return {
      id: "",
      name: "",
      age: null,
      email: "",
      phone: "",
      socials: [],
      home: "",
      nationality: "",
      favourite_band: "",
      favourite_genre: "",
      favourite_song: "",
      native_language: "",
      languages: [],
      notes: "",
      consent: { keep: false, contact: false, voice: false, given: 0, wording: 0 },
      since: 0,
      sang: [],
    };
  }

  async function refresh() {
    try {
      book = await guestBook();
      error = "";
    } catch (e) {
      error = String(e);
    }
  }

  // Again whenever the rotation changes: a song marked as sung is on a
  // guest's record the moment it is.
  $effect(() => {
    void rev;
    void refresh();
  });

  /** When a guest last sang, or was written down. */
  const latest = (guest: Guest) => guest.sang[guest.sang.length - 1]?.at ?? guest.since;

  /** The newest song first: tonight's singers at the top. */
  const ordered = $derived(
    [...(book?.guests ?? [])].sort(
      (a, b) => latest(b) - latest(a),
    ),
  );

  const young = $derived(
    editing?.age != null && book != null && editing.age < book.consent_age,
  );

  function open(guest: Guest) {
    editing = JSON.parse(JSON.stringify(guest)) as Guest;
    forgetting = false;
    error = "";
  }

  const list = (text: string) =>
    text
      .split(",")
      .map((part) => part.trim())
      .filter(Boolean);

  async function keep() {
    if (!editing) return;
    try {
      // A plain copy, not the form's live state: what is sent is what the
      // form said at the press.
      const next = await guestSave($state.snapshot(editing) as Guest);
      book = next;
      editing = null;
      error = "";
    } catch (e) {
      error = String(e);
    }
  }

  async function forget() {
    if (!editing?.id) return;
    try {
      book = await guestForget(editing.id);
      editing = null;
      forgetting = false;
    } catch (e) {
      error = String(e);
    }
  }

  async function exportTo(id: string | null, name: string) {
    try {
      const path = await saveDialog({
        defaultPath: id ? `${name}.json` : "karaoke-journal.csv",
        filters: id ? [{ name: "JSON", extensions: ["json"] }] : [{ name: "CSV", extensions: ["csv"] }],
      });
      if (!path) return;
      await guestExport(path, id);
    } catch (e) {
      error = String(e);
    }
  }

  const when = (seconds: number) =>
    seconds > 0 ? new Date(seconds * 1000).toLocaleDateString(undefined, { day: "numeric", month: "short", year: "numeric" }) : "";
</script>

<section class="guests" aria-labelledby="guest-book">
  <header>
    <h3 id="guest-book">Guest book</h3>
    <span class="count">{book?.guests.length ?? 0}</span>
    <button type="button" onclick={() => open(blank())}>Add a guest</button>
    <button
      type="button"
      disabled={!book?.guests.length}
      title="The whole journal as a table, for your own records"
      onclick={() => void exportTo(null, "")}>Export</button
    >
  </header>

  {#if error}
    <p class="error" role="alert">{error}</p>
  {/if}

  {#if editing && book}
    <form
      class="editor"
      aria-label={editing.id ? `About ${editing.name}` : "A new guest"}
      onsubmit={(event) => {
        event.preventDefault();
        void keep();
      }}
    >
      <div class="pair">
        <label>Name <input bind:value={editing.name} required autocomplete="off" /></label>
        <label class="age"
          >Age <input
            type="number"
            min="1"
            max="120"
            value={editing.age ?? ""}
            oninput={(event) => {
              const age = Number(event.currentTarget.value);
              if (editing) editing.age = event.currentTarget.value === "" || !Number.isFinite(age) ? null : Math.round(age);
            }}
          /></label
        >
      </div>

      <!--
        The three questions, in Rust's words and nobody else's, so what was
        agreed is what is written in `dj_app::guests::ASKS`.
      -->
      <fieldset class="consent">
        <legend>Asked, and agreed to</legend>
        {#each book.asks as ask (ask.name)}
          {@const barred = young && ask.name !== "keep"}
          <label class:barred>
            <input
              type="checkbox"
              name={ask.name}
              disabled={barred}
              checked={editing.consent[ask.name] && !barred}
              onchange={(event) => {
                if (editing) editing.consent[ask.name] = event.currentTarget.checked;
              }}
            />
            {ask.sentence}
          </label>
        {/each}
        {#if young}
          <p class="hint">
            Under {book.consent_age}, contact and voice need a parent's agreement, which a booth cannot check.
          </p>
        {:else if editing.consent.given > 0}
          <p class="hint">Answered {when(editing.consent.given)}.</p>
        {/if}
        {#if !editing.consent.keep}
          <p class="hint">Tonight only: gone when you start a new night.</p>
        {/if}
      </fieldset>

      <fieldset class="contact" disabled={!editing.consent.contact || young}>
        <legend>Contact</legend>
        <label>Email <input type="email" bind:value={editing.email} autocomplete="off" /></label>
        <label>Phone or WhatsApp <input type="tel" bind:value={editing.phone} autocomplete="off" /></label>
        <label
          >Social profiles <input
            value={editing.socials.join(", ")}
            placeholder="separated by commas"
            oninput={(event) => {
              if (editing) editing.socials = list(event.currentTarget.value);
            }}
          /></label
        >
      </fieldset>

      <fieldset class="about">
        <legend>About them</legend>
        <label>Home town and country <input bind:value={editing.home} /></label>
        <label>Nationality <input bind:value={editing.nationality} /></label>
        <label>Native language <input bind:value={editing.native_language} /></label>
        <label
          >Other languages <input
            value={editing.languages.join(", ")}
            placeholder="separated by commas"
            oninput={(event) => {
              if (editing) editing.languages = list(event.currentTarget.value);
            }}
          /></label
        >
        <label>Favourite band <input bind:value={editing.favourite_band} /></label>
        <label>Favourite genre <input bind:value={editing.favourite_genre} /></label>
        <label>Favourite song <input bind:value={editing.favourite_song} /></label>
        <label class="wide">Notes <textarea rows="2" bind:value={editing.notes}></textarea></label>
      </fieldset>

      {#if editing.sang.length > 0}
        <ol class="sang" aria-label="Songs {editing.name} sang">
          {#each editing.sang as song, index (index)}
            <li>
              <span class="title">{song.title}</span>
              <span class="when">{when(song.at)}{song.event ? ` · ${song.event}` : ""}</span>
            </li>
          {/each}
        </ol>
      {/if}

      <div class="actions">
        <button type="submit" class="primary">Keep</button>
        <button type="button" onclick={() => (editing = null)}>Cancel</button>
        {#if editing.id}
          <button
            type="button"
            title="Their record, whole, as a file they can be given"
            onclick={() => editing && void exportTo(editing.id, editing.name)}>Their copy</button
          >
          {#if forgetting}
            <button type="button" class="danger" onclick={() => void forget()}>Forget {editing.name} and their recordings</button>
          {:else}
            <button type="button" onclick={() => (forgetting = true)}>Forget…</button>
          {/if}
        {/if}
      </div>
    </form>
  {:else if ordered.length === 0}
    <p class="empty">Nobody yet. A singer marked as having sung is written down here.</p>
  {:else}
    <ul class="list" aria-label="Guests">
      {#each ordered as guest (guest.id)}
        {@const last = guest.sang[guest.sang.length - 1]}
        <li>
          <button type="button" class="guest" onclick={() => open(guest)}>
            <span class="name">{guest.name}</span>
            {#if last}<span class="song">{last.title}</span>{/if}
            <span class="marks">
              {#if !guest.consent.keep}<span class="mark" title="Tonight only">tonight</span>{/if}
              {#if guest.consent.contact}<span class="mark" title="Agreed to be contacted">contact</span>{/if}
              {#if guest.consent.voice}<span class="mark" title="Agreed to their voice being used">voice</span>{/if}
            </span>
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .guests {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    border-top: 1px solid var(--border);
    padding-top: 0.6rem;
  }

  header {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
  }

  h3 {
    margin: 0;
    font-size: 1em;
  }

  .count {
    color: var(--text-dim);
    margin-right: auto;
  }

  .error {
    color: var(--danger);
    margin: 0;
  }

  .empty,
  .hint {
    color: var(--text-dim);
    font-size: 0.85em;
    margin: 0.2rem 0 0;
  }

  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .guest {
    display: flex;
    align-items: baseline;
    gap: 0.6rem;
    width: 100%;
    text-align: left;
    background: var(--panel-raised);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 0.35rem 0.5rem;
    color: var(--text);
  }

  .guest .song {
    color: var(--text-dim);
    font-size: 0.85em;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .marks {
    margin-left: auto;
    display: flex;
    gap: 0.25rem;
  }

  .mark {
    font-size: 0.7em;
    border: 1px solid var(--border-strong);
    border-radius: 999px;
    padding: 0 0.4rem;
    color: var(--text-dim);
  }

  .editor {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .pair {
    display: flex;
    gap: 0.5rem;
  }

  .pair label:first-child {
    flex: 1;
  }

  .age input {
    width: 4.5rem;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    font-size: 0.85em;
    color: var(--text-dim);
  }

  input,
  textarea {
    background: var(--panel-raised);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 0.3rem 0.4rem;
    font: inherit;
    font-size: 1.05em;
  }

  fieldset {
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0.5rem 0.6rem;
    margin: 0;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(12rem, 1fr));
    gap: 0.4rem 0.6rem;
  }

  fieldset:disabled {
    opacity: 0.55;
  }

  legend {
    font-size: 0.8em;
    color: var(--text-dim);
    padding: 0 0.3rem;
  }

  .consent {
    grid-template-columns: 1fr;
  }

  .consent label {
    flex-direction: row;
    align-items: flex-start;
    gap: 0.45rem;
    color: var(--text);
  }

  .consent label.barred {
    color: var(--text-dim);
  }

  .wide {
    grid-column: 1 / -1;
  }

  .sang {
    margin: 0;
    padding-left: 1.2rem;
    font-size: 0.85em;
  }

  .sang .when {
    color: var(--text-dim);
    margin-left: 0.4rem;
  }

  .actions {
    display: flex;
    gap: 0.4rem;
    flex-wrap: wrap;
  }

  .danger {
    border-color: var(--danger);
    color: var(--danger);
  }
</style>
