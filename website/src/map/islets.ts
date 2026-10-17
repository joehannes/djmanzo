/** Where each genre's islet floats: a ring around the island, the visitor's own genres closer in. */

import { GENRES, type Genre } from "../lib/genres";

export function isletAt(g: Genre, mine: string[]): { x: number; y: number; size: number; mine: boolean } {
  const i = GENRES.indexOf(g);
  const a = (i / GENRES.length) * Math.PI * 2 - Math.PI / 2 + 0.2;
  const rank = mine.indexOf(g.id);
  const isMine = rank >= 0;
  const r = isMine ? 1850 + rank * 60 : 2500 + (i % 2) * 260;
  const size = isMine ? 300 - Math.min(rank, 3) * 30 : 200;
  return { x: Math.cos(a) * r * 1.15, y: Math.sin(a) * r, size, mine: isMine };
}
