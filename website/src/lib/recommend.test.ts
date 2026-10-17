import { describe, expect, it } from "vitest";
import { recommend, tourOrder } from "./profile.svelte";

const nobody = { role: null, venues: [], gear: [], experience: null, genres: [] };

describe("the MC's pick", () => {
  it("a karaoke host gets Karaoke", () => expect(recommend({ ...nobody, role: "karaoke", venues: ["karaoke-bar"] }).edition).toBe("karaoke"));
  it("a DJ who also runs karaoke nights gets Full", () => expect(recommend({ ...nobody, role: "dj", venues: ["karaoke-bar"] }).edition).toBe("full"));
  it("both, whatever else, gets Full", () => expect(recommend({ ...nobody, role: "both" }).edition).toBe("full"));
  it("a DJ paid by a club gets Pro, and is told why", () => {
    const r = recommend({ ...nobody, role: "dj", venues: ["club"] });
    expect(r.edition).toBe("pro");
    expect(r.why).toContain("paying room");
  });
  it("turntables mean Pro even at home", () => expect(recommend({ ...nobody, role: "dj", venues: ["bedroom"], gear: ["turntables"] }).edition).toBe("pro"));
  it("a bedroom DJ with a controller gets Hobbyist", () => expect(recommend({ ...nobody, role: "dj", venues: ["bedroom"], gear: ["controller"] }).edition).toBe("hobbyist"));
  it("someone who said nothing gets the cheapest, not the dearest", () => expect(recommend(nobody).edition).toBe("hobbyist"));
});

describe("the tour", () => {
  it("starts at the dock, ends at the box office, and visits every stamped place once", () => {
    for (const role of ["dj", "karaoke", "both", "learning", "curious", null] as const) {
      const order = tourOrder({ role, genres: [] });
      expect(order[0]).toBe("muelle");
      expect(order.at(-1)).toBe("taquilla");
      expect(new Set(order).size).toBe(order.length);
    }
  });
  it("a karaoke host sees the stage first", () => expect(tourOrder({ role: "karaoke", genres: [] })[1]).toBe("tarima"));
  it("a DJ sees the booth first", () => expect(tourOrder({ role: "dj", genres: [] })[1]).toBe("cabina"));
});
