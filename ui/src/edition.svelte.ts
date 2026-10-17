/**
 * §130/§131: which edition the licence names, and what it carries. (The
 * build — the karaoke build for older Macs — is `edition.ts`.)
 *
 * Rust decides (`crate::licence`): it refuses a command the licence does not
 * carry, and the panels, activities, arrangements, palette and tree under
 * Space it hands over already leave such things out. This is for the few
 * places the interface draws a module's controls without asking Rust for a
 * list first — a deck's stem controls, the deck counts — so they are not
 * drawn as buttons that would be refused.
 *
 * Full until Rust has answered, as every run was before editions: a control
 * missing for the second it takes to ask is worse than one that would be
 * refused for that second.
 */
import { edition as fetchEdition, type Edition } from "./api";

export const editionNow = $state<{ value: Edition | null; loaded: boolean }>({
  value: null,
  loaded: false,
});

/** Whether this run carries `module` (a `dj_core::edition::Module` slug). */
export function has(module: string): boolean {
  const value = editionNow.value;
  return value === null || value.modules.includes(module);
}

/** Ask Rust once; a failure leaves Full, which is what the gate allows anyway. */
export async function loadEdition(): Promise<void> {
  try {
    editionNow.value = await fetchEdition();
  } catch {
    editionNow.value = null;
  }
  editionNow.loaded = true;
}
