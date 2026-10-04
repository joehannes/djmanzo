/**
 * §122: a look's layers, checked before they are drawn, and the numbers they
 * answer to — kept apart from the WebGL so they can be held by tests. See
 * `docs/VISUALS.md`.
 *
 * The looks themselves are Rust's (`dj_app::looks`): data, so the DJ's AI
 * provider can be told them by name, and installed packs can add to them.
 * What is here is what only the interface can decide: whether a layer is one
 * it can draw, and what each signal reads right now.
 */
import type { AudioMetrics, Look, LookLayer } from "./api";
import { share, type DeckForPicture } from "./videoMix";

/**
 * What a layer can draw. `dj_app::looks`' test holds the same list.
 *
 * `milkdrop` is the look's *main* layer: Butterchurn drawing a MilkDrop
 * preset (`./milkdrop.ts`), taken into the stack as a picture.
 */
export const KINDS = ["wash", "pulse", "ripple", "sparks", "halo", "milkdrop"] as const;
/** What a layer can answer to. */
export const SIGNALS = [
  "loudness",
  "band.bass",
  "band.low_mid",
  "band.high_mid",
  "band.treble",
  "beat",
  "stem.vocal",
  "stem.drums",
  "stem.bass",
  "stem.other",
  "voice",
] as const;
/** How a layer lies over what is below it. */
export const BLENDS = ["over", "add", "screen"] as const;

export type Kind = (typeof KINDS)[number];
export type Signal = (typeof SIGNALS)[number];
export type Blend = (typeof BLENDS)[number];
export type Rgb = [number, number, number];

/** A layer the stack can draw: every field checked. */
export interface DrawLayer {
  kind: Kind;
  signal: Signal;
  /** The record's colours, or one fixed colour (0..1 each). */
  colour: "record" | Rgb;
  amount: number;
  blend: Blend;
  /** The MilkDrop preset a `milkdrop` layer draws. */
  preset?: string;
}

/** `#rrggbb`, `#rgb` or `rgb(r g b)` / `rgb(r, g, b)`, as 0..1; null when it is none of them. */
export function parseColour(text: string): Rgb | null {
  const hex = /^#([0-9a-f]{3}|[0-9a-f]{6})$/i.exec(text.trim());
  if (hex) {
    const digits = hex[1].length === 3 ? [...hex[1]].map((d) => d + d) : hex[1].match(/../g)!;
    return digits.map((d) => parseInt(d, 16) / 255) as Rgb;
  }
  const rgb = /^rgb\(\s*(\d{1,3})[\s,]+(\d{1,3})[\s,]+(\d{1,3})\s*\)$/i.exec(text.trim());
  if (rgb) {
    const values = [rgb[1], rgb[2], rgb[3]].map((v) => Number(v));
    if (values.every((v) => v <= 255)) return values.map((v) => v / 255) as Rgb;
  }
  return null;
}

/**
 * A look's layers as the stack will draw them, and what was left out.
 *
 * A layer with a kind, signal, blend or colour this interface does not know is
 * **dropped, with its name said** — not guessed at, and not allowed to take
 * the rest of the look with it. The amount is held to 0..1.
 */
export function validate(look: Look): { layers: DrawLayer[]; dropped: string[] } {
  const layers: DrawLayer[] = [];
  const dropped: string[] = [];
  look.layers.forEach((layer: LookLayer, index) => {
    const which = `${look.name}, layer ${index + 1}`;
    if (!(KINDS as readonly string[]).includes(layer.kind)) {
      dropped.push(`${which}: no picture called "${layer.kind}"`);
      return;
    }
    if (!(SIGNALS as readonly string[]).includes(layer.signal)) {
      dropped.push(`${which}: nothing to answer to called "${layer.signal}"`);
      return;
    }
    if (!(BLENDS as readonly string[]).includes(layer.blend)) {
      dropped.push(`${which}: no blend called "${layer.blend}"`);
      return;
    }
    const colour = layer.colour === "record" ? "record" : parseColour(layer.colour);
    if (colour === null) {
      dropped.push(`${which}: "${layer.colour}" is not a colour`);
      return;
    }
    if (layer.kind === "milkdrop" && !layer.preset) {
      dropped.push(`${which}: a MilkDrop layer that names no preset`);
      return;
    }
    const amount = Number.isFinite(layer.amount) ? Math.min(1, Math.max(0, layer.amount)) : 0;
    layers.push({
      kind: layer.kind as Kind,
      signal: layer.signal as Signal,
      colour,
      amount,
      blend: layer.blend as Blend,
      ...(layer.kind === "milkdrop" ? { preset: layer.preset ?? undefined } : {}),
    });
  });
  return { layers, dropped };
}

