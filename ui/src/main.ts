import { mount } from "svelte";
import App from "./App.svelte";
import Detached from "./Detached.svelte";
import { armBenchmark } from "./bench";
import { armRenderBenchmark } from "./renderbench";
import { armDemo } from "./demo";
// Importing it is what applies it: the module stamps the saved theme on the
// root element as it initialises, before anything mounts, so the first paint is
// already the right way round rather than flashing dark and then correcting.
import "./theme.svelte";
import "./app.css";

const target = document.getElementById("app");
if (!target) {
  throw new Error("missing #app mount point");
}

/**
 * A detached panel opens the same page with `?panel=` on it.
 *
 * Routing here rather than inside `App` because the two are not the same
 * component wearing a hat: a detached window mounts one panel and nothing else,
 * and an `App` that could hide everything but one panel would carry all of its
 * state and all of its listeners for no reason. See `crate::monitors`.
 */
const panel = new URLSearchParams(window.location.search).get("panel");
if (panel) {
  document.title = `djmanzo — ${panel}`;
}

// Listens for a `bench` event that only fires when DJMANZO_BENCH is set.
armBenchmark();
// Likewise for DJMANZO_RENDERBENCH, which chooses the living interface's
// rendering strategy rather than measuring the waveform.
armRenderBenchmark();
// And DJMANZO_DEMO, which loads a couple of tracks so a headless run has
// something to show.
armDemo();

export default panel
  ? mount(Detached, { target, props: { panel } })
  : mount(App, { target });
