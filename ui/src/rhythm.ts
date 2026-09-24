/**
 * §116: the rhythm strip's geometry.
 *
 * > use the bg of the display of a song control and waveform somehow to
 * > visualize rhythm and melody
 *
 * Rust reads the record's drum pattern onto its grid — `dj_analysis::rhythm`,
 * a step every sixteenth, kick, snare and hat each 0 to 255 — and this places
 * the steps under the waveform the way a drum machine's grid is read: a row a
 * voice, a mark where it plays, as strong as it plays. Zoomed out until the
 * steps crowd together, the strip steps aside rather than smear into a bar.
 */

/** A deck's rhythm, as `rhythm_line` sends it. */
export interface RhythmLine {
  first_frame: number;
  frames_per_step: number;
  /** Per step: kick, snare, hat, 0–255. */
  steps: [number, number, number][];
}

/** The voices, bottom row first — the kick at the floor, as on a mixer. */
export const VOICES = ["kick", "snare", "hat"] as const;

/** Narrower than this, in pixels, and the steps blur; the strip is not drawn. */
export const NARROWEST_STEP_PX = 3;

/** Below this a step is a whisper of the voice, not a hit, and is not drawn. */
export const FAINTEST = 48;

export interface Dot {
  /** Pixels from the left of the stretch drawn. */
  x: number;
  /** 0 kick, 1 snare, 2 hat. */
  voice: 0 | 1 | 2;
  /** 0–1. */
  strength: number;
}

/** The marks for the steps between `from` and `to` (frames). */
export function rhythmDots(
  line: RhythmLine,
  from: number,
  to: number,
  framesPerPixel: number,
): Dot[] {
  const step = line.frames_per_step;
  if (!(step > 0) || !(framesPerPixel > 0) || step / framesPerPixel < NARROWEST_STEP_PX) return [];
  const first = Math.max(0, Math.ceil((from - line.first_frame) / step));
  const last = Math.min(line.steps.length - 1, Math.floor((to - line.first_frame) / step));
  const dots: Dot[] = [];
  for (let i = first; i <= last; i++) {
    const x = (line.first_frame + i * step - from) / framesPerPixel;
    const strengths = line.steps[i];
    for (let voice = 0 as 0 | 1 | 2; voice < 3; voice = (voice + 1) as 0 | 1 | 2) {
      const value = strengths[voice];
      if (value >= FAINTEST) dots.push({ x, voice, strength: value / 255 });
    }
  }
  return dots;
}
