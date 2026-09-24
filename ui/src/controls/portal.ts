/**
 * Move `node` to the end of the document's body while it exists.
 *
 * A Svelte action, used as `use:portal`. For a panel that floats over the
 * rest of the window: inside a deck it is cut off by the deck's scrolling
 * zone, and in WebKitGTK -- the engine djmanzo ships on -- the deck's layout
 * containment makes `position: fixed` relative to the deck rather than the
 * window, so a panel placed under a stem strip was drawn somewhere below the
 * deck's fold and the deck scrolled to it. Chromium drew it where it was
 * asked, which is why a browser test alone did not find it.
 */
export function portal(node: HTMLElement) {
  document.body.appendChild(node);
  return {
    destroy() {
      node.remove();
    },
  };
}
