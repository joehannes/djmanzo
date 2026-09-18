/**
 * §90, at the seam between the interface and Rust.
 *
 * > Audio is the highest-priority subsystem. […] Do not let the GUI starve it.
 *
 * `snapshot_budget.rs` holds the cost of *building* a frame. This holds the
 * cost of *receiving* one, which is a different number with a different way of
 * going wrong — and it went wrong.
 *
 * # The defect this was written for
 *
 * `App.svelte` receives every frame as `snapshot = next`. Svelte's `$state`
 * proxies are deep, so replacing the root makes every deck a **new** proxy and
 * every read through it a **new** signal. An `$effect` that reads
 * `deck.analysis` and then calls Rust therefore runs on every frame — not when
 * the analysis changes. Three of them did:
 *
 * | call | per frame | what it is |
 * |---|---|---|
 * | `waveform_info` | 4 | a library query and a clone of the whole trajectory |
 * | `phrase_grid` | 2 | the grid anchor and spacing |
 * | `pad_pages` | 2 | a fixed table, fetched "once" |
 *
 * Ten frames of ordinary playback cost **forty-two** IPC round trips. At the
 * pump's rate that is over two hundred a second, on the thread that also
 * serves the engine's controls, for answers that change twice a track.
 *
 * Every one of those calls had a comment saying it was not on the snapshot
 * *because* that would be too expensive. The comments were right about the
 * intent and nothing checked the behaviour.
 *
 * # Why this is a ratchet rather than a list
 *
 * A test naming the three would pass the day a fourth arrived. This measures
 * the seam: **what does a frame of ordinary playback cost the interface**. A
 * new effect that re-asks per frame fails it whatever it is called.
 *
 * Measure with **spaced** frames. Rapid ones batch into a single effect run
 * and report a healthy-looking figure; the first attempt at measuring this
 * concluded there was no defect for exactly that reason.
 */
import { expect, test } from "@playwright/test";

import { errorsThrown, openShell } from "./shell";

/**
 * What ten frames of playback may cost.
 *
 * The real figure is 2 — `mission_bar` and `staged_current`, each once, on
 * their own throttles rather than per frame. Four is that plus room for a
 * genuinely periodic reading to arrive, and **lower it whenever the real
 * figure drops**: a ratchet that has drifted above the truth has stopped
 * holding anything.
 */
const BUDGET = 4;

/** How many frames are emitted, and how far apart the pump really spaces them. */
const FRAMES = 10;
const APART_MS = 30;

test("ordinary playback does not make the interface interrogate Rust", async ({
  page,
}) => {
  await openShell(page, "/");
  await expect(page.locator(".overview").first()).toBeVisible();
  // Everything a fresh mount asks for is not what this measures.
  await page.waitForTimeout(800);
  await page.evaluate(() => {
    (window as unknown as Record<string, unknown>).__asked = [];
  });

  // Only the playhead moves, which is what a record playing looks like.
  await page.evaluate(
    async ({ frames, apart }) => {
      const win = window as unknown as {
        __lastState: { decks: { position_frames: number }[] };
        __emit: (next: unknown) => void;
      };
      for (let i = 0; i < frames; i += 1) {
        const next = structuredClone(win.__lastState);
        next.decks[0].position_frames += 2048;
        win.__emit(next);
        await new Promise((resolve) => setTimeout(resolve, apart));
      }
    },
    { frames: FRAMES, apart: APART_MS },
  );
  await page.waitForTimeout(500);

  const asked = await page.evaluate(() => {
    const all = (window as unknown as { __asked: string[] }).__asked ?? [];
    const counted: Record<string, number> = {};
    // Tauri's own event plumbing is not a question about the mix.
    for (const cmd of all.filter((c) => !c.startsWith("plugin:"))) {
      counted[cmd] = (counted[cmd] ?? 0) + 1;
    }
    return counted;
  });
  const total = Object.values(asked).reduce((sum, n) => sum + n, 0);

  expect(
    total,
    `${FRAMES} frames of playback cost ${total} calls to Rust: ` +
      `${JSON.stringify(asked)}.\n\nAn effect that reads a deck field and then ` +
      `calls Rust runs on every frame, because the snapshot is replaced rather ` +
      `than patched. Guard the call with a key — see ui/src/waveformAsks.ts.`,
  ).toBeLessThanOrEqual(BUDGET);
  expect(errorsThrown(page)).toEqual([]);
});
