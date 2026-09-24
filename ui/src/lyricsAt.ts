/**
 * §107: where a singer is in the words, at a moment of the record.
 *
 * The times are the lyric file's — `dj_library::lrc`, sent by
 * `singer_lyrics` — and the moment is the deck's playhead in the file, so a
 * record played faster or slower is still read in its own time. What this
 * works out is the three things the singers' screen draws: which line is
 * being sung, how far through it (word by word where the file times words,
 * evenly where it does not), and a count-in when a line is coming after
 * silence.
 */
import type { LyricLine } from "./api";

/** How many beats a count-in counts. A bar, which is what a singer breathes to. */
export const COUNT_IN_BEATS = 4;

/**
 * The longest a line is wiped over, in seconds. A line followed by a long
 * instrumental would otherwise crawl across the whole break; nobody takes
 * eight seconds to sing one line.
 */
export const LONGEST_LINE = 8;

/** A silence this long before a line earns it a count-in. */
export const GAP_SECONDS = 6;

export interface Moment {
  /** The line being sung, or -1 before the first. */
  index: number;
  /** How far through that line, 0..1. */
  progress: number;
  /**
   * For a line whose words are timed: how many words are sung, with the
   * fraction of the one being sung — 2.5 is two words and half the third.
   */
  words: number;
  /** Beats until the next line, when one is coming after silence. */
  countIn: number | null;
}

/** When a line stops being sung: the next line, or its own length at most. */
function endOf(lines: LyricLine[], index: number): number {
  const line = lines[index];
  const next = lines[index + 1]?.at ?? Number.POSITIVE_INFINITY;
  return Math.min(next, line.at + LONGEST_LINE);
}

export function lyricsAt(lines: LyricLine[], seconds: number, bpm: number | null): Moment {
  let index = -1;
  for (let i = 0; i < lines.length && lines[i].at <= seconds; i += 1) index = i;

  let progress = 0;
  let words = 0;
  if (index >= 0) {
    const line = lines[index];
    const end = endOf(lines, index);
    progress = Math.min(1, Math.max(0, (seconds - line.at) / Math.max(0.001, end - line.at)));
    const timed = line.words;
    for (let w = 0; w < timed.length; w += 1) {
      const [at] = timed[w];
      if (seconds < at) break;
      const until = timed[w + 1]?.[0] ?? Math.min(end, at + 1.5);
      words = w + Math.min(1, (seconds - at) / Math.max(0.001, until - at));
    }
  }

  // A count-in: the next line is under a bar away, and it comes after
  // silence — the start of the song, a marked gap, or a long break.
  let countIn: number | null = null;
  const next = lines[index + 1];
  const beat = bpm && bpm > 0 ? 60 / bpm : null;
  if (next && beat) {
    const away = next.at - seconds;
    const afterSilence =
      index < 0 || lines[index].text === "" || next.at - endOf(lines, index) >= GAP_SECONDS;
    if (afterSilence && away > 0 && away <= COUNT_IN_BEATS * beat) {
      countIn = Math.ceil(away / beat - 1e-6);
    }
  }
  return { index, progress, words, countIn };
}
