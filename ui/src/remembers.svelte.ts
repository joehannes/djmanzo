/**
 * §8 Level 1's two loose preferences, held once for everything that reads them.
 *
 * > **Level 1 — Remember.** The software remembers: […] favorite pad pages […]
 * > preferred controls.
 *
 * Seven of §8's nine are set by *using* djmanzo — you drag a panel, you tick a
 * column, you click a heading — and they are read where they are used. These
 * two are not: a favourite pad page and a control kept within reach are things
 * a DJ decides once, in Settings, and then meets somewhere else entirely (the
 * pad zone on every deck, §74's rail).
 *
 * So they are held here rather than fetched per component. Four decks each
 * asking Rust for the same list would be four answers that can disagree, and a
 * change made in Settings would not reach a pad zone already on screen until it
 * was remounted — which is exactly the class of defect §8 is about.
 *
 * Rust still decides what the lists *mean*: both setters return what will
 * actually be used, with unknown names dropped and repeats collapsed, and that
 * is what is kept here. Nothing in this file invents a page or a control.
 */

import {
  chosenLayers,
  favouritePadPages,
  keptControls,
  setChosenLayers,
  setWaveformColouring,
  waveformColouring,
  setFavouritePadPages,
  setKeptControls,
} from "./api";

/**
 * The shared state.
 *
 * A `$state` object rather than two exported `$state` bindings, because an
 * exported binding is read once at import and never again — the runes have to
 * be reached through something for a reader to stay reactive.
 */
export const remembers = $state({
  /** Pad pages the DJ has starred, in their order. */
  pages: [] as string[],
  /** Controls the DJ keeps on §74's rail whatever the deck is doing. */
  controls: [] as string[],
  /**
   * §25's layers the waveform draws, in §25's order.
   *
   * Read by both halves of the renderer: the overlay elements check it
   * directly, and the three grid layers travel in the tile URL, where they are
   * part of the cache key. Empty only before Rust has answered — the chooser
   * never returns nothing, because a waveform rebuilt layer by layer after one
   * stray click would be a worse surface than one with no picker at all.
   */
  layers: [] as string[],
  /**
   * §110: how the spectral balance is coloured — `light` (the spectrum as
   * light, red to violet, white when everything is there) or `bands` (the
   * three EQ bands). Travels in the tile URL, like the grid layers.
   */
  colouring: "light",
  /** True once Rust has answered, so a picker can tell empty from not-yet. */
  loaded: false,
});

/**
 * Read both from Rust.
 *
 * Idempotent and safe to call from several components: whoever gets there
 * first pays for the round trip and the rest read what it stored.
 */
export async function loadRemembers(): Promise<void> {
  try {
    const [pages, controls, layers] = await Promise.all([
      favouritePadPages(),
      keptControls(),
      chosenLayers(),
    ]);
    remembers.pages = pages;
    remembers.controls = controls;
    remembers.layers = layers;
    // Its own guard: an answer this build does not recognise keeps the
    // default rather than becoming a tile URL Rust refuses.
    const colouring = await waveformColouring().catch(() => null);
    if (colouring === "light" || colouring === "bands") remembers.colouring = colouring;
  } catch {
    // Nothing starred and nothing kept — which is what djmanzo does when a DJ
    // has never set either, so a preferences file that cannot be read costs
    // the preference and not the pad zone.
  } finally {
    // Set even on failure. A picker that waited forever for an answer that is
    // not coming would show a spinner where the checkboxes are.
    remembers.loaded = true;
  }
}

/** Choose how the spectral balance is coloured, and keep what Rust kept. */
export async function chooseColouring(colouring: string): Promise<void> {
  const before = remembers.colouring;
  // Optimistic, like the layers: the lane redraws now rather than after a
  // round trip, and a refusal puts it back.
  remembers.colouring = colouring;
  try {
    remembers.colouring = await setWaveformColouring(colouring);
  } catch {
    remembers.colouring = before;
  }
}

/** Star or unstar a pad page, and keep what Rust says will be used. */
export async function starPage(page: string, on: boolean): Promise<void> {
  const next = on
    ? [...remembers.pages.filter((held) => held !== page), page]
    : remembers.pages.filter((held) => held !== page);
  // Optimistic, like the column picker: the pad zones reorder now and the
  // preferences file catches up.
  remembers.pages = next;
  try {
    remembers.pages = await setFavouritePadPages(next);
  } catch {
    // Keeping what the DJ ticked. A preferences file that cannot be written is
    // not a reason to un-tick a box under them.
  }
}

/** Keep or release a control on §74's rail. */
export async function keepControl(control: string, on: boolean): Promise<void> {
  const next = on
    ? [...remembers.controls.filter((held) => held !== control), control]
    : remembers.controls.filter((held) => held !== control);
  remembers.controls = next;
  try {
    remembers.controls = await setKeptControls(next);
  } catch {
    // As above.
  }
}

/** Show or hide one of §25's layers, and keep what Rust says will be drawn. */
export async function showLayer(layer: string, on: boolean): Promise<void> {
  const next = on
    ? [...remembers.layers.filter((held) => held !== layer), layer]
    : remembers.layers.filter((held) => held !== layer);
  // Optimistic, like the pickers above — but the round trip matters more here
  // than anywhere else, because Rust puts the two layers that *are* the
  // waveform back whether they were asked for or not, and an empty ask comes
  // back as the whole instrumentation.
  remembers.layers = next;
  try {
    remembers.layers = await setChosenLayers(next);
  } catch {
    // Keeping what the DJ ticked, as everywhere else here.
  }
}

/**
 * Whether one of §25's layers is drawn.
 *
 * An empty list means **everything**, not nothing. It is empty for the one
 * frame between a component mounting and Rust answering, and a waveform that
 * flashed layer-less on every mount would be a worse surface than one that
 * waited — and it is also what a DJ who has never opened the picker has, where
 * the answer is plainly "all of them".
 */
export function showing(layer: string): boolean {
  return remembers.layers.length === 0 || remembers.layers.includes(layer);
}
