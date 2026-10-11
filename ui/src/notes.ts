/**
 * Pitches as people read them: cents above A4 — the scale K6's readings and
 * vocal lines are on — as note names, and K8's ranges in words.
 */

const NAMES = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

/**
 * K6: a pitch as a note — the nearest note's name and octave, with how far
 * off it is when that is more than ten cents.
 */
export function noteName(cents: number): string {
  const midi = 69 + cents / 100;
  const nearest = Math.round(midi);
  const off = Math.round((midi - nearest) * 100);
  const name = `${NAMES[((nearest % 12) + 12) % 12]}${Math.floor(nearest / 12) - 1}`;
  return Math.abs(off) > 10 ? `${name} ${off > 0 ? "+" : "−"}${Math.abs(off)}` : name;
}

/** The nearest note's name and octave, without cents: a range is notes. */
export function nearestNote(cents: number): string {
  const nearest = Math.round(69 + cents / 100);
  return `${NAMES[((nearest % 12) + 12) % 12]}${Math.floor(nearest / 12) - 1}`;
}

const INTERVALS = [
  "",
  "a semitone",
  "a tone",
  "a minor third",
  "a major third",
  "a fourth",
  "a tritone",
  "a fifth",
  "a minor sixth",
  "a major sixth",
  "a minor seventh",
  "a major seventh",
];
const OCTAVES = ["", "an octave", "two octaves", "three octaves", "four octaves"];

/** K8: a range in words — "A2 – E4, an octave and a fifth". */
export function rangeWords(range: { low: number; high: number }): string {
  const semitones = Math.max(0, Math.round(69 + range.high / 100) - Math.round(69 + range.low / 100));
  const octaves = Math.min(4, Math.floor(semitones / 12));
  const rest = INTERVALS[semitones % 12];
  const span =
    octaves === 0 ? rest || "one note" : rest ? `${OCTAVES[octaves]} and ${rest}` : OCTAVES[octaves];
  return `${nearestNote(range.low)} – ${nearestNote(range.high)}, ${span}`;
}
