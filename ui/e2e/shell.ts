/**
 * A djmanzo shell running in a plain browser.
 *
 * # Why a stub and not the application
 *
 * The application is the interface *plus* an audio engine, a library, a
 * webview and a machine with a sound card. None of that is what a layout
 * budget is about: the question is where the controls end up when the window
 * is 1280x800, and that is decided entirely by CSS and the markup. So the
 * engine is replaced by a fixed answer to every command, and what is measured
 * is the real interface with real stylesheets doing real layout.
 *
 * # What this cannot tell you
 *
 * **Chromium is not WebKitGTK**, and djmanzo ships on WebKitGTK. Fonts,
 * scrollbar widths and flexbox rounding differ by a few pixels between them, so
 * a number measured here is close to but not identical to the number a DJ sees.
 * That is why {@link SLACK} exists and why the assertions are budgets rather
 * than pixel equalities: this catches a control moving *hundreds* of pixels,
 * which is the failure that has actually happened twice, and it deliberately
 * does not try to catch one moving five.
 */
import type { Page } from "@playwright/test";

/** The size djmanzo opens itself at, from `tauri.conf.json`. */
export const WINDOW = { width: 1280, height: 800 };

/**
 * How far past the fold a control may sit before the test fails.
 *
 * Zero would be the honest budget and a flaky test: Chromium and WebKitGTK
 * disagree by a few pixels on font metrics, and a threshold with no slack turns
 * that disagreement into a failure that says nothing. Sixteen is about a line
 * of text -- big enough to absorb the renderers disagreeing, far too small to
 * absorb a control drifting off the screen.
 */
export const SLACK = 16;

/**
 * The snapshot the application actually sends.
 *
 * Generated from `dj_app::snapshot::Snapshot` by
 * `crates/dj-app/tests/e2e_fixture.rs` and committed beside this file, because
 * the shape belongs to Rust. A fixture written here by hand would be a guess at
 * that shape and would go stale silently: a field added in Rust would leave
 * this measuring an interface drawing a state djmanzo no longer produces, still
 * green, still telling you nothing. The Rust test fails when they diverge.
 */
import snapshot from "./snapshot.json" with { type: "json" };
/**
 * The pad pages, generated from `dj_core::PadPage::ALL` by the same Rust test.
 *
 * Answering this command with `null` -- which is what an unlisted command gets
 * -- draws **no pad zone at all**, and `Deck.svelte` says what that means where
 * it handles the empty case: "a deck missing its whole performance surface with
 * nothing saying so". The budget measured that deck three times before anyone
 * compared it with a screenshot of the running application.
 */
import padPages from "./pad-pages.json" with { type: "json" };
import surfaces from "./surfaces.json" with { type: "json" };
import layers from "./layers.json" with { type: "json" };
/**
 * The five transition styles and what each does, generated from
 * `dj_app::shape` by the same Rust test.
 *
 * Not typed out here, for the reason the golden file's test gives: a
 * hand-written copy of the list is how `vocal drop` came to be performed by
 * the automix and offered by no panel.
 */
import styles from "./styles.json" with { type: "json" };

/**
 * Answers for the commands the shell asks on start-up.
 *
 * Unknown commands resolve to `null` rather than rejecting. A rejection would
 * make this a test of *which* commands the shell happens to call, and that list
 * changes every time a panel is added -- the budget would then fail for a
 * reason with nothing to do with geometry.
 */
