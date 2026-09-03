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
    layoutTree,
    listLayouts,
    formatTime,
    setWatershed,
    watershedShowing,
    cockpitSurfaces,
    cockpitWorkspace,
    densityBands,
    type DensityBand,
    setCockpitWorkspace,
    type Dock,
    type Layout,
    type Placed,
    type SurfacePlacement,
    type Workspace,
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
    noteAdd,
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

  /**
   * When the last mark was taken, for the button's brief acknowledgement.
   *
   * Something has to say it happened, or the DJ presses again and ends up with
   * three marks for one moment. A second and a half: long enough to read at a
   * glance across a booth, short enough that the button is ready again before
   * the next thing worth marking.
   */
  let markedAt = $state(0);
  const MARK_SHOWN_MS = 1500;

  async function mark() {
    try {
      await noteAdd();
      markedAt = Date.now();
      setTimeout(() => (markedAt = 0), MARK_SHOWN_MS);
    } catch (why) {
      // Loud, unlike most failures here: the DJ believes a moment was captured
      // and it was not, and they will not find out until they look for it
      // after the set, when the moment is gone.
      error = `Could not mark the moment: ${why}`;
    }
  }
  // Saving a set and comparing two takes. See the Session log panel below.
  let savePath = $state("");
  let saved = $state<SessionSummary | null>(null);
  let diffA = $state("");
  let diffB = $state("");
  let divergence = $state<DivergenceLine[] | null>(null);
  let logError = $state<string | null>(null);
  let slowFrames = $state<number | null>(null);
  /**
   * How the cockpit is arranged: which surfaces are open, and where.
   *
   * This replaced a single `panel` variable that held one of eight names, so
   * exactly one panel could be open. The audit's headline finding was that
   * the consequence of that one variable is that **a DJ cannot see the room
   * and the library at the same time** -- not because anybody decided it, but
   * because the shell was shaped that way years of features ago.
   *
   * The arrangement lives in Rust (`dj_app::cockpit`), is checked there
   * against what can actually be drawn, and is stored between sessions.
   */
  let workspace = $state<Workspace | null>(null);

  /**
   * What the resolver corrected or skipped.
   *
   * Same posture as the layout notes above: a workspace that half-loaded in
   * silence is worse than one that says which half.
   */
  let workspaceNotes = $state<string[]>([]);

  /**
   * The surfaces this build can actually draw, and what each is called.
   *
   * `cockpit_surfaces()` lists twenty; these eight are the ones that were
   * already top-level panels, so this migration adds docking without moving
   * any feature. The rest are components nested inside these -- the room
   * sensor inside the assistant, the journal inside the browser -- and
   * promoting them means removing them from their parent, which is its own
   * change rather than a side effect of this one.
   *
   * A stored workspace naming a surface not in this list is skipped with a
   * note. That is the same rule the widget registry follows, and it is what
   * lets a workspace written by a later djmanzo open on this one.
   */
  const DRAWN = [
    "library",
    "booth",
    "presets",
    "sampler",
    "assistant",
    "settings",
    "keys",
    "controllers",
    "log",
  ] as const;
  type Drawn = (typeof DRAWN)[number];

  /** The surfaces that are open, in dock order. */
  const placements = $derived(
    (workspace?.surfaces ?? []).filter((p) =>
      (DRAWN as readonly string[]).includes(p.surface),
    ),
  );

  const inDock = (dock: Dock) =>
    placements.filter((p) => p.dock === dock).sort((a, b) => a.order - b.order);

  const rightDock = $derived(inDock("right"));
  const bottomDock = $derived(inDock("bottom"));
  const leftDock = $derived(inDock("left"));
  /** True when any dock has something in it, so the stage yields room. */
  const docked = $derived(placements.length > 0);

  const isOpen = (name: Drawn) => placements.some((p) => p.surface === name);

  /**
   * Where a surface goes when it is opened by its toolbar button.
   *
   * From the surface's own preferred size rather than from a table here:
   * something wider than it is tall wants the bottom, and something taller
   * than it is wide wants the side. The library prefers 900x380 and lands
   * along the bottom; settings prefers 620x560 and lands beside the decks.
   * A rule beats a list of special cases, and this one is derived from a
   * number Rust already publishes.
   */
  const HOME: Record<Drawn, Dock> = {
    library: "bottom",
    booth: "bottom",
    log: "bottom",
    presets: "right",
    sampler: "right",
    assistant: "right",
    settings: "right",
    keys: "right",
    controllers: "right",
  };

  /**
   * Open or close a surface, and remember it.
   *
   * The write goes through Rust and the answer replaces the local state, so a
   * placement the resolver corrected is the one drawn. Storing the request and
   * drawing the answer is how the two drift apart.
   */
  async function toggleSurface(name: Drawn) {
    const current = workspace ?? {
      name: "Custom",
      about: "",
      surfaces: [],
      density: "standard" as const,
      focus: "performing" as const,
      theme: "",
      decks: deckCount,
      frozen: false,
    };
    const already = current.surfaces.some((p) => p.surface === name);
    const surfaces: SurfacePlacement[] = already
      ? current.surfaces.filter((p) => p.surface !== name)
      : [
          ...current.surfaces,
          {
            surface: name,
            dock: HOME[name],
            // Newest last within its dock, which is where the eye expects the
            // thing it just opened.
            order: current.surfaces.length,
            size: null,
            collapsed: false,
            pinned: false,
          },
        ];

    // Optimistic, then corrected. The panel appears on the press rather than
    // after a round trip to the filesystem, which at a laptop's worst moment
    // is not instant.
    workspace = { ...current, surfaces };
    if (name === "log" && !already) log = await sessionLog().catch(() => log);
    try {
      const resolved = await setCockpitWorkspace(workspace);
      workspace = resolved.workspace;
      workspaceNotes = resolved.notes;
    } catch {
      // Keeping the optimistic state: the DJ pressed a button and the panel
      // opened, and failing to write a preferences file is not a reason to
      // close it again under them.
    }
  }

  /**
   * How dense the interface is, when nobody has said.
   *
   * The central idea of the redesign, and the first place it does anything:
   * *the system adapts the presentation to the DJ, rather than forcing the DJ
   * to adapt to the application.* A window too short for the interface at its
   * normal size gets a denser one, in bands so it settles rather than sliding
   * about while a window edge is dragged.
   *
   * **An explicit density wins.** A layout or a workspace that names one is a
   * DJ who has decided, and adapting over a decision is the failure mode this
   * whole redesign is written to avoid. `chosen` below is that flag.
   */
  let bands = $state<DensityBand[]>([]);
  let chosenDensity = $state<number | null>(null);

  /**
   * The scale in force, as a number.
   *
   * Kept as well as set on the document because the waveform needs it as an
   * argument rather than as a style: its lane is drawn by Rust at a pixel
   * height, so scaling it in CSS would stretch tiles rendered for a different
   * size. Everything else on a deck reads `--density` off the document.
   */
  let density = $state(1);

  function fitDensity() {
    if (chosenDensity !== null || bands.length === 0) return;
    const height = window.innerHeight;
    const band = bands.find(([least]) => height >= least) ?? bands[bands.length - 1];
    density = band[2];
    document.documentElement.style.setProperty("--density", String(band[2]));
  }

  $effect(() => {
    void densityBands()
      .then((got) => {
        bands = got;
        fitDensity();
      })
      .catch(() => {});
    // Rare and user-driven, so unthrottled is honest: a DJ drags a window edge
    // a handful of times a night, and debouncing this would only delay the
    // moment the interface settles.
    window.addEventListener("resize", fitDensity);
    return () => window.removeEventListener("resize", fitDensity);
  });

  /** What a surface is called, from Rust rather than from a list here. */
  let surfaceTitles = $state<Record<string, string>>({});
  const titleOf = (name: string) => surfaceTitles[name] ?? name;

  async function loadWorkspace() {
    try {
      const known = await cockpitSurfaces();
      surfaceTitles = Object.fromEntries(known.map((s) => [s.name, s.title]));
    } catch {
      // A surface with no title falls back to its name, which is still a word
      // a DJ can read -- worse than "Session log", better than an empty header.
    }
    try {
      const resolved = await cockpitWorkspace();
      workspace = resolved.workspace;
      workspaceNotes = resolved.notes;
    } catch {
      workspace = null;
      workspaceNotes = [];
    }
  }

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
  /**
   * What the chosen layout asked for that djmanzo could not give it.
   *
   * ADR-0008's third rule is that an unknown widget, slot or token is skipped
   * rather than fatal, so a layout written for a newer djmanzo still opens.
   * That rule is only honest if the DJ can find out *which* parts were
   * skipped — a layout that half-loaded in silence is worse than one that
   * refused, because the missing half looks like a bug in the application.
   */
  let layoutNotes = $state<string[]>([]);

  async function loadLayouts() {
    try {
      layouts = await listLayouts();
      // Restore last night's choice. A DJ who set the interface up the way
      // they wanted should not have to do it again before every set.
      const previous = await chosenLayout();
      if (previous) applyLayout(previous, false);
      else void resolveLayout();
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
    // A layout that names a density is a DJ who has decided, so the
    // window-fitting above stands down rather than arguing with it.
    chosenDensity = next.density;
    density = next.density;
    document.documentElement.style.setProperty("--density", String(next.density));
    if (next.browser && !isOpen("library")) void toggleSurface("library");
    // Not when restoring, or every start-up would rewrite the file it just
    // read — harmless, but it makes the file's timestamp a lie about when the
    // DJ last chose anything.
    if (remember) void chooseLayout(next.name).catch(() => {});
    void resolveLayout();
  }

  /**
   * Read the chosen layout back as a checked widget tree.
   *
   * The tokens come from here rather than from the flat layout because this is
   * the path a DJ's own layout file will take: Rust owns the vocabulary,
   * validates every token against its declared shape, and hands back only
   * values that are safe to put on the document. The interface never decides
   * what a token may contain — see `dj_app::widgets::token`.
   */
  async function resolveLayout() {
    try {
      const tree = await layoutTree();
      for (const [name, value] of Object.entries(tree.tokens)) {
        document.documentElement.style.setProperty(`--${name}`, value);
      }
      layoutNotes = tree.notes;
      layoutSlots = tree.slots;
    } catch {
      // A layout that cannot be read leaves the interface as it is, which is
      // the same posture `loadLayouts` already takes.
      layoutNotes = [];
      layoutSlots = {};
    }
  }

  /**
   * The resolved tree, by slot.
   *
   * Empty until it arrives and empty again if it cannot be read, which every
   * reader below treats as "you decide" rather than as "draw nothing" -- see
   * `Deck.svelte`, which falls back to the full deck.
   */
  let layoutSlots = $state<Record<string, Placed[]>>({});

  /**
   * What one deck should draw, from the tree.
   *
   * Matched on the `number` prop rather than on position, because a layout may
   * place decks in any order or place only some of them, and a deck drawing
   * another deck's widget list is the kind of bug that looks like a rendering
   * glitch and is actually a layout being read wrong.
   */
  function deckZones(number: number): Placed[] | null {
    const stage = layoutSlots["stage"];
    if (!stage) return null;
    const deck = stage.find(
      (placed) => placed.widget === "deck" && placed.props.number === number,
    );
    return deck?.children?.deck ?? null;
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
    void getSnapshot()
      .then((initial) => {
        snapshot ??= initial;
      })
      // The one failure that leaves nothing on screen at all: the stream only
      // emits on change, so without this first read a quiet engine means a
      // blank interface with no explanation of why.
      .catch((problem) => {
        error = `the engine did not answer: ${problem}`;
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
      void activeDevice()
        .then((device) => {
          active ??= device;
        })
        // Left as "No device" rather than as an error line. This only fills in
        // a name for something that is demonstrably open -- `ready` is true --
        // so the worst case is a caption that says less than it could.
        .catch(() => {});
    }
  });

  $effect(() => {
    void loadLayouts();
  });

  $effect(() => {
    void loadWorkspace();
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
    void hasBrandLogo()
      .then((present) => {
        logo = present;
      })
      // Falls back to the wordmark, which is a complete interface. Not worth
      // an error line on startup.
      .catch(() => {});
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
      if (!isOpen("settings")) void toggleSurface("settings");
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
      {#if isOpen("settings")}
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
          onclick={() => toggleSurface("settings")}
        >
          {active ? active.name : "No device"}
        </button>
        <IconButton
          icon="fa-solid fa-hand-pointer"
          title="Map a controller"
          active={isOpen("controllers")}
          onClick={() => toggleSurface("controllers")}
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

      Naming them left the row twelve controls long, holding three unlike things
      at one weight: seven panels you open, three controls over what the stage
      shows, and two acts on the night that carry live state. `aria-label`
      said "Panels", which described the first seven and misdescribed the rest.
      So the row is three named groups, and the names are real -- a screen
      reader hears the same three groups the eye is being shown.

      Grouped, not hidden. The standing complaint about the products this
      competes with is menus you cannot find; every control that was one press
      away is still one press away, in the same reading order, and the
      watershed is named like its neighbours instead of remaining the last
      unlabelled square in the row this comment opens by complaining about.
    -->
    <div class="go">
      <nav class="go-group" aria-label="Panels">
        <IconButton icon="fa-solid fa-folder-open" label="Browse" title="Find and load tracks" active={isOpen("library")} onClick={() => toggleSurface("library")} />
        <IconButton icon="fa-solid fa-layer-group" label="Presets" title="Effect and mix presets" active={isOpen("presets")} onClick={() => toggleSurface("presets")} />
        <!--
          The booth: microphone, automix, a plugin insert and the master
          effects. These used to sit in a slab under the decks, always drawn
          and always taking room, which is what stopped a deck from ever being
          bounded -- and they are the things set up once a night rather than
          reached for during a mix, which is exactly what a dock is for.
        -->
        <IconButton icon="fa-solid fa-sliders" label="Booth" title="Microphone, automix, plugin insert and master effects" active={isOpen("booth")} onClick={() => toggleSurface("booth")} />
        <!--
          The panel is for setting the sampler up — loading, modes, routing. The
          playing is done from the pads, which is why this is a thing you open
          rather than something taking room on a deck all night.
        -->
        <IconButton icon="fa-solid fa-th" label="Sampler" title="Load and route the sample banks" active={isOpen("sampler")} onClick={() => toggleSurface("sampler")} />
        <IconButton icon="fa-solid fa-robot" label="Assistant" title="Ask for a next track, or a transition" active={isOpen("assistant")} onClick={() => toggleSurface("assistant")} />
        <IconButton icon="fa-solid fa-cog" label="Settings" title="Audio, sources, controllers, timecode" active={isOpen("settings")} onClick={() => toggleSurface("settings")} />
        <IconButton icon="fa-solid fa-keyboard" label="Keys" title={keyboard.enabled ? "Keyboard shortcuts — enabled" : "Keyboard shortcuts — disabled"} active={isOpen("keys")} onClick={() => toggleSurface("keys")} />
        <IconButton icon="fa-solid fa-file-lines" label="Log" title="What the session has done so far" active={isOpen("log")} onClick={() => toggleSurface("log")} />
      </nav>

      <!--
        What the stage shows. Not one of these opens anything: they change the
        picture already in front of you, which is a different promise from the
        panels beside them and the reason they are no longer mixed in with them.
      -->
      <div class="go-group" role="group" aria-label="Stage">
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
        <!--
          What the layout asked for and did not get. Skipping the unknown parts
          is the rule; saying nothing about it is not — a DJ whose layout half
          loaded should be able to see which half, and the title carries the
          whole list because the chip has room for a count and not for reasons.
        -->
        {#if layoutNotes.length > 0}
          <span class="warn-chip" title={layoutNotes.join("\n")}>
            {layoutNotes.length} not shown
          </span>
        {/if}
        <IconButton
          icon="fa-solid fa-water"
          label="Watershed"
          title={living
            ? `Hide the watershed${backend ? ` (drawing with ${backend})` : ""}`
            : "Show the watershed — the mix drawn as moving water"}
          active={living}
          onClick={() => (living = !living)}
        />
      </div>

      <!--
        The night itself, rather than the application.

        Recording and marking are the two controls a DJ has to be able to find
        at the start of a set without hunting, and the only two in this row
        whose state has to be readable from across a booth once they are
        running. They sit together, at the end, where nothing shifts under
        them: the groups before them can gain a panel or a layout without
        moving these two.
      -->
      <div class="go-group set" role="group" aria-label="This set">
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
              <span class="dot" aria-hidden="true">●</span>
              {formatTime(setRecording.seconds)}
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
            <!--
              A recording that has stopped writing. Louder than the gap above
              because a gap costs you a bar and this costs you the rest of the
              night.
            -->
            <span
              class="warn-chip bad"
              title="The recording stopped on its own — the disk is probably full"
            >
              write failed
            </span>
          {/if}
        {/if}
        <!--
          Marking a moment, beside REC for the same reason REC is here: it is a
          control that has to be findable without hunting, while both hands are
          busy and the music is playing.

          It takes the moment and nothing else — the time, and what is on the
          decks. Writing it up happens in the Journal afterwards, because a DJ
          who has just watched the floor empty has about ninety seconds of
          attention and composing a sentence loses the observation.
        -->
        <button
          class="mark"
          class:done={markedAt > 0}
          disabled={!ready}
          onclick={mark}
          title="Mark this moment — write it up in the Journal later"
        >
          {markedAt > 0 ? "Marked" : "Mark"}
        </button>
      </div>
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
  <!--
    One snippet per surface, then the docks that draw them.

    This is the shape the audit asked for. What was here was a single `panel`
    variable holding one of eight names and one `<div class="panel">` rendering
    whichever it held -- so opening the assistant closed the browser, and a DJ
    could not look at the room and the library at once. Nothing about that was
    a decision; it is what one variable does.
  -->
  {#snippet surfaceLibrary()}
    <Browse enabled={ready} deckCount={deckCount} decks={snapshot?.decks ?? []} />
  {/snippet}
  {#snippet surfaceBooth()}
    {#if snapshot}
      <div class="mixer">
      <!--
        What is left below: the things set up once a night rather than reached
        for during a mix. The microphone, the automix, a plugin insert and the
        master effects.
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
      </div>
    {/if}
  {/snippet}

  {#snippet surfacePresets()}
    <Presets enabled={ready} deckCount={2} />
  {/snippet}
  {#snippet surfaceAssistant()}
    <Assistant enabled={ready} />
  {/snippet}
  {#snippet surfaceKeys()}
    <Shortcuts {keyboard} onclose={() => toggleSurface("keys")} />
  {/snippet}
  {#snippet surfaceControllers()}
    <!-- Two panels in the space of one: what is connected, then what it does. -->
    <div class="stack">
      <Controllers mappings={controlMappings} />
      <MappingEditor mappings={controlMappings} />
    </div>
  {/snippet}
  {#snippet surfaceSampler()}
    {#if snapshot}
      <Sampler sampler={snapshot.master.sampler} enabled={ready} {send} />
    {/if}
  {/snippet}
  {#snippet surfaceSettings()}
    <Settings onLogoChange={refreshLogo} deviceChannels={active?.channels ?? null} />
  {/snippet}
  {#snippet surfaceLog()}
    <div class="log">
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
    </div>
  {/snippet}

  {#snippet surface(placement: SurfacePlacement)}
    <!--
      A titled, closable frame around every surface.
      
      The old single panel had no chrome at all, because there was only ever one
      of it and the toolbar button that opened it was the label. With two or
      three docked together, each needs to say what it is and offer the way out
      -- otherwise "close the assistant" means finding the right toolbar button
      again, which is a trip to the other end of the window mid-set.
    -->
    <section class="surface" data-surface={placement.surface}>
      <header class="surface-head">
        <h2>{titleOf(placement.surface)}</h2>
        <button
          class="shut"
          title="Close {titleOf(placement.surface)}"
          aria-label="Close {titleOf(placement.surface)}"
          onclick={() => toggleSurface(placement.surface as Drawn)}
        >&times;</button>
      </header>
      <div class="surface-body">
        {#if placement.surface === "library"}{@render surfaceLibrary()}
        {:else if placement.surface === "booth"}{@render surfaceBooth()}
        {:else if placement.surface === "presets"}{@render surfacePresets()}
        {:else if placement.surface === "assistant"}{@render surfaceAssistant()}
        {:else if placement.surface === "keys"}{@render surfaceKeys()}
        {:else if placement.surface === "controllers"}{@render surfaceControllers()}
        {:else if placement.surface === "sampler"}{@render surfaceSampler()}
        {:else if placement.surface === "settings"}{@render surfaceSettings()}
        {:else if placement.surface === "log"}{@render surfaceLog()}
        {/if}
      </div>
    </section>
  {/snippet}

  <div class="cockpit">
  {#if leftDock.length > 0}
    <div class="dock side left">
      {#each leftDock as placement (placement.surface)}
        {@render surface(placement)}
      {/each}
    </div>
  {/if}

  <div class="middle">
  <div class="stage" class:shared={docked}>
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
          zones={deckZones(deck.number)}
          {density}
          careful={conductCare}
        />
      {/each}
    </div>


  {:else}
    <p class="waiting">Waiting for the engine…</p>
  {/if}
  </div>

  <!--
    Outside the scrolling stage, and that is the whole point of moving it.

    It sat inside, under the decks, and it scrolled away with them: at
    djmanzo's own default window the decks are taller than the room the stage
    has, so the crossfader was 56 px past the bottom edge with nothing but a
    scroll to bring it back. A DJ mid-transition has one hand free and no time
    to scroll, which is the same failure this control has had three times now
    in three different forms.

    Nothing moved in the reading order -- deck, crossfader, deck, exactly where
    it was. It is simply no longer part of what scrolls, the way every DJ
    application treats its mixer. `flex: none` is what says so.
  -->
  {#if snapshot}
    <!--
    The crossfader, directly under the decks.

    It used to sit at the top of the strip below, which put it about 1,500 px
    down at djmanzo's own default window size -- two screens under the
    waveforms it is used against, on a machine where a DJ has one hand free
    and no time to scroll. It is the most-used control in the application and
    it was the least reachable one.

    Under the decks rather than between them: between is what hardware does,
    but a third column at the 900 px minimum width leaves three columns too
    narrow to aim at. Under keeps the eye's path -- deck, crossfader, deck --
    and survives the narrow case.
  -->
  <section class="bridge">
    <MasterMixer
      master={snapshot.master}
      {ready}
      {split}
      {cueSplit}
      {limiterOn}
      {send}
    />
  </section>
  {/if}

  {#if bottomDock.length > 0}
    <div class="dock bottom">
      {#each bottomDock as placement (placement.surface)}
        {@render surface(placement)}
      {/each}
    </div>
  {/if}
  </div>

  {#if rightDock.length > 0}
    <div class="dock side right">
      {#each rightDock as placement (placement.surface)}
        {@render surface(placement)}
      {/each}
    </div>
  {/if}
  </div>

  <!--
    What the arrangement asked for and did not get. Same posture as the layout
    notes in the bar above: skipping what cannot be drawn is the rule, saying
    nothing about it is not.
  -->
  {#if workspaceNotes.length > 0}
    <p class="warn-chip notes" title={workspaceNotes.join("\n")}>
      {workspaceNotes.length} of the saved arrangement could not be drawn
    </p>
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

  /*
    Same shape as REC beside it: both are controls a DJ reaches for mid-set
    without looking. Neither carries a rule of its own beyond its lit state —
    they inherit the button style, which is what makes them look like
    siblings.
  */
  .mark.done {
    border-color: var(--accent);
    color: var(--accent);
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
    /*
      Three times the gap inside a group, which is the whole of the grouping.

      A hairline rule between groups would read more strongly and it has to be
      an element in the flow, so when this row wraps -- and it wraps, on any
      laptop screen -- the rule lands at the start of a line as a stray mark
      belonging to nothing. Space is what wrapping is made of, so space is the
      separator that survives it.

      Measured at 840 px, which is narrower than the application's own default
      window: the row now breaks *at a group boundary* -- the seven panels hold
      one line and the stage and set groups drop to the next together -- because
      a group is one flex child and a flex child is not split. It used to break
      wherever the twelfth control happened to land.
    */
    gap: 1.3rem;
    flex-wrap: wrap;
    /* Sits under the readouts and above the decks, so the eye meets "how it is
       going" and then "where to go" in that order -- which is the order a DJ
       asks them in. */
    padding-top: 0.35rem;
  }

  .go-group {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    flex-wrap: wrap;
  }

  /*
    Recording, said as a state instead of as a reading.

    `class:on` has been on this button since the recorder shipped and matched
    no rule anywhere in the application, so a running recording looked exactly
    like a stopped one: the only difference was the text changing from "REC" to
    a timer, which is a thing you have to walk over and read, from the one
    control in the row whose entire job is to be legible from the other side of
    a booth.
  */
  .record.on {
    border-color: var(--danger);
    background: color-mix(in srgb, var(--danger) 20%, transparent);
    color: var(--danger);
    /* So the timer does not jitter its own width once a second. */
    font-variant-numeric: tabular-nums;
  }

  /*
    A booth tally light: slow, and on the dot rather than on the button, so the
    thing a DJ aims at never changes size or position while they are aiming at
    it. Colour carries the state on its own -- this only says "right now".
  */
  .record .dot {
    animation: rec-pulse 2s ease-in-out infinite;
  }

  @keyframes rec-pulse {
    50% {
      opacity: 0.2;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .record .dot {
      animation: none;
    }
  }

  /*
    A recording with a hole in it, and a recording that has stopped writing.
    Both of these were plain body text sitting beside the timer -- the one
    place in the interface where "the file you are trusting is damaged" was
    said in the same voice as everything else.
  */
  .warn-chip {
    padding: 0.2rem 0.45rem;
    border: 1px solid var(--warn);
    border-radius: var(--radius);
    background: color-mix(in srgb, var(--warn) 14%, transparent);
    color: var(--warn);
    font-size: 0.75rem;
    white-space: nowrap;
  }

  /* A gap costs you a bar; this costs you the rest of the night. */
  .warn-chip.bad {
    border-color: var(--danger);
    background: color-mix(in srgb, var(--danger) 16%, transparent);
    color: var(--danger);
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

  /*
    The cockpit: a middle column with docks around it.

    A row rather than the column this used to be. The old shell stacked one
    panel *under* the decks, which is why only one could be open -- two stacked
    panels on an 800 px window leave nothing for the thing they are both about.
    Side by side, the library can run along the bottom while the assistant
    stands beside the decks, which is the arrangement every DJ application
    converges on and the one the audit found djmanzo structurally could not
    reach.
  */
  .cockpit {
    display: flex;
    flex-direction: row;
    align-items: stretch;
    gap: 0.9rem;
    flex: 1;
    min-height: 0;
    min-width: 0;
  }

  .middle {
    display: flex;
    flex-direction: column;
    gap: 0.9rem;
    flex: 1;
    min-height: 0;
    min-width: 0;
  }

  /*
    A dock is a stack of surfaces, and it scrolls as one.

    `min-width: 0` and `min-height: 0` on every level of this, because a flex
    child's default `min-*: auto` refuses to shrink below its content -- which
    is exactly how the browser panel came to be clipped at djmanzo's own
    default window size, twice.
  */
  .dock {
    display: flex;
    gap: 0.6rem;
    min-height: 0;
    min-width: 0;
    overflow: auto;
  }

  .dock.side {
    flex-direction: column;
    /* A share of the width rather than a fixed one, with a floor: a side dock
       squeezed under about 320 px stops being a panel and becomes a column of
       ellipses. */
    flex: 0 1 clamp(320px, 30%, 520px);
  }

  .dock.bottom {
    flex-direction: row;
    flex-wrap: wrap;
    /* The same floor the old single panel had, and for the same reason: a
       short window should leave the panel usable rather than a sliver. */
    min-height: 220px;
    flex: 1 1 45%;
  }

  .dock.bottom > .surface {
    flex: 1 1 420px;
    min-width: 0;
  }

  .surface {
    display: flex;
    flex-direction: column;
    min-height: 0;
    min-width: 0;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 10px;
    overflow: hidden;
  }

  /*
    Every surface says what it is and how to close it.

    The single panel needed neither: there was one of it, and the toolbar
    button that opened it was the label. With two or three docked at once,
    a DJ closing the assistant should not have to find the right button at the
    other end of the window to do it.
  */
  .surface-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    padding: 0.35rem 0.6rem;
    border-bottom: 1px solid var(--border);
    background: var(--panel-hover);
    flex: none;
  }

  .surface-head h2 {
    margin: 0;
    font-size: 0.8em;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-dim);
  }

  .surface-head .shut {
    background: transparent;
    border: none;
    color: var(--text-dim);
    font-size: 1.1em;
    line-height: 1;
    padding: 0 0.3rem;
    cursor: pointer;
  }

  .surface-head .shut:hover {
    color: var(--text);
  }

  .surface-body {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
    min-width: 0;
    /*
      The ceiling that is actually enforced. `.stage` has scrolled since it was
      written; the old panel did not, so content running past the bottom of the
      window was cut off by `main`'s hidden overflow -- the last rows of every
      library table, and any control below them. About eighty pixels at
      djmanzo's own default size. Surfaces that size themselves correctly are
      unaffected; the ones that do not are reachable instead of invisible.
    */
    overflow: auto;
    padding: 0.6rem;
  }

  .warn-chip.notes {
    margin: 0.4rem 0 0;
    flex: none;
  }

  /*
    The stage does not scroll, and that is what lets a deck pin anything.

    It used to, which meant a deck's height was whatever its content came to
    and the bottom of it -- the channel strip, the cue, the crossfader
    assignment -- was simply below the fold with a scrollbar as the only way
    back. A child cannot pin itself inside a parent that grows to fit it.

    So the stage now hands its room to `.decks`, each deck is exactly as tall
    as it is given, and the scrolling moved *inside* the deck where it can
    leave the controls that matter behind. The booth panel that used to sit
    under the decks here -- microphone, automix, plugin, master effects -- is a
    dock surface now, which is what it always was: the things set up once a
    night rather than reached for during a mix.
  */
  .stage {
    display: flex;
    flex-direction: column;
    gap: 0.9rem;
    min-height: 0;
    flex: 1;
  }

  .stage .decks {
    flex: 1;
    min-height: 0;
  }

  .stage.shared {
    flex: 1 1 55%;
  }

  .panel {
    display: flex;
    flex-direction: column;
    flex: 1 1 45%;
    /* A floor, so a short window still leaves the panel usable rather than
       collapsing it to a sliver. */
    min-height: 220px;
    /*
      And a ceiling that is actually enforced. `.stage` above has scrolled
      since it was written; this did not, so a panel whose content ran past
      the bottom of the window was simply cut off by `main`'s hidden
      overflow — the last rows of every library table, and any control below
      them. At djmanzo's own default 1280x800 with the decks open that is
      about eighty pixels of panel that could not be reached at all.

      Panels that size themselves correctly are unaffected: their content
      fits, so no scrollbar appears. The ones that do not are now reachable
      instead of invisible.
    */
    overflow: auto;
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
    /*
      The grid takes the stage's room and hands it to the decks rather than
      growing to fit them.

      `minmax(0, 1fr)` rather than the default `auto`, and that is the whole
      trick: a grid row's default height is its content's, so a deck could
      never be shorter than what is inside it and could never pin anything --
      the first attempt at this set `max-height: 100%` on the deck and moved
      nothing, because 100% of an `auto` row is the content again. The `0`
      minimum is the part that matters; `min-content` is what `1fr` alone
      would floor at.
    */
    min-height: 0;
    grid-template-rows: minmax(0, 1fr);
    grid-auto-rows: minmax(0, 1fr);
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

  /*
    The crossfader's own row. Full width and centred, so it reads as the thing
    the two decks meet in rather than as the first cell of the strip below.
  */
  .bridge {
    /* Never scrolls out of reach. See the comment where it is rendered. */
    flex: none;
    /* No panel of its own: the master strip draws its own shell, and a border
       around a border is a box inside a box. */
    display: flex;
    justify-content: center;
  }

  /*
    The booth, now that it lives in a dock rather than under the decks.

    The panel and the border are gone because the surface frame around it
    draws both, and a box inside a box reads as two things rather than one.
    The columns wrap rather than being fixed at four: a bottom dock is as wide
    as the window and a side dock is 320 px, and four fixed columns in 320 px
    is four things too narrow to read.
  */
  .mixer {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
    gap: 1.2rem;
    align-items: start;
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

  .surface[data-surface="log"] h2 {
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
