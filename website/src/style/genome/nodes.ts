/**
 * The genome's nodes. Three paths are written in depth — EDM → Electro
 * swing, Latin & Caribbean → Bachata, Urban → Hip-hop — and the rest are
 * seeded from the island's genre notes (`lib/genres.ts`) and named lightly,
 * to be deepened the same way.
 *
 * Every djmanzo theme and skin named here is one the application ships and
 * will wear together (`ui/src/controls/themes/packages.ts`,
 * `ui/src/skins/skins.ts`); a test holds them to that list.
 */

import { GENRES, type Genre } from "../../lib/genres";
import type { GenomeNode, StyleTokens } from "./types";

type Settable = GenomeNode["set"];

/** Everything, once, for the root: a dark room before the music starts. */
const ROOT: Omit<StyleTokens, "id" | "name" | "lineage"> = {
  sound: { bpm: [90, 130], signature: ["a kick drum", "a bass line", "a voice"], structure: "Phrases of eight, sixteen and thirty-two bars, and a DJ who counts them." },
  visual: ["record sleeves", "club flyers", "photographs of rooms"],
  type: { display: "Bungee", text: "Figtree" },
  palette: { night: "#0b0b10", day: "#efe9df", ink: "#f4efe6", accents: ["#ff5d73", "#ffd166", "#5ef2d6"], glow: "#ffd166" },
  light: ["one warm lamp over the booth", "colour washes"],
  decor: ["a booth", "a floor", "a door that opens at night"],
  materials: ["vinyl", "steel", "skin", "air"],
  fashion: ["whatever you can dance in"],
  dance: ["moving to the beat"],
  mood: ["anticipation", "release", "together"],
  values: ["the floor belongs to everyone on it", "look out for each other", "protect your ears"],
  roots: { places: [], story: "Every scene began with someone playing records for a room." },
  complements: [
    { what: "Dance classes", why: "The floor is better when more people can move on it." },
    { what: "Ear protection and good sleep", why: "A DJ's ears are the instrument; a long career needs them." },
  ],
  craft: ["count the phrases", "match the tempo", "read the room"],
  app: { theme: "Booth", skin: null },
  motion: { pulse: "four", ease: "power2.inOut", energy: 0.5 },
  brush: { radius: 6, grain: 0.3, flow: "radial" },
  scene: {
    silence: "A dark room before the doors open; one beam of light, and a needle held above a record.",
    groove: "A record seen from a hand's width: grooves like ploughed fields at night.",
    needle: "A stylus in macro, riding one wall of the groove.",
    cue: "Headphones on a hook, a small pool of light only one person stands in.",
    confluence: "Two streams of light meeting at a mixer.",
    voice: "A microphone on a stand, and a spot on the floor in front of it.",
    amp: "Amplifier valves warming up behind a grille; a power dial.",
    cone: "A speaker cone breathing, dust lifting off its surface.",
    floor: "Silhouettes dancing under one colour.",
    afterglow: "Dawn through an open door; the room emptying, the music still in everyone's step.",
  },
  photos: ["dj booth dark", "vinyl record macro", "dance floor silhouettes"],
  voice: "Warm, direct, a little mysterious; says one true thing and lets the picture say the rest.",
  sources: [],
};

