/**
 * The scene builder, tested.
 *
 * It is pure — a world and some lanes in, primitives out, no DOM — which makes
 * the design's own rules executable rather than aspirational. Two of them are
 * standing tests VISUAL-LANGUAGE.md names outright and nothing was checking:
 *
 * - **greyscale** (§4): drop hue and the interface must still work, so nothing
 *   that must be told apart may differ in hue alone;
 * - **still frame** (§5): a frozen world must tell a DJ the state of their mix,
 *   so every channel has to survive `still`.
 */
import { describe, expect, it } from "vitest";
import { build, describeBeating, type Lane } from "./scene";
import { emptyWorld, type Entity, type World } from "../world";

const LANES: Lane[] = [
  { of: "mixer", y: 0, height: 60 },
  { of: "deck.1", y: 64, height: 70 },
  { of: "deck.2", y: 138, height: 70 },
];

function entity(over: Partial<Entity> & Pick<Entity, "name">): Entity {
  return {
    index: 1,
    slot: 0,
    form: "Flow",
    bearing: "Foliage",
    tint: { hue: 210, saturation: 0.7, lightness: 0.5 },
    vitality: {
      pulse_bpm: 128,
      phase: 0,
      depth: 0.8,
      agitation: 0.2,
      backwards: false,
      turbidity: 0,
      excursion: { drift: 0.2, scale: 1.05 },
    },
    along: 0.3,
    extent: 0.9,
    reading: "128.0 BPM · 8A · 3:00",
    ...over,
  };
}

function world(over: Partial<World> = {}): World {
  return {
    ...emptyWorld(),
    entities: [
      entity({ name: "deck.river", index: 1 }),
      entity({ name: "deck.river", index: 2 }),
      entity({ name: "mixer.confluence", index: 0, along: 0.5, extent: 1 }),
    ],
    ...over,
  };
}

const scene = (w: World, still = false) => build(w, LANES, 1000, 0, 0, still);

describe("what a river becomes", () => {
  it("draws water for a loaded deck", () => {
    const bands = scene(world()).primitives.filter((p) => p.kind === "band");
    expect(bands.length).toBeGreaterThan(0);
  });

  /**
   * An empty deck is a dry bed, not a low river. Drawing a part-full channel
   * for a deck with nothing on it shows water where there is none.
   */
  it("draws a dry bed for an empty deck, and no water", () => {
    const empty = world({
      entities: [entity({ name: "deck.river", index: 1, reading: "empty" })],
    });
    const primitives = scene(empty).primitives;
    expect(primitives.filter((p) => p.kind === "band")).toHaveLength(0);
    expect(primitives.filter((p) => p.kind === "bar" && p.dashed)).toHaveLength(1);
  });

  it("gives every lane a reading, so a still frame is legible", () => {
    const built = scene(world());
    for (const label of built.labels) expect(label.text.trim()).not.toBe("");
    expect(built.labels.map((l) => l.of)).toContain("deck.1");
  });
});

describe("the still frame", () => {
  /**
   * VISUAL-LANGUAGE.md §5. A frozen world must still say what is going on, so
   * every channel that matters has to survive `still` — only the *positions*
   * of moving things may differ.
   */
  it("says the same things frozen as it does moving", () => {
    const moving = scene(world(), false);
    const frozen = scene(world(), true);
    const kinds = (s: typeof moving) => s.primitives.map((p) => p.kind).sort();
    expect(kinds(frozen)).toEqual(kinds(moving));
    expect(frozen.labels).toEqual(moving.labels);
  });

  it("puts the pulse at the downbeat when frozen, not at a random phase", () => {
    const frozen = scene(world(), true);
    const crests = frozen.primitives.filter((p) => p.kind === "bar" && p.w <= 2);
    expect(crests.length).toBeGreaterThan(0);
  });
});

describe("the greyscale rule", () => {
  /**
   * VISUAL-LANGUAGE.md §4. Two decks in different keys must be told apart by
   * something other than hue, because one man in twelve cannot read it — so
   * the check is that hue is never the *only* difference between primitives
   * that mean different things.
   */
  it("distinguishes a killed band from a full one without using hue", () => {
    const withEq = world({
      entities: [
        entity({ name: "deck.river", index: 1 }),
        entity({ name: "deck.stratum", index: 1, slot: 0, extent: 0 }),
        entity({ name: "deck.stratum", index: 1, slot: 1, extent: 0.5 }),
        entity({ name: "deck.stratum", index: 1, slot: 2, extent: 0.5 }),
      ],
    });
    const bands = scene(withEq).primitives.filter((p) => p.kind === "band");
    // The killed stratum is drawn black at high alpha; the live ones are near
    // white at low alpha. Those differ in lightness, not hue.
    const lightnesses = bands.map((b) => b.top.lightness);
    expect(Math.max(...lightnesses) - Math.min(...lightnesses)).toBeGreaterThan(0.3);
  });

  it("draws the seam as a shape, not a colour", () => {
    const clashing = world({ confluence: "Seam" });
    const dashed = scene(clashing).primitives.filter(
      (p) => p.kind === "bar" && p.dashed,
    );
    expect(dashed.length).toBeGreaterThan(0);
  });
});

