import { describe, expect, it } from "vitest";
import { Keyboard, chordOf, grouped, pretty, typing } from "./keyboard.svelte";
import type { KeyBinding } from "./api";

function binding(over: Partial<KeyBinding>): KeyBinding {
  return {
    chord: "space",
    label: "Play",
    group: "Transport",
    held: false,
    press: "deck 1 play_pause",
    release: null,
    ...over,
  };
}

/** A keyboard wired to a list instead of the engine. */
function wired(bindings: KeyBinding[]) {
  const sent: string[] = [];
  const keyboard = new Keyboard({
    send: (action) => sent.push(action),
    keys: async () => bindings,
  });
  return { keyboard, sent };
}

/** The parts of a `KeyboardEvent` this module actually reads. */
function event(
  code: string,
  over: {
    shift?: boolean;
    alt?: boolean;
    ctrl?: boolean;
    meta?: boolean;
    repeat?: boolean;
    target?: unknown;
  } = {},
): KeyboardEvent {
  return {
    code,
    shiftKey: over.shift ?? false,
    altKey: over.alt ?? false,
    ctrlKey: over.ctrl ?? false,
    metaKey: over.meta ?? false,
    repeat: over.repeat ?? false,
    target: over.target ?? { tagName: "BODY" },
  } as unknown as KeyboardEvent;
}

describe("chords", () => {
  it("spells a chord the way Rust does", () => {
    expect(chordOf(event("Space"))).toBe("space");
    expect(chordOf(event("KeyQ", { shift: true }))).toBe("shift+keyq");
    expect(chordOf(event("KeyA", { ctrl: true, alt: true, shift: true, meta: true }))).toBe(
      "ctrl+alt+shift+meta+keya",
    );
  });

  it("reads a chord back the way a DJ does", () => {
    expect(pretty("space")).toBe("Space");
    expect(pretty("shift+space")).toBe("⇧Space");
    expect(pretty("keyq")).toBe("Q");
    expect(pretty("digit1")).toBe("1");
    expect(pretty("shift+digit1")).toBe("⇧1");
    expect(pretty("arrowleft")).toBe("←");
    expect(pretty("semicolon")).toBe(";");
  });
});

describe("typing", () => {
  it("leaves a text field alone", () => {
    const as = (shape: object) => shape as EventTarget;
    expect(typing(as({ tagName: "INPUT" }))).toBe(true);
    expect(typing(as({ tagName: "TEXTAREA" }))).toBe(true);
    expect(typing(as({ tagName: "DIV", isContentEditable: true }))).toBe(true);
    expect(typing(as({ tagName: "DIV" }))).toBe(false);
    expect(typing(null)).toBe(false);
  });

  /**
   * The case this exists for. A DJ searching the library for "space jam"
   * should get the letters, not deck 1.
   */
  it("does not play a deck while somebody is searching", async () => {
    const { keyboard, sent } = wired([binding({})]);
    await keyboard.load();
    expect(keyboard.down(event("Space", { target: { tagName: "INPUT" } }))).toBe(false);
    expect(sent).toEqual([]);
  });
});

describe("a key press", () => {
  it("sends the action bound to it", async () => {
    const { keyboard, sent } = wired([binding({})]);
    await keyboard.load();
    expect(keyboard.down(event("Space"))).toBe(true);
    expect(sent).toEqual(["deck 1 play_pause"]);
  });

  it("says nothing for a key that is not bound", async () => {
    const { keyboard, sent } = wired([binding({})]);
    await keyboard.load();
    expect(keyboard.down(event("KeyZ"))).toBe(false);
    expect(sent).toEqual([]);
  });

  /** Shift is part of the chord, not a modifier the lookup should ignore. */
  it("does not answer a shifted key with the unshifted binding", async () => {
    const { keyboard, sent } = wired([binding({})]);
    await keyboard.load();
    expect(keyboard.down(event("Space", { shift: true }))).toBe(false);
    expect(sent).toEqual([]);
  });

  /**
   * The operating system repeats a held key at whatever rate it is set to,
   * which nobody chose with a DJ in mind. Ninety cue jumps a second is not a
   * feature.
   */
  it("ignores auto-repeat", async () => {
    const { keyboard, sent } = wired([binding({})]);
    await keyboard.load();
    keyboard.down(event("Space"));
    keyboard.down(event("Space", { repeat: true }));
    keyboard.down(event("Space", { repeat: true }));
    expect(sent).toEqual(["deck 1 play_pause"]);
  });

  /**
   * ...and still claims the event, because the browser's own "space scrolls
   * the page" has to be swallowed on every repeat, not just the first.
   */
  it("still claims a repeated key so the page does not scroll", async () => {
    const { keyboard } = wired([binding({})]);
    await keyboard.load();
    keyboard.down(event("Space"));
    expect(keyboard.down(event("Space", { repeat: true }))).toBe(true);
  });

  it("does not fire twice when a repeat arrives without the flag set", async () => {
    const { keyboard, sent } = wired([binding({})]);
    await keyboard.load();
    keyboard.down(event("Space"));
    keyboard.down(event("Space"));
    expect(sent).toEqual(["deck 1 play_pause"]);
  });
});

