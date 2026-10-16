/**
 * §127: one deck slim, the other given the room.
 *
 * > as of karaoke, there should be a special feature, enabling one mixer of
 * > 2 to be minimized/hidden and the other more useful bigger (playlist
 * > visible??)
 *
 * Which deck is slim, if any — one at most, and only with two decks on
 * screen: with four or six there is no "the other one" to give the room to.
 * Remembered in this window's storage, as a layout choice is, and never
 * required: a storage that throws leaves both decks as they were.
 */

const KEY = "djmanzo.slimDeck";

function read(): number | null {
  try {
    const raw = globalThis.localStorage?.getItem(KEY);
    const n = raw === null || raw === undefined ? NaN : Number(raw);
    return n === 1 || n === 2 ? n : null;
  } catch {
    return null;
  }
}

export const deckFocus = $state<{ slim: number | null }>({ slim: read() });

/** Make deck `deck` slim, or with `null` give both decks their room back. */
export function setSlim(deck: number | null) {
  deckFocus.slim = deck === 1 || deck === 2 ? deck : null;
  try {
    if (deckFocus.slim === null) globalThis.localStorage?.removeItem(KEY);
    else globalThis.localStorage?.setItem(KEY, String(deckFocus.slim));
  } catch {
    // A choice that does not stick is a small annoyance, not a fault.
  }
}
