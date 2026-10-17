/**
 * Everything the website says djmanzo does, and where it says so.
 *
 * The owner's rule (DIRECTIVE §133): no page, guide or article may say
 * anything wrong or outdated about the application. So nothing on the site
 * — a stage, the box office, the guide, a blog draft — describes the
 * application from memory: it reads this list. Each entry names the module
 * that carries it (`editions.json`, from `dj_core::edition`), whether it is
 * shipped, part-built or planned, and the document that says so.
 *
 * `CHECKED_AGAINST` is the commit of `main` the list was last read against.
 * `npm run facts:drift` prints every change to those documents since then;
 * read it, correct this file, and move the commit forward before a deploy.
 *
 * Statuses are the documents' own. Where FEATURES §2 ("Beyond VirtualDJ")
 * speaks in the present tense about something §4 ("Outlook") plans — Pro DJ
 * Link, StagelinQ, Art-Net — the plan is what is true, and this list says
 * planned.
 */

import type { Module } from "./catalog";

export const CHECKED_AGAINST = "18c34467c6ddd656b662ad92134d42cbd05b9da9";

/** The documents a fact can come from, as `git diff` names them. */
export const SOURCES = ["README.md", "docs/FEATURES.md", "docs/EDITIONS.md", "docs/MARKETPLACE.md", "docs/VIDEO.md", "docs/KARAOKE-NIGHT.md"] as const;

export type Status = "shipped" | "part" | "planned";

/** Where on the Signal Path a fact is told. */
export type Stage = "groove" | "needle" | "cue" | "confluence" | "voice" | "amp" | "cone" | "floor" | "afterglow" | "outboard";

export interface Fact {
  id: string;
  name: string;
  /** One or two sentences a DJ would understand, no further than the source goes. */
  about: string;
  /** The module that carries it, or the foundation every edition has. */
  module: Module | "foundation";
  status: Status;
  /** What the source itself says has not been proved. Shown wherever the fact is. */
  caveat?: string;
  stage: Stage;
  /** Document and section. */
  source: string;
}

