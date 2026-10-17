import { describe, expect, it } from "vitest";
import {
  BUY_ONCE,
  CHEAPEST_WITH,
  EDITIONS,
  EDITION_INFO,
  FOUNDATION,
  MARKET,
  MODULES,
  MODULE_PRICE,
  NAMED,
  PRO_MONTH,
  betterThanCustom,
  listPrice,
  modulesOf,
  psychological,
} from "../shared/catalog";
import { FACTS } from "../shared/facts";
import { academic, band, proven, regional, studentPrice } from "../shared/regions";

describe("the editions are the application's", () => {
  it("seven editions and fifteen modules, as dj_core::edition has them", () => {
    expect(EDITIONS).toEqual(["autopilot", "hobbyist", "apprentice", "karaoke", "pro", "full", "custom"]);
    expect(MODULES).toHaveLength(15);
    expect(EDITION_INFO.apprentice.modules).toEqual(["assistant", "teacher"]);
    expect(EDITION_INFO.karaoke.modules).toEqual(["karaoke"]);
    expect(EDITION_INFO.pro.modules).not.toContain("karaoke");
    expect(EDITION_INFO.pro.modules).not.toContain("teacher");
    expect(EDITION_INFO.full.modules).toEqual(MODULES);
    expect(CHEAPEST_WITH.stems).toBe("pro");
  });

  it("Custom carries what was chosen, in the application's order, and nothing it does not have", () => {
    expect(modulesOf("custom", ["dvs", "stems", "not-a-module"])).toEqual(["stems", "dvs"]);
    expect(modulesOf("karaoke", ["stems"])).toEqual(["karaoke"]);
  });

  it("every fact names a module the application has", () => {
    for (const f of FACTS) expect(f.module === "foundation" || MODULES.includes(f.module), f.id).toBe(true);
  });
});

describe("prices are EDITIONS.md's", () => {
  it("buying once costs what EDITIONS.md proposes", () => {
    expect(BUY_ONCE).toEqual({ autopilot: 4900, hobbyist: 3900, apprentice: 6900, karaoke: 5500, pro: 11900, full: 14900 });
  });

  it("Pro DJ is half of the professional average", () => {
    const pros = ["Serato DJ Pro", "VirtualDJ Pro Infinity", "Traktor Pro 4", "PCDJ DEX 4 Pro"];
    const avg = MARKET.filter((r) => pros.includes(r.product)).reduce((a, r) => a + r.perpetual!, 0) / pros.length;
    expect(avg).toBeCloseTo(236.5);
    expect(Math.abs(BUY_ONCE.pro / 100 - avg / 2)).toBeLessThan(1);
  });

  it("every edition costs less than the same modules bought in Custom", () => {
    for (const e of NAMED) {
      if (e === "hobbyist") continue;
      expect(BUY_ONCE[e], e).toBeLessThan(listPrice("custom", "perpetual", modulesOf(e)));
    }
    expect(listPrice("custom", "perpetual", MODULES)).toBe(21400);
    expect(FOUNDATION).toBe(BUY_ONCE.hobbyist);
  });

  it("a Custom choice that reaches an edition's price is pointed at the edition", () => {
    expect(betterThanCustom(["autopilot"])).toBe("autopilot");
    expect(betterThanCustom(["karaoke", "four_decks"])).toBeNull();
    expect(betterThanCustom(["assistant", "teacher"])).toBe("apprentice");
    expect(betterThanCustom(["stems", "dvs"])).toBeNull();
    expect(listPrice("custom", "perpetual", ["stems", "dvs"])).toBe(FOUNDATION + MODULE_PRICE.stems + MODULE_PRICE.dvs);
  });

  it("a month is no more than half the competitors', and a year is ten months", () => {
    expect(PRO_MONTH).toBeCloseTo(((11.99 + 19 + 15) / 3 / 2) * 100, 0);
    expect(listPrice("pro", "monthly")).toBeLessThanOrEqual(PRO_MONTH);
    for (const e of NAMED) {
      const m = listPrice(e, "monthly");
      expect([49, 99], `${e} monthly`).toContain(m % 100);
      expect(listPrice(e, "yearly"), `${e} yearly`).toBe(psychological(m * 10));
      expect(listPrice(e, "yearly"), `${e}: a year is less than buying once`).toBeLessThan(BUY_ONCE[e]);
    }
    // In proportion: Full's month to Pro DJ's as their one-time prices.
    expect(listPrice("full", "monthly") / listPrice("pro", "monthly")).toBeCloseTo(BUY_ONCE.full / BUY_ONCE.pro, 1);
  });

  it("prices end in .49 or .99 when computed", () => {
    expect(psychological(883)).toBe(899);
    expect(psychological(433)).toBe(449);
    expect(psychological(14950)).toBe(14949);
  });
});

describe("regional and student prices", () => {
  it("bands countries by the World Bank's income groups", () => {
    expect(band("US")).toBe(1);
    expect(band("AT")).toBe(1);
    expect(band("ES")).toBe(1);
    expect(band("DO")).toBe(2);
    expect(band("MX")).toBe(2);
    expect(band("IN")).toBe(3);
    expect(band("HT")).toBe(3);
    expect(band("ET")).toBe(4);
    expect(band(null)).toBe(1);
    expect(band("TW")).toBe(1);
  });

  it("each band pays its share: 60, 40 and 30 %", () => {
    expect(regional("pro", "perpetual", 1)).toBe(11900);
    expect(regional("pro", "perpetual", 2)).toBe(psychological(11900 * 0.6));
    expect(regional("pro", "perpetual", 3)).toBe(psychological(11900 * 0.4));
    expect(regional("pro", "perpetual", 4)).toBe(psychological(11900 * 0.3));
    expect(regional("pro", "monthly", 3)).toBeLessThan(listPrice("pro", "monthly"));
  });

  it("students and teachers pay half", () => {
    expect(studentPrice("full", "perpetual")).toBe(psychological(14900 / 2));
  });

  it("a school address is one a domain can vouch for", () => {
    expect(academic("ana@mit.edu")).toBe(true);
    expect(academic("ana@student.uni.edu.do")).toBe(true);
    expect(academic("ana@ox.ac.uk")).toBe(true);
    expect(academic("ana@univie.ac.at")).toBe(true);
    expect(academic("ana@gmail.com")).toBe(false);
    expect(academic("ana@educated.com")).toBe(false);
    expect(academic("ana@tu-berlin.de")).toBe(false);
    expect(academic("ana@tu-berlin.de", ["tu-berlin.de"])).toBe(true);
    expect(academic(null)).toBe(false);
  });

  it("grants the claimed band when card and address are both there", () => {
    expect(proven({ instrument: "DO", billing: "DO" }, 2)).toMatchObject({ band: 2, granted: true });
  });

  it("a foreign card does not lower a price, whatever the address says", () => {
    expect(proven({ instrument: "US", billing: "DO" }, 2)).toMatchObject({ band: 1, granted: false });
  });

  it("a borrowed address does not lower a price either", () => {
    expect(proven({ instrument: "IN", billing: "DE" }, 3)).toMatchObject({ band: 1, granted: false });
  });

  it("a missing country proves nothing", () => {
    expect(proven({ instrument: "DO", billing: null }, 2)).toMatchObject({ band: 1, granted: false });
  });

  it("a buyer who claims full price and proves a cheaper band still pays what they agreed", () => {
    expect(proven({ instrument: "IN", billing: "IN" }, 1)).toMatchObject({ band: 1, granted: true });
  });

  it("refuses embargoed countries outright", () => {
    expect(proven({ instrument: "CU", billing: "CU" }, 2).refused).toBe(true);
  });
});
