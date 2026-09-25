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
/**
 * §109's activities, generated from `dj_app::activity::shipped` by the same
 * Rust test as the rest. The strip draws what Rust ships, keys and all.
 */
import activities from "./activities.json" with { type: "json" };
/** §54's functional presets, generated from `dj_app::setup::ALL` by the same
 *  Rust test. */
import setups from "./setups.json" with { type: "json" };
/** §16's knowledge packs, generated from `dj_assistant::pack::ALL` and the
 *  technique catalogue by the same Rust test. The `teaches` count comes from
 *  the other side of the workspace, so a hand-written stub would report a
 *  number nothing computes. */
import packs from "./packs.json" with { type: "json" };
/** §20's library columns, generated from `dj_app::columns::Column::ALL` by the
 *  same Rust test. Hand-written here they drifted: fifteen entries under a
 *  comment claiming fourteen, and a sixteenth the picker in these tests could
 *  not offer. */
import columns from "./columns.json" with { type: "json" };
/** §81's six kinds of night, generated from `dj_app::setting::Setting::ALL` by
 *  the same Rust test. Hand-written here they would be the copy the panel was
 *  just relieved of, one file further out. */
import nightSettings from "./night-settings.json" with { type: "json" };
/** §32's themes, generated from `dj_app::theme::ALL` by the same Rust test.
 *  The picker draws the ones that ship from the interface's own package list;
 *  this is the other half, the nine §32 asked for and djmanzo has not built. */
import themeRows from "./themes.json" with { type: "json" };
/** §8's seven, generated from `dj_app::level::Level::ALL` by the same Rust
 *  test. One press on this axis writes the posture and all six of §79's locks,
 *  so a hand-written stub would let the browser check that press against a
 *  range djmanzo no longer has. */
import levelRows from "./levels.json" with { type: "json" };
/** §48's seven at Eco, generated from `dj_app::thrift::Spend::ALL` by the same
 *  Rust test. Eco rather than Ultra because Eco is where the claim is: at Ultra
 *  the list is all kept and would pass whatever the ordering said. */
import spendRows from "./spends.json" with { type: "json" };
import layers from "./layers.json" with { type: "json" };
import stores from "./stores.json" with { type: "json" };
import share from "./share.json" with { type: "json" };
/** §117's tree behind Space, generated by the same Rust test from `dj_app::leader::tree`. */
import leaderTree from "./leader.json" with { type: "json" };
/** §117's dashboards, for no activity and for two, generated by Rust from `dj_app::dashboard::build`. */
import dashboards from "./dashboards.json" with { type: "json" };
/**
 * §5B's two deck compositions, resolved, generated by the same Rust test.
 *
 * The tree is what a deck is built from, so a hand-written one would be a third
 * description of a deck after `layout::builtin()` and `widgets::from_layout` --
 * and the first of the three to drift.
 */
import compositions from "./compositions.json" with { type: "json" };
/**
 * §118's event panel: Rust's own answers for a half-prepared wedding, the same
 * wedding after its first idea is taken, and a fresh event -- written by
 * `tests/e2e_fixture.rs` from `dj_app::gig`, because what a step lacks and
 * which ideas a night is offered are rules only Rust runs.
 */
import events from "./events.json" with { type: "json" };
/**
 * §118b's welcome: Rust's own plan for one DJ, the fresh answers a first run
 * starts from, and what the set-up hands back -- written by
 * `tests/e2e_fixture.rs` from `dj_app::welcome`.
 */
import welcome from "./welcome.json" with { type: "json" };
/**
 * §118a's words, as Rust says them (`tests/e2e_fixture.rs`); the records a
 * decision offers are the rail's fixture below, one per direction.
 */
import decide from "./decide.json" with { type: "json" };
/** §118a's guides, as Rust offers them in four booths (`tests/e2e_fixture.rs`). */
import guides from "./guides.json" with { type: "json" };
/** §118d's press kit, as Rust answers it (`tests/e2e_fixture.rs`). */
import kit from "./kit.json" with { type: "json" };
/**
 * §40's twenty-six and which half the assistant sees, generated from
 * `dj_app::sight::ALL` by the same Rust test.
 *
 * The unseen half especially: a hand-written copy would go on telling a DJ the
 * assistant cannot see something it now can, which is the failure the panel
 * exists to prevent, one file further out.
 */
