/**
 * The places on Isla Manzo.
 *
 * The island follows djmanzo's own metaphor (docs/VISUAL-LANGUAGE.md): a
 * watershed. Records wait in the highlands like springs; the river runs down
 * through the booth, where the decks meet the way rivers meet at a
 * confluence; it reaches the sea at the harbour, and the sea is the room.
 * Every place sits where that metaphor puts it, so a visitor who learns the
 * island has learned how djmanzo thinks.
 */

export type PlaceId =
  | "muelle"
  | "cabina"
  | "tienda"
  | "tarima"
  | "faro"
  | "taquilla"
  | "muro"
  | "radio"
  | "arena"
  | "gigs"
  | "plaza"
  | "pase"
  | "backstage";

export interface Place {
  id: PlaceId;
  /** Its name on the island's signs. */
  name: string;
  /** What it is, for someone who does not read the sign's Spanish. */
  what: string;
  /** One line on the map. */
  line: string;
  x: number;
  y: number;
  emoji: string;
  /** Hot-cue pad number, 1–8, for the eight that have a pad. */
  pad?: number;
  /** Counts toward the island's prize when visited. */
  stamp: boolean;
  hidden?: boolean;
}

export const PLACES: Place[] = [
  { id: "muelle", name: "El Muelle", what: "The dock — start here", line: "Where every visitor lands. The MC is waiting.", x: 40, y: 640, emoji: "⚓", pad: 1, stamp: true },
  { id: "cabina", name: "La Cabina", what: "The booth — try two decks", line: "Two decks, a mixer and the effects. Mix two genres yourself.", x: 0, y: 0, emoji: "🎚️", pad: 2, stamp: true },
  { id: "tienda", name: "Tienda de Discos", what: "The record shop — your library", line: "Up in the hills, where records wait like springs.", x: -300, y: -540, emoji: "💿", pad: 3, stamp: true },
  { id: "tarima", name: "La Tarima", what: "The karaoke stage", line: "On the beach. The singers' screen, the voice knob, the queue.", x: 780, y: 300, emoji: "🎤", pad: 4, stamp: true },
  { id: "faro", name: "El Faro", what: "The lighthouse — visuals", line: "Light for the room, drawn by the music.", x: 900, y: -420, emoji: "🗼", pad: 5, stamp: true },
  { id: "taquilla", name: "La Taquilla", what: "The box office — editions and prices", line: "At the harbour. Four editions, three ways to pay for them.", x: -640, y: 420, emoji: "🎟️", pad: 6, stamp: true },
  { id: "muro", name: "El Muro", what: "The wall — the wishlist", line: "Paste a wish. See which ones became features.", x: -900, y: -40, emoji: "🧱", pad: 7, stamp: true },
  { id: "radio", name: "Radio Manzo", what: "The radio tower — the blog", line: "Stories from the booth, in many languages.", x: -760, y: -500, emoji: "📻", pad: 8, stamp: true },
  { id: "arena", name: "La Arena", what: "The arena — honest comparisons", line: "djmanzo next to the others, strengths and gaps.", x: 520, y: -240, emoji: "🥊", stamp: true },
  { id: "gigs", name: "El Tablero", what: "The gig board — which edition for which night", line: "Weddings, clubs, karaoke bars, bedrooms: what each needs.", x: 300, y: 430, emoji: "📌", stamp: true },
  { id: "plaza", name: "La Plaza", what: "The plaza — people, mixes, videos", line: "What DJs and singers say, and what they made.", x: -330, y: 170, emoji: "⛲", stamp: true },
  { id: "pase", name: "Tu Pase", what: "Your pass — sign-in, licences, link djmanzo", line: "Your account, your licences, and the code for the app.", x: -230, y: 660, emoji: "🪪", stamp: false },
  { id: "backstage", name: "Backstage", what: "For the crew", line: "Codes, wishes, posts, orders.", x: 130, y: -150, emoji: "🔧", stamp: false, hidden: true },
];

export const PLACE_BY_ID = Object.fromEntries(PLACES.map((p) => [p.id, p])) as Record<PlaceId, Place>;

/** The river's course, from the record shop's springs to the sea at the dock. */
export const RIVER: [number, number][] = [
  [-300, -540],
  [-220, -380],
  [-120, -250],
  [-40, -120],
  [0, 0],
  [30, 160],
  [10, 330],
  [40, 520],
  [40, 760],
];

/** The second river, from the radio hill, joining at the booth: two decks, one confluence. */
export const RIVER_B: [number, number][] = [
  [-760, -500],
  [-560, -360],
  [-380, -200],
  [-180, -70],
  [0, 0],
];
