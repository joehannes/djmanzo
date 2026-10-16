import { describe, expect, it } from "vitest";

import { cloneOf, installLegacy } from "./legacy";

describe("what an older webview lacks", () => {
  it("clones deeply, cycles and all, sharing nothing with the original", () => {
    const kit = { name: "DJ Rosa", fees: [{ what: "A night", price: "€400" }], when: new Date(0), tags: new Set(["a"]) };
    const copy = cloneOf(kit);
    expect(copy).toEqual(kit);
    copy.fees[0].price = "€1";
    copy.tags.add("b");
    expect(kit.fees[0].price).toBe("€400");
    expect(kit.tags.has("b")).toBe(false);
    expect(copy.when).not.toBe(kit.when);
    const loop: { self?: unknown } = {};
    loop.self = loop;
    const cloned = cloneOf(loop);
    expect(cloned.self).toBe(cloned);
  });

  it("fills in only what is missing", () => {
    const own = () => 1;
    const old = { structuredClone: undefined, Object: { ...Object, hasOwn: undefined } } as unknown as typeof globalThis;
    installLegacy(old);
    const filled = old as unknown as { structuredClone: (v: unknown) => unknown; Object: { hasOwn: (o: object, k: string) => boolean } };
    expect(filled.structuredClone({ a: [1] })).toEqual({ a: [1] });
    expect(filled.Object.hasOwn({ a: 1 }, "a")).toBe(true);
    expect(filled.Object.hasOwn(Object.create({ a: 1 }), "a")).toBe(false);
    const prefixed = function webkitAudioContext() {};
    const safari13 = { webkitAudioContext: prefixed, Object: { ...Object } } as unknown as typeof globalThis;
    installLegacy(safari13);
    expect((safari13 as unknown as { AudioContext: unknown }).AudioContext).toBe(prefixed);
    const current = { structuredClone: own, Object: { ...Object } } as unknown as typeof globalThis;
    installLegacy(current);
    expect((current as unknown as { structuredClone: unknown }).structuredClone).toBe(own);
  });
});
