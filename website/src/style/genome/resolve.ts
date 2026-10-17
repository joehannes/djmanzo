/**
 * Inheritance: a style is its family's, changed by its genre, changed by its
 * subgenre. Objects merge field by field; a list or a tuple is replaced
 * whole, because "the electro-swing room's decor" is not EDM's decor with
 * gramophones appended — it is a different room on the same rig.
 */

import type { GenomeNode, StyleTokens } from "./types";

function isPlainObject(v: unknown): v is Record<string, unknown> {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}

/** `over` laid onto `base`: objects merge, everything else replaces. Neither argument is changed. */
export function merge<T>(base: T, over: unknown): T {
  if (!isPlainObject(base) || !isPlainObject(over)) return (over === undefined ? base : over) as T;
  const out: Record<string, unknown> = { ...base };
  for (const [k, v] of Object.entries(over)) {
    if (v === undefined) continue;
    out[k] = isPlainObject(v) && isPlainObject(out[k]) ? merge(out[k], v) : v;
  }
  return out as T;
}

/** The nodes from the root down to `id`. Throws on an unknown id or a loop. */
export function chain(nodes: Record<string, GenomeNode>, id: string): GenomeNode[] {
  const out: GenomeNode[] = [];
  const seen = new Set<string>();
  let at: string | null = id;
  while (at) {
    const node: GenomeNode | undefined = nodes[at];
    if (!node) throw new Error(`No style is called ${JSON.stringify(at)}.`);
    if (seen.has(at)) throw new Error(`The style ${JSON.stringify(id)} inherits from itself.`);
    seen.add(at);
    out.unshift(node);
    at = node.parent;
  }
  return out;
}

/** The whole style of `id`, with everything it inherits. */
export function resolveWith(nodes: Record<string, GenomeNode>, id: string): StyleTokens {
  const path = chain(nodes, id);
  const merged = path.reduce<Record<string, unknown>>((acc, n) => merge(acc, n.set), {});
  const leaf = path[path.length - 1];
  return { ...(merged as unknown as StyleTokens), id: leaf.id, name: leaf.name, lineage: path.map((n) => n.id) };
}