const FAMILIES: GenomeNode[] = [
  { id: "music", name: "Music", parent: null, set: ROOT },

  // --- EDM / electronic --------------------------------------------------------
  {
    id: "edm",
    name: "EDM & electronic",
    parent: "music",
    set: {
      sound: {
        bpm: [120, 130],
        signature: ["four-on-the-floor kick", "side-chained pads", "risers and white-noise sweeps", "synth leads", "the drop"],
        structure: "Intro, build, drop, breakdown, drop, outro — in sixteen- and thirty-two-bar phrases a DJ mixes across.",
      },
      visual: ["rave flyers", "festival line-up posters", "chrome and LED type", "laser photography"],
      type: { display: "Orbitron", text: "Inter" },
      palette: { night: "#05060f", day: "#e8eeff", ink: "#f2f5ff", accents: ["#00e5ff", "#ff2bd6", "#c6ff00"], glow: "#00e5ff" },
      light: ["laser fans", "LED walls", "moving heads", "CO₂ jets — never strobes on this site"],
      decor: ["scaffolding stages", "LED tunnels", "kinetic light bars", "a field of flags"],
      materials: ["brushed aluminium", "glass", "mesh", "light itself"],
      fashion: ["festival wear", "reflective fabric", "bucket hats", "trainers made for eight hours", "glitter"],
      dance: ["the Melbourne shuffle", "hands up on the drop", "free movement"],
      mood: ["euphoric", "communal", "the held breath before the drop"],
      values: ["PLUR — peace, love, unity, respect", "kandi bracelets traded as a greeting", "water, rest and looking out for each other"],
      roots: {
        places: ["Chicago", "Detroit", "Manchester and London", "Berlin", "Ibiza", "Las Vegas"],
        story: "Chicago house and Detroit techno in the 1980s; the UK's acid-house summer of 1988; Europe's clubs and festivals; and the American festival stages of the 2010s that gave it the name EDM.",
      },
      complements: [
        { what: "Morning and sober raves", why: "Daybreaker and Morning Gloryville dance before work — the euphoria without the night." },
        { what: "Running clubs", why: "A steady 125 is a running cadence too." },
        { what: "Spin and dance fitness", why: "The drop on a bike." },
        { what: "Making tracks", why: "Every DJ who loves a drop wants to build one." },
      ],
      craft: ["long blends phrase over phrase", "swap the low end on the one", "filter builds", "harmonic mixing by Camelot key"],
      app: { theme: "Club", skin: "Halftone" },
      motion: { pulse: "four", ease: "power2.inOut", energy: 0.75 },
      brush: { radius: 5, grain: 0.25, flow: "radial" },
      scene: {
        confluence: "An LED-wall booth on scaffolding, two decks lit from below.",
        amp: "Racks of amplifiers backstage, their meters in green and red.",
        cone: "A line-array tower with a laser fan opening above it.",
        floor: "A festival field at night, ten thousand hands up on the drop.",
        afterglow: "Sunrise over a festival field; the last set still playing to the ones who stayed.",
      },
      photos: ["festival stage lasers", "dj booth led wall", "festival crowd sunrise"],
      voice: "Bright and communal; short lines that land on the one.",
      sources: ["Simon Reynolds, Energy Flash (1998)", "Rentfrow & Gosling, The Do Re Mi's of Everyday Life (2003)", "Daybreaker, daybreaker.com"],
    },
  },

  // --- The jazz age, which electro swing also belongs to ----------------------------
  {
    id: "jazzage",
    name: "Jazz age & vintage",
    parent: "music",
    set: {
      sound: { bpm: [110, 180], signature: ["horn sections", "clarinet", "upright bass", "brushed snare", "the crackle of shellac"], structure: "Songs and big-band arrangements: head, solos, shout chorus." },
      visual: ["Art Deco posters", "cabaret bills", "sepia photographs"],
      type: { display: "Limelight", text: "Josefin Sans" },
      palette: { night: "#0d0a07", day: "#f3e9d2", ink: "#f5e6c4", accents: ["#d4a64a", "#7a1e2b", "#2e5e4e"], glow: "#ffcf70" },
      mood: ["playful", "nostalgic", "theatrical"],
      app: { theme: "Velvet", skin: null },
      motion: { pulse: "swing", ease: "back.out(1.4)", energy: 0.6 },
      brush: { radius: 7, grain: 0.6, flow: "horizontal" },
    },
  },

  // --- Latin & Caribbean ---------------------------------------------------------
  {
    id: "latin",
    name: "Latin & Caribbean",
    parent: "music",
    set: {
      sound: {
        bpm: [95, 135],
        signature: ["the clave", "güira and bongó", "congas and timbales", "brass", "the tresillo under everything"],
        structure: "Songs built for dancing in couples — verse, chorus and a section where the band takes off; DJs mix song to song, often short.",
      },
      visual: ["hand-painted rótulos", "colmado walls", "carnival posters", "album covers in gold script"],
      type: { display: "Shrikhand", text: "Figtree" },
      palette: { night: "#1a0b12", day: "#fff3e0", ink: "#fff4e6", accents: ["#ff4f6d", "#ffc93c", "#18c29c"], glow: "#ffb347" },
      light: ["string lights", "coloured bulbs", "a colmado's neon sign"],
      decor: ["plastic chairs pushed back for dancing", "a tower of speakers", "palm fronds", "hand-painted signs", "a domino table"],
      materials: ["painted wood", "corrugated metal", "terracotta", "palm"],
      fashion: ["guayaberas and linen", "a gold chain", "a dress that turns with you", "suede-soled dance shoes"],
      dance: ["partner dances, led and followed", "the turn", "everyone, from the grandmother to the kids"],
      mood: ["warm", "flirtatious", "family", "nostalgic and celebratory at once"],
      values: ["dance with anyone who asks", "elders and kids on the same floor", "music as family history"],
      roots: {
        places: ["Dominican Republic", "Puerto Rico", "Cuba", "Colombia", "Haiti", "Jamaica", "Trinidad", "New York", "Miami", "Madrid"],
        story: "African, Indigenous and European music meeting in the Caribbean, then travelling with migration to New York, Miami and Madrid — and coming home changed.",
      },
      complements: [
        { what: "Partner-dance classes", why: "Every Latin floor is better when more people can lead and follow." },
        { what: "Cooking the food of the party", why: "Sancocho, empanadas, a pot for everyone — the party starts in the kitchen." },
        { what: "Baseball and dominoes", why: "The other two national pastimes on the same street." },
        { what: "Cycling the malecón", why: "The sea road at sunrise, the morning after." },
      ],
      craft: ["mix by song and by feel", "keep the clave", "quick cuts on the break", "build an arc across merengue, bachata and dembow"],
      app: { theme: "Latin", skin: "Papel picado" },
      motion: { pulse: "tresillo", ease: "sine.inOut", energy: 0.7 },
      brush: { radius: 6, grain: 0.35, flow: "swirl" },
      scene: {
        confluence: "A colmado terrace: two decks on a plastic table, a tower of speakers behind.",
        cone: "A tower of bocinas on a pickup's bed under string lights.",
        floor: "Couples turning on a terrace floor, a grandmother leading.",
        afterglow: "A sea road at dawn, the sea grey and gold.",
      },
      photos: ["colmado dominican republic", "caribbean street party string lights", "couples dancing salsa"],
      voice: "Warm and quick, teasing and tender, Spanish where it belongs.",
      sources: ["Deborah Pacini Hernández, Bachata (1995)", "Paul Austerlitz, Merengue (1997)"],
    },
  },

  // --- Urban ------------------------------------------------------------------------
  {
    id: "urban",
    name: "Urban",
    parent: "music",
    set: {
      sound: { bpm: [80, 100], signature: ["breakbeats", "samples", "the 808", "scratches", "a voice on the beat"], structure: "Verses and hooks over loops; DJs cut on the one and double records up." },
      visual: ["graffiti pieces", "mixtape covers", "block-party flyers", "sneaker boxes"],
      type: { display: "Permanent Marker", text: "Inter" },
      palette: { night: "#0b0b0d", day: "#f2efe9", ink: "#f7f7f2", accents: ["#ffd400", "#ff3d00", "#00b3ff"], glow: "#ffd400" },
      light: ["sodium streetlights", "a lamp over the decks", "a projector on a brick wall"],
      decor: ["brick walls", "crates of records", "a speaker stack", "folding tables"],
      materials: ["brick", "spray paint", "cardboard", "chrome"],
      fashion: ["sneakers kept clean", "caps", "tracksuits", "oversized tees", "a little gold"],
      dance: ["breaking", "popping and locking", "the cypher"],
      mood: ["confident", "playful", "competitive in good faith", "proud"],
      values: ["each one teach one", "the cypher gives everyone a turn", "be original", "respect the pioneers"],
      roots: { places: ["New York", "Atlanta", "Los Angeles", "London", "Paris", "Lagos", "Johannesburg"], story: "Block parties in the Bronx in the 1970s, then the whole world." },
      complements: [
        { what: "Breaking", why: "An Olympic sport since Paris 2024, and a cypher in every park." },
        { what: "Making beats", why: "A sampler and a crate: where every DJ's ear goes next." },
        { what: "Poetry and writing", why: "Bars are verse." },
        { what: "Legal walls and street art", why: "Graffiti's craft, with permission." },
        { what: "Basketball and skating", why: "The same streets, the same shoes." },
      ],
      craft: ["cut on the one", "doubles of the same record", "acapellas over instrumentals", "scratching"],
      app: { theme: "Scratch", skin: null },
      motion: { pulse: "half", ease: "power3.out", energy: 0.7 },
      brush: { radius: 4, grain: 0.45, flow: "cross" },
      scene: {
        confluence: "Two turntables on a folding table at a block party.",
        cone: "A speaker stack against a brick wall, a crowd's backs to the camera.",
        floor: "A cypher: a circle of sneakers, one dancer in the middle.",
        afterglow: "A rooftop at dawn over the city.",
      },
      photos: ["block party turntables", "graffiti wall", "breakdance cypher"],
      voice: "Confident and funny, quick on the beat; respect first, then the punchline.",
      sources: ["Jeff Chang, Can't Stop Won't Stop (2005)"],
    },
  },

  // --- Classics, and the party ---------------------------------------------------
  {
    id: "classics",
    name: "Classics",
    parent: "music",
    set: {
      visual: ["gatefold sleeves", "music-press photography", "concert posters"],
      mood: ["joyful", "nostalgic", "everyone knows the words"],
      app: { theme: "Spotlight", skin: "Halftone" },
      craft: ["play the records people grew up with", "mix on the chorus", "let a classic breathe"],
    },
  },
  {
    id: "party",
    name: "Party & sing-along",
    parent: "music",
    set: {
      mood: ["joyful", "silly in the best way", "together"],
      values: ["everyone sings", "the shy ones get a turn too"],
      app: { theme: "Spotlight", skin: "Halftone" },
      craft: ["read the age range in the room", "the chorus is the drop", "requests, and saying yes to the right ones"],
      complements: [
        { what: "A choir or a karaoke night", why: "Singing together is the oldest party there is." },
        { what: "Line-dance and folk-dance evenings", why: "Steps everyone can learn in a song." },
      ],
    },
  },
];

