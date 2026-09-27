/**
 * §114: a stem's chip draws where its current plays.
 *
 * > i'd also like to see the forms and shapes of the controls and widgets to
 * > be individual and useful and resembling nature and functionality
 *
 * The four chips under a deck's waveform mute a current, fade it and solo
 * it, and until now looked alike but for their colour. Each now carries its
 * current's share of the record, window by window, as columns along the
 * chip's foot — the same measurement §25's `vocal` and `stems` layers draw
 * (`dj_analysis::presence`), read one current at a time — with the playhead
 * on it. So the VOCALS chip shows where the singing is and whether it is
 * coming, the BASS chip where the bass drops out, before the chip is pressed.
 *
 * Columns rather than a line, for the overview's reason: each window is one
 * measurement over eight beats, and there is no reading between them.
 */
import type { EnergyTrajectory } from "./api";

/**
 * The share at which a chip's column is full: a drawing scale, the one the
 * overview's vocal strip uses, not a claim that a current is carrying the
 * record (that is Rust's `presence::STRONG`, deliberately not imported).
 */
export const PRESENCE_FULL = 0.25;

/**
 * One current's share across the record as columns in a 100 × 10 box, floor
 * at the bottom. Empty when nothing was measured; a window with no
 * measurement draws nothing rather than silence.
 */
export function presencePath(
  sections: EnergyTrajectory["sections"],
  totalFrames: number,
  stem: number,
): string {
  if (!(totalFrames > 0) || sections.length < 2) return "";
  const span = sections[1].at - sections[0].at;
  const width = (span / totalFrames) * 100;
  const columns: string[] = [];
  for (const section of sections) {
    if (!section.parts) continue;
    const share = section.parts[stem] ?? 0;
    const height = Math.min(1, Math.max(0, share / PRESENCE_FULL)) * 10;
    if (!(height > 0)) continue;
    const x = (section.at / totalFrames) * 100;
    columns.push(
      `M ${x.toFixed(2)} 10 V ${(10 - height).toFixed(2)} H ${(x + width).toFixed(2)} V 10 Z`,
    );
  }
  return columns.join(" ");
}
