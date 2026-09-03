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
  sidelist: [],
  library_search: [],
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
) {
  const state = { ...snapshot, master: { ...snapshot.master, ...master } };
  const thrown: string[] = [];
  pageErrors.set(page, thrown);
  page.on("pageerror", (error) => thrown.push(error.message));
  await page.setViewportSize(WINDOW);
  await page.addInitScript(
    ([answers, state]: [Record<string, unknown>, unknown]) => {
      const handlers = new Map<string, number>();
      const win = window as unknown as Record<string, unknown>;

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
          if (cmd === "set_cockpit_workspace") {
            return Promise.resolve({ workspace: args.workspace, notes: [] });
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
    [ANSWERS, state] as [Record<string, unknown>, unknown],
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
 * The centre of a control, or null when it is not on the page at all.
 *
 * The *centre* rather than the top edge: a fader whose top is on screen and
 * whose thumb is not is a fader a DJ cannot use, and the top edge would pass.
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
