import { describe, expect, it } from "vitest";
import type { LeaderNode } from "./api";
import { Leader, keyOf, mnemonicAt } from "./leader.svelte";

const leaf = (key: string, label: string, run: string): LeaderNode => ({
  key,
  label,
  run,
  children: [],
  mine: false,
});
const group = (key: string, label: string, children: LeaderNode[]): LeaderNode => ({
  key,
  label,
  run: null,
  children,
  mine: false,
});

/** A small tree with the shape of djmanzo's: Space › d Deck › 1 Deck 1 › p Play. */
const TREE = group("space", "Space", [
  leaf("space", "Find anything", "ui palette"),
  leaf("1", "Mix", "switch activity mix"),
  group("d", "Deck", [
    group("1", "Deck 1", [leaf("p", "Play / pause", "action deck 1 play_pause"), leaf("c", "Cue", "action deck 1 cue")]),
  ]),
]);

/** The parts of a `KeyboardEvent` the leader reads. */
function key(
  name: string,
  over: { shift?: boolean; ctrl?: boolean; alt?: boolean; meta?: boolean; repeat?: boolean; target?: unknown } = {},
): KeyboardEvent {
  return {
    key: name,
    code: name === " " ? "Space" : "",
    shiftKey: over.shift ?? false,
    ctrlKey: over.ctrl ?? false,
    altKey: over.alt ?? false,
    metaKey: over.meta ?? false,
    repeat: over.repeat ?? false,
    target: over.target ?? null,
  } as unknown as KeyboardEvent;
}

function leader() {
  const ran: string[] = [];
  const l = new Leader((run) => ran.push(run), 0);
  l.tree = TREE;
  return { l, ran };
}

describe("§117: the leader key", () => {
  /**
   * **Space, then a word at a time, runs the leaf** — `Space d 1 p` is deck 1
   * play — and the guide closes behind it. Every key on the way is taken, so
   * none of them reaches the decks.
   */
  it("walks the tree one key at a time and runs the leaf", () => {
    const { l, ran } = leader();
    expect(l.press(key(" "))).toBe(true);
    expect(l.open).toBe(true);
    expect(l.press(key("d"))).toBe(true);
    expect(l.press(key("1"))).toBe(true);
    expect(l.path.map((node) => node.label)).toEqual(["Space", "Deck", "Deck 1"]);
    expect(l.press(key("p"))).toBe(true);
    expect(ran).toEqual(["action deck 1 play_pause"]);
    expect(l.open).toBe(false);
  });

  /**
   * **A key that leads nowhere is said, not let through.** One letter off
   * must not become a deck action; the guide stays where it was and names the
   * key.
   */
  it("keeps a key that leads nowhere and says so", () => {
    const { l, ran } = leader();
    l.press(key(" "));
    l.press(key("d"));
    expect(l.press(key("j"))).toBe(true);
    expect(l.missed).toBe("j");
    expect(l.here?.label).toBe("Deck");
    expect(ran).toEqual([]);
  });

  it("goes back a step on Backspace and closes on Escape", () => {
    const { l } = leader();
    l.press(key(" "));
    l.press(key("d"));
    l.press(key("1"));
    l.press(key("Backspace"));
    expect(l.here?.label).toBe("Deck");
    l.press(key("Escape"));
    expect(l.open).toBe(false);
    l.press(key(" "));
    l.press(key("Backspace"));
    expect(l.open).toBe(false);
  });

  /**
   * Space twice is the palette, and a digit under Space is an activity — the
   * same digits that switch activities on their own.
   */
  it("reaches the palette on Space Space and an activity on a digit", () => {
    const { l, ran } = leader();
    l.press(key(" "));
    l.press(key(" "));
    l.press(key(" "));
    l.press(key("1"));
    expect(ran).toEqual(["ui palette", "switch activity mix"]);
  });

  /**
   * **Not while typing, not before the tree, not on a held key; and a chord
   * with Ctrl, Alt or ⌘ is somebody else's** — it closes the guide and goes
   * through, so Ctrl+K still opens the palette.
   */
  it("leaves typing, auto-repeat and modifier chords alone", () => {
    const { l } = leader();
    const field = { tagName: "INPUT" };
    expect(l.press(key(" ", { target: field }))).toBe(false);
    expect(l.press(key(" ", { repeat: true }))).toBe(false);
    l.press(key(" "));
    expect(l.press(key("k", { ctrl: true }))).toBe(false);
    expect(l.open).toBe(false);

    const empty = new Leader(() => {}, 0);
    expect(empty.press(key(" "))).toBe(false);
  });

  it("reads keys as the tree writes them", () => {
    expect(keyOf(key(" "))).toBe("space");
    expect(keyOf(key("D"))).toBe("d");
    expect(keyOf(key("?", { shift: true }))).toBe("?");
    expect(keyOf(key("Enter"))).toBeNull();
  });

  /** The guide underlines the key's letter where it starts a word. */
  it("finds the mnemonic letter in the label", () => {
    expect(mnemonicAt("Play / pause", "p")).toBe(0);
    expect(mnemonicAt("Set plan", "p")).toBe(4);
    expect(mnemonicAt("Session log", "e")).toBe(1);
    expect(mnemonicAt("Mix", "1")).toBe(-1);
  });
});