/** The three paths written in depth. */
const DEEP: GenomeNode[] = [
  {
    id: "electroswing",
    name: "Electro swing",
    parent: "edm",
    also: ["jazzage"],
    set: {
      sound: {
        bpm: [118, 132],
        signature: ["looped swing-era horns and clarinet", "Charleston rhythms", "upright-bass lines", "shellac crackle", "a house or hip-hop beat underneath", "swung hats"],
        structure: "House arrangements built on looped big-band phrases; breakdowns where the horns answer each other before the beat comes back.",
      },
      visual: ["Art Deco posters", "1920s and 30s cabaret bills", "gilded geometric type", "sepia film stills"],
      type: { display: "Limelight", text: "Josefin Sans" },
      // EDM's rig, through Art Deco: gold and black, one neon accent kept from the rave.
      palette: { night: "#0d0a07", day: "#f3e9d2", ink: "#f5e6c4", accents: ["#d4a64a", "#7a1e2b", "#00e5ff"], glow: "#ffcf70" },
      light: ["brass spotlights", "chandeliers", "a film projector's slow flicker", "gobos cut as Deco fans"],
      decor: ["a speakeasy bar", "velvet curtains", "gramophones", "a cabaret stage", "palms in brass pots"],
      materials: ["brass", "velvet", "black lacquer", "mirror glass", "shellac"],
      fashion: ["“1920s-inspired, not costume”", "fringe dresses", "pinstripes and suspenders", "flat caps and bow ties", "T-bar shoes", "a feather headband"],
      dance: ["the Charleston", "Lindy Hop", "Balboa", "the Shim Sham, which the whole room joins"],
      mood: ["playful", "nostalgic", "theatrical", "a wink"],
      values: ["dressing up is an invitation, never a door policy", "ask, dance, thank, rotate partners", "drop-in classes before the party"],
      roots: {
        places: ["Linz, Austria", "Paris", "Copenhagen", "London", "Berlin"],
        story: "In the 2000s producers looped swing and jazz records of the 1920s–40s over house and hip-hop beats. Parov Stelar, from Linz, and Caravan Palace, from Paris, made it a club sound; London's Electro Swing Club nights and Berlin's Bohème Sauvage parties gave it rooms to dress up for.",
      },
      complements: [
        { what: "Lindy Hop and Charleston classes", why: "The dances came first; the beat just came back for them." },
        { what: "Vintage markets and tailoring", why: "The look is half the night, and it lasts longer when it's made well." },
        { what: "Jazz history", why: "Django Reinhardt, Ella Fitzgerald, Cab Calloway — the records the loops came from." },
        { what: "Zero-proof cocktail craft", why: "The speakeasy's art, without the hangover." },
      ],
      craft: ["loop a horn stab over a house groove", "mix on the horns' call and the beat's answer", "keylock for old samples at new tempos", "a filter sweep that sounds like a radio dial"],
      app: { theme: "Velvet", skin: null },
      motion: { pulse: "swing", ease: "back.out(1.4)", energy: 0.65 },
      brush: { radius: 7, grain: 0.6, flow: "horizontal" },
      scene: {
        silence: "A gramophone in a dark speakeasy; one brass lamp, and dust in its light.",
        groove: "A shellac 78 under the lamp, its grooves a Deco fan.",
        needle: "A steel gramophone needle riding shellac.",
        cue: "A dressing-room mirror ringed with bulbs.",
        confluence: "A brass Art Deco console on a cabaret stage: two turntables in walnut.",
        voice: "A ribbon microphone on a chrome stand, a velvet curtain behind it.",
        amp: "Valve amplifiers glowing amber behind brass grilles.",
        cone: "A gramophone horn the size of a door, breathing.",
        floor: "Lindy hoppers and Charleston kicks under chandeliers.",
        afterglow: "Dawn on the street outside the cabaret; confetti on the cobbles.",
      },
      photos: ["art deco interior", "vintage gramophone", "swing dancers", "brass chandelier", "speakeasy bar"],
      voice: "Wry and theatrical, a wink in each line — a cabaret host introducing the next act.",
      sources: ["parovstelar.com", "Caravan Palace", "Electro Swing Club, London", "Bohème Sauvage, Berlin"],
    },
  },
  {
    id: "bachata",
    name: "Bachata",
    parent: "latin",
    set: {
      sound: {
        bpm: [120, 140],
        signature: ["the requinto guitar's arpeggios", "bongó", "güira", "electric bass", "a voice that sounds heartbroken"],
        structure: "Verse and chorus, then the mambo: the bongó gives way to the güira and the guitars drive.",
      },
      visual: ["romantic portrait covers in gold script", "roses and broken hearts", "hand-painted colmado signs"],
      type: { display: "Shrikhand", text: "Figtree" },
      palette: { night: "#2a0f2e", day: "#ffeef4", ink: "#fff1f6", accents: ["#ff6fa8", "#f6c453", "#7b2d5e"], glow: "#ff9cc2" },
      light: ["one red bulb over the counter", "string lights across the street"],
      decor: ["a colmado's shelves", "plastic chairs", "a speaker on the step", "roses painted on the wall", "a domino table"],
      fashion: ["a pressed guayabera or a fitted shirt", "a dress that moves on the turn", "suede-soled dance shoes"],
      dance: ["the basic — three steps and a tap, the hip on the tap", "Dominican footwork, quick and close", "the sensual style, born in Spain"],
      mood: ["romantic", "longing", "sweet", "funny about heartbreak"],
      values: ["once dismissed as amargue — bar music — now Dominican heritage", "the close hold is an invitation, and so is a no"],
      roots: {
        places: ["Dominican Republic", "the Bronx", "Madrid"],
        story: "From the colmados and countryside of the Dominican Republic in the 1960s (José Manuel Calderón's first recordings, 1962), through the 1990s of Luis Vargas, Antony Santos and Juan Luis Guerra, to Aventura in the Bronx and Romeo Santos. UNESCO listed the music and dance of Dominican bachata as heritage in 2019.",
      },
      complements: [
        { what: "Bachata classes, the Dominican way", why: "The footwork answers the bongó; learn it from the source." },
        { what: "Guitar lessons", why: "The requinto is the voice of the song." },
        { what: "Dominoes and baseball", why: "The colmado's other two companions." },
        { what: "La bandera", why: "Rice, beans and meat: the Dominican lunch the night grows out of." },
      ],
      craft: ["keep the requinto clean: low end out of the outgoing record", "mix on the mambo", "the arc from merengue through bachata to dembow"],
      app: { theme: "Bachata Rosa", skin: "Palm shadows" },
      motion: { pulse: "four", ease: "sine.inOut", energy: 0.5 },
      brush: { radius: 6, grain: 0.3, flow: "swirl" },
      scene: {
        silence: "A colmado at closing time, one red bulb on, a guitar on a chair.",
        groove: "A bachata single's label, worn soft at the edges.",
        confluence: "Two decks on a colmado counter under a red bulb.",
        voice: "A microphone wrapped in tape on a terrace, a crowd singing the chorus back.",
        cone: "A single speaker on the colmado's step, facing the street.",
        floor: "Couples close in the hold, a tap on four.",
        afterglow: "The Malecón at dawn.",
      },
      photos: ["colmado dominican republic", "bachata dancers", "santo domingo malecon dawn", "requinto guitar"],
      voice: "Warm and tender, a little heartbroken, and funny about it.",
      sources: ["UNESCO 2019: Music and dance of Dominican bachata", "Deborah Pacini Hernández, Bachata: A Social History of a Dominican Popular Music (1995)"],
    },
  },
  {
    id: "hiphop",
    name: "Hip-hop",
    parent: "urban",
    set: {
      sound: {
        bpm: [85, 98],
        signature: ["the break, looped", "sampled soul and funk", "scratches", "an MC on the beat"],
        structure: "Sixteen-bar verses and hooks over a loop; the DJ extends the break by playing two copies of the same record.",
      },
      visual: ["graffiti pieces", "mixtape covers", "Polaroids of the party", "flyers photocopied in black and white"],
      type: { display: "Rubik Spray Paint", text: "Inter" },
      palette: { night: "#0b0b0d", day: "#f2efe9", ink: "#f7f7f2", accents: ["#ffd400", "#e83a2e", "#3a7bd5"], glow: "#ffd400" },
      dance: ["breaking — toprock, footwork, freezes", "popping and locking", "the cypher"],
      roots: {
        places: ["The Bronx", "Queens", "Brooklyn", "then the world"],
        story: "The Bronx, 1973: DJ Kool Herc played the breaks of funk records back to back at a back-to-school party at 1520 Sedgwick Avenue. Grandmaster Flash made the mix precise; Grand Wizzard Theodore found the scratch.",
      },
      craft: ["the scratch and the cut", "back-to-back doubles to stretch the break", "acapellas over instrumentals", "juggling beats"],
      app: { theme: "Scratch", skin: null },
      motion: { pulse: "swing", ease: "power3.out", energy: 0.7 },
      scene: {
        silence: "A crate of records on a curb, a streetlight just coming on.",
        groove: "A funk record's break, worn grey by the needle going back to it.",
        needle: "A needle in a scratch, the record pulled back by a hand.",
        confluence: "Two turntables on a folding table, a mixer between them, a park behind.",
        voice: "An MC's microphone, the cable looped over a shoulder.",
        cone: "A speaker stack plugged into a lamppost, the block listening.",
        floor: "A cypher, a b-boy in a freeze.",
        afterglow: "A rooftop at dawn over the Bronx.",
      },
      photos: ["turntables block party", "graffiti wall", "breakdance cypher", "record crates"],
      voice: "Confident and funny, quick on the beat; respect first, then the punchline.",
      sources: ["Jeff Chang, Can't Stop Won't Stop (2005)", "Grandmaster Flash, The Adventures of Grandmaster Flash (2008)"],
    },
  },
];