const ANSWERS: Record<string, unknown> = {
  pad_pages: padPages,
  list_layouts: [],
  chosen_layout: null,
  layout_folder: null,
  layout_tree: { name: "Test", about: "", tokens: {}, slots: {}, notes: [] },
  widget_catalog: [],
  layout_vocabulary: { slots: [], tokens: [] },
  watershed: false,
  list_devices: [],
  has_logo: false,
  demo_folder: null,
  // Answered, and this one is not decoration: it is why the deck this budget
  // measured had no pad zone for three runs. `Stems.svelte` reads
  // `status.available` straight out of the answer, the application's own type
  // is not optional so it can never be null there, and a `null` from this stub
  // therefore threw inside the deck's own subtree -- taking the pads down with
  // it, silently, while every assertion below stayed green.
  //
  // The `pageErrors` guard added to `openShell` is the general form of that
  // lesson. This entry is the specific one.
  stems_status: { available: true, backend: null, reason: null },
  // The cockpit opens with nothing docked, which is what "Perform" means and
  // what a fresh install gets. `set_cockpit_workspace` is answered by the
  // handler below rather than from this table, because its answer depends on
  // what was asked.
  // What the assistant surface asks for.
  assistant_packs: [],
  list_llm_providers: [],
  learned_taste: { favourites: [], plays: 0, confident: false },
  // §13/§14. Two gestures that reached four occurrences in one phase, with the
  // sentences Rust writes — never "you like", always what was seen and when.
  learned_tendencies: [
    {
      says: "You often ride the EQ when the night is at its peak. Seen 9 times.",
      gesture: "eq-moved",
      phase: "peak",
      seen: 9,
    },
    {
      says: "You sometimes sweep the filter when the night is building. Seen 4 times.",
      gesture: "filter-swept",
      phase: "heat",
      seen: 4,
    },
  ],
  assistant_state: {
    provider: "",
    model: "",
    spent_usd: 0,
    cap_usd: 0,
    unpriced_calls: 0,
  },
  assistant_conduct: {
    posture: "suggest",
    occasion: "open",
    decks_held: [],
    anything_held: false,
    next_step: "",
    because: "",
    mistakes_are_costly: false,
    verbosity: 1,
  },
  room_read: {
    watching: false,
    recent: 0,
    enough: false,
    notes: [],
    disagreement: null,
    hour: null,
    light: null,
    movement: null,
    loudness: null,
  },
  // A night nothing has read yet, which is what a fresh application has. The
  // shape matters more than the values: `Night.svelte` indexes its label
  // tables by `basis` and `warrant`, so a `null` here would throw inside the
  // surface and take the dock with it -- the failure `stems_status` above
  // documents, in a different panel.
  // The waveform's own answer, without which `Waveform.svelte` never leaves
  // its "no tiles yet" state — so every lane in the pair view rendered as an
  // empty box and no browser test had ever seen one, including the ones whose
  // commit message said "each with its waveform". The tiles themselves are
  // `wave://` URLs that resolve to nothing here; what is being measured is the
  // marks and cues drawn over them, which is the part a DJ grabs.
  // 12 000 000 frames is a little over four minutes at 48 kHz. The mix-out
  // window is placed inside it the way Rust would: near the end, a couple of
  // phrases wide, opening on a phrase boundary.
  waveform_info: {
    deck: 1,
    ready: true,
    total_frames: 12_000_000,
    epoch: 1,
    mix_out: { opens_frame: 10_800_000, closes_frame: 11_600_000, on_phrase: true },
  },
  // §25's inventory, answered from the same table Rust publishes. The test
  // that matters reads it back and checks every `data-layer` on screen is in
  // it, so this stub is deliberately the real shape rather than a stand-in.
  waveform_layers: layers,
  // §74's rail, in the shape `dj_app::rail` produces for a deck being got
  // ready: five controls, each an action the parser accepts, with the loop and
  // the sync showing their state.
  at_hand: {
    deck: 2,
    doing: "preparing",
    because: "this record is cued and waiting",
    controls: [
      { label: "cue", action: "deck 2 cue", on: false },
      { label: "mark", action: "deck 2 hotcue_set 1", on: false },
      { label: "loop 4", action: "deck 2 loop 4", on: false },
      { label: "sync", action: "deck 2 sync", on: true },
      { label: "keylock", action: "deck 2 keylock_toggle", on: false },
    ],
  },
  // Two mixes, the shape `dj_app::mixes` derives them in: a blend into a cut,
  // oldest first, because that is the order the log produces and the panel is
  // what reverses it.
  session_mixes: [
    {
      at: 214.0,
      took_seconds: 15.5,
      beats: 32,
      out_deck: 1,
      in_deck: 2,
      out_title: "Bachata Rosa",
      in_title: "Ojalá Que Llueva Café",
      style: "blend",
      kept: 0,
    },
    {
      at: 461.0,
      took_seconds: 1.4,
      beats: null,
      out_deck: 2,
      in_deck: 1,
      out_title: "Ojalá Que Llueva Café",
      in_title: null,
      style: "cut",
      kept: 2,
    },
  ],
  // Nothing staged, which is what a fresh application has. `Staged.svelte`
  // draws nothing at all for this, which is the point: the strip costs the
  // decks no height until there is something to decide.
  staged_current: null,
  authority_matrix: [],
  night_read: {
    phase: null,
    words: null,
    energy: null,
    certainty: null,
    certainty_about: null,
    basis: null,
    drift: null,
    time_of_day: null,
    declared: null,
    measured: null,
    readings: 0,
    still_needed: 90,
    notes: ["Nothing has read the night yet."],
    warrant: "speak",
  },
    // What the presets and settings surfaces ask for.
  //
  // The list is long because `Settings.svelte` is one panel over every
  // preference in the application, and each section asks its own subsystem
  // whether it is there. All of them answer with a struct or a `Vec` in Rust
  // and none can be null, so the stub's `null` default is a throw waiting for
  // whoever first opens the panel under it -- which nothing did until the dock
  // tests started opening every surface in turn.
  list_presets: [],
  preset_folder: null,
  list_panels: [],
  list_sources: [],
  list_inputs: [],
  secrets_persist: true,
  music_library: { folders: [], tracks: 0 },
  stem_out: { deck: null, decks: null, deckCapacity: 6, channels: null },
  remote_status: {
    running: false,
    address: null,
    token_set: false,
    error: null,
    osc: null,
  },
  clock_status: {
    running: false,
    port: null,
    error: null,
    following: null,
    external_bpm: null,
  },
  midi_outputs: { ports: [], unavailable: null },
  control_status: {
    inputs: [],
    open_port: null,
    open_mapping: null,
    unavailable: null,
    keyboard: true,
    keyboard_name: "",
  },
  peer_status: {
    running: false,
    address: null,
    sendTo: null,
    peers: 0,
    peerBpm: null,
    error: null,
  },
  timecode_status: {
    decks: [],
    formats: [],
    engineRunning: false,
    caveat: "",
  },
    // The two the keyboard and log surfaces need. Both are `Vec` in Rust.
  keyboard_keys: [],
  session_log: [],
    // What the library surface asks for the moment it opens.
  //
  // Answered with the *shape* the application sends, empty. `null` is not a
  // shape djmanzo ever produces for any of these -- the Rust types are `Vec`
  // and a struct -- and a component that spreads or maps the answer throws on
  // it, which ends the render pass and takes the rest of the surface with it.
  // That is the `stems_status` bug again, and it is why `errorsThrown` exists.
  list_playlists: [],
  track_functions: [
    { slug: "opener", label: "Opener", about: "Sets the room going from nothing.", count: 0 },
    { slug: "peak", label: "Peak", about: "The top of an arc -- spent, not saved.", count: 0 },
  ],
  sidelist: [],
  sidelist_add: null,
  // Two candidates, so the rail has a ranking and not just a row.
  //
  // §27's ghost, on the same record `waveform_info` above describes: a
  // 32-beat blend starting where the mix-out window opens, with the
  // candidate's first full phrase eight beats into it.
  //
  // Five of §27's seven answered and two not, in the shape and the order Rust
  // sends them. The two that are not are the point of the fixture as much as
  // the five that are: the panel has to say so, and a stub carrying only the
  // answered ones would let it ship silently dropping them.
  ghost_preview: {
    track: "b".repeat(64),
    deck: 1,
    start_frame: 10_800_000,
    end_frame: 11_568_000,
    start_seconds: 225,
    end_seconds: 241,
    length_beats: 32,
    style: "blend",
    bpm_delta: 3,
    pitch_percent: -2.3,
    key_relation: "neighbour",
    landing: { frame: 10_992_000, lead_beats: 8, within_mix: true },
    weakens_from: 10_800_000,
    weakens_to: 11_600_000,
    reasons: ["phrase start (beat 450)", "128 into 131 BPM", "96 beats left"],
    asked: [
      {
        slug: "first-phrase",
        about: "where its first strong phrase would align",
        answered: true,
      },
      { slug: "vocal-entry", about: "where the vocal enters", answered: false },
      { slug: "drop", about: "where the drop occurs", answered: false },
      {
        slug: "outgoing-weakens",
        about: "where the outgoing track becomes weak",
        answered: true,
      },
      { slug: "overlap", about: "likely transition overlap", answered: true },
      { slug: "key-relation", about: "key relationship", answered: true },
      { slug: "tempo-movement", about: "BPM movement", answered: true },
    ],
  },
  // The shape `suggest_next` and `similar_to` both return, including the
  // `summary` line the rail actually renders -- a fixture that carried only
  // `reasons` would have let the rail ship showing nothing and passed.
  suggest_next: [
    {
      track: {
        id: "b".repeat(64),
        path: "/music/ojala-que-llueva-cafe.flac",
        title: "Ojal\u00e1 Que Llueva Caf\u00e9",
        artist: "Juan Luis Guerra",
        album: null,
        genre: "Merengue",
        year: 1989,
        duration_seconds: 262,
        bpm: 127,
        key: "9A",
        loudness_lufs: -7.2,
        analysed: true,
        play_count: 0,
        rating: null,
        colour: null,
      },
      score: 7.9,
      reasons: ["harmonic (9A)", "127 BPM fits", "+1 dB"],
      summary: "+3 BPM \u00b7 8A\u21929A \u00b7 +1 dB",
      confidence: 0.94,
    },
    {
      track: {
        id: "c".repeat(64),
        path: "/music/burbujas-de-amor.flac",
        title: "Burbujas de Amor",
        artist: "Juan Luis Guerra",
        album: null,
        genre: "Bachata",
        year: 1990,
        duration_seconds: 279,
        bpm: 118,
        key: "3B",
        loudness_lufs: -11.0,
        analysed: true,
        play_count: 0,
        rating: null,
        colour: null,
      },
      score: 1.2,
      reasons: ["key clash (3B)", "118 BPM fits", "-3 dB"],
      summary: "-6 BPM \u00b7 8A\u21923B clash \u00b7 -3 dB",
      confidence: 0.55,
    },
  ],
  similar_to: [],
  // What the pair view is handed: two records and the seam between them.
  //
  // §68's transition object, in the shape `dj_app::commands::TransitionDto`
  // serialises. Nothing is held at start-up -- `transition_current` answers
  // null until something is set up -- because that is the state a DJ opening
  // the panel is actually in, and it is the state in which the empty message
  // has to be right.
  plan_transition: {
    outgoing: {
      deck: 1,
      track: {
        id: "a".repeat(64),
        path: "/music/bachata-rosa.flac",
        title: "Bachata Rosa",
        artist: "Juan Luis Guerra",
        album: null,
        genre: "Bachata",
        year: 1990,
        duration_seconds: 244,
        bpm: 124,
        key: "8A",
        loudness_lufs: -9.4,
        analysed: true,
        play_count: 0,
        rating: null,
        colour: null,
      },
      phrase_beats: 16,
      key_standard: "Am",
      functions: ["opener"],
    },
    incoming: {
      deck: 2,
      track: {
        id: "b".repeat(64),
        path: "/music/ojala-que-llueva-cafe.flac",
        title: "Ojal\u00e1 Que Llueva Caf\u00e9",
        artist: "Juan Luis Guerra",
        album: null,
        genre: "Merengue",
        year: 1989,
        duration_seconds: 262,
        bpm: 127,
        key: "9A",
        loudness_lufs: -7.2,
        analysed: true,
        play_count: 0,
        rating: null,
        colour: null,
      },
      phrase_beats: 16,
      key_standard: "Em",
      functions: [],
    },
    start_beat: 320,
    start_seconds: 154.8,
    end_seconds: 170.3,
    start_frame: 6_830_000,
    end_frame: 7_513_000,
    length_beats: 32,
    style: "blend",
    bpm_delta: 3,
    key_relation: "neighbour",
    confidence: 0.82,
    edited: false,
    armed: false,
    reasons: [
      "phrase start (beat 320)",
      "124 into 127 BPM",
      "keys sit together",
      "88 beats left",
    ],
    // The blend's own shape, taken from the same generated table rather than
    // copied -- so a restyle below can look the new one up and the panel says
    // what the new style actually does.
    shape: styles.find((style) => style.name === "blend")?.shape,
  },
  transition_styles: styles,
  // §76's lens, answered per row below so a test can tell the columns apart.
  library_lens: [],
  // §29's gestures, answered per deck below so a test can prove the menu acts
  // on the deck it was opened on.
  control_handles: [],
  // §31. Steady by default — which is what djmanzo answers on almost every
  // tick, and is the whole point of the section.
  theme_now: { theme: "pkg-organic", over_ms: 0, locked: false },
  theme_lock: null,
  theme_chosen: null,
  // §81. Tonight opens unnamed, which is the state the picker exists to end —
  // and the state in which the hint has to be right.
  night_now: {
    setting: null,
    density: null,
    style: null,
    posture: null,
    techniques: [],
  },
  // Three wedding nights and three club nights, which is what §81 is about:
  // the same DJ, two different answers, and never their average.
  learned_profiles: [
    {
      setting: "wedding",
      title: "Wedding",
      nights: 3,
      density: "Relaxed",
      style: "fade",
      automation: "prepare",
      techniques: ["eq-moved"],
      genres: [
        ["Bachata", 0.7],
        ["Merengue", 0.3],
      ],
      says: "Wedding, over 3 nights: mostly fade transitions, 70% Bachata, assistant on prepare.",
    },
    {
      setting: "club",
      title: "Club",
      nights: 4,
      density: "Pro Dense",
      style: "blend",
      automation: "suggest",
      techniques: ["looped", "filter-swept"],
      genres: [["Techno", 1]],
      says: "Club, over 4 nights: mostly blend transitions, 100% Techno, assistant on suggest.",
    },
  ],
  // A rehearsal, in the shape `dj_app::commands::RehearsalDto` serialises. The
  // stub answers per style below, because the whole of what this panel does is
  // tell four renders of one pair apart.
  practice_rehearse: null,
  transition_current: null,
  transition_clear: null,
  // The palette's answer, which Rust ranks. Two actions and one surface, so a
  // test can prove each kind runs the right way -- and the first entry is the
  // typed-action tier, which is what makes the palette more than a menu.
  palette: [
    {
      label: "Run: deck 2 loop 8",
      about: "The vocabulary accepts this exactly as typed.",
      kind: "action",
      run: "deck 2 loop 8",
    },
    {
      label: "Deck 1 \u00b7 play",
      about: "start playback",
      kind: "action",
      run: "deck 1 play",
    },
    {
      label: "Show Prepare",
      about: "Records on their way to a deck, before they are on one.",
      kind: "surface",
      run: "prepare",
    },
  ],
  learned_taste: { favourites: [], plays: 0 },
  // A three-record plan whose middle seam needs a cut.
  //
  // Two easy joins and one difficult one, because a fixture in which every
  // seam is fine cannot tell a Set Flow that draws risk from one that draws
  // nothing.
  setlist_build: [
    {
      track: {
        id: "d".repeat(64),
        path: "/music/opener.flac",
        title: "A Pedir Su Mano",
        artist: "Juan Luis Guerra",
        album: null,
        genre: "Merengue",
        year: 1989,
        duration_seconds: 240,
        bpm: 124,
        key: "8A",
        loudness_lufs: -10.0,
        analysed: true,
        play_count: 0,
        rating: null,
        colour: null,
      },
      through: 0,
      trajectory: "lift",
      reasons: [],
      link: null,
    },
    {
      track: {
        id: "e".repeat(64),
        path: "/music/middle.flac",
        title: "La Bilirrubina",
        artist: "Juan Luis Guerra",
        album: null,
        genre: "Merengue",
        year: 1990,
        duration_seconds: 250,
        bpm: 127,
        key: "9A",
        loudness_lufs: -8.0,
        analysed: true,
        play_count: 0,
        rating: null,
        colour: null,
      },
      through: 0.5,
      trajectory: "lift",
      reasons: ["harmonic (9A)"],
      link: { summary: "+3 BPM \u00b7 8A\u21929A \u00b7 +2 dB", confidence: 0.91, risky: false },
    },
    {
      track: {
        id: "f".repeat(64),
        path: "/music/closer.flac",
        title: "Visa Para Un Sue\u00f1o",
        artist: "Juan Luis Guerra",
        album: null,
        genre: "Techno",
        year: 1990,
        duration_seconds: 230,
        bpm: 174,
        key: "3B",
        loudness_lufs: -6.0,
        analysed: true,
        play_count: 0,
        rating: null,
        colour: null,
      },
      through: 1,
      trajectory: "hold",
      reasons: ["key clash (3B)"],
      link: { summary: "+47 BPM stretch \u00b7 9A\u21923B clash", confidence: 0.18, risky: true },
    },
  ],
  // One record, so a test can press a gesture *on* something.
  //
  // An empty library is the state in which every row-level gesture is
  // trivially fine, which is why breaking the browser's "set aside" button
  // failed nothing: there was no row to press it on. A fixture with one track
  // is the difference between testing the panel and testing the workflow.
  library_search: [
    {
      id: "a".repeat(64),
      path: "/music/bachata-rosa.flac",
      title: "Bachata Rosa",
      artist: "Juan Luis Guerra",
      album: null,
      genre: "Bachata",
      year: 1990,
      duration_seconds: 244,
      bpm: 124,
      key: "8A",
      loudness_lufs: null,
      analysed: true,
      play_count: 0,
      rating: null,
      colour: null,
    },
    // A second record, rated, so a test can tell a favourite from one that is
    // not — and so the card grid has more than one cell to lay out.
    {
      id: "b".repeat(64),
      path: "/music/ojala.flac",
      title: "Ojalá Que Llueva Café",
      artist: "Juan Luis Guerra",
      album: "Ojalá Que Llueva Café",
      genre: "Merengue",
      year: 1989,
      duration_seconds: 262,
      bpm: 138,
      key: "11B",
      loudness_lufs: null,
      analysed: true,
      play_count: 3,
      rating: 5,
      colour: null,
    },
  ],
  default_music_folder: null,
  library_status: {
    tracks: 0,
    pending: 0,
    failed: [],
    folders: [],
    identified: 0,
    working: false,
    path: null,
  },
  // The real bands, so the harness measures the interface a DJ gets at this
  // window size rather than one at a density nothing chooses.
  // Copied from `dj_app::cockpit::BANDS`, and kept honest by a Rust test that
  // reads this file: `the_harness_and_rust_agree_about_the_bands`. A stub that
  // answers with numbers Rust no longer holds is a harness measuring an
  // application that does not exist, and this table has moved twice.
  density_bands: [
    [1500, "Relaxed", 1.15],
    [1130, "Standard", 1.0],
    [1060, "Compact", 0.92],
    [1020, "Pro Dense", 0.86],
    [0, "Ultra Dense", 0.8],
  ],
    cockpit_surfaces: surfaces,
  cockpit_workspace: {
    workspace: {
      name: "Perform",
      about: "",
      surfaces: [],
      density: "standard",
      focus: "performing",
      theme: "",
      decks: 2,
      frozen: false,
    },
    notes: [],
  },
};

