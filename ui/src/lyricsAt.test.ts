import { describe, expect, it } from "vitest";

import type { LyricLine } from "./api";
import { COUNT_IN_BEATS, LONGEST_LINE, lyricsAt } from "./lyricsAt";

const line = (at: number, text: string, words: [number, string][] = []): LyricLine => ({ at, text, words });

// 120 BPM: half a second a beat, two seconds a bar.
const BPM = 120;

describe("§107: where the singer is in the words", () => {
  const song = [line(10, "First line"), line(13, "Second line"), line(30, "After a break")];

  it("is before the first line until it starts, then on each in turn", () => {
    expect(lyricsAt(song, 5, BPM).index).toBe(-1);
    expect(lyricsAt(song, 10, BPM).index).toBe(0);
    expect(lyricsAt(song, 14, BPM).index).toBe(1);
    expect(lyricsAt(song, 31, BPM).index).toBe(2);
  });

  it("wipes a line evenly to where the next begins", () => {
    expect(lyricsAt(song, 11.5, BPM).progress).toBeCloseTo(0.5);
  });

  it("wipes a line before a long break over a line's length, not the whole break", () => {
    expect(lyricsAt(song, 13 + LONGEST_LINE / 2, BPM).progress).toBeCloseTo(0.5);
    expect(lyricsAt(song, 25, BPM).progress).toBe(1);
  });

  it("counts in the first line, a bar out, in beats", () => {
    expect(lyricsAt(song, 10 - 3 * 0.5, BPM).countIn).toBe(3);
    expect(lyricsAt(song, 10 - 0.2, BPM).countIn).toBe(1);
    // More than a bar out: not yet.
    expect(lyricsAt(song, 10 - (COUNT_IN_BEATS + 1) * 0.5, BPM).countIn).toBeNull();
  });

  it("counts in after a break, and not between two lines sung straight through", () => {
    expect(lyricsAt(song, 30 - 1, BPM).countIn).toBe(2);
    expect(lyricsAt(song, 13 - 0.5, BPM).countIn).toBeNull();
  });

  it("counts in after a marked gap, however short", () => {
    const gapped = [line(10, "Sung"), line(12, ""), line(14, "Back in")];
    expect(lyricsAt(gapped, 13.2, BPM).countIn).toBe(2);
  });

  it("follows timed words, a word and its fraction at a time", () => {
    const timed = [line(10, "So long my friend", [[10, "So"], [10.5, "long"], [11, "my"], [12, "friend"]])];
    expect(lyricsAt(timed, 10.25, BPM).words).toBeCloseTo(0.5);
    expect(lyricsAt(timed, 11.5, BPM).words).toBeCloseTo(2.5);
    expect(lyricsAt(timed, 9, BPM).words).toBe(0);
  });

  it("needs a tempo to count, and says nothing rather than guessing without one", () => {
    expect(lyricsAt(song, 9, null).countIn).toBeNull();
  });
});
