import { describe, expect, it } from "vitest";

import { hueOf, spread } from "./micColours";

describe("B3: a duet's colours", () => {
  it("reads a hue however the theme wrote it", () => {
    expect(hueOf("#ff0000")).toBe(0);
    expect(hueOf("#0f0")).toBe(120);
    expect(hueOf("rgb(0, 0, 255)")).toBe(240);
    expect(hueOf("var(--accent)")).toBeNull();
    // A grey has no hue to divide out: none, read as red's, never NaN.
    expect(hueOf("#777")).toBe(0);
  });

  it("keeps the voice's colour first and puts the next singer farthest from it", () => {
    // An amber theme: accent and warning a few degrees apart, the second
    // accent a blue — the second microphone takes the blue, not the warning.
    expect(spread(["#e6c98a", "#a8b8d8", "#e0a33a", "#e06b6b"])).toEqual([0, 1, 3, 2]);
    // A green theme whose two accents are both green: the red goes second,
    // the amber third, the second green last.
    expect(spread(["#66d6a6", "#28b460", "#f6bd4a", "#ef6b6b"])).toEqual([0, 3, 2, 1]);
    expect(spread([])).toEqual([]);
    expect(spread(["#66d6a6", "nonsense", "#f6bd4a"])).toEqual([0, 2, 1]);
  });
});
