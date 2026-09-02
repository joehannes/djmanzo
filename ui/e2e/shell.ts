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
