import { mount } from "svelte";
import App from "./App.svelte";
import { armBenchmark } from "./bench";
import "./app.css";

const target = document.getElementById("app");
if (!target) {
  throw new Error("missing #app mount point");
}

// Listens for a `bench` event that only fires when DJMANZO_BENCH is set.
armBenchmark();

export default mount(App, { target });
