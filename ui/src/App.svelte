<script lang="ts">
  import Assistant from "./Assistant.svelte";
  import Browse from "./Browse.svelte";
  import Deck from "./Deck.svelte";
  import Presets from "./Presets.svelte";
  import Settings from "./Settings.svelte";
  import { watchFrameRate } from "./framerate";
  import {
    chooseLayout,
    chosenLayout,
    listLayouts,
    setWatershed,
    watershedShowing,
    type CrossfaderAssign,
    type Layout,
  } from "./api";
  import Confluence from "./Confluence.svelte";
  import River from "./River.svelte";
  import {
    alarmDeck,
    emptyWorld,
    getWorld,
    prefersStillness,
    tierFor,
    type Entity,
    type World,
  } from "./world";
  import {
    dispatch,
    getSnapshot,
    hasBrandLogo,
    listDevices,
    logoUrl,
    onSnapshot,
    openDevice,
    sessionLog,
    type ActiveDevice,
    type Device,
    type Snapshot,
  } from "./api";

  let devices = $state<Device[]>([]);
  let selectedDevice = $state<string | null>(null);
  /**
   * A second sound card for the headphone cue. Null means "keep it on the main
   * device", which is always better when that device has the channels — two
   * cards means two clocks, and a resampler between them.
   */
  let selectedCueDevice = $state<string | null>(null);
  let bufferFrames = $state(256);
  let active = $state<ActiveDevice | null>(null);
  let error = $state<string | null>(null);
  let snapshot = $state<Snapshot | null>(null);
  let log = $state<string[]>([]);
  let showLog = $state(false);
  let slowFrames = $state<number | null>(null);
  /** Which side panel is open, if any. Only one at a time: the decks matter more. */
  let panel = $state<"none" | "browse" | "assistant" | "presets" | "settings">("none");

  /**
   * The layout in force.
   *
   * `null` until the list arrives, and the interface draws its own defaults
   * meanwhile rather than nothing — a DJ opening the application should see
   * decks, not a blank window waiting on a preference.
   */
  let layout = $state<Layout | null>(null);
  let layouts = $state<Layout[]>([]);

  async function loadLayouts() {
    try {
      layouts = await listLayouts();
      // Restore last night's choice. A DJ who set the interface up the way
      // they wanted should not have to do it again before every set.
      const previous = await chosenLayout();
      if (previous) applyLayout(previous, false);
    } catch {
      // Only the DJ's own are missing; the interface has its defaults.
    }
  }

  /**
   * Apply a layout.
   *
   * Density goes on the root as a scale factor because every other measurement
   * in the interface is in `em`, so one number moves all of them together —
   * which is what "denser" means to a DJ, rather than forty separate sizes.
   */
  function applyLayout(next: Layout, remember = true) {
    layout = next;
    deckCount = next.decks;
    document.documentElement.style.setProperty("--density", String(next.density));
    if (next.browser && panel === "none") panel = "browse";
    // Not when restoring, or every start-up would rewrite the file it just
    // read — harmless, but it makes the file's timestamp a lie about when the
    // DJ last chose anything.
    if (remember) void chooseLayout(next.name).catch(() => {});
  }
  let logo = $state(false);
  /** Bumped when the logo changes, to defeat the webview's image cache. */
  let logoVersion = $state(0);
  /**
   * How many decks are on screen.
   *
   * The engine has always run four; the interface showed two. Two is the right
   * default — it is what most sets are, and four half-width decks on a laptop
   * screen is worse than two readable ones — but the extra pair is a click
   * away rather than a rebuild away.
   */
  let deckCount = $state(2);

  // The engine only exists once a device is open, so every control that would
  // send an action stays disabled until then.
  //
  // Derived from the *engine's* sample rate rather than from this component's
  // record of having pressed Connect. The two are not the same: a device opened
  // by anything else — the benchmark harness, a restored setting, the assistant
  // — leaves `active` null while the engine is quite happily playing, and every
  // control on screen sits disabled next to a moving playhead. The engine
  // publishes a rate only once a device is open, so it is the honest source.
  const ready = $derived((snapshot?.master.sample_rate ?? 0) > 0);

  $effect(() => {
    const unlisten = onSnapshot((next) => {
      snapshot = next;
    });
    // Paint immediately rather than waiting for the engine to change something.
    // The stream only emits on change, so a quiet startup would otherwise leave
    // the interface blank.
    void getSnapshot().then((initial) => {
      snapshot ??= initial;
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  });

  $effect(() => {
    refreshDevices();
  });

  $effect(() => {
    void loadLayouts();
  });

  $effect(() => {
    void watershedShowing()
      .then((showing) => {
        living = showing;
      })
      .catch(() => {})
      .finally(() => {
        livingRestored = true;
      });
  });

  /*
    The living interface, per ADR-0009. Off by default while it is being built
    out: it currently says a subset of what the deck panels say, and a DJ should
    not have to choose between a pretty river and a usable mixer.
  */
  let living = $state(false);
  /** Set once the stored choice has been read, so restoring it is not saved back. */
  let livingRestored = $state(false);
  let world = $state<World>(emptyWorld());

  /*
    Pulled rather than pushed. The snapshot already streams at 60 Hz and a
    second stream alongside it would double the traffic to say the same thing
    twice, so the world is asked for only while it is being drawn — which, when
    the river is hidden, is never.

    Twenty times a second, not sixty: the world carries rates rather than
    positions, so the renderer interpolates the pulse between reads and a slower
    poll costs nothing visible. See `phaseAt` in world.ts.
  */
  $effect(() => {
    if (!living) return;
    let alive = true;
    const tick = async () => {
      while (alive) {
        try {
          world = await getWorld();
        } catch {
          // A world we could not read is not worth an error in a booth; the
          // last one stays on screen until the next read succeeds.
        }
        await new Promise((r) => setTimeout(r, 50));
      }
    };
    void tick();
    return () => {
      alive = false;
    };
  });

  $effect(() => {
    const showing = living;
    // Not while restoring, or start-up would write back the value it just read.
    if (!livingRestored) return;
    void setWatershed(showing).catch(() => {});
  });

  const rivers = $derived(
    world.entities.filter((e) => e.name === "deck.river" && e.index <= deckCount),
  );
  const mouthFor = (deck: number) =>
    world.entities.find((e) => e.name === "deck.mouth" && e.index === deck) ?? null;
  /** Which river holds the peripheral channel. At most one ever does. */
  const alarmingDeck = $derived(alarmDeck(world.alarm));
  const confluenceEntity = $derived(
    world.entities.find((e) => e.name === "mixer.confluence") ?? null,
  );
  /*
    Which two rivers meet, read from the crossfader assignments the same way the
    world does — with four decks the pair is the DJ's choice, and drawing the
    wrong pair's beating would be worse than drawing none.
  */
  const bankEntities = $derived.by((): [Entity | null, Entity | null] => {
    const side = (want: CrossfaderAssign) => {
      const deck = snapshot?.decks.find((d) => d.loaded && d.crossfader_assign === want);
      return deck ? (rivers.find((r) => r.index === deck.number) ?? null) : null;
    };
    return [side("left"), side("right")];
  });

  /*
    The tier, from measurement rather than from asking the platform what it can
    do — see `tierFor`. `slowFrames` is already the probe's verdict, so the
    demotion costs nothing new.
  */
  const reducedMotion = prefersStillness();
  const tier = $derived(tierFor(slowFrames, reducedMotion));

  $effect(() => {
    void hasBrandLogo().then((present) => {
      logo = present;
    });
  });

  async function refreshLogo() {
    logo = await hasBrandLogo();
    logoVersion += 1;
  }

  // Watch our own frame rate. On a machine where the webview has no accelerated
  // compositing the waveform drops to ~16 fps with nothing to indicate why --
  // see the benchmark in ADR-0004. Better to say so than to look broken.
  $effect(() => watchFrameRate((health) => {
    slowFrames = health.degraded ? health.fps : null;
  }));

  async function refreshDevices() {
    try {
      devices = await listDevices();
      selectedDevice ??= devices.find((d) => d.is_default)?.id ?? devices[0]?.id ?? null;
      error = null;
    } catch (e) {
      error = String(e);
    }
  }

  async function connect() {
    try {
      active = await openDevice(selectedDevice, selectedCueDevice, bufferFrames);
      error = null;
    } catch (e) {
      error = String(e);
      active = null;
    }
  }

  async function send(action: string) {
    try {
      await dispatch(action);
      error = null;
    } catch (e) {
      error = String(e);
    }
  }

  async function toggleLog() {
    showLog = !showLog;
    if (showLog) log = await sessionLog();
  }

  const load = $derived(snapshot?.master.cpu_load ?? 0);
  // Read outside the template so event handlers, which run later, do not have
  // to prove `snapshot` is still non-null.
  const cueSplit = $derived(snapshot?.master.cue_split ?? false);
  const reduction = $derived(snapshot?.master.limiter_reduction_db ?? 0);
  // Before a device is open there is no engine, so the parameter table still
  // holds its zeroed defaults. Reading that as "bypassed" would announce a
  // safety feature was off when in fact nothing is running at all — so the
  // idle case is its own state rather than being folded into the off one.
  const limiterOn = $derived(!ready || (snapshot?.master.limiter_enabled ?? true));
  const split = $derived(snapshot?.master.split_output ?? null);
  const quantizeOn = $derived(snapshot?.master.quantize ?? false);
</script>

<main>
  <header class="topbar">
    <!--
      The DJ's own logo, if they set one. A booth screen carrying someone
      else's product name all night is a small daily insult, so the
      application steps out of the way when asked.
    -->
    <h1 class:branded={logo}>
      {#if logo}
        <img src={logoUrl(logoVersion)} alt="" />
      {:else}
        djmanzo
      {/if}
    </h1>

    <div class="device">
      <select bind:value={selectedDevice} disabled={devices.length === 0}>
        {#each devices as device (device.id)}
          <option value={device.id}>
            {device.name}{device.is_default ? " (default)" : ""}
          </option>
        {/each}
      </select>

      <select bind:value={bufferFrames}>
        {#each [64, 128, 256, 512, 1024] as frames (frames)}
          <option value={frames}>{frames} frames</option>
        {/each}
      </select>

      <!--
        Only worth offering when there is somewhere to send it. On a card with
        four channels the cue already has a home and a second device would add
        latency and a resampler for nothing.
      -->
      {#if devices.length > 1}
        <select
          bind:value={selectedCueDevice}
          title="Send the headphone cue to a second sound card. Only needed when the main device has no spare channels."
        >
          <option value={null}>Cue: same device</option>
          {#each devices.filter((d) => d.id !== selectedDevice) as device (device.id)}
            <option value={device.id}>Cue: {device.name}</option>
          {/each}
        </select>
      {/if}

      <button class="primary" onclick={connect}>
        {active ? "Reconnect" : "Connect"}
      </button>
    </div>

    <div class="status mono">
      {#if active}
        <span>{active.sample_rate / 1000} kHz</span>
        <span>{active.latency_ms.toFixed(1)} ms</span>
        <span class:hot={load > 0.7}>CPU {(load * 100).toFixed(0)}%</span>
        {#if snapshot && snapshot.master.xruns > 0}
          <span class="xruns">{snapshot.master.xruns} xruns</span>
        {/if}
        <!--
          Two cards means two crystals, and this is the measured disagreement
          between them. Shown because it is otherwise completely invisible: a
          figure that settles is a healthy pair, and one that keeps climbing is
          a device misreporting its rate — which you would otherwise only find
          out when the headphones started clicking mid-set.
        -->
        {#if split}
          <span
            class="drift"
            class:xruns={!split.healthy}
            title="Clock difference between the two sound cards, corrected by resampling. {split.queue_ms.toFixed(1)} ms queued."
          >
            {split.drift_ppm >= 0 ? "+" : ""}{split.drift_ppm.toFixed(0)} ppm
          </span>
        {/if}
      {:else}
        <span class="idle">no device</span>
      {/if}
      <button
        class:active={panel === "browse"}
        onclick={() => (panel = panel === "browse" ? "none" : "browse")}
      >
        Browse
      </button>
      <button
        class:active={panel === "presets"}
        onclick={() => (panel = panel === "presets" ? "none" : "presets")}
      >
        Presets
      </button>
      <button
        class:active={panel === "assistant"}
        onclick={() => (panel = panel === "assistant" ? "none" : "assistant")}
      >
        Assistant
      </button>
      <button
        class:active={panel === "settings"}
        onclick={() => (panel = panel === "settings" ? "none" : "settings")}
      >
        Settings
      </button>
      <button
        class:on={living}
        onclick={() => (living = !living)}
        title={living
          ? "Hide the watershed"
          : "Show the decks as a watershed — flow, pulse, clarity and how long is left"}
      >
        Watershed
      </button>
      <button
        onclick={() => (deckCount = deckCount === 2 ? 4 : 2)}
        title="Show {deckCount === 2 ? 'four' : 'two'} decks. The engine runs four either way."
      >
        {deckCount} decks
      </button>
      <!--
        The layout picker. A layout is data — it can hide the FX rack, it
        cannot change what a control does — so choosing one is safe even when
        somebody else wrote it. See `dj_app::layout`.
      -->
      <select
        class="layout"
        aria-label="Layout"
        onchange={(event) => {
          const chosen = layouts.find((l) => l.name === event.currentTarget.value);
          if (chosen) applyLayout(chosen);
        }}
      >
        <option value="">Layout…</option>
        {#each layouts as option (option.name)}
          <option value={option.name} selected={layout?.name === option.name}>
            {option.name}
          </option>
        {/each}
      </select>
      <button onclick={toggleLog}>Log</button>
    </div>
  </header>

  {#if error}
    <p class="error">{error}</p>
  {/if}

  <!--
    A headphone device that would not open is not fatal — the master still
    runs — but it is silent unless said out loud, and the DJ would be reaching
    for a cue that is not there.
  -->
  {#if active?.cue_error}
    <p class="warning">
      The headphone device would not open ({active.cue_error}). Cueing has
      stayed on the main device.
    </p>
  {/if}

  {#if split && !split.healthy}
    <p class="warning">
      The headphone device has lost audio
      ({split.starved_frames > 0
        ? `${split.starved_frames.toFixed(0)} frames of silence`
        : `${split.dropped_samples.toFixed(0)} samples dropped`}). Try a larger
      buffer, or put the cue back on the main device.
    </p>
  {/if}

  {#if slowFrames !== null}
    <p class="warning">
      Interface running at {slowFrames.toFixed(0)} fps. This usually means the
      webview has no hardware acceleration — the audio engine is unaffected, but
      the waveform will not scroll smoothly.
    </p>
  {/if}

  <!--
    Decks and mixer sit in their own scrolling region so that opening the
    browser compresses them rather than being squeezed to nothing itself. This
    is the layout every DJ application converges on, for the reason it matters:
    you search for the next track while the current one is playing, so both
    have to be on screen at once.
  -->
  <div class="stage" class:shared={panel !== "none"}>
  {#if snapshot}
    <!--
      The watershed. Above the decks rather than replacing them: it answers
      "how is this going" at a glance, and the panels below answer "what
      exactly" — nature carries the gestalt, digits carry the precision, and
      removing either would be the fastest way to make this design fail in a
      real booth. See docs/VISUAL-LANGUAGE.md.
    -->
    {#if living && rivers.length > 0}
      <div class="watershed" class:seam={world.confluence === "Seam"}>
        {#if confluenceEntity && bankEntities.some((b) => b !== null)}
          <Confluence
            confluence={confluenceEntity}
            banks={bankEntities}
            beating={world.beating}
            seam={world.confluence === "Seam"}
            height={72}
            latencyMs={snapshot.master.output_latency_ms}
            {tier}
          />
        {/if}
        {#each rivers as river (river.index)}
          <River
            {river}
            mouth={mouthFor(river.index)}
            latencyMs={snapshot.master.output_latency_ms}
            {tier}
            alarming={alarmingDeck === river.index ||
              (alarmingDeck === null && world.alarm !== null)}
          />
        {/each}
      </div>
    {/if}

    <div class="decks" class:four={deckCount === 4}>
      {#each snapshot.decks.slice(0, deckCount) as deck (deck.number)}
        <Deck {deck} enabled={ready} cueAvailable={snapshot.master.cue_available} {layout} />
      {/each}
    </div>

    <section class="mixer">
      <label class="control">
        <span>Crossfader</span>
        <input
          type="range"
          min="-1"
          max="1"
          step="0.01"
          value={snapshot.master.crossfader}
          disabled={!ready}
          oninput={(e) => send(`crossfader ${e.currentTarget.value}`)}
        />
        <div class="ends"><span>1</span><span>2</span></div>
      </label>

      <label class="control">
        <span>Master <em class="mono">{snapshot.master.gain_db.toFixed(1)} dB</em></span>
        <input
          type="range"
          min="-24"
          max="6"
          step="0.5"
          value={snapshot.master.gain_db}
          disabled={!ready}
          oninput={(e) => send(`master gain ${e.currentTarget.value}`)}
        />
      </label>

      <div class="cue-section" class:unavailable={!snapshot.master.cue_available}>
        {#if snapshot.master.cue_available}
          <label class="control">
            <span>Headphones <em class="mono">{snapshot.master.cue_split ? "split" : "blend"}</em></span>
            <input
              type="range"
              min="0"
              max="1"
              step="0.01"
              value={snapshot.master.cue_mix}
              disabled={!ready || snapshot.master.cue_split}
              oninput={(e) => send(`cue mix ${e.currentTarget.value}`)}
            />
            <div class="ends"><span>cue</span><span>master</span></div>
          </label>
          <button
            class:active={snapshot.master.cue_split}
            disabled={!ready}
            onclick={() => send(`cue ${cueSplit ? "split_off" : "split_on"}`)}
            title="Cue in one ear, master in the other"
          >
            Split
          </button>
        {:else}
          <p class="no-cue">
            {#if ready}
              No headphone cue — this device has only two output channels.
              Cueing needs a four-channel interface.
            {:else}
              Connect a device to see whether it can carry a headphone cue.
            {/if}
          </p>
        {/if}
      </div>

      <!--
        Quantize is global rather than per-deck because it is a way of working,
        not a property of a track: a DJ who wants quantised jumps wants them on
        whichever deck they happen to be touching.
      -->
      <div class="quantize">
        <button
          class:active={snapshot.master.quantize}
          disabled={!ready}
          onclick={() => send(`quantize ${quantizeOn ? "off" : "on"}`)}
          title="Snap beat jumps to the grid"
        >
          Quantize
        </button>
      </div>

      <div class="output-strip">
      <div class="master-meters">
        <div class="meter">
          <div class="meter-fill" style:width="{Math.min(snapshot.master.peak_left, 1) * 100}%"></div>
        </div>
        <div class="meter">
          <div
            class="meter-fill"
            style:width="{Math.min(snapshot.master.peak_right, 1) * 100}%"
          ></div>
        </div>
      </div>

      <!--
        The master meters read post-limiter, so they physically cannot show
        over 0 dB. Without a reduction meter beside them there would be no way
        to tell a mix sitting neatly at the ceiling from one being crushed into
        it by 9 dB, because both look identical up there.
      -->
      <div class="limiter" class:bypassed={ready && !snapshot.master.limiter_enabled}>
        <!--
          Only "active" once there is an engine to be active *about*. A filled
          button that is also disabled fades to 40% along with its label, and on
          the light theme white-on-faded-teal is unreadable — so with no device
          this stays a plain disabled button and the text beside it carries the
          meaning.
        -->
        <button
          class="limiter-toggle"
          class:active={ready && limiterOn}
          disabled={!ready}
          onclick={() => send(`limiter ${limiterOn ? "off" : "on"}`)}
          title="Bypass only if something downstream is already limiting. Latency is unchanged either way."
        >
          Limiter
        </button>
        {#if !ready}
          <em class="mono limiter-idle">on at connect</em>
        {:else if snapshot.master.limiter_enabled}
          <div class="reduction" title="Gain reduction">
            <!-- Drawn right-to-left: reduction pulls *down* from the ceiling. -->
            <div
              class="reduction-fill"
              style:width="{Math.min(snapshot.master.limiter_reduction_db / 12, 1) * 100}%"
            ></div>
          </div>
          <em class="mono reduction-value" class:working={reduction >= 0.1}>
            {reduction < 0.1 ? "—" : `-${reduction.toFixed(1)} dB`}
          </em>
        {:else}
          <em class="mono bypass-note">bypassed</em>
        {/if}
      </div>
      </div>

      {#if snapshot.master.output_latency_ms > 0}
        <p class="latency-note">
          Output delayed {snapshot.master.output_latency_ms.toFixed(1)} ms by
          the limiter's look-ahead. The headphone cue is delayed to match, so
          beatmatching stays true.
          {#if split}
            The second sound card adds {split.queue_ms.toFixed(1)} ms more, and
            its clock is being corrected by {Math.abs(split.drift_ppm).toFixed(0)}
            ppm.
          {/if}
        </p>
      {/if}
    </section>
  {:else}
    <p class="waiting">Waiting for the engine…</p>
  {/if}
  </div>

  {#if panel !== "none"}
    <div class="panel">
      {#if panel === "browse"}
        <Browse enabled={ready} deckCount={deckCount} decks={snapshot?.decks ?? []} />
      {:else if panel === "presets"}
        <Presets enabled={ready} deckCount={2} />
      {:else if panel === "assistant"}
        <Assistant enabled={ready} />
      {:else}
        <Settings onLogoChange={refreshLogo} />
      {/if}
    </div>
  {/if}

  {#if showLog}
    <section class="log">
      <h2>Session log</h2>
      <p class="hint">
        Every action, in order, with its timestamp. This log is what makes a set
        replayable — see ADR-0003.
      </p>
      <pre class="mono">{log.length ? log.join("\n") : "(nothing yet)"}</pre>
    </section>
  {/if}
</main>

<style>
  main {
    display: flex;
    flex-direction: column;
    gap: 0.9rem;
    padding: 0.9rem;
    height: 100vh;
    overflow: hidden;
  }

  .topbar {
    display: flex;
    align-items: center;
    gap: 1rem;
    flex-wrap: wrap;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 0.7rem 0.9rem;
  }

  h1 {
    margin: 0;
    font-size: 1.1rem;
    letter-spacing: 0.02em;
    color: var(--accent);
    display: flex;
    align-items: center;
  }

  /* A logo is given room but never allowed to push the toolbar around. */
  h1.branded img {
    height: 28px;
    max-width: 200px;
    object-fit: contain;
    display: block;
  }

  .status button.active {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--on-accent);
  }

  /*
    A basis rather than a bare `flex: 1`, so that when the window is too narrow
    the toolbar *wraps* instead of crushing this group. With `min-width: 0` and
    no basis it would shrink towards nothing and everything would appear to
    fit — which is how the Connect button, the one control that matters before
    a device is open, ended up squeezed to zero width behind the status row.
  */
  .watershed {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    margin-bottom: 0.6rem;
  }

  /*
    Keys that do not mix get a seam down the middle of the confluence, rather
    than a colour saying so — hue already means key, and one man in twelve
    cannot read it anyway. Behaviour is the redundant channel.
  */
  .watershed.seam {
    border-left: 2px solid var(--warn, #d8a657);
    padding-left: 0.4rem;
  }

  .device {
    display: flex;
    gap: 0.4rem;
    flex: 1 1 26rem;
    min-width: 0;
  }

  /* The device names are the long ones, so they are what gives way. */
  .device select:first-child {
    flex: 1 1 8rem;
    min-width: 0;
  }

  /* Fixed-width things never shrink below what their text needs. */
  .device select:not(:first-child) {
    flex: 0 1 auto;
    min-width: 0;
  }

  .device .primary {
    flex: 0 0 auto;
  }

  .status {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 0.7rem;
    font-size: 0.85em;
    color: var(--text-dim);
  }

  .status .hot {
    color: var(--warn);
  }

  .status .xruns {
    color: var(--danger);
  }

  .status .idle {
    color: var(--warn);
  }

  .stage {
    display: flex;
    flex-direction: column;
    gap: 0.9rem;
    min-height: 0;
    /* Takes everything when alone; yields to the panel when one is open, and
       scrolls rather than clipping if the window is short. */
    flex: 1;
    overflow: auto;
  }

  .stage.shared {
    flex: 1 1 55%;
  }

  .panel {
    display: flex;
    flex-direction: column;
    min-height: 0;
    flex: 1 1 45%;
    /* A floor, so a short window still leaves the panel usable rather than
       collapsing it to a sliver. */
    min-height: 220px;
  }

  .decks {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.9rem;
  }

  /*
    Four decks wrap to two rows rather than four columns. A quarter-width deck
    cannot hold eight pads, a jump row and a loop row without them becoming
    unreadable, and a deck you cannot read at arm's length in a dark booth is
    not a deck.
  */
  .decks.four {
    grid-template-columns: repeat(2, 1fr);
    grid-auto-rows: min-content;
  }

  .mixer {
    display: grid;
    grid-template-columns: 2fr 1fr 1.2fr auto auto;
    gap: 1.2rem;
    align-items: center;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 0.9rem;
  }

  .control {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    font-size: 0.85em;
    color: var(--text-dim);
  }

  .control em {
    font-style: normal;
    color: var(--text);
  }

  .ends {
    display: flex;
    justify-content: space-between;
    font-size: 0.8em;
  }

  .cue-section {
    display: flex;
    align-items: end;
    gap: 0.5rem;
  }

  .cue-section.unavailable {
    align-items: center;
  }

  .no-cue {
    margin: 0;
    font-size: 0.75em;
    line-height: 1.4;
    color: var(--text-dim);
  }

  .quantize {
    display: flex;
    align-items: center;
  }

  .quantize button {
    font-size: 0.85em;
    padding: 0.3rem 0.6rem;
  }

  .output-strip {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    width: 140px;
  }

  .master-meters {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .limiter {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.75em;
  }

  .limiter-toggle {
    padding: 0.15rem 0.4rem;
    font-size: 0.95em;
  }

  .reduction {
    flex: 1;
    height: 6px;
    background: var(--panel-raised);
    border-radius: 3px;
    overflow: hidden;
    /*
      Reduction pulls the signal *down* from the ceiling, so the bar grows
      from the right — the same direction the gain is moving. A left-to-right
      bar would read as "more is better", which is exactly backwards here.
    */
    display: flex;
    justify-content: flex-end;
  }

  .reduction-fill {
    height: 100%;
    background: var(--warn);
    transition: width 80ms linear;
  }

  .reduction-value {
    font-style: normal;
    color: var(--text-dim);
    min-width: 4.2em;
    text-align: right;
  }

  .reduction-value.working {
    color: var(--warn);
  }

  .bypass-note {
    font-style: normal;
    color: var(--warn);
  }

  .limiter-idle {
    font-style: normal;
    color: var(--text-dim);
  }

  .drift {
    color: var(--text-dim);
  }

  .limiter.bypassed .limiter-toggle {
    border-color: var(--warn);
  }

  .latency-note {
    grid-column: 1 / -1;
    margin: 0;
    font-size: 0.72em;
    line-height: 1.4;
    color: var(--text-dim);
  }

  .meter {
    height: 6px;
    background: var(--panel-raised);
    border-radius: 3px;
    overflow: hidden;
  }

  .meter-fill {
    height: 100%;
    background: linear-gradient(90deg, var(--accent-2), var(--warn));
  }

  .log {
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 0.9rem;
    overflow: auto;
    flex: 1;
    min-height: 0;
  }

  .log h2 {
    margin: 0 0 0.3rem;
    font-size: 0.95rem;
  }

  .hint {
    margin: 0 0 0.6rem;
    color: var(--text-dim);
    font-size: 0.8em;
  }

  .log pre {
    margin: 0;
    font-size: 0.8em;
    white-space: pre-wrap;
    user-select: text;
    -webkit-user-select: text;
  }

  .error {
    margin: 0;
    padding: 0.6rem 0.9rem;
    background: color-mix(in srgb, var(--danger) 12%, var(--panel));
    border: 1px solid var(--danger);
    border-radius: 8px;
    color: var(--danger);
    font-size: 0.85em;
  }

  .waiting {
    color: var(--text-dim);
  }

  .warning {
    margin: 0;
    padding: 0.6rem 0.9rem;
    background: color-mix(in srgb, var(--warn) 12%, var(--panel));
    border: 1px solid var(--warn);
    border-radius: 8px;
    color: var(--warn);
    font-size: 0.85em;
  }
</style>