import sight from "./sight.json" with { type: "json" };
export { sight as sightRows };
export { compositions };
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
export const ANSWERS: Record<string, unknown> = {
  pad_pages: padPages,
  assistant_sight: sight,
  /**
   * The deck compositions that ship, as `layout::builtin()` gives them.
   *
   * Not empty any more: §5B lets an arrangement name one, and a shell that
   * looked the name up in an empty list would skip it and look exactly like a
   * shell that honoured it.
   */
  list_layouts: [
    {
      name: "Performance",
      description: "Maximum control density for a controller-driven set.",
      decks: 4,
      waveform_height: 72,
      overview: true,
      progress: true,
      pads: true,
      loops: true,
      fx: true,
      beat_jump: true,
      filter: true,
      keylock: true,
      browser: false,
      density: 0.85,
    },
    // The reduction §5B's supervisory mode names. Listed with the rest because
    // a shell that looked the name up in a list without it would skip the
    // composition and look exactly like a shell that honoured it.
    {
      name: "Starter",
      description: "Two decks and big waveforms. Everything you need and nothing else.",
      decks: 2,
      waveform_height: 160,
      overview: true,
      pads: false,
      loops: false,
      fx: false,
      beat_jump: false,
      filter: false,
      keylock: false,
      browser: false,
      density: 1.1,
      jog: 70,
      stems_open: false,
    },
    // §5B's two, which the cockpit's arrangements now name. A harness that
    // listed neither would let a preset naming one pass by skipping it, which
    // looks exactly like a preset that honoured it.
    {
      name: "Scratch",
      description: "Big platters and a tall waveform, for hands on the records.",
      decks: 2,
      waveform_height: 140,
      overview: true,
      pads: true,
      loops: true,
      fx: false,
      beat_jump: false,
      filter: true,
      keylock: true,
      browser: false,
      density: 1,
      jog: 200,
      stems_open: false,
    },
    {
      name: "Stem Performance",
      description: "The four parts of each record, open and to hand.",
      decks: 2,
      waveform_height: 140,
      overview: true,
      pads: true,
      loops: true,
      fx: true,
      beat_jump: true,
      filter: true,
      keylock: true,
      browser: false,
      density: 1,
      jog: 70,
      stems_open: true,
    },
  ],
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
  event_options: events.options,
  list_events: events.list,
  event_view: events.wedding,
  new_event: events.fresh,
  take_event_idea: events.taken.view,
  // No night being played unless a test says so: a Tonight line in every
  // test's top bar would be measured by every layout budget.
  live_event: null,
  event_tonight: events.tonight,
  // Seen, so the welcome stays shut in every test that is not about it; a
  // first run is a test saying `seen: false`.
  welcome_state: { seen: true, answers: welcome.fresh },
  welcome_plan: welcome.plan,
  welcome_applied: welcome.applied,
  welcome_refused: welcome.refused,
  // §118a: a decision's words, carried into the page with the rest.
  decision_words: decide,
  // A quiet booth: every guide says why not, so no test that is not about
  // them finds one open.
  guides: guides.quiet,
  // §118d: the whole kit; a test about a first kit says `kit_view: kit.fresh`.
  kit_view: kit.full,
  kit_compose: kit.wedding,
  kit_refused: kit.refused,
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
    // §35's baseline. Empty and null, because the fixture is a room nothing
    // has looked at — but present, because `RoomSense.svelte` reads
    // `.baseline.length`, and an absent array is the `stems_status` failure
    // above in a different panel.
    baseline: [],
    phase: null,
  },
  /**
   * §5's Mission Bar, at its emptiest.
   *
   * The bar `dj_app::mission` builds from a reading with nothing in it: a room
   * nobody is watching, a clean bus, no sound card and an idle machine. A Rust
   * test (`the_harness_and_rust_agree_about_an_unopened_bar`) reads this file,
   * so a stub bar cannot describe an application that does not exist. A test
   * that wants a fuller bar passes its own.
   */
  mission_bar: [
    {
      slug: "room",
      label: "ROOM",
      value: "—",
      level: "quiet",
      about: "Nothing is watching the room.",
    },
    {
      slug: "output",
      label: "OUT",
      value: "clean",
      level: "quiet",
      about: "No dropouts, and the limiter is working within itself.",
    },
    {
      slug: "device",
      label: "",
      value: "no device",
      level: "watch",
      about: "No sound card is open. Nothing will be heard until one is.",
    },
    {
      slug: "health",
      label: "CPU",
      value: "0%",
      level: "quiet",
      about: "The audio thread is using 0% of its time.",
    },
  ],
  // §37. Empty by default, because a fresh install has never watched a room —
  // and present, because `RoomSense.svelte` maps over it.
  room_history: [],
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
    // §110: the spectrum has landed. A test that wants it still pending
    // answers `waveform_info` with this true and `waveform_info_then` with
    // what comes after.
    colour_pending: false,
    mix_out: { opens_frame: 10_800_000, closes_frame: 11_600_000, on_phrase: true },
    // And the other end: where a mix into this record could begin. Opening on
    // its first phrase and closing before the drop the trajectory below
    // records, which is what `plan::mix_in` produces for a record shaped like
    // this one — a fixture whose two bands overlapped would let a lane that
    // drew one of them twice pass.
    // Opening at a non-zero frame on purpose, and the reason is worth keeping:
    // the first version of this fixture opened at 0, which made the band's
    // width and its closing frame the same number — so a lane that drew
    // `closes_frame` as the width passed. A record whose first whole phrase is
    // a few seconds in is also the commoner shape.
    mix_in: {
      opens_frame: 300_000,
      closes_frame: 2_400_000,
      on_phrase: true,
      before_a_drop: true,
    },
    // §25's saved loops, in the order `saved_loops_of` delivers them: slot
    // order, which is pad order. The *sorting* is Rust's and is tested there —
    // a stub could only prove the stub — so what this fixture is for is the
    // half a browser can prove: that both bands are drawn, at the frames they
    // were saved at, carrying the number a DJ would press. One labelled and one
    // not, which are the two states the title has to handle.
    saved_loops: [
      { slot: 1, start_frame: 4_000_000, end_frame: 4_600_000, label: "the break" },
      { slot: 3, start_frame: 7_200_000, end_frame: 7_800_000, label: null },
    ],
    // §75's trajectory, on the same twelve-million-frame record. Four windows
    // with the third thinned out and the fourth back, which is the shape the
    // detector is written to find -- a fixture whose energy never moved would
    // let an overview that drew a flat row of columns pass.
    // §25's `vocal` and `stems` ride on the same windows, and the fixture
    // gives them a shape of their own rather than the energy curve's. The four
    // shares are in the project's one stem order -- **vocal, drums, bass,
    // other** -- and they add to one, as `dj_analysis::presence` produces
    // them.
    //
    // A different current carries each window on purpose: drums the opening,
    // the voice once it is in, and the pads through the breakdown where the
    // kick and the bass are gone. A fixture whose dominant current never
    // changed would let a band that drew one colour for the whole record pass,
    // and one where the vocal and the energy moved together would let an
    // overview that drew the energy twice pass.
    //
    // No window has two currents level with each other, deliberately: a tie
    // makes the test about the tie-break rather than about the reading.
    //
    // The first window is `null`: "nobody measured", drawn as nothing rather
    // than as silence.
    trajectory: {
      sections: [
        // §75's transient density beside the shares, and deliberately not
        // correlated with them: the breakdown at 6M is the *densest* window
        // here and the quietest, which is a shaker under a pad. A fixture
        // where density tracked energy would let a component that drew the
        // energy curve twice pass every assertion.
        { at: 0, energy: 0.9, low: 0.95, parts: null, strikes: null },
        { at: 3_000_000, energy: 1.0, low: 1.0, parts: [0.35, 0.25, 0.25, 0.15], strikes: 4 },
        { at: 6_000_000, energy: 0.35, low: 0.05, parts: [0.25, 0.05, 0.1, 0.6], strikes: 11 },
        { at: 9_000_000, energy: 0.95, low: 0.98, parts: [0.02, 0.48, 0.3, 0.2], strikes: 2 },
      ],
      beats_per_section: 32,
      breakdowns: [{ from: 6_000_000, to: 9_000_000 }],
      drops: [9_000_000],
    },
    // §116: what `Trajectory::changes` reads out of windows shaped like these,
    // on a record long enough for each to hold: the drums leave and the pads
    // take over at the breakdown, and at the drop the drums are back and the
    // voice has gone. A settle into the breakdown and a rise out of it. In
    // order, as Rust sends them.
    changes: [
      { at: 6_000_000, until: null, kind: "enters", stem: 3 },
      { at: 6_000_000, until: null, kind: "leaves", stem: 1 },
      { at: 6_000_000, until: 6_000_000, kind: "settles", stem: null },
      { at: 9_000_000, until: null, kind: "enters", stem: 1 },
      { at: 9_000_000, until: null, kind: "leaves", stem: 0 },
      { at: 9_000_000, until: 9_000_000, kind: "rises", stem: null },
    ],
  },
  // §116's melody line for the same record: ten points a second at 48 kHz,
  // phrases of four seconds with a second's rest between, climbing a scale —
  // so a lane anywhere in the record has notes on screen and a gap to break
  // at. Each note's step is where Rust would put its pitch in the spectrum;
  // the colours are a stand-in ramp, because what a browser can hold is that
  // the step it was sent picks the colour it draws.
  melody_line: {
    frames_per_point: 4_800,
    points: Array.from({ length: 2_500 }, (_, i) =>
      i % 50 < 40 ? [220 * 2 ** ((Math.floor(i / 5) % 8) / 12), 100 + (Math.floor(i / 5) % 8)] : null,
    ),
    colours: Array.from({ length: 256 }, (_, i) => [i, 255 - i, 128]),
    range: [70, 700],
  },
  // §116's rhythm, as `rhythm_line` sends it: four on the floor, a snare on
  // two and four, hats on every off-beat — a step every sixteenth at 124 BPM
  // and 48 kHz.
  rhythm_line: {
    first_frame: 0,
    frames_per_step: 5_806,
    steps: Array.from({ length: 2_400 }, (_, i) => [
      i % 4 === 0 ? 255 : 0,
      i % 8 === 4 ? 220 : 0,
      i % 4 === 2 ? 180 : 0,
    ]),
  },
  // §25's inventory, answered from the same table Rust publishes. The test
  // that matters reads it back and checks every `data-layer` on screen is in
  // it, so this stub is deliberately the real shape rather than a stand-in.
  waveform_layers: layers,
  // §111's "find it to buy" links, as Rust answers them — see `stores.json`.
  // §111's downloads folder, as `downloads` answers on a machine where the DJ
  // has chosen both folders but not switched filing on, with two records
  // filed on an earlier run and one that could not be.
  downloads: {
    watch: "/home/dj/Downloads",
    into: "/home/dj/Music",
    on: false,
    filed: [
      { arrived: "rising.wav", to: "House/Kerri Chandler/rising.wav", at: 1_700_000_200, problem: null },
      { arrived: "sing-along.mp3", to: "Karaoke/Queen/sing-along.mp3", at: 1_700_000_100, problem: null },
      { arrived: "locked.flac", to: "", at: 1_700_000_000, problem: "Permission denied (os error 13)" },
    ],
  },
  stores_plain: stores.plain,
  stores_karaoke: stores.karaoke,
  // §108's share sheet: the channels, and what Rust writes for one fixed
  // night on each — see `share.json`.
  share_channels: share.channels,
  share_messages: share.messages,
  // §108: no recording was made on the fixture's night; a test that wants
  // one passes it.
  recording_chapters: [],
  // §108's going live, switched off, as a fresh install answers it: the file
  // is known before anything is written, and nothing is being said.
  live: {
    on: false,
    overlay: null,
    file: "/home/dj/.config/app.djmanzo.desktop/now-playing.txt",
    saying: "",
    problem: null,
  },
  // §74's rail, in the shape `dj_app::rail` produces for a deck being got
  // ready: five controls, each an action the parser accepts, with the loop and
  // the sync showing their state.
  at_hand: {
    deck: 2,
    doing: "preparing",
    because: "this record is cued and waiting",
    controls: [
      { reach: "cue", label: "cue", action: "deck 2 cue", on: false, kept: false },
      { reach: "mark", label: "mark", action: "deck 2 hotcue_set 1", on: false, kept: false },
      { reach: "loop", label: "loop 4", action: "deck 2 loop 4", on: false, kept: false },
      { reach: "sync", label: "sync", action: "deck 2 sync", on: true, kept: false },
      {
        reach: "keylock",
        label: "keylock",
        action: "deck 2 keylock_toggle",
        on: false,
        // §8 Level 1's *preferred controls*: this one is on the rail because
        // the DJ asked for it, not because djmanzo judged it relevant, and the
        // rail has to say which.
        kept: true,
      },
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
  /**
   * §53: nothing plugged in, which is the ordinary case and the one the
   * interface is designed around.
   *
   * `null` rather than a profile of zeros, because those are different answers:
   * a laptop-only DJ has no gaps for the interface to fill, and a profile
   * reading "no stem controls" would unfold a 370-pixel module for somebody who
   * never asked.
   */
  controller_hands: null,
  /**
   * §53's other direction: nothing lit, because nothing is plugged in.
   *
   * Three fields rather than a count, because a dark board has three different
   * causes with three different answers — no lights declared, no output to
   * send them to, and an output another application holds — and a single
   * number cannot tell them apart.
   */
  controller_lights: { lit: 0, port: "", unlit: "" },
  /**
   * §75: an eight-beat phrase starting on beat four, which is the fixture's
   * own reading — `snapshot.json`'s first deck carries `phrase_beats: 8` and
   * `phrase_anchor: 4`.
   *
   * At 120 BPM a beat is 24 000 frames, so the boundaries sit 192 000 apart
   * beginning at 96 000. Written out rather than derived here, because a stub
   * that computed it would be the interface's arithmetic tested against itself.
   */
  phrase_grid: { first_frame: 96000, spacing_frames: 192000 },
  // §26's contextual beat jump. Rust decides what fits and what to call it;
  // this is the shape and the order it answers in, mid-record, on a track with
  // phrase structure. The phrase entries are last on purpose -- the ordering
  // is Rust's and is tested there, and a fixture that sorted them differently
  // would let a component that re-sorted the list pass.
  waveform_moves: [
    { label: "Back 8 beats", action: "deck 1 beatjump -8" },
    { label: "Back 4 beats", action: "deck 1 beatjump -4" },
    { label: "Forward 4 beats", action: "deck 1 beatjump 4" },
    { label: "Forward 8 beats", action: "deck 1 beatjump 8" },
    { label: "Back a phrase", action: "deck 1 phrasejump -1" },
    { label: "Forward a phrase", action: "deck 1 phrasejump 1" },
  ],
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
  leader_tree: leaderTree,
  // §117: the toolbars shown, so every test written against them still
  // tests them; the slim header has tests of its own that turn them off.
  interface_settings: { toolbars: true, uses: {} },
  // Not a command: the dashboards the `dashboard` stub below answers from,
  // carried here because the stubs run in the page, where this module's
  // imports are not.
  dashboards,
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
  // Six of §27's seven answered and one not, in the shape and the order Rust
  // sends them. The one that is not is the point of the fixture as much as the
  // six that are: the panel has to say so, and a stub carrying only the
  // answered ones would let it ship silently dropping it.
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
    says: "32-beat blend at 3:45",
    landing: { frame: 10_992_000, lead_beats: 8, within_mix: true },
    weakens_from: 10_800_000,
    weakens_to: 11_600_000,
    // §27's drop, inside the stretch the mix would cover: a quarter of the way
    // into a 32-beat blend. **All seven are answered now**, the vocal entry
    // included -- and it is deliberately *not* the same frame as the drop or
    // the landing, so a view that drew one of them twice cannot pass.
    drop_frame: 10_992_000,
    vocal_entry_frame: 11_184_000,
    reasons: ["phrase start (beat 450)", "128 into 131 BPM", "96 beats left"],
    asked: [
      {
        slug: "first-phrase",
        about: "where its first strong phrase would align",
        answered: true,
      },
      { slug: "vocal-entry", about: "where the vocal enters", answered: true },
      { slug: "drop", about: "where the drop occurs", answered: true },
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
  // §22's audition, answered the way Rust answers it: the candidate, where it
  // started and *why*. The reason is the part worth stubbing rather than the
  // frame -- a rail that played a record ninety seconds in and said nothing is
  // the failure, and a fixture carrying only the number would let it ship.
  audition: {
    track: "b".repeat(64),
    from_frame: 4_233_600,
    from_seconds: 88.2,
    because: "drop",
    says: "from the drop",
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
        energy: 0.31,
        analysed: true,
        play_count: 0,
        rating: null,
        colour: null,
      },
      score: 7.9,
      reasons: ["harmonic (9A)", "127 BPM fits", "+1 dB"],
      summary: "+3 BPM \u00b7 8A\u21929A \u00b7 +1 dB",
      confidence: 0.94,
      // §22's estimated transition type. The phrase is Rust's — the rail and
      // §27's ghost panel both draw it, so a stub that made one up here would
      // let the two drift apart without a test noticing.
      transition: {
        style: "blend",
        length_beats: 32,
        at_seconds: 225,
        says: "32-beat blend at 3:45",
      },
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
        energy: 0.88,
        analysed: true,
        play_count: 0,
        rating: null,
        colour: null,
      },
      score: 1.2,
      reasons: ["key clash (3B)", "118 BPM fits", "-3 dB"],
      summary: "-6 BPM \u00b7 8A\u21923B clash \u00b7 -3 dB",
      confidence: 0.55,
      // A clashing pair, so the planner cuts rather than blends — which is
      // the whole value of the line: the two records are the same distance
      // apart as the row above and the mix is a different one.
      transition: {
        style: "cut",
        length_beats: 8,
        at_seconds: 241,
        says: "8-beat cut at 4:01",
      },
    },
  ],
  similar_to: [],
  // §12: no profile by default, because a fresh install has never been told
  // what kind of night it is — and `null` rather than absent, because
  // `Next.svelte` reads it and an unstubbed command is the failure the
  // `stems_status` note above documents.
  // §12's coach, in the state that makes both of its blanks testable: a move
  // observed, a correction to make, and a lesson **withheld because of the
  // moment** rather than because the curriculum is finished. A fixture with a
  // lesson offered would let a panel that drew one blank for both pass.
  coach_report: {
    observed: [
      {
        technique: "long blend",
        what: "Two records overlapped for a whole phrase.",
        metaphor: "Two rivers meeting, neither stopping.",
        at: 41.5,
      },
    ],
    note: {
      what: "Both records have their bass up.",
      why: "Two basses do not add up to more bass. They add up to mud.",
      fix: "Pull one low down as the other comes up.",
    },
    next: null,
    next_metaphor: null,
    next_withheld:
      "Not while you are mixing. The lesson will be here between records.",
  },
  profile_tonight: null,
  // §17: what the phase of the night asks of the ranking. Warm-up by default,
  // because that is what the captured snapshot's night reads as and because a
  // fixture that started at the one phase asking for nothing would let a rail
  // that never followed anything pass. `phase_asks` answers whatever this says
  // and a test may vary it, the same discipline every other answer here
  // follows.
  phase_asks: { trajectory: "lift", words: "gradual energy", prefer: null },
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
        energy: 0.62,
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
        energy: 0.31,
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
  /**
   * §29's AI hover.
   *
   * Nothing by default, which is what djmanzo answers whenever no transition
   * is armed — most of the time. A test that wants one overrides this, and the
   * handler below stamps the deck number in so a suggestion cannot be read as
   * being about the other deck.
   */
  control_suggestions: [],
  // §31. Steady by default — which is what djmanzo answers on almost every
  // tick, and is the whole point of the section.
  theme_now: { theme: "pkg-organic", over_ms: 0, locked: false },
  theme_lock: null,
  theme_chosen: null,
  themes: themeRows,
  adaptation_levels: levelRows,
  under_load: spendRows,
  /** §48's audience-polling saving, as the panel asks for it. The stub answers
   *  the healthy number: the browser runs at whatever frame rate the harness
   *  gives it, and a test about the *list* must not depend on that. */
  room_poll_ms: 2000,
  // §110, from `dj_render::Colouring`: the two ways the spectral balance is
  // coloured, in the order the picker offers them.
  waveform_colourings: [
    {
      slug: "light",
      title: "The spectrum as light",
      about:
        "Every pitch in its own colour of light, 20 Hz deep red to 20 kHz violet: the kick a red core, the voice yellow and green around it, the hats a violet edge. Each part fades with its EQ knob.",
    },
    {
      slug: "bands",
      title: "Three bands, like the EQ",
      about:
        "Low, mid and high at the mixer's own crossovers, so what you see is what the EQ knobs act on.",
    },
  ],
  waveform_colouring: "light",
  // §109: the strip, from the golden, in the full cockpit as a fresh install is.
  activities,
  // §115: nothing suggested unless a test says otherwise.
  activity_suggestion: null,
  // The room's requests as a fresh install has them: the page not running and
  // nothing asked. Needed since §109 put the requests on a surface of their
  // own — the Requests activity mounts it, and an unlisted command answers
  // `null`, which is a shape Rust never sends and the list reads `.length` of.
  audience_status: {
    running: false,
    open: false,
    port: 0,
    heading: "",
    language: "en",
    show_playing: false,
    ways_in: [],
    announcing: false,
    announce_error: null,
    error: null,
    waiting: 0,
  },
  audience_waiting: [],
  audience_all: [],
  /** Nothing chosen: djmanzo is running the way it shipped, which is the state
   *  the axis exists to end and the one a fresh install is actually in. */
  standing: { level: "", departures: [], locked: [] },
  /**
   * §32's Watershed Living can be *chosen* from the picker now, so the world
   * has to be answerable in a browser — it never was, because nothing in a
   * test had a way to open the watershed. An empty world rather than a
   * populated one: what a browser can prove about the watershed is that
   * choosing its theme opens it, and the drawing itself is `dj_world`'s and is
   * tested there against real readings.
   */
  world: {
    entities: [],
    confluence: "Unknown",
    strain: 0,
    alarm: null,
    beating: "Unknown",
    unsurveyed: 0,
  },
  // §81. Tonight opens unnamed, which is the state the picker exists to end —
  // and the state in which the hint has to be right.
  night_settings: nightSettings,
  night_now: {
    setting: null,
    density: null,
    style: null,
    posture: null,
    techniques: [],
  },
  /**
   * §81's other two: what tonight's profile would fit.
   *
   * Nothing by default, which is what most nights answer — there is no profile
   * until three nights of a setting. A test that wants one overrides this.
   */
  night_fits: {
    density: null,
    posture: null,
    withheld: [],
  },
  /**
   * §11's eight fields, as this container answers them: nothing playing, no
   * MIDI service, nobody watching the room.
   *
   * Every absent field is `null` rather than a neutral value, because that is
   * what djmanzo sends and it is the whole design — a room with no camera has
   * not been read, and a zero there would be a measurement nobody took.
   */
  dj_context: {
    session_phase: null,
    occasion: "open",
    music: { bpm: null, key: null, playing: 0, ready: 0, bpm_spread: null },
    hardware: {
      sample_rate: 48000,
      output_latency_ms: 11.6,
      controller: null,
      midi: false,
      cue: false,
    },
    audience: null,
    behaviour: {
      gestures_per_minute: 0,
      commonest: null,
      taken: 0,
      ignored: 0,
    },
    attention: {
      promoted_controls: 5,
      suggestions: 5,
      notices: 3,
      reflow: true,
      tier: "preparation",
      motion: "full",
    },
    health: {
      cpu_load: 0.03,
      dropouts: 0,
      limiter_reduction_db: 0,
      // §90's worker utilisation, in the two states that make the claim
      // testable: one thread that has accounted for time and one that never
      // ran. A fixture where both were numbers would let a panel that drew
      // `0%` for an absent thread pass, and an absent thread is the state a
      // build with no library open is actually in.
      workers: { interface: 0.31, library: null },
    },
  },
  // Three wedding nights and three club nights, which is what §81 is about:
  // the same DJ, two different answers, and never their average.
  /**
   * §80's four, in the states that make the claims testable: two offered, one
   * already accepted, and one djmanzo has not seen enough of. A stub where
   * every row looked the same would let a panel that drew one shape for all
   * four pass.
   *
   * None of them carries a `why_not` any more — all four are derivable since
   * the stem trait started reading the actions rather than §14's gestures — but
   * the field stays, and so does the branch that draws it: §80 names four
   * traits and the next one added may well be one djmanzo cannot see.
   */
  learned_persona: [
    {
      slug: "density",
      says: "You keep the layout at dense.",
      because: "On all 2 kinds of night djmanzo has enough of, you run dense.",
      why_not: "",
      verdict: "offered",
    },
    {
      slug: "automix-at-peak",
      says: "You keep the assistant on prepare.",
      because:
        "On all 2 kinds of night djmanzo has enough of, it has been on prepare.",
      why_not: "",
      verdict: "accepted",
    },
    {
      slug: "blend-length",
      says: "",
      because: "",
      why_not: "",
      verdict: "offered",
    },
    {
      slug: "stems-for-vocals",
      says: "You use the stems mostly for the vocal.",
      because: "More than half the times you reached for a stem, it was the vocal.",
      why_not: "",
      verdict: "offered",
    },
  ],
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
  // Ordered by §58's hierarchy, the way Rust orders it before cutting to
  // twelve — a stub in some other order would let the interface pass a test
  // about the ranking by accident.
  palette: {
    // Nothing was cut. §18's note is empty unless the budget actually
    // removed something a DJ would otherwise have seen, and a fixture
    // carrying it always would let a test about the quiet palette pass
    // over a loud one.
    because: "",
    entries: [
      {
        label: "Run: deck 2 loop 8",
        about: "The vocabulary accepts this exactly as typed.",
        kind: "action",
        run: "deck 2 loop 8",
        tier: "glanceable",
      },
      {
        label: "Deck 1 \u00b7 play",
        about: "start playback",
        kind: "action",
        run: "deck 1 play",
        tier: "glanceable",
      },
      {
        label: "Deck 1 \u00b7 eq_low",
        about: "set the low band",
        kind: "action",
        run: "deck 1 eq_low 1",
        tier: "performable",
      },
      {
        label: "Show Prepare",
        about: "Records on their way to a deck, before they are on one.",
        kind: "surface",
        run: "prepare",
        tier: "preparation",
      },
    ],
  },
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
        energy: 0.44,
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
        energy: 0.5,
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
        energy: 0.5,
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
      // §20's two measurements, deliberately disagreeing. This is the louder
      // record and the less energetic one -- a wall-of-sound master with not
      // much happening in it -- which is the whole reason djmanzo now has two
      // columns rather than one under the wrong name. A fixture where the
      // louder record were also the more energetic one would let a cell that
      // read the wrong field pass.
      loudness_lufs: -7.2,
      energy: 0.31,
      // §20's *vocal availability*. A bachata has a singer on it, so this one
      // reads strongly -- and the merengue below is left un-measured, because
      // a blank and a "no" are different cells and a fixture with only one of
      // them cannot tell a test which it is looking at.
      vocal: { share: 0.42, strong: true },
      analysed: true,
      play_count: 0,
      rating: null,
      colour: null,
      // §20's two new readings, and neither is filled in here: this record has
      // never been played and its phrase structure is not clear enough to say.
      // Both are real answers, and a test about a blank cell needs a row that
      // really is blank.
      last_played: null,
      phrase_beats: null,
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
      // And the quieter, harder-hitting one.
      loudness_lufs: -11.0,
      energy: 0.88,
      vocal: null,
      analysed: true,
      play_count: 3,
      rating: 5,
      colour: null,
      // And this one has both, so §20's columns have something to draw.
      last_played: 1_700_000_000,
      phrase_beats: 32,
    },
  ],
  default_music_folder: null,
  /**
   * §20's columns, from the same table Rust publishes.
   *
   * A golden file rather than a hand-written stub, and it became one the hard
   * way: the list here was typed out under a comment claiming fourteen while
   * holding fifteen, and adding §20's sixteenth left the picker in these tests
   * with no row to tick. Nothing compared the two lists, so the copy drifted
   * in silence. `e2e_fixture.rs` now regenerates this from
   * `columns::Column::ALL` and fails when they diverge.
   */
  library_columns: columns,
  /** What a fresh install draws: the six the browser has always had. */
  chosen_columns: ["title", "artist", "album", "bpm", "key", "duration"],
  /**
   * §8 Level 1's nine, as `dj_app::remembered` lists them.
   *
   * All nine, and all nine kept — the waveform row said otherwise until §25's
   * layers became something a DJ could choose from, and this fixture is what
   * the settings block's claim is measured against.
   */
  remembered: [
    {
      slug: "workspace",
      about: "The arrangement you last worked in",
      forgotten: "You would set the room up again every night",
      kept: true,
      why_not: "",
    },
    {
      slug: "density",
      about: "How tightly the interface is packed",
      forgotten: "A laptop screen would open at a club's spacing",
      kept: true,
      why_not: "",
    },
    {
      slug: "columns",
      about: "Which columns the browser carries",
      forgotten: "The six columns djmanzo ships, not the ones you read",
      kept: true,
      why_not: "",
    },
    {
      slug: "sorting",
      about: "Which column the browser is ordered by, and which way",
      forgotten: "Back to artist, A to Z, every time you open the browser",
      kept: true,
      why_not: "",
    },
    {
      slug: "panels",
      about: "Where each panel sits, and which are pinned open",
      forgotten: "Every panel you pinned would be closed again",
      kept: true,
      why_not: "",
    },
    {
      slug: "decks",
      about: "How many decks are on screen",
      forgotten: "Two decks, however many you play on",
      kept: true,
      why_not: "",
    },
    {
      slug: "pad-pages",
      about: "The pad pages you play from",
      forgotten: "Cues first, even if you never touch them",
      kept: true,
      why_not: "",
    },
    {
      slug: "waveform-display",
      about: "How the waveform is drawn",
      forgotten: "Whatever djmanzo draws by default",
      kept: true,
      why_not: "",
    },
    {
      slug: "controls",
      about: "The controls you keep within reach",
      forgotten: "Only what djmanzo judges you need this second",
      kept: true,
      why_not: "",
    },
  ],
  /** Every control §74's rail can hold, as `at_hand::Reach` lists them. */
  rail_controls: [
    { slug: "cue", about: "Jump to the cue point" },
    { slug: "play", about: "Play, or stop" },
    { slug: "loop", about: "Loop four beats, or leave the loop" },
    { slug: "sync", about: "Match tempo and phase to the other record" },
    { slug: "bass", about: "Pull the low band out, or put it back" },
    { slug: "filter", about: "Sweep the filter, or return it to the middle" },
    { slug: "keylock", about: "Hold the pitch while the tempo moves" },
    { slug: "slip", about: "Keep the record running underneath" },
    { slug: "reverse", about: "Run the record backwards" },
    { slug: "censor", about: "Reverse while held, then drop back in place" },
    { slug: "mark", about: "Set the first hot cue where you are" },
    { slug: "stem-vocal", about: "Mute the vocal" },
    { slug: "stem-drums", about: "Mute the drums" },
    { slug: "stem-bass", about: "Mute the bass line" },
    { slug: "stem-other", about: "Mute everything else" },
  ],
  /** A fresh install: nothing starred and nothing kept. */
  favourite_pad_pages: [],
  kept_controls: [],
  /**
   * Every layer djmanzo can draw, which is what an empty ask comes back as.
   *
   * Not `[]`: the chooser never returns nothing, and a stub that did would make
   * every waveform spec a spec about a strip with no instrumentation on it.
   */
  chosen_layers: layers
    .filter((l) => l.drawn !== "nowhere")
    .map((l) => l.name),
  /** Artist, A to Z — what the browser has always opened on. */
  library_sort: { column: "artist", ascending: true },
  /**
   * §54's six, from the same table Rust publishes.
   *
   * A golden file rather than a hand-written stub, for the reason the layers
   * are: this is the fixture a test about "one press reaches six systems" is
   * measured against, and a copy written here would go stale the first time a
   * preset changed what it does.
   */
  setups,
  /** §16's eight, from the same table Rust publishes. */
  knowledge_packs: packs,
  /** Nothing chosen: the whole catalogue, which is what a DJ who has not picked
   *  a pack gets and the state the picker exists to change. */
  chosen_pack: "",
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
    /**
   * §17's phase priorities.
   *
   * What the *Peak* phase asks for, because that is the phase whose whole
   * instruction is "minimal UI clutter" and so the one worth a default that
   * cannot be mistaken for a rearrangement. A test wanting another phase's
   * list passes its own; `dj_app::cockpit::priorities` holds the real table
   * and is tested there.
   */
  phase_priorities: ["room"],
  cockpit_surfaces: surfaces,
  /**
   * §7's presets, for the picker.
   *
   * Four of the twenty-three, chosen to cover what applying one has to do: a
   * deck count, a density, a theme, and panels that open. Copied field for
   * field from `dj_app::cockpit::workspaces()` and kept honest by a Rust test
   * that reads this file — `the_harness_and_rust_agree_about_the_presets`.
   * A stub with made-up presets would prove the picker draws a list and
   * nothing about whether pressing one does what it says.
   */
  cockpit_workspaces: [
    {
      name: "Perform",
      about: "The decks and nothing else.",
      surfaces: [],
      density: "standard",
      focus: "performing",
      theme: "",
      layout: "",
      decks: 2,
      locked: [],
    },
    {
      name: "Open Format",
      about: "Four decks and the whole collection, for a night that goes anywhere.",
      surfaces: [
        { surface: "library", dock: "bottom", order: 0, size: null, collapsed: false, pinned: false },
        { surface: "next", dock: "right", order: 0, size: null, collapsed: false, pinned: false },
      ],
      density: "compact",
      focus: "preparing",
      theme: "",
      layout: "",
      decks: 4,
      locked: [],
    },
    {
      name: "High Contrast",
      about: "The decks, in the theme built for a dark booth and a bright screen.",
      surfaces: [],
      density: "standard",
      focus: "performing",
      theme: "pkg-booth",
      layout: "",
      decks: 2,
      locked: [],
    },
    {
      name: "Laptop Compact",
      about: "Everything that fits on a small screen, and nothing that does not.",
      surfaces: [],
      density: "ultra-dense",
      focus: "performing",
      theme: "",
      // §5B: this is the one arrangement in the fixture that rebuilds the deck,
      // and it is the one §5B names — "laptop compact mode: dense controls
      // optimised for limited screen height". A fixture where every row left
      // the deck alone would let a shell that ignored the field pass.
      layout: "Performance",
      decks: 2,
      locked: [],
    },
    {
      // §5B's autopilot supervisory mode. In the fixture because it is the one
      // arrangement built out of readings rather than controls, and because
      // "simplified" is a claim about what is *not* on the deck -- which only a
      // browser can check.
      name: "Autopilot",
      about: "What is playing, what is next and how the room is taking it, while it drives and you watch.",
      surfaces: [
        { surface: "assistant", dock: "right", order: 0, size: null, collapsed: false, pinned: false },
        { surface: "next", dock: "right", order: 1, size: null, collapsed: false, pinned: false },
        { surface: "room", dock: "bottom", order: 0, size: null, collapsed: false, pinned: false },
      ],
      density: "standard",
      focus: "supervising",
      theme: "",
      layout: "Starter",
      decks: 2,
      locked: [],
    },
  ],
  /**
   * §79's six, as Rust lists them. Answered here so the Settings panel draws
   * the switches it draws in the application -- a panel handed `null` would
   * show none, and a test asserting a lock does something would then be
   * asserting it against a control that is not on screen.
   */
  /**
   * §7 and §103's modularity: the arrangements this DJ has kept.
   *
   * Empty, which is what a fresh install has. `keep_workspace` and
   * `forget_workspace` are answered by the handler below rather than from this
   * table, because their answer depends on what was asked.
   */
  my_workspaces: [],
  /**
   * §43's appetite. The state every fresh session is in: nothing offered yet,
   * so nothing to go on, and the rate at full.
   */
  assistant_appetite: {
    appetite: "full",
    ignored_in_a_row: 0,
    offers: 0,
    taken: 0,
    says: "Nothing offered yet, so nothing to go on.",
  },
  cockpit_locks: [
    { slug: "workspace", about: "The arrangement stays the one you chose." },
    { slug: "arrangement", about: "Nothing opens, closes or moves unless you do it." },
    { slug: "density", about: "The interface stops resizing itself to the window." },
    { slug: "theme", about: "The palette stays the one you are wearing." },
    { slug: "waveform", about: "The waveform and the controls stop answering the audio." },
    { slug: "assistant", about: "The assistant keeps working, and stops moving anything." },
  ],
  cockpit_workspace: {
    workspace: {
      name: "Perform",
      about: "",
      surfaces: [],
      density: "standard",
      focus: "performing",
      theme: "",
      decks: 2,
      locked: [],
    },
    notes: [],
    permits: { rearrange: true, resize: true, retheme: true, restyle: true },
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
  // The array a test may already be holding, emptied, rather than a new one.
  //
  // **This used to allocate, and thirty-six guards could not fail because of
  // it.** The natural way to write one of these tests is
  // `const thrown = errorsThrown(page)` at the top and
  // `expect(thrown).toEqual([])` at the bottom — and with a fresh array
  // installed here, the reference the test is holding is one nothing ever
  // pushes to. Every one of those assertions was comparing an empty array
  // against itself, and a real thrown error was found the moment the first
  // test asked at the right moment instead.
  const thrown = errorsThrown(page);
  thrown.length = 0;
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

      /*
        A test affordance: deliver **another** snapshot.

        Without it every browser test sees exactly one frame of djmanzo, and a
        component that asks once at start-up looks identical to one that keeps
        up with the decks. That is not hypothetical — the Next rail shipped
        asking once, before the decks had loaded, and nothing in this suite
        could tell.

        The last state is kept beside it so a test can clone it and change one
        field, rather than hand-writing a snapshot the application never sends.
      */
      win.__lastState = state;
      win.__emit = (next: unknown) => {
        win.__lastState = next;
        const id = handlers.get("snapshot");
        if (id === undefined) return;
        const handler = win[`_${id}`] as ((event: unknown) => void) | undefined;
        handler?.({ event: "snapshot", id: 0, payload: next });
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
          // Which theme was declared, not only that one was. §31 adapts the
          // theme on its own, so painting the colours and telling djmanzo are
          // two different things and only the second survives the next tick --
          // a distinction `__asked` cannot draw, because it keeps names and
          // throws the arguments away.
          if (cmd === "theme_chosen") {
            // Rust's round trip, mirrored: an id nothing ships is refused
            // rather than worn, because `applyPackagePalette` falls back to the
            // organic palette and a typo would otherwise be worn silently as
            // another theme's colours.
            const known = (answers.themes ?? []) as { pack: string }[];
            const asked = args.theme as string;
            if (!known.some((row) => row.pack === asked)) {
              return Promise.reject(
                new Error(`djmanzo does not ship a theme called \`${asked}\``),
              );
            }
            win.__chosenTheme = asked;
            return Promise.resolve(asked);
          }
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
          // §120: each provider's own models, so a test can tell which
          // provider's list a model was chosen from -- and the choice echoed
          // and recorded, because the provider it is set with is the thing
          // that was wrong.
          if (cmd === "list_llm_models") {
            const provider = String(args.provider);
            return Promise.resolve([
              { id: `${provider}/fast`, name: `${provider} fast`, free: true, context: null, input_price: null, output_price: null },
              { id: `${provider}/large`, name: `${provider} large`, free: false, context: null, input_price: 1, output_price: 2 },
            ]);
          }
          // §118: every write the event panel makes is recorded, so a test
          // can tell what was kept. A save is answered with the view of the
          // event as sent -- the steps are the fixture's, because the rules
          // that recompute them are Rust's and do not run here.
          // §118d: every write and every hand-over the press kit makes,
          // recorded. A save is answered with the view of the kit as sent --
          // the occasions are the fixture's, because the words are Rust's
          // and are tested there -- or with Rust's refusal of the one answer
          // the fixture has one for.
          // The file dialog answers with the path a test put in
          // `__dialogAnswer`, or with nothing, as a dialog cancelled does.
          if (cmd === "plugin:dialog|open") {
            return Promise.resolve(win.__dialogAnswer ?? null);
          }
          if (
            cmd === "kit_save" ||
            cmd === "kit_compose" ||
            cmd === "kit_send" ||
            cmd === "kit_add" ||
            cmd === "kit_forget"
          ) {
            ((win.__kitCalls ??= []) as unknown[]).push({ cmd, ...JSON.parse(JSON.stringify(args)) });
            const view = answers.kit_view as { kit: Record<string, unknown> } & Record<string, unknown>;
            if (cmd === "kit_save") {
              const sent = args.kit as { phone?: string };
              const refused = answers.kit_refused as { phone: string; message: string };
              if (sent.phone === refused.phone) return Promise.reject(refused.message);
              return Promise.resolve({ ...view, kept: true, kit: args.kit });
            }
            if (cmd === "kit_compose") return Promise.resolve(answers.kit_compose);
            if (cmd === "kit_send") {
              return Promise.resolve(args.way === "save" ? "/home/dj/.config/djmanzo/kit/press-kit.html" : null);
            }
            const kind = String(args.kind) === "photo" ? "photos" : "documents";
            const list = [...((view.kit[kind] ?? []) as { file: string; caption: string }[])];
            if (cmd === "kit_add") {
              list.push({ file: String(args.path).split("/").pop() ?? "file", caption: "" });
            } else {
              const at = list.findIndex((k) => k.file === args.file);
              if (at >= 0) list.splice(at, 1);
            }
            const next = { ...view, kit: { ...view.kit, [kind]: list } };
            answers.kit_view = next;
            return Promise.resolve(next);
          }
          if (cmd === "save_event") {
            ((win.__eventSaves ??= []) as unknown[]).push(args.gig);
            const base = (answers.event_view ?? {}) as Record<string, unknown>;
            return Promise.resolve({ ...base, gig: args.gig });
          }
          if (cmd === "set_live_event" || cmd === "event_tonight") {
            ((win.__eventCalls ??= []) as unknown[]).push({ cmd, ...args });
            if (cmd === "set_live_event") return Promise.resolve(args.id ?? null);
            return Promise.resolve(answers.event_tonight);
          }
          if (cmd === "take_event_idea" || cmd === "new_event" || cmd === "forget_event") {
            ((win.__eventCalls ??= []) as unknown[]).push({ cmd, ...args });
            if (cmd === "forget_event") {
              const list = (answers.list_events ?? []) as { id: string }[];
              return Promise.resolve(list.filter((e) => e.id !== args.id));
            }
            return Promise.resolve(answers[cmd]);
          }
          if (cmd === "set_assistant_model") {
            const chosen = { provider: String(args.provider), model: String(args.model) };
            ((win.__chosenModels ??= []) as unknown[]).push(chosen);
            return Promise.resolve({ ...chosen, spent_usd: 0, cap_usd: 2, unpriced_calls: 0 });
          }
          // The mix re-render. Answered here rather than from the table so a
          // test can check *which* mix was asked for: the panel holds two
          // numbers per row and passing the wrong row's would produce a
          // plausible file of the wrong twenty seconds.
          // §24's keep. Answered here rather than from the table so a test
          // can check *which* mix was kept: the panel holds a timestamp per
          // row, and keeping the wrong row would store a plausible pair of the
          // wrong two records.
          // §17: which direction the rail actually asked for. Recorded rather
          // than inferred from the highlighted button, because those are two
          // different claims: a rail that lit *Lift* and sent "hold" would look
          // exactly like one that worked, and the direction is an input to the
          // ranking rather than a decoration on it. Answered from the table
          // afterwards, like everything else.
          // §118a: a decision, composed as `next_decision` composes it --
          // the rail's records, one per direction, in Rust's words -- with
          // what was asked and what was loaded recorded, because loading the
          // wrong record on the wrong deck looks like working to a count.
          if (cmd === "next_decision") {
            const deck = Number(args.deck);
            ((win.__decisionAsked ??= []) as number[]).push(deck);
            ((win.__decisionDecks ??= []) as number[]).push(Number(args.decks));
            const rail = (answers.suggest_next ?? []) as Record<string, unknown>[];
            const words = answers.decision_words as typeof decide;
            return Promise.resolve({
              from: deck,
              into: deck === 1 ? 2 : 1,
              choices: rail.slice(0, 3).map((s, i) => ({ ...s, ...words.directions[i] })),
              stall: words.stall,
            });
          }
          if (cmd === "guides") {
            ((win.__guidesDecks ??= []) as number[]).push(Number(args.decks));
          }
          if (cmd === "load_track") {
            ((win.__loadedTracks ??= []) as unknown[]).push({ deck: args.deck, path: args.path });
          }
          if (cmd === "suggest_next") {
            ((win.__ranked ??= []) as string[]).push(String(args.trajectory));
            return Promise.resolve(answers.suggest_next ?? []);
          }
          // §106: what the room surface measured, in order. Recorded rather
          // than counted, because the claim a camera test makes is about the
          // numbers — a surface that sent three readings of zero looks like
          // one that worked to anything that only counts calls.
          if (cmd === "room_saw") {
            ((win.__saw ??= []) as unknown[]).push({
              light: args.light,
              movement: args.movement,
              loudness: args.loudness,
            });
            return Promise.resolve(null);
          }
          // §106: what the hum handed Rust — how much, at what rate, and how
          // loud — without keeping eight seconds of floats in the page.
          if (cmd === "hum") {
            const samples = (args.samples ?? []) as number[];
            let squares = 0;
            for (const s of samples) squares += s * s;
            win.__hummed = {
              count: samples.length,
              rate: args.rate,
              rms: samples.length ? Math.sqrt(squares / samples.length) : 0,
            };
            return Promise.resolve(
              answers.hum ?? {
                key: "8A",
                tempo: 120,
                seconds: samples.length / Number(args.rate || 1),
                near: [],
                melody: [],
                voiced: 0.8,
              },
            );
          }
          // §117: the DJ's own keys under Space, held between calls the way
          // Rust holds them, and laid over the tree Rust built (`leader.json`)
          // as `dj_app::leader::with_mine` lays them: a chain makes the groups
          // it needs, marked as the DJ's. Rust's refusal of keys that would
          // hide a group is mirrored for one case, `d`, which the tests use.
          type MyKey = { keys: string[]; label: string; run: string };
          type Node = { key: string; label: string; run: string | null; children: Node[]; mine: boolean };
          const myKeys = () => (win.__myKeys ??= []) as MyKey[];
          if (cmd === "leader_mine") return Promise.resolve(structuredClone(myKeys()));
          // §117: the dashboard Rust builds for the activity the DJ is in,
          // and the settings the toolbars switch writes.
          if (cmd === "dashboard") {
            const boards = answers.dashboards as Record<string, unknown>;
            return Promise.resolve(structuredClone(boards[String(args.current)] ?? boards[""]));
          }
          if (cmd === "set_toolbars") {
            const settings = { ...(answers.interface_settings as object), toolbars: Boolean(args.on) };
            answers.interface_settings = settings;
            return Promise.resolve(structuredClone(settings));
          }
          if (cmd === "used_tile") {
            ((win.__usedTiles ??= []) as string[]).push(String(args.id));
            return Promise.resolve(null);
          }
          if (cmd === "keep_mnemonic") {
            // Copied: the interface passes its own reactive array, which the
            // real bridge serialises and `structuredClone` cannot.
            const keys = Array.from(args.keys as string[]);
            if (keys.length === 1 && keys[0] === "d") {
              return Promise.reject(
                new Error("Space d already opens a group of keys; choose keys below it or elsewhere"),
              );
            }
            const kept = myKeys().filter((one) => one.keys.join(" ") !== keys.join(" "));
            kept.push({ keys, label: String(args.label).trim(), run: String(args.run) });
            win.__myKeys = kept;
            return Promise.resolve(structuredClone(kept));
          }
          if (cmd === "forget_mnemonic") {
            const keys = Array.from(args.keys as string[]).join(" ");
            win.__myKeys = myKeys().filter((one) => one.keys.join(" ") !== keys);
            return Promise.resolve(structuredClone(win.__myKeys));
          }
          if (cmd === "leader_tree") {
            const root = structuredClone(answers.leader_tree) as Node;
            for (const one of myKeys()) {
              let here = root;
              for (const key of one.keys.slice(0, -1)) {
                let next = here.children.find((child) => child.key === key);
                if (!next) {
                  next = { key, label: "Yours", run: null, children: [], mine: true };
                  here.children.push(next);
                }
                here = next;
              }
              const last = one.keys[one.keys.length - 1];
              const leaf: Node = { key: last, label: one.label, run: one.run, children: [], mine: true };
              const at = here.children.findIndex((child) => child.key === last);
              if (at >= 0) here.children[at] = leaf;
              else here.children.push(leaf);
            }
            return Promise.resolve(root);
          }
          // §109: the activity strip, held between calls the way Rust holds
          // it, so a test can move between activities and keep its own. The
          // rules are Rust's, mirrored: a shipped name is refused, keeping a
          // name again replaces it, and the keys are dealt 1 to 9 in order.
          const strip = () => {
            win.__activities ??= structuredClone(answers.activities);
            return win.__activities as {
              activities: {
                slug: string;
                title: string;
                doing: string;
                icon: string;
                shipped: boolean;
                key: string | null;
                workspace: unknown;
              }[];
              back: string;
              on: boolean;
              current: string;
              previous: string;
            };
          };
          const redeal = (state: ReturnType<typeof strip>) => {
            state.activities.forEach((activity, index) => {
              activity.key = index < 9 ? `Digit${index + 1}` : null;
            });
            return structuredClone(state);
          };
          if (cmd === "activities") return Promise.resolve(structuredClone(strip()));
          if (cmd === "set_activity_mode") {
            const state = strip();
            const current = String(args.current ?? "");
            if (current && !state.activities.some((a) => a.slug === current)) {
              return Promise.reject(new Error(`there is no activity called "${current}"`));
            }
            if (current !== state.current) {
              state.previous = state.current;
              state.current = current;
            }
            state.on = Boolean(args.on);
            ((win.__activityMoves ??= []) as string[]).push(`${state.on ? "on" : "off"}:${current}`);
            return Promise.resolve(structuredClone(state));
          }
          if (cmd === "keep_activity") {
            const state = strip();
            const title = String(args.title ?? "").trim();
            const slug = title
              .split(/[^\p{L}\p{N}]+/u)
              .filter(Boolean)
              .map((word) => word.toLowerCase())
              .join("-");
            if (!slug) return Promise.reject(new Error("an activity needs a name"));
            if (state.activities.some((a) => a.shipped && a.slug === slug)) {
              return Promise.reject(
                new Error(`djmanzo already has an activity called "${title}" — choose another name`),
              );
            }
            const mine = {
              slug,
              title,
              doing: "Your own arrangement.",
              icon: "flag",
              shipped: false,
              key: null,
              // Through JSON, the way the real bridge carries it: the
              // interface passes a live `$state` object, which IPC
              // serialises and `structuredClone` cannot copy.
              workspace: { ...JSON.parse(JSON.stringify(args.workspace)), name: title },
            };
            const at = state.activities.findIndex((a) => !a.shipped && a.slug === slug);
            if (at >= 0) state.activities[at] = mine;
            else state.activities.push(mine);
            return Promise.resolve(redeal(state));
          }
          // §118b: the welcome's writes, recorded; applying it keeps the
          // plan's activities the way `keep_activity` above does.
          if (cmd === "welcome_save" || cmd === "welcome_plan" || cmd === "welcome_apply") {
            ((win.__welcomeCalls ??= []) as unknown[]).push({ cmd, ...JSON.parse(JSON.stringify(args)) });
            const given = JSON.parse(JSON.stringify(args.answers ?? {}));
            // The one answer the fixture has Rust's refusal for.
            const refused = answers.welcome_refused as { bpm_low: number; bpm_high: number; message: string };
            if (given.bpm_low === refused.bpm_low && given.bpm_high === refused.bpm_high) {
              return Promise.reject(refused.message);
            }
            if (cmd === "welcome_save") return Promise.resolve(given);
            const plan = answers.welcome_plan as { activities: { title: string; workspace: string }[] };
            if (cmd === "welcome_plan") return Promise.resolve(plan);
            const state = strip();
            for (const planned of plan.activities) {
              const slug = planned.title.toLowerCase().split(/[^a-z0-9]+/).filter(Boolean).join("-");
              if (state.activities.some((a) => a.slug === slug)) continue;
              state.activities.push({
                slug,
                title: planned.title,
                doing: "Your own arrangement.",
                icon: "flag",
                shipped: false,
                key: null,
                workspace: { name: planned.workspace },
              });
            }
            const applied = answers.welcome_applied as { workspace: string; theme: string };
            return Promise.resolve({
              plan,
              workspace: applied.workspace,
              theme: given.theme || applied.theme,
              activities: redeal(state),
              answers: { ...given, done: true },
            });
          }
          if (cmd === "forget_activity") {
            const state = strip();
            const slug = String(args.slug ?? "");
            state.activities = state.activities.filter((a) => a.shipped || a.slug !== slug);
            if (state.current === slug) state.current = "";
            if (state.previous === slug) state.previous = "";
            return Promise.resolve(redeal(state));
          }
          // §111: the stores, from the golden Rust blesses, and every store
          // the interface asked to open, with the song it asked about — the
          // address itself is built in Rust and tested there.
          if (cmd === "store_links") {
            return Promise.resolve(structuredClone(args.karaoke ? answers.stores_karaoke : answers.stores_plain));
          }
          // §111's downloads folder: one setting, kept, and what it filed.
          if (cmd === "downloads" || cmd === "set_downloads") {
            const kept = (win.__downloads ??= structuredClone(answers.downloads)) as Record<string, unknown>;
            if (cmd === "set_downloads") {
              if (args.on && (!args.watch || !args.into)) {
                return Promise.reject(new Error("choose the folder to watch and the music folder before switching it on"));
              }
              Object.assign(kept, { watch: args.watch ?? null, into: args.into ?? null, on: Boolean(args.on) });
              ((win.__setDownloads ??= []) as unknown[]).push({ ...args });
            }
            return Promise.resolve(structuredClone(kept));
          }
          // §108: one switch, kept. Switched on, the overlay is served and the
          // stream is told the record deck 1 is playing; which record is
          // `dj_app::live::lead`'s decision, tested in Rust.
          if (cmd === "live_status" || cmd === "set_live") {
            const kept = (win.__live ??= structuredClone(answers.live)) as Record<string, unknown>;
            if (cmd === "set_live") {
              const on = Boolean(args.on);
              Object.assign(kept, {
                on,
                overlay: on ? "http://127.0.0.1:7332/" : null,
                saying: on ? "Aventura - Obsesión" : "",
              });
              ((win.__setLive ??= []) as unknown[]).push(on);
            }
            return Promise.resolve(structuredClone(kept));
          }
          // §108: the share sheet, answered with what Rust writes for one
          // fixed night on each channel (`share.json`), and every hand-off
          // recorded with the channel it was for.
          // §115: the answers to a proposal, recorded with their words,
          // because "not now" and "not tonight" are two different promises to
          // the DJ. The proposal itself arrives on the snapshot.
          if (cmd === "whisper_answer") {
            ((win.__whisperAnswered ??= []) as unknown[]).push({ kind: args.kind, answer: args.answer });
            return Promise.resolve(null);
          }
          if (cmd === "share_channels") return Promise.resolve(structuredClone(answers.share_channels));
          if (cmd === "share_preview" || cmd === "share_to" || cmd === "share_to_whatsapp") {
            const slug = cmd === "share_to_whatsapp" ? "whatsapp" : String(args.channel ?? "whatsapp");
            const said = (answers.share_messages as Record<string, unknown>)[slug];
            if (!said) return Promise.reject(new Error(`djmanzo does not share to \`${slug}\``));
            if (cmd !== "share_preview") ((win.__shared ??= []) as string[]).push(slug);
            return Promise.resolve(structuredClone(said));
          }
          if (cmd === "open_store") {
            ((win.__opened ??= []) as unknown[]).push({ ...args });
            return Promise.resolve(null);
          }
          // §107: the singer rotation. The rules are Rust's and tested there;
          // this holds one rotation and records every call with its
          // arguments, so a test can check what the surface *sent* — which is
          // the half a browser can see.
          if (cmd.startsWith("karaoke_")) {
            ((win.__karaoke ??= []) as unknown[]).push({ cmd, ...args });
            const rotation = (win.__rotation ??= structuredClone(
              answers.karaoke_rotation ?? { singers: [], up_next: null, lately: [] },
            )) as {
              singers: { name: string; songs: { title: string; track: string | null; path: string | null; key: number }[]; turns: number }[];
              up_next: string | null;
              lately: unknown[];
            };
            const upNext = () => rotation.singers.find((s) => s.songs.length > 0)?.name ?? null;
            if (cmd === "karaoke_ask") {
              const name = String(args.singer ?? "").trim();
              const title = String(args.title ?? "").trim();
              if (!name || !title) return Promise.reject(new Error("a request needs a singer's name and a song"));
              const song = { title, track: (args.track as string) ?? null, path: (args.path as string) ?? null, key: Number(args.key ?? 0) };
              const found = rotation.singers.find((s) => s.name.toLowerCase() === name.toLowerCase());
              if (found) found.songs.push(song);
              else rotation.singers.push({ name, songs: [song], turns: 0 });
            }
            if (cmd === "karaoke_sang") {
              const at = rotation.singers.findIndex((s) => s.songs.length > 0);
              if (at >= 0) {
                const [singer] = rotation.singers.splice(at, 1);
                const song = singer.songs.shift();
                singer.turns += 1;
                rotation.lately.unshift({ singer: singer.name, title: song?.title, track: song?.track, key: song?.key });
                rotation.singers.push(singer);
              }
            }
            if (cmd === "karaoke_key") {
              const singer = rotation.singers.find((s) => s.name === args.singer);
              if (singer?.songs[0]) singer.songs[0].key = Math.max(-7, Math.min(7, Number(args.key)));
            }
            rotation.up_next = upNext();
            return Promise.resolve(structuredClone(rotation));
          }
          // §110: which colouring was chosen, echoed the way Rust echoes it.
          if (cmd === "set_waveform_colouring") {
            const asked = String(args.colouring);
            if (asked !== "light" && asked !== "bands") {
              return Promise.reject(new Error(`djmanzo does not colour the waveform "${asked}"`));
            }
            ((win.__coloured ??= []) as string[]).push(asked);
            return Promise.resolve(asked);
          }
          // §110: the colour still being measured, then landing, the way the
          // spectrum arrives off the load path. Timed rather than counted:
          // every lane and every overview asks at load, and "only the first
          // ask is pending" would hand the rest the landed answer without
          // their ever having to ask again — which is the thing under test.
          if (cmd === "waveform_info" && answers.waveform_info_then !== undefined) {
            const first = (win.__firstWaveformAsk ??= Date.now()) as number;
            const landed = Date.now() - first >= 600;
            return Promise.resolve(landed ? answers.waveform_info_then : answers.waveform_info);
          }
          if (cmd === "keep_mix") {
            win.__keptAt = args.at;
            return Promise.resolve(1);
          }
          // §22's audition. Answered here rather than from the table because
          // a stop and a start are two different answers to one command, and
          // a fixed one would make a toggle that works and one that only ever
          // starts look identical -- which is the failure that leaves a record
          // playing in a DJ's headphones with no way to stop it.
          if (cmd === "audition") {
            const track = String(args.track ?? "");
            ((win.__auditioned ??= []) as string[]).push(track);
            if (track.trim() === "") return Promise.resolve(null);
            return Promise.resolve(answers.audition ?? null);
          }
          if (cmd === "session_render_mix") {
            win.__renderMixArgs = { at: args.at, tookSeconds: args.tookSeconds };
            return Promise.resolve(
              `20s → /home/dj/.config/djmanzo/recordings/mix-at-${Math.round(
                Number(args.at),
              )}s.wav`,
            );
          }
          // §7's keep. Answered here rather than from the table because the
          // collection is *held between calls*: a fixed answer would make a
          // save that worked and one that did nothing look identical, and the
          // whole claim of the picker is that the row appears.
          //
          // The two refusals are Rust's rules, mirrored: an empty name and one
          // djmanzo ships. A Rust test keeps the sentences honest; what a
          // browser can prove is that the refusal reaches the screen.
          // §20's columns, held between calls: the whole claim of the picker
          // is that ticking a box changes the table, and a fixed answer would
          // make a working picker and a broken one look identical.
          if (cmd === "set_chosen_columns") {
            const asked = ((args.columns ?? []) as string[]).map((c) => c.trim());
            const known = (answers.library_columns ?? []) as { slug: string }[];
            const out: string[] = [];
            for (const slug of asked) {
              if (known.some((c) => c.slug === slug) && !out.includes(slug)) out.push(slug);
            }
            // Rust's two rules, mirrored: nothing means the shipped six, and the
            // title always survives. A Rust test keeps the rules honest; what a
            // browser can prove is that the table follows.
            const shipped = ["title", "artist", "album", "bpm", "key", "duration"];
            const settled = out.length === 0 ? shipped : out.includes("title") ? out : ["title", ...out];
            win.__columns = settled;
            return Promise.resolve(settled);
          }
          if (cmd === "chosen_columns") {
            return Promise.resolve(
              win.__columns ?? answers.chosen_columns ?? ["title", "artist", "album", "bpm", "key", "duration"],
            );
          }
          // §8 Level 1, all three held between calls and for the same reason
          // the columns are: the claim each one makes is that setting it
          // *changes something*, and a fixed answer would make a preference
          // that works and one that is thrown away look identical.
          if (cmd === "set_library_sort") {
            const known = (answers.library_columns ?? []) as { slug: string }[];
            const asked = String(args.column ?? "");
            // Rust's rule, mirrored: a column this build does not have falls
            // back rather than being stored and silently ignored.
            const settled = known.some((c) => c.slug === asked)
              ? { column: asked, ascending: Boolean(args.ascending) }
              : { column: "artist", ascending: true };
            win.__sort = settled;
            return Promise.resolve(settled);
          }
          if (cmd === "library_sort") {
            return Promise.resolve(
              win.__sort ?? answers.library_sort ?? { column: "artist", ascending: true },
            );
          }
          if (cmd === "set_favourite_pad_pages") {
            const known = (answers.pad_pages ?? []) as { name: string }[];
            const out: string[] = [];
            for (const name of (args.pages ?? []) as string[]) {
              if (known.some((p) => p.name === name) && !out.includes(name)) out.push(name);
            }
            win.__pages = out;
            return Promise.resolve(out);
          }
          if (cmd === "favourite_pad_pages") {
            return Promise.resolve(win.__pages ?? answers.favourite_pad_pages ?? []);
          }
          if (cmd === "set_kept_controls") {
            const known = (answers.rail_controls ?? []) as { slug: string }[];
            const out: string[] = [];
            for (const slug of (args.controls ?? []) as string[]) {
              if (known.some((c) => c.slug === slug) && !out.includes(slug)) out.push(slug);
            }
            // §74's ceiling, mirrored: the rail holds eight.
            win.__railKept = out.slice(0, 8);
            return Promise.resolve(win.__railKept);
          }
          if (cmd === "kept_controls") {
            return Promise.resolve(win.__railKept ?? answers.kept_controls ?? []);
          }
          if (cmd === "set_chosen_layers") {
            const known = (answers.waveform_layers ?? []) as {
              name: string;
              choosable: boolean;
            }[];
            const asked = (args.layers ?? []) as string[];
            const everything = (answers.chosen_layers ?? []) as string[];
            // Rust's three rules, mirrored: an empty ask is everything djmanzo
            // draws, the two that *are* the waveform go back whether they were
            // asked for or not, and the answer comes back in §25's own order
            // rather than the order the boxes were ticked.
            const out =
              asked.length === 0
                ? everything
                : known
                    .filter(
                      (layer) =>
                        everything.includes(layer.name) &&
                        (!layer.choosable || asked.includes(layer.name)),
                    )
                    .map((layer) => layer.name);
            win.__layers = out;
            return Promise.resolve(out);
          }
          if (cmd === "chosen_layers") {
            return Promise.resolve(win.__layers ?? answers.chosen_layers ?? []);
          }
          // §16's pack. Held between calls, and dropped rather than stored when
          // it is a slug this build does not have — Rust's own round trip,
          // mirrored, because a picker that showed what was *asked for* rather
          // than what was *kept* would claim a curriculum the coach is not
          // teaching from.
          if (cmd === "set_chosen_pack") {
            const known = (answers.knowledge_packs ?? []) as { id: string }[];
            const asked = (args.pack ?? "") as string;
            win.__pack = known.some((p) => p.id === asked) ? asked : "";
            return Promise.resolve(win.__pack);
          }
          // §8's axis. Held between calls, and it writes §79's locks, because
          // the claim this makes is that one press reaches seven controls: a
          // fixed answer would make a level that worked and one that did
          // nothing look the same.
          // §80's answer. Held between calls, and a rejection takes the claim
          // off the row rather than only marking it — that is the whole of what
          // "reject" has to mean, and a stub that only flipped a flag would let
          // an interface which re-offered a refused claim pass.
          if (cmd === "answer_persona") {
            const rows = (win.__persona ??
              answers.learned_persona ??
              []) as Record<string, unknown>[];
            const next = rows.map((row) =>
              row.slug === args.slug
                ? {
                    ...row,
                    verdict: args.verdict,
                    says: args.verdict === "rejected" ? "" : row.says,
                    because: args.verdict === "rejected" ? "" : row.because,
                  }
                : row,
            );
            win.__persona = next;
            return Promise.resolve(next);
          }
          if (cmd === "learned_persona") {
            return Promise.resolve(win.__persona ?? answers.learned_persona);
          }
          if (cmd === "set_adaptation_level") {
            const known = (answers.adaptation_levels ?? []) as {
              slug: string;
              adapts: boolean;
            }[];
            const asked = args.level as string;
            const step = known.find((l) => l.slug === asked);
            if (!step) {
              return Promise.reject(
                new Error(`${JSON.stringify(asked)} is not one of §8's levels`),
              );
            }
            // Rust's rule, mirrored: every lock on below Adaptive, none above.
            const locks = (answers.cockpit_locks ?? []) as { slug: string }[];
            const held = {
              level: step.slug,
              departures: [] as string[],
              locked: step.adapts ? [] : locks.map((lock) => lock.slug),
            };
            win.__standing = held;
            return Promise.resolve(held);
          }
          if (cmd === "standing") {
            return Promise.resolve(win.__standing ?? answers.standing);
          }
          if (cmd === "chosen_pack") {
            return Promise.resolve(win.__pack ?? answers.chosen_pack ?? "");
          }
          // §54's apply. Held between calls, like every other picker here: the
          // claim is that one press reaches six systems, and a fixed answer
          // would make a preset that worked and one that did nothing look the
          // same.
          if (cmd === "apply_setup") {
            const night = ((answers.setups ?? []) as {
              slug: string;
              workspace: string;
              theme: string;
              changes: string[];
            }[]).find((n) => n.slug === args.setting);
            if (!night) {
              return Promise.reject(
                new Error(`${JSON.stringify(args.setting)} is not a kind of night djmanzo knows`),
              );
            }
            win.__setUp = night.slug;
            return Promise.resolve({
              workspace: night.workspace,
              theme: night.theme,
              changes: night.changes,
            });
          }
          if (cmd === "keep_workspace") {
            const held = (win.__kept ??= []) as { name: string }[];
            const name = String(args.name ?? "").trim();
            if (!name) {
              return Promise.reject(new Error("an arrangement needs a name to be found by"));
            }
            const shipped = (answers.cockpit_workspaces ?? []) as { name: string }[];
            const clash = shipped.find(
              (w) => w.name.trim().toLowerCase() === name.toLowerCase(),
            );
            if (clash) {
              return Promise.reject(
                new Error(
                  `djmanzo already ships an arrangement called "${clash.name}" — pick another name`,
                ),
              );
            }
            const next = held.filter((w) => w.name.toLowerCase() !== name.toLowerCase());
            next.push({ ...(args.workspace as object), name } as { name: string });
            next.sort((a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase()));
            win.__kept = next;
            return Promise.resolve(next);
          }
          if (cmd === "forget_workspace") {
            const held = (win.__kept ??= []) as { name: string }[];
            const name = String(args.name ?? "").trim().toLowerCase();
            win.__kept = held.filter((w) => w.name.trim().toLowerCase() !== name);
            return Promise.resolve(win.__kept);
          }
          if (cmd === "set_cockpit_workspace") {
            // Recorded as well as echoed. What a test needs to know about a
            // save is that it *happened and carried the right thing* — an
            // arrangement drawn on screen and never written looks identical
            // until the application is reopened, which is the defect §89's
            // four-deck configuration found.
            ((win.__saved ??= []) as unknown[]).push(args.workspace);
            // A test may ask for the write to fail, by answering this command
            // with the string "reject". The shell's whole posture towards a
            // failed save is that the DJ pressed something and it happened
            // anyway — a preferences file that cannot be written is not a
            // reason to undo it under them — and that posture is a branch
            // nothing exercised, so it was free to rot.
            if (answers.set_cockpit_workspace === "reject") {
              return Promise.reject(new Error("the preferences file is read-only"));
            }
            // The permits come back with the workspace, derived the way
            // `cockpit::Workspace::permits` derives them. Mirrored here rather
            // than fixed at "everything permitted", because the whole of §78
            // is that a lock the DJ just set takes effect now — a stub that
            // always answered "permitted" would make a working freeze and a
            // broken one look identical.
            //
            // The mapping is `Lock::stops`'s and a Rust test keeps the two
            // spellings the same; what it must not do is grow a second opinion
            // about which lock stops what.
            const locked = ((args.workspace as { locked?: string[] }).locked ?? []) as string[];
            const stops = (...names: string[]) => !names.some((n) => locked.includes(n));
            return Promise.resolve({
              workspace: args.workspace,
              notes: [],
              permits: {
                rearrange: stops("workspace", "arrangement", "assistant"),
                resize: stops("density"),
                retheme: stops("theme"),
                restyle: stops("waveform"),
              },
            });
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
              const startFrame =
                (held.start_frame as number) + beats * beatFrames;
              // §26's transition end. A *length*, worked out from the position
              // the handle was dropped at, because that is what djmanzo
              // answers with: `Transition::end_at` rounds to a whole beat and
              // clamps to what a transition can be, and both of those are
              // tested in Rust. The stub needs only enough of it to move the
              // mark -- a floor of one beat, so a drag past the start does not
              // put the end behind it and make a working drag look broken.
              const length =
                args.endFrame !== undefined
                  ? Math.max(
                      1,
                      Math.round(
                        (Number(args.endFrame) - startFrame) / beatFrames,
                      ),
                    )
                  : ((args.lengthBeats ?? held.length_beats) as number);
              win.__transition = {
                ...held,
                length_beats: length,
                style: restyled,
                shape:
                  offered.find((style) => style.name === restyled)?.shape ??
                  held.shape,
                start_beat: (held.start_beat as number) + beats,
                start_frame: startFrame,
                // Derived from the start and the length rather than nudged
                // along with the start, which is the same trap the comment
                // above records: a stub that moved the mark for one kind of
                // adjustment and not another would let a shortened transition
                // go on drawing its old end.
                end_frame: startFrame + length * beatFrames,
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
          /**
           * §29's AI hover, answered per deck for the same reason the handles
           * are: the one thing a browser can prove about a suggestion is that
           * the action it offers names the deck it was drawn on. `deck N` in
           * the override is rewritten to the deck actually asked about.
           */
          if (cmd === "control_suggestions") {
            const n = Number(args.deck ?? 1);
            const offered = (answers.control_suggestions ?? []) as {
              control: string;
              to: number | null;
              action: string;
              because: string;
            }[];
            return Promise.resolve(
              offered.map((found) => ({
                ...found,
                action: found.action.replace(/^deck \d+ /, `deck ${n} `),
              })),
            );
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
          // The assistant's posture, held between calls so a press is visible
          // to the next question rather than vanishing into a stub.
          if (cmd === "assistant_set_posture") {
            win.__posture = args.posture;
            return Promise.resolve(null);
          }
          /**
           * §81's fits, answered the way Rust answers them: **nothing is
           * offered where nothing would change.** That rule is the reason the
           * panel can be read at all — an offer that stayed up after being
           * taken would look identical to one that did nothing — so the stub
           * mirrors it rather than answering a fixed pair forever, the same
           * way `night_setting` mirrors `note_night`.
           */
          if (cmd === "night_fits") {
            const offered = (answers.night_fits ?? {
              density: null,
              posture: null,
              withheld: [],
            }) as Record<string, { to: string } | null | string[]>;
            const bands = (answers.density_bands ?? []) as [
              number,
              string,
              number,
            ][];
            const density = offered.density as { to: string } | null;
            const posture = offered.posture as { to: string } | null;
            const wearing = document.documentElement.style.getPropertyValue(
              "--density",
            );
            const wanted = bands.find(
              ([, name]) =>
                name.toLowerCase().replace(/ /g, "-") === density?.to,
            );
            return Promise.resolve({
              density:
                wanted && String(wanted[2]) === wearing ? null : density,
              posture: win.__posture === posture?.to ? null : posture,
              withheld: offered.withheld ?? [],
            });
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
          // An answer that changes over time, given as a list: each ask
          // takes the next, and the last is kept. What a command says while
          // something loads in Rust and after it has loaded is two answers,
          // and a fixed one could not show the panel moving from one to the
          // other.
          const answer = answers[cmd];
          if (Array.isArray(answer) && (answers.__sequences as string[] | undefined)?.includes(cmd)) {
            return Promise.resolve(answer.length > 1 ? answer.shift() : answer[0]);
          }
          return Promise.resolve(answer ?? null);
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
  // than for a network idle that says nothing about whether it rendered. A
  // detached panel (`?panel=`) has no crossfader — it is one panel and nothing
  // else — so it is waited for by its own shell.
  if (new URL(url, "http://localhost").searchParams.has("panel")) {
    await page.locator("main.detached").waitFor();
  } else {
    await page.getByRole("slider", { name: "Crossfader" }).waitFor();
  }
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
/// Registers the list on first ask, so the answer is the same array whenever
/// it is called — before `openShell` or after it. See the note in `openShell`
/// for what asking too early used to cost.
export function errorsThrown(page: Page): string[] {
  let found = pageErrors.get(page);
  if (!found) {
    found = [];
    pageErrors.set(page, found);
  }
  return found;
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
