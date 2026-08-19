/**
 * Load and play a couple of tracks on startup, for development.
 *
 * Not a feature. The interface can only be judged with something actually
 * playing — a still screenshot of an empty mixer says nothing about whether the
 * watershed reads — and on a headless machine there is nobody to click Load.
 *
 * Triggered by `DJMANZO_DEMO=<folder>`, so it never runs in a normal session.
 *
 * The interface *asks* whether there is a demo folder rather than waiting to be
 * told: the first version emitted an event three seconds after startup and
 * raced the webview, so on a cold dev server the run looked like it had
 * silently failed.
 */
import { invoke } from "@tauri-apps/api/core";

import {
  createPlaylist,
  dispatch,
  libraryAddFolder,
  librarySearch,
  loadSample,
  loadTrack,
  openDevice,
} from "./api";

export function armDemo(): void {
  void (async () => {
    const folder = await invoke<string | null>("demo_folder").catch(() => null);
    if (!folder) return;
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
      // One of each kind of crate, first, so the tree has read them by the
      // time it mounts — the demo talks to the commands directly and cannot
      // tell a component that its data changed.
      await step("make a list", () => createPlaylist("Friday", null, "list"));
      await step("make a folder", () => createPlaylist("Latin", null, "folder"));
      await step("make a smart folder", () =>
        createPlaylist("Fast", null, "smart", "bpm > 100"));

      await step("open device", () => openDevice(null, null, 256));
      await step("add folder", () => libraryAddFolder(folder));
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

      // Exercise the control surface, so a headless run shows the strata, the
      // shear, an eddy and some stones rather than two plain rivers.
      await new Promise((r) => setTimeout(r, 2000));
      await step("kill deck 1 lows", () => dispatch("deck 1 eq_low 0"));
      await step("boost deck 1 highs", () => dispatch("deck 1 eq_high 1.6"));
      await step("filter deck 2", () => dispatch("deck 2 filter -0.6"));
      await step("cue deck 1", () => dispatch("deck 1 hotcue_set 1"));
      await step("loop deck 2", () => dispatch("deck 2 loop 4"));
      // Slip armed on the looping deck, so the shadow marker is visible.
      await step("slip deck 2", () => dispatch("deck 2 slip_on"));

      // And a sample in the first slot, so a headless run can check the
      // sampler too — the pads, the panel and the mixing path all need
      // something loaded before any of them show anything.
      const first = found[0]?.path;
      if (first) {
        await step("load sample", () => loadSample(1, 1, first));
        await step("loop the sample", () => dispatch("sampler 1 loop"));
      }


    } catch (e) {
      console.error("demo", e);
    }
  })();
}
