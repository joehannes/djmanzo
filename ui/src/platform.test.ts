import { describe, expect, it } from "vitest";
import { chordRun, modName, onMac } from "./platform";

const key = (code: string, over: { ctrl?: boolean; meta?: boolean; alt?: boolean; shift?: boolean } = {}) =>
  ({
    code,
    ctrlKey: over.ctrl ?? false,
    metaKey: over.meta ?? false,
    altKey: over.alt ?? false,
    shiftKey: over.shift ?? false,
  }) as unknown as KeyboardEvent;

const ACTIVITIES = ["dig", "mix", "perform"];

describe("§117: the platform's own chords", () => {
  /** **The same key in each system's hand**: ⌘ on a Mac, Ctrl elsewhere, and never the other. */
  it("answers ⌘ on a Mac and Ctrl elsewhere, and not the other way round", () => {
    expect(chordRun(key("KeyF", { ctrl: true }), ACTIVITIES, false)).toBe("ui search");
    expect(chordRun(key("KeyF", { meta: true }), ACTIVITIES, true)).toBe("ui search");
    expect(chordRun(key("KeyF", { meta: true }), ACTIVITIES, false)).toBeNull();
    expect(chordRun(key("KeyF", { ctrl: true }), ACTIVITIES, true)).toBeNull();
    expect(chordRun(key("Comma", { ctrl: true }), ACTIVITIES, false)).toBe("surface settings");
    expect(chordRun(key("Slash", { ctrl: true, shift: true }), ACTIVITIES, false)).toBe("surface keys");
    expect(chordRun(key("F1"), ACTIVITIES, false)).toBe("surface keys");
  });

  it("switches activities on the modifier and a digit, as a browser switches tabs", () => {
    expect(chordRun(key("Digit2", { ctrl: true }), ACTIVITIES, false)).toBe("switch activity mix");
    expect(chordRun(key("Digit9", { ctrl: true }), ACTIVITIES, false)).toBeNull();
    expect(chordRun(key("Digit1", { ctrl: true, shift: true }), ACTIVITIES, false)).toBeNull();
  });

  /** The palette answers its own chord, and Alt is the keyboard map's layer. */
  it("leaves K to the palette and Alt to the keyboard map", () => {
    expect(chordRun(key("KeyK", { ctrl: true }), ACTIVITIES, false)).toBeNull();
    expect(chordRun(key("KeyF", { ctrl: true, alt: true }), ACTIVITIES, false)).toBeNull();
    expect(chordRun(key("KeyF"), ACTIVITIES, false)).toBeNull();
  });

  it("knows a Mac by its platform", () => {
    expect(onMac("MacIntel")).toBe(true);
    expect(onMac("Linux x86_64")).toBe(false);
    expect(modName(true)).toBe("⌘");
    expect(modName(false)).toBe("Ctrl");
  });
});
