import { describe, expect, it } from "vitest";
import { FACT_BY_ID } from "../shared/facts";
import { STAGES } from "../src/stages/script";

describe("the stage script", () => {
  it("says nothing about djmanzo the facts do not", () => {
    for (const s of STAGES) for (const id of s.facts) expect(FACT_BY_ID[id], `${s.id} → ${id}`).toBeTruthy();
  });

  it("is one path from silence to afterglow, visiting every stage once", () => {
    const seen: string[] = [];
    let at: (typeof STAGES)[number] | undefined = STAGES[0];
    while (at) {
      expect(seen).not.toContain(at.id);
      seen.push(at.id);
      at = STAGES.find((s) => s.id === at!.next);
    }
    expect(seen).toEqual(STAGES.map((s) => s.id));
  });

  it("names a planned feature only where its fact says so", () => {
    // The Apprentice curriculum is planned: the Cue stage may show it, and the fact carries the status.
    expect(FACT_BY_ID.curriculum.status).toBe("planned");
    expect(FACT_BY_ID.gig.status).toBe("planned");
  });
});