/** What the signals need of a deck: the snapshot's own fields. */
export interface DeckForSignals extends DeckForPicture {
  stem_levels: [number, number, number, number];
  beat_phase: number;
  effective_bpm: number | null;
}

/** Everything the signals are read from, as the snapshot has it. */
export interface Hearing {
  decks: DeckForSignals[];
  crossfader: number;
  audio: AudioMetrics;
  /** Each singer's microphone level, 0..1. */
  voices: number[];
}

export type Signals = Record<Signal, number>;

export const SILENT: Signals = Object.fromEntries(SIGNALS.map((s) => [s, 0])) as Signals;

const clamp = (v: number) => (Number.isFinite(v) ? Math.min(1, Math.max(0, v)) : 0);

function heardDecks(hearing: Hearing) {
  return hearing.decks.map((deck) => ({
    deck,
    share: deck.playing ? share(deck, hearing.crossfader) : 0,
  }));
}

/** The most-heard deck with a grid: the one the room is in time with. */
function leadingDeck(heard: ReturnType<typeof heardDecks>) {
  return heard
    .filter(({ deck, share }) => share > 0 && (deck.effective_bpm ?? 0) > 0)
    .sort((a, b) => b.share - a.share)[0];
}

/** Where in the beat the room is, 0..1, from the most-heard deck with a grid; 0 with none. */
export function phase(hearing: Hearing): number {
  const leading = leadingDeck(heardDecks(hearing));
  return leading ? clamp(leading.deck.beat_phase) : 0;
}

/**
 * What each signal reads, 0..1, for the mix as it is heard.
 *
 * The bands and the loudness are the master bus's. A stem is each deck's
 * level of it **weighed by the deck's share of what is heard** — its fader
 * and its side of the crossfader, by the same rule the Video panel uses — so a
 * record being cued under a closed fader lights nothing. The beat is the
 * most-heard deck with a grid: 1 on the beat, falling away through it. The
 * voice is the loudest singer's microphone.
 */
export function signals(hearing: Hearing): Signals {
  const heard = heardDecks(hearing);
  const stem = (index: number) =>
    clamp(heard.reduce((sum, { deck, share }) => sum + share * (deck.stem_levels?.[index] ?? 0), 0));
  const leading = leadingDeck(heard);
  // A sharp attack on the beat and a fall through it: a kick, not a sine.
  const beat = leading ? Math.pow(1 - clamp(leading.deck.beat_phase), 3) * Math.min(1, leading.share) : 0;
  const [bass, lowMid, highMid, treble] = hearing.audio.bands;
  return {
    loudness: clamp(hearing.audio.loudness),
    "band.bass": clamp(bass),
    "band.low_mid": clamp(lowMid),
    "band.high_mid": clamp(highMid),
    "band.treble": clamp(treble),
    beat,
    "stem.vocal": stem(0),
    "stem.drums": stem(1),
    "stem.bass": stem(2),
    "stem.other": stem(3),
    voice: clamp(Math.max(0, ...hearing.voices)),
  };
}

/** How fast a light rises to a louder signal, and how slowly it lets go, in seconds. */
export const RISE = 0.03;
export const FALL = 0.25;

/**
 * Each signal followed as a light follows sound: up at once, down slowly.
 *
 * The snapshot's levels are peaks per block, which flicker; a layer drawn
 * straight from them strobes. `seconds` is the time since the last frame, so
 * a slow frame moves further and a projector at 30 frames a second looks the
 * same as a screen at 120. The beat is not followed: it has its own shape.
 */
export function follow(previous: Signals, target: Signals, seconds: number): Signals {
  const dt = Math.max(0, Math.min(0.25, seconds));
  const next = { ...target };
  for (const signal of SIGNALS) {
    if (signal === "beat") continue;
    const from = previous[signal];
    const to = target[signal];
    const tau = to > from ? RISE : FALL;
    next[signal] = from + (to - from) * (1 - Math.exp(-dt / tau));
  }
  return next;
}

/**
 * The record's colours as four, strongest first: repeated when it has fewer,
 * and djmanzo's own when it has none yet (§110's red, amber, teal and violet).
 */
export const FALLBACK: Rgb[] = [
  [0.85, 0.2, 0.25],
  [0.95, 0.65, 0.2],
  [0.15, 0.7, 0.65],
  [0.55, 0.3, 0.9],
];

export function palette(shades: { colour: string }[]): Rgb[] {
  const parsed = shades.map((s) => parseColour(s.colour)).filter((c): c is Rgb => c !== null);
  if (parsed.length === 0) return FALLBACK;
  return Array.from({ length: 4 }, (_, i) => parsed[i % parsed.length]);
}
