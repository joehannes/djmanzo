<script lang="ts">
  import Assistant from "./Assistant.svelte";
  import Browse from "./Browse.svelte";
  import Deck from "./Deck.svelte";
  import Fx from "./Fx.svelte";
  import Presets from "./Presets.svelte";
  import Automix from "./Automix.svelte";
  import Mic from "./Mic.svelte";
  import MasterMixer from "./MasterMixer.svelte";
  import Plugin from "./Plugin.svelte";
  import Sampler from "./Sampler.svelte";
  import Settings from "./Settings.svelte";
  import Shortcuts from "./Shortcuts.svelte";
  import Controllers from "./Controllers.svelte";
  import MappingEditor from "./MappingEditor.svelte";
  import { Keyboard } from "./keyboard.svelte";
  import { controlMappings as listControlMappings } from "./api";
  import { open } from "@tauri-apps/plugin-dialog";
  import { watchFrameRate } from "./framerate";
  import {
    deviceMissing,
    deviceToOpen,
    readAudioPreference,
    writeAudioPreference,
  } from "./audiopref";
  import { publishAudio } from "./audiovars.svelte";
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
  import ThemeSwitcher from "./ThemeSwitcher.svelte";
  import IconButton from "./controls/IconButton.svelte";
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
    assistantConduct,
    sessionLog,
    sessionSave,
    sessionDiff,
    type DivergenceLine,
    type SessionSummary,
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

  /**
   * Whether a mistake right now is expensive, from the assistant's occasion.
   *
   * Read here and passed down rather than fetched by every deck: it changes
   * when the DJ changes the occasion, which is a handful of times a night, and
   * four decks polling for the same boolean would be four times the work for
   * one answer.
   *
   * Polled slowly on purpose. The alternative -- pushing it through the
   * snapshot -- would put a value that changes a few times a night into a
   * stream that fires sixty times a second.
   */
  let conductCare = $state(false);
  // Saving a set and comparing two takes. See the Session log panel below.
  let savePath = $state("");
  let saved = $state<SessionSummary | null>(null);
  let diffA = $state("");
  let diffB = $state("");
  let divergence = $state<DivergenceLine[] | null>(null);
  let logError = $state<string | null>(null);
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
    | "mapping"
  >("none");

  /**
   * Mappings the editor can start a draft from.
   *
   * Fetched once: the list only changes when a mapping is saved, and a DJ who
   * has just saved one is looking at their own work rather than at this list.
   */
  let controlMappings = $state<{ name: string }[]>([]);

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

  // The list the mapping editor can start a draft from. Fetched once: it only
  // changes when a mapping is saved, and a DJ who has just saved one is
  // looking at their own work rather than at this list.
  $effect(() => {
    void controlMappings_load();
  });

  /**
   * Keep the care level current.
   *
   * Every four seconds. The occasion changes a handful of times a night, so
   * this is already far faster than it needs to be -- and pushing it through
   * the snapshot instead would put a value that barely moves into a stream
   * that fires sixty times a second.
   */
  $effect(() => {
    const read = async () => {
      try {
        conductCare = (await assistantConduct()).mistakes_are_costly;
      } catch {
        // Not fatal, and deliberately silent: if the assistant cannot be
        // reached the controls simply stay ordinary presses, which is the
        // safe direction to fail in -- a hold that is missing is an
        // inconvenience, one that appears unexpectedly is a control that
        // looks broken.
        conductCare = false;
      }
    };
    void read();
    const timer = setInterval(() => void read(), 4000);
    return () => clearInterval(timer);
  });

  async function controlMappings_load() {
    try {
      controlMappings = await listControlMappings();
    } catch (why) {
      // Not fatal: the editor still works, it just cannot offer a starting
      // point. One line rather than a dialog on launch.
      console.warn("control mappings:", why);
    }
  }

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
      // Smart defaults: if no explicit selection, pick a sensible device.
      selectedDevice = deviceToOpen(selectedDevice, devices);
      if (!selectedDevice) {
        // Prefer a device marked default with at least 2 channels, else the highest-channel device.
        const prefer = devices.find((d) => d.is_default && d.channels >= 2) ?? devices.slice().sort((a, b) => b.channels - a.channels)[0];
        selectedDevice = prefer?.id ?? null;
      }
      // A headphone device that has been unplugged would fail the open and
      // take the master down with it, so it is dropped rather than carried.
      if (deviceMissing(selectedCueDevice, devices)) selectedCueDevice = null;
      // If no cue device chosen and there is a second device, choose one only
      // when the master device lacks enough channels (less than 4) to host
      // separate cue channels. Prefer a device with channels >= 2.
      if (!selectedCueDevice && devices.length > 1) {
        const master = devices.find((d) => d.id === selectedDevice);
        if (!master || master.channels < 4) {
          const candidate = devices.find((d) => d.id !== selectedDevice && d.channels >= 2) ?? devices.find((d) => d.id !== selectedDevice);
          selectedCueDevice = candidate?.id ?? null;
        }
      }
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
      // Show settings so the user can correct the audio configuration.
      panel = "settings";
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
    // Attempt an intelligent auto-connect once devices are known and we are
    // not already connected. This uses the smart defaults chosen in
    // `refreshDevices` and will surface the settings panel on failure.
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
  // Before a device is open there is no engine, so the parameter table still
  // holds its zeroed defaults. Reading that as "bypassed" would announce a
  // safety feature was off when in fact nothing is running at all — so the
  // idle case is its own state rather than being folded into the off one.
  const limiterOn = $derived(!ready || (snapshot?.master.limiter_enabled ?? true));
  const split = $derived(snapshot?.master.split_output ?? null);
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

    <div style="display:flex; gap:0.6rem; align-items:center;">
      <ThemeSwitcher />
    </div>

    <div class="device">
      {#if panel === "settings"}
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
      {:else}
        <!--
          The device's **name**, which was the one thing about it not shown
          anywhere. This said "48 kHz • 5.3 ms" -- the same two numbers already
          in the readouts three inches to the right -- while the question a DJ
          actually has when they glance up is "am I playing out of the
          interface or the laptop speakers?", which no part of the interface
          answered.

          The settings cog that sat beside it is gone: it opened the same panel
          as the labelled Settings button below, so there were two identical
          icons for one destination.
        -->
        <button
          class="device-brief"
          title={active ? `Playing out of ${active.name}. Press to change it.` : "Nothing is open. Press to choose a sound card."}
          onclick={() => (panel = "settings")}
        >
          {active ? active.name : "No device"}
        </button>
        <IconButton
          icon="fa-solid fa-hand-pointer"
          title="Map a controller"
          active={panel === "mapping"}
          onClick={() => (panel = panel === "mapping" ? "none" : "mapping")}
        />
      {/if}
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
    </div>

    <!--
      Where you go, as opposed to how it is going.

      This was one row with the readouts above, and every destination in it was
      an unlabelled grey square: Browse looked exactly like Presets looked
      exactly like the keyboard-shortcut reference. Finding the browser -- the
      most-used control in the application, and a DJ's very first action -- meant
      hovering each square in turn, which nobody does with a record running out.

      So the destinations are named, and they are their own row: a readout is
      something you glance at, a destination is something you press, and putting
      the two in one line meant neither read as what it was.
    -->
    <nav class="go" aria-label="Panels">
      <IconButton icon="fa-solid fa-folder-open" label="Browse" title="Find and load tracks" active={panel === "browse"} onClick={() => (panel = panel === "browse" ? "none" : "browse")} />
      <IconButton icon="fa-solid fa-layer-group" label="Presets" title="Effect and mix presets" active={panel === "presets"} onClick={() => (panel = panel === "presets" ? "none" : "presets")} />
      <!--
        The panel is for setting the sampler up — loading, modes, routing. The
        playing is done from the pads, which is why this is a thing you open
        rather than something taking room on a deck all night.
      -->
      <IconButton icon="fa-solid fa-th" label="Sampler" title="Load and route the sample banks" active={panel === "sampler"} onClick={() => (panel = panel === "sampler" ? "none" : "sampler")} />
      <IconButton icon="fa-solid fa-robot" label="Assistant" title="Ask for a next track, or a transition" active={panel === "assistant"} onClick={() => (panel = panel === "assistant" ? "none" : "assistant")} />
      <IconButton icon="fa-solid fa-cog" label="Settings" title="Audio, sources, controllers, timecode" active={panel === "settings"} onClick={() => (panel = panel === "settings" ? "none" : "settings")} />
      <IconButton icon="fa-solid fa-keyboard" label="Keys" title={keyboard.enabled ? "Keyboard shortcuts — enabled" : "Keyboard shortcuts — disabled"} active={panel === "keyboard"} onClick={() => (panel = panel === "keyboard" ? "none" : "keyboard")} />
      <IconButton icon="fa-solid fa-water" title={living ? `Hide the watershed${backend ? ` (drawing with ${backend})` : ""}` : "Show the watershed"} active={living} onClick={() => (living = !living)} />
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
      <IconButton icon="fa-solid fa-file-lines" label="Log" title="What the session has done so far" onClick={toggleLog} />
    </nav>
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
          stemSwap={snapshot.master.stem_swap}
          {deckCount}
          {layout}
          careful={conductCare}
        />
      {/each}
    </div>

    <section class="mixer">
      <MasterMixer
        master={snapshot.master}
        {ready}
        {split}
        {cueSplit}
        {limiterOn}
        {send}
      />

      <!--
        The microphone, automix, plugin insert and master FX keep their existing
        component implementations for now. The booth-critical master controls
        above are SVG-native so the workstation's primary spatial model stays
        inside the same vector surface as the decks and meters.
      -->
      <Mic mic={snapshot.master.mic} enabled={ready} {send} />
      <Automix automix={snapshot.master.automix} enabled={ready} {send} />
      <Plugin clap={snapshot.master.clap} enabled={ready} {send} />

      {#if layout?.fx ?? true}
        <div class="master-fx">
          <span class="label">Master FX</span>
          <Fx slots={snapshot.master.fx} enabled={ready} target="master" {send} />
        </div>
      {/if}

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
      {:else if panel === "mapping"}
        <div class="stack">
          <Controllers mappings={controlMappings} />
          <MappingEditor mappings={controlMappings} />
        </div>
      {:else if panel === "sampler"}
        {#if snapshot}
          <Sampler sampler={snapshot.master.sampler} enabled={ready} {send} />
        {/if}
      {:else}
        <Settings onLogoChange={refreshLogo} deviceChannels={active?.channels ?? null} />
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

      <!--
        Saving and comparing. The path is typed rather than picked from a
        dialog: this panel is a developer and practice tool, and a file picker
        here would be more ceremony than the thing is worth.
      -->
      <div class="log-tools">
        <label>
          Save as
          <input
            type="text"
            bind:value={savePath}
            placeholder="/home/you/sets/friday.djset"
            spellcheck="false"
          />
        </label>
        <button
          disabled={!savePath || log.length === 0}
          onclick={async () => {
            try {
              saved = await sessionSave(savePath);
              logError = null;
            } catch (e) {
              logError = String(e);
            }
          }}>Save</button
        >
      </div>

      {#if saved}
        <p class="hint">
          {saved.events} events over {saved.seconds.toFixed(0)}s, {saved.tracks}
          {saved.tracks === 1 ? "track" : "tracks"} → <span class="mono">{saved.path}</span>
        </p>
      {/if}

      <div class="log-tools">
        <label>
          Compare
          <input type="text" bind:value={diffA} placeholder="take one" spellcheck="false" />
        </label>
        <label>
          against
          <input type="text" bind:value={diffB} placeholder="take two" spellcheck="false" />
        </label>
        <button
          disabled={!diffA || !diffB}
          onclick={async () => {
            try {
              divergence = await sessionDiff(diffA, diffB);
              logError = null;
            } catch (e) {
              logError = String(e);
              divergence = null;
            }
          }}>Diff</button
        >
      </div>

      {#if logError}
        <p class="hint error">{logError}</p>
      {/if}

      {#if divergence}
        {#if divergence.length === 0}
          <p class="hint">The two takes are the same set, move for move.</p>
        {:else}
          <ul class="divergence">
            {#each divergence as line, i (line.kind + line.event + i)}
              <li class={line.kind}>
                <span class="mono">{line.event}</span>
                <span class="delta">
                  {#if line.kind === "drift"}
                    {line.seconds > 0 ? "+" : ""}{line.seconds.toFixed(2)}s
                  {:else if line.kind === "only_in_first"}
                    only in the first
                  {:else}
                    only in the second
                  {/if}
                </span>
              </li>
            {/each}
          </ul>
        {/if}
      {/if}
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

  /* The open device, as a control rather than a caption: it names what is
     playing and takes you to where that is changed. */
  .device-brief {
    background: transparent;
    border: 1px solid transparent;
    color: var(--text-dim);
    cursor: pointer;
    font-size: 0.9em;
    padding: 0.2rem 0.4rem;
    border-radius: var(--radius);
    max-width: 18rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .device-brief:hover:not(:disabled) {
    border-color: var(--border-strong);
    color: var(--text);
  }

  .go {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    flex-wrap: wrap;
    /* Sits under the readouts and above the decks, so the eye meets "how it is
       going" and then "where to go" in that order -- which is the order a DJ
       asks them in. */
    padding-top: 0.35rem;
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

  /* Two panels in the space of one: what is connected, then what it does.
     They scroll together, because the mapping editor alone is taller than the
     panel on a laptop screen. */
  .stack {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    min-height: 0;
    overflow-y: auto;
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

  .latency-note {
    grid-column: 1 / -1;
    margin: 0;
    font-size: 0.72em;
    line-height: 1.4;
    color: var(--text-dim);
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

  .log-tools {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
    margin-top: 0.4rem;
  }

  .log-tools label {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    font-size: 0.8rem;
  }

  .log-tools input {
    min-width: 14rem;
    font-family: inherit;
    font-size: 0.8rem;
  }

  .divergence {
    list-style: none;
    margin: 0.4rem 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    font-size: 0.8rem;
  }

  .divergence li {
    display: flex;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.1rem 0.3rem;
    border-left: 3px solid transparent;
  }

  /* Drift is the ordinary case and reads as neutral; a move present in only
     one take is the thing worth spotting. */
  .divergence li.drift {
    border-left-color: var(--accent, rgba(128, 128, 128, 0.5));
  }

  .divergence li.only_in_first,
  .divergence li.only_in_second {
    border-left-color: var(--warn, #d97706);
  }

  .delta {
    opacity: 0.75;
    white-space: nowrap;
  }
</style>
