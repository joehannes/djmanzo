/**
 * Files dragged onto the window from the desktop.
 *
 * # Why this is not an HTML5 `ondrop`
 *
 * Two reasons, and the second is the one that decides it:
 *
 * - A webview's `DataTransfer` gives a `File` object and **not a path**. The
 *   engine loads by path — it hands the name to `dj-decode`, which opens it —
 *   so a `File` would have to be read into memory in JavaScript and written
 *   back out to disk to be loadable, which for a 60 MB FLAC is absurd.
 * - Tauri intercepts desktop drops before the page sees them, so the HTML5
 *   event does not fire at all unless drag-drop is disabled window-wide, which
 *   would also break the playlist reordering that *does* use it.
 *
 * So the drop arrives through Tauri's own event, which carries real paths, and
 * the *position* is hit-tested against whatever registered itself below.
 *
 * # Why a registry rather than a prop
 *
 * The event is window-wide and arrives with a point, not a target. Something
 * has to turn a point into "deck 2", and only the decks know where they are. So
 * each one registers its element and its handler, and this asks them in turn.
 */
import { getCurrentWebview } from "@tauri-apps/api/webview";

type Target = {
  element: HTMLElement;
  onDrop: (paths: string[]) => void;
};

const targets = new Set<Target>();

/** Which target is under the pointer, or `null` over nothing that takes files. */
let hovered = $state<HTMLElement | null>(null);

let listening = false;
let stop: (() => void) | null = null;

/**
 * Whether a desktop drag is currently over `element`.
 *
 * Read by a deck so it can light up. A drag with no feedback is a drag you
 * cannot aim: without this, a DJ dropping a file learns which deck they hit by
 * hearing it.
 */
export function isOver(element: HTMLElement | null): boolean {
  return element !== null && hovered === element;
}

/**
 * Take files dropped on `element`.
 *
 * Returns the teardown, so a component can register on mount and forget.
 */
export function acceptFiles(
  element: HTMLElement,
  onDrop: (paths: string[]) => void,
): () => void {
  const target: Target = { element, onDrop };
  targets.add(target);
  void listen();
  return () => {
    targets.delete(target);
    if (hovered === element) hovered = null;
  };
}

/**
 * The topmost target under a point.
 *
 * Reverse order, so a target registered later -- which is drawn on top -- wins
 * over one underneath it. Registration order is not paint order in general, but
 * it is here, because panels open over decks and never the other way round.
 */
function at(x: number, y: number): Target | null {
  const found = [...targets].reverse().find((target) => {
    const box = target.element.getBoundingClientRect();
    return x >= box.left && x <= box.right && y >= box.top && y <= box.bottom;
  });
  return found ?? null;
}

async function listen(): Promise<void> {
  if (listening) return;
  listening = true;
  try {
    const unlisten = await getCurrentWebview().onDragDropEvent((event) => {
      const payload = event.payload;
      if (payload.type === "over") {
        // The position is in physical pixels; the DOM is in CSS ones.
        const scale = window.devicePixelRatio || 1;
        hovered =
          at(payload.position.x / scale, payload.position.y / scale)?.element ??
          null;
        return;
      }
      if (payload.type === "drop") {
        const scale = window.devicePixelRatio || 1;
        const target = at(
          payload.position.x / scale,
          payload.position.y / scale,
        );
        hovered = null;
        if (target && payload.paths.length > 0) {
          target.onDrop(payload.paths);
        }
        return;
      }
      hovered = null;
    });
    stop = unlisten;
  } catch {
    // Not running under Tauri -- a browser preview, or a test. Dropping simply
    // does nothing, which is better than failing to start.
    listening = false;
  }
}

/** Stop listening. Only used by tests; the window's listener outlives any deck. */
export function reset(): void {
  stop?.();
  stop = null;
  listening = false;
  targets.clear();
  hovered = null;
}
