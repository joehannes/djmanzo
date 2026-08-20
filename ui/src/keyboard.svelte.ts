/**
 * The keyboard, listening.
 *
 * # Why the lookup is here and not in Rust
 *
 * A key press has to feel instant. Sending every keystroke across the bridge
 * to be translated would spend a round trip on keys that turn out not to be
 * bound at all — which, on a keyboard, is most of them. So the mapping is
 * fetched once and the lookup happens here; the *actions* still go through
 * `dispatch`, which is the only way to reach the engine.
 *
 * The mapping is still authored, validated and owned in Rust. This file
 * receives a table and reads it.
 */
import { dispatch, keyboardKeys, type KeyBinding } from "./api";

/** Where a key event is text, not a command. */
const TYPING = new Set(["INPUT", "TEXTAREA", "SELECT"]);

/**
 * Whether this event is somebody typing rather than playing.
 *
 * A DJ searching the library for "space jam" must not start deck 1 on the
 * space bar. Checked on the event's own target rather than
 * `document.activeElement`, which lags behind a focus change that happened in
 * the same tick.
 */
export function typing(target: EventTarget | null): boolean {
  const element = target as HTMLElement | null;
  if (!element || !element.tagName) return false;
  if (TYPING.has(element.tagName)) return true;
  return element.isContentEditable === true;
}

/** The canonical chord for an event, matching what Rust produced. */
export function chordOf(event: KeyboardEvent): string {
  let chord = "";
  if (event.ctrlKey) chord += "ctrl+";
  if (event.altKey) chord += "alt+";
  if (event.shiftKey) chord += "shift+";
  if (event.metaKey) chord += "meta+";
  return chord + event.code.toLowerCase();
}

/** What the interface needs to send an action. Injectable, so this is testable. */
export interface Wiring {
  send: (action: string) => void;
  keys: () => Promise<KeyBinding[]>;
}

const live: Wiring = {
  send: (action) => {
    void dispatch(action).catch((error) => {
      // A deck action before a device is open is the common case here and is
      // not worth a dialog — the engine says so and the interface already
      // shows there is no device.
      console.warn("keyboard:", error);
    });
  },
  keys: keyboardKeys,
};

/**
 * The keyboard, as a thing that can be switched on and listened to.
 *
 * Deliberately a class rather than module state: the shortcut sheet reads the
 * same binding list the handler uses, and two copies of that would drift the
 * first time a user mapping loaded.
 */
export class Keyboard {
  /** The binding table, by chord. */
  #by = new Map<string, KeyBinding>();
  /** The full list, in file order, for the sheet. */
  bindings = $state<KeyBinding[]>([]);
  /** Whether key events are acted on. */
  enabled = $state(true);
  /**
   * Chords currently held down. Reactive, because the shortcut sheet lights
   * a key while it is held — the only way to see that a censor is on when the
   * deck looks like it is playing normally.
   *
   * Kept so a release only fires for a press this actually saw. Without it,
   * a key pressed while a text field had focus and released after it lost
   * focus would send a `censor_off` for a censor that never went on — or
   * worse, the reverse: a held key whose release was swallowed leaves the deck
   * censored with nothing on screen to say so.
   */
  held = $state<string[]>([]);
  #wiring: Wiring;

  constructor(wiring: Wiring = live) {
    this.#wiring = wiring;
  }

  /** Fetch the mapping. Safe to call again when a user mapping loads. */
  async load(): Promise<void> {
    const keys = await this.#wiring.keys();
    this.bindings = keys;
    this.#by = new Map(keys.map((key) => [key.chord, key]));
  }

  /** What is bound to a chord, if anything. */
  binding(chord: string): KeyBinding | undefined {
    return this.#by.get(chord);
  }