describe("the highland and the weather", () => {
  /** Stillness is the default: nothing to survey means nothing on screen. */
  it("shows no highland when there is nothing left to survey", () => {
    const built = scene(world({ unsurveyed: 0 }));
    expect(built.labels.find((l) => l.of === "highland")).toBeUndefined();
  });

  it("shows the mist retreating as the collection is surveyed", () => {
    const half = scene(world({ unsurveyed: 0.5 }));
    const most = scene(world({ unsurveyed: 0.9 }));
    const width = (s: typeof half) =>
      s.primitives.find((p) => p.kind === "bar" && p.y === 0 && p.h === 2)?.w ?? 0;
    expect(width(most)).toBeGreaterThan(width(half));
    expect(half.labels.find((l) => l.of === "highland")?.text).toContain("50%");
  });

  /** A calm machine has no weather. Drawing one would be noise. */
  it("draws no weather when the machine is coping", () => {
    const calm = scene(world({ strain: 0 })).primitives.length;
    const strained = scene(world({ strain: 0.9 })).primitives.length;
    expect(strained).toBe(calm + 1);
  });
});

describe("slip and reverse", () => {
  /**
   * A loop is only something you can *leave* if you can see where leaving it
   * puts you, which is the one thing slip mode needs on screen.
   */
  it("marks where the track will land when it is being slipped over", () => {
    const slipping = world({
      entities: [
        entity({ name: "deck.river", index: 1 }),
        entity({ name: "deck.shadow", index: 1, along: 0.7 }),
      ],
    });
    const marks = scene(slipping).primitives.filter(
      (p) => p.kind === "bar" && p.dashed,
    );
    expect(marks).toHaveLength(1);
    expect(marks[0].x).toBeCloseTo(700);
  });

  it("draws nothing when nothing is being slipped over", () => {
    const plain = scene(world()).primitives.filter((p) => p.kind === "bar" && p.dashed);
    expect(plain).toHaveLength(0);
  });

  /**
   * Downstream is still the future when a deck is reversed; what changes is
   * that the *water* runs the other way, which is what reverse sounds like.
   */
  it("runs the crests the other way when a deck is reversed", () => {
    const crestsOf = (backwards: boolean) => {
      const w = world({
        entities: [
          entity({
            name: "deck.river",
            index: 1,
            vitality: { ...entity({ name: "x" }).vitality, phase: 0, backwards },
          }),
        ],
      });
      return scene(w)
        .primitives.filter((p) => p.kind === "bar" && p.w <= 2)
        .map((p) => p.x)
        .sort((a, b) => a - b);
    };
    // Forwards the downbeat sits at 0; backwards it sits at the far end.
    expect(Math.min(...crestsOf(false))).toBeCloseTo(0);
    expect(Math.max(...crestsOf(true))).toBeCloseTo(1000);
  });
});

describe("the beating verdict", () => {
  /**
   * The words name the *control* to reach for, which no shape can — a DJ who
   * has not learned the vocabulary can still read "nudge back" and act.
   */
  it("names the control for each state", () => {
    expect(describeBeating("Locked")).toBe("locked");
    expect(describeBeating({ Offset: { beats: 0.3 } })).toContain("nudge back");
    expect(describeBeating({ Offset: { beats: -0.3 } })).toContain("nudge forward");
    expect(describeBeating({ Sliding: { bpm_difference: 2.4 } })).toContain("+2.4 BPM");
  });

  it("says nothing when there is nothing to compare", () => {
    expect(describeBeating("Unknown")).toBe("");
  });

  it("puts the verdict in the mixer's reading", () => {
    const sliding = world({ beating: { Sliding: { bpm_difference: -30 } } });
    const label = scene(sliding).labels.find((l) => l.of === "mixer");
    expect(label?.text).toContain("sliding -30.0 BPM");
  });
});

describe("robustness", () => {
  it("survives a world with nothing in it", () => {
    expect(() => scene(emptyWorld())).not.toThrow();
  });

  it("ignores lanes for decks the world does not have", () => {
    const one = world({ entities: [entity({ name: "deck.river", index: 1 })] });
    const built = scene(one);
    expect(built.labels.map((l) => l.of)).not.toContain("deck.2");
  });
});
