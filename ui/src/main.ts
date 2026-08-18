import { mount } from "svelte";
import App from "./App.svelte";
import { armBenchmark } from "./bench";
import { armRenderBenchmark } from "./renderbench";
// Importing it is what applies it: the module stamps the saved theme on the
// root element as it initialises, before anything mounts, so the first paint is
// already the right way round rather than flashing dark and then correcting.
import "./theme.svelte";
import "./app.css";

const target = document.getElementById("app");
if (!target) {
  throw new Error("missing #app mount point");
}

// Listens for a `bench` event that only fires when DJMANZO_BENCH is set.
armBenchmark();
// Likewise for DJMANZO_RENDERBENCH, which chooses the living interface's
// rendering strategy rather than measuring the waveform.
armRenderBenchmark();

export default mount(App, { target });
