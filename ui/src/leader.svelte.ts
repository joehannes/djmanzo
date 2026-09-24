/**
 * §117: the leader key.
 *
 * Space opens the guide at the top of the tree; each key after it goes one
 * step down, and a leaf runs. The tree is Rust's (`dj_app::leader`); this is
 * only the walk through it, here for the same reason the keyboard's lookup is:
 * a key has to answer in the frame it was pressed.
 *
 * # While the guide is open, keys belong to it
 *
 * Every key is the guide's until it closes — a key that leads nowhere is said
 * so ("nothing on j here") rather than let through. Letting it through is what
 * which-key does in an editor, and in a booth it would mean `Space d 1 j`, one
 * letter off, starting deck 2. Escape closes; Backspace goes back a step.
 * Chords with Ctrl, Alt or ⌘ are not the guide's: they close it and go on to
 * whatever owns them, so Ctrl+K still opens the palette.
 *
 * # The guide appears only when it is waited for
 *
 * Like which-key, the panel is drawn after a short pause, so a DJ who knows
 * `Space d 1 p` never sees it flash; one who stops to think does, at once.
 */
import type { LeaderNode } from "./api";
import { typing } from "./keyboard.svelte";

/** How long a pause before the guide is drawn, in milliseconds. */
export const GUIDE_AFTER_MS = 180;

/** The key a keyboard event means in the tree, or null for one it has no word for. */
export function keyOf(event: KeyboardEvent): string | null {
  if (event.key === " " || event.code === "Space") return "space";
  if (event.key.length !== 1) return null;
  // A letter typed with Caps Lock on is still that letter.
  return event.shiftKey ? event.key : event.key.toLowerCase();
}

/** What the guide shows for a key. */
export function keyLabel(key: string): string {
  return key === "space" ? "Space" : key;
}

/**
 * Where the key's letter is in the label, to be underlined: the first place
 * the letter starts a word, else the first place it appears at all; -1 when
 * it is not in the label (a digit, `?`).
 */
export function mnemonicAt(label: string, key: string): number {
  if (key.length !== 1 || !/[a-z]/i.test(key)) return -1;
  const lower = label.toLowerCase();
  const letter = key.toLowerCase();
  for (let i = 0; i < lower.length; i++) {
    if (lower[i] === letter && (i === 0 || !/[a-z0-9]/i.test(lower[i - 1]))) return i;
  }
  return lower.indexOf(letter);
}

export class Leader {
  /** The tree, when it has arrived. Nothing opens before it has. */
  tree = $state<LeaderNode | null>(null);
  /** The groups gone into, the top of the tree first. Empty when closed. */
  path = $state<LeaderNode[]>([]);
  /** Whether the guide is drawn — after a pause, once open. */
  shown = $state(false);
  /** The last key that led nowhere, for the guide to say so. */
  missed = $state<string | null>(null);
  #run: (run: string) => void;
  #delay: number;
  #timer: ReturnType<typeof setTimeout> | null = null;

  constructor(run: (run: string) => void, delay = GUIDE_AFTER_MS) {
    this.#run = run;
    this.#delay = delay;
  }

  /** Whether the guide is taking keys. */
  get open(): boolean {
    return this.path.length > 0;
  }

  /** The group the next key chooses from. */
  get here(): LeaderNode | null {
    return this.path[this.path.length - 1] ?? null;
  }

  /** Open at the top of the tree. */
  start(): void {
    if (!this.tree) return;
    this.path = [this.tree];
    this.missed = null;
    this.#schedule();
  }

  /** Close, running nothing. */
  close(): void {
    this.path = [];
    this.missed = null;
    this.shown = false;
    if (this.#timer) clearTimeout(this.#timer);
    this.#timer = null;
  }

  /** One step back; closed from the top. */
  back(): void {
    if (this.path.length <= 1) this.close();
    else {
      this.path = this.path.slice(0, -1);
      this.missed = null;
    }
  }

  /** Choose a child of where the guide is, by its key. */
  choose(key: string): void {
    const here = this.here;
    if (!here) return;
    const child = here.children.find((each) => each.key === key);
    if (!child) {
      this.missed = key;
      this.shown = true;
      return;
    }
    if (child.run) {
      const run = child.run;
      this.close();
      this.#run(run);
      return;
    }
    this.path = [...this.path, child];
    this.missed = null;
  }

  /**
   * A key going down. True when the guide took it, so the caller swallows it
   * before the keyboard map or anything else sees it.
   */
  press(event: KeyboardEvent): boolean {
    if (event.ctrlKey || event.metaKey || event.altKey) {
      if (this.open) this.close();
      return false;
    }
    if (!this.open) {
      if (keyOf(event) !== "space" || event.repeat || typing(event.target) || !this.tree) return false;
      this.start();
      return true;
    }
    if (event.key === "Escape") this.close();
    else if (event.key === "Backspace") this.back();
    else if (!event.repeat) {
      const key = keyOf(event);
      if (key !== null) this.choose(key);
    }
    return true;
  }

  #schedule(): void {
    if (this.#timer) clearTimeout(this.#timer);
    this.shown = false;
    if (this.#delay <= 0) {
      this.shown = true;
      return;
    }
    this.#timer = setTimeout(() => {
      this.#timer = null;
      if (this.open) this.shown = true;
    }, this.#delay);
  }
}
