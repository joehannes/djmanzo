import { describe, expect, it } from "vitest";

import { breakAt, partOf, partsOf } from "./lyricBreak";

const line = (at: number, text: string) => ({ at, text, words: [] as [number, string][] });
const SONG = [line(10, "First"), line(12, "Second"), line(40, ""), line(52, "Back in"), line(54, "Last")];

describe("B3: an instrumental break", () => {
  it("is the intro, a wordless line, or the silence after a line, if long enough", () => {
    expect(breakAt(SONG, 5)).toMatchObject({ from: 0, to: 10, left: 5, progress: 0.5 });
    // "Second" is sung for at most LONGEST_LINE (8 s): the break is 20 → 40.
    expect(breakAt(SONG, 13)).toBeNull();
    expect(breakAt(SONG, 30)).toMatchObject({ from: 20, to: 40, left: 10, progress: 0.5 });
    expect(breakAt(SONG, 46)).toMatchObject({ from: 40, to: 52, left: 6 });
  });

  it("is not a breath between lines, nor after the last", () => {
    expect(breakAt(SONG, 11)).toBeNull();
    // A line sung for its eight seconds, then two of silence: a breath.
    expect(breakAt([line(0, "Long"), line(10, "Next")], 9)).toBeNull();
    expect(breakAt([line(4, "Soon")], 1)).toBeNull();
    expect(breakAt(SONG, 60)).toBeNull();
    expect(breakAt([], 3)).toBeNull();
  });
});

describe("B3: a duet's parts", () => {
  it("are the singers on the microphones, named at the start of a line", () => {
    const on = ["Ana", null, "Ben"];
    expect(partOf("Ana: so long", on)).toEqual({ singer: 0, text: "so long", marked: true, together: false });
    expect(partOf(" ben :and you", on)).toEqual({ singer: 2, text: "and you", marked: true, together: false });
    expect(partOf("BEN: and you", on)).toEqual({ singer: 2, text: "and you", marked: true, together: false });
    expect(partOf("Oh: what a night", on)).toEqual({ singer: null, text: "Oh: what a night", marked: false, together: false });
    expect(partOf("No colon here", on)).toEqual({ singer: null, text: "No colon here", marked: false, together: false });
  });

  it("are marked rather than named: the first part sung is the first singer's", () => {
    const song = [{ text: "D: together now" }, { text: "F: she starts" }, { text: "M: he answers" }, { text: "F: again" }];
    const order = partsOf(song);
    expect(order).toEqual(["f", "m"]);
    // Mic 1 empty, Ana on Mic 2, Ben on Mic 3.
    const on = [null, "Ana", "Ben"];
    expect(partOf("F: she starts", on, order)).toEqual({ singer: 1, text: "she starts", marked: true, together: false });
    expect(partOf("m:he answers", on, order)).toEqual({ singer: 2, text: "he answers", marked: true, together: false });
    expect(partOf("Female: spelled out", on, order).singer).toBe(1);
    expect(partOf("D: together now", on, order)).toEqual({ singer: null, text: "together now", marked: true, together: true });
    // A name still wins, and a mark the song never set up has no singer.
    expect(partOf("Ben: mine", on, order).singer).toBe(2);
    expect(partOf("M: alone", on, ["m"])).toMatchObject({ singer: 1, marked: true });
    expect(partOf("Oh: not a mark", on, order)).toMatchObject({ singer: null, marked: false });
  });
});