export const FACTS: Fact[] = [
  // --- The foundation: every edition -------------------------------------------
  { id: "decks", name: "Two decks and the mixer", about: "Play, cue, sync, keylock, pitch ranges up to ±100 %, channel faders, a crossfader with its curve, three-band EQ with kills, a filter, gain from loudness analysis, headphone cue, booth out and a limiter.", module: "foundation", status: "shipped", stage: "confluence", source: "FEATURES §1 Decks, Mixer" },
  { id: "transport", name: "Slip, reverse, brake and backspin", about: "The performance moves on the deck itself, with quantize and beat jump.", module: "foundation", status: "shipped", stage: "confluence", source: "FEATURES §1 Decks" },
  { id: "pads", name: "Cues, loops and pads", about: "Named, coloured hot cues, loops by beat count, loop rolls down to 1/16, a slicer, and pad pages for cues, loops, rolls, the sampler and effects.", module: "foundation", status: "shipped", stage: "confluence", source: "FEATURES §1 Cues, loops, pads" },
  { id: "fx", name: "The effects rack", about: "Echo, delay, reverb, gate, crush, flanger, phaser and filter — beat-synced, three slots per deck and on the master, with chain presets.", module: "foundation", status: "shipped", stage: "confluence", source: "FEATURES §1 Effects" },
  { id: "sampler", name: "The sampler", about: "Banks of pads — one-shot, loop, hold, stutter — synced to the master tempo, and recorded from a deck.", module: "foundation", status: "shipped", stage: "confluence", source: "FEATURES §1 Sampler" },
  { id: "record", name: "Recording the master", about: "The set to disk, as a WAV.", module: "foundation", status: "shipped", stage: "cone", source: "FEATURES §1 Output" },
  { id: "library", name: "The library", about: "Folders, playlists, crates and smart folders; imports from rekordbox, Serato, Traktor and iTunes with their cues and grids; duplicates found; instant search; the collection as cards with their own artwork.", module: "foundation", status: "shipped", stage: "groove", source: "FEATURES §1 Browser and library" },
  { id: "camelot", name: "Key and tempo, read", about: "BPM, key on the Camelot wheel with compatible keys, loudness, and 31 genre families from bachata to techno.", module: "foundation", status: "shipped", stage: "needle", source: "FEATURES §1 Browser; §1 Genre families" },
  { id: "waveforms", name: "Waveforms drawn in Rust", about: "Scrolling and overview waveforms with an editable beat grid, cue and loop overlays and colour by band, rendered in Rust so they stay fast on Linux.", module: "foundation", status: "shipped", stage: "needle", source: "FEATURES §1 Waveforms; README Stack" },
  { id: "phrases", name: "Phrases", about: "Phrase length and anchor found in the record, marked on the waveform, with phrase-locked loops and jumps.", module: "foundation", status: "shipped", caveat: "Verified against synthetic tracks, not yet measured against a corpus of real records.", stage: "needle", source: "FEATURES §1 Phrase detection" },
  { id: "find", name: "Find it from memory", about: "A half-remembered line, a description, or a hum.", module: "foundation", status: "shipped", stage: "groove", source: "FEATURES §1 Find it from memory" },
  { id: "keyboard", name: "The keyboard is a controller", about: "The same vocabulary and file format as a MIDI mapping, so a laptop with nothing plugged in is a playable instrument.", module: "foundation", status: "shipped", stage: "confluence", source: "README For the hands" },
  { id: "mapped", name: "A controller on its shipped mapping", about: "Pioneer DDJ-200, DDJ-SR, CDJ-3000 in HID mode, and a generic two-deck mapping.", module: "foundation", status: "shipped", stage: "confluence", source: "README For the hands; EDITIONS foundation" },
  { id: "themes", name: "Themes and skins", about: "Palettes, among them four Caribbean themes, with artwork layers over them; panels moved, zoomed and popped out into windows of their own.", module: "foundation", status: "shipped", stage: "cone", source: "README For the night; DIRECTIVE-STATUS §127, §129" },
  { id: "layouts", name: "Layouts", about: "Starter, Essentials, Pro and Performance, and a DJ's own as a file that cannot run code.", module: "foundation", status: "shipped", stage: "confluence", source: "FEATURES §3" },
  { id: "requests", name: "Requests from the room", about: "The room asks for songs from a QR code — no app to install — and the DJ reads the list most-wanted first.", module: "foundation", status: "shipped", stage: "floor", source: "FEATURES §1 Audience requests" },
  { id: "share", name: "Share a set", about: "Tonight's tracklist handed to WhatsApp, X, Bluesky or Threads with the message written; the DJ presses send.", module: "streaming", status: "shipped", stage: "afterglow", source: "FEATURES §1 Share a set; EDITIONS module table (Going live)" },
  { id: "linux", name: "Linux, properly", about: "PipeWire and JACK supported as themselves; macOS and Linux are the promised platforms.", module: "foundation", status: "shipped", stage: "afterglow", source: "README Under it" },

  // --- Modules -----------------------------------------------------------------
  { id: "decks6", name: "Four to six decks", about: "Decks three and four, and up to six.", module: "four_decks", status: "shipped", stage: "confluence", source: "FEATURES §1 Decks" },
  { id: "stems", name: "Live stems", about: "Vocals, drums, bass and the rest apart, separated ahead of the playhead into a cache, so the audio adds no latency; EQ per stem, and one deck's vocal over another's mix.", module: "stems", status: "shipped", stage: "needle", source: "README; FEATURES §1 Effects, Hardware" },
  { id: "automix", name: "Automix and the autopilot", about: "The application runs the mix from an assembled set, in six postures, and gives way the moment a hand touches a control.", module: "autopilot", status: "shipped", caveat: "Never verified against real audio: the development machine has no output device.", stage: "confluence", source: "FEATURES §1 Autopilot, Assistant takeover" },
  { id: "autopilot-surface", name: "The Auto-Pilot's own surface", about: "Style the night — genres, energy over time, tempo, era — fill the playlist, start it, take over.", module: "autopilot", status: "planned", stage: "confluence", source: "EDITIONS What is built" },
  { id: "suggest", name: "What to play next, and why", about: "Next-track suggestions ranked by key, tempo, loudness and phrase, each with its reasons; a transition planner that proposes where and how.", module: "assistant", status: "shipped", caveat: "Energy is approximated by loudness.", stage: "cue", source: "FEATURES §1 Next-track suggestions, Transition planner" },
  { id: "asks", name: "It asks before it acts", about: "The assistant stages a whole transition as one thing to accept, modify or reject, and speaks only in actions a DJ could have pressed.", module: "assistant", status: "shipped", stage: "cue", source: "FEATURES §1 Interface; README What makes it different" },
  { id: "packs", name: "Knowledge packs and the set plan", about: "A genre's or a night's shape for the assistant; the night planned as an arc, steered without rebuilding.", module: "assistant", status: "shipped", stage: "cue", source: "README For the night; FEATURES §1 Set planning workspace" },
  { id: "teacher", name: "The teacher", about: "Guides, the practice lab and a practice deck only the headphones hear, the coach that names what the hands just did, twenty-eight techniques and a quiz.", module: "teacher", status: "shipped", stage: "cue", source: "EDITIONS What is built; FEATURES §1 Learning module, DJ technique catalogue" },
  { id: "curriculum", name: "The Apprentice curriculum", about: "From the first beatmatch to the first gig, with exercises the teacher checks and a record of progress.", module: "teacher", status: "planned", stage: "cue", source: "EDITIONS What is built" },
  { id: "karaoke", name: "A karaoke night", about: "The singers' screen with words in time, the queue and a sign-up page, vocals out, a microphone and effects per singer, duets, a guide voice, break music, and finding the song a singer cannot name.", module: "karaoke", status: "shipped", stage: "voice", source: "README For the night; FEATURES §1 Karaoke" },
  { id: "oldmac", name: "Karaoke on older Macs", about: "A build for Macs back to macOS 10.13.", module: "karaoke", status: "shipped", stage: "voice", source: "README For the night" },
  { id: "room", name: "Reading the room", about: "A camera and a microphone say whether the floor is stiller or busier than earlier tonight; the crowd's reactions from their phones; what each mix did — each stated with how sure it is.", module: "crowd", status: "shipped", caveat: "No image is recorded, saved or sent.", stage: "floor", source: "README For the night; FEATURES §1 Reading the room" },
  { id: "mapping", name: "Map any controller", about: "Press a control, choose what it does, save; HID devices, lights, Lua for what a table cannot say, and an interface that reads what is plugged in.", module: "controllers", status: "shipped", stage: "confluence", source: "FEATURES §1 Hardware" },
  { id: "platters", name: "Motorised platters", about: "Absolute platter position with the motor driven by the transport.", module: "controllers", status: "shipped", stage: "confluence", source: "FEATURES §1 Hardware" },
  { id: "dvs", name: "Timecode vinyl", about: "Real turntables drive a deck, relative or absolute; djmanzo writes its own control signal, so no licensed record is needed.", module: "dvs", status: "shipped", caveat: "Not yet run against a pressed record.", stage: "confluence", source: "FEATURES §1 Timecode vinyl / DVS" },
  { id: "visuals", name: "Video and visuals", about: "Video on a deck, layered WebGL looks, MilkDrop presets and cameras.", module: "video", status: "shipped", stage: "cone", source: "README For the night; VIDEO.md" },
  { id: "live", name: "Going live", about: "Now playing on a stream: an overlay for OBS and a text file, for Twitch, YouTube, Instagram, TikTok or Kick.", module: "streaming", status: "shipped", stage: "cone", source: "FEATURES §1 Now playing on a live stream" },
  { id: "outputs", name: "Stems and decks out", about: "One deck as four stereo pairs, or each deck on its own pair, for outboard processing.", module: "stems", status: "shipped", stage: "cone", source: "FEATURES §1 Output" },
  { id: "clap", name: "CLAP plugins", about: "Audio plugins hosted on the master.", module: "plugins", status: "shipped", stage: "confluence", source: "FEATURES §1 Effects" },
  { id: "sources", name: "Online sources", about: "Search Spotify, YouTube, Jamendo and the Internet Archive, and match a result to a file already owned. For finding and planning: mixing Spotify or YouTube Music audio is not allowed by them.", module: "sources", status: "shipped", stage: "groove", source: "FEATURES §1 Browser; Deliberately not planned" },
  { id: "events", name: "Events and the press kit", about: "A booking kept step by step, the night's plan, and a press kit.", module: "events", status: "shipped", stage: "afterglow", source: "README For the night" },
  { id: "replay", name: "Replay", about: "A set is data: heard back, rendered again offline faster than real time, a transition looped to practise, two takes compared.", module: "replay", status: "shipped", caveat: "Live inputs — microphone, aux, timecode — cannot be reproduced and are said to be.", stage: "cone", source: "FEATURES §1 Deterministic replay; §2" },
  { id: "api", name: "An open surface", about: "A WebSocket and OSC control API in the same actions the interface uses, MIDI clock, and tempo sync between djmanzo machines.", module: "controllers", status: "shipped", stage: "confluence", source: "README Under it; FEATURES §1 Output" },

  // --- Planned, and said so -------------------------------------------------------
  { id: "editions", name: "Seven editions, one download", about: "Auto-Pilot, Hobbyist, Apprentice, Karaoke, Pro DJ, Full and Custom, all one application, unlocked by signing in.", module: "foundation", status: "part", caveat: "Nothing is locked in the beta: every edition's djmanzo opens every feature for now, and the licence says what was bought.", stage: "amp", source: "EDITIONS; ADR-0011" },
  { id: "trial", name: "Seven days of any edition", about: "A free trial of any edition, Full included, once per account.", module: "foundation", status: "part", caveat: "Started on the website; the application shows the days left in Settings → Account.", stage: "amp", source: "EDITIONS The 7-day trial" },
  { id: "marketplace", name: "The marketplace", about: "Themes, skins, visuals, controller mappings, knowledge packs, lessons, karaoke screens, plugins and DJ techniques the autopilot can perform — everything in it free.", module: "foundation", status: "planned", stage: "outboard", source: "MARKETPLACE; ADR-0012" },
  { id: "hardware", name: "More hardware", about: "Mappings for the controllers people own — DDJ-FLX4, DDJ-400, Mixtrack, Inpulse, Kontrol S2 and S4, Denon, Rane, Reloop — and phones as remotes.", module: "controllers", status: "planned", stage: "confluence", source: "FEATURES §4 More hardware" },
  { id: "club", name: "Club players and lighting", about: "Pro DJ Link, StagelinQ, Ableton Link, Art-Net and DMX, MIDI 2.0.", module: "controllers", status: "planned", caveat: "None of these is proved until it meets the real device.", stage: "confluence", source: "FEATURES §4 More hardware" },
  { id: "windows", name: "Windows", about: "Built, tested in CI and packaged with every release.", module: "foundation", status: "part", caveat: "Never yet run by a person on a Windows machine.", stage: "afterglow", source: "FEATURES §4 Platforms" },
  { id: "gig", name: "A real gig", about: "Everything here has been tested in the repository; none of it in front of a crowd yet.", module: "foundation", status: "planned", stage: "floor", source: "README What's coming" },
];

export const FACT_BY_ID: Record<string, Fact> = Object.fromEntries(FACTS.map((f) => [f.id, f]));

/** The one sentence every page that sells djmanzo must be able to say. */
export const STATUS_LINE =
  "djmanzo is in beta: a complete DJ application for macOS and Linux that has been tested thoroughly and has not yet played a real gig.";

/** What an edition carries, as facts: the foundation's and its modules'. */
export function factsFor(modules: readonly Module[]): Fact[] {
  return FACTS.filter((f) => f.module === "foundation" || modules.includes(f.module));
}
