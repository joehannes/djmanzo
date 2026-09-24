/**
 * §116: the melody line, as paths to draw over the visible stretch of a lane.
 *
 * > also use the bg of the display of a song control and waveform somehow to
 * > visualize rhythm and melody
 *
 * The notes are Rust's — `dj_analysis::melody::pitches`, ten a second — and
 * so is each note's colour, a step of §110's spectrum sent with them so the
 * formula lives in one place. This places them: height by pitch on a
 * semitone scale, so a fifth is the same height anywhere, and one path per
 * run of one colour, broken where nothing was pitched and where the line
 * leaps further than a melody does in one tenth of a second.
 */
import type { MelodyLine } from "./api";

/** A leap wider than this is a new line, not a note of the old one. */
export const LEAP_SEMITONES = 10;

/** Room above and below the line, as a share of the lane's height. */
const MARGIN = 0.12;

export interface MelodyPath {
  d: string;
  colour: string;
}

/** Semitones between two pitches. */
const semitones = (from: number, to: number) => 12 * Math.log2(to / from);

/**
 * The paths for the points between `from` and `to` (frames of the file), in
 * pixels from `from` at `framesPerPixel`, in a lane `height` pixels tall.
 */
export function melodyPaths(
  line: MelodyLine,
  from: number,
  to: number,
  framesPerPixel: number,
  height: number,
): MelodyPath[] {
  const [low, high] = line.range;
  const span = semitones(low, high);
  if (!(span > 0) || !(framesPerPixel > 0) || !(line.frames_per_point > 0)) return [];
  const y = (hz: number) => {
    const up = Math.min(1, Math.max(0, semitones(low, hz) / span));
    return height * (MARGIN + (1 - 2 * MARGIN) * (1 - up));
  };
  const first = Math.max(0, Math.floor(from / line.frames_per_point));
  const last = Math.min(line.points.length - 1, Math.ceil(to / line.frames_per_point));

  const paths: MelodyPath[] = [];
  let d = "";
  let step = -1;
  let previous: { x: number; y: number; hz: number } | null = null;
  const close = () => {
    if (d.includes("L")) {
      const [r, g, b] = line.colours[step] ?? [128, 128, 128];
      paths.push({ d, colour: `rgb(${r}, ${g}, ${b})` });
    }
    d = "";
  };
  for (let index = first; index <= last; index += 1) {
    const point = line.points[index];
    if (!point) {
      close();
      previous = null;
      continue;
    }
    const [hz, colour] = point;
    const here = {
      x: Math.round(((index * line.frames_per_point - from) / framesPerPixel) * 10) / 10,
      y: Math.round(y(hz) * 10) / 10,
      hz,
    };
    if (previous && Math.abs(semitones(previous.hz, hz)) > LEAP_SEMITONES) {
      close();
      previous = null;
    }
    if (!previous) {
      step = colour;
      d = `M${here.x} ${here.y}`;
    } else if (colour !== step) {
      // A new colour carries on from the last point, so the line is unbroken.
      close();
      step = colour;
      d = `M${previous.x} ${previous.y} L${here.x} ${here.y}`;
    } else {
      d += ` L${here.x} ${here.y}`;
    }
    previous = here;
  }
  close();
  return paths;
}
