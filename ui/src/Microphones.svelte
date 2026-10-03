<script lang="ts">
  /**
   * K3: the singers' microphones — a row for every input the interface has.
   *
   * What a host acts on in a hurry (docs/KARAOKE.md §6): which microphone,
   * open or closed, its fader, its level with the gate on it, how hard its
   * compressor is working, and what it is for — one row each. Above them, how
   * many are working and what that costs the audio thread, and how late a
   * singer hears themselves. The chain behind each strip is Rust's
   * (`dj_vocal`); this draws the snapshot's readings and sends the host's
   * choices, which Rust keeps for the next time djmanzo starts. A press on a
   * row's name opens the chain behind it (`StripChain`), and who is singing
   * into it: a singer from the rotation put on a microphone sings through
   * the chain kept for them, laid over the strip's row by Rust, and *Keep for
   * this singer* keeps what the host has changed for them. Below them,
   * on an output with eight channels, the music's level in the singers'
   * monitor — the wedge's own, apart from the room's.
   */
  import {
    dispatch,
    listInputs,
    MONITOR_MUSIC_OFF_DB,
    vocalStripKeep,
    vocalStripPreset,
    vocalStripSet,
    vocalStripSinger,
    vocalsClose,
    vocalsOpen,
    vocalsState,
    type Device,
    type MicDevice,
    type OnMic,
    type StripSettings,
    type VocalPreset,
    type Vocals,
    type VocalsState,
  } from "./api";
  import StripChain from "./StripChain.svelte";

  let {
    live,
    enabled = true,
    vocals = $bindable(null),
    singers = [],
  }: {
    live?: VocalsState;
    enabled?: boolean;
    /** Rust's answer, shared with the rotation above, which puts singers on microphones too. */
    vocals?: Vocals | null;
    /** Tonight's rotation, by name: who can be put on a microphone. */
    singers?: string[];
  } = $props();

  let inputs = $state<Device[]>([]);
  let device = $state("");
  let opened = $state<MicDevice | null>(null);
  let busy = $state(false);
  let error = $state("");
  /** Which rows have their chain open. */
  let chains = $state<boolean[]>([]);
  /** The fader's range, Rust's. */
  const fader = $derived(vocals?.limits.gain_db ?? [-60, 12]);

  async function refresh() {
    try {
      vocals = await vocalsState();
    } catch (e) {
      error = String(e);
    }
  }

  $effect(() => {
    void refresh();
    void listInputs()
      .then((found) => {
        inputs = found;
        device ||= found.find((d) => d.is_default)?.id ?? found[0]?.id ?? "";
      })
      .catch(() => {});
  });

  /** How many strips the engine holds; the settings are asked for again when it changes. */
  const count = $derived(live?.inputs ?? 0);
  let known = -1;
  $effect(() => {
    if (count === known) return;
    known = count;
    void refresh();
  });

  async function open() {
    busy = true;
    error = "";
    try {
      opened = await vocalsOpen(device || null);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function close() {
    busy = true;
    try {
      await vocalsClose();
      opened = null;
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function set(strip: number, settings: StripSettings) {
    try {
      vocals = await vocalStripSet(strip, settings);
    } catch (e) {
      error = String(e);
    }
  }

  async function putOn(strip: number, singer: string | null) {
    try {
      vocals = await vocalStripSinger(strip, singer);
    } catch (e) {
      error = String(e);
    }
  }

  async function keep(strip: number) {
    error = "";
    try {
      vocals = await vocalStripKeep(strip);
    } catch (e) {
      error = String(e);
    }
  }

  /** Who can be put on a strip: the rotation, and whoever is on it already. */
  const choices = (on: OnMic | null) =>
    on && !singers.some((name) => name.toLowerCase() === on.singer.toLowerCase()) ? [on.singer, ...singers] : singers;

  /** Where the chain a singer is singing through came from, said for the host. */
  function whence(on: OnMic): string {
    if (on.kept === "guest-book") return `${on.singer}'s own, from the guest book`;
    if (on.kept === "tonight") return `${on.singer}'s own, kept for tonight`;
    return `the strip's own — nothing kept for ${on.singer} yet`;
  }

  async function preset(strip: number, chosen: VocalPreset) {
    try {
      vocals = await vocalStripPreset(strip, chosen);
    } catch (e) {
      error = String(e);
    }
  }

  /**
   * The input has gone when the ring has kept running dry for half a second —
   * an interface unplugged, or its driver stopped. Never a stopped record: the
   * strips are fed silence and their tails finish.
   */
  let lastStarved = 0;
  let dryingSince: number | null = null;
  let lost = $state(false);
  /** When each strip last heard anything, to say which open microphone is silent. */
  let heard: number[] = [];
  let quiet = $state<boolean[]>([]);
  const QUIET_MS = 10_000;
  $effect(() => {
    const now = performance.now();
    const starved = live?.starved_frames ?? 0;
    if (count > 0 && starved > lastStarved) {
      dryingSince ??= now;
    } else {
      dryingSince = null;
    }
    lastStarved = starved;
    lost = dryingSince !== null && now - dryingSince > 500;
    const faces = live?.strips ?? [];
    const settings = vocals?.strips ?? [];
    quiet = faces.map((face, i) => {
      const open = settings[i]?.open ?? false;
      // A closed strip's silence is not news: the clock starts when it opens.
      if (!open || face.level > 0.01 || heard[i] === undefined) heard[i] = now;
      return open && now - heard[i] > QUIET_MS;
    });
  });

  /** The audio thread's share, as a percentage, for `strips` working at once. */
  const share = (strips: number) => Math.round(strips * (vocals?.chain_cost ?? 0) * 1000) / 10;
  /** Too much for this machine if every strip worked at once. */
  const heavy = $derived(share(count) > 60);
  /** Input buffer and output buffer: how late a singer hears themselves in a monitor. */
  const late = $derived(opened ? Math.round(opened.latencyMs * 2) : null);

  /** The music in the singers' monitor; `null` when the output has no pair for one. */
  const monitor = $derived(live?.monitor_music_db ?? null);
  /** The level said as the host thinks of it: off at the bottom, dB above. */
  const level = (db: number) =>
    db <= MONITOR_MUSIC_OFF_DB ? "off" : `${db > 0 ? "+" : db < 0 ? "−" : ""}${Math.abs(Math.round(db))} dB`;

  function monitorMusic(db: number) {
    dispatch(`monitor music ${db}`).catch((e) => (error = String(e)));
  }
</script>

<section class="mics" aria-labelledby="singers-mics">
  <h3 id="singers-mics">Microphones</h3>
  {#if count === 0}
    <p class="hint">
      A strip and a vocal chain for every input of your interface — gate, EQ, compressor, de-esser, echo and a
      little room, each singer's their own.
    </p>
    <div class="open">
      <select aria-label="Interface for the singers' microphones" bind:value={device} disabled={!enabled}>
        {#each inputs as input (input.id)}
          <option value={input.id}>{input.name} — {input.channels} inputs</option>
        {:else}
          <option value="">No input found</option>
        {/each}
      </select>
      <button onclick={() => void open()} disabled={!enabled || busy || inputs.length === 0}>Open the inputs</button>
    </div>
  {:else}
    <p class="rig" role="status" data-heavy={heavy}>
      {live?.working ?? 0} of {count} microphones working — about {share(live?.working ?? 0)} % of the audio thread{#if count > 1}; all {count} at once, {share(count)} %{/if}.
      {#if late !== null}A singer hears themselves about {late} ms late.{/if}
      {#if heavy}<strong>That is more than this machine should be asked for — close some.</strong>{/if}
    </p>
    {#if lost}
      <p class="lost" role="alert">Microphones lost — reconnect the interface; djmanzo tries it again every two seconds.</p>
    {/if}
    <ul class="strips" aria-label="Singers' microphones">
      {#each (vocals?.strips ?? []).slice(0, count) as strip, i (i)}
        {@const face = live?.strips[i]}
        {@const on = vocals?.on[i] ?? null}
        <li
          class="strip"
          data-strip={i + 1}
          data-open={strip.open}
          data-working={face?.working ?? false}
          data-gate={face?.gate_open ?? false}
          data-singer={on?.singer ?? ""}
        >
          <button
            class="name"
            aria-expanded={chains[i] ?? false}
            title={on ? `${on.singer}, on Mic ${i + 1}: the chain behind it` : `The chain behind Mic ${i + 1}`}
            onclick={() => (chains[i] = !chains[i])}
            >{on ? on.singer : `Mic ${i + 1}`}<span class="more" aria-hidden="true">{chains[i] ? " ▾" : " ▸"}</span
            ></button
          >
          <button
            class="switch"
            aria-pressed={strip.open}
            aria-label="Mic {i + 1} {strip.open ? 'open' : 'closed'}"
            onclick={() => void set(i, { ...strip, open: !strip.open })}>{strip.open ? "Open" : "Closed"}</button
          >
          <input
            class="fader"
            type="range"
            min={fader[0]}
            max={fader[1]}
            step="0.5"
            value={strip.gain_db}
            aria-label="Mic {i + 1} level"
            onchange={(e) => void set(i, { ...strip, gain_db: Number(e.currentTarget.value) })}
          />
          <span class="meter" aria-hidden="true">
            <span class="fill" style:width="{Math.min(1, face?.level ?? 0) * 100}%"></span>
          </span>
          <span class="squeeze" title="How hard the compressor is working"
            >{(face?.compression_db ?? 0) >= 0.5 ? `−${Math.round(face?.compression_db ?? 0)} dB` : ""}</span
          >
          <select
            aria-label="Mic {i + 1} is for"
            value={strip.preset}
            onchange={(e) => void preset(i, e.currentTarget.value as VocalPreset)}
          >
            {#each vocals?.presets ?? [] as choice (choice.id)}
              <option value={choice.id}>{choice.name}</option>
            {/each}
          </select>
          {#if quiet[i]}
            <span class="silent">nothing on Mic {i + 1}</span>
          {/if}
          {#if chains[i] && vocals}
            <div class="who" role="group" aria-label="Who is on Mic {i + 1}">
              <select
                aria-label="Who is singing on Mic {i + 1}"
                value={on?.singer ?? ""}
                onchange={(e) => void putOn(i, e.currentTarget.value || null)}
              >
                <option value="">Nobody — the strip's own chain</option>
                {#each choices(on) as name (name)}
                  <option value={name}>{name}</option>
                {/each}
              </select>
              {#if on}
                <span class="whence">{whence(on)}</span>
                <button
                  title="What {on.singer} sounds best through, kept for the next time they are on a microphone"
                  onclick={() => void keep(i)}>Keep for {on.singer}</button
                >
              {/if}
            </div>
            <StripChain
              index={i}
              {strip}
              limits={vocals.limits}
              everyStage={vocals.every_stage}
              onchange={(settings) => void set(i, settings)}
            />
          {/if}
        </li>
      {/each}
    </ul>
    {#if monitor !== null}
      <div class="monitor">
        <label for="singers-monitor-music">Music in the singers' monitor</label>
        <input
          id="singers-monitor-music"
          type="range"
          min={MONITOR_MUSIC_OFF_DB}
          max="12"
          step="1"
          value={monitor}
          oninput={(e) => monitorMusic(Number(e.currentTarget.value))}
        />
        <output for="singers-monitor-music">{level(monitor)}</output>
      </div>
    {:else}
      <p class="hint">
        No singers' monitor on this output — an interface with eight outputs or more gives them a wedge of their own,
        on outputs 7 and 8.
      </p>
    {/if}
    <button class="close" onclick={() => void close()} disabled={busy}>Close the inputs</button>
  {/if}
  {#if error}
    <p class="error" role="alert">{error}</p>
  {/if}
</section>

<style>
  .mics {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    border-top: 1px solid var(--border);
    padding-top: 0.6rem;
  }

  h3 {
    margin: 0;
    font-size: 1em;
  }

  .hint,
  .rig {
    color: var(--text-dim);
    font-size: 0.85em;
    margin: 0;
  }

  .rig[data-heavy="true"] strong,
  .lost,
  .error {
    color: var(--danger);
    margin: 0;
  }

  .open {
    display: flex;
    gap: 0.4rem;
    flex-wrap: wrap;
  }

  select,
  input {
    font: inherit;
  }

  .strips {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    container: mics / inline-size;
  }

  .strip {
    display: grid;
    grid-template-columns: max-content 4.6rem minmax(5rem, 1fr) 4rem 3.2rem auto;
    align-items: center;
    gap: 0.4rem;
    padding: 0.25rem 0.4rem;
    border: 1px solid var(--border);
    border-radius: var(--radius, 6px);
    background: var(--panel-raised);
  }

  .strip[data-open="true"] {
    border-color: var(--accent);
  }

  /* WebKitGTK draws a select wider than Chromium does: in the application
     the preset ran past the panel's edge until it was let give way. The
     browser tests cannot see it; the application could. */
  .strip select {
    min-width: 0;
    max-width: 100%;
  }

  /* A narrow panel — the Singers surface docked at the side — has no room for
     a row's six things on one line: the level and the compression go under
     the fader, and the row stays inside the panel. */
  @container mics (max-width: 30rem) {
    .strip {
      grid-template-columns: max-content max-content minmax(3rem, 1fr) minmax(5.5rem, 7rem);
    }

    .meter {
      grid-column: 3;
      grid-row: 2;
    }

    .squeeze {
      grid-column: 4;
      grid-row: 2;
    }
  }

  .name {
    font: inherit;
    font-weight: 600;
    white-space: nowrap;
    /* A singer's name in place of "Mic 3": a long one is cut, not let push
       the fader off the row. */
    max-width: 8rem;
    overflow: hidden;
    text-overflow: ellipsis;
    text-align: left;
    padding: 0.1rem 0.2rem;
    border: none;
    background: none;
    color: inherit;
    cursor: pointer;
  }

  .more {
    color: var(--text-dim);
  }

  .switch[aria-pressed="true"] {
    color: var(--accent);
  }

  .meter {
    position: relative;
    height: 0.5rem;
    border-radius: 0.25rem;
    background: var(--panel);
    overflow: hidden;
    outline: 1px solid var(--border);
  }

  .strip[data-gate="true"] .meter {
    outline-color: var(--accent);
  }

  .fill {
    position: absolute;
    inset: 0 auto 0 0;
    background: var(--accent);
  }

  .squeeze,
  .silent {
    color: var(--text-dim);
    font-size: 0.8em;
  }

  .silent {
    grid-column: 1 / -1;
  }

  .who {
    grid-column: 1 / -1;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.3rem 0.5rem;
    padding-top: 0.3rem;
    font-size: 0.85em;
  }

  .whence {
    color: var(--text-dim);
  }

  .monitor {
    display: grid;
    grid-template-columns: auto minmax(6rem, 1fr) 3.2rem;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.9em;
  }

  .monitor output {
    color: var(--text-dim);
    text-align: right;
  }

  .close {
    align-self: flex-start;
  }
</style>
