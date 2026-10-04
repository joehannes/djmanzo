import { describe, expect, it } from "vitest";

import { DRIFT_PLAYING, follow, share, stack, type DeckForPicture } from "./videoMix";

function deck(number: number, over: Partial<DeckForPicture> = {}): DeckForPicture {
  return {
    number,
    playing: true,
    position_seconds: 10,
    rate: 1,
    volume: 1,
    crossfader_assign: number === 1 ? "left" : "right",
    reversed: false,
    ...over,
  };
}

/** What the stack draws, as the weight each deck's picture ends up with over black. */
function seen(s: ReturnType<typeof stack>): Record<number, number> {
  const weights: Record<number, number> = {};
  let rest = 1;
  for (const layer of [...s.layers].reverse()) {
    weights[layer.deck] = s.whole * rest * layer.opacity;
    rest *= 1 - layer.opacity;
  }
  return weights;
}

describe("§122: the pictures as the mix is heard", () => {
  it("a deck's share is its fader and its side of the crossfader", () => {
    expect(share(deck(1), -1)).toBe(1);
    expect(share(deck(1), 1)).toBe(0);
    expect(share(deck(2), 0)).toBe(0.5);
    expect(share(deck(2, { volume: 0.5 }), 1)).toBe(0.5);
    expect(share(deck(3, { crossfader_assign: "thru" }), 1)).toBe(1);
  });

  it("two pictures at the middle are seen half and half", () => {
    const s = stack([deck(1), deck(2)], [1, 2], 0);
    expect(s.whole).toBe(1);
    const w = seen(s);
    expect(w[1]).toBeCloseTo(0.5);
    expect(w[2]).toBeCloseTo(0.5);
  });

  it("the crossfader over to one side shows that side's picture alone", () => {
    const s = stack([deck(1), deck(2)], [1, 2], -1);
    expect(s.layers).toEqual([{ deck: 1, opacity: 1 }]);
    expect(seen(s)[1]).toBeCloseTo(1);
  });

  it("a picture fades to black as a record without one takes over", () => {
    // Deck 2 has no picture. A quarter of the way across, deck 1 is three
    // quarters of what is heard, and so of what is seen.
    const s = stack([deck(1), deck(2)], [1], -0.5);
    expect(s.whole).toBeCloseTo(0.75);
    expect(seen(s)[1]).toBeCloseTo(0.75);
    expect(stack([deck(1), deck(2)], [1], 1)).toEqual({ whole: 0, layers: [] });
  });

  it("nothing heard is nothing seen: faders down, or nothing playing", () => {
    expect(stack([deck(1, { volume: 0 }), deck(2, { volume: 0 })], [1, 2], 0).whole).toBe(0);
    expect(stack([deck(1, { playing: false }), deck(2, { playing: false })], [1, 2], 0).whole).toBe(0);
  });

  it("three pictures are each seen in their share", () => {
    const decks = [deck(1), deck(2), deck(3, { crossfader_assign: "thru", volume: 0.5 })];
    const w = seen(stack(decks, [1, 2, 3], 0));
    // Shares 0.5, 0.5, 0.5 of 1.5: a third each.
    for (const n of [1, 2, 3]) expect(w[n]).toBeCloseTo(1 / 3);
  });

  it("a pinned deck is shown alone whatever the faders say, and only if it has a picture", () => {
    expect(stack([deck(1), deck(2, { volume: 0 })], [1, 2], 1, 2)).toEqual({
      whole: 1,
      layers: [{ deck: 2, opacity: 1 }],
    });
    expect(stack([deck(1), deck(2)], [1], 0, 2)).toEqual({ whole: 0, layers: [] });
  });
});

describe("§122: a picture keeps to its deck", () => {
  it("plays at the deck's rate and lets small drift be", () => {
    expect(follow(deck(1, { rate: 1.06 }), 10.05)).toEqual({ play: true, rate: 1.06, seek: null });
  });

  it("is moved when it drifts past a few frames", () => {
    expect(follow(deck(1), 10 + DRIFT_PLAYING + 0.01)).toEqual({ play: true, rate: 1, seek: 10 });
    expect(follow(deck(1, { position_seconds: 42 }), 3)).toEqual({ play: true, rate: 1, seek: 42 });
  });

  it("holds still with a paused deck, at the deck's frame", () => {
    expect(follow(deck(1, { playing: false }), 10)).toEqual({ play: false, rate: 1, seek: null });
    expect(follow(deck(1, { playing: false }), 9)).toEqual({ play: false, rate: 1, seek: 10 });
  });

  it("follows a deck played backwards, or past what an element can play, by jumps", () => {
    expect(follow(deck(1, { reversed: true }), 11)).toEqual({ play: false, rate: 1, seek: 10 });
    expect(follow(deck(1, { rate: 8 }), 11).play).toBe(false);
    expect(follow(deck(1, { rate: 0.1 }), 11).play).toBe(false);
  });
});
