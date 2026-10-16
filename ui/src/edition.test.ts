import { describe, expect, it } from "vitest";

import { firstActivity } from "./edition";

describe("where a build opens", () => {
  it("the karaoke build opens on Karaoke, once, and then where its DJ left it", () => {
    expect(firstActivity("karaoke", "")).toBe("karaoke");
    expect(firstActivity("karaoke", null)).toBe("karaoke");
    expect(firstActivity("karaoke", "dig")).toBeNull();
    expect(firstActivity("karaoke", "karaoke")).toBeNull();
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
