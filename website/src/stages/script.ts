/**
 * The Signal Path, stage by stage: what each game screen says.
 *
 * Every screen has four beats (WEBSITE.md): a **mystery** — one abstract
 * line over a painting — a **reveal** — the real application doing it, and
 * one honest fact — the **play**, which is art and never a pretend
 * djmanzo, and the **pass**, where to go next. The facts a reveal may show
 * are named by id from `shared/facts.ts`, so a stage cannot say what the
 * application does in words of its own.
 */

import type { StageId } from "../style/genome/types";

export interface Stage {
  id: StageId;
  /** The stage's name on the map. */
  name: string;
  /** Its place in the night (One Night): sets the light. */
  night: "doors" | "warm-up" | "build" | "peak" | "release" | "sunrise";
  /** The mystery: one line, abstract, said over the painting before anything is explained. */
  mystery: string;
  /** The reveal's punchline: what the mystery was about, in one sentence. */
  reveal: string;
  /** Facts the reveal shows, by id, in order. */
  facts: string[];
  /** The screens from `docs/screenshots/` that show it (the `<screen>` part of the file name). */
  screens: string[];
  /** The play: what the visitor's hands do here. Art, not the app. */
  play: string;
  /** Where the signal goes next. */
  next: StageId | null;
}

export const STAGES: Stage[] = [
  {
    id: "silence",
    name: "Silence",
    night: "doors",
    mystery: "Before the first note, there is a room holding its breath.",
    reveal: "djmanzo begins with you: tell it what you play, and it dresses the night in it.",
    facts: [],
    screens: [],
    play: "Answer the guide; with each answer the needle drops a little closer to the record.",
    next: "groove",
  },
  {
    id: "groove",
    name: "Groove",
    night: "warm-up",
    mystery: "Every record is a landscape someone carved by ear.",
    reveal: "Your collection, read like a map: every record's tempo, key and phrases, and the ones you half remember found by a line or a hum.",
    facts: ["library", "find", "sources"],
    screens: ["library", "browser", "find"],
    play: "Drag the needle through the groove and travel your music's lineage: the eras, the rooms, the dances change under it.",
    next: "needle",
  },
  {
    id: "needle",
    name: "Needle",
    night: "warm-up",
    mystery: "A diamond reads a mountain range one millimetre at a time.",
    reveal: "djmanzo reads the music before you play it: the waveform, the beat grid, the phrases, the key — and the voice, drums, bass and the rest apart.",
    facts: ["waveforms", "camelot", "phrases", "stems"],
    screens: ["waveform", "stems", "cockpit"],
    play: "Touch the painting and the loop comes apart into four currents — the voice, the drums, the bass, the rest.",
    next: "cue",
  },
  {
    id: "cue",
    name: "Cue",
    night: "warm-up",
    mystery: "The most important music of the night is the music no one else hears.",
    reveal: "In your headphones, djmanzo prepares with you: what to play next and why, a practice deck only you hear, and a coach for the moves you are learning.",
    facts: ["suggest", "asks", "packs", "teacher", "curriculum"],
    screens: ["next", "prepare", "practice"],
    play: "Lean in: the room goes quiet and you hear the next record alone.",
    next: "confluence",
  },
  {
    id: "confluence",
    name: "Confluence",
    night: "build",
    mystery: "Two rivers never meet by accident.",
    reveal: "Two decks — or six — meet at the mixer: sync, keylock, the effects, the sampler, your keyboard or your controller, and an autopilot that gives way to your hand.",
    facts: ["decks", "pads", "fx", "sampler", "keyboard", "mapped", "decks6", "mapping", "dvs", "automix", "autopilot-surface", "replay"],
    screens: ["cockpit", "mixer", "fx", "pads"],
    play: "Two of the night's loops run as two streams of paint; where your pointer lets them, they meet.",
    next: "voice",
  },
  {
    id: "voice",
    name: "Voice",
    night: "build",
    mystery: "Every signal path has a second input: the room itself.",
    reveal: "A karaoke night in the same booth: the singers' screen with words in time, the queue, the voice taken out of a record you own, a microphone for each singer.",
    facts: ["karaoke", "oldmac", "requests"],
    screens: ["karaoke-singers", "karaoke-queue", "karaoke"],
    play: "Allow the microphone, and your voice paints the scene. Nothing is recorded or sent.",
    next: "amp",
  },
  {
    id: "amp",
    name: "Amp",
    night: "peak",
    mystery: "How much power does your night need?",
    reveal: "Seven editions of one application — from Hobbyist to Full, or your own from modules — at half what the professionals charge, and seven days of any of them free.",
    facts: ["editions", "trial"],
    screens: [],
    play: "Turn the dial: each edition fills the room a little more.",
    next: "cone",
  },
  {
    id: "cone",
    name: "Cone & air",
    night: "peak",
    mystery: "Sound is just air, moving in the right order.",
    reveal: "What the room sees and hears out of it: visuals and video, the set recorded, going live to a stream, each deck or stem out to outboard gear.",
    facts: ["visuals", "live", "record", "outputs", "themes", "replay"],
    screens: ["visuals", "video", "going-live"],
    play: "The cone breathes with the music; push the air with your pointer and the light follows.",
    next: "floor",
  },
  {
    id: "floor",
    name: "Floor",
    night: "release",
    mystery: "The last stage of the signal is a body that cannot keep still.",
    reveal: "djmanzo listens back: the floor by microphone and camera, the room's requests, and what each mix did — and it has still to play its first real gig.",
    facts: ["room", "requests", "gig"],
    screens: ["room", "requests"],
    play: "The dancers of your music move on the floor; your cursor joins them.",
    next: "afterglow",
  },
  {
    id: "afterglow",
    name: "Afterglow",
    night: "sunrise",
    mystery: "Zoom out.",
    reveal: "The landscape was a DJ's setup all along — and the path you took through it is yours to keep.",
    facts: ["share", "events", "linux", "windows"],
    screens: ["account"],
    play: "Your path through the night becomes a constellation: a poster to keep or share. Sign in, and plug djmanzo in.",
    next: null,
  },
];

export const STAGE_BY_ID = Object.fromEntries(STAGES.map((s) => [s.id, s])) as Record<StageId, Stage>;