/** Which family each of the island's genres belongs to. */
const FAMILY_OF: Record<string, string> = {
  bachata: "latin",
  merengue: "latin",
  dembow: "latin",
  reggaeton: "latin",
  salsa: "latin",
  cumbia: "latin",
  kompa: "latin",
  dancehall: "latin",
  afrobeats: "urban",
  hiphop: "urban",
  house: "edm",
  techno: "edm",
  dnb: "edm",
  lounge: "edm",
  disco: "classics",
  rock: "classics",
  pop: "party",
};

/** A lighter node from one of the island's genre notes. */
function seeded(g: Genre): GenomeNode {
  const set: Settable = {
    sound: { bpm: g.bpm },
    visual: [g.art],
    type: { display: g.display },
    palette: { night: g.palette.bg, ink: g.palette.ink, accents: [g.palette.primary, g.palette.secondary], glow: g.palette.primary },
    decor: [g.rooms],
    fashion: [g.style],
    dance: g.dances.map((d) => d.name),
    roots: { places: [], story: g.roots },
    craft: [g.mixing],
    app: { theme: g.theme, skin: g.skin },
  };
  return { id: g.id, name: g.name, parent: FAMILY_OF[g.id] ?? "music", set };
}

/** Genres the island did not have, named lightly. */
const LIGHT: GenomeNode[] = [
  {
    id: "trance",
    name: "Trance",
    parent: "edm",
    set: {
      sound: { bpm: [132, 140], signature: ["arpeggiated leads", "long breakdowns", "the uplifting key change"], structure: "Long builds and breakdowns, mixed over minutes rather than bars." },
      type: { display: "Audiowide" },
      palette: { night: "#060716", ink: "#eef0ff", accents: ["#7c5cff", "#00e5ff"], glow: "#7c5cff" },
      mood: ["uplifting", "emotional", "hands to the sky"],
      app: { theme: "Cyber Trance", skin: null },
      roots: { places: ["Frankfurt", "Amsterdam", "Goa"], story: "Germany and the Netherlands in the 1990s, and the beaches of Goa for its psychedelic branch." },
    },
  },
  {
    id: "mainstage",
    name: "Mainstage & big room",
    parent: "edm",
    set: {
      sound: { bpm: [126, 130], signature: ["the festival drop", "a chant", "a snare roll that goes on and on"], structure: "Short tracks, big drops, mixed fast." },
      app: { theme: "Festival", skin: "Halftone" },
      mood: ["huge", "together", "a field singing a melody"],
    },
  },
  {
    id: "hardstyle",
    name: "Hardstyle",
    parent: "edm",
    set: {
      sound: { bpm: [150, 160], signature: ["the distorted, pitched kick", "reverse bass", "euphoric screeches"], structure: "Kick-led drops between melodic breaks." },
      dance: ["jumpstyle", "the shuffle"],
      app: { theme: "Industrial Techno", skin: "Trash polka" },
      roots: { places: ["the Netherlands", "Belgium"], story: "The Netherlands and Belgium around 2000, out of hard trance and gabber." },
    },
  },
  {
    id: "synthwave",
    name: "Synthwave",
    parent: "edm",
    set: {
      sound: { bpm: [80, 118], signature: ["analogue synths", "gated reverb drums", "a saxophone at midnight"], structure: "Songs that remember the 1980s that never quite were." },
      visual: ["neon grids", "chrome lettering", "sunsets on a horizon line"],
      type: { display: "Monoton" },
      palette: { night: "#140726", ink: "#ffe6ff", accents: ["#ff3cac", "#2b86c5"], glow: "#ff3cac" },
      app: { theme: "Sunset", skin: "Waves" },
    },
  },
  {
    id: "soca",
    name: "Soca",
    parent: "latin",
    set: {
      sound: { bpm: [150, 165], signature: ["steel pan", "brass", "the call to wave a rag"], structure: "Carnival anthems: verse, chorus, a call the road answers." },
      dance: ["wining", "jumping and waving", "road march"],
      mood: ["jubilant", "carnival", "the whole road moving"],
      roots: { places: ["Trinidad and Tobago"], story: "Trinidad in the 1970s: Lord Shorty's soul of calypso." },
      app: { theme: "Carnaval", skin: "Halftone" },
    },
  },
  {
    id: "rnb",
    name: "R&B & soul",
    parent: "urban",
    set: {
      sound: { bpm: [60, 100], signature: ["a voice that carries the song", "keys", "a slow groove"], structure: "Songs: verse, chorus, a bridge that lifts." },
      mood: ["intimate", "smooth", "slow dance"],
      app: { theme: "Velvet", skin: "Glowing plankton" },
    },
  },
  {
    id: "eighties",
    name: "80s & 90s",
    parent: "classics",
    set: {
      sound: { bpm: [100, 130], signature: ["synth hooks", "big drums", "the chorus everyone knows"], structure: "Pop songs built for radio." },
      app: { theme: "Spectrum", skin: null },
    },
  },
  {
    id: "karaoke",
    name: "Karaoke night",
    parent: "party",
    set: {
      sound: { bpm: [70, 130], signature: ["a voice in the room", "the words on the screen", "a chorus the room sings back"], structure: "One song each, in turn." },
      app: { theme: "Karaoke Neon", skin: "Halftone" },
      scene: { voice: "A microphone passed hand to hand, the words lit on a screen behind." },
    },
  },
  {
    id: "schlager",
    name: "Schlager & après-ski",
    parent: "party",
    set: {
      sound: { bpm: [110, 140], signature: ["a sing-along chorus", "an accordion or a synth brass", "a beat for clapping on"], structure: "Choruses the whole hut sings." },
      dance: ["the Fox", "arms linked, swaying"],
      roots: { places: ["Austria", "Germany", "Switzerland"], story: "German-language party songs, from the dance halls to the ski huts." },
      complements: [{ what: "The slopes, the morning after", why: "Après-ski is after the skiing, not instead of it." }],
      app: { theme: "Festival", skin: "Papel picado" },
    },
  },
  {
    id: "wedding",
    name: "Weddings & open format",
    parent: "party",
    set: {
      mood: ["joyful", "every generation on one floor"],
      craft: ["three generations, one arc", "the first dance on time", "a room that is not there for the DJ"],
      app: { theme: "Wedding", skin: "Papel picado" },
    },
  },
];

const deepIds = new Set(DEEP.map((n) => n.id));

/** Every node, by id. */
export const NODES: Record<string, GenomeNode> = Object.fromEntries(
  [...FAMILIES, ...DEEP, ...GENRES.filter((g) => !deepIds.has(g.id)).map(seeded), ...LIGHT].map((n) => [n.id, n]),
);

/** The families, for choosing a broad style first. */
export const FAMILY_IDS = FAMILIES.filter((n) => n.parent === "music").map((n) => n.id);

/** The paths written in depth. */
export const DEEP_IDS = DEEP.map((n) => n.id);
