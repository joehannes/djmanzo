/**
 * Karaoke beta B3: what the singers' screen draws between the words — how
 * long an instrumental break lasts, and whose line is whose in a duet.
 * Pure, so it is tested without a screen (`lyricBreak.test.ts`).
 */
import type { LyricLine } from "./api";
import { GAP_SECONDS, LONGEST_LINE } from "./lyricsAt";

/** A stretch with no words, long enough to be told how long it lasts. */
export interface Break {
  /** When it began and when the next line comes, in seconds. */
  from: number;
  to: number;
  /** How far through it, 0..1. */
  progress: number;
  /** Whole seconds until the next line. */
  left: number;
}

/**
 * The break at `seconds`, if one: the intro before the first line, a line
 * with no words, or the silence after a line has been sung — each only when
 * it lasts at least `GAP_SECONDS`, since a breath between lines is not a
 * break and a bar for it would only hurry the singer.
 */
export function breakAt(lines: LyricLine[], seconds: number): Break | null {
  if (lines.length === 0) return null;
  let index = -1;
  for (let i = 0; i < lines.length && lines[i].at <= seconds; i += 1) index = i;
  const next = lines[index + 1];
  if (!next) return null;
  let from: number;
  if (index < 0) from = 0;
  else if (lines[index].text.trim() === "") from = lines[index].at;
  else from = Math.min(next.at, lines[index].at + LONGEST_LINE);
  const to = next.at;
  if (to - from < GAP_SECONDS || seconds < from) return null;
  return {
    from,
    to,
    progress: Math.min(1, Math.max(0, (seconds - from) / (to - from))),
    left: Math.max(0, Math.ceil(to - seconds - 1e-6)),
  };
}

/** One part of a duet: a line that begins with a singer's name and a colon. */
export interface Part {
  /** Which of `names` sings it; `null` for a line without a name. */
  singer: number | null;
  /** The line without the name. */
  text: string;
}

const fold = (s: string) => s.trim().toLocaleLowerCase();

/**
 * Whose line `text` is: "Ana: so long" is Ana's when Ana is one of `names`
 * (the singers on the microphones, by strip). A name that is not one of
 * them is left as part of the line — a lyric can begin "Oh:".
 */
export function partOf(text: string, names: (string | null)[]): Part {
  const colon = text.indexOf(":");
  if (colon > 0 && colon <= 24) {
    const who = fold(text.slice(0, colon));
    const singer = names.findIndex((name) => name !== null && fold(name) === who);
    if (singer >= 0) return { singer, text: text.slice(colon + 1).trimStart() };
  }
  return { singer: null, text };
}
