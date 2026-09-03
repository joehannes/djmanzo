/**
 * The handoff between the library and the Prepare space.
 *
 * # Why this module exists
 *
 * The directive's §21 asks for a **first-class Prepare space** and says a
 * track should move `Library → Candidate → Prepare → Next → Deck` *"without
 * being copied into awkward parallel systems"*, with consistent gesture
 * semantics — and it names the failure to avoid, which is the inconsistent
 * Prepare behaviour Engine DJ users complain about.
 *
 * Prepare used to be a child of the browser, so the browser could hand it a
 * track by passing a prop down. Making it first class means it is a dock
 * surface of its own, open beside the library or below it or not at all, and
 * two surfaces that are siblings cannot pass props to each other.
 *
 * So the handoff is one module-level rune. Not a parallel system: the sidelist
 * itself still lives in the library database exactly as it did, and this holds
 * nothing but *the track the DJ just pointed at*, for the moment between the
 * gesture and the Prepare space consuming it.
 *
 * # Why a rune in a module rather than a context or an event
 *
 * Svelte's context is set by an ancestor and read by a descendant, which is
 * the relationship this change exists to remove. A custom DOM event would
 * work and would put the interface's own state on the document, where
 * anything can dispatch it and nothing can be sure who did. A module rune is
 * the smallest thing that is exactly what it looks like: one value, one
 * writer, one reader.
 */

/**
 * The track the browser has asked to set aside, if any.
 *
 * A single value rather than a queue, because the gesture is one press and the
 * Prepare space consumes it immediately. A queue would only matter if the
 * space could be closed while presses continued — and see [`setAside`] for why
 * that case is answered by not losing the press rather than by buffering it.
 */
let pending = $state<string | null>(null);

/** What the Prepare space should pick up, or null. */
export function pendingTrack(): string | null {
  return pending;
}

/**
 * Point at a track from the browser.
 *
 * Sets the value whether or not a Prepare space is open. Nothing consumes it
 * until one is, so opening Prepare after pressing → still finds the track
 * waiting — which is the consistent semantics §21 asks for, and the opposite
 * of a press that silently does nothing because a panel was closed.
 */
export function setAside(id: string) {
  pending = id;
}

/** Called by the Prepare space once it has taken the track. */
export function consumed() {
  pending = null;
}
