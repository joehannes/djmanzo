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
  import {
    chosenPack,
    cockpitLocks,
    knowledgePacks,
    padPages,
    applySetup,
    railControls,
    remembered,
    setChosenPack,
    setups,
    waveformLayers,
    type KnowledgePack,
    type LockOption,
    type PadPageDto,
    type RailControl,
    type Remembered,
    type Setup,
    type WaveformLayer,
  } from "./api";
  import {
    remembers,
    starPage,
    keepControl,
    showLayer,
    loadRemembers,
  } from "./remembers.svelte";
  import { open, save as saveDialog } from "@tauri-apps/plugin-dialog";
  import {
    addMusicFolder,
    clearBrandLogo,
    clearSecret,
    hasBrandLogo,
    listSources,
    logoUrl,
    musicLibrary,
    openSignupLink,
    removeMusicFolder,
    secretsPersist,
    setBrandLogo,
    setSecret,
    remoteStatus,
    startRemote,
    stopRemote,
    startOsc,
    stopOsc,
    clockStatus,
    midiOutputs,
    startClock,
    stopClock,
    followClock,
    unfollowClock,
    controlStatus,
    stemOut,
    setStemOut,
    peerStatus,
    startPeerSync,
    stopPeerSync,
    type PeerStatus,
    setDeckOut,
    listInputs,
    timecodeStatus,
    startTimecode,
    stopTimecode,
    writeTimecodeSignal,
    type Device,
    type TimecodeStatus,
    type StemOut,
    type ClockStatus,
    type MidiOutputs,
    type Library,
    type RemoteStatus,
    type Source,
  } from "./api";
  import { theme, type ThemePreference } from "./theme.svelte";
  import { performance, type PerformanceLevel } from "./performance.svelte";
  import SvgPad from "./controls/SvgPad.svelte";
  import { themePackages } from "./controls/themes/packages";
  import IconButton from "./controls/IconButton.svelte";
  import SvgKnob from "./controls/SvgKnob.svelte";

  let {
    onLogoChange,
    deviceChannels = null,
    locked = [],
    onLock,
    onSetUp,
  }: {
    onLogoChange: () => void;
    /**
     * §79's locks that are on, by slug.
     *
     * Passed in and handed back rather than read and written here, because a
     * lock is a field of the workspace and the workspace belongs to the shell:
     * two components writing `set_cockpit_workspace` from their own copies is
     * how one of them silently drops what the other just saved.
     */
    locked?: string[];
    /** Store a new set of locks. */
    onLock?: (locked: string[]) => void;
    /**
     * Open an arrangement and wear a theme, for §54's presets.
     *
     * Handed up rather than done here, and for the same reason the locks are:
     * the cockpit is resolved against a window only the shell has measured,
     * and two components applying a workspace from their own copies is how one
     * silently drops what the other just saved.
     */
    onSetUp?: (workspace: string, theme: string) => void;
    /**
     * How many outputs the open device has, or `null` for none.
     *
     * Passed in rather than asked for once on mount, because the sequence a DJ
     * actually performs is: open this panel, pick the interface, press
     * Reconnect. A panel that asked only on mount would still be telling them
     * their interface is too narrow while the wide one they just connected was
     * running.
     */
    deviceChannels?: number | null;
  } = $props();

  /**
   * §78 and §79's switches.
   *
   * The list is Rust's — `cockpit::Lock::ALL`, with each one's sentence — so a
   * lock added there appears here without this file being touched, and the
   * wording a DJ reads is the wording the rule was written with.
   */
  let lockOptions = $state<LockOption[]>([]);
  $effect(() => {
    void cockpitLocks()
      .then((got) => (lockOptions = got))
      // A panel that cannot list the locks shows none rather than a broken row:
      // every other setting on this surface still works.
      .catch(() => {});
  });

  const frozen = $derived(
    lockOptions.length > 0 && lockOptions.every((lock) => locked.includes(lock.slug)),
  );

  function toggleLock(slug: string, on: boolean) {
    const next = locked.filter((held) => held !== slug);
    if (on) next.push(slug);
    onLock?.(next);
  }


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

  let remote = $state<RemoteStatus>({
    running: false,
    address: null,
    token_set: false,
    error: null,
    osc: null,
  });
  // 9000 is what TouchOSC and most surfaces default to.
  let oscAddress = $state("127.0.0.1:9000");

  async function startOscPort() {
    try {
      remote = await startOsc(oscAddress);
    } catch (e) {
      remote = { ...remote, osc: null, error: String(e) };
    }
  }

  async function stopOscPort() {
    remote = await stopOsc();
  }
  // Loopback and a port well clear of anything a DJ laptop already runs.
  let remoteAddress = $state("127.0.0.1:7654");
  let remoteToken = $state("");

  async function startRemoteServer() {
    try {
      remote = await startRemote(remoteAddress, remoteToken);
    } catch (e) {
      // The refusal is the useful part -- "not loopback, so a passphrase is
      // required" is an instruction, not a failure.
      remote = { ...remote, running: false, error: String(e) };
    }
  }

  let clock = $state<ClockStatus>({
    running: false,
    port: null,
    error: null,
    following: null,
    external_bpm: null,
  });
  let clockInputs = $state<string[]>([]);
  let followPort = $state<string | null>(null);

  async function followClockIn() {
    if (!followPort) return;
    try {
      clock = await followClock(followPort);
    } catch (e) {
      clock = { ...clock, following: null, error: String(e) };
    }
  }

  async function unfollowClockIn() {
    clock = await unfollowClock();
  }
  let outputs = $state<MidiOutputs>({ ports: [], unavailable: null });

  /**
   * Sending one deck out in parts, for an external mixer or a DAW.
   *
   * `supported` is a fact about the interface that is open, so it is asked for
   * rather than guessed. The card says which of the two reasons the control is
   * unavailable — no device, or a device too narrow — because only one of them
   * is something the DJ can do anything about.
   */
  let stems = $state<StemOut>({
    deck: null,
    decks: null,
    deckCapacity: 0,
    channels: null,
    required: 8,
    supported: false,
  });

  // Re-ask whenever the device changes width. Reading the prop is what
  // subscribes this effect to it; the value itself comes back from the
  // backend, so the panel never computes support from a number the engine did
  // not agree to.
  $effect(() => {
    void deviceChannels;
    void refreshStemOut();
  });

  async function refreshStemOut() {
    try {
      stems = await stemOut();
    } catch {
      // Left as it was. A failed re-ask is not evidence the device changed.
    }
  }

  /**
   * Timecode vinyl.
   *
   * Polled while this panel is open rather than carried on the 60 Hz snapshot:
   * quality and speed move at audio rate, but nobody watches them except while
   * setting a turntable up, and a deck's own display has no room for them.
   */
  let vinyl = $state<TimecodeStatus>({
    decks: [],
    formats: [],
    engineRunning: false,
    caveat: "",
  });
  let inputs = $state<Device[]>([]);
  let vinylDevice = $state<string | null>(null);
  let vinylFormat = $state<string | null>(null);
  let vinylAbsolute = $state(false);
  let vinylError = $state<string | null>(null);
  let written = $state<string | null>(null);

  /**
   * Poll only while a deck is on a record.
   *
   * A calibration reading that nobody is producing is not worth a timer, and
   * this panel is open for minutes at a time while a DJ fills in credentials.
   */
  $effect(() => {
    if (!vinyl.decks.some((d) => d.running)) return;
    const timer = setInterval(() => void refreshVinyl(), 250);
    return () => clearInterval(timer);
  });

  async function refreshVinyl() {
    try {
      vinyl = await timecodeStatus();
    } catch {
      // Left as it was; a failed poll is not evidence the needle lifted.
    }
  }

  async function toggleVinyl(deck: number, running: boolean) {
    try {
      vinyl = running
        ? await stopTimecode(deck)
        : await startTimecode(deck, vinylDevice, vinylFormat, vinylAbsolute);
      vinylError = null;
    } catch (e) {
      vinylError = String(e);
    }
  }

  async function makeControlSignal() {
    const path = await saveDialog({
      title: "Write a control signal",
      defaultPath: "djmanzo-timecode.wav",
      filters: [{ name: "WAV", extensions: ["wav"] }],
    });
    if (typeof path !== "string") return;
    busy = true;
    try {
      const made = await writeTimecodeSignal(path, vinylFormat, null, 44100);
      written = `Wrote ${made.seconds.toFixed(1)}s of ${made.format} to ${made.path}`;
      vinylError = null;
    } catch (e) {
      vinylError = String(e);
      written = null;
    } finally {
      busy = false;
    }
  }

  /**
   * How to draw one deck's reading.
   *
   * Three states from one number, and they must not collapse: negative is not
   * on a record, zero is on one and hearing nothing — a dead cartridge, a
   * lifted needle, the wrong input picked — and above that is reading.
   */
  function vinylReading(quality: number, speed: number): string {
    if (quality < 0) return "not on a record";
    if (quality < 0.05) return "connected, hearing nothing — check the input and the cartridge";
    if (quality < 0.5) return `struggling (${Math.round(quality * 100)}%) at ${speed.toFixed(2)}×`;
    return `reading (${Math.round(quality * 100)}%) at ${speed.toFixed(2)}×`;
  }

  let network = $state<PeerStatus>({
    running: false,
    address: null,
    sendTo: null,
    peers: 0,
    peerBpm: null,
    error: null,
  });
  let peerListen = $state("127.0.0.1:7655");
  let peerSendTo = $state("127.0.0.1:7655");

  async function togglePeerSync() {
    try {
      network = network.running
        ? await stopPeerSync()
        : await startPeerSync(peerListen, peerSendTo);
      error = null;
    } catch (e) {
      error = String(e);
    }
  }

  async function chooseDeckOut(decks: number | null) {
    try {
      stems = await setDeckOut(decks);
      error = null;
    } catch (e) {
      error = String(e);
    }
  }

  async function chooseStemOut(deck: number | null) {
    try {
      stems = await setStemOut(deck);
      error = null;
    } catch (e) {
      error = String(e);
    }
  }
  let clockPort = $state<string | null>(null);

  async function startClockOut() {
    if (!clockPort) return;
    try {
      clock = await startClock(clockPort);
    } catch (e) {
      clock = { ...clock, running: false, error: String(e) };
    }
  }

  async function stopClockOut() {
    clock = await stopClock();
  }

  async function stopRemoteServer() {
    remote = await stopRemote();
    // Typed once, kept out of memory once it is no longer in force.
    remoteToken = "";
  }

  $effect(() => {
    void refresh();
  });

  async function refresh() {
    try {
      [sources, library, persist, logo, remote] = await Promise.all([
        listSources(),
        musicLibrary(),
        secretsPersist(),
        hasBrandLogo(),
        remoteStatus(),
      ]);
      const [status, ports, control, parts, net, vin, ins] = await Promise.all([
        clockStatus(),
        midiOutputs(),
        controlStatus(),
        stemOut(),
        peerStatus(),
        timecodeStatus(),
        listInputs(),
      ]);
      clock = status;
      outputs = ports;
      clockInputs = control.inputs;
      stems = parts;
      network = net;
      vinyl = vin;
      inputs = ins;
      if (!vinylFormat) vinylFormat = vinyl.formats[0]?.name ?? null;
      if (!vinylDevice) vinylDevice = inputs.find((d) => d.is_default)?.id ?? inputs[0]?.id ?? null;
      if (!clockPort) clockPort = clock.port ?? outputs.ports[0] ?? null;
      if (!followPort) followPort = clock.following ?? clockInputs[0] ?? null;
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

  /**
   * §8 Level 1's own list, read off Rust.
   *
   * Nine rows including the one djmanzo does *not* keep, which says so. A list
   * of eight would read as the whole of §8, and a preference that is silently
   * forgotten looks exactly like one that was never set.
   */
  let remembers_list = $state<Remembered[]>([]);
  /** Every pad page there is, so the stars have something to be set against. */
  let allPages = $state<PadPageDto[]>([]);
  /** Every control §74's rail can hold. */
  let allControls = $state<RailControl[]>([]);
  /** §25's twenty, and which of them a DJ may turn off. */
  let allLayers = $state<WaveformLayer[]>([]);
  /** §54's functional presets, one per kind of night §81 names. */
  let allSetups = $state<Setup[]>([]);
  /** The one whose changes are being read, before anything is applied. */
  let reading = $state<string | null>(null);
  /** What the last one actually did, so nothing is hidden after the fact. */
  let didSetUp = $state<{ title: string; changes: string[] } | null>(null);

  /**
   * §16's knowledge packs, and the one in force.
   *
   * Read off Rust, every field of it. §16 ends *"Do not hard-code this logic
   * into UI components"*, and the families, the techniques and the ceilings all
   * live in tables Rust owns; a picker that spelled out "house, tech house,
   * disco" here would be the second description of a table that already exists,
   * and the copy is the one that goes stale.
   */
  let allPacks = $state<KnowledgePack[]>([]);
  /** The chosen pack's id, or empty for all of djmanzo's knowledge. */
  let pack = $state("");

  /**
   * Choose a pack, taking back what Rust will actually use.
   *
   * The round trip matters: a slug this build does not have is dropped rather
   * than stored, and showing the stored value instead of the honoured one would
   * leave the picker claiming a curriculum the coach is not teaching from.
   */
  async function choosePack(id: string) {
    try {
      pack = await setChosenPack(id === pack ? "" : id);
      error = null;
    } catch (e) {
      error = String(e);
    }
  }

  /**
   * Apply one of §54's presets.
   *
   * Rust does what Rust owns and hands back the two the shell does, which is
   * why this is three steps rather than one call: the layers, the pad pages,
   * the assistant's posture and the request page are set in Rust; the
   * arrangement and the theme go up to the shell; and `loadRemembers` pulls the
   * first two back so the pickers below this block show what was just applied
   * rather than what was there a second ago.
   */
  async function setUp(preset: Setup) {
    try {
      const applied = await applySetup(preset.slug);
      onSetUp?.(applied.workspace, applied.theme);
      await loadRemembers();
      didSetUp = { title: preset.title, changes: applied.changes };
      reading = null;
      error = null;
    } catch (e) {
      error = String(e);
    }
  }

  $effect(() => {
    // Deck 1 because the *names* of the pages are the same on every deck — only
    // the action strings are addressed to a deck number, and none of those are
    // read here.
    void Promise.all([
      remembered(),
      padPages(1),
      railControls(),
      waveformLayers(),
      setups(),
      knowledgePacks(),
      chosenPack(),
    ])
      .then(([rows, pages, controls, layers, nights, packs, chosen]) => {
        remembers_list = rows;
        allPages = pages;
        allControls = controls;
        allLayers = layers;
        allSetups = nights;
        allPacks = packs;
        pack = chosen;
      })
      .catch(() => {
        // The block draws nothing rather than a guess, on the same principle as
        // the column picker: a settings page that invented its own list of what
        // djmanzo remembers would be the second description this table exists
        // to prevent.
      });
  });

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
    §54 first, because it is the one control here that changes six others. A DJ
    setting up for a wedding had to find the arrangement, the theme, the
    waveform layers, the pad pages, the assistant's posture and the request
    page separately; this is that evening in one press.
  -->
  <div class="block setups">
    <h3>Set up for tonight</h3>
    <p class="hint">
      Starting points, not identities — every control one of these touches stays
      where you can change it afterwards. Press one to read what it would do;
      press it again to do it.
    </p>
    <ul class="night-list">
      {#each allSetups as night (night.slug)}
        <li data-setup={night.slug}>
          <button
            class:chosen={reading === night.slug}
            aria-expanded={reading === night.slug}
            onclick={() =>
              reading === night.slug ? void setUp(night) : (reading = night.slug)}
          >
            <span class="night-name">{night.title}</span>
            <span class="night-about">{night.about}</span>
          </button>
          <!--
            The list *before* anything happens, not after. §54's presets touch
            six systems at once, and the rule `dj_app::presets` states about the
            small ones applies with more force here: a preset that silently
            changes six things is the kind of feature people stop trusting.
          -->
          {#if reading === night.slug}
            <ul class="will">
              {#each night.changes as change (change)}
                <li>{change}</li>
              {/each}
            </ul>
            <p class="hint again">Press {night.title} again to apply it.</p>
          {/if}
        </li>
      {/each}
    </ul>
    {#if didSetUp}
      <p class="did" role="status">
        {didSetUp.title}: {didSetUp.changes.join(" · ")}
      </p>
    {/if}
  </div>

  <!--
    §16, directly under §54, because they are the two halves of the same
    evening: the setup is how djmanzo *looks* for this kind of night, the pack
    is what it *knows* about it. Every word in this block comes off Rust — the
    families, the ceiling and the count are read from the tables that own them,
    because §16 ends "Do not hard-code this logic into UI components" and a
    picker that spelled out a genre list would be exactly that.
  -->
  <div class="block packs">
    <h3>What djmanzo knows</h3>
    <p class="hint">
      A pack narrows what the coach teaches you to the moves your kind of night
      actually uses. Choosing none leaves the whole catalogue on, which is the
      right answer if you play everything. Press the one you are on to turn it
      off again.
    </p>
    <ul class="pack-list">
      {#each allPacks as body (body.id)}
        <li data-pack={body.id}>
          <button
            class:chosen={pack === body.id}
            aria-pressed={pack === body.id}
            onclick={() => void choosePack(body.id)}
          >
            <span class="pack-name">{body.title}</span>
            <span class="pack-about">{body.about}</span>
            <!--
              The count is what choosing a pack *costs*. A pack is a narrowing,
              and a DJ pressing one deserves to see how far it narrows before
              the coach quietly stops mentioning half the catalogue.
            -->
            <span class="pack-reach">
              {body.teaches} moves{body.families.length > 0
                ? ` · ${body.families.join(", ")}`
                : ""}
            </span>
          </button>
        </li>
      {/each}
    </ul>
  </div>

  <!--
    Screens first, because it is the one setting that changes the shape of the
    whole application rather than a detail inside it.
  -->
  <div class="block">
    <Screens />
  </div>

  <!--
    §78 and §79, above Appearance on purpose: everything below this point is a
    thing djmanzo might otherwise change on its own, and this is the switch that
    says it may not.
  -->
  <div class="block locks">
    <h3>Lock my workflow</h3>
    <p class="hint">
      djmanzo adapts: it opens a panel when the night turns over, fits the
      density to the window, wears a palette the set has earned, and answers the
      audio in its own colours. Any of that can be turned off, and none of it
      stops the assistant thinking — it keeps reading the room, planning and
      offering; it just stops moving your screen.
    </p>
    <div class="row">
      <button
        class="freeze"
        class:active={frozen}
        aria-pressed={frozen}
        onclick={() => onLock?.(frozen ? [] : lockOptions.map((lock) => lock.slug))}
        title="Every lock at once — nothing about the interface changes unless you change it"
      >
        {frozen ? "Frozen" : "Freeze everything"}
      </button>
    </div>
    <ul class="lock-list">
      {#each lockOptions as lock (lock.slug)}
        <!-- Named on the row rather than found by its words: two of §79's six
             sentences contain another one's name ("The *arrangement* stays the
             one you chose" is the workspace lock), so a test matching on text
             matches two switches. -->
        <li data-lock={lock.slug}>
          <label>
            <input
              type="checkbox"
              checked={locked.includes(lock.slug)}
              onchange={(event) => toggleLock(lock.slug, event.currentTarget.checked)}
            />
            <span class="lock-name">{lock.slug}</span>
            <span class="lock-about">{lock.about}</span>
          </label>
        </li>
      {/each}
    </ul>
  </div>

  <!--
    §8 Level 1, directly under §79's locks and on purpose: that block says what
    djmanzo may change about itself, and this one says what it keeps about you.
    They are the two halves of one question a DJ asks once — "will this be the
    way I left it tomorrow" — and answering them ten sections apart would be
    answering half of it.
  -->
  <div class="block remembers">
    <h3>What djmanzo remembers</h3>
    <p class="hint">
      Nine things survive a restart, and one does not. The list is djmanzo's own
      rather than a description of it: a test fails if a row here claims
      something no file backs.
    </p>
    <ul class="remember-list">
      {#each remembers_list as row (row.slug)}
        <li data-remembers={row.slug} class:unkept={!row.kept}>
          <span class="remember-about">{row.about}</span>
          {#if row.kept}
            <span class="remember-cost">Forgotten: {row.forgotten}</span>
          {:else}
            <!-- Said plainly, in the place a DJ would go looking. "Not yet, and
                 here is what has to happen first" is a different thing from
                 "no". -->
            <span class="remember-not">Not yet — {row.why_not}</span>
          {/if}
        </li>
      {/each}
    </ul>

    <h4>Pad pages you play from</h4>
    <p class="hint">
      Starred pages come first on every deck's pad zone. The rest stay where
      they are — a page you never star is still a page you can reach.
    </p>
    <ul class="picker" data-picker="pad-pages">
      {#each allPages as page (page.name)}
        <li>
          <label>
            <input
              type="checkbox"
              checked={remembers.pages.includes(page.name)}
              onchange={(event) => void starPage(page.name, event.currentTarget.checked)}
            />
            <span class="pick-name">{page.name}</span>
          </label>
        </li>
      {/each}
    </ul>

    <h4>What the waveform draws</h4>
    <p class="hint">
      §25's twenty semantic layers. Twelve exist; the rest are named rather than
      offered empty, because a box that ticks and changes nothing teaches you
      the wrong thing about the ones that work. Amplitude and spectral balance
      are the waveform itself and cannot be turned off.
    </p>
    <ul class="picker" data-picker="waveform-layers">
      {#each allLayers as layer (layer.name)}
        <li data-layer-row={layer.name} class:unavailable={!layer.choosable}>
          <label>
            <input
              type="checkbox"
              checked={remembers.layers.includes(layer.name)}
              disabled={!layer.choosable}
              onchange={(event) => void showLayer(layer.name, event.currentTarget.checked)}
            />
            <span class="pick-name">{layer.title}</span>
            <!-- The reason, where a disabled box would otherwise just look
                 broken. The same posture §20's title column takes. -->
            <span class="pick-about">{layer.choosable ? layer.about : layer.why_not}</span>
          </label>
        </li>
      {/each}
    </ul>

    <h4>Controls to keep within reach</h4>
    <p class="hint">
      The rail holds the four to eight controls djmanzo judges you need this
      second. Anything kept here is on it whatever the deck is doing — keep
      eight and the rail stops moving altogether, which is a thing you are
      allowed to want.
    </p>
    <ul class="picker" data-picker="rail-controls">
      {#each allControls as control (control.slug)}
        <li>
          <label>
            <input
              type="checkbox"
              checked={remembers.controls.includes(control.slug)}
              onchange={(event) => void keepControl(control.slug, event.currentTarget.checked)}
            />
            <span class="pick-name">{control.slug}</span>
            <span class="pick-about">{control.about}</span>
          </label>
        </li>
      {/each}
    </ul>
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
        <SvgKnob size={48} value={0.6} min={0} max={1} />
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
    <h3>Stems out</h3>
    <p class="hint">
      Sends one deck out as four separate stereo pairs — vocals, drums, bass and
      everything else — for an external mixer or a DAW. The parts leave before
      djmanzo's own EQ, filter, fader and keylock, because what a processor on
      the other end wants is the separated parts, not the parts with someone
      else's tone shaping already on them.
    </p>
    <p class="hint">
      <strong>This takes the whole output.</strong> Four stems need
      {stems.required} channels, which leaves nowhere for a master — so while
      it is on, the room is hearing the external mixer, not djmanzo.
    </p>
    {#if stems.channels === null}
      <p class="hint">
        No audio device is open. Connect one with at least {stems.required}
        outputs and this becomes available.
      </p>
    {:else if !stems.supported}
      <p class="hint">
        The open interface has {stems.channels}
        {stems.channels === 1 ? "output" : "outputs"} and this needs
        {stems.required}. You can still choose a deck now — it takes effect if
        you plug in a wider interface later.
      </p>
    {/if}
    <div class="row" style="gap: 0.5rem;">
      <button
        class:active={stems.deck === null}
        onclick={() => chooseStemOut(null)}
      >
        Off
      </button>
      {#each [1, 2, 3, 4] as n (n)}
        <button
          class:active={stems.deck === n}
          onclick={() => chooseStemOut(stems.deck === n ? null : n)}
        >
          Deck {n}
        </button>
      {/each}
    </div>
    {#if stems.deck !== null && stems.supported}
      <p class="hint">
        Deck {stems.deck} is going out in parts: vocals on outputs 1–2, drums on
        3–4, bass on 5–6, everything else on 7–8. A stem you mute here is silent
        on its own pair and leaves the other three alone. Until the track has
        been separated, the pairs carry silence rather than four copies of the
        mix.
      </p>
    {/if}
  </div>

  <div class="block">
    <h3>Decks out</h3>
    <p class="hint">
      Sends each deck out on a stereo pair of its own instead of mixing them,
      for mixing on an external mixer. Deck 1 on outputs 1–2, deck 2 on 3–4, and
      so on — pre-fader, because the mixer on the other end has its own fader
      and two in series is one the person standing at it cannot see.
    </p>
    <p class="hint">
      <strong>This takes the whole output</strong>, like Stems out: there is no
      master left to send anywhere and no headphone cue to take from a mix that
      no longer exists. Choosing one of the two puts the other away.
    </p>
    {#if stems.deckCapacity === 0}
      <p class="hint">
        No audio device is open. Connect one and this becomes available — each
        deck needs two outputs.
      </p>
    {:else}
      <p class="hint">
        The open interface has {stems.channels}
        {stems.channels === 1 ? "output" : "outputs"}, which is room for
        {stems.deckCapacity}
        {stems.deckCapacity === 1 ? "deck" : "decks"}.
      </p>
    {/if}
    <div class="row" style="gap: 0.5rem;">
      <button
        class:active={stems.decks === null}
        onclick={() => chooseDeckOut(null)}
      >
        Off
      </button>
      {#each [2, 4, 6] as n (n)}
        <button
          class:active={stems.decks === n}
          disabled={n > stems.deckCapacity}
          title={n > stems.deckCapacity
            ? `Needs ${n * 2} outputs; this interface has ${stems.channels ?? 0}`
            : `Send ${n} decks out on ${n} pairs`}
          onclick={() => chooseDeckOut(stems.decks === n ? null : n)}
        >
          {n} decks
        </button>
      {/each}
    </div>
    {#if stems.decks !== null}
      <p class="hint">
        {stems.decks} decks are going out separately, pre-fader. djmanzo's own
        crossfader, master gain, microphone and limiter are out of the path —
        the mixing is happening on the other end of the cables.
      </p>
    {/if}
  </div>

  <div class="block">
    <h3>Timecode vinyl</h3>
    <p class="hint">
      A control record on a real turntable drives a deck: the platter's speed
      becomes the track's speed, and scratching the record scratches the track.
      What the needle picks up is not music but a signal, and djmanzo reads it
      back into a position and a speed.
    </p>
    <p class="hint">
      <strong>Read this before buying a record.</strong> {vinyl.caveat}
    </p>

    <h4>Make a control signal</h4>
    <p class="hint">
      Writes djmanzo's own timecode to a WAV. Burn it to a CD, put it on a USB
      stick or play it off a phone — any turntable, CD deck or media player then
      controls a deck, without buying anything. On a turntable this needs a
      record; on a CD deck or a phone it does not.
    </p>
    <div class="row" style="gap: 0.5rem; flex-wrap: wrap;">
      <select class="grow" aria-label="Timecode format" bind:value={vinylFormat}>
        {#each vinyl.formats as format (format.name)}
          <option value={format.name} disabled={!format.usable}>
            {format.name} — {Math.round(format.carrierHz)} Hz, good for
            {Math.round(format.unambiguousSeconds / 60)} min
          </option>
        {/each}
      </select>
      <button onclick={makeControlSignal} disabled={busy || !vinylFormat}>
        Write a WAV
      </button>
    </div>
    {#if written}
      <p class="hint" role="status">{written}</p>
    {/if}

    <h4>Put a deck on a record</h4>
    {#if !vinyl.engineRunning}
      <p class="hint">
        No audio device is open. A control record is an input attached to a
        running engine, and there is no engine until an output is connected.
      </p>
    {:else if inputs.length === 0}
      <p class="hint">
        Nothing on this machine can capture. A turntable needs a phono stage and
        an interface between it and the computer.
      </p>
    {:else}
      <div class="row" style="gap: 0.5rem; flex-wrap: wrap;">
        <select class="grow" aria-label="Input the timecode arrives on" bind:value={vinylDevice}>
          {#each inputs as device (device.id)}
            <option value={device.id}>{device.name}</option>
          {/each}
        </select>
        <button
          class:active={vinylAbsolute}
          title={vinylAbsolute
            ? "Where the needle sits on the record is where the playhead sits in the track"
            : "Only the movement is followed; lifting and re-dropping changes nothing"}
          onclick={() => (vinylAbsolute = !vinylAbsolute)}
        >
          {vinylAbsolute ? "Absolute" : "Relative"}
        </button>
      </div>
      <p class="hint">
        {#if vinylAbsolute}
          <strong>Absolute.</strong> Dropping the needle two minutes into the
          record starts the track two minutes in. Needs a clean record: a skip
          moves the playhead.
        {:else}
          <strong>Relative.</strong> Only the movement is followed, so nudging
          the record to beatmatch does not undo itself and a lift-and-drop
          changes nothing. What most DJs want most of the time.
        {/if}
      </p>
      <div class="vinyl-decks">
        {#each vinyl.decks as deck (deck.deck)}
          <div class="vinyl-deck">
            <button
              class:active={deck.running}
              onclick={() => toggleVinyl(deck.deck, deck.running)}
            >
              Deck {deck.deck}
            </button>
            <span class="hint">
              {#if deck.running}
                {vinylReading(deck.quality, deck.speed)} · {deck.format} ·
                {deck.absolute ? "absolute" : "relative"} · {deck.device}
              {:else}
                on its own transport
              {/if}
            </span>
          </div>
        {/each}
      </div>
    {/if}
    {#if vinylError}
      <p class="hint" role="status">{vinylError}</p>
    {/if}
  </div>

  <div class="block">
    <h3>Network tempo sync</h3>
    <p class="hint">
      Keeps two djmanzo instances on one tempo and one downbeat — a second
      laptop, or a DJ playing back to back with you. Every instance both
      announces and listens; there is no master, so they converge on each other
      and one appearing or leaving costs nothing.
    </p>
    <p class="hint">
      <strong>This is not Ableton Link.</strong> Link is GPL-or-proprietary, so
      djmanzo cannot link it (ADR-0002), and a reimplementation that claimed
      Link compatibility without ever having been tested against Live or Serato
      would be a claim nobody here could stand behind. It syncs djmanzo to
      djmanzo, and says so.
    </p>
    <p class="hint">
      There is no passphrase, and unlike the control server that is deliberate:
      UDP has no handshake to carry one. What a stranger on the network can do
      is bounded instead — a tempo more than six percent away is ignored, and
      the nudge that reaches a deck is clamped to one percent.
    </p>
    <div class="row" style="gap: 0.5rem; flex-wrap: wrap;">
      <label class="peer-field">
        Listen on
        <input bind:value={peerListen} disabled={network.running} spellcheck="false" />
      </label>
      <label class="peer-field">
        Announce to
        <input bind:value={peerSendTo} disabled={network.running} spellcheck="false" />
      </label>
      <button class:active={network.running} onclick={togglePeerSync}>
        {network.running ? "Stop" : "Start"}
      </button>
    </div>
    {#if network.error}
      <p class="hint" role="status">{network.error}</p>
    {:else if network.running}
      <p class="hint" role="status">
        Listening on {network.address}, announcing to {network.sendTo}.
        {#if network.peers === 0}
          No other instances yet.
        {:else}
          {network.peers}
          {network.peers === 1 ? "instance" : "instances"} on the network{#if network.peerBpm}, at {network.peerBpm.toFixed(2)} BPM{:else}, none of them playing{/if}.
        {/if}
      </p>
    {/if}
  </div>

  <div class="block">
    <h3>MIDI clock</h3>
    <p class="hint">
      Makes djmanzo the clock master: twenty-four pulses to the beat, at
      whatever the loudest playing deck is doing. A drum machine, a light desk
      or a second piece of software follows it. It is the oldest sync protocol
      there is and still the one most likely to be on the other end of a cable
      in a small club.
    </p>
    {#if outputs.unavailable}
      <p class="hint">
        {outputs.unavailable}. That is MIDI itself, not an empty list — plugging
        something in will not change it.
      </p>
    {:else if outputs.ports.length === 0}
      <p class="hint">No MIDI outputs on this machine.</p>
    {:else}
      <div class="row">
        <select class="grow" aria-label="MIDI port to send clock on" bind:value={clockPort} disabled={clock.running}>
          {#each outputs.ports as port (port)}
            <option value={port}>{port}</option>
          {/each}
        </select>
        {#if clock.running}
          <IconButton icon="ban" title="Stop the clock" onClick={stopClockOut} />
        {:else}
          <IconButton
            icon="check"
            title="Send clock to this output"
            onClick={startClockOut}
            disabled={!clockPort}
          />
        {/if}
      </div>
    {/if}
    {#if clock.running}
      <p class="hint">
        Sending to <code>{clock.port}</code>. Pausing every deck sends a stop
        rather than going quiet — a follower that simply stops hearing pulses
        decides it has lost its master.
      </p>
    {/if}

    <h4>Follow somebody else's clock</h4>
    <p class="hint">
      The other direction: djmanzo takes its tempo from a drum machine or a
      second DJ. While a clock is being followed it <em>outranks every deck</em>
      as the sync leader — a DJ who plugged the room's clock in wants the
      room's clock, not deck 1.
    </p>
    {#if clockInputs.length === 0}
      <p class="hint">No MIDI inputs to follow.</p>
    {:else}
      <div class="row">
        <select class="grow" aria-label="MIDI port to follow clock from" bind:value={followPort} disabled={!!clock.following}>
          {#each clockInputs as port (port)}
            <option value={port}>{port}</option>
          {/each}
        </select>
        {#if clock.following}
          <IconButton icon="ban" title="Stop following" onClick={unfollowClockIn} />
        {:else}
          <IconButton
            icon="check"
            title="Follow this input"
            onClick={followClockIn}
            disabled={!followPort}
          />
        {/if}
      </div>
    {/if}
    {#if clock.following}
      <p class="hint">
        Following <code>{clock.following}</code>{clock.external_bpm
          ? ` at ${clock.external_bpm.toFixed(1)} BPM.`
          : " — still counting pulses."}
      </p>
    {/if}

    {#if clock.error}
      <p class="warn">{clock.error}</p>
    {/if}
  </div>

  <div class="block">
    <h3>Remote control</h3>
    <p class="hint">
      Lets something else drive djmanzo — a script, a Stream Deck, a lighting
      desk — using the same actions the interface and your controller send. One
      JSON object per line, over TCP. Nothing it can ask for is anything you
      could not do by hand.
    </p>
    <p class="hint">
      <strong>It is off until you switch it on.</strong>
      <code>127.0.0.1:7654</code> reaches this machine only, which is what a
      script on the same laptop wants. <code>0.0.0.0:7654</code> faces the
      network, and then a passphrase is <em>required</em> — otherwise anybody
      on the club's wifi has a hand on your crossfader.
    </p>
    <div class="row">
      <input
        aria-label="Address the line protocol listens on"
        class="grow"
        bind:value={remoteAddress}
        placeholder="127.0.0.1:7654"
        disabled={remote.running}
      />
      <input
        aria-label="Passphrase for the line protocol"
        class="grow"
        type="password"
        bind:value={remoteToken}
        placeholder="passphrase (required off this machine)"
        disabled={remote.running}
      />
      {#if remote.running}
        <IconButton icon="ban" title="Stop listening" onClick={stopRemoteServer} />
      {:else}
        <IconButton icon="check" title="Start listening" onClick={startRemoteServer} />
      {/if}
    </div>
    {#if remote.running}
      <p class="hint">
        Listening on <code>{remote.address}</code>{remote.token_set
          ? " — a passphrase is required."
          : " — no passphrase, which is fine on this machine only."}
      </p>
    {/if}

    <h4>OSC</h4>
    <p class="hint">
      The protocol TouchOSC, Lemur and QLab already speak. djmanzo invents no
      address space: the action grammar <em>is</em> the addresses, with slashes
      for spaces — <code>/deck/1/play</code>, <code>/deck/1/volume</code> with a
      float. A layout reads like a controller mapping.
    </p>
    <p class="hint">
      <strong>Loopback only, and that is not a default.</strong> OSC is UDP, so
      there is no handshake to carry a passphrase — nothing to authenticate
      with, so a port facing the network is refused rather than protected
      badly. Use the line protocol above for anything off this machine.
    </p>
    <div class="row">
      <input
        aria-label="Address the OSC port listens on"
        class="grow"
        bind:value={oscAddress}
        placeholder="127.0.0.1:9000"
        disabled={!!remote.osc}
      />
      {#if remote.osc}
        <IconButton icon="ban" title="Close the OSC port" onClick={stopOscPort} />
      {:else}
        <IconButton icon="check" title="Open the OSC port" onClick={startOscPort} />
      {/if}
    </div>
    {#if remote.osc}
      <p class="hint">Listening for OSC on <code>{remote.osc}</code>.</p>
    {/if}

    {#if remote.error}
      <p class="warn">{remote.error}</p>
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
        <IconButton icon="fa-solid fa-xmark" title="Remove folder" aria-label={`Remove ${folder}`} onClick={() => dropFolder(folder)} />
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
              <IconButton icon="fa-solid fa-floppy-disk" title="Save credential" onClick={() => save(credential.id)} />
              {#if credential.is_set}
                <IconButton icon="fa-solid fa-trash" title="Forget credential" aria-label="Forget credential" onClick={() => forget(credential.id)} />
              {/if}
            </div>
            <p class="free-tier">
              {credential.free_tier}
              <!-- See Assistant.svelte: a webview link cannot reach a browser. -->
              <button type="button" class="signup" onclick={() => openSignupLink(credential.signup_url)}>
                Get one →
              </button>
            </p>
          </div>
        {/each}
      </article>
    {/each}
  </div>
</section>

<style>
  .vinyl-decks {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    margin-top: 0.5rem;
  }

  .vinyl-deck {
    display: flex;
    align-items: center;
    gap: 0.6rem;
  }

  /* The reading is the thing being watched while a needle is set up, so it
     gets a fixed-width face: a number that changes width as it changes value
     makes the line jitter, and jitter reads as instability in the signal. */
  .vinyl-deck .hint {
    font-family: ui-monospace, SFMono-Regular, "SF Mono", Menlo, monospace;
    font-size: 0.8em;
    font-variant-numeric: tabular-nums;
  }

  .peer-field {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.85em;
    color: var(--text-dim);
  }

  .peer-field input {
    font-family: ui-monospace, SFMono-Regular, "SF Mono", Menlo, monospace;
    width: 12em;
  }

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

  h4 {
    margin: 0.6rem 0 0;
    font-size: 0.72rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-dim);
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

  /*
    Was an anchor until it turned out a webview anchor reaches nothing. It is a
    button now and still has to read as a link, because the DJ's understanding
    of it -- "this takes me somewhere" -- was the only correct part before.
  */
  .signup {
    background: none;
    border: none;
    padding: 0;
    font: inherit;
    color: var(--accent);
    white-space: nowrap;
    cursor: pointer;
  }
  .signup:hover {
    text-decoration: underline;
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

  /* §79's six. A list rather than a row of chips: each one carries a sentence,
     and a sentence is the difference between a switch a DJ can use and a switch
     they leave alone because they cannot tell what it does. */
  .lock-list {
    list-style: none;
    margin: 0.6rem 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  .lock-list label {
    display: flex;
    align-items: baseline;
    gap: 0.5rem;
    cursor: pointer;
  }

  .lock-name {
    min-width: 6.5rem;
    font-variant: small-caps;
    letter-spacing: 0.04em;
  }

  .lock-about {
    color: var(--muted);
    font-size: 0.85em;
  }

  .night-list {
    list-style: none;
    margin: 0.6rem 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  .night-list > li > button {
    display: flex;
    align-items: baseline;
    gap: 0.5rem;
    width: 100%;
    text-align: left;
    flex-wrap: wrap;
  }

  .night-list > li > button.chosen {
    border-color: var(--selected);
  }

  .night-name {
    min-width: 7.5rem;
    font-variant: small-caps;
    letter-spacing: 0.04em;
  }

  .night-about,
  .will,
  .did,
  .again {
    color: var(--muted);
    font-size: 0.85em;
  }

  /* Indented under the preset it belongs to, so six systems read as one
     answer rather than as six more rows in the list. */
  .will {
    list-style: none;
    margin: 0.35rem 0 0;
    padding: 0 0 0 1rem;
    border-left: 2px solid var(--edge);
  }

  .did {
    margin: 0.6rem 0 0;
    color: var(--active);
  }

  .pack-list {
    list-style: none;
    margin: 0.6rem 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  .pack-list > li > button {
    display: flex;
    align-items: baseline;
    gap: 0.5rem;
    width: 100%;
    text-align: left;
    flex-wrap: wrap;
  }

  /* An outline rather than a fill, the same as the preset above it: the chosen
     pack is a standing state, and a block of colour in a settings list reads as
     something that just happened.

     The inset edge is there because the outline alone was not enough. On screen
     the chosen row sat a hair brighter than the seven around it and had to be
     hunted for — which is the wrong failure for a standing state, since the one
     question a DJ opens this block with is *which pack am I on*. */
  .pack-list > li > button.chosen {
    border-color: var(--selected);
    box-shadow: inset 3px 0 0 var(--selected);
  }

  .pack-name {
    min-width: 7.5rem;
    font-variant: small-caps;
    letter-spacing: 0.04em;
  }

  .pack-about,
  .pack-reach {
    color: var(--muted);
    font-size: 0.85em;
  }

  .remembers h4 {
    margin: 1rem 0 0;
    font-size: 0.85em;
    font-variant: small-caps;
    letter-spacing: 0.04em;
  }

  .remember-list,
  .picker {
    list-style: none;
    margin: 0.6rem 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
  }

  .picker {
    gap: 0.35rem;
  }

  /*
    Wider between rows than within one. In a narrow dock every row wraps onto
    two lines, and with one gap for both a reader cannot tell which cost belongs
    to which thing -- the list reads as nine alternating claims instead of nine
    pairs. Driving the panel was the only way to see it: at the width the
    stylesheet was written against, every row fitted on one line.
  */
  .remember-list {
    gap: 0.7rem;
  }

  .remember-list li {
    display: block;
  }

  .remember-about {
    display: block;
  }

  /* Indented under what it is about, for the same reason. */
  .remember-cost,
  .remember-not {
    display: block;
    padding-left: 0.9rem;
  }

  .remember-cost,
  .remember-not {
    color: var(--muted);
    font-size: 0.85em;
  }


  /*
    The one row djmanzo does not keep is dimmed and its sentence warned, so the
    list can be read at a glance as "these survive, that one does not" without
    reading nine lines of prose to find the exception.
  */
  .remember-list li.unkept .remember-about {
    color: var(--muted);
  }

  .remember-not {
    color: var(--warn);
  }

  .picker label {
    display: flex;
    align-items: baseline;
    gap: 0.5rem;
    cursor: pointer;
  }

  .pick-name {
    min-width: 6.5rem;
    font-variant: small-caps;
    letter-spacing: 0.04em;
  }

  .pick-about {
    color: var(--muted);
    font-size: 0.85em;
  }

  /* A row djmanzo cannot offer reads as unavailable rather than as broken. */
  .picker li.unavailable .pick-name {
    color: var(--muted);
  }

  .picker li.unavailable label {
    cursor: default;
  }
</style>
