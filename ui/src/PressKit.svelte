<script lang="ts">
  /**
   * §118d: the DJ's own press kit.
   *
   * > the DJ can accumulate his personal stuff in a orchestrated way and on
   * > the occasion has easy and direct access to many share mechanisms that
   * > are prepackaged/optimised for typical practical occasions
   *
   * The occasions first, because they are what the panel is opened for in a
   * hurry: the card for whoever asks at the booth (and a QR code of the
   * contact to scan), the answer to a booking enquiry for a kind of night,
   * where the DJ plays next, and the whole kit as one page. Each is Rust's
   * words from the kit (`kit::compose`), says what the kit still lacks for
   * it, and leaves by a way the DJ presses -- a composer opens with the words
   * in it, and the DJ sends. Below them, the kit itself, kept a moment after
   * typing stops, begun from what the welcome was told.
   */
  import { open } from "@tauri-apps/plugin-dialog";
  import {
    eventOptions,
    kitAdd,
    kitCompose,
    kitForget,
    kitSave,
    kitSend,
    kitUrl,
    kitView,
    type GigOptions,
    type Kit,
    type KitComposed,
    type KitView,
    type KitWay,
  } from "./api";

  let view = $state<KitView | null>(null);
  let draft = $state<Kit | null>(null);
  let options = $state<GigOptions | null>(null);
  let error = $state<string | null>(null);
  /** The kind of night an enquiry is for. */
  let night = $state("");
  let enquiry = $state<KitComposed | null>(null);
  let copied = $state<string | null>(null);
  let savedAt = $state<string | null>(null);
  /** Bumped when a photo changes, so the webview does not show the old one. */
  let version = $state(0);
  /**
   * What is on show: the occasions, or the kit's fields. Two, because in
   * one the occasions above changed height as the fields below were typed
   * into -- a line saying what was missing went away, a text grew -- and
   * the next field clicked had moved from under the pointer.
   */
  let tab = $state<"send" | "kit">("send");

  let edits = 0;
  let pending: ReturnType<typeof setTimeout> | null = null;
  let saving: Promise<void> | null = null;

  /** What Rust refused, with the kit it refused: said only while that stands. */
  let refused = $state<{ message: string; of: string } | null>(null);
  const refusal = $derived(
    refused && draft && refused.of === JSON.stringify($state.snapshot(draft)) ? refused.message : null,
  );

  $effect(() => {
    void (async () => {
      try {
        const [found, opts] = await Promise.all([kitView(), eventOptions()]);
        view = found;
        draft = structuredClone(found.kit);
        options = opts;
      } catch (e) {
        error = String(e);
      }
    })();
  });

  // The enquiry for the night chosen, asked again when the night or the
  // kit changes.
  $effect(() => {
    const chosen = night;
    void view;
    if (!chosen) {
      enquiry = null;
      return;
    }
    void (async () => {
      try {
        enquiry = await kitCompose("enquiry", chosen);
      } catch (e) {
        error = String(e);
      }
    })();
  });

  const occasions = $derived(
    (view?.occasions ?? []).map((o) => (o.occasion === "enquiry" && enquiry ? enquiry : o)),
  );

  /** Held while an address or a link is half typed, rather than refused. */
  const held = $derived.by(() => {
    if (!draft) return null;
    if (draft.email && !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(draft.email)) {
      return "An e-mail address reads name@example.com.";
    }
    if (draft.links.some((l) => l.trim() && !/^https?:\/\/\S+\.\S+/.test(l.trim()))) {
      return "A link starts with https:// — the address as the browser shows it.";
    }
    return null;
  });

  function edited() {
    edits += 1;
    if (pending) clearTimeout(pending);
    pending = setTimeout(() => {
      pending = null;
      if (held) return;
      saving = send();
    }, 400);
  }

  async function send() {
    if (!draft) return;
    const sent = edits;
    const kit = $state.snapshot(draft) as Kit;
    try {
      const answer = await kitSave(kit);
      view = answer;
      refused = null;
      error = null;
      if (edits === sent) draft = structuredClone(answer.kit);
    } catch (e) {
      refused = { message: String(e), of: JSON.stringify(kit) };
    }
  }

  async function flush() {
    if (pending) {
      clearTimeout(pending);
      pending = null;
      saving = send();
    }
    if (saving) await saving;
  }

  async function hand(composed: KitComposed, way: KitWay) {
    await flush();
    error = null;
    if (way === "copy") {
      try {
        await navigator.clipboard.writeText(composed.text);
        copied = composed.occasion;
        setTimeout(() => {
          if (copied === composed.occasion) copied = null;
        }, 2500);
      } catch {
        // No clipboard: the words are selectable, which is the fallback.
        error = "Copy is not available here; select the words and copy them.";
      }
      return;
    }
    try {
      const path = await kitSend(composed.occasion, composed.occasion === "enquiry" ? night || null : null, way);
      if (path) savedAt = path;
    } catch (e) {
      error = String(e);
    }
  }

  async function add(kind: "photo" | "document") {
    await flush();
    try {
      const path = await open({
        multiple: false,
        filters:
          kind === "photo"
            ? [{ name: "Photos", extensions: ["png", "jpg", "jpeg", "gif", "webp"] }]
            : [{ name: "Documents", extensions: ["pdf", "txt", "md", "doc", "docx", "odt", "png", "jpg", "jpeg"] }],
      });
      if (typeof path !== "string") return;
      const answer = await kitAdd(kind, path);
      view = answer;
      draft = structuredClone(answer.kit);
      version += 1;
      error = null;
    } catch (e) {
      error = String(e);
    }
  }

  async function forget(kind: "photo" | "document", file: string) {
    await flush();
    try {
      const answer = await kitForget(kind, file);
      view = answer;
      draft = structuredClone(answer.kit);
      error = null;
    } catch (e) {
      error = String(e);
    }
  }

  function toggleGenre(genre: string) {
    if (!draft) return;
    const at = draft.genres.indexOf(genre);
    if (at >= 0) draft.genres.splice(at, 1);
    else draft.genres.push(genre);
    edited();
  }

  function addFee() {
    if (!draft) return;
    draft.fees.push({ night: null, what: "", price: "" });
  }
