/**
 * The ADR-0004 gate, measured.
 *
 * The question is not whether Rust can produce tiles fast enough — that is
 * already measured at 0.46 ms each. It is whether **WebKitGTK can composite a
 * CSS-transformed strip at 60 fps on four decks**, which can only be answered
 * from inside the webview on the target platform.
 *
 * Triggered by the `DJMANZO_BENCH` environment variable so it never runs in a
 * normal session.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { dispatch, loadTrack, openDevice } from "./api";

/** How long to measure, once everything is playing. */
const MEASURE_MS = 8_000;
/** Ignore the first frames: layer promotion and image decode land there. */
const WARMUP_MS = 2_000;

function percentile(sorted: number[], p: number): number {
  if (sorted.length === 0) return 0;
  const index = Math.min(sorted.length - 1, Math.floor(sorted.length * p));
  return sorted[index];
}

async function measure(label: string): Promise<void> {
  const deltas: number[] = [];
  const start = performance.now();
  let previous = start;

  await new Promise<void>((resolve) => {
    const tick = () => {
      const now = performance.now();
      const delta = now - previous;
      previous = now;
      if (now - start > WARMUP_MS) deltas.push(delta);
      if (now - start > WARMUP_MS + MEASURE_MS) {
        resolve();
        return;
      }
      requestAnimationFrame(tick);
    };
    requestAnimationFrame(tick);
  });

  deltas.sort((a, b) => a - b);
  const mean = deltas.reduce((a, b) => a + b, 0) / Math.max(deltas.length, 1);

  await invoke("report_bench", {
    label,
    fps: 1000 / mean,
    p50Ms: percentile(deltas, 0.5),
    p95Ms: percentile(deltas, 0.95),
    worstMs: deltas[deltas.length - 1] ?? 0,
  });
}

export function armBenchmark(): void {
  void listen<string>("bench", async (event) => {
    const path = event.payload;
    try {
      // Single device: the benchmark measures webview compositing, and a
      // second sound card would only add a resampler to the audio path.
      await openDevice(null, null, 256);

      // Baseline: the interface running with nothing on the decks. Anything
      // measured later has to be read against this, not against 60 fps in the
      // abstract — a headless X server has its own ceiling.
      await measure("idle (no tracks)");

      for (const deck of [1, 2, 3, 4]) {
        await loadTrack(deck, path);
      }
      await measure("4 decks loaded, paused");

      // Scale the number of *moving* lanes one at a time. If frame time grows
      // linearly with moving layer area, the bottleneck is fill rate — i.e. the
      // compositor is blitting on the CPU — rather than anything in our JS or
      // DOM. That distinction decides whether this is our bug or the absence of
      // a GPU.
      await dispatch("deck 1 play");
      await measure("1 deck scrolling");

      await dispatch("deck 2 play");
      await measure("2 decks scrolling");

      await dispatch("deck 3 play");
      await dispatch("deck 4 play");
      await measure("4 decks scrolling");
    } catch (e) {
      await invoke("report_bench", {
        label: `FAILED: ${String(e)}`,
        fps: 0,
        p50Ms: 0,
        p95Ms: 0,
        worstMs: 0,
      });
    }
  });
}