/**
 * Install the stub and open the shell at the window size djmanzo opens at.
 *
 * `addInitScript` rather than a route handler, because the stub has to be in
 * place before the bundle's first `invoke` -- which happens during module
 * evaluation, not after load.
 */
export async function openShell(
  page: Page,
  url: string,
  /**
   * Fields to change in the fixture's `master` before it is delivered.
   *
   * For branches the engine really has and the captured state happens not to
   * be in -- a four-channel cue device, say. Deliberately narrow: it varies a
   * flag the application sets, it does not let a test invent a shape the
   * application never sends.
   */
  master: Record<string, unknown> = {},
  /**
   * Answers to change before the shell starts.
   *
   * The same discipline `master` follows: it varies an answer djmanzo really
   * gives, so a test can measure the panel in a state the captured fixture
   * happens not to be in — a night the evidence disputes, say. It does not let
   * a test invent a shape the application never sends.
   */
  answers: Record<string, unknown> = {},
) {
  const state = { ...snapshot, master: { ...snapshot.master, ...master } };
  const table = { ...ANSWERS, ...answers };
  const thrown: string[] = [];
  pageErrors.set(page, thrown);
  page.on("pageerror", (error) => thrown.push(error.message));
  await page.setViewportSize(WINDOW);
  await page.addInitScript(
    ([answers, state]: [Record<string, unknown>, unknown]) => {
      const handlers = new Map<string, number>();
      const win = window as unknown as Record<string, unknown>;
      // Exposed so a test can deliver an event djmanzo would emit — the
      // `cockpit` one especially, which is the only way an operation the DJ
      // did not press ever reaches this window.
      win.__handlers = handlers;

      /**
       * Hand the snapshot to whoever is listening for it.
       *
       * The shell subscribes with `listen("snapshot", ...)`, so the state has
       * to arrive the way the application sends it -- through the event
       * channel -- rather than as an answer to a command. Delivering it as a
       * command would measure a code path that does not exist.
       */
      const deliver = () => {
        const id = handlers.get("snapshot");
        if (id === undefined) return;
        const handler = win[`_${id}`] as
          | ((event: unknown) => void)
          | undefined;
        handler?.({ event: "snapshot", id: 0, payload: state });
      };

      win.__TAURI_INTERNALS__ = {
        invoke: (cmd: string, args: Record<string, unknown>) => {
          // A record of what the interface asked for, so a stub that answers
          // the wrong shape can be told apart from one never asked at all.
          ((win.__asked ??= []) as string[]).push(cmd);
          if (cmd === "plugin:event|listen") {
            handlers.set(String(args.event), args.handler as number);
            // After the promise settles, so the shell has finished wiring up.
            setTimeout(deliver, 0);
            return Promise.resolve(1);
          }
          if (cmd === "plugin:event|unlisten") return Promise.resolve(null);
          // Echoed rather than tabulated: the application stores what this
          // hands back and draws that, so a stub returning a fixed answer
          // would make every dock test measure the fixture instead of the
          // press. Rust's resolver is tested in Rust; what matters here is
          // that the round trip carries the arrangement.
          // Every action the interface sends, in order. Recorded rather than
          // counted: `__asked` holds command *names*, and what matters about a
          // control on §74's rail is the exact action text it dispatched — a
          // rail whose buttons all reached the bus with the wrong argument
          // would look identical to one that worked.
          if (cmd === "dispatch") {
            ((win.__dispatched ??= []) as string[]).push(String(args.action));
            return Promise.resolve(null);
          }
          // The mix re-render. Answered here rather than from the table so a
          // test can check *which* mix was asked for: the panel holds two
          // numbers per row and passing the wrong row's would produce a
          // plausible file of the wrong twenty seconds.
          // §24's keep. Answered here rather than from the table so a test
          // can check *which* mix was kept: the panel holds a timestamp per
          // row, and keeping the wrong row would store a plausible pair of the
          // wrong two records.
          if (cmd === "keep_mix") {
            win.__keptAt = args.at;
            return Promise.resolve(1);
          }
          if (cmd === "session_render_mix") {
            win.__renderMixArgs = { at: args.at, tookSeconds: args.tookSeconds };
            return Promise.resolve(
              `20s → /home/dj/.config/djmanzo/recordings/mix-at-${Math.round(
                Number(args.at),
              )}s.wav`,
            );
          }
          if (cmd === "set_cockpit_workspace") {
            // Recorded as well as echoed. What a test needs to know about a
            // save is that it *happened and carried the right thing* — an
            // arrangement drawn on screen and never written looks identical
            // until the application is reopened, which is the defect §89's
            // four-deck configuration found.
            ((win.__saved ??= []) as unknown[]).push(args.workspace);
            return Promise.resolve({ workspace: args.workspace, notes: [] });
          }
          // The transition object, held between calls the way djmanzo holds
          // it. Answered here rather than from the table above for the same
          // reason the workspace is: the panel draws what comes back, so a
          // fixed answer would make a press that changes the mix look
          // identical to one that does nothing, and the wiring is the whole of
          // what a browser test can prove. The arithmetic is Rust's, and is
          // tested there.
          if (
            cmd === "transition_arm" ||
            cmd === "transition_adjust" ||
            cmd === "transition_current" ||
            cmd === "transition_replan" ||
            cmd === "transition_clear"
          ) {
            type Held = Record<string, unknown>;
            const planned = answers.plan_transition as Held;
            if (cmd === "transition_arm") {
              win.__transition = { ...planned, armed: true };
            } else if (cmd === "transition_replan") {
              win.__transition = win.__transition
                ? { ...planned, armed: true }
                : null;
            } else if (cmd === "transition_clear") {
              win.__transition = null;
            } else if (cmd === "transition_adjust" && win.__transition) {
              const held = win.__transition as Held;
              // Moved in frames as well as in beats, because that is what
              // djmanzo answers with and what the waveform draws. A stub that
              // moved only the beat index left the mark sitting exactly where
              // it was, so a drag that worked and a drag that did nothing
              // looked identical — which is the whole thing this is here to
              // tell apart.
              const beats = Number(args.moveBeats ?? 0);
              const beatFrames =
                ((held.end_frame as number) - (held.start_frame as number)) /
                (held.length_beats as number);
              // A restyle brings the new style's shape with it, the way
              // `describe_transition` derives it. Carrying the old shape would
              // leave the panel describing a blend's bass swap under the word
              // "cut", which is the one thing this row exists to tell you.
              //
              // Read out of the answers table rather than off the import at
              // the top of this file: this function is serialised into the
              // browser, where a binding from this module's scope is a
              // ReferenceError that surfaces as an adjustment doing nothing.
              const restyled = (args.style ?? held.style) as string;
              const offered = (answers.transition_styles ?? []) as {
                name: string;
                shape: unknown;
              }[];
              win.__transition = {
                ...held,
                length_beats: args.lengthBeats ?? held.length_beats,
                style: restyled,
                shape:
                  offered.find((style) => style.name === restyled)?.shape ??
                  held.shape,
                start_beat: (held.start_beat as number) + beats,
                start_frame: (held.start_frame as number) + beats * beatFrames,
                end_frame: (held.end_frame as number) + beats * beatFrames,
                edited: true,
              };
            }
            return Promise.resolve(win.__transition ?? null);
          }
          // §29's control handles. Answered per deck rather than fixed above,
          // because the one thing worth proving in a browser is that a menu
          // opened on deck 2 sends actions about deck 2 — a menu that acted on
          // whichever deck is the accident a DJ cannot risk mid-mix.
          if (cmd === "control_handles") {
            const n = Number(args.deck ?? 1);
            const of = (verb: string, unity: string, options: [string, string][]) => ({
              control: verb,
              reset: `deck ${n} ${verb} ${unity}`,
              fine: 0.25,
              options,
            });
            return Promise.resolve([
              of("eq_low", "1", [
                ["Kill", `deck ${n} eq_low 0`],
                ["Unity", `deck ${n} eq_low 1`],
                ["Full", `deck ${n} eq_low 4`],
              ]),
              of("eq_mid", "1", [["Unity", `deck ${n} eq_mid 1`]]),
              of("eq_high", "1", [["Unity", `deck ${n} eq_high 1`]]),
              of("filter", "0", [
                ["Off", `deck ${n} filter 0`],
                ["Low-pass", `deck ${n} filter -0.6`],
              ]),
            ]);
          }
          // §76's lens: one row per id it is handed, and *only* the ids it is
          // handed. Answered here rather than fixed above because the whole
          // claim of the lens is that it is about the rows already on screen —
          // a fixed answer could not tell that apart from a lens that queried
          // the library itself.
          if (cmd === "library_lens") {
            const ids = (args.tracks ?? []) as string[];
            return Promise.resolve(
              ids.map((id, n) => ({
                track: id,
                // The first row has an opinion; the second has none, so a test
                // can prove a blank is drawn as a blank rather than as zero.
                likely_next: n === 0 ? 0.82 : null,
                affinity: n === 0 ? 0.55 : null,
                phase_fit: n === 0 ? 1 : null,
                risks: n === 0 ? [["keys", "keys clash"]] : [],
                novelty: n === 0 ? 1 : 0.25,
                familiarity: n === 0 ? 0 : 1,
                functions: n === 0 ? ["peak"] : [],
              })),
            );
          }
          // §81's setting, held between calls the way djmanzo holds it. A
          // fixed answer would make naming the night look identical to not
          // naming it, and the whole of what a browser can prove here is that
          // the press reaches Rust and the answer comes back.
          if (cmd === "night_setting") {
            const before = (win.__night ?? {
              setting: null,
              density: null,
              style: null,
              posture: null,
              techniques: [],
            }) as Record<string, unknown>;
            win.__night = {
              ...before,
              // An absent setting never overwrites a named one — the rule
              // `Library::note_night` enforces, mirrored so the panel's own
              // refresh cannot look like it is un-saying the DJ's answer.
              setting: args.setting ?? before.setting,
              density: args.density ?? before.density,
            };
            return Promise.resolve(win.__night);
          }
          if (cmd === "night_now") {
            return Promise.resolve(win.__night ?? answers.night_now);
          }
          // A rehearsal, answered per style. Fixed answers would make four
          // renders of the same pair indistinguishable, and telling them apart
          // is the whole of what the practice panel is for.
          if (cmd === "practice_rehearse") {
            const offered = (answers.transition_styles ?? []) as {
              name: string;
              shape: unknown;
            }[];
            const held = win.__transition as Record<string, unknown> | null;
            if (!held) {
              return Promise.reject(
                new Error("set a transition up in the pair view first"),
              );
            }
            const style = (args.style ?? held.style) as string;
            return Promise.resolve({
              style,
              path: `/recordings/practice/aaaaaaaa-bbbbbbbb-${style.replace(" ", "-")}.wav`,
              seconds: 27.5,
              mix_from: 8,
              mix_to: 23.5,
              actions: style === "cut" ? 12 : 964,
              shape: offered.find((s) => s.name === style)?.shape ?? null,
            });
          }
          // The staged transaction, held between calls the way djmanzo holds
          // it — the same reasoning as the transition above. Accept and
          // Modify both change what comes back, and a fixed answer would make
          // a press that changes the plan look identical to one that does
          // nothing. What Rust does with the plan is tested in Rust.
          if (
            cmd === "staged_prepare" ||
            cmd === "staged_current" ||
            cmd === "staged_choose" ||
            cmd === "staged_reject" ||
            cmd === "staged_accept"
          ) {
            type Plan = { moves: Record<string, unknown>[] } & Record<string, unknown>;
            if (cmd === "staged_prepare") {
              win.__staged = JSON.parse(
                JSON.stringify(answers.staged_prepare ?? null),
              );
            } else if (cmd === "staged_reject") {
              win.__staged = null;
              return Promise.resolve(null);
            } else if (cmd === "staged_accept") {
              const held = win.__staged as Plan | null;
              const done = (held?.moves ?? [])
                .filter((m) => m.chosen && m.allowance !== "no")
                .map((m) => String(m.about));
              win.__staged = null;
              return Promise.resolve({ done, stopped: null });
            } else if (cmd === "staged_choose" && win.__staged) {
              const held = win.__staged as Plan;
              const move = held.moves[Number(args.index)];
              if (move && move.allowance !== "no") move.chosen = Boolean(args.chosen);
            }
            return Promise.resolve(win.__staged ?? null);
          }
          return Promise.resolve(answers[cmd] ?? null);
        },
        transformCallback: (callback: unknown) => {
          const id = Math.floor(Math.random() * 1e9);
          win[`_${id}`] = callback;
          return id;
        },
        unregisterCallback: () => {},
        convertFileSrc: (path: string) => path,
      };
    },
    [table, state] as [Record<string, unknown>, unknown],
  );

  await page.goto(url);
  // Waiting for the crossfader is waiting for the thing being measured, rather
  // than for a network idle that says nothing about whether it rendered.
  await page.getByRole("slider", { name: "Crossfader" }).waitFor();
}

