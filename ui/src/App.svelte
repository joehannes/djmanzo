<script lang="ts">
  import Assistant from "./Assistant.svelte";
  import Guide from "./Guide.svelte";
  import DashboardView from "./Dashboard.svelte";
  import Icon from "./controls/Icon.svelte";
  import { Leader } from "./leader.svelte";
  import { chordRun } from "./platform";
  import { tick } from "svelte";
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
  import { Keyboard, typing } from "./keyboard.svelte";
  import {
    controlMappings as listControlMappings,
    type MappingInfo,
  } from "./api";
  import { open } from "@tauri-apps/plugin-dialog";
  import { watchFrameRate } from "./framerate";
  import {
    deviceMissing,
    deviceToOpen,
    readAudioPreference,
    writeAudioPreference,
  } from "./audiopref";
  import { loadRemembers } from "./remembers.svelte";
  import { watchHands } from "./hands.svelte";
  import { publishAudio } from "./audiovars.svelte";
  import {
    activities as loadActivities,
    activitySuggestion,
    forgetActivity,
    keepActivity,
    setActivityMode,
    leaderTree,
    uiDo,
    interfaceSettings,
    setToolbars,
    dashboard as fetchDashboard,
    usedTile,
    type InterfaceSettings,
    type Dashboard,
    type Activities,
    type ActivitySuggestion,
    chooseLayout,
    chosenLayout,
    layoutTree,
    listLayouts,
    formatTime,
    setWatershed,
    watershedShowing,
    cockpitSurfaces,
    onCockpit,
    cockpitWorkspace,
    forgetWorkspace,
    keepWorkspace,
    myWorkspaces,
    cockpitWorkspaces,
    densityBands,
    phasePriorities,
    themeChosen,
    applyPreset,
    type DensityBand,
    setCockpitWorkspace,
    liveEvent,
    setLiveEvent,
    welcomeState,
    type WelcomeAnswers,
    type WelcomeApplied,
    type Dock,
    type DockSizes,
    type Layout,
    type Placed,
    type SurfacePlacement,
    type Permits,
    type Workspace,
  } from "./api";
  import Next from "./Next.svelte";
  import Pair from "./Pair.svelte";
  import MissionBar from "./MissionBar.svelte";
  import Whisper from "./Whisper.svelte";
  import Guides from "./Guides.svelte";
  import Night from "./Night.svelte";
  import RoomSense from "./RoomSense.svelte";
  import Mixes from "./Mixes.svelte";
  import Practice from "./Practice.svelte";
  import EventPanel from "./Event.svelte";
  import Tonight from "./Tonight.svelte";
  import Welcome from "./Welcome.svelte";
  import AtHand from "./AtHand.svelte";
  import Staged from "./Staged.svelte";
  import Palette from "./Palette.svelte";
  import Plan from "./Plan.svelte";
  import SideView from "./SideView.svelte";
  import Watershed from "./Watershed.svelte";
  import ActivityStrip from "./ActivityStrip.svelte";
  import Requests from "./Requests.svelte";
  import Singers from "./Singers.svelte";
  import { findInCollection } from "./find.svelte";
  import ThemeSwitcher from "./ThemeSwitcher.svelte";
  import { theme } from "./theme.svelte";
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
    themeNow,
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
   * §78 and §79: what djmanzo may still change about itself.
   *
   * Rust's answer, not this file's. The workspace stores *locks* — six of them,
   * which is §79's list — and `cockpit::Lock::stops` says what each takes away;
   * working that out here would be the same judgement written a second time, in
   * the one place it cannot be tested.
   *
   * Everything permitted until Rust says otherwise, which is what a fresh
   * install is and what every automatic path below falls back to when the
   * resolver cannot be reached. The safe default for a *lock* is the permissive
   * one: a DJ who has locked nothing and finds the interface refusing to adapt
   * has no way to work out why.
   */
  let permits = $state<Permits>({
    rearrange: true,
    resize: true,
    retheme: true,
    restyle: true,
  });

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
    "prepare",
    "next",
    "plan",
    "pair",
    "practice",
    "event",
    "night",
    "room",
    "booth",
    "presets",
    "sampler",
    "assistant",
    "settings",
    "keys",
    "controllers",
    "log",
    "mixes",
    "athand",
    "requests",
    "karaoke",
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
  /** Panels a workspace put over the decks: drawn lifted, closed on dismissal. */
  const overlayDock = $derived(inDock("overlay"));
  /** True when any dock has something in it, so the stage yields room. */
  const docked = $derived(placements.length > 0);

  const isOpen = (name: Drawn) => placements.some((p) => p.surface === name);

  /**
   * Where a surface lands when you open it.
   *
   * **From Rust, not from a table here.** This was a `Record<Drawn, Dock>` in
   * this file, and the moment §41 let the assistant open a panel there were
   * two answers to "where does this go" — with the assistant's version
   * occasionally naming a dock the surface is not allowed in, which the
   * resolver silently dropped. `cockpit::Surface::home` is the single answer
   * now, and a test asserts every home is a dock that surface can be placed in.
   */
  let surfaceHomes = $state<Record<string, Dock>>({});
  const homeOf = (name: string): Dock => surfaceHomes[name] ?? "right";

  /**
   * The arrangements that ship, for the picker.
   *
   * §7 asks for twenty-four named starting points; `dj_app::cockpit` holds
   * twenty-three of them (there is no visual surface to build the twenty-fourth
   * out of) and this only draws the list. The names, the descriptions and what
   * each one opens are Rust's — the same split as the layouts beside them.
   */
  let presets = $state<Workspace[]>([]);

  /**
   * §7 and §103's *modularity*: the arrangements this DJ has kept.
   *
   * Held apart from the shipped list rather than merged into it, because the
   * picker says which is which. A DJ looking for the layout they built for
   * their Saturday residency should not have to find it among twenty-three they
   * have never opened.
   */
  let mine = $state<Workspace[]>([]);
  /**
   * The picker's one entry that is not an arrangement.
   *
   * A sentinel rather than an empty value or a control character: the value has
   * to be something no arrangement can be called, and "no arrangement is called
   * this" is a claim about a name a DJ types, so it may as well be readable in
   * the markup and in a test.
   */
  const KEEP = "__keep__";
  /** The name being typed, or `null` when nobody is naming anything. */
  let naming = $state<string | null>(null);
  let namingError = $state<string | null>(null);

  /**
   * Apply a preset, and then forget it is one.
   *
   * §7: *make these starting points, not rigid identities. Every preset should
   * remain editable.* So this writes the preset as the workspace and stops
   * there — the next panel the DJ opens goes through `toggleSurface` into the
   * same workspace, under the same name, with no preset to be "off" of.
   *
   * The density and the theme are applied here rather than left in the stored
   * workspace, which is where they used to sit and do nothing: the whole
   * difference between "Laptop Compact" and "Minimal" is the density, and a
   * picker where those two do the same thing is a picker nobody presses twice.
   */
  /**
   * §16's chosen knowledge pack, as the shell's copy of what Rust holds.
   *
   * Here rather than in the panel that sets it because two surfaces need it and
   * neither owns it: Settings is where a DJ presses it, and the Next rail is
   * where it changes what they are offered. The rail asks Rust once and then
   * only when the record it follows changes, so without a value it can watch,
   * pressing *Latin* leaves a ranking made for another night on screen until a
   * deck happens to move.
   *
   * Empty until Settings has been opened at least once, and that is honest
   * rather than a gap: the rail's first question already carried whatever Rust
   * had stored, and what this exists to catch is the pack *changing* under a
   * rail that has already asked.
   */
  let chosenPack = $state("");

  /**
   * §54's half of a functional preset that the shell owns.
   *
   * The arrangement and the theme, and nothing else: Rust has already set the
   * waveform layers, the pad pages, the assistant's posture and the request
   * page, which are its. This is here rather than in Settings for the reason
   * the locks are — a workspace is resolved against a window only the shell has
   * measured, and two components writing one from their own copies is how one
   * silently drops what the other just saved.
   *
   * An arrangement the DJ has since edited is *theirs*: `presets` is the
   * shipped list, and applying by name gets whatever that name means now,
   * which is §7's rule that a preset is a starting point rather than a copy.
   */
  function setUpForTonight(name: string, pack: string) {
    const preset = presets.find((p) => p.name === name);
    if (preset) void applyWorkspace(preset);
    // Both halves, for the reason `applyWorkspace` gives at length: painting
    // the colours and telling djmanzo a choice was made are two different
    // things, and §31 undoes the first within four seconds without the second.
    if (pack) {
      theme.setPackage(pack);
      void themeChosen(pack).catch(() => {});
    }
  }

  /**
   * Change one thing about one placement, and keep it.
   *
   * §3's list of what a surface must be able to do is eleven verbs, and three
   * of them — **resized**, **collapsed/expanded** and **pinned** — were fields
   * on `Placement` that Rust stored, serialised and resolved, and that nothing
   * on this side ever read. The fifth table in this codebase found in that
   * state. A DJ could collapse nothing, resize nothing and pin nothing, and
   * the workspace file faithfully recorded all three.
   *
   * Through Rust and back, like every other write here: the resolver may
   * correct a placement, and drawing the request while storing the answer is
   * how the two drift.
   */
  async function setPlacement(name: string, change: Partial<SurfacePlacement>) {
    if (!workspace) return;
    const next = {
      ...workspace,
      surfaces: workspace.surfaces.map((p) =>
        p.surface === name ? { ...p, ...change } : p,
      ),
    };
    workspace = next;
    try {
      const resolved = await setCockpitWorkspace(next);
      workspace = resolved.workspace;
      workspaceNotes = resolved.notes;
      permits = resolved.permits;
    } catch {
      // Keeping the optimistic state, for the reason `toggleSurface` gives.
    }
  }

  /**
   * The size a placement asks for, as a style on its own axis.
   *
   * **The axis is the one the dock stacks along**, which is `Placement::size`'s
   * own words and is the only axis a single surface can vary: a side dock is a
   * column, so its surfaces share one width and differ in height; the bottom
   * dock is a row, so they share one height and differ in width. Setting the
   * other axis makes a panel wider than the dock holding it, which is what the
   * first version of this did — the surface grew and the dock did not, so it
   * simply overflowed. A browser test measuring the surface passed; driving it
   * showed the panel had not moved.
   *
   * `null` is "no opinion" and takes the surface's own preference, which is
   * what every placement said before anything read this.
   */
  function sizeStyle(placement: SurfacePlacement): string {
    if (placement.size == null) return "";
    return placement.dock === "bottom"
      ? `width: ${placement.size}px; flex: none;`
      : `height: ${placement.size}px; flex: none;`;
  }

  /** Which way a dock stacks, which is the axis a surface can vary along. */
  const alongY = (placement: SurfacePlacement) => placement.dock !== "bottom";

  /** Each open surface's own element, so a drag can measure and set it. */
  let boxes = $state<Record<string, HTMLElement | undefined>>({});

  /** Where a drag on a surface's edge is now, in pixels along its own axis. */
  let dragging = $state<{ surface: string; from: number; was: number } | null>(
    null,
  );

  function startResize(event: PointerEvent, placement: SurfacePlacement, box: HTMLElement) {
    const along = alongY(placement) ? box.offsetHeight : box.offsetWidth;
    dragging = {
      surface: placement.surface,
      from: alongY(placement) ? event.clientY : event.clientX,
      was: along,
    };
    // Capture so a fast drag that leaves the six-pixel handle keeps going.
    // Wrapped because a pointer that is no longer down throws here, and a
    // resize that fails is not a reason to stop the interface.
    try {
      (event.target as HTMLElement).setPointerCapture(event.pointerId);
    } catch {
      // Without capture the drag still works while the pointer is over the
      // handle, which is the common case.
    }
    event.preventDefault();
  }

  function onResize(event: PointerEvent, placement: SurfacePlacement, box: HTMLElement) {
    if (dragging?.surface !== placement.surface) return;
    // The handle is on the surface's trailing edge along the stacking axis —
    // the bottom of a panel in a side dock, the right of one along the bottom
    // — so dragging away from the surface always grows it. One rule for all
    // three docks, rather than three that have to be got the right way round.
    const now = alongY(placement) ? event.clientY : event.clientX;
    const wanted = Math.round(dragging.was + (now - dragging.from));
    box.style.setProperty("flex", "none");
    box.style.setProperty(
      alongY(placement) ? "height" : "width",
      `${Math.max(MIN_SURFACE, wanted)}px`,
    );
  }

  function endResize(event: PointerEvent, placement: SurfacePlacement, box: HTMLElement) {
    if (dragging?.surface !== placement.surface) return;
    dragging = null;
    const along = alongY(placement) ? box.offsetHeight : box.offsetWidth;
    try {
      (event.target as HTMLElement).releasePointerCapture(event.pointerId);
    } catch {
      // Never captured, or already released. Either way there is nothing to do.
    }
    void setPlacement(placement.surface, { size: Math.round(along) });
  }

  /**
   * The smallest a surface may be dragged to.
   *
   * A panel narrower than this is one whose own header does not fit, and a DJ
   * who drags it there has lost the handle to drag it back.
   */
  const MIN_SURFACE = 160;

  /**
   * §118b: the welcome, open when this holds its answers.
   *
   * It opens by itself only on a first run -- when djmanzo has never kept a
   * `welcome.json` -- and from Settings after that. The DJ's name, once
   * given, stands where the wordmark was.
   */
  let welcoming = $state<WelcomeAnswers | null>(null);
  let djName = $state("");

  $effect(() => {
    welcomeState()
      .then((found) => {
        djName = found.answers.name;
        if (!found.seen) welcoming = found.answers;
      })
      .catch(() => {});
  });

  async function openWelcome() {
    try {
      welcoming = (await welcomeState()).answers;
    } catch {
      // No settings folder: nothing to welcome into.
    }
  }

  function welcomed(applied: WelcomeApplied) {
    welcoming = null;
    djName = applied.answers.name;
    activityState = applied.activities;
    if (applied.workspace || applied.theme) setUpForTonight(applied.workspace, applied.theme);
  }

  /**
   * §118: the event being played, if one is.
   *
   * Kept by Rust beside the events, so a restart in the middle of the night
   * comes back to it rather than to an evening with nothing prepared.
   */
  let liveId = $state<string | null>(null);
  /** §118a: whether the night's quick decision is open, which the guides can ask for. */
  let tonightDeciding = $state(false);

  $effect(() => {
    liveEvent()
      .then((id) => (liveId = id))
      .catch(() => {});
  });

  /**
   * Play a prepared event: the narrow focus the owner asked for -- the decks
   * and the mixer (the Mix activity), with the night's line in the top bar.
   */
  async function goLive(id: string) {
    try {
      liveId = await setLiveEvent(id);
    } catch {
      return;
    }
    await chooseActivity("mix");
  }

  async function endNight() {
    try {
      liveId = await setLiveEvent(null);
    } catch {
      liveId = null;
    }
  }

  /**
   * §120: a dock's own size, dragged from the edge that faces the stage.
   *
   * A panel could be resized along its dock, but the dock was a share of the
   * window -- a side dock 30 % of it, between 320 and 520 px -- and the DJ
   * had no say. The grip is the gap between the dock and the stage: dragging
   * towards the stage grows the dock, a double-click gives it back its own
   * share, and the size is the workspace's (`cockpit::DockSizes`), bounded by
   * Rust. The bounds are Rust's too, repeated here only so the live drag
   * stops where the saved size will; a Rust test holds the two spellings the
   * same.
   */
  const DOCK_SIDE = [280, 960];
  const DOCK_BOTTOM = [140, 720];

  type DockName = "left" | "right" | "bottom";

  let dockEls = $state<Record<DockName, HTMLElement | undefined>>({
    left: undefined,
    right: undefined,
    bottom: undefined,
  });
  let dockDrag = $state<{ dock: DockName; from: number; was: number } | null>(null);

  /**
   * The dock's size as a style, when the DJ has set one -- and not while
   * every panel in it is folded, because a folded dock gives its room up and
   * a stored size would hold it open as a blank.
   */
  function dockStyle(dock: DockName, placed: SurfacePlacement[]): string {
    const size = workspace?.docks?.[dock];
    if (size == null) return "";
    if (placed.every((p) => p.collapsed)) return "";
    return dock === "bottom" ? `flex: 0 0 ${size}px; min-height: 0;` : `flex: 0 0 ${size}px;`;
  }

  async function setDocks(docks: DockSizes) {
    if (!workspace) return;
    const next = { ...workspace, docks };
    workspace = next;
    try {
      const resolved = await setCockpitWorkspace(next);
      workspace = resolved.workspace;
      workspaceNotes = resolved.notes;
      permits = resolved.permits;
    } catch {
      // Kept as drawn, for the reason `toggleSurface` gives.
    }
  }

  function startDockResize(event: PointerEvent, dock: DockName) {
    const el = dockEls[dock];
    if (!el) return;
    dockDrag = {
      dock,
      from: dock === "bottom" ? event.clientY : event.clientX,
      was: dock === "bottom" ? el.offsetHeight : el.offsetWidth,
    };
    try {
      (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    } catch {
      // Without capture the drag still works while over the grip.
    }
    event.preventDefault();
  }

  function onDockResize(event: PointerEvent, dock: DockName) {
    if (dockDrag?.dock !== dock) return;
    const el = dockEls[dock];
    if (!el) return;
    const now = dock === "bottom" ? event.clientY : event.clientX;
    // Towards the stage grows the dock: right for the left one, left for the
    // right one, up for the bottom one.
    const moved = dock === "left" ? now - dockDrag.from : dockDrag.from - now;
    const [least, most] = dock === "bottom" ? DOCK_BOTTOM : DOCK_SIDE;
    const size = Math.max(least, Math.min(most, Math.round(dockDrag.was + moved)));
    el.style.setProperty("flex", `0 0 ${size}px`);
    if (dock === "bottom") el.style.setProperty("min-height", "0");
  }

  function endDockResize(event: PointerEvent, dock: DockName) {
    if (dockDrag?.dock !== dock) return;
    dockDrag = null;
    try {
      (event.currentTarget as HTMLElement).releasePointerCapture(event.pointerId);
    } catch {
      // Never captured, or already released.
    }
    const el = dockEls[dock];
    if (!el) return;
    const size = Math.round(dock === "bottom" ? el.offsetHeight : el.offsetWidth);
    void setDocks({ ...(workspace?.docks ?? {}), [dock]: size });
  }

  function resetDock(dock: DockName) {
    const el = dockEls[dock];
    el?.style.removeProperty("flex");
    el?.style.removeProperty("min-height");
    void setDocks({ ...(workspace?.docks ?? {}), [dock]: null });
  }

  /**
   * §3's *temporarily surfaced*, and §120's temporary windows: *"temporary
   * windows can take a lot of space for the focused moment and then get out
   * of the way"*.
   *
   * Any open panel can be lifted over the decks -- the whole of the stage, for
   * as long as it is wanted -- and it goes back to where it was docked, at
   * the size it had, on Escape, on a press anywhere outside it, or on its own
   * button again. A press outside is the important one: the DJ who lifted the
   * browser to find a record and then reaches for the crossfader has said the
   * moment is over, and the panel gets out of the way without being asked.
   *
   * Not stored. A lift is a moment, not an arrangement, so it is not in the
   * workspace and does not survive a restart. A placement whose dock *is*
   * `overlay` -- a workspace can put the room or the night there -- is drawn
   * the same way, and dismissing it closes it: Rust's own description of
   * that dock is "dismissed by the next thing the DJ does". It was accepted,
   * resolved and stored, and never drawn at all.
   */
  let lifted = $state<string | null>(null);
  let stageEl = $state<HTMLElement | undefined>();
  /** Where the stage is, so a lifted panel covers exactly it. */
  let stageBox = $state<{ top: number; left: number; width: number; height: number } | null>(null);

  function measureStage() {
    const r = stageEl?.getBoundingClientRect();
    stageBox = r ? { top: r.top, left: r.left, width: r.width, height: r.height } : null;
  }

  const isLifted = (placement: SurfacePlacement) =>
    placement.dock === "overlay" || lifted === placement.surface;

  function lift(name: string) {
    measureStage();
    lifted = lifted === name ? null : name;
  }

  /** Put every lifted panel back; close the ones that only ever float. */
  function dismissLifted() {
    lifted = null;
    for (const placement of overlayDock) void toggleSurface(placement.surface as Drawn);
  }

  /** The lifted panel's box: the stage, a little inside its edges. */
  function liftStyle(): string {
    if (!stageBox) return "";
    const inset = 6;
    return (
      `position: fixed; top: ${Math.round(stageBox.top + inset)}px; ` +
      `left: ${Math.round(stageBox.left + inset)}px; ` +
      `width: ${Math.round(stageBox.width - inset * 2)}px; ` +
      `height: ${Math.round(stageBox.height - inset * 2)}px;`
    );
  }

  const anyLifted = $derived(lifted !== null || overlayDock.length > 0);

  $effect(() => {
    // A panel closed while lifted is no longer lifted.
    if (lifted && !placements.some((p) => p.surface === lifted)) lifted = null;
  });

  $effect(() => {
    if (!anyLifted) return;
    measureStage();
    // The stage moves when a lift empties a dock, and a lifted panel has to
    // follow it there rather than keep the box it was lifted into.
    const watch = new ResizeObserver(measureStage);
    if (stageEl) watch.observe(stageEl);
    // A press outside puts the panel back when it is *released*, not when it
    // lands. Putting it back returns its dock, and the master strip moves to
    // make room: done on the press, the crossfader a DJ had just taken hold
    // of jumped a quarter of the screen up under the hand, mid-gesture.
    let outside = false;
    const onPress = (event: PointerEvent) => {
      const target = event.target as Element | null;
      outside = !target?.closest?.(".surface.lifted");
    };
    const onRelease = () => {
      if (!outside) return;
      outside = false;
      dismissLifted();
    };
    // Capture, so a press on a control that stops its own event still counts
    // as the next thing the DJ did -- and nothing is prevented, so the press
    // on the crossfader still moves the crossfader.
    window.addEventListener("pointerdown", onPress, true);
    window.addEventListener("pointerup", onRelease, true);
    window.addEventListener("pointercancel", onRelease, true);
    window.addEventListener("resize", measureStage);
    return () => {
      watch.disconnect();
      window.removeEventListener("pointerdown", onPress, true);
      window.removeEventListener("pointerup", onRelease, true);
      window.removeEventListener("pointercancel", onRelease, true);
      window.removeEventListener("resize", measureStage);
    };
  });

  /**
   * `keepDensity`: leave how big things are drawn exactly as it is — neither
   * the workspace's named density nor its deck composition's. §109's
   * activities ask for this: density belongs to the window and the DJ's eyes,
   * and applying one marks it as chosen, which switches the window fitting
   * off and rescales everything on every switch.
   */
  async function applyWorkspace(asked: Workspace, { keepDensity = false } = {}) {
    // §3's *pinned*, honoured where it means something. A pinned surface is
    // "never moved, resized or closed by adaptation", and an arrangement is
    // the loudest adaptation there is: it replaces every placement at once.
    // So a pinned one is carried across unchanged, and one the preset also
    // names loses the preset's version rather than the DJ's — which is the
    // whole of what pinning it said.
    const kept = (workspace?.surfaces ?? []).filter((p) => p.pinned);
    // §120: the docks keep the size the DJ dragged them to, unless the
    // arrangement names its own. How wide the side panels are is about the
    // DJ's screen, not the activity -- the reason density stays with the
    // window -- and an activity switch that snapped a dragged dock back is a
    // size a DJ has to set again every time they change what they are doing.
    const askedDocks = asked.docks ?? {};
    const namesDocks = Object.values(askedDocks).some((size) => size != null);
    const preset = {
      ...asked,
      docks: namesDocks ? askedDocks : (workspace?.docks ?? {}),
      surfaces: [
        ...kept,
        ...asked.surfaces.filter(
          (p) => !kept.some((k) => k.surface === p.surface),
        ),
      ],
    };
    // Optimistic, then corrected — the same posture as `toggleSurface`, and
    // for the same reason: the panels appear on the press.
    workspace = preset;
    deckCount = preset.decks;
    if (!keepDensity) applyDensity(preset.density);
    // An empty theme is a preset with no opinion, not a preset asking for the
    // default: a DJ who chose "Cyber Trance" and then picked "4 Deck" keeps
    // their theme.
    //
    // Both halves, and the second is not decoration. §31 reads the night and
    // adapts the theme on a tick; `setPackage` alone paints the colours and
    // says nothing to djmanzo, so "High Contrast" wore the booth theme for
    // about four seconds and then quietly went back to green under a DJ who
    // had just asked for a dark-booth palette. `theme_chosen` is how a choice
    // is declared, and the adaptation stops deciding over it. Found by driving
    // the application; no type-check and no browser test could have seen it,
    // because neither has a night to read.
    if (preset.theme) {
      theme.setPackage(preset.theme);
      void themeChosen(preset.theme).catch(() => {});
    }
    // §5B: an arrangement may also change what a *deck* is made of, not only
    // which panels are open. Empty is a preset with no opinion, on the same
    // rule as the theme — most of §7's arrangements are about panels, and
    // rebuilding the deck under a DJ who only asked for the browser would be
    // the surprise §78 forbids.
    //
    // The deck count and the density stay the arrangement's. `applyLayout`
    // sets both, and a layout carries its own — so without this, naming a
    // composition would silently override the two things the arrangement
    // itself states. `Laptop Compact` says Ultra Dense and the `Performance`
    // composition says 0.85, and the browser test that asserts the band caught
    // exactly that: a layout named by an arrangement is *what a deck is made
    // of*, and how many there are and how tightly they are packed are the
    // arrangement's to say.
    if (preset.layout) {
      const named = layouts.find((l) => l.name === preset.layout);
      // A name nothing answers to is skipped rather than guessed, the same as
      // an unknown surface: the other five things the preset does still take,
      // and `layoutNotes` is where a half-loaded arrangement already says so.
      if (named) {
        applyLayout({
          ...named,
          decks: preset.decks,
          // The arrangement's band when it names one it has; otherwise the
          // composition's own, which is the honest fallback rather than a
          // guess.
          density: keepDensity ? density : (densityOf(preset.density) ?? named.density),
          // Not remembered as the DJ's layout when an activity chose it. The
          // layout file is *the DJ's* choice, restored with its own density
          // on the next start — so an activity that wrote it came back from
          // a restart drawn at another size: the scale a switch is careful
          // not to change, changed by the restart instead. Seen by driving
          // the application. The activity itself is remembered, and put
          // back at start-up (below).
        }, !keepDensity, { keepDensity });
      }
    }
    try {
      const resolved = await setCockpitWorkspace(preset);
      workspace = resolved.workspace;
      workspaceNotes = resolved.notes;
      permits = resolved.permits;
      deckCount = resolved.workspace.decks;
    } catch {
      // Keeping the optimistic state, for the reason `toggleSurface` gives.
    }
  }

  // ---------------------------------------------------------------- §109

  /** The activity strip and where the DJ is in it, from `dj_app::activity`. */
  let activityState = $state<Activities | null>(null);
  /** What the moment seems to call for. Marked on the strip, never followed. */
  let suggestion = $state<ActivitySuggestion | null>(null);
  /** Why the last activity was not kept, said on the strip. */
  let activityError = $state("");
  /** The height of the button row, and the full cockpit's, which the strip holds. */
  let goHeight = $state(0);
  let fullGoHeight = $state(0);
  $effect(() => {
    if (!activityMode && goHeight > 0) fullGoHeight = goHeight;
  });
  /** The full cockpit's own "keep as activity" field, open or not. */
  let namingActivity = $state(false);
  let activityName = $state("");
  const activityMode = $derived(activityState?.on ?? false);

  $effect(() => {
    void loadActivities()
      .then((found) => (activityState = found))
      .catch(() => {
        // No strip rather than a guessed one: the full cockpit still works.
      });
  });

  /**
   * §115: ask what the moment calls for, while the strip is showing.
   *
   * Every three seconds, and only in activity mode: the full cockpit has no
   * strip to mark, and a suggestion nobody can see is a query for nothing.
   */
  $effect(() => {
    if (!activityMode) {
      suggestion = null;
      return;
    }
    const ask = () =>
      void activitySuggestion()
        .then((found) => (suggestion = found ?? null))
        .catch(() => (suggestion = null));
    ask();
    const poll = setInterval(ask, 3000);
    return () => clearInterval(poll);
  });

  /**
   * Move to an activity: its arrangement, through the one path every
   * arrangement takes, and then where the DJ is, kept for a restart.
   */
  async function chooseActivity(slug: string) {
    const found = activityState?.activities.find((activity) => activity.slug === slug);
    if (!found) return;
    counted(`activity:${slug}`);
    board = null;
    // The density stays the DJ's. It is how big things are drawn for this
    // window and these eyes, not what the job needs — and an activity that
    // set it rescaled the whole interface on every switch, which moved the
    // decks by 27 px under the DJ's hands. Found by the test that measures.
    await applyWorkspace(
      { ...found.workspace, density: workspace?.density ?? found.workspace.density },
      { keepDensity: true },
    );
    try {
      activityState = await setActivityMode(true, slug);
    } catch {
      // The arrangement is on screen; only the bookkeeping failed.
    }
  }

  /**
   * After a restart in activity mode, the activity the DJ was in, drawn again
   * — its composition as well as its panels, at the DJ's own size. Once, when
   * both the activities and the layouts have arrived; the panels themselves
   * come back with the workspace, so this adds only what they cannot carry.
   */
  let activityRestored = false;
  $effect(() => {
    if (activityRestored || !activityState || layouts.length === 0) return;
    activityRestored = true;
    if (activityState.on && activityState.current) void chooseActivity(activityState.current);
  });

  /**
   * §115: a switch from the palette — `theme <pack>`, `activity <slug>`,
   * `workspace <name>` or `preset <id> [deck]` — carried out by the path its
   * own picker takes, so a theme chosen here is declared to djmanzo as one
   * chosen in the switcher is, and a workspace keeps what is pinned.
   */
  async function switchTo(run: string) {
    const space = run.indexOf(" ");
    const kind = run.slice(0, space);
    const which = run.slice(space + 1);
    if (kind !== "activity") counted(`${kind}:${kind === "preset" ? which.split(" ")[0] : which}`);
    if (kind === "theme") {
      theme.setPackage(which);
      void themeChosen(which).catch(() => {});
    } else if (kind === "activity") {
      await chooseActivity(which);
    } else if (kind === "workspace") {
      const found = [...mine, ...presets].find((w) => w.name === which);
      if (found) await applyWorkspace(found);
    } else if (kind === "preset") {
      const [id, deck] = which.split(" ");
      try {
        await applyPreset(id, deck ? Number(deck) : undefined);
      } catch {
        // The engine is not taking actions; the palette has already closed,
        // and the preset panel is where a refusal is explained.
      }
    }
  }

  /** Into activity mode, at wherever the DJ last was, or what is suggested. */
  async function enterActivities() {
    const start = activityState?.current || suggestion?.activity || "mix";
    await chooseActivity(start);
  }

  /** Back to the full cockpit. The arrangement on screen stays as it is. */
  async function leaveActivities() {
    try {
      activityState = await setActivityMode(false, activityState?.current ?? "");
    } catch {
      if (activityState) activityState = { ...activityState, on: false };
    }
  }

  /** Keep what is on screen as an activity of the DJ's own, and be in it. */
  async function keepAsActivity(title: string) {
    if (!workspace) return;
    activityError = "";
    try {
      const kept = await keepActivity(title, workspace);
      activityState = kept;
      const mine = kept.activities.find((activity) => !activity.shipped && activity.title === title.trim());
      if (mine) activityState = await setActivityMode(true, mine.slug);
    } catch (error) {
      // A name djmanzo ships, or an empty one: said where the DJ is looking.
      activityError = String(error);
    }
  }

  async function forgetOne(slug: string) {
    try {
      activityState = await forgetActivity(slug);
    } catch {
      // Still on the strip; the next load will say whether it went.
    }
  }

  /**
   * `1` to `9` move between activities and the key under Escape goes back
   * (§117: the owner's *"1,2,3, ... , not F1,F2,F3"*).
   *
   * A digit works from the full cockpit too, and goes into activity mode at
   * that activity: the digits are the views now, wherever the DJ is. The
   * back key only means something once there is an activity to go back to.
   * Never while typing, and never with a modifier held — `Shift` and a digit
   * is a hot cue, and the other chords belong to the DJ's own mapping.
   */
  function onActivityKey(event: KeyboardEvent) {
    // §117: the platform's own chords first — ⌘ on a Mac, Ctrl elsewhere —
    // unless the DJ's keyboard mapping already took the key.
    if (!event.defaultPrevented) {
      const chord = chordRun(event, (activityState?.activities ?? []).slice(0, 9).map((a) => a.slug));
      if (chord) {
        event.preventDefault();
        void runLeaf(chord);
        return;
      }
    }
    // §120: Escape puts a lifted panel back before it does anything else.
    if (event.key === "Escape" && anyLifted && !typing(event.target)) {
      dismissLifted();
      return;
    }
    // §117: `0` calls the dashboard up and puts it away; Escape puts it away.
    if (!typing(event.target) && !event.ctrlKey && !event.altKey && !event.metaKey && !event.shiftKey) {
      if (event.code === "Digit0") {
        event.preventDefault();
        void toggleDashboard();
        return;
      }
      if (event.key === "Escape" && board) {
        board = null;
        return;
      }
    }
    if (!activityState) return;
    if (typing(event.target)) return;
    if (event.ctrlKey || event.altKey || event.metaKey || event.shiftKey) return;
    if (event.code === activityState.back) {
      if (!activityState.on) return;
      if (activityState.previous) {
        event.preventDefault();
        void chooseActivity(activityState.previous);
      }
      return;
    }
    const match = activityState.activities.find((activity) => activity.key === event.code);
    if (match) {
      event.preventDefault();
      void chooseActivity(match.slug);
    }
  }

  /**
   * Wear a density a workspace named.
   *
   * The scale comes from the band table rather than from a copy here: Rust
   * holds the five points and what each is worth, and `density_bands` already
   * carries every one of them. The workspace stores the slug (`pro-dense`) and
   * the band its spoken name (`Pro Dense`); a Rust test asserts those two
   * spellings stay the same word, because a mismatch here is silent — the
   * preset would simply open at whatever density the window fitted.
   */
  /**
   * The scale factor a density band's name stands for, or `null`.
   *
   * Split out of `applyDensity` because §5B needs the number without the side
   * effects: an arrangement that names a deck composition has to keep its own
   * density, and `applyLayout` would otherwise write the composition's.
   */
  function densityOf(named: string): number | null {
    const band = bands.find(([, name]) => name.toLowerCase().replace(/ /g, "-") === named);
    return band ? band[2] : null;
  }

  function applyDensity(named: string) {
    const band = bands.find(([, name]) => name.toLowerCase().replace(/ /g, "-") === named);
    if (!band) return;
    // An explicit density wins over the window fitting, which is what
    // `chosenDensity` means: the DJ has decided.
    chosenDensity = band[2];
    density = band[2];
    densityName = band[1];
    document.documentElement.style.setProperty("--density", String(band[2]));
  }

  /**
   * §7: keep this arrangement under a name.
   *
   * The whole of §7's *starting points, not rigid identities* was true except
   * for the last step: a DJ could pick an arrangement, move what they liked, and
   * the edit survived a restart — it simply had no name, so the second layout
   * they built replaced the first. What is kept is the arrangement on screen,
   * resolved by Rust first, so a DJ reopening it gets what can be drawn rather
   * than what was asked for.
   *
   * The refusal is shown rather than swallowed. "djmanzo already ships an
   * arrangement called Club" is a sentence a DJ can act on in one press; a save
   * that quietly did nothing is one they discover an hour later.
   */
  async function keepThis() {
    const current = workspace;
    const name = (naming ?? "").trim();
    if (!current) return;
    try {
      mine = await keepWorkspace(name, { ...current, decks: deckCount });
      naming = null;
      namingError = null;
      // Named, so the picker shows it as chosen: the arrangement on screen and
      // the row the DJ just made are the same thing, and a picker that went
      // back to reading "Workspace…" would suggest otherwise.
      //
      // Rust stores the rename as well, so this survives a restart. It did not
      // at first — the row was in the list and the picker went back to the
      // shipped name it had been saved from, which is a DJ being told they are
      // in an arrangement they are not. Found by restarting the application.
      workspace = { ...current, name };
    } catch (e) {
      namingError = String(e);
    }
  }

  /** Take one of the DJ's own arrangements out of the collection. */
  async function forgetThis(name: string) {
    try {
      mine = await forgetWorkspace(name);
    } catch {
      // The row stays until djmanzo says it is gone, which is the same posture
      // every other write on this surface takes.
    }
  }

  /**
   * §79: store what djmanzo may not change by itself.
   *
   * Goes through the workspace like everything else on this surface, because a
   * lock *is* a field of the workspace — stored with it, restored with it, and
   * carried by the preset a DJ saves. A separate preference file would be a
   * second place the answer lives, and the first thing to disagree with it
   * would be the workspace a DJ shares with somebody else.
   *
   * Optimistic, then corrected, the same as every other write here: the panel's
   * switches move on the press and the resolver's answer replaces them.
   */
  async function saveLocks(locked: string[]) {
    const current = workspace;
    if (!current) return;
    const next = { ...current, locked };
    workspace = next;
    try {
      const resolved = await setCockpitWorkspace(next);
      workspace = resolved.workspace;
      workspaceNotes = resolved.notes;
      permits = resolved.permits;
    } catch {
      // Keeping the optimistic state, for the reason `toggleSurface` gives.
    }
  }

  /**
   * Write the current arrangement, without changing it.
   *
   * `toggleSurface` saves as a side effect of opening a panel; this is for the
   * changes that are not a panel — the deck count especially, which a DJ sets
   * once and expects to find again.
   */
  async function saveWorkspace() {
    const current = workspace;
    if (!current) return;
    const next = { ...current, decks: deckCount };
    workspace = next;
    try {
      const resolved = await setCockpitWorkspace(next);
      workspace = resolved.workspace;
      workspaceNotes = resolved.notes;
      permits = resolved.permits;
    } catch {
      // Keeping the optimistic state, for the reason `toggleSurface` gives:
      // failing to write a preferences file is not a reason to undo what the
      // DJ just did.
    }
  }

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
      // §5B: no opinion about the deck's composition. A fallback arrangement
      // built because none was stored must not rebuild the deck.
      layout: "",
      decks: deckCount,
      locked: [],
    };
    const already = current.surfaces.some((p) => p.surface === name);
    if (!already) counted(`surface:${name}`);
    const surfaces: SurfacePlacement[] = already
      ? current.surfaces.filter((p) => p.surface !== name)
      : [
          ...current.surfaces,
          {
            surface: name,
            dock: homeOf(name),
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
    //
    // `decks` comes from the live count rather than from `current`: the deck
    // toggle changes what is on screen without saving, so a workspace written
    // from the stored value would quietly file away a number the DJ had
    // already changed.
    workspace = { ...current, surfaces, decks: deckCount };
    if (name === "log" && !already) log = await sessionLog().catch(() => log);
    try {
      const resolved = await setCockpitWorkspace(workspace);
      workspace = resolved.workspace;
      workspaceNotes = resolved.notes;
      permits = resolved.permits;
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
  /** The same band, by name — what §81 stores as a night's density. */
  let densityName = $state("Standard");

  function fitDensity() {
    // §78: *no automatic surface resizing*. The density band is the only thing
    // in djmanzo that resizes a surface without being asked, so this is where
    // that bullet lands. A DJ who has locked it keeps the size they set at a
    // window edge they are about to drag, which is the whole point of locking
    // it: the booth screen is the one they learned the layout on.
    if (!permits.resize) return;
    if (chosenDensity !== null || bands.length === 0) return;
    const height = window.innerHeight;
    const band = bands.find(([least]) => height >= least) ?? bands[bands.length - 1];
    density = band[2];
    densityName = band[1];
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

  /**
   * A deck the assistant has asked the DJ to look at, if any.
   *
   * §41's `ui focus 2`. It fades on its own after a few seconds rather than
   * latching, because attention is a moment: a deck still outlined ten minutes
   * later is teaching the DJ to ignore the outline, which costs the next one.
   */
  let focusedDeck = $state<number | null>(null);
  let focusTimer: ReturnType<typeof setTimeout> | undefined;

  /** How long an asked-for glance lasts. */
  const FOCUS_MS = 6000;

  function focusDeck(number: number) {
    focusedDeck = number;
    clearTimeout(focusTimer);
    focusTimer = setTimeout(() => (focusedDeck = null), FOCUS_MS);
  }

  /** What a surface is called, from Rust rather than from a list here. */
  let surfaceTitles = $state<Record<string, string>>({});
  const titleOf = (name: string) => surfaceTitles[name] ?? name;

  async function loadWorkspace() {
    try {
      const known = await cockpitSurfaces();
      surfaceTitles = Object.fromEntries(known.map((s) => [s.name, s.title]));
      surfaceHomes = Object.fromEntries(known.map((s) => [s.name, s.home]));
    } catch {
      // A surface with no title falls back to its name, which is still a word
      // a DJ can read -- worse than "Session log", better than an empty header.
    }
    try {
      mine = await myWorkspaces();
    } catch {
      // An empty collection rather than a broken picker: the shipped
      // arrangements are still there and still work.
    }
    try {
      presets = await cockpitWorkspaces();
    } catch {
      // No picker rather than an empty one: the select below hides itself when
      // this list is empty, and every other way of arranging the cockpit still
      // works.
    }
    try {
      const resolved = await cockpitWorkspace();
      workspace = resolved.workspace;
      workspaceNotes = resolved.notes;
      permits = resolved.permits;
      // A workspace remembers how many decks were on screen — `toggleSurface`
      // writes `decks: deckCount` into every one it saves — and nothing read
      // it back. So a DJ who arranged four decks, saved the workspace and
      // reopened djmanzo got two, with the workspace still claiming four.
      // Found by §89's four-deck configuration failing to be four decks.
      deckCount = resolved.workspace.decks;
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
  let controlMappings = $state<MappingInfo[]>([]);

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

  /**
   * §117: Space, and the tree of English words behind it. Each leaf is run by
   * the path its own button or picker takes; see `runLeaf`.
   */
  const leader = new Leader((run) => void runLeaf(run));
  let paletteRef = $state<{ openPalette: () => void } | null>(null);

  /**
   * §117: the interface's own settings — the toolbars shown or not — and the
   * dashboard, when it is up.
   */
  let chrome = $state<InterfaceSettings | null>(null);
  const toolbars = $derived(chrome?.toolbars ?? false);
  let board = $state<Dashboard | null>(null);
  $effect(() => {
    void interfaceSettings()
      .then((settings) => (chrome = settings))
      .catch(() => {});
  });

  async function showToolbars(on: boolean) {
    try {
      chrome = await setToolbars(on);
    } catch {
      // The header stays as it is; the switch will say so on the next look.
    }
  }

  /** Call the dashboard up, for the activity the DJ is in; or put it away. */
  async function toggleDashboard() {
    if (board) {
      board = null;
      return;
    }
    try {
      board = await fetchDashboard(activityState?.on ? activityState.current : "");
    } catch {
      board = null;
    }
  }

  /** Count a use toward the dashboard's arrangement, whatever reached it. */
  function counted(id: string) {
    void usedTile(id).catch(() => {});
  }

  /** Bumped when the DJ keeps or forgets one of their own keys. */
  let leaderVersion = $state(0);

  /** The tree changes with the decks and with what the DJ has kept. */
  $effect(() => {
    const decks = deckCount;
    void leaderVersion;
    void activityState?.activities.length;
    void mine.length;
    void leaderTree(decks)
      .then((tree) => (leader.tree = tree))
      .catch((why) => console.warn("leader tree:", why));
  });

  /** Carry out one leaf of the tree. */
  async function runLeaf(run: string) {
    const space = run.indexOf(" ");
    const kind = run.slice(0, space);
    const rest = run.slice(space + 1);
    if (kind === "action") {
      await send(rest);
    } else if (kind === "surface") {
      if ((DRAWN as readonly string[]).includes(rest)) await toggleSurface(rest as Drawn);
    } else if (kind === "switch") {
      await switchTo(rest);
    } else if (kind === "ui") {
      if (rest === "palette") paletteRef?.openPalette();
      else if (rest === "search") await searchLibrary();
      else if (rest === "back") {
        if (activityState?.previous) await chooseActivity(activityState.previous);
      } else if (rest === "everything") await leaveActivities();
      else if (rest === "record") {
        if (setRecording) await send(setRecording.active ? "record off" : "record on");
      } else if (rest === "mark") await mark();
      else if (rest === "dashboard") await toggleDashboard();
    } else if (kind === "uiop") {
      // §41's operations, which a DJ's own key may name because the palette
      // offers them; carried out the way the palette carries them out.
      await uiDo(rest).catch(() => {});
    }
  }

  /** Open the library if it is not, and put the cursor in its search. */
  async function searchLibrary() {
    if (!isOpen("library")) await toggleSurface("library");
    await tick();
    document.querySelector<HTMLInputElement>('input[aria-label="Search the library"]')?.focus();
  }

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
    // The guide first: while it is open every key is its, and Space opens it.
    // Registered before the keyboard map's listener, on the same capture
    // phase, so a key the guide takes never reaches a deck.
    const onLeaderKey = (event: KeyboardEvent) => {
      if (leader.press(event)) {
        event.preventDefault();
        event.stopImmediatePropagation();
      }
    };
    window.addEventListener("keydown", onLeaderKey, true);
    const detachKeys = keyboard.attach(window);
    const detach = () => {
      detachKeys();
      window.removeEventListener("keydown", onLeaderKey, true);
    };
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
  function applyLayout(next: Layout, remember = true, { keepDensity = false } = {}) {
    layout = next;
    deckCount = next.decks;
    // A layout that names a density is a DJ who has decided, so the
    // window-fitting above stands down rather than arguing with it — unless
    // the caller is an activity, whose composition is what a deck is made of
    // and says nothing about how big it is drawn. See `applyWorkspace`.
    if (!keepDensity) {
      chosenDensity = next.density;
      density = next.density;
      document.documentElement.style.setProperty("--density", String(next.density));
    }
    // A layout naming a browser opens one, unless the DJ has pinned the
    // arrangement: this is a panel appearing without anybody pressing anything,
    // which is exactly what §78's first bullet is about.
    if (next.browser && !isOpen("library") && permits.rearrange) {
      void toggleSurface("library");
    }
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
    // §41. An arrangement can now change without anybody in this window
    // pressing anything — the assistant asks, Rust applies and stores, and
    // this is how the panel actually appears. `focus` is not stored, so it is
    // acted on here and forgotten.
    const unwatchCockpit = onCockpit((applied) => {
      workspace = applied.workspace.workspace;
      workspaceNotes = applied.workspace.notes;
      permits = applied.workspace.permits;
      deckCount = applied.workspace.workspace.decks;
      if (applied.focus !== null) focusDeck(applied.focus);
    });

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
    /**
     * §31: ask djmanzo what to wear, slowly.
     *
     * Every twenty seconds, not every snapshot. The answer is almost always
     * "the same thing" — `dj_app::mood` holds a four-minute minimum and a
     * forty-second settling time, so a faster tick asks a question whose answer
     * cannot have moved, and it walks the night's reading to produce it.
     *
     * The decision is not made here. Rust owns the rule, this owns the pixels,
     * which is the same split the density bands use.
     */
    const wardrobe = setInterval(() => {
      void themeNow()
        // §78: *no theme changes unless explicitly permitted*. Read at the
        // moment of the tick rather than captured when the interval was made,
        // so locking the theme stops the next one twenty seconds later instead
        // of at the next restart.
        //
        // The question is still asked. `themeNow` is what §31 reads the night
        // with, and a DJ who unlocks the theme an hour in should get the
        // palette the night has earned rather than the one it had when they
        // locked it -- so what stops here is the wearing, not the reading.
        .then((mood) => {
          if (permits.retheme) theme.adapt(mood.theme, mood.over_ms);
        })
        .catch(() => {});
    }, 20_000);

    return () => {
      clearInterval(wardrobe);
      void unlisten.then((fn) => fn());
      void unwatchCockpit.then((fn) => fn());
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

  // §53: what the controller now open reaches. Polled slowly, because this is
  // the one question whose answer changes when somebody physically touches the
  // machine — a scale of seconds rather than of frames.
  $effect(() => watchHands());

  // §8 Level 1's favourite pad pages and kept controls, read once for the whole
  // application. Here rather than in the pad zone because four decks asking the
  // same question would be four answers that can disagree, and a change made in
  // Settings would not reach a pad zone already on screen.
  $effect(() => {
    void loadRemembers();
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
          // `?? emptyWorld()` rather than trusting the answer. A backend that
          // answered null here used to take the whole interface down on
          // `world.entities`, and the `catch` right below shows the author of
          // this loop already expected the read to be able to fail — a null is
          // the same failure arriving through the other door.
          world = (await getWorld()) ?? emptyWorld();
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
    // §78's fourth bullet: *no surprise visual transitions*. These six
    // properties are the whole of how the interface answers the audio, so
    // handing them nothing is the interface sitting still -- `publishAudio`
    // writes zeros for an absent context, which is the rest state rather than
    // whatever pulse it happened to be mid-way through. Freezing them at their
    // last value would pin a random frame of the music onto the booth screen
    // for the rest of the night.
    publishAudio(permits.restyle ? snapshot?.context : undefined);
  });

  /*
    And the attention budget goes to CSS the same way.

    One attribute rather than a prop threaded through thirty components: what
    the budget governs is whether things move, and motion is decided in the
    stylesheet. `cockpit::Attention` derives the level in Rust from the context
    engine, so a panel cannot decide for itself that now is a good moment to
    animate -- see `app.css` for what `none` actually costs.
  */
  $effect(() => {
    const motion = snapshot?.attention.motion;
    if (motion) document.documentElement.dataset.motion = motion;
  });

  /*
    §17: when the night moves to a new phase, open what that phase is for.

    # Why this is an effect on the phase and not a poll

    A phase changes a handful of times a night. What is watched is the *word*,
    as a `$derived` string, rather than the snapshot -- which is a fresh object
    sixty times a second, and reading it inside the effect is the trap that
    remounted every knob in the application during §29.

    # Why it may refuse to do anything

    §18's rule, and it is not this component's to soften: `attention.reflow` is
    false during a mix, always, because moving a panel while somebody is
    reaching for it is the failure that makes adaptive interfaces feel hostile.
    A phase that turns over mid-mix is therefore *skipped*, not queued: by the
    time the mix ends the phase is either still the same one -- and the next
    snapshot's effect run picks it up, because `promoted` was never advanced --
    or it has moved on and the stale one was never worth applying.

    # Why it only ever opens

    §17 ends with "the DJ must always be able to override it", and the
    overriding has to work without a dialog. So everything already on screen
    stays, the phase adds, and closing one of its panels is the override. A
    phase that replaced the arrangement would be taking a decision back from
    the DJ every twenty minutes.
  */
  let promoted = $state<string | null>(null);
  const phaseNow = $derived(snapshot?.context.session?.phase ?? null);

  $effect(() => {
    const now = phaseNow;
    // Two gates, and they answer different questions. `reflow` is §18's and is
    // derived: *is this a bad moment* — false during every mix. `permits` is
    // §79's and is chosen: *is this allowed at all*. A DJ who freezes the layout
    // has decided something the context engine has no vote on, so neither gate
    // substitutes for the other.
    const mayMove = (snapshot?.attention.reflow ?? false) && permits.rearrange;
    if (!ready || !mayMove || now === promoted) return;
    promoted = now;
    void phasePriorities()
      .then((wanted) => {
        for (const surface of wanted) {
          if (!(DRAWN as readonly string[]).includes(surface)) continue;
          if (isOpen(surface as Drawn)) continue;
          void toggleSurface(surface as Drawn);
        }
      })
      // A phase whose priorities cannot be read leaves the arrangement alone,
      // which is the same thing it does during a mix.
      .catch(() => {});
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

  // Read outside the template so event handlers, which run later, do not have
  // to prove `snapshot` is still non-null.
  const cueSplit = $derived(snapshot?.master.cue_split ?? false);
  // Before a device is open there is no engine, so the parameter table still
  // holds its zeroed defaults. Reading that as "bypassed" would announce a
  // safety feature was off when in fact nothing is running at all — so the
  // idle case is its own state rather than being folded into the off one.
  const limiterOn = $derived(!ready || (snapshot?.master.limiter_enabled ?? true));
  const split = $derived(snapshot?.master.split_output ?? null);
  /** §18: whether the layout must hold still right now — true during every mix. */
  const quietLayout = $derived(snapshot ? !snapshot.attention.reflow : false);
</script>

<svelte:window onkeydown={onActivityKey} />

<main>
  <!--
    The page's own name, for a reader that cannot see the mark in the corner.

    Not decoration and not a box ticked: a screen reader's usual way into an
    unfamiliar application is the heading list, and this one had no headings at
    all above the panels -- so the whole booth was one flat run of controls with
    nothing saying where it started. Off-screen rather than drawn, because the
    brand already says it to anyone who can see it and §5's bar has no room for
    it twice.
  -->
  <h1 class="offscreen">djmanzo — the booth</h1>
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
          <span data-dj-name>{djName || "DJ MANZO"}</span>
        {/if}
      </button>
      {#if logo}
        <button class="brand-reset" onclick={resetLogo} title="Restore the DJ MANZO mark">
          Reset
        </button>
      {/if}
    </div>

    <div style="display:flex; gap:0.6rem; align-items:center;">
      <!--
        §32's one theme whose identity is the metaphor opens the watershed.
        One-directional: nothing here closes it, because the metaphor must be
        available to a DJ who wants it under any theme and must not be a cage
        under its own.
      -->
      <ThemeSwitcher onWorld={() => (living = true)} />
    </div>

    <div class="device">
      {#if isOpen("settings")}
        <select
          aria-label="Sound card"
          bind:value={selectedDevice}
          disabled={devices.length === 0}
        >
          {#each devices as device (device.id)}
            <option value={device.id}>
              {device.name}{device.is_default ? " (default)" : ""}
            </option>
          {/each}
        </select>

        <select aria-label="Buffer size" bind:value={bufferFrames}>
          {#each [64, 128, 256, 512, 1024] as frames (frames)}
            <option value={frames}>{frames} frames</option>
          {/each}
        </select>

        {#if devices.length > 1}
          <select
            aria-label="Headphone cue sound card"
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

    <!--
      §5's Mission Bar. `dj_app::mission` decides what is on it, what each
      reading says and which of them is worth a colour; this only places it.

      It replaced a strip that had grown here a reading at a time — the sample
      rate, the latency, the load, the dropouts, the clock drift between two
      cards, and §39's room chip — each with its own threshold written beside
      it. The load went amber at 0.7 here and nothing else had a rule at all,
      so a mix being flattened by the limiter and a recording that had stopped
      writing were both invisible while a CPU figure a DJ can do nothing about
      was the one thing on screen in colour.

      The split-card drift is the one reading that did not move onto the bar.
      It is not one of §5's eleven, it only exists on a two-device setup, and
      it belongs with the device that produces it — so it stays here, beside
      the bar rather than inside it.
    -->
    <div class="status mono">
      <MissionBar onOpenRoom={() => toggleSurface("room")} />
      {#if split}
        <!--
          Two cards means two crystals, and this is the measured disagreement
          between them. Shown because it is otherwise completely invisible: a
          figure that settles is a healthy pair, and one that keeps climbing is
          a device misreporting its rate — which you would otherwise only find
          out when the headphones started clicking mid-set.
        -->
        <span
          class="drift"
          class:xruns={!split.healthy}
          title="Clock difference between the two sound cards, corrected by resampling. {split.queue_ms.toFixed(1)} ms queued."
        >
          {split.drift_ppm >= 0 ? "+" : ""}{split.drift_ppm.toFixed(0)} ppm
        </span>
      {/if}
      {#if slowFrames !== null}
        <!--
          The interface's own frame rate when it is too low to scroll a
          waveform smoothly. Here, beside the readings, rather than in the band
          above the decks: it comes and goes as the frame rate does, and in
          the band it moved the decks every time. The whole sentence is the
          title; the chip is the number.
        -->
        <span
          class="warn-chip"
          data-slow-frames
          title="Interface running at {slowFrames.toFixed(0)} fps. This usually means the webview has no hardware acceleration — the audio engine is unaffected, but the waveform will not scroll smoothly."
        >
          UI {slowFrames.toFixed(0)} fps
        </span>
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
    <!--
      §109: the row keeps the full cockpit's height while the strip is up, so
      entering and leaving activity mode swap one set of buttons for another
      and the decks below do not move. Measured rather than set: the full row
      wraps differently at every window width, and a fixed number would be
      right at one of them.
    -->
    <div
      class="go"
      class:holding={toolbars && activityMode && fullGoHeight > 0}
      bind:clientHeight={goHeight}
      style:min-height={toolbars && activityMode && fullGoHeight > 0 ? `${fullGoHeight}px` : null}
    >
      {#if !toolbars}
        <!--
          §117: no toolbars. The two rows of buttons are on the dashboard and
          under Space; what stays is where the DJ is and the two ways to
          everything else, so the room above the decks goes to the activity.
        -->
        <nav class="go-group slim" aria-label="Where you are">
          <button type="button" class="dash" onclick={() => void toggleDashboard()} title="Everything else — activities, panels, presets, themes, workspaces (0)">
            <Icon name="table-cells" size="0.95rem" /> Dashboard <kbd>0</kbd>
          </button>
          <span class="where" data-where>
            {#if activityMode && activityState}
              {@const at = activityState.activities.findIndex((a) => a.slug === activityState?.current)}
              {#if at >= 0 && at < 9}<kbd>{at + 1}</kbd>{/if}
              {activityState.activities[at]?.title ?? "Activity"}
            {:else}
              Everything
            {/if}
          </span>
          <button type="button" class="keys-hint" onclick={() => leader.start()} title="Every key from here, one word at a time">
            <kbd>Space</kbd> keys
          </button>
        </nav>
      {:else if activityMode && activityState}
        <!--
          §109's activity mode: the strip stands where the panel buttons and
          the stage pickers stand, so switching into it swaps one row for
          another and nothing below moves. The set group — REC, Mark, SAFE —
          stays, because it is read from across the booth whatever the DJ is
          doing.
        -->
        <ActivityStrip
          activities={activityState.activities}
          current={activityState.current}
          {suggestion}
          error={activityError}
          onchoose={(slug) => void chooseActivity(slug)}
          onkeep={(title) => void keepAsActivity(title)}
          onforget={(slug) => void forgetOne(slug)}
          onleave={() => void leaveActivities()}
        />
      {:else}
      <nav class="go-group" aria-label="Panels">
        <IconButton icon="fa-solid fa-folder-open" label="Browse" title="Find and load tracks" active={isOpen("library")} onClick={() => toggleSurface("library")} />
        <!--
          Prepare: what you have set aside for the next twenty minutes, and
          what follows from the decks. Its own surface rather than a corner of
          the browser, which is what the directive's §21 means by first class
          -- the library can be along the bottom and this beside the decks, or
          this alone while a set is planned.
        -->
        <IconButton icon="fa-solid fa-layer-group" label="Prepare" title="Tracks set aside, before they are on a deck" active={isOpen("prepare")} onClick={() => toggleSurface("prepare")} />
        <!--
          The rail: what could come next, and why. Beside Prepare rather than
          inside it, because a rail is glanced at while a transition is
          happening and a tab is something you go and find.
        -->
        <IconButton icon="fa-solid fa-forward" label="Next" title="What could come next, and why" active={isOpen("next")} onClick={() => toggleSurface("next")} />
        <!--
          The set plan: the shape of the whole night rather than the next
          record. Along the bottom, because a sequence is read across and a
          side dock is 360 px wide.
        -->
        <IconButton icon="fa-solid fa-list-ol" label="Plan" title="The shape of the night, as a sequence" active={isOpen("plan")} onClick={() => toggleSurface("plan")} />
        <!--
          The pair: two records side by side and the seam between them. Along
          the bottom for the same reason the plan is -- two columns and a
          waveform each need width, and a side dock is 360 px.
        -->
        <IconButton icon="fa-solid fa-code-compare" label="Pair" title="Two records side by side, and the seam between them" active={isOpen("pair")} onClick={() => toggleSurface("pair")} />
        <!--
          The lab. Beside the pair view because it is the same two records
          asked a different question: that one says what the mix *is*, this one
          lets you hear it before the room does.
        -->
        <IconButton icon="fa-solid fa-flask" label="Practice" title="Hear a transition before you play it, without touching the decks" active={isOpen("practice")} onClick={() => toggleSurface("practice")} />
        <IconButton icon="fa-solid fa-moon" label="Night" title="Where the set is in its arc, and what says so" active={isOpen("night")} onClick={() => toggleSurface("night")} />
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
        <!--
          §107's Singers surface has no button in this row, on purpose: the
          row already wraps on a laptop screen, and one more button pushed a
          deck's pads below the fold — the density test caught it. It is the
          Karaoke activity's (F8), which is where a host works from.
        -->
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
          onclick={() => {
            deckCount = deckCount === 2 ? 4 : deckCount === 4 ? 6 : 2;
            // Remembered, like every other arrangement. Without this a DJ who
            // set up four decks found two the next time they opened djmanzo,
            // with nothing having said the change was temporary — the
            // workspace already carried a `decks` field and only ever wrote
            // the stale one.
            void saveWorkspace();
          }}
          title="Show {deckCount === 2 ? 'four' : deckCount === 4 ? 'six' : 'two'} decks. The engine runs six either way."
        >
          {deckCount} decks
        </button>
        <!--
          §7's workspace picker. A preset is a starting point: it opens some
          panels, sets a deck count and a density, and may name a theme. It is
          not a mode — nothing is "in" a workspace afterwards, and the next
          panel the DJ opens edits it like any other arrangement.

          Beside the layout picker rather than inside Settings because this is
          the control a DJ reaches for when the night changes — a wedding turns
          into an open-format floor at half past midnight, and that should be
          one press from the booth screen.
        -->
        {#if presets.length > 0}
          <select
            class="workspace-preset"
            aria-label="Workspace"
            onchange={(event) => {
              const value = event.currentTarget.value;
              if (value === KEEP) {
                // Naming, rather than a dialog. The field appears in the slot
                // the description occupies, so the row does not grow and the
                // two buttons at the end of it do not move — which §39's
                // fourteenth panel button already proved they must not.
                naming = workspace?.name ?? "";
                namingError = null;
                event.currentTarget.value = "";
                return;
              }
              const chosen = [...mine, ...presets].find((w) => w.name === value);
              if (chosen) void applyWorkspace(chosen);
            }}
          >
            <option value="">Workspace…</option>
            <!--
              §7 and §103's modularity. Theirs first: a DJ who has saved a
              layout is looking for that one, and reading past twenty-three
              they have never opened to reach it is the thing having saved it
              was supposed to avoid.
            -->
            {#if mine.length > 0}
              <optgroup label="Yours">
                {#each mine as option (option.name)}
                  <option
                    value={option.name}
                    title={option.about}
                    selected={workspace?.name === option.name}
                  >
                    {option.name}
                  </option>
                {/each}
              </optgroup>
            {/if}
            <optgroup label="djmanzo's">
              {#each presets as option (option.name)}
                <option
                  value={option.name}
                  title={option.about}
                  selected={workspace?.name === option.name}
                >
                  {option.name}
                </option>
              {/each}
            </optgroup>
            <optgroup label="This one">
              <option value={KEEP}>Save this arrangement…</option>
            </optgroup>
          </select>
          {#if naming !== null}
            <!--
              In the description's slot, so the row keeps its width. Enter
              saves and Escape gives up, because a DJ naming a layout between
              two records has a hand on the keyboard and not on the mouse.
            -->
            <input
              class="naming"
              aria-label="Name for this arrangement"
              placeholder="Name this arrangement"
              bind:value={naming}
              onkeydown={(event) => {
                if (event.key === "Enter") void keepThis();
                if (event.key === "Escape") {
                  naming = null;
                  namingError = null;
                }
              }}
            />
            <button class="naming-keep" onclick={() => void keepThis()} title="Keep it">
              Keep
            </button>
            {#if workspace && mine.some((w) => w.name === workspace?.name)}
              <button
                class="naming-forget"
                onclick={() => void forgetThis(workspace?.name ?? "")}
                title="Take this arrangement out of your collection"
              >
                Forget
              </button>
            {/if}
            {#if namingError}
              <!--
                Shown, not swallowed. "djmanzo already ships an arrangement
                called Club" is a sentence a DJ acts on in one press; a save
                that quietly did nothing is one they find out about an hour
                later, looking for a layout that was never kept.
              -->
              <span class="naming-error" role="alert">{namingError}</span>
            {/if}
          {:else if workspace?.about}
            <!--
              What the chosen one is for. The select shows twenty-three names and
              a name is not a description: "Open Format" and "Club" are both
              plausible at midnight, and this is the line that says which one
              gives you four decks.
            -->
            <span class="preset-about" title={workspace.about}>{workspace.about}</span>
          {/if}
        {/if}
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
        <!--
          §109: into activity mode — the strip in place of the panel buttons,
          so the decks below do not move. The first activity is where the DJ
          last was, or what the moment suggests, or the mix.
        -->
        <IconButton
          icon="fa-solid fa-hand-pointer"
          label="Activities"
          title="Show only what the job in front of you needs — dig, mix, perform, prepare — and switch with 1 to 9"
          onClick={() => void enterActivities()}
        />
        <!--
          And the way in for a DJ's own: arrange the cockpit, then keep it.
          Here as well as on the strip, because entering activity mode opens an
          activity — an arrangement built out here would be replaced before it
          could be kept.
        -->
        {#if namingActivity}
          <form
            class="naming-activity"
            onsubmit={(event) => {
              event.preventDefault();
              const title = activityName.trim();
              if (!title) return;
              namingActivity = false;
              activityName = "";
              void keepAsActivity(title);
            }}
          >
            <!-- svelte-ignore a11y_autofocus -->
            <input
              bind:value={activityName}
              placeholder="Name this activity"
              aria-label="Name for the new activity"
              autofocus
              onkeydown={(event) => {
                if (event.key === "Escape") namingActivity = false;
              }}
            />
            <button type="submit" disabled={!activityName.trim()}>Keep</button>
          </form>
        {:else}
          <IconButton
            icon="fa-solid fa-plus"
            title="Keep this arrangement as an activity of your own"
            aria-label="Keep this arrangement as an activity"
            onClick={() => (namingActivity = true)}
          />
        {/if}
        <!-- Said here too: the strip is not on screen out here, and a refusal
             only the strip could show was a press that silently did nothing. -->
        {#if activityError && !activityMode}
          <span class="warn-chip bad" role="alert">{activityError}</span>
        {/if}
        <!--
          §112: the watershed has no switch on this bar any more. It was one
          of the most visible buttons in the interface for a view that repeats
          what the waveforms and the mixer already say and, when open, pushed
          the pads off a 1280×800 screen. It is §55's *world*, chosen like one
          — *Watershed Living* in the theme picker opens it — and it closes
          from its own band, where the DJ who opened it is looking.
        -->
      </div>
      {/if}

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
        <!--
          §47. Beside REC and Mark because it is the third control that has to
          be findable without hunting, and the only one that is found while
          something is going wrong.

          It is `safe` on the action bus, so the same thing is on a controller
          pad, on a keyboard shortcut and at the top of a script. What it does
          and — more to the point — what it refuses to do is written down in
          `commands::make_safe`: it never stops a record and never moves a
          fader, because an emergency control that silences the floor is worse
          than the emergency.
        -->
        <button
          class="safe"
          disabled={!ready}
          onclick={() => send("safe")}
          title="Take every control back, clear every effect, flatten the tone. Nothing stops playing."
        >
          SAFE
        </button>
      </div>
      <!--
        §115: the quiet proposer, in the room this row has after the set
        group — which is where the eye already goes for the state of the
        night, and wider than the gap in the row above, where a sentence was
        squeezed out and left its button alone. See `Whisper.svelte` and
        `dj_app::whisper`.
      -->
      <!--
        §118: the night being played. After the set group and before the
        proposer, because while a night is on it is the state of the night.
      -->
      <!--
        §118a: the assistant's guides, one per topic -- the next record, the
        mix, and while a night is on, its plans for trouble.
      -->
      <Guides send={(action) => send(action)} decks={deckCount} onTrouble={() => (tonightDeciding = true)} />
      {#if liveId}
        <Tonight id={liveId} onEnd={() => void endNight()} bind:deciding={tonightDeciding} />
      {/if}
      <Whisper offered={snapshot?.whisper} send={(action) => send(action)} decks={deckCount} />
    </div>
  </header>

  <!--
    The notice band. In the flow — pushing the decks down by the height of what
    it says — only while §18 allows the interface to move. While two records
    are audible it floats over the top edge of the stage instead: covering a
    deck's title for as long as a notice is up is the lesser harm, and moving a
    fader out from under a hand mid-mix is the harm §18 exists to prevent. A
    headphone device failing is exactly the kind of thing that happens mid-mix.
  -->
  <div class="notices" class:floating={quietLayout} data-notices>
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

    <!--
      The interface's own frame rate is not said here any more; see the chip
      beside the Mission Bar. This band sits above the decks, and a notice that
      comes and goes with the frame rate moved them down and up again — 36 px,
      on exactly the struggling machine where it appears, and in the middle of
      whatever mix was slowing it down. §18 says the interface may not reflow
      while two records are audible; a warning that does is the warning breaking
      the rule it exists to help with.
    -->

    <!--
      §44's staged transaction, in the notice band rather than as a surface.

      It belongs here for the reason the audit gives: the AI is never the largest
      thing on screen, and what it normally has to say is one line and two
      buttons. It occupies no height at all when nothing is staged, so the decks
      are not paying for it the rest of the night.
    -->
    <Staged enabled={ready} />
  </div>

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
    <Browse enabled={ready} deckCount={deckCount} />
  {/snippet}
  {#snippet surfacePrepare()}
    <SideView enabled={ready} {deckCount} decks={snapshot?.decks ?? []} />
  {/snippet}

  {#snippet surfaceNext()}
    <Next enabled={ready} {deckCount} decks={snapshot?.decks ?? []} pack={chosenPack} />
  {/snippet}

  {#snippet surfacePlan()}
    <Plan enabled={ready} />
  {/snippet}

  {#snippet surfacePair()}
    <Pair enabled={ready} {deckCount} decks={snapshot?.decks ?? []} />
  {/snippet}

  {#snippet surfacePractice()}
    <Practice enabled={ready} />
  {/snippet}

  {#snippet surfaceEvent()}
    <EventPanel {liveId} onGoLive={(id) => void goLive(id)} />
  {/snippet}

  {#snippet surfaceNight()}
    <Night enabled={ready} density={densityName} onDensity={applyDensity} />
  {/snippet}

  {#snippet surfaceKaraoke()}
    <Singers enabled={ready} {deckCount} decks={snapshot?.decks ?? []} />
  {/snippet}

  {#snippet surfaceRequests()}
    <!--
      §109: the room's requests beside the decks, for the Requests activity.
      They also live as a view inside the collection; here they are a surface
      of their own so the list and the records to answer it with are on screen
      together. "Find" opens the collection and searches it.
    -->
    <Requests
      enabled={ready}
      onFind={(text) => {
        if (!isOpen("library")) void toggleSurface("library");
        findInCollection(text);
      }}
    />
  {/snippet}

  {#snippet surfaceRoom()}
    <RoomSense enabled={ready} />
  {/snippet}

  {#snippet surfaceMixes()}
    <Mixes enabled={ready} />
  {/snippet}

  {#snippet surfaceAtHand()}
    <AtHand enabled={ready} send={(action) => void send(action)} />
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
    <Shortcuts {keyboard} onclose={() => toggleSurface("keys")} decks={deckCount} onKeysChanged={() => (leaderVersion += 1)} />
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
    <Settings
      onLogoChange={refreshLogo}
      deviceChannels={active?.channels ?? null}
      locked={workspace?.locked ?? []}
      onLock={saveLocks}
      onSetUp={setUpForTonight}
      onWelcome={() => void openWelcome()}
      onPackChange={(id) => (chosenPack = id)}
      {toolbars}
      onToolbars={(on) => void showToolbars(on)}
    />
  {/snippet}
  {#snippet surfaceLog()}
    <!--
      `tabindex` and a name because this scrolls. §33 asks for keyboard
      operation, and a region that overflows and cannot be focused is one a
      keyboard cannot reach the bottom of — axe calls it
      `scrollable-region-focusable` and it is right. It went unnoticed until a
      taller Assistant panel pushed this one into overflowing: the defect was
      always here, and the layout was hiding it.

      The `svelte-ignore` is the two rules disagreeing rather than a shortcut.
      Svelte's `a11y_no_noninteractive_tabindex` is a good general heuristic —
      do not make static things focusable — and it has no way to know this one
      scrolls. A focusable scroll container is the documented fix for the axe
      rule, and the `role` and name are what keep it from being a bare
      focusable div: a screen reader announces a named region rather than
      landing somewhere with nothing to say.
    -->
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <div class="log" tabindex="0" role="region" aria-label="Session log">
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

  {#snippet dockGrip(dock: DockName)}
    <!--
      §120: the gap between a dock and the stage is the handle for the
      dock's size. The gap rather than a bar of its own, so the layout is
      exactly as it was and the handle is as wide as the space a hand
      already aims between the two.
    -->
    <div
      class="dock-grip {dock === 'bottom' ? 'across' : 'along'}"
      class:dragging={dockDrag?.dock === dock}
      role="separator"
      aria-label="Resize the {dock} dock"
      aria-orientation={dock === "bottom" ? "horizontal" : "vertical"}
      title="Drag to size the {dock} panels; double-click for their own share"
      data-dock-grip={dock}
      onpointerdown={(e) => startDockResize(e, dock)}
      onpointermove={(e) => onDockResize(e, dock)}
      onpointerup={(e) => endDockResize(e, dock)}
      onpointercancel={(e) => endDockResize(e, dock)}
      ondblclick={() => resetDock(dock)}
    ></div>
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
    <section
      class="surface"
      class:collapsed={placement.collapsed && !isLifted(placement)}
      class:pinned={placement.pinned}
      class:lifted={isLifted(placement)}
      data-surface={placement.surface}
      data-collapsed={placement.collapsed}
      data-pinned={placement.pinned}
      data-lifted={isLifted(placement)}
      bind:this={boxes[placement.surface]}
      style={isLifted(placement) ? liftStyle() : placement.collapsed ? "" : sizeStyle(placement)}
    >
      <header class="surface-head">
        <h2>{titleOf(placement.surface)}</h2>
        {#if placement.dock !== "overlay"}
          <!--
            §120's temporary window. The panel takes the whole stage for the
            moment and gives it back, unchanged, the moment the DJ is done.
          -->
          <button
            class="lift"
            class:on={lifted === placement.surface}
            title={lifted === placement.surface
              ? `Put ${titleOf(placement.surface)} back`
              : `Lift ${titleOf(placement.surface)} over the decks for now`}
            aria-label={lifted === placement.surface
              ? `Put ${titleOf(placement.surface)} back`
              : `Lift ${titleOf(placement.surface)}`}
            aria-pressed={lifted === placement.surface}
            onclick={() => lift(placement.surface)}
          ><Icon name={lifted === placement.surface ? "compress" : "expand"} size="0.95rem" /></button>
        {/if}
        <!--
          §3's *collapsed* and *expanded*. Two of the eleven verbs it lists,
          and the field behind them was stored, serialised and resolved by Rust
          while nothing on this side read it — so a workspace faithfully
          recorded a fold nobody could make.
        -->
        <button
          class="fold"
          title={placement.collapsed
            ? `Expand ${titleOf(placement.surface)}`
            : `Collapse ${titleOf(placement.surface)}`}
          aria-label={placement.collapsed
            ? `Expand ${titleOf(placement.surface)}`
            : `Collapse ${titleOf(placement.surface)}`}
          aria-expanded={!placement.collapsed}
          onclick={() =>
            setPlacement(placement.surface, { collapsed: !placement.collapsed })}
        >{placement.collapsed ? "+" : "–"}</button>
        <!--
          §3's *pinned*, which is the per-surface half of §78's freeze: an
          arrangement may not move, resize or close it. A DJ who has put the
          room panel where they want it keeps it when they press a preset.
        -->
        <button
          class="pin"
          class:on={placement.pinned}
          title={placement.pinned
            ? `Unpin ${titleOf(placement.surface)}`
            : `Pin ${titleOf(placement.surface)} where it is`}
          aria-label={placement.pinned
            ? `Unpin ${titleOf(placement.surface)}`
            : `Pin ${titleOf(placement.surface)}`}
          aria-pressed={placement.pinned}
          onclick={() =>
            setPlacement(placement.surface, { pinned: !placement.pinned })}
        >&#9679;</button>
        <button
          class="shut"
          title="Close {titleOf(placement.surface)}"
          aria-label="Close {titleOf(placement.surface)}"
          onclick={() => toggleSurface(placement.surface as Drawn)}
        >&times;</button>
      </header>
      <!--
        §3's *resized*. On the dock's own axis, and on the edge that faces the
        performance zone: the left dock grows to the right, the right dock to
        the left, and the bottom one upwards. A handle on the wrong edge makes
        the panel run away from the pointer.
      -->
      {#if !placement.collapsed && !isLifted(placement)}
        <div
          class="grip"
          role="separator"
          aria-label="Resize {titleOf(placement.surface)}"
          aria-orientation={placement.dock === "bottom" ? "vertical" : "horizontal"}
          onpointerdown={(e) =>
            boxes[placement.surface] &&
            startResize(e, placement, boxes[placement.surface]!)}
          onpointermove={(e) =>
            boxes[placement.surface] &&
            onResize(e, placement, boxes[placement.surface]!)}
          onpointerup={(e) =>
            boxes[placement.surface] &&
            endResize(e, placement, boxes[placement.surface]!)}
          onpointercancel={(e) =>
            boxes[placement.surface] &&
            endResize(e, placement, boxes[placement.surface]!)}
          ondblclick={() => {
            // Back to the size the panel asks for itself: the quick way out
            // of a size that seemed right at the time.
            const box = boxes[placement.surface];
            box?.style.removeProperty("flex");
            box?.style.removeProperty(alongY(placement) ? "height" : "width");
            void setPlacement(placement.surface, { size: null });
          }}
          title="Drag to resize; double-click for its own size"
        ></div>
      {/if}
      <div class="surface-body" hidden={placement.collapsed && !isLifted(placement)}>
        {#if placement.surface === "library"}{@render surfaceLibrary()}
        {:else if placement.surface === "prepare"}{@render surfacePrepare()}
        {:else if placement.surface === "next"}{@render surfaceNext()}
        {:else if placement.surface === "plan"}{@render surfacePlan()}
        {:else if placement.surface === "pair"}{@render surfacePair()}
        {:else if placement.surface === "practice"}{@render surfacePractice()}
        {:else if placement.surface === "event"}{@render surfaceEvent()}
        {:else if placement.surface === "night"}{@render surfaceNight()}
        {:else if placement.surface === "room"}{@render surfaceRoom()}
        {:else if placement.surface === "requests"}{@render surfaceRequests()}
        {:else if placement.surface === "karaoke"}{@render surfaceKaraoke()}
        {:else if placement.surface === "mixes"}{@render surfaceMixes()}
        {:else if placement.surface === "athand"}{@render surfaceAtHand()}
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

  <!--
    The command palette. Mounted outside the docks because it is not a surface:
    it is a way of reaching everything, including the surfaces themselves, and
    a thing that opens over the whole window has no dock to belong to.
  -->
  <!-- §117: the guide behind Space, drawn over everything while a chain is typed. -->
  <Guide {leader} />
  {#if board}
    <DashboardView
      {board}
      onclose={() => (board = null)}
      onrun={(tile) => {
        board = null;
        void runLeaf(tile.run);
      }}
    />
  {/if}

  <Palette
    bind:this={paletteRef}
    enabled={ready}
    {deckCount}
    onAction={(action) => void send(action)}
    onSurface={(surface) => {
      if ((DRAWN as readonly string[]).includes(surface)) {
        void toggleSurface(surface as Drawn);
      }
    }}
    onSwitch={(run) => void switchTo(run)}
  />

  <div class="cockpit">
  {#if leftDock.length > 0}
    <div class="dock side left" bind:this={dockEls.left} style={dockStyle("left", leftDock)}>
      {#each leftDock as placement (placement.surface)}
        {@render surface(placement)}
      {/each}
    </div>
    {@render dockGrip("left")}
  {/if}

  <div class="middle">
  <div class="stage" class:shared={docked} data-watershed={living ? "open" : "closed"} bind:this={stageEl}>
  {#if snapshot}
    <!--
      The watershed. Above the decks rather than replacing them: it answers
      "how is this going" at a glance, and the panels below answer "what
      exactly" — nature carries the gestalt, digits carry the precision, and
      removing either would be the fastest way to make this design fail in a
      real booth. See docs/VISUAL-LANGUAGE.md.
    -->
    {#if living && rivers.length > 0}
      <div class="watershed-band">
        <Watershed
          {world}
          {tier}
          decks={rivers.map((r) => r.index)}
          latencyMs={snapshot.master.output_latency_ms}
          {accelerate}
          ondriver={(what) => (backend = what)}
        />
        <span class="watershed-close">
          <IconButton
            icon="fa-solid fa-xmark"
            aria-label="Hide the watershed"
            title={`Hide the watershed${backend ? ` (drawing with ${backend})` : ""}`}
            onClick={() => (living = false)}
          />
        </span>
      </div>
    {/if}

    <div class="decks" class:four={deckCount === 4} class:six={deckCount === 6}>
      {#each snapshot.decks.slice(0, deckCount) as deck (deck.number)}
        <!--
          Wrapped rather than given a prop: the outline is about *this window's*
          attention, not about the deck's state, and threading it through Deck
          would put a presentational flag next to the audio ones.
        -->
        <div class="deck-slot" class:looking={focusedDeck === deck.number}>
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
        </div>
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
    {#if !bottomDock.every((p) => p.collapsed)}{@render dockGrip("bottom")}{/if}
    <div class="dock bottom" bind:this={dockEls.bottom} style={dockStyle("bottom", bottomDock)}>
      {#each bottomDock as placement (placement.surface)}
        {@render surface(placement)}
      {/each}
    </div>
  {/if}
  </div>

  {#if overlayDock.length > 0}
    <div class="dock overlay">
      {#each overlayDock as placement (placement.surface)}
        {@render surface(placement)}
      {/each}
    </div>
  {/if}

  {#if rightDock.length > 0}
    {@render dockGrip("right")}
    <div class="dock side right" bind:this={dockEls.right} style={dockStyle("right", rightDock)}>
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
  {#if welcoming}
    <Welcome
      answers={welcoming}
      onPickLogo={() => void chooseLogo()}
      onApplied={welcomed}
      onClose={() => (welcoming = null)}
    />
  {/if}

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
  /*
    A deck the assistant has asked you to look at. An outline rather than a
    colour change: the deck's own colours mean things about the audio, and
    borrowing one of them for "look here" would make the two indistinguishable
    at the far end of a dark booth.
  */
  .deck-slot {
    display: contents;
  }

  .deck-slot.looking :global(.deck) {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }

  .mark.done {
    border-color: var(--accent);
    color: var(--accent);
  }

  /*
    The one control in the row that is coloured when nothing is wrong.

    REC and Mark light up when they are doing something; this is lit all the
    time, because the moment it is wanted is the moment nobody is going to scan
    a row of grey squares for it. §47's whole requirement is "must not search
    through menus", and a control that only announces itself once you have
    found it has not met it.
  */
  .safe {
    border-color: var(--danger);
    color: var(--danger);
    font-weight: 700;
    letter-spacing: 0.06em;
  }

  .safe:hover:not(:disabled) {
    background: var(--danger);
    color: var(--bg);
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

  /*
    §18: notices float over the stage's top edge while the layout must hold
    still, instead of pushing the decks down mid-mix. Centred and narrower than
    the window, so the decks' own edges — where the hands are — stay clear.
  */
  .notices.floating {
    position: absolute;
    left: 50%;
    transform: translateX(-50%);
    width: min(92vw, 56rem);
    z-index: 40;
    filter: drop-shadow(0 6px 18px rgba(0, 0, 0, 0.45));
  }

  /*
    §109: the strip centred in the height it holds, rather than sitting on top
    of an empty band — the band is there so the decks do not move, and should
    read as the row's own padding rather than as something missing.
  */
  .go.holding {
    align-content: center;
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

  /* §117: the slim header's one group — where you are, and the two ways on. */
  .slim {
    flex-wrap: nowrap;
  }

  .slim button {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    padding: 0.25rem 0.55rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-s, 4px);
    background: var(--panel);
    color: var(--text);
    font: inherit;
    font-size: 0.82rem;
    cursor: pointer;
  }

  .slim button:hover {
    background: var(--panel-hover);
  }

  .slim .where {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    padding: 0 0.4rem;
    font-weight: 600;
    font-size: 0.9rem;
    white-space: nowrap;
  }

  .slim kbd {
    padding: 0 0.3em;
    border: 1px solid var(--border-strong, var(--border));
    border-radius: 0.25rem;
    font-family: var(--mono, monospace);
    font-size: 0.72rem;
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
  /*
    The picker and the line beside it.

    Capped and truncated rather than allowed to push the recording controls
    along: "Everything that fits on a small screen, and nothing that does not"
    is a sentence, and the two buttons at the end of this row are the two that
    must never move. The whole sentence is in the `title`.
  */
  .workspace-preset {
    /*
      Wide enough for the longest name a DJ is likely to sit on all night --
      "Laptop Compact", "High Contrast", "Pro Performance". It was 11rem, which
      cut "Laptop Compact" to "Laptop Compa" in the running application. The
      three longest names still truncate and the line beside them says what
      they are.
    */
    max-width: 13rem;
  }

  /*
    Naming an arrangement, in the width the description was using.

    Capped to the same 18rem for the reason that one is: the two buttons at the
    end of this row are the two that must never move, and §39's fourteenth panel
    button is the standing proof that one more control here wraps the line and
    takes forty pixels out of every deck.
  */
  .naming {
    max-width: 12rem;
    padding: 0.25rem 0.45rem;
    border: 1px solid var(--accent);
    border-radius: var(--radius);
    background: var(--panel-raised);
    color: var(--text);
    font: inherit;
    font-size: 0.78rem;
  }

  /* §33: the refusal is a sentence, not a red border. */
  .naming-error {
    max-width: 20rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--danger);
    font-size: 0.78rem;
  }

  .naming-keep,
  .naming-forget {
    padding: 0.25rem 0.5rem;
    font-size: 0.78rem;
    white-space: nowrap;
  }

  .preset-about {
    max-width: 18rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-dim);
    font-size: 0.78rem;
  }

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

  /* Still worn by the split-card drift, which is the one reading left here. */
  .status .xruns {
    color: var(--danger);
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

  /*
    And a floor on the height of each panel in it.

    Flex children shrink to nothing by default, so a third surface in a side
    dock left the one at the bottom showing a single row cut through the
    middle of its letters — found by opening Tonight's mixes with the Night and
    the assistant already there. The dock already scrolls; without a floor it
    never reaches the height that would make it, and squeezes instead. Eight
    rems is about the smallest height at which every side surface is still a
    panel rather than a title bar with a hint of content under it, and it is
    close to the least height `cockpit::Surface` asks for.
  */
  .dock.side > .surface {
    min-height: 8rem;
  }

  /*
    And a collapsed one is exempt from it.

    Found by driving the application: the fold hid the body and the panel kept
    its eight rems, so folding the night left a header above a hand's width of
    empty panel — which is not a fold, it is a blank. The browser test asserted
    the body was hidden and the attribute was set, both of which were true.
  */
  .dock.side > .surface.collapsed,
  .dock.bottom > .surface.collapsed {
    min-height: 0;
    flex: none;
  }

  /*
    A bottom dock whose every surface is folded gives its own floor up too.

    Written as a negated `:has` on purpose: where the selector is not
    understood the rule is dropped and the floor stays, which is the behaviour
    that was there before and the one that keeps a half-open dock from becoming
    a sliver.
  */
  .dock.bottom:not(:has(> .surface:not(.collapsed))) {
    min-height: 0;
    /* The basis as well as the floor: the dock asks for 45% and grows, so
       zeroing only the minimum leaves it exactly as tall as it was. */
    flex: none;
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

  .surface-head h2 {
    /* The title takes the room, so the three controls sit together at the
       right rather than drifting apart as a panel is resized. */
    margin-right: auto;
  }

  .surface-head .shut,
  .surface-head .fold,
  .surface-head .lift,
  .surface-head .pin {
    background: transparent;
    border: none;
    color: var(--text-dim);
    font-size: 1.1em;
    line-height: 1;
    padding: 0 0.3rem;
    cursor: pointer;
  }

  .surface-head .shut:hover,
  .surface-head .fold:hover,
  .surface-head .lift:hover,
  .surface-head .pin:hover {
    color: var(--text);
  }

  .surface-head .lift {
    font-size: 0.72em;
  }

  .surface-head .lift.on {
    color: var(--selected);
  }

  /*
    §120's temporary window: over the stage, above everything on it and
    below every menu, the palette and the guide. `position: fixed` and its
    box come from `liftStyle`, measured off the stage, so the panel covers
    the decks and nothing else -- the master strip and the other panels stay
    where a hand can reach them, which is also where a press puts this back.
  */
  .surface.lifted {
    z-index: 35;
    border-color: var(--selected);
    box-shadow: 0 18px 48px rgba(0, 0, 0, 0.55);
    animation: panel-in var(--motion-enter) var(--ease);
  }

  /*
    §120: the dock's own size, from the gap beside it. Laid over the gap by
    negative margins as wide as itself, so the space between the dock and the
    stage stays exactly the cockpit's gap and the grip is all of it.
  */
  .dock-grip {
    flex: none;
    position: relative;
    z-index: 2;
    touch-action: none;
  }

  .dock-grip.along {
    width: 0.9rem;
    margin-inline: -0.9rem;
    cursor: ew-resize;
  }

  .dock-grip.across {
    height: 0.9rem;
    margin-block: -0.9rem;
    cursor: ns-resize;
  }

  .dock-grip::after {
    content: "";
    position: absolute;
    border-radius: 2px;
    background: transparent;
  }

  .dock-grip.along::after {
    top: 20%;
    bottom: 20%;
    left: calc(50% - 1.5px);
    width: 3px;
  }

  .dock-grip.across::after {
    left: 20%;
    right: 20%;
    top: calc(50% - 1.5px);
    height: 3px;
  }

  .dock-grip:hover::after,
  .dock-grip.dragging::after {
    background: var(--accent);
    opacity: 0.55;
  }

  /*
    Its children float; the dock itself takes no room. And a dock whose every
    panel is lifted gives its room up for as long as they are, so the stage
    grows into it and the lifted panel -- which covers the stage -- grows with
    it. Lifted out of the bottom dock the library had the stage above the
    master strip, about as tall as the dock it left; now it has the dock's
    height as well. `display: contents` rather than a zero size, because a
    zero-sized dock still takes the cockpit's gap on either side of it.
  */
  .dock.overlay,
  .dock:not(:has(> .surface:not(.lifted))) {
    display: contents;
  }


  /* A pin that is in reads as in, and §33's rule applies: the pressed state is
     carried by `aria-pressed` as well as by the colour. */
  .surface-head .pin {
    font-size: 0.7em;
  }

  /*
    §30: a state is painted with the role it means. Pinning is the DJ choosing
    this surface to be held, which is exactly `Role::Selected` — not the
    accent, which is a colour rather than a meaning. An existing token guard
    refused the accent the moment it was written, which is the guard working.
  */
  .surface-head .pin.on {
    color: var(--selected);
  }

  /*
    §3's *resized*. On the edge that faces the performance zone, so the handle
    is where a hand reaches for it, and wide enough to hit in a dark booth
    without being a visible bar down the side of every panel.
  */
  .surface {
    position: relative;
  }

  .surface .grip {
    position: absolute;
    z-index: 5;
    touch-action: none;
  }

  .dock.side .surface .grip {
    left: 0;
    right: 0;
    bottom: 0;
    height: 6px;
    cursor: ns-resize;
  }

  .dock.bottom .surface .grip {
    top: 0;
    bottom: 0;
    right: 0;
    width: 6px;
    cursor: ew-resize;
  }

  .surface .grip:hover {
    background: var(--accent);
    opacity: 0.4;
  }

  /* A collapsed surface is its own header and nothing else. */
  .surface.collapsed {
    flex: none;
    min-height: 0;
  }

  /*
    A collapsed surface's body is hidden with the attribute, and a rule that
    sets `display` wins over it. The reset in the head carries
    `[hidden]{display:none!important}` for exactly this; the shell has its own
    stylesheet and needs its own.
  */
  .surface-body[hidden] {
    display: none;
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

  /* §112: the watershed closes from its own band. */
  .watershed-band {
    position: relative;
  }
  .watershed-close {
    position: absolute;
    top: 4px;
    right: 4px;
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
  }

  /*
    Six decks are three rows of two, not two rows of three, for the same reason
    four are two rows of two: the width of a deck is what decides whether its
    pads are readable, and thirds of a screen are not enough.
  */
  .decks.six {
    grid-template-columns: repeat(2, 1fr);
  }

  /*
    Neither of those sets `grid-auto-rows`, and that is deliberate.

    Both used to say `min-content`, which was written meaning "let the extra
    rows be as tall as they need and scroll". Nothing scrolls them -- the
    stage does not scroll, by design, three comments up -- so what it actually
    did was hand the free space to whichever row was not `min-content`.
    Measured with four decks at 1280x800: row one got **115 px** and row two
    got 433, and row one's 168 px channel strip ran a hundred pixels down into
    the deck below it. With a surface docked it was 22 px against 565.

    Inheriting `minmax(0, 1fr)` from `.decks` gives every row an equal share
    of the stage, whether there are one, two or three of them. Six decks then
    means genuinely short decks, which is honest: each one scrolls inside
    itself, and `Deck.svelte` stops pinning its channel strip when there is
    not room for the strip and the waveform both.
  */

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