describe("a held key", () => {
  const censor = binding({
    chord: "keyf",
    label: "Censor (hold)",
    held: true,
    press: "deck 1 censor_on",
    release: "deck 1 censor_off",
  });

  it("undoes itself on release", async () => {
    const { keyboard, sent } = wired([censor]);
    await keyboard.load();
    keyboard.down(event("KeyF"));
    keyboard.up(event("KeyF"));
    expect(sent).toEqual(["deck 1 censor_on", "deck 1 censor_off"]);
  });

  /**
   * A release without a press this saw is not a release. It happens whenever a
   * key goes down while a text field has focus and comes up after it lost it —
   * and a `censor_off` for a censor that never went on is a deck turning back
   * on mid-phrase.
   */
  it("ignores a release it never saw the press for", async () => {
    const { keyboard, sent } = wired([censor]);
    await keyboard.load();
    expect(keyboard.up(event("KeyF"))).toBe(false);
    expect(sent).toEqual([]);
  });

  /**
   * The case that actually bites: hold the bass kill, hit Cmd-Tab, and the
   * key-up goes to whatever you switched to. Without this the deck stays
   * killed until you come back and press the key again.
   */
  it("lets go of everything when the window loses focus", async () => {
    const kill = binding({
      chord: "keyz",
      label: "Bass kill (hold)",
      held: true,
      press: "deck 1 eq_low 0",
      release: "deck 1 eq_low 1",
    });
    const { keyboard, sent } = wired([censor, kill]);
    await keyboard.load();
    keyboard.down(event("KeyF"));
    keyboard.down(event("KeyZ"));
    expect(keyboard.heldCount).toBe(2);

    keyboard.releaseAll();
    expect(sent).toEqual([
      "deck 1 censor_on",
      "deck 1 eq_low 0",
      "deck 1 censor_off",
      "deck 1 eq_low 1",
    ]);
    expect(keyboard.heldCount).toBe(0);
  });

  /** And letting go twice must not send the release twice. */
  it("only lets go once", async () => {
    const { keyboard, sent } = wired([censor]);
    await keyboard.load();
    keyboard.down(event("KeyF"));
    keyboard.releaseAll();
    keyboard.releaseAll();
    expect(sent).toEqual(["deck 1 censor_on", "deck 1 censor_off"]);
  });

  it("shows which keys are down, for the sheet to light up", async () => {
    const { keyboard } = wired([censor]);
    await keyboard.load();
    expect(keyboard.isDown("keyf")).toBe(false);
    keyboard.down(event("KeyF"));
    expect(keyboard.isDown("keyf")).toBe(true);
  });
});

describe("switched off", () => {
  it("sends nothing", async () => {
    const { keyboard, sent } = wired([binding({})]);
    await keyboard.load();
    keyboard.enabled = false;
    expect(keyboard.down(event("Space"))).toBe(false);
    expect(sent).toEqual([]);
  });

  /**
   * Switching off while a key is held must still deliver its release, or the
   * off switch itself becomes a way to leave a deck censored. The handler does
   * not gate `up` on `enabled` for exactly this reason.
   */
  it("still releases a key that was already down", async () => {
    const { keyboard, sent } = wired([
      binding({
        chord: "keyf",
        held: true,
        press: "deck 1 censor_on",
        release: "deck 1 censor_off",
      }),
    ]);
    await keyboard.load();
    keyboard.down(event("KeyF"));
    keyboard.enabled = false;
    expect(keyboard.up(event("KeyF"))).toBe(true);
    expect(sent).toEqual(["deck 1 censor_on", "deck 1 censor_off"]);
  });
});

describe("the sheet", () => {
  it("groups in the order the groups first appear", () => {
    const groups = grouped([
      binding({ group: "Deck 1" }),
      binding({ group: "Mixer" }),
      binding({ group: "Deck 1" }),
    ]);
    expect(groups.map(([name]) => name)).toEqual(["Deck 1", "Mixer"]);
    expect(groups[0][1]).toHaveLength(2);
  });

  it("puts a binding with no group somewhere rather than dropping it", () => {
    const groups = grouped([binding({ group: "" })]);
    expect(groups).toEqual([["Other", [binding({ group: "" })]]]);
  });
});
