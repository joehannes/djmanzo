/**
 * What a visitor has told the island, and what the island makes of it.
 *
 * Kept in the browser first (so nothing is asked twice and nothing needs an
 * account) and copied to the visitor's account once they sign in, so
 * another browser knows it too. Every adaptation on the island reads
 * this one object: the colours, the loop, the order of the tour, the
 * edition the box office opens on.
 */

import type { Edition } from "../../shared/catalog";
import { GENRE_BY_ID, ISLAND, type Genre } from "./genres";
import { load, save } from "./store";
import type { PlaceId } from "./places";

export type Role = "dj" | "karaoke" | "both" | "learning" | "curious";
export type Venue = "bedroom" | "parties" | "bar" | "club" | "weddings" | "karaoke-bar" | "radio" | "festival" | "church";
export type Gear = "laptop" | "controller" | "turntables" | "cdjs" | "mic" | "none";
export type Experience = "new" | "some" | "pro";
export type DaylightMode = "auto" | "system" | "day" | "night";

export interface Profile {
  name: string;
  role: Role | null;
  venues: Venue[];
  genres: string[];
  gear: Gear[];
  experience: Experience | null;
  /** Questions the MC has asked and had answered (or skipped). */
  asked: string[];
  /** Places visited, for the passport. */
  stamps: PlaceId[];
  sound: boolean;
  daylight: DaylightMode;
  /** The island's prize code, once claimed. */
  prize: string | null;
  /** The MC has finished the first conversation. */
  welcomed: boolean;
}

const EMPTY: Profile = {
  name: "",
  role: null,
  venues: [],
  genres: [],
  gear: [],
  experience: null,
  asked: [],
  stamps: [],
  sound: false,
  daylight: "auto",
  prize: null,
  welcomed: false,
};

const KEY = "islamanzo.profile";

class ProfileState {
  p = $state<Profile>(load(KEY, EMPTY));

  set<K extends keyof Profile>(key: K, value: Profile[K]) {
    this.p[key] = value;
    this.persist();
  }

  toggle<K extends "venues" | "genres" | "gear">(key: K, value: Profile[K][number]) {
    const list = this.p[key] as string[];
    const i = list.indexOf(value as string);
    if (i >= 0) list.splice(i, 1);
    else list.push(value as string);
    this.persist();
  }

  stamp(id: PlaceId) {
    if (!this.p.stamps.includes(id)) {
      this.p.stamps.push(id);
      this.persist();
    }
  }

  answered(q: string) {
    if (!this.p.asked.includes(q)) this.p.asked.push(q);
    this.persist();
  }

  reset() {
    this.p = structuredClone(EMPTY);
    this.persist();
  }

  persist() {
    save(KEY, $state.snapshot(this.p));
  }

  get genre(): Genre | null {
    return GENRE_BY_ID[this.p.genres[0]] ?? null;
  }

  get secondGenre(): Genre | null {
    return GENRE_BY_ID[this.p.genres[1]] ?? null;
  }

  get palette() {
    return this.genre?.palette ?? ISLAND;
  }
}

export const profile = new ProfileState();

export interface Recommendation {
  edition: Edition;
  why: string;
}

/**
 * Which edition fits, and the sentence that says why. Pure, so the box
 * office, the MC and the tests agree.
 */
export function recommend(p: Pick<Profile, "role" | "venues" | "gear" | "experience" | "genres">): Recommendation {
  const paid = p.venues.some((v) => ["bar", "club", "weddings", "radio", "festival", "karaoke-bar"].includes(v));
  const karaokeRoom = p.venues.includes("karaoke-bar");
  const proGear = p.gear.includes("turntables") || p.gear.includes("cdjs");
  if (p.role === "both" || (p.role === "dj" && karaokeRoom) || (p.role === "karaoke" && (proGear || p.venues.includes("club")))) {
    return { edition: "full", why: "You do both — the DJ booth and the karaoke stage — so you want both halves in one application." };
  }
  if (p.role === "karaoke") {
    return { edition: "karaoke", why: "You host singers: the singers' screen, the queue, vocals out and a microphone per singer are the heart of your nights." };
  }
  if (p.role === "learning" || (p.role === "dj" && p.experience === "new")) {
    return { edition: "apprentice", why: "You're learning: the Apprentice edition adds the assistant and the teacher — the coach, the practice deck, the quiz — to everything a DJ needs." };
  }
  if (p.role === "dj" && (paid || proGear || p.experience === "pro")) {
    const because = proGear ? "you play on turntables or CDJs" : paid ? "you play for a paying room" : "you play professionally";
    return { edition: "pro", why: `Because ${because}: Pro DJ adds stems, decks three to six, timecode vinyl, any controller, video and going live, the assistant and the autopilot.` };
  }
  return { edition: "hobbyist", why: "You're mixing for yourself and your friends — Hobbyist is everything basic a DJ needs, and it costs least." };
}

/**
 * The order the tour visits the island in. Everyone starts at the dock and
 * ends at the box office; in between, what the visitor cares about comes first.
 */
export function tourOrder(p: Pick<Profile, "role" | "genres">): PlaceId[] {
  const middle: PlaceId[] =
    p.role === "karaoke"
      ? ["tarima", "cabina", "tienda", "gigs", "faro", "arena", "plaza", "muro"]
      : p.role === "both"
        ? ["cabina", "tarima", "tienda", "gigs", "faro", "arena", "plaza", "muro"]
        : ["cabina", "tienda", "faro", "gigs", "arena", "tarima", "plaza", "muro"];
  return ["muelle", ...middle, "taquilla"];
}
