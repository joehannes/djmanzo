import { describe, expect, it } from "vitest";
import { current, FADE, HOLD, offer, start, step, type Show } from "./artistShow";

/** `seconds` of frames at thirty a second. */
function run<P>(show: Show<P>, seconds: number) {
  for (let t = 0; t < seconds; t += 1 / 30) step(show, 1 / 30);
}

describe("the artist's photographs", () => {
  it("fade in, are held, and give way to the next in turn", () => {
    const show = start<string>();
    expect(current(show)).toBeNull();
    offer(show, ["a", "b", "c"]);
    step(show, 1 / 30);
    expect(current(show)).toBe("a");
    expect(show.fade).toBe(0);

    run(show, FADE / 2);
    expect(show.fade).toBeGreaterThan(0.4);
    expect(show.fade).toBeLessThan(0.6);
    run(show, FADE / 2 + HOLD - 0.5);
    expect(current(show)).toBe("a");
    expect(show.fade).toBe(1);

    // Held its time, it fades out — still itself until it has.
    run(show, 0.5 + FADE / 2);
    expect(current(show)).toBe("a");
    expect(show.fade).toBeLessThan(0.6);
    run(show, FADE / 2 + 0.1);
    expect(current(show)).toBe("b");
    run(show, FADE + HOLD + FADE + 0.2);
    expect(current(show)).toBe("c");
    run(show, FADE + HOLD + FADE + 0.2);
    expect(current(show)).toBe("a");
  });

  it("holds a single photograph for as long as its artist is heard", () => {
    const show = start<string>();
    offer(show, ["only"]);
    run(show, FADE + HOLD * 5);
    expect(current(show)).toBe("only");
    expect(show.fade).toBe(1);
  });

  it("lets the last artist fade out before the next fades in", () => {
    const show = start<string>();
    offer(show, ["first"]);
    run(show, FADE + 1);
    offer(show, ["second"]);
    run(show, FADE / 2);
    expect(current(show)).toBe("first");
    expect(show.fade).toBeLessThan(0.6);
    run(show, FADE / 2 + 0.1);
    expect(current(show)).toBe("second");
    run(show, FADE + 0.1);
    expect(show.fade).toBe(1);

    // An artist with none fades the last one out, and shows nothing.
    offer(show, []);
    run(show, FADE + 0.1);
    expect(current(show)).toBeNull();
    run(show, HOLD);
    expect(current(show)).toBeNull();
    expect(show.fade).toBe(0);
  });
});
