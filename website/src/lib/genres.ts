import type { Edition } from "../../shared/catalog";
/**
 * The genres the island knows, and what each looks, sounds, dresses and
 * dances like.
 *
 * Everything the island adapts when a visitor names their music comes from
 * here: the colours, the display face, the background motif, the loop the
 * booth plays, the effects it suggests, the dance it teaches, the theme and
 * skin djmanzo itself ships for that room (real names from the application's
 * `theme.rs` and `skins.ts`). A working DJ's map of each scene, not a
 * musicology: wrong at the edges, and written down so it can be corrected.
 */

export type Motif = "palms" | "waves" | "papel" | "halftone" | "grid" | "concrete" | "neon" | "disco" | "graffiti" | "plankton" | "sunburst" | "tape";

export interface Pattern {
  /** Steps per bar (16, or 32 for two-bar patterns like clave). */
  steps: number;
  kick: number[];
  snare: number[];
  hat: number[];
  /** A hand percussion voice: güira, guacharaca, shaker, bongó, conga, log drum. */
  perc: number[];
  percVoice: "guira" | "shaker" | "bongo" | "conga" | "log" | "rim" | "clave" | "cowbell";
  bass: number[];
  /** Semitone offsets from the root, one per bass hit, cycled. */
  bassNotes: number[];
  /** Semitone offsets for a chord, played on `chordSteps`. */
  chord: number[];
  chordSteps: number[];
  /** How the chord sounds: a picked requinto, a piano montuno, a pad, a stab. */
  chordVoice: "pluck" | "piano" | "pad" | "stab" | "organ";
  swing: number;
}

export interface Genre {
  id: string;
  name: string;
  /** Where it comes from, in a line. */
  roots: string;
  bpm: [number, number];
  /** Colours: background, surface, primary, secondary, ink. */
  palette: { bg: string; surface: string; primary: string; secondary: string; ink: string };
  /** A Google Fonts family for headings in this genre's world. */
  display: string;
  motif: Motif;
  /** The rooms it lives in. */
  rooms: string;
  /** What people wear there. */
  style: string;
  /** Its visual art: flyers, sleeves, signs. */
  art: string;
  /** Dances, each with a one-line how-to. */
  dances: { name: string; how: string }[];
  /** Effects a DJ in this genre reaches for, in djmanzo's own FX names. */
  fx: ("echo" | "reverb" | "filter" | "flanger" | "crush" | "gate")[];
  /** Mixing habits worth knowing. */
  mixing: string;
  /** djmanzo's own theme and skin for this room. */
  theme: string;
  /** A skin the app offers with that theme (`ui/src/skins/skins.ts` `suits`), or none. */
  skin: string | null;
  /** An emoji that reads as the scene. */
  emoji: string;
  /** The root note for the loop, in Hz. */
  root: number;
  scale: "minor" | "major" | "dorian";
  pattern: Pattern;
  /** Which edition a DJ in this scene most often needs first. */
  leaning: Edition;
}

const FOUR = [0, 4, 8, 12];

