import { describe, expect, it } from "vitest";
import { HOLD_MS, hint, holdComplete, holdProgress, needsHold } from "./care";

describe("how hard a control is to hit", () => {
  /**
   * **Only destructive controls are ever hard to press.**
   *
   * Making ordinary controls need a hold would slow a DJ down on every action
   * to guard against a few, which is a worse trade than the accident it
   * prevents.
   */
  it("never asks for a hold on an ordinary control", () => {
    expect(needsHold(true, null)).toBe(false);
    expect(needsHold(false, null)).toBe(false);
  });

  /**
   * **The same control is a click at home and a hold at peak.**
   *
   * The whole point: a mis-click alone costs nothing and in front of a room is
   * heard by everyone.
   */
  it("asks for a hold only when a mistake is expensive", () => {
    expect(needsHold(true, "eject")).toBe(true);
    expect(needsHold(false, "eject")).toBe(false);
  });

  it("says so, rather than silently refusing a press", () => {
    expect(hint(true, "eject")).toBe("hold");
    expect(hint(false, "eject")).toBe("");
    expect(hint(true, null)).toBe("");
  });

  /**
   * **A brush past does not fire it.**
   *
   * Accidental contact is tens of milliseconds; the threshold is an order of
   * magnitude above that.
   */
  it("does not complete on a brief touch", () => {
    expect(holdComplete(1000, 1000 + 50)).toBe(false);
    expect(holdComplete(1000, 1000 + HOLD_MS - 1)).toBe(false);
    expect(holdComplete(1000, 1000 + HOLD_MS)).toBe(true);
  });

  /**
   * **A clock that jumps backwards does not break the progress bar.**
   *
   * A DJ's laptop is suspended between every gig, and `performance.now` is not
   * guaranteed monotonic across that. A negative width would leave an invisible
   * bar at the exact moment it is being watched.
   */
  it("clamps progress at both ends", () => {
    expect(holdProgress(1000, 500)).toBe(0);
    expect(holdProgress(1000, 1000 + HOLD_MS * 3)).toBe(1);
    expect(holdProgress(1000, 1000 + HOLD_MS / 2)).toBeCloseTo(0.5);
  });

  it("survives a clock that reports nonsense", () => {
    expect(holdProgress(Number.NaN, 1000)).toBe(0);
    expect(holdProgress(1000, Number.POSITIVE_INFINITY)).toBe(0);
  });
});
