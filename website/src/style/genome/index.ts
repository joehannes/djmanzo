/** The style genome: `resolveStyle("electroswing")` is EDM's rig through Art Deco. */

import { NODES } from "./nodes";
import { chain, resolveWith } from "./resolve";
import type { StyleTokens } from "./types";

export { DEEP_IDS, FAMILY_IDS, NODES } from "./nodes";
export type { GenomeNode, StageId, StyleTokens } from "./types";

const cache = new Map<string, StyleTokens>();

/** The whole style of a node, with everything it inherits. */
export function resolveStyle(id: string): StyleTokens {
  let s = cache.get(id);
  if (!s) {
    s = resolveWith(NODES, id);
    cache.set(id, s);
  }
  return s;
}

/** The nodes a style comes from, root first. */
export function lineage(id: string): string[] {
  return chain(NODES, id).map((n) => n.id);
}

/** The children of a node, for choosing a genre within a family. */
export function children(id: string): string[] {
  return Object.values(NODES)
    .filter((n) => n.parent === id || n.also?.includes(id))
    .map((n) => n.id);
}