  /**
   * Handle a key going down. Returns true when it was ours, so the caller
   * knows whether to swallow the event.
   *
   * Auto-repeat is not a press. A finger resting on the cue key should not
   * fire ninety cues — and the browser repeats at whatever rate the operating
   * system is set to, which nobody chose with a DJ in mind.
   */
  down(event: KeyboardEvent): boolean {
    if (!this.enabled || typing(event.target)) return false;
    const chord = chordOf(event);
    const found = this.#by.get(chord);
    if (!found) return false;
    // Claimed either way: swallowing the browser's own space-scrolls-the-page
    // behaviour matters even when the repeat is ignored.
    if (event.repeat || this.held.includes(chord)) return true;
    this.held = [...this.held, chord];
    if (found.press) this.#wiring.send(found.press);
    return true;
  }

  /** Handle a key coming up. */
  up(event: KeyboardEvent): boolean {
    const chord = chordOf(event);
    const found = this.#by.get(chord);
    if (!found || !this.held.includes(chord)) return false;
    this.held = this.held.filter((each) => each !== chord);
    if (found.release) this.#wiring.send(found.release);
    return true;
  }

  /**
   * Let go of everything held.
   *
   * Called when the window loses focus, which is the case that actually bites:
   * hold the bass kill, hit Cmd-Tab, and the key-up is delivered to whatever
   * you switched to. Without this the deck stays killed until you come back
   * and press the key again — during a set, in front of a room.
   */
  releaseAll(): void {
    const letting = this.held;
    this.held = [];
    for (const chord of letting) {
      const found = this.#by.get(chord);
      if (found?.release) this.#wiring.send(found.release);
    }
  }

  /** Whether a chord is being held. For the sheet to light up. */
  isDown(chord: string): boolean {
    return this.held.includes(chord);
  }

  /** How many keys are held. Used by tests and by nothing else. */
  get heldCount(): number {
    return this.held.length;
  }

  /**
   * Attach to a window. Returns the detach function.
   *
   * Listens on `window` in the capture phase so a control that has focus does
   * not swallow the space bar first — a play button that has just been clicked
   * would otherwise turn the space bar into "press me again".
   */
  attach(target: Window): () => void {
    const onDown = (event: KeyboardEvent) => {
      if (this.down(event)) event.preventDefault();
    };
    const onUp = (event: KeyboardEvent) => {
      if (this.up(event)) event.preventDefault();
    };
    const onBlur = () => this.releaseAll();
    target.addEventListener("keydown", onDown, true);
    target.addEventListener("keyup", onUp, true);
    target.addEventListener("blur", onBlur);
    return () => {
      target.removeEventListener("keydown", onDown, true);
      target.removeEventListener("keyup", onUp, true);
      target.removeEventListener("blur", onBlur);
    };
  }
}

/** The bindings grouped for the sheet, in the order the groups first appear. */
export function grouped(bindings: KeyBinding[]): [string, KeyBinding[]][] {
  const groups = new Map<string, KeyBinding[]>();
  for (const binding of bindings) {
    const group = binding.group || "Other";
    const existing = groups.get(group);
    if (existing) existing.push(binding);
    else groups.set(group, [binding]);
  }
  return [...groups.entries()];
}

/** A chord as a DJ reads it: `⇧ Space`, `Q`, `⌥ 1`. */
export function pretty(chord: string): string {
  const parts = chord.split("+");
  const key = parts.pop() ?? "";
  const modifiers = parts
    .map((m) => ({ shift: "⇧", alt: "⌥", ctrl: "⌃", meta: "⌘" })[m] ?? m)
    .join("");
  return modifiers + label(key);
}

function label(code: string): string {
  if (code.startsWith("key")) return code.slice(3).toUpperCase();
  if (code.startsWith("digit")) return code.slice(5);
  const names: Record<string, string> = {
    space: "Space",
    semicolon: ";",
    comma: ",",
    period: ".",
    slash: "/",
    backquote: "`",
    minus: "−",
    equal: "=",
    bracketleft: "[",
    bracketright: "]",
    quote: "'",
    backslash: "\\",
    arrowleft: "←",
    arrowright: "→",
    arrowup: "↑",
    arrowdown: "↓",
    escape: "Esc",
    enter: "↵",
    tab: "Tab",
    backspace: "⌫",
  };
  return names[code] ?? code.toUpperCase();
}