</script>

<div class="kit">
  {#if view && draft}
    <div class="tabs" role="tablist" aria-label="Press kit">
      <button type="button" role="tab" aria-selected={tab === "send"} onclick={() => (tab = "send")}>Send</button>
      <button type="button" role="tab" aria-selected={tab === "kit"} onclick={() => (tab = "kit")}>Your kit</button>
    </div>

    {#if !view.kept}
      <p class="begun">Begun from what the welcome was told. Everything here is kept as you type.</p>
    {/if}

    {#if tab === "send"}
    <div class="occasions" aria-label="Send" role="tabpanel">
      {#each occasions as composed (composed.occasion)}
        <article class="occasion" data-occasion={composed.occasion}>
          <header>
            <strong>{composed.title}</strong>
            <span class="when">{composed.when}</span>
          </header>
          {#if composed.occasion === "enquiry"}
            <label class="night">For a
              <select bind:value={night} aria-label="The kind of night asked about">
                <option value="">night</option>
                {#each options?.nights ?? [] as n (n.slug)}
                  <option value={n.slug}>{n.title.toLowerCase()} night</option>
                {/each}
              </select>
            </label>
          {/if}
          {#if composed.missing.length > 0}
            <p class="missing">
              Still to add: {composed.missing.join(", ")}.
              <button type="button" class="to-kit" onclick={() => (tab = "kit")}>Add it</button>
            </p>
          {/if}
          {#if composed.occasion === "card" && view.qr}
            <div class="qr" role="img" aria-label="Your contact as a QR code: a phone that scans it can save it">
              <!-- Rust's own SVG, drawn from the kit's contact. -->
              {@html view.qr}
            </div>
          {/if}
          {#if composed.text}
            <pre class="text">{composed.text}</pre>
          {/if}
          <div class="ways">
            {#each composed.ways as offered (offered.way)}
              <button
                type="button"
                data-way={offered.way}
                disabled={!offered.fits || (composed.text === "" && offered.way !== "save")}
                title={offered.fits ? undefined : "Too long for this way; copy it instead"}
                onclick={() => void hand(composed, offered.way)}
              >
                {copied === composed.occasion && offered.way === "copy" ? "Copied" : offered.name}
              </button>
            {/each}
          </div>
          {#if composed.occasion === "page" && savedAt}
            <p class="saved" role="status">Saved to {savedAt}</p>
          {/if}
        </article>
      {/each}
    </div>
    {/if}

    {#if error ?? refusal ?? held}
      <p class="error" role="alert">{error ?? refusal ?? held}</p>
    {/if}

    {#if tab === "kit"}
    <div class="fields" aria-label="Your kit" role="tabpanel">
      <h3>Who you are</h3>
      <label>Name <input type="text" bind:value={draft.name} oninput={edited} placeholder="The name you play as" /></label>
      <label>In one line <input type="text" bind:value={draft.tagline} oninput={edited} placeholder="What you are, in a sentence" /></label>
      <label>Based in <input type="text" bind:value={draft.based} oninput={edited} placeholder="Where you are, and play" /></label>
      <label>Bio <textarea rows="4" bind:value={draft.bio} oninput={edited} placeholder="A paragraph or two"></textarea></label>

      <h3>Your music</h3>
      <div class="genres" role="group" aria-label="Genres">
        {#each options?.genres ?? [] as genre (genre)}
          <button type="button" class="choice" class:on={draft.genres.includes(genre)} aria-pressed={draft.genres.includes(genre)} onclick={() => toggleGenre(genre)}>{genre}</button>
        {/each}
      </div>
      <label>Experience, one a line
        <textarea
          rows="3"
          value={draft.experience.join("\n")}
          oninput={(e) => {
            if (draft) draft.experience = e.currentTarget.value.split("\n");
            edited();
          }}
          placeholder="Resident at … since …"
        ></textarea>
      </label>

      <h3>Bookings</h3>
      <label>E-mail <input type="text" inputmode="email" bind:value={draft.email} oninput={edited} placeholder="name@example.com" /></label>
      <label>Phone <input type="text" inputmode="tel" bind:value={draft.phone} oninput={edited} placeholder="+43 …" /></label>
      <label>How to book <textarea rows="2" bind:value={draft.booking} oninput={edited} placeholder="A deposit, an agent, what the rider asks for"></textarea></label>
      <div class="fees" role="group" aria-label="Fees">
        {#each draft.fees as fee, i (i)}
          <div class="fee">
            <select bind:value={fee.night} onchange={edited} aria-label="Which kind of night">
              <option value={null}>Any night</option>
              {#each options?.nights ?? [] as n (n.slug)}
                <option value={n.slug}>{n.title}</option>
              {/each}
            </select>
            <input type="text" bind:value={fee.what} oninput={edited} placeholder="What is booked" aria-label="What is booked" />
            <input type="text" bind:value={fee.price} oninput={edited} placeholder="Price" aria-label="Price" class="price" />
            <button
              type="button"
              aria-label="Remove this fee"
              onclick={() => {
                draft?.fees.splice(i, 1);
                edited();
              }}>×</button
            >
          </div>
        {/each}
        <button type="button" onclick={addFee}>+ A fee</button>
      </div>

      <h3>Where to find you</h3>
      <label>Links, one a line
        <textarea
          rows="3"
          value={draft.links.join("\n")}
          oninput={(e) => {
            if (draft) draft.links = e.currentTarget.value.split("\n");
            edited();
          }}
          placeholder="https://soundcloud.com/…"
        ></textarea>
      </label>
      {#if view.sites.length > 0}
        <p class="sites">{view.sites.join(" · ")}</p>
      {/if}

      <h3>Photos</h3>
      <div class="photos">
        {#each draft.photos as photo, i (photo.file)}
          <figure class="photo" data-photo={photo.file}>
            <img src={kitUrl(photo.file, version)} alt={photo.caption || photo.file} />
            <input type="text" bind:value={draft.photos[i].caption} oninput={edited} placeholder="Caption" aria-label="Caption for {photo.file}" />
            <button type="button" onclick={() => void forget("photo", photo.file)}>Remove</button>
          </figure>
        {/each}
      </div>
      <button type="button" onclick={() => void add("photo")}>+ A photo</button>

      <h3>Documents</h3>
      <ul class="documents">
        {#each draft.documents as doc, i (doc.file)}
          <li data-document={doc.file}>
            <span class="file">{doc.file}</span>
            <input type="text" bind:value={draft.documents[i].caption} oninput={edited} placeholder="What it is: a rider, a tech sheet" aria-label="What {doc.file} is" />
            <button type="button" onclick={() => void forget("document", doc.file)}>Remove</button>
          </li>
        {/each}
      </ul>
      <button type="button" onclick={() => void add("document")}>+ A document</button>

      {#if view.booked.length > 0}
        <h3>Already booked</h3>
        <ul class="booked">
          {#each view.booked as b (b.date + b.title)}
            <li>{b.date} — {b.title}{b.place ? `, ${b.place}` : ""}</li>
          {/each}
        </ul>
        <p class="hint">From your events: a night with a date is a date taken.</p>
      {/if}
    </div>
    {/if}
  {:else if error}
    <p class="error">{error}</p>
  {/if}
</div>

<style>
  .kit {
    display: flex;
    flex-direction: column;
    gap: 0.7rem;
    padding: 0.5rem;
    overflow: auto;
    min-height: 0;
    height: 100%;
  }

  .tabs {
    display: flex;
    gap: 0.3rem;
  }

  .tabs [role="tab"] {
    flex: 1 1 0;
  }

  .tabs [role="tab"][aria-selected="true"] {
    background: var(--active);
    color: var(--on-active, #000);
  }

  .to-kit {
    margin-left: 0.3rem;
    padding: 0 0.4rem;
    font-size: 0.95em;
  }

  .begun,
  .hint,
  .when,
  .sites {
    color: var(--text-dim);
    font-size: 0.85em;
    margin: 0;
  }

  .occasions {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(15rem, 1fr));
    gap: 0.5rem;
  }

  .occasion {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    padding: 0.5rem 0.6rem;
    border: 1px solid var(--border);
    border-radius: 0.5rem;
    min-width: 0;
  }

  .occasion header {
    display: flex;
    flex-direction: column;
  }

  .missing {
    margin: 0;
    color: var(--warn);
    font-size: 0.85em;
  }

  .text {
    margin: 0;
    max-height: 9rem;
    overflow: auto;
    white-space: pre-wrap;
    font-family: inherit;
    font-size: 0.85em;
    padding: 0.35rem 0.45rem;
    border-radius: 0.35rem;
    background: var(--sunken, var(--surface));
    user-select: text;
  }

  /* Paper white on every theme, as the sticker's QR is: a reader in a dark
     room looks through a camera for dark squares on white, and a QR that
     wore the palette would not scan on the dark ones. */
  .qr {
    align-self: center;
    width: 9rem;
    height: 9rem;
    background: rgb(255, 255, 255);
    padding: 0.3rem;
    border-radius: 0.3rem;
  }

  .qr :global(svg) {
    width: 100%;
    height: 100%;
    display: block;
  }

  .ways {
    display: flex;
    flex-wrap: wrap;
    gap: 0.3rem;
  }

  .saved {
    margin: 0;
    font-size: 0.8em;
    color: var(--text-dim);
    overflow-wrap: anywhere;
  }

  .error {
    margin: 0;
    color: var(--warn);
  }

  .fields {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .fields h3 {
    margin: 0.6rem 0 0;
    font-size: 0.95em;
  }

  .fields label {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    font-size: 0.85em;
    color: var(--text-dim);
  }

  .fields input,
  .fields textarea,
  .fields select {
    color: var(--text);
  }

  .genres {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem;
  }

  .choice {
    font-size: 0.8em;
    padding: 0.1rem 0.45rem;
    border-radius: 999px;
  }

  .choice.on {
    background: var(--active);
    color: var(--on-active, #000);
  }

  .fees {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    align-items: flex-start;
  }

  .fee {
    display: flex;
    gap: 0.3rem;
    width: 100%;
  }

  .fee input {
    flex: 1 1 8rem;
    min-width: 0;
  }

  .fee .price {
    flex: 0 1 6rem;
  }

  .photos {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(8rem, 1fr));
    gap: 0.45rem;
  }

  .photo {
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
  }

  .photo img {
    width: 100%;
    aspect-ratio: 4 / 3;
    object-fit: cover;
    border-radius: 0.3rem;
    background: var(--sunken, var(--surface));
  }

  .documents,
  .booked {
    margin: 0;
    padding-left: 1rem;
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .documents li {
    display: flex;
    gap: 0.3rem;
    align-items: center;
  }

  .documents .file {
    flex: none;
    max-width: 10rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .documents input {
    flex: 1 1 auto;
    min-width: 0;
  }
</style>
