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
  /** Which of `names` sings it; `null` for a line without a name, and for one sung together. */
  singer: number | null;
  /** The line without the name. */
  text: string;
  /** The line began with a name or a part's mark, taken off `text`. */
  marked: boolean;
  /** Marked as sung together ("D:", "Both:"). */
  together: boolean;
}

const fold = (s: string) => s.trim().toLocaleLowerCase();

/**
 * The marks a duet's lyrics use where they carry no singer's name: a man's
 * part, a woman's, and the lines sung together — "M:", "F:", "D:" as the
 * karaoke `.lrc` convention writes them, and the words spelled out.
 */
const MARKS: Record<string, "m" | "f" | "d"> = {
  m: "m",
  male: "m",
  man: "m",
  f: "f",
  female: "f",
  woman: "f",
  d: "d",
  duet: "d",
  both: "d",
  all: "d",
};

/** A line's mark and the rest of it, or none. */
function markOf(text: string): { mark: "m" | "f" | "d"; rest: string } | null {
  const colon = text.indexOf(":");
  if (colon <= 0 || colon > 8) return null;
  const mark = MARKS[fold(text.slice(0, colon))];
  return mark ? { mark, rest: text.slice(colon + 1).trimStart() } : null;
}

/**
 * A duet's parts by mark, in the order the song first gives them: the
 * first singer on a microphone takes the first part sung, the second the
 * other. Who is the man and who the woman is not djmanzo's to guess; who
 * comes in first is the song's.
 */
export function partsOf(lines: { text: string }[]): ("m" | "f")[] {
  const order: ("m" | "f")[] = [];
  for (const line of lines) {
    const found = markOf(line.text);
    if (found && found.mark !== "d" && !order.includes(found.mark)) order.push(found.mark);
    if (order.length === 2) break;
  }
  return order;
}

/**
 * Whose line `text` is: "Ana: so long" is Ana's when Ana is one of `names`
 * (the singers on the microphones, by strip). A name that is not one of
 * them is left as part of the line — a lyric can begin "Oh:". Where the
 * song marks parts rather than names them ("M:", "F:"), `order` — from
 * [`partsOf`] — gives each part to a singer on a microphone, in turn; a
 * line sung together ("D:") is everyone's.
 */
export function partOf(text: string, names: (string | null)[], order: ("m" | "f")[] = []): Part {
  const colon = text.indexOf(":");
  if (colon > 0 && colon <= 24) {
    const who = fold(text.slice(0, colon));
    const singer = names.findIndex((name) => name !== null && fold(name) === who);
    if (singer >= 0) return { singer, text: text.slice(colon + 1).trimStart(), marked: true, together: false };
  }
  const found = markOf(text);
  if (found) {
    if (found.mark === "d") return { singer: null, text: found.rest, marked: true, together: true };
    const on = names.flatMap((name, strip) => (name !== null ? [strip] : []));
    const turn = order.indexOf(found.mark);
    return { singer: on[turn] ?? null, text: found.rest, marked: true, together: false };
  }
  return { singer: null, text, marked: false, together: false };
}