export const GENRES: Genre[] = [
  {
    id: "bachata",
    name: "Bachata",
    roots: "Dominican Republic — from the colmados and bars of the 1960s to Aventura and Romeo Santos.",
    bpm: [120, 140],
    palette: { bg: "#2a0f2e", surface: "#45193f", primary: "#ff6fa8", secondary: "#f6c453", ink: "#fff1f6" },
    display: "Shrikhand",
    motif: "palms",
    rooms: "Colmados with a speaker on the step, beach bars, Sunday afternoons in the yard, Bronx and Madrid social nights.",
    style: "Pressed guayabera or a fitted shirt, gold chain, a dress that moves on the turn; dancers in suede-soled shoes.",
    art: "Hand-painted colmado signs, roses and broken hearts, romantic portrait covers with gold script.",
    dances: [
      { name: "The basic", how: "Three steps side to side and a tap on four — the hip pops on the tap." },
      { name: "Bachata dominicana footwork", how: "Close embrace, quick syncopated steps that answer the bongó." },
      { name: "Sensual body roll", how: "A wave from the head down, led in the hold — a Spanish-born style, not the Dominican original." },
    ],
    fx: ["reverb", "echo", "filter"],
    mixing: "Mix on the phrase, never over the requinto's lead line; a short reverb tail on the last guitar note carries the room into the next song.",
    theme: "Bachata Rosa",
    skin: "Palm shadows",
    emoji: "🌹",
    root: 220,
    scale: "minor",
    pattern: { steps: 16, kick: [0, 8], snare: [], hat: [0, 2, 4, 6, 8, 10, 12, 14], perc: [2, 3, 6, 10, 11, 14], percVoice: "bongo", bass: [0, 6, 8, 14], bassNotes: [0, 0, 7, 5], chord: [0, 3, 7, 12], chordSteps: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15], chordVoice: "pluck", swing: 0 },
    leaning: "karaoke",
  },
  {
    id: "merengue",
    name: "Merengue",
    roots: "Dominican Republic — Cibao's perico ripiao, then the big bands of Juan Luis Guerra and Wilfrido Vargas.",
    bpm: [140, 170],
    palette: { bg: "#06233a", surface: "#0d3b5c", primary: "#ffd23f", secondary: "#ee4266", ink: "#fffaf0" },
    display: "Bungee",
    motif: "waves",
    rooms: "Weddings, quinceañeras, carnival in La Vega, every family party that has ever happened.",
    style: "Bright prints, white linen, sombrero de cana; carnival brings the diablos cojuelos' masks and mirrors.",
    art: "Carnival masks with horns and mirrors, flag colours, brass sections shining on sleeves.",
    dances: [
      { name: "Paso de merengue", how: "March in place, weight from foot to foot, the hips follow — never the shoulders." },
      { name: "La vuelta", how: "Hands held high, turn under the arm without losing the march." },
    ],
    fx: ["echo", "filter", "gate"],
    mixing: "Fast and bright: cut in on the one, ride the tambora's break, and keep the güira in phase or the floor feels it.",
    theme: "Caribbean",
    skin: "Waves",
    emoji: "🎺",
    root: 196,
    scale: "major",
    pattern: { steps: 16, kick: [0, 8], snare: [4, 12], hat: [], perc: [0, 1, 2, 3, 4, 6, 8, 9, 10, 11, 12, 14], percVoice: "guira", bass: [2, 6, 10, 14], bassNotes: [0, 7, 5, 7], chord: [0, 4, 7], chordSteps: [2, 6, 10, 14], chordVoice: "piano", swing: 0 },
    leaning: "full",
  },
  {
    id: "dembow",
    name: "Dembow",
    roots: "Santo Domingo's barrios in the 2000s — El Alfa, Chimbala, the teteo.",
    bpm: [115, 130],
    palette: { bg: "#0d0d10", surface: "#1d1d24", primary: "#39ff14", secondary: "#ff2e88", ink: "#f4f4f4" },
    display: "Rubik Glitch",
    motif: "graffiti",
    rooms: "Street corners with a wall of speakers on a pickup, the teteo, car shows, packed rooftops.",
    style: "Designer tracksuits, chains, slides with socks, colourful hair, sunglasses at night.",
    art: "Neon type, stickers, phone-camera flyers, the speaker wall itself as decor.",
    dances: [
      { name: "El sandungueo", how: "Bend the knees, bounce on every beat, let the hips lead in small fast circles." },
      { name: "Free-style teteo", how: "Arms up, jump, point at the DJ when the drop lands — energy over steps." },
    ],
    fx: ["crush", "echo", "gate"],
    mixing: "Short songs, quick cuts, airhorns and drops; double-drop two records on the same snare and ride the crowd's call-and-response.",
    theme: "Malecón Night",
    skin: "Palm shadows",
    emoji: "🔊",
    root: 147,
    scale: "minor",
    pattern: { steps: 16, kick: FOUR, snare: [3, 6, 11, 14], hat: [2, 6, 10, 14], perc: [7, 15], percVoice: "rim", bass: [0, 3, 8, 11], bassNotes: [0, 0, 3, 5], chord: [0, 3, 7], chordSteps: [0, 8], chordVoice: "stab", swing: 0 },
    leaning: "pro",
  },
  {
    id: "reggaeton",
    name: "Reggaeton",
    roots: "Panama's reggae en español and Puerto Rico's underground, out to the whole world.",
    bpm: [88, 100],
    palette: { bg: "#1a0b2e", surface: "#2d1450", primary: "#ffb000", secondary: "#00e0ff", ink: "#fff7e6" },
    display: "Bungee Inline",
    motif: "halftone",
    rooms: "Club nights from San Juan to Madrid, festival main stages, every quinceañera after midnight.",
    style: "Oversized jerseys, bucket hats, nails, sneakers kept clean.",
    art: "Chrome type, sunset gradients, island flags, the perreo poster.",
    dances: [
      { name: "Perreo", how: "Knees bent, hips low, roll on the dembow — the snare tells the hips when." },
      { name: "Bounce step", how: "Step-touch with a shoulder bounce on every snare." },
    ],
    fx: ["echo", "filter", "reverb"],
    mixing: "Everything sits around 90–96 bpm, so blends are easy; mix on the chorus, drop the bass on the switch.",
    theme: "Carnaval",
    skin: "Halftone",
    emoji: "🍑",
    root: 165,
    scale: "minor",
    pattern: { steps: 16, kick: FOUR, snare: [3, 6, 11, 14], hat: [0, 2, 4, 6, 8, 10, 12, 14], perc: [], percVoice: "rim", bass: [0, 8], bassNotes: [0, 5], chord: [0, 3, 7, 10], chordSteps: [0, 3, 6, 8, 11, 14], chordVoice: "pad", swing: 0 },
    leaning: "pro",
  },
  {
    id: "salsa",
    name: "Salsa",
    roots: "New York, Havana, San Juan, Cali — Fania, son cubano, the bands of the 1970s.",
    bpm: [160, 220],
    palette: { bg: "#3a0d0d", surface: "#5c1717", primary: "#ffcf56", secondary: "#ff6b35", ink: "#fff5e1" },
    display: "Abril Fatface",
    motif: "sunburst",
    rooms: "Salsa socials, ballrooms, Cali's salsotecas, a Sunday orchestra in the plaza.",
    style: "Two-tone shoes, flowing skirts, a hat brim at an angle, the band in matching suits.",
    art: "Fania's photo-typographic covers, brass and congas, bold serif headlines.",
    dances: [
      { name: "On1 / On2", how: "Six steps across eight counts — on the one (LA) or the two (New York)." },
      { name: "Cross-body lead", how: "The lead opens a slot; the follow walks through it and turns." },
      { name: "Caleña footwork", how: "Cali style: tiny, lightning-fast steps on the fast records." },
    ],
    fx: ["reverb", "filter"],
    mixing: "Clave decides everything: never cross a 3-2 into a 2-3. Most salsa DJs play whole songs and mix gently between them.",
    theme: "Latin",
    skin: "Papel picado",
    emoji: "💃",
    root: 196,
    scale: "minor",
    pattern: { steps: 32, kick: [0, 16], snare: [], hat: [], perc: [0, 6, 12, 20, 24], percVoice: "clave", bass: [6, 12, 22, 28], bassNotes: [0, 0, 5, 7], chord: [0, 3, 7, 12], chordSteps: [0, 3, 6, 10, 12, 16, 19, 22, 26, 28], chordVoice: "piano", swing: 0 },
    leaning: "hobbyist",
  },
  {
    id: "cumbia",
    name: "Cumbia",
    roots: "Colombia's Caribbean coast, then Mexico's sonideros, Argentina's villera, Peru's chicha.",
    bpm: [85, 110],
    palette: { bg: "#123524", surface: "#1e5a3c", primary: "#ffde59", secondary: "#ff5c8a", ink: "#fffbe6" },
    display: "Chango",
    motif: "papel",
    rooms: "Sonidero street parties, picós in Barranquilla, family weddings, quinceañeras.",
    style: "Pollera skirts and sombrero vueltiao on the coast; in the city, sonidero shirts and cowboy boots.",
    art: "The picó sound systems' hand-painted art, papel picado, chicha posters in fluorescent ink.",
    dances: [
      { name: "Paso de cumbia", how: "Step back on one foot, return, other foot — a gentle circle that never stops." },
      { name: "La rueda", how: "Partners turn around each other while the step keeps going." },
    ],
    fx: ["echo", "reverb", "flanger"],
    mixing: "The sonidero speaks over the music — saludos to the barrio — so leave room on the mic and echo the shout-outs.",
    theme: "Latin",
    skin: "Papel picado",
    emoji: "🪗",
    root: 175,
    scale: "minor",
    pattern: { steps: 16, kick: [0, 8], snare: [6, 14], hat: [], perc: [0, 2, 3, 4, 6, 8, 10, 11, 12, 14], percVoice: "shaker", bass: [0, 6, 8, 14], bassNotes: [0, 7, 5, 7], chord: [0, 3, 7], chordSteps: [2, 6, 10, 14], chordVoice: "organ", swing: 0.05 },
    leaning: "hobbyist",
  },
  {
    id: "kompa",
    name: "Kompa & zouk",
    roots: "Haiti's konpa dirèk (Nemours Jean-Baptiste) and the French Antilles' zouk.",
    bpm: [110, 130],
    palette: { bg: "#08283d", surface: "#0f4c6a", primary: "#f9c80e", secondary: "#f86624", ink: "#fffdf0" },
    display: "Bevan",
    motif: "waves",
    rooms: "Bals in Port-au-Prince and Miami's Little Haiti, Martinique terraces, slow-dance late night.",
    style: "Sharp white, linen, a flower behind the ear, polished shoes.",
    art: "Painted tap-tap buses, bright naïve painting, sea and mountain.",
    dances: [{ name: "Kompa sway", how: "Close hold, a small two-step, hips sway on the cymbal's lilt." }],
    fx: ["reverb", "echo"],
    mixing: "Long, smooth blends on the steady cymbal; the guitar answers the vocal, so do not step on either.",
    theme: "Sunset",
    skin: "Waves",
    emoji: "🌊",
    root: 185,
    scale: "major",
    pattern: { steps: 16, kick: [0, 8], snare: [12], hat: [0, 2, 3, 4, 6, 8, 10, 11, 12, 14], perc: [], percVoice: "cowbell", bass: [0, 3, 8, 11], bassNotes: [0, 0, 5, 7], chord: [0, 4, 7, 11], chordSteps: [2, 6, 10, 14], chordVoice: "pluck", swing: 0.08 },
    leaning: "hobbyist",
  },
  {
    id: "dancehall",
    name: "Dancehall & reggae",
    roots: "Kingston's sound systems, from King Tubby's dub to the riddim era.",
    bpm: [70, 105],
    palette: { bg: "#0b2d10", surface: "#174a1f", primary: "#ffcc00", secondary: "#e4002b", ink: "#fffbe0" },
    display: "Ultra",
    motif: "tape",
    rooms: "Street dances on Kingston corners, sound clashes, Notting Hill's carnival, basement selectors.",
    style: "Mesh vests, bright two-pieces, clarks shoes, the selector's towel on the shoulder.",
    art: "Hand-painted dance posters with stacked type, sound-system speaker boxes as murals.",
    dances: [
      { name: "Gully creeper", how: "Low, sliding steps with a shoulder glide — creep across the floor." },
      { name: "Bogle", how: "The whole body waves from the chest — named for the dancer who invented it." },
    ],
    fx: ["echo", "reverb", "filter"],
    mixing: "Juggling: many records on one riddim, cut fast, rewind the tune when the crowd demands — the pull-up is part of the show.",
    theme: "Festival",
    skin: "Halftone",
    emoji: "📢",
    root: 147,
    scale: "minor",
    pattern: { steps: 16, kick: [0, 6, 8, 14], snare: [4, 12], hat: [2, 6, 10, 14], perc: [3, 11], percVoice: "rim", bass: [0, 3, 6, 10], bassNotes: [0, 0, 3, 5], chord: [0, 3, 7], chordSteps: [2, 6, 10, 14], chordVoice: "organ", swing: 0.1 },
    leaning: "pro",
  },
  {
    id: "afrobeats",
    name: "Afrobeats & amapiano",
    roots: "Lagos and Accra's afrobeats; Johannesburg and Pretoria's amapiano and its log drum.",
    bpm: [100, 115],
    palette: { bg: "#2b1600", surface: "#4a2600", primary: "#ffb627", secondary: "#2ec4b6", ink: "#fff8ec" },
    display: "Righteous",
    motif: "sunburst",
    rooms: "Lagos rooftops, London's day parties, Joburg's shisanyamas, beach clubs from Ghana to Tulum.",
    style: "Ankara prints, bucket hats, matching sets, a lot of gold.",
    art: "Bold wax-print patterns, sun motifs, warm gradients, photographic flyers with big type.",
    dances: [
      { name: "Shaku shaku", how: "Arms crossed and swinging, feet shuffling sideways." },
      { name: "Amapiano groove", how: "Slow, cool and low: shoulders and feet answering the log drum." },
    ],
    fx: ["reverb", "filter", "echo"],
    mixing: "Long blends; let the log drum carry across both records, and filter the low end of the one coming in.",
    theme: "Sunset",
    skin: "Palm shadows",
    emoji: "🌅",
    root: 175,
    scale: "dorian",
    pattern: { steps: 16, kick: [0, 7, 8], snare: [4, 12], hat: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15], perc: [3, 6, 11, 13], percVoice: "log", bass: [0, 10], bassNotes: [0, 5], chord: [0, 3, 7, 10], chordSteps: [0, 6, 12], chordVoice: "pad", swing: 0.12 },
    leaning: "pro",
  },
  {
    id: "hiphop",
    name: "Hip-hop",
    roots: "The Bronx, 1973: Kool Herc's breaks, Grandmaster Flash's hands, everything since.",
    bpm: [80, 100],
    palette: { bg: "#151515", surface: "#2a2a2a", primary: "#ffd400", secondary: "#e63946", ink: "#f8f8f2" },
    display: "Rubik Spray Paint",
    motif: "graffiti",
    rooms: "Block parties, basement battles, club nights, radio mix shows.",
    style: "Sneakers first, tracksuits, gold rope chains, fitted caps, the record bag.",
    art: "Subway-car graffiti, photocopied flyers, gold-chain covers, wildstyle tags.",
    dances: [
      { name: "Top rock", how: "Standing footwork before the breaker goes down — cross, step, kick, on the beat." },
      { name: "The two-step", how: "Step side, bring the other foot in, lean into it. Everyone can." },
    ],
    fx: ["echo", "filter", "crush"],
    mixing: "Cuts, scratches and quick blends on the break; know your doubles, and never mix over the hook.",
    theme: "Scratch",
    skin: null,
    emoji: "🎤",
    root: 110,
    scale: "minor",
    pattern: { steps: 16, kick: [0, 7, 10], snare: [4, 12], hat: [0, 2, 4, 6, 8, 10, 12, 14], perc: [], percVoice: "rim", bass: [0, 7, 10], bassNotes: [0, 0, 3], chord: [0, 3, 7, 10], chordSteps: [0, 8], chordVoice: "piano", swing: 0.16 },
    leaning: "pro",
  },
  {
    id: "house",
    name: "House",
    roots: "Chicago's Warehouse and Frankie Knuckles, New York's Paradise Garage, then everywhere.",
    bpm: [118, 128],
    palette: { bg: "#10183a", surface: "#1c2a5e", primary: "#ff7eb6", secondary: "#7afcff", ink: "#f5f7ff" },
    display: "Syne",
    motif: "disco",
    rooms: "Lofts, warehouses, Ibiza terraces, Sunday day parties that run into the night.",
    style: "Loose and comfortable: white tees, wide trousers, fans, sneakers you can dance in for eight hours.",
    art: "Warehouse flyers, gospel-tinged type, the mirror ball, soft gradients.",
    dances: [
      { name: "Jacking", how: "The chest pumps forward and back with the kick — the original house move." },
      { name: "Lofting", how: "Footwork, spins and floor moves from the New York lofts." },
    ],
    fx: ["filter", "echo", "reverb"],
    mixing: "Long beatmatched blends, EQ swaps on the phrase: bass out of one, bass into the other, on the 16 or the 32.",
    theme: "Club",
    skin: "Halftone",
    emoji: "🏠",
    root: 110,
    scale: "dorian",
    pattern: { steps: 16, kick: FOUR, snare: [4, 12], hat: [2, 6, 10, 14], perc: [3, 7, 11, 15], percVoice: "shaker", bass: [2, 6, 10, 14], bassNotes: [0, 0, 5, 7], chord: [0, 3, 7, 10], chordSteps: [3, 6, 11, 14], chordVoice: "stab", swing: 0.06 },
    leaning: "pro",
  },
  {
    id: "techno",
    name: "Techno",
    roots: "Detroit's Belleville Three, then Berlin after the Wall, then the world's warehouses.",
    bpm: [125, 145],
    palette: { bg: "#0b0b0b", surface: "#1a1a1a", primary: "#e5e5e5", secondary: "#ff3b30", ink: "#fafafa" },
    display: "Unbounded",
    motif: "concrete",
    rooms: "Warehouses, bunkers, power plants, fields at 6 a.m., the room with one red light.",
    style: "Black, black, black. Harnesses, mesh, utility trousers, boots. No photos on the floor.",
    art: "Brutalist black-and-white architecture, minimal type, a single colour, strict grids.",
    dances: [{ name: "The stomp", how: "Feet planted, the body locks to the kick, eyes closed. It is a trance, not a step." }],
    fx: ["filter", "echo", "reverb", "gate"],
    mixing: "Long, hypnotic blends; three records at once is normal. The filter and a delay throw are the instrument.",
    theme: "Industrial Techno",
    skin: "Trash polka",
    emoji: "🏭",
    root: 55,
    scale: "minor",
    pattern: { steps: 16, kick: FOUR, snare: [], hat: [2, 6, 10, 14], perc: [4, 12], percVoice: "rim", bass: [2, 3, 6, 7, 10, 11, 14, 15], bassNotes: [0, 0, 0, 12], chord: [0, 3, 7], chordSteps: [3, 11], chordVoice: "stab", swing: 0 },
    leaning: "pro",
  },
  {
    id: "dnb",
    name: "Drum & bass / jungle",
    roots: "London and Bristol, 1992 — breakbeats, sound-system bass and Jamaican toasting at double speed.",
    bpm: [165, 178],
    palette: { bg: "#0a0f1f", surface: "#121d3a", primary: "#00ff9f", secondary: "#ff006e", ink: "#eafff6" },
    display: "Major Mono Display",
    motif: "neon",
    rooms: "Pirate radio, Fabric's Room One, festival tents, rain.",
    style: "Tracksuits and Air Max, bucket hats, puffer jackets, glowsticks at the old ones.",
    art: "Dystopian rave flyers, airbrushed chrome, the Amen break as a sacred text.",
    dances: [
      { name: "Liquid stepping", how: "Fast feet, relaxed body: step half time while the drums run double." },
      { name: "Skanking", how: "Arms and knees flicking, a nod to the dub roots." },
    ],
    fx: ["crush", "flanger", "echo", "filter"],
    mixing: "Double drops, quick rewinds, mixing on the 32; the MC hypes the drop, the selector chooses it.",
    theme: "Cyber Trance",
    skin: "Trash polka",
    emoji: "⚡",
    root: 73,
    scale: "minor",
    pattern: { steps: 16, kick: [0, 10], snare: [4, 12], hat: [0, 2, 4, 6, 7, 8, 10, 12, 14, 15], perc: [], percVoice: "rim", bass: [0, 3, 10], bassNotes: [0, 0, 7], chord: [0, 3, 7], chordSteps: [0], chordVoice: "pad", swing: 0 },
    leaning: "pro",
  },
  {
    id: "disco",
    name: "Disco & funk",
    roots: "Philadelphia strings, New York's Loft and Studio 54, Italo and nu-disco ever since.",
    bpm: [110, 125],
    palette: { bg: "#2a0a3a", surface: "#47135f", primary: "#ffd166", secondary: "#ef476f", ink: "#fff6fb" },
    display: "Monoton",
    motif: "disco",
    rooms: "Roller rinks, lit floors, rooftop bars, wedding late sets that turn into a party.",
    style: "Sequins, flares, platform shoes, glitter — and a fan for the heat.",
    art: "Chrome script, mirror balls, airbrushed sleeves, 12-inch white labels.",
    dances: [
      { name: "The hustle", how: "Partner dance on a six-count, spins and walks to the string section." },
      { name: "Point to the sky", how: "Hip out, finger up, finger down across the body. You know it." },
    ],
    fx: ["filter", "flanger", "echo"],
    mixing: "Live drums drift, so ride the pitch; the edits and extended mixes give you long intros to mix into.",
    theme: "Velvet",
    skin: "Glowing plankton",
    emoji: "🪩",
    root: 147,
    scale: "dorian",
    pattern: { steps: 16, kick: FOUR, snare: [4, 12], hat: [2, 6, 10, 14], perc: [1, 3, 5, 7, 9, 11, 13, 15], percVoice: "shaker", bass: [0, 2, 4, 6, 8, 10, 12, 14], bassNotes: [0, 12, 0, 12, 5, 17, 7, 19], chord: [0, 3, 7, 10], chordSteps: [2, 6, 10, 14], chordVoice: "stab", swing: 0 },
    leaning: "full",
  },
  {
    id: "pop",
    name: "Pop & sing-along",
    roots: "Every radio hit everyone knows the words to — the heart of a karaoke night.",
    bpm: [95, 128],
    palette: { bg: "#170a2e", surface: "#2c1258", primary: "#ff4fd8", secondary: "#2ef2ff", ink: "#fff3fe" },
    display: "Bungee Shade",
    motif: "neon",
    rooms: "Karaoke bars, birthdays, office parties, weddings when the whole room sings.",
    style: "Whatever the singer wants: costumes, sequins, the duet partner's matching shirt.",
    art: "Neon signs, glossy pop sleeves, lyrics on screens, microphones everywhere.",
    dances: [
      { name: "The chorus jump", how: "Everybody jumps on the chorus. That is the whole dance." },
      { name: "Mic-stand point", how: "Point the mic at the room on the line everyone knows." },
    ],
    fx: ["reverb", "echo"],
    mixing: "Mix to the singalong: keep the hits coming, shorten the outros, let the room sing the last chorus a cappella.",
    theme: "Karaoke Neon",
    skin: "Halftone",
    emoji: "🎶",
    root: 262,
    scale: "major",
    pattern: { steps: 16, kick: [0, 8], snare: [4, 12], hat: [0, 2, 4, 6, 8, 10, 12, 14], perc: [], percVoice: "shaker", bass: [0, 8], bassNotes: [0, 7, 9, 5], chord: [0, 4, 7], chordSteps: [0, 4, 8, 12], chordVoice: "pad", swing: 0 },
    leaning: "karaoke",
  },
  {
    id: "rock",
    name: "Rock & classics",
    roots: "Guitars from the 1960s on — the anthems, the power ballads, the bar-band singalongs.",
    bpm: [100, 160],
    palette: { bg: "#1b1b1b", surface: "#303030", primary: "#ff5a36", secondary: "#f2e94e", ink: "#f7f7f7" },
    display: "Metal Mania",
    motif: "tape",
    rooms: "Pubs, biker bars, karaoke nights' power-ballad hour, festivals in fields.",
    style: "Band tees, leather, denim, boots, the air guitar.",
    art: "Gig posters, photocopied zines, tattoo flash, torn tape.",
    dances: [{ name: "Headbang & air guitar", how: "Nod with the snare; on the solo, the guitar is invisible but very real." }],
    fx: ["reverb", "echo", "filter"],
    mixing: "Mostly whole songs; segue on the last chord ringing out, or cut tight on the count-in.",
    theme: "Booth",
    skin: null,
    emoji: "🎸",
    root: 82,
    scale: "minor",
    pattern: { steps: 16, kick: [0, 8, 10], snare: [4, 12], hat: [0, 2, 4, 6, 8, 10, 12, 14], perc: [], percVoice: "rim", bass: [0, 2, 4, 6, 8, 10, 12, 14], bassNotes: [0, 0, 0, 0, 5, 5, 3, 3], chord: [0, 7, 12], chordSteps: [0, 2, 4, 6, 8, 10, 12, 14], chordVoice: "stab", swing: 0 },
    leaning: "karaoke",
  },
  {
    id: "lounge",
    name: "Lounge, chill & ambient",
    roots: "Café del Mar sunsets, downtempo, bossa nova's grandchildren, the bioluminescent bay at night.",
    bpm: [70, 110],
    palette: { bg: "#041b2d", surface: "#08324f", primary: "#5ef2d6", secondary: "#a58bff", ink: "#ecfffb" },
    display: "Josefin Sans",
    motif: "plankton",
    rooms: "Hotel bars, sunset terraces, dinners before the party, the warm-up hour.",
    style: "Linen, sandals, a drink with a garnish.",
    art: "Long-exposure photographs, soft gradients, sea and sky.",
    dances: [{ name: "The sway", how: "A drink in one hand, the other free. Nobody dances yet — that is the point." }],
    fx: ["reverb", "echo", "filter"],
    mixing: "Glacial blends, key-matched; the warm-up's job is to set a mood without spending the night's energy.",
    theme: "Bioluminescent Bay",
    skin: "Glowing plankton",
    emoji: "🌙",
    root: 131,
    scale: "major",
    pattern: { steps: 16, kick: [0, 10], snare: [], hat: [4, 12], perc: [2, 6, 10, 14], percVoice: "shaker", bass: [0, 10], bassNotes: [0, 5], chord: [0, 4, 7, 11, 14], chordSteps: [0, 8], chordVoice: "pad", swing: 0.1 },
    leaning: "hobbyist",
  },
];

export const GENRE_BY_ID = Object.fromEntries(GENRES.map((g) => [g.id, g])) as Record<string, Genre>;

/** The island's own colours when nobody has told it anything yet. */
export const ISLAND = {
  bg: "#06233a",
  surface: "#0d3b5c",
  primary: "#ffb703",
  secondary: "#ff5d73",
  ink: "#fdf6e3",
  sand: "#f3d9a4",
  palm: "#1f7a5c",
};

export function scaleNotes(scale: Genre["scale"]): number[] {
  return scale === "major" ? [0, 2, 4, 5, 7, 9, 11] : scale === "dorian" ? [0, 2, 3, 5, 7, 9, 10] : [0, 2, 3, 5, 7, 8, 10];
}
