/**
 * The island's own soundtrack: the visitor's genre on deck one, quietly,
 * while they wander. Off until they ask — a website that starts making
 * noise uninvited is a website that gets closed.
 */

import { getEngine, hasEngine } from "../lib/audio/engine";
import { profile } from "../lib/profile.svelte";

class IslandSound {
  on = $state(false);

  async start() {
    const e = getEngine();
    await e.wake();
    const a = profile.genre?.id ?? "house";
    const b = profile.secondGenre?.id ?? (a === "bachata" ? "merengue" : "bachata");
    e.decks[0].setGenre(a);
    e.decks[1].setGenre(b);
    const g = e.decks[0].genre;
    e.setTempo((g.bpm[0] + g.bpm[1]) / 2);
    e.setCrossfader(0);
    e.setVolume(0.35);
    e.decks[0].start();
    this.on = true;
    profile.set("sound", true);
  }

  stop() {
    if (hasEngine()) {
      const e = getEngine();
      e.decks.forEach((d) => d.stop());
    }
    this.on = false;
    profile.set("sound", false);
  }

  toggle() {
    return this.on ? this.stop() : this.start();
  }

  /** The visitor changed their genre: the island's music follows. */
  follow() {
    if (!this.on || !hasEngine()) return;
    const e = getEngine();
    const g = profile.genre;
    if (g) {
      e.decks[0].setGenre(g.id);
      e.setTempo((g.bpm[0] + g.bpm[1]) / 2);
    }
  }
}

export const islandSound = new IslandSound();
