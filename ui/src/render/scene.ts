/**
 * The world, turned into primitives — and nothing more.
 *
 * This is the layer that makes [ADR-0009](../../../docs/adr/0009-the-living-interface.md)'s
 * claim true rather than aspirational: **the world model is not the renderer's
 * to define.** A scene knows about rivers, strata, eddies and stones; it does
 * not know whether a canvas, a GL context or something else will draw it.
 *
 * The proof is that two renderers consume this file unchanged. If a drawing
 * decision were to leak into one of them, the two would drift and the
 * abstraction would be a comment rather than a fact.
 *
 * # Why one surface for the whole watershed
 *
 * The rendering benchmark found that on a machine without acceleration the cost
 * is *document invalidation*, and one self-repainting surface beats N animating
 * ones by a wide margin. A canvas per river was already better than a div per
 * river; a canvas for the whole world is better still, and it is the only shape
 * a GL renderer could take anyway — a WebGL context per lane would be absurd.
 */
import { phaseAt, type Entity, type World } from "../world";

/** What every primitive has. Coordinates are CSS pixels from the top left. */
interface Placed {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** Colour as the renderers take it: hue in degrees, the rest 0..1. */
export interface Paint {
  hue: number;
  saturation: number;
  lightness: number;
  alpha: number;
}

/**
 * A filled rectangle, optionally shading from top to bottom.
 *
 * The water body, the strata, the shear, the mouth. Everything that is an area
 * rather than a mark.
 */
export interface Band extends Placed {
  kind: "band";
  top: Paint;
  /** When absent, the band is flat. */
  bottom?: Paint;
  /**
   * Shade left-to-right rather than top-to-bottom.
   *
   * The mouth is the one thing that fades *along* the river rather than down
   * through the water, because it is a distance rather than a depth.
   */
  horizontal?: boolean;
}

/** A thin line. Crests, the playhead, the seam, the settled alarm edge. */
export interface Bar extends Placed {
  kind: "bar";
  paint: Paint;
  /** Dashed bars say "this is a boundary", not "this is a thing". */
  dashed?: boolean;
}

/** A small shape at a point. Cues are triangles; crests at the confluence are discs. */
export interface Mark extends Placed {
  kind: "mark";
  paint: Paint;
  shape: "disc" | "triangle";
}

/** The eddy's whirl: an arc that goes round rather than along. */
export interface Whirl extends Placed {
  kind: "whirl";
  paint: Paint;
  /** Radians. */
  from: number;
  sweep: number;
  thickness: number;
}

/** A curve from one point to another. The tributaries into the confluence. */
export interface Stream extends Placed {
  kind: "stream";
  paint: Paint;
  toX: number;
  toY: number;
  thickness: number;
}

export type Primitive = Band | Bar | Mark | Whirl | Stream;

/** Where a lane sits, and what it is for. */
export interface Lane {
  /** `deck.<n>` or `mixer`. */
  of: string;
  y: number;
  height: number;
}

export interface Scene {
  width: number;
  height: number;
  primitives: Primitive[];
  /** Text to place in the document, never drawn — see ADR-0009. */
  labels: { of: string; text: string; x: number; y: number }[];
}

const structural = (lightness: number, alpha = 1): Paint => ({
  hue: 0,
  saturation: 0,
  lightness,
  alpha,
});

const of = (entity: Entity, alpha = 1, lightnessScale = 1): Paint => ({
  hue: entity.tint.hue,
  saturation: entity.tint.saturation,
  lightness: entity.tint.lightness * lightnessScale,
  alpha,
});

/** How much of the lane's height the water fills, at rest and at full. */
const FLOOR = 0.12;
const CEILING = 0.68;

/**
 * Build the scene.
 *
 * `seconds` is how long since the world was read, so the pulse can be
 * interpolated between polls; `latencyMs` pulls it back so the crest lands when
 * the *room* hears the beat rather than when the engine computed it.
 * `still` drops every time-dependent term, which is what Tier 0 asks for and
 * also the cheapest test of whether a channel is carrying information.
 */
export function build(
  world: World,
  lanes: Lane[],
  width: number,
  seconds: number,
  latencyMs: number,
  still: boolean,
): Scene {
  const primitives: Primitive[] = [];
  const labels: Scene["labels"] = [];
  const at = (t: number) => (still ? 0 : t);


  for (const lane of lanes) {
    const deck = Number(lane.of.split(".")[1]);
    if (!Number.isFinite(deck)) continue;
    const river = pick(world, "deck.river", deck);
    if (!river) continue;

    labels.push({ of: lane.of, text: river.reading, x: 8, y: lane.y + 4 });

    // An empty deck is a dry bed, not a low river: drawing a quarter-full
    // channel for a deck with nothing on it shows water where there is none.
    if (river.reading === "empty") {
      primitives.push({
        kind: "bar",
        x: 0,
        y: lane.y + lane.height * 0.72,
        w: width,
        h: 1,
        paint: structural(0.42, 0.35),
        dashed: true,
      });
      continue;
    }

    const surface = lane.height * (FLOOR + river.extent * CEILING);
    const top = lane.y + lane.height - surface;
    const phase = phaseAt(river.vitality, at(seconds), latencyMs);

    // The water. Deeper is darker, which is also the low stratum.
    primitives.push({
      kind: "band",
      x: 0,
      y: top,
      w: width,
      h: surface,
      top: of(river, 0.85),
      bottom: of(river, 0.95, 0.45),
    });

    // The three strata. Slot 0 is low and low is the bottom.
    const slice = surface / 3;
    for (const stratum of all(world, "deck.stratum", deck)) {
      const bandTop = top + (2 - stratum.slot) * slice;
      const fill = Math.min(1, stratum.extent * 2);
      if (fill <= 0.001) {
        // Drought: scoured out, not dimmed. A kill is not a gentle turn.
        primitives.push({
          kind: "band",
          x: 0,
          y: bandTop,
          w: width,
          h: slice,
          top: structural(0, 0.55),
        });
        continue;
      }
      primitives.push({
        kind: "band",
        x: 0,
        y: bandTop + slice * (1 - fill),
        w: width,
        h: slice * fill,
        top: structural(1, 0.04 + 0.1 * (fill - 0.5)),
      });
    }

    // The filter, shearing the channel from one side. `along` below the middle
    // is a low-pass, so the renderer never has to know about filter signs.
    const shear = pick(world, "deck.shear", deck);
    if (shear) {
      const fromTop = shear.along < 0.5;
      const cut = surface * shear.extent;
      primitives.push({
        kind: "band",
        x: 0,
        y: fromTop ? top : top + surface - cut,
        w: width,
        h: cut,
        top: { hue: 220, saturation: 0.12, lightness: 0.08, alpha: 0.72 },
      });
      primitives.push({
        kind: "bar",
        x: 0,
        y: fromTop ? top + cut : top + surface - cut,
        w: width,
        h: 1,
        paint: structural(0.85, 0.5),
      });
    }

    // The crests: the beat, travelling. Four to a bar, the downbeat brightest,
    // because a bar you can count is worth more than four identical pulses.
    //
    // Downstream is still the future when a deck is reversed; what changes is
    // that the *water* runs the other way, which is what reverse sounds like.
    // So the crests travel right to left.
    if (river.vitality.pulse_bpm > 0 && river.vitality.depth > 0.001) {
      const backwards = river.vitality.backwards;
      for (let n = 0; n < 4; n += 1) {
        const t = (n + phase) / 4;
        primitives.push({
          kind: "bar",
          x: (backwards ? 1 - t : t) * width,
          y: top,
          w: 1.5,
          h: surface,
          paint: structural(1, (n === 0 ? 0.55 : 0.22) * river.vitality.depth),
        });
      }
    }

    // Where the track will land when whatever is diverting it stops. A loop is
    // only something you can *leave* if you can see where leaving it puts you.
    //
    // Drawn taller than the water and topped with a mark, because the first
    // version was a thin dashed line inside the lane and was simply lost among
    // the crests — findable only by counting them. A channel nobody can find
    // is not carrying information.
    const shadow = pick(world, "deck.shadow", deck);
    if (shadow) {
      const at = width * shadow.along;
      primitives.push({
        kind: "bar",
        x: at,
        y: lane.y,
        w: 2,
        h: lane.height,
        paint: structural(0.85, 0.8),
        dashed: true,
      });
      // A cap at the top, so it reads as a destination rather than as one more
      // vertical line in a lane full of them.
      primitives.push({
        kind: "mark",
        x: at - 4,
        y: lane.y,
        w: 8,
        h: 8,
        paint: structural(0.85, 0.9),
        shape: "triangle",
      });
    }

    // Murk, where the grid is not trusted. You do not navigate water you
    // cannot see through.
    if (river.vitality.turbidity > 0.001) {
      primitives.push({
        kind: "band",
        x: 0,
        y: top,
        w: width,
        h: surface,
        top: { hue: 30, saturation: 0.12, lightness: 0.55, alpha: river.vitality.turbidity * 0.45 },
      });
    }

    // Mist over an unsurveyed stretch: you know the river is there and cannot
    // see its features. Distinct from murk, which is a grid you have and do not
    // trust — this is not having one yet, and the two should not look alike.
    if (river.reading.includes("analysing")) {
      primitives.push({
        kind: "band",
        x: 0,
        y: lane.y,
        w: width,
        h: lane.height,
        top: { hue: 210, saturation: 0.08, lightness: 0.7, alpha: 0.3 },
        bottom: { hue: 210, saturation: 0.08, lightness: 0.7, alpha: 0.12 },
      });
    }

    // The eddy: water going round instead of forward.
    const eddy = pick(world, "deck.eddy", deck);
    if (eddy) {
      const from = width * eddy.along;
      const w = Math.max(3, width * eddy.extent);
      primitives.push({
        kind: "band",
        x: from,
        y: top,
        w,
        h: surface,
        top: structural(1, 0.12),
      });
      const radius = Math.min(surface * 0.32, w * 0.42);
      if (radius >= 2) {
        primitives.push({
          kind: "whirl",
          x: from + w / 2 - radius,
          y: top + surface / 2 - radius,
          w: radius * 2,
          h: radius * 2,
          paint: structural(1, 0.7),
          from: at(phase) * Math.PI * 2,
          sweep: Math.PI * 1.4,
          thickness: 1.5,
        });
      }
    }

    // Stones: fixed places, visible from upstream.
    for (const stone of all(world, "deck.stone", deck)) {
      primitives.push({
        kind: "mark",
        x: width * stone.along - 3,
        y: top - 2,
        w: 6,
        h: 6,
        paint: of(stone, 0.9),
        shape: "triangle",
      });
    }

    // Where we are along the river. Structural: this is time, not music.
    primitives.push({
      kind: "bar",
      x: width * river.along,
      y: top - 4,
      w: 1,
      h: surface + 4,
      paint: structural(0.9, 0.8),
    });

    // The mouth: the end, visible from far off.
    const mouth = pick(world, "deck.mouth", deck);
    if (mouth && mouth.extent > 0.001) {
      const reach = width * 0.35 * mouth.extent;
      primitives.push({
        kind: "band",
        x: width - reach,
        y: lane.y,
        w: reach,
        h: lane.height,
        // Transparent upstream, solid at the mouth: the end is a thing you see
        // approaching, not a block that appears.
        top: of(mouth, 0),
        bottom: of(mouth, 0.6 * mouth.extent),
        horizontal: true,
      });
    }

    // The peripheral claim, when this river holds it. Luminance, never hue:
    // the periphery reads brightness well and colour badly.
    if (holdsAlarm(world, deck)) {
      primitives.push({
        kind: "bar",
        x: 0,
        y: lane.y,
        w: width,
        h: 2,
        paint: structural(1, 0.55),
      });
    }
  }

  // Weather. The machine struggling is a fact about the whole watershed, not
  // about any one river, so it is drawn across all of them. Dropouts already
  // own the peripheral channel when they happen; this is the state below that,
  // and it stays still — a strained machine is a condition, not an event.
  if (world.strain > 0.05) {
    const last = lanes[lanes.length - 1];
    primitives.push({
      kind: "band",
      x: 0,
      y: 0,
      w: width,
      h: last ? last.y + last.height : 0,
      top: { hue: 25, saturation: 0.35, lightness: 0.5, alpha: world.strain * 0.16 },
    });
  }

  // The highland: how much of the collection is still under mist. Drawn only
  // while there is something to survey — a band saying "nothing is happening"
  // is noise, and stillness is the default.
  if (world.unsurveyed > 0.001) {
    const bar = lanes[0];
    if (bar) {
      primitives.push({
        kind: "bar",
        x: 0,
        y: 0,
        w: width * world.unsurveyed,
        h: 2,
        paint: { hue: 210, saturation: 0.25, lightness: 0.72, alpha: 0.6 },
      });
      labels.push({
        of: "highland",
        // The count, not the percentage: a DJ waiting on forty files wants to
        // know it is forty.
        text: `surveying · ${Math.round(world.unsurveyed * 100)}% under mist`,
        x: 8,
        y: bar.y + bar.height - 14,
      });
    }
  }

  const mixerLane = lanes.find((l) => l.of === "mixer");
  if (mixerLane) {
    buildConfluence(world, mixerLane, width, at(seconds), latencyMs, primitives, labels);
  }

  return { width, height: 0, primitives, labels };
}

function buildConfluence(
  world: World,
  lane: Lane,
  width: number,
  seconds: number,
  latencyMs: number,
  primitives: Primitive[],
  labels: Scene["labels"],
) {
  const confluence = world.entities.find((e) => e.name === "mixer.confluence");
  if (!confluence) return;
  const verdict = describeBeating(world.beating);
  labels.push({
    of: "mixer",
    // The verdict names the *control* to reach for, which no shape can. A DJ
    // who has not learned the vocabulary can still read "nudge back" and act.
    text: [confluence.reading, verdict, world.confluence === "Seam" ? "keys clash" : ""]
      .filter(Boolean)
      .join("  ·  "),
    x: 8,
    y: lane.y + 4,
  });

  const meet = width * confluence.along;
  const mid = lane.y + lane.height * 0.5;
  const banks = confluenceBanks(world);

  banks.forEach((bank, index) => {
    if (!bank) return;
    const fromTop = index === 0;
    // Clear of the reading, which sits at the top. Text is trunk and stays
    // legible; the water gives way to it, not the other way round.
    const startY = lane.y + lane.height * (fromTop ? 0.24 : 0.9);
    // How much of this side survives the crossfader.
    const survives = fromTop ? 1 - confluence.along : confluence.along;
    primitives.push({
      kind: "stream",
      x: 0,
      y: startY,
      w: meet,
      h: Math.abs(mid - startY),
      toX: meet,
      toY: mid,
      thickness: Math.max(1, lane.height * 0.18 * bank.extent * (0.25 + survives * 0.75)),
      paint: of(bank, 0.8),
    });
  });

  // Downstream of the meeting point is the merge, drawn once. Letting each bank
  // run to the right edge painted the same stretch twice, so whichever was
  // drawn last won — which said the crossfader had no effect at all.
  primitives.push({
    kind: "bar",
    x: meet,
    y: mid - Math.max(0.5, lane.height * 0.1 * confluence.extent),
    w: width - meet,
    h: Math.max(1, lane.height * 0.2 * confluence.extent),
    paint: of(confluence, 0.9),
  });

  // Keys that will not mix: a seam, which is behaviour rather than colour —
  // hue already means key, and one man in twelve could not read a colour-coded
  // warning anyway.
  if (world.confluence === "Seam") {
    primitives.push({
      kind: "bar",
      x: meet,
      y: mid,
      w: width - meet,
      h: 1.5,
      paint: { hue: 40, saturation: 0.6, lightness: 0.85, alpha: 0.95 },
      dashed: true,
    });
  }

  // The crests arriving from each side. Two marks at the same x means two decks
  // in time; a slide is two marks visibly walking apart.
  const run = Math.max(8, meet);
  banks.forEach((bank, index) => {
    if (!bank || bank.vitality.depth <= 0.001 || !(bank.vitality.pulse_bpm > 0)) return;
    const phase = phaseAt(bank.vitality, seconds, latencyMs);
    const y = lane.y + lane.height * (index === 0 ? 0.36 : 0.78);
    primitives.push({
      kind: "mark",
      x: run * phase - 3,
      y: y - 3,
      w: 6,
      h: 6,
      paint: structural(1, 0.85),
      shape: "disc",
    });
  });

  // The estuary's banks: fixed, and the water squeezed through them.
  if (confluence.extent < 0.999) {
    const squeeze = (1 - confluence.extent) * lane.height * 0.3;
    primitives.push({
      kind: "bar",
      x: width - 3,
      y: mid - squeeze,
      w: 3,
      h: squeeze * 2,
      paint: { hue: 20, saturation: 0.4, lightness: 0.7, alpha: 0.5 },
    });
  }
}

/**
 * What the beating says, in words.
 *
 * Three states because they are three different actions, and the words say
 * which: nothing, a nudge (with its direction), or the pitch fader.
 */
export function describeBeating(beating: World["beating"]): string {
  if (beating === "Unknown") return "";
  if (beating === "Locked") return "locked";
  if ("Offset" in beating) {
    const beats = beating.Offset.beats;
    return `${beats > 0 ? "nudge back" : "nudge forward"} ${Math.abs(beats).toFixed(2)} beat`;
  }
  const difference = beating.Sliding.bpm_difference;
  return `sliding ${difference > 0 ? "+" : ""}${difference.toFixed(1)} BPM`;
}

/** The two rivers meeting, left bank first. */
export function confluenceBanks(world: World): [Entity | null, Entity | null] {
  const rivers = world.entities.filter((e) => e.name === "deck.river");
  // The world already decided which pair meets — it is in the confluence's own
  // reading — so this reads the banks the same way rather than guessing.
  return [rivers[0] ?? null, rivers[1] ?? null];
}

function pick(world: World, name: string, deck: number): Entity | undefined {
  return world.entities.find((e) => e.name === name && e.index === deck);
}

function all(world: World, name: string, deck: number): Entity[] {
  return world.entities.filter((e) => e.name === name && e.index === deck);
}

function holdsAlarm(world: World, deck: number): boolean {
  const alarm = world.alarm;
  if (alarm == null) return false;
  if (typeof alarm === "string") return true;
  const which = "RunningOut" in alarm ? alarm.RunningOut.deck : alarm.EndingSoon.deck;
  return which === deck;
}
