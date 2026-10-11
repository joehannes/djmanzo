import { describe, expect, it } from "vitest";

import { nearestNote, noteName, rangeWords } from "./notes";

describe("notes", () => {
  it("names a pitch, with cents only when they matter", () => {
    expect(noteName(0)).toBe("A4");
    expect(noteName(-1200)).toBe("A3");
    expect(noteName(-500)).toBe("E4");
    expect(noteName(-480)).toBe("E4 +20");
    expect(noteName(-905)).toBe("C4");
    expect(nearestNote(-480)).toBe("E4");
  });

  it("says a range as notes and the span between them", () => {
    expect(rangeWords({ low: -2400, high: -500 })).toBe("A2 – E4, an octave and a fifth");
    expect(rangeWords({ low: -1200, high: 0 })).toBe("A3 – A4, an octave");
    expect(rangeWords({ low: -500, high: 0 })).toBe("E4 – A4, a fourth");
    expect(rangeWords({ low: 0, high: 20 })).toBe("A4 – A4, one note");
    expect(rangeWords({ low: -3600, high: 0 })).toBe("A1 – A4, three octaves");
  });
});
