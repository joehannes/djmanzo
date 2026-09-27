/**
 * §114: each effect's knob draws what the effect does — see `fxFaces.ts`.
 * The numbers are `dj_dsp::fx`'s; `dj_dsp::fx::tests::the_effect_faces_draw_these_effects`
 * holds the two files together.
 */
import { describe, expect, it } from "vitest";

import { EFFECTS } from "../api";
import { hzAt } from "./faces";
import {
  AUTO_FILTER_MIDDLE_HZ,
  ECHO_TAPS,
  TIME,
  autoFilterDb,
  crushed,
  delayTaps,
  echoTaps,
  flangerDb,
  fxFaceOf,
  fxFacePath,
  gateGain,
  phaserDb,
  reverbDb,
} from "./fxFaces";

/** The response over the face's axis, finely. */
const sweep = (response: (hz: number) => number, points = 2_000) =>
  Array.from({ length: points }, (_, i) => {
    const hz = hzAt(i / (points - 1));
    return { hz, db: response(hz) };
  });

const stems = (d: string) => (d.match(/M /g) ?? []).length;

describe("an effect's face", () => {
  it("every effect in the rack has one, and an empty slot has none", () => {
    for (const kind of EFFECTS) {
      if (kind === "none") expect(fxFaceOf(kind)).toBeUndefined();
      else expect(fxFaceOf(kind), kind).toBe(`fx-${kind}`);
    }
    expect(fxFaceOf("wobble")).toBeUndefined();
  });

  /** **The echo's repeats, each quieter by the feedback**: 0.9 of the knob. */
  it("an echo draws its repeats, each the last times the feedback", () => {
    const taps = echoTaps(0.5);
    expect(taps).toHaveLength(ECHO_TAPS);
    expect(taps[0]).toBe(1);
    for (let k = 1; k < taps.length; k++) expect(taps[k] / taps[k - 1]).toBeCloseTo(0.45, 9);
    expect(stems(fxFacePath("fx-echo", 0)), "no feedback, one repeat").toBe(1);
    expect(stems(fxFacePath("fx-echo", 1)), "full feedback rings on").toBe(ECHO_TAPS);
    expect(stems(fxFacePath("fx-echo", 0.3))).toBeLessThan(ECHO_TAPS);
  });

  it("a delay's two taps come apart with the spread", () => {
    expect(delayTaps(0)).toEqual({ left: 1, right: 1 });
    expect(delayTaps(1)).toEqual({ left: 0.5, right: 1.5 });
    const [left, right] = [...fxFacePath("fx-delay", 1).matchAll(/M ([\d.]+)/g)].map((m) => Number(m[1]));
    expect(right - left).toBeGreaterThan(15);
  });

  /**
   * **A room's tail is a straight line in decibels**, and a bigger room's is
   * shallower: the smallest is sixty down in under a second, the largest
   * barely fifteen down after two.
   */
  it("a reverb's tail falls straight in decibels, slower the bigger the room", () => {
    const small = [0.25, 0.5].map((s) => reverbDb(0, s));
    expect(small[1]).toBeCloseTo(2 * small[0], 6);
    expect(reverbDb(0, 0.8)).toBe(-60);
    expect(reverbDb(1, 2)).toBeGreaterThan(-30);
    expect(reverbDb(1, 1)).toBeGreaterThan(reverbDb(0.5, 1));
    expect(reverbDb(0.5, 1)).toBeGreaterThan(reverbDb(0, 1));
  });

  /** **The gate's width is the share of each beat it lets through**, 10 % to 90 %. */
  it("a gate lets through a share of its period set by the width", () => {
    const open = (amount: number) => {
      const n = 10_000;
      let sum = 0;
      for (let i = 0; i < n; i++) sum += gateGain(amount, i / n);
      return sum / n;
    };
    expect(open(0)).toBeCloseTo(0.1, 1);
    expect(open(1)).toBeCloseTo(0.9, 1);
    expect(gateGain(0.5, 0)).toBe(1);
    expect(gateGain(0.5, 0.7)).toBe(0);
  });

  /** **Coarser in level and in time together**: smooth at nothing, a few steps at full. */
  it("a crush steps a sine coarser in time and in level", () => {
    const distinct = (values: number[]) => new Set(values).size;
    // A sine through 64 samples has 33 distinct values: each twice but the peaks.
    expect(distinct(crushed(0))).toBe(33);
    expect(distinct(crushed(1))).toBeLessThanOrEqual(3);
    expect(distinct(crushed(0.5))).toBeLessThan(distinct(crushed(0)));
    for (const value of crushed(1)) expect(Math.abs(value * 8 - Math.round(value * 8))).toBeLessThan(1e-9);
  });

  /**
   * **A flanger is a comb**: with no feedback its teeth reach 0 dB and its
   * notches go deep; with feedback the teeth rise above the dry.
   */
  it("a flanger draws a comb whose teeth rise with the depth", () => {
    const none = sweep((hz) => flangerDb(0, hz));
    expect(Math.min(...none.map((p) => p.db))).toBeLessThan(-20);
    expect(Math.max(...none.map((p) => p.db))).toBeLessThanOrEqual(0.001);
    const full = sweep((hz) => flangerDb(1, hz));
    expect(Math.max(...full.map((p) => p.db))).toBeGreaterThan(3);
  });

  /** **A phaser's notches sweep higher the more of its travel it is given.** */
  it("a phaser's notches reach higher with the sweep", () => {
    // The lowest notch: the first local minimum ten decibels down.
    const firstNotch = (amount: number) => {
      const points = sweep((hz) => phaserDb(amount, hz), 4_000);
      const at = points.findIndex(
        (p, i) => i > 0 && i < points.length - 1 && p.db < -10 && p.db <= points[i - 1].db && p.db <= points[i + 1].db,
      );
      return at < 0 ? Infinity : points[at].hz;
    };
    expect(Math.min(...sweep((hz) => phaserDb(1, hz)).map((p) => p.db))).toBeLessThan(-20);
    expect(firstNotch(1)).toBeGreaterThan(firstNotch(0));
  });

  /**
   * **The bite is resonance**: at nothing the auto-filter is a plain
   * Butterworth with no peak; at full it whistles at its corner by about
   * twenty times log Q.
   */
  it("the auto-filter peaks at its corner by its bite", () => {
    const peak = (amount: number) => Math.max(...sweep((hz) => autoFilterDb(amount, hz)).map((p) => p.db));
    expect(peak(0)).toBeLessThan(0.1);
    expect(peak(1)).toBeGreaterThan(15);
    expect(peak(1)).toBeLessThan(18);
    expect(autoFilterDb(1, AUTO_FILTER_MIDDLE_HZ * 8)).toBeLessThan(-20);
  });

  it("every face stays on the knob", () => {
    for (const kind of EFFECTS) {
      const face = fxFaceOf(kind);
      if (!face) continue;
      for (const amount of [0, 0.5, 1]) {
        const numbers = [...fxFacePath(face, amount).matchAll(/[\d.]+/g)].map((m) => Number(m[0]));
        for (const value of numbers) {
          expect(value, `${face} at ${amount}`).toBeGreaterThanOrEqual(0);
          expect(value, `${face} at ${amount}`).toBeLessThanOrEqual(100);
        }
      }
    }
    expect(TIME.floor).toBeGreaterThan(TIME.top);
  });
});
