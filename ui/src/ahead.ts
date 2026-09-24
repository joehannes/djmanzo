/**
 * §116: what is coming in the record, counted in bars.
 *
 * > i'd love to also make the frequency and intensity visible ... so DJs can
 * > see instruments coming via the waveform ahead of time
 *
 * Every *claim* here is Rust's: which currents come and go and where the
 * record builds is `dj_analysis::energy::Trajectory::changes`, and where it
 * thins out and comes back is the trajectory's own breakdowns and drops. What
 * this does is the arithmetic a DJ would otherwise do in their head — how far
 * away the next one is, in the bars they count in — and which to say when two
 * land on the same downbeat.
 */
import type { EnergyTrajectory, RecordChange } from "./api";
import { STEM_COLORS, STEM_LABELS } from "./stems";

/** How far ahead is worth saying: thirty-two bars, two phrases of sixteen. */
export const HORIZON_BARS = 32;

/** One thing coming, ready to draw. */
export interface Coming {
  at: number;
  /** What a DJ would call it: "Vocals in", "Drop". */
  says: string;
  /** Which of §25's layers it belongs to, so the layer switches still hide it. */
  layer: "stems" | "drops" | "breakdowns" | "energy";
  /** A colour token, never a hex value. */
  colour: string;
  /** Whole bars away, rounded down; 0 inside the last bar. */
  bars: number;
  /** Beats away, for the last bar. */
  beats: number;
}

/**
 * Frames in one beat, from the trajectory's own windows.
 *
 * Measured from the gap between two windows rather than from the tempo: this
 * side of the bridge does not know the grid, and the gap is a whole number of
 * beats by construction — the same reasoning the overview's columns use.
 */
export function framesPerBeat(trajectory: EnergyTrajectory | null | undefined): number | null {
  const sections = trajectory?.sections ?? [];
  const beats = trajectory?.beats_per_section ?? 0;
  if (sections.length < 2 || beats <= 0) return null;
  const gap = sections[1].at - sections[0].at;
  return gap > 0 ? gap / beats : null;
}

/**
 * Everything in the record that could be said, before any is chosen.
 *
 * A drop outranks a rise at the same place and a breakdown a settle: they are
 * the same moment, and the named one is the one a DJ plans around.
 */
export function happenings(
  changes: RecordChange[],
  trajectory: EnergyTrajectory | null | undefined,
): Omit<Coming, "bars" | "beats">[] {
  const drops = trajectory?.drops ?? [];
  const breakdowns = (trajectory?.breakdowns ?? []).map((span) => span.from);
  const all: Omit<Coming, "bars" | "beats">[] = [];
  for (const at of drops) all.push({ at, says: "Drop", layer: "drops", colour: "var(--shape)" });
  for (const at of breakdowns) {
    all.push({ at, says: "Breakdown", layer: "breakdowns", colour: "var(--shape)" });
  }
  for (const change of changes) {
    if ((change.kind === "enters" || change.kind === "leaves") && change.stem != null) {
      const name = STEM_LABELS[change.stem];
      if (!name) continue;
      all.push({
        at: change.at,
        says: `${name} ${change.kind === "enters" ? "in" : "out"}`,
        layer: "stems",
        colour: STEM_COLORS[change.stem],
      });
    } else if (change.kind === "rises" && !drops.includes(change.at)) {
      all.push({ at: change.at, says: "Builds", layer: "energy", colour: "var(--shape)" });
    } else if (change.kind === "settles" && !breakdowns.includes(change.at)) {
      all.push({ at: change.at, says: "Settles", layer: "energy", colour: "var(--shape)" });
    }
  }
  return all.sort((a, b) => a.at - b.at);
}

/**
 * What is coming after `position`, nearest first, within the horizon.
 *
 * `shown` is the layer switch: a DJ who turned the stems off does not want
 * them read out either.
 */
export function comingUp(
  position: number,
  changes: RecordChange[],
  trajectory: EnergyTrajectory | null | undefined,
  shown: (layer: Coming["layer"]) => boolean = () => true,
  horizonBars = HORIZON_BARS,
): Coming[] {
  const beat = framesPerBeat(trajectory);
  if (beat == null) return [];
  const bar = beat * 4;
  return happenings(changes, trajectory)
    .filter((thing) => thing.at > position && shown(thing.layer))
    .map((thing) => {
      const away = thing.at - position;
      return { ...thing, bars: Math.floor(away / bar), beats: Math.ceil(away / beat) };
    })
    .filter((thing) => thing.bars < horizonBars);
}

/** "8 bars", "1 bar", "3 beats" — how a DJ says how far. */
export function distance(thing: Pick<Coming, "bars" | "beats">): string {
  if (thing.bars >= 1) return thing.bars === 1 ? "1 bar" : `${thing.bars} bars`;
  return thing.beats === 1 ? "1 beat" : `${thing.beats} beats`;
}
