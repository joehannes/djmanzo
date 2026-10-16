/**
 * What an older webview lacks, filled in before anything else runs.
 *
 * The karaoke build reaches back to a 2011 Mac laptop, whose last macOS
 * (10.13, High Sierra) carries the WebKit of Safari 13. Its syntax is
 * lowered at build time (`vite build --mode karaoke`, `safari13`); what
 * cannot be lowered is a function that does not exist, and two that the
 * interface and Svelte's own runtime call arrived only in Safari 15.4:
 * `structuredClone` and `Object.hasOwn`; and Web Audio there is only
 * `webkitAudioContext`. Each is defined here only when the
 * webview has none, so a current one keeps its own.
 */

/** A deep copy of what the interface clones: data from Rust and its own state. */
function cloneOf<T>(value: T, seen = new Map<unknown, unknown>()): T {
  if (value === null || typeof value !== "object") return value;
  if (seen.has(value)) return seen.get(value) as T;
  if (value instanceof Date) return new Date(value.getTime()) as T;
  if (value instanceof RegExp) return new RegExp(value.source, value.flags) as T;
  if (ArrayBuffer.isView(value)) return (value as unknown as { slice(): unknown }).slice() as T;
  if (value instanceof ArrayBuffer) return value.slice(0) as T;
  if (value instanceof Map) {
    const out = new Map();
    seen.set(value, out);
    value.forEach((v, k) => out.set(cloneOf(k, seen), cloneOf(v, seen)));
    return out as T;
  }
  if (value instanceof Set) {
    const out = new Set();
    seen.set(value, out);
    value.forEach((v) => out.add(cloneOf(v, seen)));
    return out as T;
  }
  if (Array.isArray(value)) {
    const out: unknown[] = [];
    seen.set(value, out);
    for (const item of value) out.push(cloneOf(item, seen));
    return out as T;
  }
  const out: Record<string, unknown> = {};
  seen.set(value, out);
  for (const key of Object.keys(value)) out[key] = cloneOf((value as Record<string, unknown>)[key], seen);
  return out as T;
}

export function installLegacy(scope: typeof globalThis = globalThis): void {
  const g = scope as unknown as Record<string, unknown>;
  if (typeof g.structuredClone !== "function") {
    g.structuredClone = <T>(value: T) => cloneOf(value);
  }
  // Safari before 14.1 has Web Audio only under its prefixed name.
  if (typeof g.AudioContext !== "function" && typeof g.webkitAudioContext === "function") {
    g.AudioContext = g.webkitAudioContext;
  }
  const object = g.Object as unknown as Record<string, unknown>;
  if (typeof object.hasOwn !== "function") {
    object.hasOwn = (target: object, key: PropertyKey) => Object.prototype.hasOwnProperty.call(target, key);
  }
}

installLegacy();

export { cloneOf };