/**
 * What each page threw while it was being measured.
 *
 * Kept per page rather than in a module variable because Playwright runs the
 * tests in parallel, and a shared list would attribute one test's error to
 * another's assertion.
 */
const pageErrors = new WeakMap<Page, string[]>();

/**
 * Errors the interface threw, for a test to refuse.
 *
 * This exists because of a specific failure worth not repeating. The stub
 * answers a command it does not know with `null`, deliberately -- rejecting
 * would make this a test of *which* commands the shell happens to call. But
 * `null` is a shape the application never sends, so a component that reads a
 * field off the answer throws; Svelte then abandons the rest of that render
 * pass, and a whole zone of the deck is simply absent. Every geometry
 * assertion still passes, because a shorter deck is not a taller one.
 *
 * That cost three runs of measuring a deck with no pad zone and a note in the
 * documentation admitting the figures were floors. A thrown error is the one
 * signal that distinguishes "this layout is fine" from "this layout did not
 * finish", so it is now a failure rather than a line in the console.
 */
export function errorsThrown(page: Page): string[] {
  return pageErrors.get(page) ?? [];
}

/**
 * Whether a control is inside the region a DJ can actually see and touch.
 *
 * **A page coordinate is not reachability, and the difference cost a green
 * suite against a broken screen.** The master strip was pinned to the bottom
 * of the window and the decks kept scrolling behind it, so a deck's own Volume
 * fader reported y 679 on an 800 px window and passed -- while in the running
 * application it was underneath the pinned strip, unreachable. A screenshot
 * showed it; the assertion could not.
 *
 * Geometry against the scroll container rather than hit-testing, and that is
 * deliberate. `elementFromPoint` sounds like the right tool and is not: these
 * controls are an `<input type="range">` for the keyboard and a screen reader
 * with an SVG drawn over it, so the point hits the SVG and the input is
 * "covered" by its own picture. Two attempts at a hit-test predicate each
 * reported a different pair of these four controls as unreachable, and neither
 * agreed with the screenshot.
 *
 * What is asked instead is whether the control's box lies inside the box of
 * the thing that clips it: the window for something pinned, and the scrolling
 * stage for anything on a deck.
 */
