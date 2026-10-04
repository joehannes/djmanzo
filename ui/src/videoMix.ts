/**
 * §122: which deck's picture is seen, how much, and where in it — the Video
 * panel's arithmetic, kept apart from the panel so it can be held by tests.
 * See `docs/VIDEO.md`.
 */

/** What the arithmetic needs of a deck: the snapshot's own fields. */
export interface DeckForPicture {
  number: number;
  playing: boolean;
  position_seconds: number;
  rate: number;
  volume: number;
  crossfader_assign: "left" | "right" | "thru";
  reversed: boolean;
}

/**
 * How much of what is heard a deck is: its fader, and the crossfader on its
 * side. The crossfader is taken by its travel, linearly — a picture dissolves
 * as a video mixer's does, whatever curve the sound is on.
 */
export function share(deck: DeckForPicture, crossfader: number): number {
  const x = (Math.min(1, Math.max(-1, crossfader)) + 1) / 2;
  const side = deck.crossfader_assign === "left" ? 1 - x : deck.crossfader_assign === "right" ? x : 1;
  return Math.max(0, deck.volume) * side;
}

/** One picture in the stack, from the bottom. */
export interface Layer {
  deck: number;
  opacity: number;
}

export interface Stack {
  /** The opacity of the whole stack over black: how much of what is heard has a picture. */
  whole: number;
  layers: Layer[];
}

/**
 * The pictures to draw and how strongly, for the mix as it is heard.
 *
 * Each deck's picture is seen in the share of the sound it has, over black:
 * two pictures at half each are half and half, and a picture fades to black as
 * a record without one takes over. Drawn as a stack — the bottom layer whole,
 * each one above it at its share of what is drawn so far, the whole stack at
 * how much of the sound has a picture — which is what `over` compositing needs
 * to come out as that weighted sum.
 *
 * `pinned` shows one deck's picture alone, whatever the faders say.
 */
export function stack(
  decks: DeckForPicture[],
  pictures: number[],
  crossfader: number,
  pinned: number | null = null,
): Stack {
  if (pinned !== null) {
    return pictures.includes(pinned)
      ? { whole: 1, layers: [{ deck: pinned, opacity: 1 }] }
      : { whole: 0, layers: [] };
  }
  const heard = decks.map((d) => ({ deck: d.number, share: d.playing ? share(d, crossfader) : 0 }));
  const total = heard.reduce((sum, d) => sum + d.share, 0);
  const seen = heard
    .filter((d) => pictures.includes(d.deck) && d.share > 0)
    // The strongest at the bottom: if anything is lost to rounding, it is the
    // weakest picture's.
    .sort((a, b) => b.share - a.share);
  if (total <= 0 || seen.length === 0) return { whole: 0, layers: [] };
  let below = 0;
  const layers = seen.map((d) => {
    below += d.share;
    return { deck: d.deck, opacity: d.share / below };
  });
  return { whole: below / total, layers };
}

/** What a picture's element is told to do, to keep to its deck. */
export interface Follow {
  /** Play, or hold still. */
  play: boolean;
  /** Its rate, when playing. */
  rate: number;
  /** Where to jump to, when it has drifted too far; otherwise null. */
  seek: number | null;
}

/** How far a playing picture may drift from its deck before it is moved: a few frames. */
export const DRIFT_PLAYING = 0.12;
/** How far a still one may: a frame. */
export const DRIFT_STILL = 0.04;

/** The slowest and fastest a media element will play. */
const SLOWEST = 0.25;
const FASTEST = 4;

/**
 * Keep a picture at `current` seconds to its deck.
 *
 * Playing forwards, the element plays at the deck's rate and is moved only
 * when it has drifted more than [`DRIFT_PLAYING`] — a seek costs a stutter,
 * so small drift is left to the rate. Playing backwards or out of the range
 * an element can play, it holds still and is moved to the deck, which is the
 * nearest a picture can come to a scratch.
 */
export function follow(deck: DeckForPicture, current: number): Follow {
  const target = Math.max(0, deck.position_seconds);
  const drift = Math.abs(current - target);
  const playable = deck.playing && !deck.reversed && deck.rate >= SLOWEST && deck.rate <= FASTEST;
  if (playable) {
    return { play: true, rate: deck.rate, seek: drift > DRIFT_PLAYING ? target : null };
  }
  return { play: false, rate: 1, seek: drift > DRIFT_STILL ? target : null };
}
