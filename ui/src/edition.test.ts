import { describe, expect, it } from "vitest";

import { firstActivity, productName } from "./edition";

describe("where a build opens", () => {
  it("the karaoke build opens on Karaoke, once, and then where its DJ left it", () => {
    expect(firstActivity("karaoke", "")).toBe("karaoke");
    expect(firstActivity("karaoke", null)).toBe("karaoke");
    expect(firstActivity("karaoke", "dig")).toBeNull();
    expect(firstActivity("karaoke", "karaoke")).toBeNull();
  });

  it("an edition the licence names opens on its own activity, once", () => {
    expect(firstActivity("full", "", "karaoke")).toBe("karaoke");
    expect(firstActivity("full", null, "autopilot")).toBe("autopilot");
    expect(firstActivity("full", "dig", "autopilot")).toBeNull();
    expect(firstActivity("full", "", "pro")).toBeNull();
    expect(firstActivity("full", "", "hobbyist")).toBeNull();
  });

  it("the whole application chooses nothing for its DJ", () => {
    expect(firstActivity("full", "")).toBeNull();
    expect(firstActivity("full", "dig")).toBeNull();
  });
});

describe("how hard a build works the machine at first", () => {
  it("the karaoke build starts at Eco and the whole application at Ultra", async () => {
    const { startingTier } = await import("./performance.svelte");
    expect(startingTier("karaoke")).toBe("Eco");
    expect(startingTier("full")).toBe("Ultra");
  });
});

describe("what a build calls itself", () => {
  it("the karaoke build says so where it introduces itself", () => {
    expect(productName("karaoke")).toBe("djmanzo Karaoke");
    expect(productName("full")).toBe("djmanzo");
  });
});
