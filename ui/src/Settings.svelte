<script lang="ts">
  /**
   * Sources, credentials, folders and branding.
   *
   * The source cards render `dj-sources`' own catalog text rather than
   * restating it here. That is deliberate: the honest paragraph about what a
   * service will and will not do is the same string the code obeys, so the two
   * cannot drift apart and leave the interface promising something the engine
   * refuses.
   */
  import Screens from "./Screens.svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import {
    addMusicFolder,
    clearBrandLogo,
    clearSecret,
    hasBrandLogo,
    listSources,
    logoUrl,
    musicLibrary,
    removeMusicFolder,
    secretsPersist,
    setBrandLogo,
    setSecret,
    type Library,
    type Source,
  } from "./api";
  import { theme, type ThemePreference } from "./theme.svelte";
  import { performance, type PerformanceLevel } from "./performance.svelte";
  import SvgPad from "./controls/SvgPad.svelte";
  import { themePackages } from "./controls/themes/packages";
  import IconButton from "./controls/IconButton.svelte";
  import SvgKnob from "./controls/SvgKnob.svelte";

  let { onLogoChange }: { onLogoChange: () => void } = $props();


  let sources = $state<Source[]>([]);
  let library = $state<Library>({ folders: [], tracks: 0 });
  let persist = $state(true);
  let logo = $state(false);
  let logoVersion = $state(0);
  let error = $state<string | null>(null);
  let busy = $state(false);
  /** Field values being typed, keyed by credential id. */
  let drafts = $state<Record<string, string>>({});
  let expanded = $state<Record<string, boolean>>({});

  $effect(() => {
    void refresh();
  });

  async function refresh() {
    try {
      [sources, library, persist, logo] = await Promise.all([
        listSources(),
        musicLibrary(),
        secretsPersist(),
        hasBrandLogo(),
      ]);
      error = null;
    } catch (e) {
      error = String(e);
    }
  }

  async function save(id: string) {
    const value = (drafts[id] ?? "").trim();
    if (!value) return;
    try {
      await setSecret(id, value);
      drafts[id] = "";
      await refresh();
    } catch (e) {
      error = String(e);
    }
  }

  async function forget(id: string) {
    try {
      await clearSecret(id);
      await refresh();
    } catch (e) {
      error = String(e);
    }
  }

  async function pickFolder() {
    const path = await open({ directory: true, multiple: false });
    if (typeof path !== "string") return;
    busy = true;
    try {
      await addMusicFolder(path);
      await refresh();
      error = null;
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function dropFolder(path: string) {
    await removeMusicFolder(path);
    await refresh();
  }

  async function pickLogo() {
    const path = await open({
      multiple: false,
      filters: [{ name: "Image", extensions: ["png", "jpg", "jpeg", "gif", "webp", "svg"] }],
    });
    if (typeof path !== "string") return;
    try {
      await setBrandLogo(path);
      logoVersion += 1;
      await refresh();
      onLogoChange();
      error = null;
    } catch (e) {
      error = String(e);
    }
  }

  async function dropLogo() {
    await clearBrandLogo();
    logoVersion += 1;
    await refresh();
    onLogoChange();
  }

  const audioLabel = (source: Source) =>
    source.audio === "direct"
      ? "Mixable"
      : source.audio === "user_supplied"
        ? "Your own files"
        : "No audio";
</script>

<section class="settings">
  {#if error}
    <p class="error">{error}</p>
  {/if}

  <!--
    Screens first, because it is the one setting that changes the shape of the
    whole application rather than a detail inside it.
  -->
  <div class="block">
    <Screens />
  </div>

  <!--
    Appearance and branding next: these are the two settings that change what
    the DJ looks at all night.
  -->
  <div class="block">
    <h3>Appearance</h3>
    <p class="hint">
      Dark by default, because a white screen at eye level in a dark room costs
      you the night vision you need to find anything on the actual mixer. The
      waveform is recoloured too — it is drawn outside the browser, so it does
      not follow a stylesheet on its own.
    </p>
    <div class="theme-preview" style="display:flex; gap:0.6rem; align-items:center; margin-bottom:0.6rem;">
      <div style="width:120px; height:70px; display:flex; align-items:center; justify-content:center; background:var(--panel); border:1px solid var(--border); border-radius:8px; padding:0.5rem;">
        <SvgPad width={100} height={50} active={true} />
      </div>
      <div style="display:flex; flex-direction:column; gap:0.3rem;">
        <SvgKnob width={48} height={48} value={0.6} />
        <div style="width:60px; height:14px; background:var(--accent); border-radius:4px;"></div>
      </div>
    </div>
    <div class="row theme-choice" style="gap: 1rem;">
      {#each [{ id: "dark", label: "Dark" }, { id: "light", label: "Light" }, { id: "system", label: "Follow system" }] as option (option.id)}
        <div style="display: flex; flex-direction: column; align-items: center; gap: 0.5rem; width: 80px;">
          <SvgPad
            width={80}
            height={50}
            active={theme.preference === option.id}
            onclick={() => theme.set(option.id as ThemePreference)}
          />
          <span style="font-size: 0.85em; color: var(--text-dim); text-align: center;">{option.label}</span>
        </div>
      {/each}
    </div>
    {#if theme.preference === "system"}
      <p class="hint">
        Currently {theme.resolved}. Follows the operating system, including if
        it changes mid-set.
      </p>
    {/if}
  </div>

  <div class="block">
    <h3>Visual Package</h3>
    <p class="hint">
      The aesthetic geometry and behavior of the SVG controls. Curated into specific vibes.
    </p>
    <div class="row theme-choice" style="gap: 1rem;">
      {#each themePackages as pkg (pkg.id)}
        <div style="display: flex; flex-direction: column; align-items: center; gap: 0.5rem; width: 100px;">
          <SvgPad
            width={100}
            height={50}
            active={theme.activePackage.id === pkg.id}
            onclick={() => theme.setPackage(pkg.id)}
          />
          <span style="font-size: 0.85em; color: var(--text-dim); text-align: center;">{pkg.name}</span>
        </div>
      {/each}
    </div>
  </div>

  <div class="block">
    <h3>Performance</h3>
    <p class="hint">
      Complex SVG themes can be expensive to draw. If the app detects frame drops, 'Auto' mode will step down visual complexity (Eco, Balanced) to ensure your live set runs flawlessly without stuttering.
    </p>
    <div class="row theme-choice" style="gap: 1rem;">
      {#each [{ id: "Auto", label: "Auto (Detect)" }, { id: "Ultra", label: "Ultra (Full FX)" }, { id: "Balanced", label: "Balanced" }, { id: "Eco", label: "Eco (Static)" }] as option (option.id)}
        <div style="display: flex; flex-direction: column; align-items: center; gap: 0.5rem; width: 90px;">
          <SvgPad
            width={90}
            height={50}
            active={performance.preference === option.id}
            onclick={() => performance.set(option.id as PerformanceLevel)}
          />
          <span style="font-size: 0.85em; color: var(--text-dim); text-align: center;">{option.label}</span>
        </div>
      {/each}
    </div>
    {#if performance.preference === "Auto"}
      <p class="hint">
        Currently rendering at {performance.resolved} mode.
      </p>
    {/if}
  </div>

  <div class="block">
    <h3>Your logo</h3>
    <p class="hint">
      Replaces the djmanzo wordmark in the title bar. The image is copied into
      the app, so it keeps working after the original is moved or the stick is
      pulled.
    </p>
      <div class="row">
      {#if logo}
        <img class="logo-preview" src={logoUrl(logoVersion)} alt="Your logo" />
      {:else}
        <span class="wordmark">djmanzo</span>
      {/if}
      <IconButton icon="fa-solid fa-image" title={logo ? 'Replace logo' : 'Choose image'} onClick={pickLogo} />
      {#if logo}
        <IconButton icon="fa-solid fa-trash" title="Remove logo" onClick={dropLogo} />
      {/if}
    </div>
  </div>

  <div class="block">
    <h3>Music folders <em class="mono">{library.tracks} tracks</em></h3>
    <p class="hint">
      Scanned on the spot. This is the only source that needs nobody's
      permission and keeps working when a venue's wifi does not.
    </p>
    {#each library.folders as folder (folder)}
      <div class="row folder">
        <span class="path mono" title={folder}>{folder}</span>
        <button onclick={() => dropFolder(folder)}>Remove</button>
      </div>
    {/each}
    <button class="primary" onclick={pickFolder} disabled={busy}>
      {busy ? "Scanning…" : "Add folder"}
    </button>
  </div>

  <div class="block">
    <h3>Sources</h3>
    {#if !persist}
      <p class="warning">
        No keychain is available on this machine, so keys are held in memory and
        will be gone after a restart. Better to know now than to find out
        mid-set.
      </p>
    {/if}

    {#each sources as source (source.id)}
      <article class="source" class:gated={source.partner_gated}>
        <header>
          <span class="name">{source.label}</span>
          <span class="badge {source.audio}">{audioLabel(source)}</span>
          <span class="status {source.status}">{source.status_detail}</span>
          <button
            class="more"
            onclick={() => (expanded[source.id] = !expanded[source.id])}
            aria-expanded={expanded[source.id] ?? false}
          >
            {expanded[source.id] ? "Less" : "More"}
          </button>
        </header>

        <p class="summary">{source.summary}</p>

        {#if expanded[source.id]}
          <!-- The catalog's own words. Whatever this says is what the engine does. -->
          <p class="detail">{source.detail}</p>
        {/if}

        {#if source.audio === "none" && source.audio_note}
          <p class="audio-note">{source.audio_note}</p>
        {/if}

        {#each source.credentials as credential (credential.id)}
          <div class="credential">
            <label for="cred-{credential.id}">
              {credential.label}
              {#if credential.is_set}
                <em class="mono set">{credential.hint}</em>
              {/if}
            </label>
            <div class="row">
              <input
                id="cred-{credential.id}"
                type="password"
                autocomplete="off"
                spellcheck="false"
                placeholder={credential.is_set ? "Replace…" : "Paste your key"}
                bind:value={drafts[credential.id]}
                onkeydown={(e) => e.key === "Enter" && save(credential.id)}
              />
              <button onclick={() => save(credential.id)}>Save</button>
              {#if credential.is_set}
                <button onclick={() => forget(credential.id)}>Forget</button>
              {/if}
            </div>
            <p class="free-tier">
              {credential.free_tier}
              <a href={credential.signup_url} target="_blank" rel="noreferrer">
                Get one →
              </a>
            </p>
          </div>
        {/each}
      </article>
    {/each}
  </div>
</section>

<style>
  .settings {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    overflow: auto;
    flex: 1;
    min-height: 0;
    padding-right: 0.3rem;
  }

  .block {
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 0.9rem;
  }

  h3 {
    margin: 0 0 0.3rem;
    font-size: 0.95rem;
    display: flex;
    gap: 0.5rem;
    align-items: baseline;
  }

  h3 em {
    font-style: normal;
    font-size: 0.8rem;
    color: var(--text-dim);
  }

  .hint {
    margin: 0 0 0.7rem;
    color: var(--text-dim);
    font-size: 0.8em;
    line-height: 1.5;
  }

  .row {
    display: flex;
    gap: 0.4rem;
    align-items: center;
    margin-bottom: 0.4rem;
  }

  .theme-choice {
    flex-wrap: wrap;
  }

  .row input {
    flex: 1;
    min-width: 0;
  }

  .folder .path {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 0.8em;
    color: var(--text-dim);
  }

  .logo-preview {
    max-height: 40px;
    max-width: 180px;
    object-fit: contain;
  }

  .wordmark {
    font-weight: 700;
    color: var(--accent);
    letter-spacing: 0.02em;
  }

  .source {
    border-top: 1px solid var(--border);
    padding: 0.7rem 0 0.2rem;
  }

  .source.gated {
    opacity: 0.85;
  }

  .source header {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
  }

  .name {
    font-weight: 600;
  }

  .badge {
    font-size: 0.65em;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    padding: 0.1rem 0.4rem;
    border-radius: 3px;
    border: 1px solid var(--border);
    color: var(--text-dim);
  }

  /* Mixable is the distinction that matters most, so it is the one with colour. */
  .badge.direct {
    color: var(--accent-2);
    border-color: color-mix(in srgb, var(--accent-2) 50%, var(--border));
  }

  .badge.none {
    color: var(--danger);
    border-color: color-mix(in srgb, var(--danger) 40%, var(--border));
  }

  .status {
    font-size: 0.75em;
    color: var(--text-dim);
    flex: 1;
    min-width: 0;
  }

  .status.ready {
    color: var(--accent-2);
  }

  .status.needs_credentials {
    color: var(--warn);
  }

  .more {
    padding: 0.15rem 0.45rem;
    font-size: 0.7em;
  }

  .summary,
  .detail,
  .audio-note,
  .free-tier {
    margin: 0.35rem 0 0;
    font-size: 0.8em;
    line-height: 1.55;
    color: var(--text-dim);
    user-select: text;
    -webkit-user-select: text;
  }

  .detail {
    white-space: pre-line;
    color: var(--text);
    background: var(--panel-raised);
    border-radius: 6px;
    padding: 0.6rem 0.7rem;
  }

  .audio-note {
    color: var(--danger);
  }

  .credential {
    margin-top: 0.6rem;
  }

  .credential label {
    display: block;
    font-size: 0.8em;
    color: var(--text-dim);
    margin-bottom: 0.2rem;
  }

  .credential .set {
    font-style: normal;
    color: var(--accent-2);
    margin-left: 0.3rem;
  }

  .free-tier a {
    color: var(--accent);
    white-space: nowrap;
  }

  .error,
  .warning {
    margin: 0 0 0.6rem;
    padding: 0.6rem 0.9rem;
    border-radius: 8px;
    font-size: 0.85em;
    line-height: 1.5;
  }

  .error {
    background: color-mix(in srgb, var(--danger) 12%, var(--panel));
    border: 1px solid var(--danger);
    color: var(--danger);
  }

  .warning {
    background: color-mix(in srgb, var(--warn) 12%, var(--panel));
    border: 1px solid var(--warn);
    color: var(--warn);
  }
</style>
