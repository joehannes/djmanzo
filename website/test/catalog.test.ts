import { describe, expect, it } from "vitest";
import { BASE, EDITIONS, PLANS, psychological, typical } from "../shared/catalog";
import { band, proven, regional } from "../shared/regions";

describe("prices are half the market's", () => {
  it("every base price is within a few percent of half the typical reference", () => {
    for (const e of EDITIONS)
      for (const p of PLANS) {
        const t = typical(e, p);
        expect(t, `${e} ${p} has a reference`).not.toBeNull();
        if (p === "yearly") continue; // capped at ten months, checked below
        const half = (t! * 100) / 2;
        expect(Math.abs(BASE[e][p] - half) / half, `${e} ${p}`).toBeLessThan(0.06);
      }
  });

  it("a year costs no more than about ten months, and never more than half the market", () => {
    for (const e of EDITIONS) {
      expect(BASE[e].yearly).toBeLessThanOrEqual(BASE[e].monthly * 10 + 99);
      expect(BASE[e].yearly).toBeLessThanOrEqual((typical(e, "yearly")! * 100) / 2 + 50);
    }
  });

  it("prices end in .49 or .99", () => {
    expect(psychological(883)).toBe(899);
    expect(psychological(433)).toBe(449);
    expect(psychological(14950)).toBe(14949);
    for (const e of EDITIONS) for (const p of PLANS) expect([49, 99]).toContain(BASE[e][p] % 100);
  });
});

describe("regional prices need proof", () => {
  it("bands countries by purchasing power", () => {
    expect(band("US")).toBe(1);
    expect(band("ES")).toBe(2);
    expect(band("DO")).toBe(3);
    expect(band("IN")).toBe(4);
    expect(band(null)).toBe(1);
  });

  it("grants the claimed band when card and address are both there", () => {
    const p = proven({ instrument: "DO", billing: "DO" }, 3);
    expect(p).toMatchObject({ band: 3, granted: true });
  });

  it("a foreign card does not lower a price, whatever the address says", () => {
    const p = proven({ instrument: "US", billing: "DO" }, 3);
    expect(p.granted).toBe(false);
    expect(p.band).toBe(1);
  });

  it("a borrowed address does not lower a price either", () => {
    expect(proven({ instrument: "IN", billing: "DE" }, 4)).toMatchObject({ band: 1, granted: false });
  });

  it("a missing country proves nothing", () => {
    expect(proven({ instrument: "DO", billing: null }, 3)).toMatchObject({ band: 1, granted: false });
  });

  it("a buyer who claims full price and proves a cheaper band still pays what they agreed", () => {
    expect(proven({ instrument: "IN", billing: "IN" }, 1)).toMatchObject({ band: 1, granted: true });
  });

  it("refuses embargoed countries outright", () => {
    expect(proven({ instrument: "CU", billing: "CU" }, 3).refused).toBe(true);
  });

  it("a regional price is lower and still a readable price", () => {
    expect(regional("pro", "monthly", 3)).toBeLessThan(BASE.pro.monthly);
    expect([49, 99]).toContain(regional("pro", "monthly", 3) % 100);
  });
});
