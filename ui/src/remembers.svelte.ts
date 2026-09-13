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
  favouritePadPages,
  keptControls,
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
    const [pages, controls] = await Promise.all([favouritePadPages(), keptControls()]);
    remembers.pages = pages;
    remembers.controls = controls;
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
