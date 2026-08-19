/**
 * Load and play a couple of tracks on startup, for development.
 *
 * Not a feature. The interface can only be judged with something actually
 * playing — a still screenshot of an empty mixer says nothing about whether the
 * watershed reads — and on a headless machine there is nobody to click Load.
 *
 * Triggered by `DJMANZO_DEMO=<folder>`, so it never runs in a normal session.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { dispatch, libraryAddFolder, librarySearch, loadTrack, openDevice } from "./api";

export function armDemo(): void {
  void listen<string>("demo", async (event) => {
    // Each step independently, so one failure does not silently abort the
    // rest — a demo that opens no device should still load and draw tracks.
    const step = async (what: string, run: () => Promise<unknown>) => {
      try {
        await run();
      } catch (e) {
        await invoke("report_bench", {
          label: `demo: ${what} failed: ${String(e)}`,
          fps: 0,
          p50Ms: 0,
          p95Ms: 0,
          worstMs: 0,
        });
      }
    };

    try {
      await step("open device", () => openDevice(null, null, 256));
      await step("add folder", () => libraryAddFolder(event.payload));
      // A scan is two halves and only the cheap one is synchronous, so rows
      // appear before they have been identified. Wait for a couple to arrive.
      let found: Awaited<ReturnType<typeof librarySearch>> = [];
      for (let attempt = 0; attempt < 30 && found.length < 2; attempt += 1) {
        found = await librarySearch("");
        if (found.length < 2) await new Promise((r) => setTimeout(r, 1000));
      }
      await invoke("report_bench", {
        label: `demo: ${found.length} tracks found`,
        fps: 0,
        p50Ms: 0,
        p95Ms: 0,
        worstMs: 0,
      });
      for (const [index, track] of found.slice(0, 2).entries()) {
        if (!track.path) continue;
        await step(`load ${track.path}`, () => loadTrack(index + 1, track.path!));
        await step("play", () => dispatch(`deck ${index + 1} play`));
      }
    } catch (e) {
      console.error("demo", e);
    }
  });
}
