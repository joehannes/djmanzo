<script lang="ts">
  import Assistant from "./Assistant.svelte";
  import Browse from "./Browse.svelte";
  import Deck from "./Deck.svelte";
  import Fx from "./Fx.svelte";
  import Presets from "./Presets.svelte";
  import Automix from "./Automix.svelte";
  import Mic from "./Mic.svelte";
  import Plugin from "./Plugin.svelte";
  import Sampler from "./Sampler.svelte";
  import Settings from "./Settings.svelte";
  import Shortcuts from "./Shortcuts.svelte";
  import { Keyboard } from "./keyboard.svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { watchFrameRate } from "./framerate";
  import {
    deviceMissing,
    deviceToOpen,
    readAudioPreference,
    writeAudioPreference,
  } from "./audiopref";
  import { publishAudio } from "./audiovars.svelte";
  import { fill } from "./meter";
  import {
    chooseLayout,
    chosenLayout,
    listLayouts,
    formatTime,
    setWatershed,
    watershedShowing,
    type Layout,
  } from "./api";
  import Watershed from "./Watershed.svelte";
  import {
    emptyWorld,
    getWorld,
    prefersStillness,
    tierFor,
    type World,
  } from "./world";
  import {
    dispatch,
    getSnapshot,
    hasBrandLogo,
    clearBrandLogo,
    listDevices,
    logoUrl,
    setBrandLogo,
    onSnapshot,
    activeDevice,
    openDevice,
    sessionLog,
    type ActiveDevice,
    type Device,
    type Snapshot,
  } from "./api";

  const remembered = readAudioPreference();

  let devices = $state<Device[]>([]);
  let selectedDevice = $state<string | null>(remembered.device);
  /**
   * A second sound card for the headphone cue. Null means "keep it on the main
   * device", which is always better when that device has the channels — two
   * cards means two clocks, and a resampler between them.
   */
  let selectedCueDevice = $state<string | null>(remembered.cue);
  let bufferFrames = $state(remembered.bufferFrames);
  /**
   * The sound card the DJ chose last time, when it is not here any more.
   *
   * Said out loud rather than silently falling back: "playing through the
   * laptop speakers because your interface is not plugged in" and "playing
   * through the laptop speakers because that is what you chose" look
   * identical, and only one of them is a surprise.
   */
  let missingDevice = $state<string | null>(null);
  /**
   * Whether the launch connection has been attempted.
   *
   * Once, not on every device-list refresh: a DJ who deliberately disconnected
   * should not be reconnected behind their back the next time the list is
   * polled.
   */
  let connectedOnce = false;
  let active = $state<ActiveDevice | null>(null);
  let error = $state<string | null>(null);
  let snapshot = $state<Snapshot | null>(null);
  let log = $state<string[]>([]);
  let showLog = $state(false);
  let slowFrames = $state<number | null>(null);
  /** Which side panel is open, if any. Only one at a time: the decks matter more. */
  let panel = $state<
    | "none"
    | "browse"
    | "assistant"
    | "presets"
    | "sampler"
    | "settings"
    | "keyboard"
  >("none");

  /**
   * The keyboard, listening.
   *
   * Attached to the window rather than to any element, in the capture phase,
   * so a button that has just been clicked does not swallow the space bar and
   * turn it into "press me again". Built once for the life of the window: the
   * shortcut sheet reads the same object the handler writes, and two copies
   * would drift the first time a user mapping loaded.
   */
  const keyboard = new Keyboard();

  $effect(() => {
    const detach = keyboard.attach(window);
    void keyboard.load().catch((why) => {
      // Not fatal. The keyboard does nothing and every other way in still
      // works, which is worth one line rather than a dialog on launch.
      console.warn("keyboard mapping:", why);
    });
    return () => {
      // Let go of anything held before detaching, or a censor held while the
      // window closes stays on in the engine with nothing left to switch it off.
      keyboard.releaseAll();
      detach();
    };
  });

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

  /**
   * Adopt a device somebody else opened.
   *
   * `ready` comes from the snapshot, which is the engine's own account of
   * itself; `active` is only ever set by this component's Connect button. When
   * the first disagrees with the second, the snapshot is right — so ask the
   * backend what is open rather than showing "no device" over playing audio.
   *
   * The condition is the disagreement itself, so this fires once and then stays
   * quiet, and it covers every way a device gets opened without this button:
   * the demo harness, a preset, a script, the assistant, a restored session.
   */
  $effect(() => {
    if (ready && active == null) {
      void activeDevice().then((device) => {
        active ??= device;
      });
    }
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
  /** Which backend ended up drawing, for the log. Never an input to anything. */
  let backend = $state("");
  /*
    WebGL is opt-in, and that is a measurement rather than a preference.

    Drawing the identical scene on the no-GPU floor, Canvas 2D held 12 fps and
    WebGL managed 8 — the opposite of what `renderbench.ts` predicted in
    isolation. The isolated test drew discs into a bare canvas; embedded in a
    real page on software GL, *compositing the GL surface into the document*
    costs more than the drawing saves, and a per-frame instance upload is not
    free either. The bench measures a renderer; this measures an application.

    So Canvas 2D is the default until a machine with a GPU says otherwise, which
    is the same posture ADR-0004 takes with its own open gate. `?accel` turns
    WebGL on to take that measurement.
  */
  const accelerate = new URLSearchParams(window.location.search).has("accel");
  /*
    Which two rivers meet, read from the crossfader assignments the same way the
    world does — with four decks the pair is the DJ's choice, and drawing the
    wrong pair's beating would be worse than drawing none.
  */
  /*
    The tier, from measurement rather than from asking the platform what it can
    do — see `tierFor`. `slowFrames` is already the probe's verdict, so the
    demotion costs nothing new.
  */
  /*
    The audio goes to CSS, not into the controls.

    One property write per snapshot, inherited by every control on screen, in
    place of rewriting a stroke on every path of every knob sixty times a
    second. See `audiovars.svelte.ts` for the measurement this follows from.
  */
  $effect(() => {
    publishAudio(snapshot?.context);
  });

  /**
   * The set recording, or nothing if no snapshot has arrived.
   *
   * Pulled out rather than read through `snapshot` at each use: the record
   * button's handler is a closure, and a closure cannot keep a narrowing on a
   * variable that is reassigned every frame.
   */
  const setRecording = $derived(snapshot?.master.recording ?? null);

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

  /**
   * The identity mark is also its control. This keeps a booth-critical action
   * in the one place a DJ naturally looks for it, rather than burying it in
   * Settings beside unrelated application preferences.
   */
  async function chooseLogo() {
    try {
      const path = await open({
        multiple: false,
        filters: [
          { name: "Images", extensions: ["png", "jpg", "jpeg", "gif", "webp", "svg"] },
        ],
      });
      if (typeof path !== "string") return;
      await setBrandLogo(path);
      await refreshLogo();
      error = null;
    } catch (e) {
      error = `Could not set logo: ${String(e)}`;
    }
  }

  async function resetLogo() {
    try {
      await clearBrandLogo();
      await refreshLogo();
      error = null;
    } catch (e) {
      error = `Could not reset logo: ${String(e)}`;
    }
  }

  // Watch our own frame rate. On a machine where the webview has no accelerated
  // compositing the waveform drops to ~16 fps with nothing to indicate why --
  // see the benchmark in ADR-0004. Better to say so than to look broken.
  //
  // The banner's *appearance* is edge-triggered, so a single hitch does not
  // flash it. The number in it is not: it comes from `onSample`, once a second,
  // for as long as the banner is up. Driving both from `onChange` meant the
  // figure was whatever it had been at the moment things first went bad and
  // never moved again — so an interface that recovered from 4 fps to 30 went on
  // claiming 4, and one that got worse went on claiming it was fine.
  $effect(() =>
    watchFrameRate(
      (health) => {
        slowFrames = health.degraded ? health.fps : null;
      },
      (health) => {
        if (slowFrames !== null) slowFrames = health.fps;
      },
    ),
  );

  async function refreshDevices() {
    try {
      devices = await listDevices();
      if (deviceMissing(remembered.device, devices)) {
        missingDevice = remembered.device;
      }
      selectedDevice = deviceToOpen(selectedDevice, devices);
      // A headphone device that has been unplugged would fail the open and
      // take the master down with it, so it is dropped rather than carried.
      if (deviceMissing(selectedCueDevice, devices)) selectedCueDevice = null;
      error = null;
    } catch (e) {
      error = String(e);
    }
  }

  async function connect() {
    try {
      active = await openDevice(selectedDevice, selectedCueDevice, bufferFrames);
      error = null;
      // Remembered only on success. Storing a device that failed to open would
      // make the failure permanent across restarts.
      writeAudioPreference({
        device: selectedDevice,
        cue: selectedCueDevice,
        bufferFrames,
      });
      missingDevice = null;
    } catch (e) {
      error = String(e);
      active = null;
    }
  }

  /**
   * Open the sound card on launch.
   *
   * A DJ opening djmanzo expects it to make sound. Waiting to be told to
   * connect meant loading a track and pressing play did nothing, with no
   * visible reason — the interface looks the same connected or not. Every
   * other DJ application opens the default output on launch.
   *
   * Guarded so it happens once: a later refresh of the device list must not
   * reconnect a device the DJ deliberately closed.
   */
  $effect(() => {
    if (connectedOnce || devices.length === 0 || active !== null) return;
    connectedOnce = true;
    void connect();
  });

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
    <div class="brand">
      <button
        class:branded={logo}
        class="brand-trigger"
        onclick={chooseLogo}
        title="Choose a booth logo (PNG, JPEG, GIF, WebP or SVG)"
        aria-label="Choose your DJ logo"
      >
        {#if logo}
          <img src={logoUrl(logoVersion)} alt="Your DJ logo" />
        {:else}
          <span class="brand-mark" aria-hidden="true">✦</span>
          <span>DJ MANZO</span>
        {/if}
      </button>
      {#if logo}
        <button class="brand-reset" onclick={resetLogo} title="Restore the DJ MANZO mark">
          Reset
        </button>
      {/if}
    </div>

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
      <!--
        The panel is for setting the sampler up — loading, modes, routing. The
        playing is done from the pads, which is why this is a thing you open
        rather than something taking room on a deck all night.
      -->
      <button
        class:active={panel === "sampler"}
        onclick={() => (panel = panel === "sampler" ? "none" : "sampler")}
      >
        Sampler
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
        class:active={panel === "keyboard"}
        onclick={() => (panel = panel === "keyboard" ? "none" : "keyboard")}
        title={keyboard.enabled
          ? "The keys and what they do"
          : "The keyboard is not listening"}
      >
        Keys{keyboard.enabled ? "" : " ·"}
      </button>
      <button
        class:on={living}
        onclick={() => (living = !living)}
        title={living
          ? `Hide the watershed${backend ? ` (drawing with ${backend})` : ""}`
          : "Show the decks as a watershed — flow, pulse, clarity and how long is left"}
      >
        Watershed
      </button>
      <!--
        Recording the set. Beside the panel toggles rather than inside one,
        because it is the control a DJ has to be able to find at the start of a
        night without hunting for it — and the one whose state has to be
        readable from across a booth once it is running.
      -->
      {#if setRecording}
        <button
          class="record"
          class:on={setRecording.active}
          disabled={!ready}
          onclick={() => send(setRecording.active ? "record off" : "record on")}
          title={setRecording.active
            ? "Stop recording and finish the file"
            : "Record the master to disk, beside the settings"}
        >
          {#if setRecording.active}
            ● {formatTime(setRecording.seconds)}
          {:else}
            REC
          {/if}
        </button>
        {#if setRecording.dropped > 0}
          <!--
            A gap in the file, said now rather than discovered on playback. The
            audio thread never waits for a disk, so this is the honest cost of
            that and not something to hide.
          -->
          <span
            class="warn-chip"
            title="The disk could not keep up, so the recording has a gap in it"
          >
            {setRecording.dropped} lost
          </span>
        {/if}
        {#if setRecording.failed}
          <span
            class="warn-chip"
            title="The recording stopped on its own — the disk is probably full"
          >
            write failed
          </span>
        {/if}
      {/if}
      <!--
        Two, four or six. The engine builds six whatever this says: an idle deck
        is a branch per block that returns immediately, so there is nothing to
        save by building fewer, and a count the engine and the interface could
        disagree about is worse than an unused deck.
      -->
      <button
        onclick={() => (deckCount = deckCount === 2 ? 4 : deckCount === 4 ? 6 : 2)}
        title="Show {deckCount === 2 ? 'four' : deckCount === 4 ? 'six' : 'two'} decks. The engine runs six either way."
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
    The sound card from last time, not here any more. Silently falling back
    would look identical to having chosen the laptop speakers on purpose, and
    only one of those is a surprise.
  -->
  {#if missingDevice}
    <p class="warning">
      The sound card you used last time is not here. Playing through
      <strong>{devices.find((d) => d.id === selectedDevice)?.name ?? "the default output"}</strong>
      instead — plug the other one in and press Reconnect.
      <button class="inline" onclick={() => (missingDevice = null)}>Dismiss</button>
    </p>
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
      <Watershed
        {world}
        {tier}
        decks={rivers.map((r) => r.index)}
        latencyMs={snapshot.master.output_latency_ms}
        {accelerate}
        ondriver={(what) => (backend = what)}
      />
    {/if}

    <div class="decks" class:four={deckCount === 4} class:six={deckCount === 6}>
      {#each snapshot.decks.slice(0, deckCount) as deck (deck.number)}
        <Deck
          {deck}
          sampler={snapshot.master.sampler}
          enabled={ready}
          cueAvailable={snapshot.master.cue_available}
          {layout}
        />
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
        The microphone sits in the mixer rather than behind a panel, because it
        is a channel strip: it has a level, a switch and a send, exactly like
        the decks either side of it. Putting it in a dialogue would make going
        on air a two-step operation.
      -->
      <Mic mic={snapshot.master.mic} enabled={ready} {send} />

      <!--
        Automix belongs in the mixer for the same reason: it moves the channel
        faders. It is not a library panel that happens to start tracks — it is
        a hand on the mixer that is not yours, and it goes where the other
        hands are.
      -->
      <Automix automix={snapshot.master.automix} enabled={ready} {send} />

      <!--
        The plugin insert sits on the master, between the effect rack and the
        limiter, so it belongs with the master controls rather than in a
        dialogue. See `dj_clap` for why its own window is not shown.
      -->
      <Plugin clap={snapshot.master.clap} enabled={ready} {send} />

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
          <div class="meter-fill" style:scale="{fill(snapshot.master.peak_left)} 1"></div>
        </div>
        <div class="meter">
          <div
            class="meter-fill"
            style:scale="{fill(snapshot.master.peak_right)} 1"
          ></div>
        </div>
      </div>

      <!--
        The master rack. Beside the meters rather than in the deck column,
        because what it acts on is the mix — and it stays here whichever deck
        the DJ is looking at.
      -->
      {#if layout?.fx ?? true}
        <div class="master-fx">
          <span class="label">Master FX</span>
          <Fx slots={snapshot.master.fx} enabled={ready} target="master" {send} />
        </div>
      {/if}

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
              style:scale="{fill(snapshot.master.limiter_reduction_db / 12)} 1"
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
      {:else if panel === "keyboard"}
        <Shortcuts {keyboard} onclose={() => (panel = "none")} />
      {:else if panel === "sampler"}
        {#if snapshot}
          <Sampler sampler={snapshot.master.sampler} enabled={ready} {send} />
        {/if}
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
  .master-fx {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    margin: 0.4rem 0;
  }

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

  .brand {
    display: flex;
    align-items: center;
    gap: 0.25rem;
  }

  /* A logo is given room but never allowed to push the toolbar around. */
  .brand-trigger {
    min-height: 2.35rem;
    margin: 0;
    padding: 0.3rem 0.5rem;
    border-color: transparent;
    background: transparent;
    color: var(--accent-2);
    font-size: 0.9rem;
    font-weight: 800;
    letter-spacing: 0.12em;
    white-space: nowrap;
  }

  .brand-trigger:hover:not(:disabled),
  .brand-trigger:focus-visible {
    background: color-mix(in srgb, var(--accent) 14%, transparent);
    border-color: color-mix(in srgb, var(--accent) 45%, transparent);
  }

  .brand-mark {
    display: inline-grid;
    place-items: center;
    width: 1.5rem;
    height: 1.5rem;
    margin-right: 0.3rem;
    border: 1px solid currentColor;
    border-radius: 50%;
    color: var(--warn);
    font-size: 0.8rem;
  }

  .brand-trigger.branded {
    padding: 0.2rem 0.35rem;
  }

  .brand-trigger.branded img {
    height: 28px;
    max-width: 200px;
    object-fit: contain;
    display: block;
  }

  .brand-reset {
    padding: 0.25rem 0.4rem;
    border-color: transparent;
    background: transparent;
    color: var(--text-dim);
    font-size: 0.7rem;
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

  /*
    Six decks are three rows of two, not two rows of three, for the same reason
    four are two rows of two: the width of a deck is what decides whether its
    pads are readable, and thirds of a screen are not enough. Three rows is
    taller than a screen, which is correct — six decks is a scrolling rig, and
    pretending otherwise would shrink all six to fit.
  */
  .decks.six {
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
    width: 100%;
    transform-origin: left center;
    height: 100%;
    background: var(--warn);
    transition: scale 80ms linear;
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
    width: 100%;
    transform-origin: left center;
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

  /* A dismiss that sits inside its own sentence rather than beside it. */
  .inline {
    margin-left: 0.4rem;
    padding: 0.05rem 0.4rem;
    font-size: 0.7rem;
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