export async function within(
  page: Page,
  container: string,
  role: "slider" | "button" | "group",
  name: string,
): Promise<boolean> {
  const found = page.getByRole(role, { name, exact: true }).first();
  if ((await found.count()) === 0) return false;
  const box = await found.boundingBox();
  if (!box) return false;
  const clip =
    container === "window"
      ? { x: 0, y: 0, width: WINDOW.width, height: WINDOW.height }
      : await page.locator(container).first().boundingBox();
  if (!clip) return false;
  return (
    box.y + box.height <= clip.y + clip.height + SLACK &&
    box.y >= clip.y - SLACK &&
    box.x + box.width <= clip.x + clip.width + SLACK
  );
}

/**
 * The centre of a control, or null when it is not on the page at all.
 *
 * The *centre* rather than the top edge: a fader whose top is on screen and
 * whose thumb is not is a fader a DJ cannot use, and the top edge would pass.
 *
 * **A page coordinate, not a claim about reachability.** Use `reachable`
 * above for that; see the note there for what this alone failed to catch.
 */
export async function centreOf(
  page: Page,
  role: "slider" | "button" | "group",
  name: string,
): Promise<{ x: number; y: number } | null> {
  const found = page.getByRole(role, { name, exact: true }).first();
  if ((await found.count()) === 0) return null;
  const box = await found.boundingBox();
  if (!box) return null;
  return { x: box.x + box.width / 2, y: box.y + box.height / 2 };
}
