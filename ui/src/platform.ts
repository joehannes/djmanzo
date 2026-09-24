/**
 * §117: the chords every desktop application answers, on each system's own
 * modifier.
 *
 * > i want very natural and typical (even per system linux/windows/mac)
 * > shortcuts
 *
 * ⌘ on a Mac, Ctrl on Linux and Windows — the same key in each system's
 * hand. Find is `F`, settings is `,` (macOS's Preferences and, since, most
 * cross-platform applications'), the list of keys is `/` (GNOME asks for
 * Ctrl+? for the shortcuts window) and `F1` (help, on Windows and GNOME). The
 * palette, `K`, is the palette's own. `1`–`9` with the modifier switch
 * activities, as they switch tabs in every browser, and work while typing,
 * where the bare digits type. Sources in docs/RESEARCH.md.
 *
 * A DJ's own keyboard mapping may bind any of these; when it does, the
 * mapping has already taken the key and these stand aside.
 */

/** Whether this is a Mac, where ⌘ does what Ctrl does elsewhere. */
export function onMac(platform: string = typeof navigator === "undefined" ? "" : navigator.platform): boolean {
  return /Mac|iPhone|iPad/.test(platform);
}

/** The modifier's name, as the guide writes it. */
export function modName(mac = onMac()): string {
  return mac ? "⌘" : "Ctrl";
}

/** One chord, its key by position, and the leaf it runs. */
export interface Chord {
  code: string;
  label: string;
  run: string;
}

/** The chords, with the modifier. `K` is listed for the guide; the palette answers it itself. */
export const CHORDS: Chord[] = [
  { code: "KeyK", label: "Find anything", run: "ui palette" },
  { code: "KeyF", label: "Search the library", run: "ui search" },
  { code: "Comma", label: "Settings", run: "surface settings" },
  { code: "Slash", label: "Every key", run: "surface keys" },
];

/**
 * What a key event runs as a platform chord, or null. `activities` are the
 * slugs of the first nine, in order.
 */
export function chordRun(event: KeyboardEvent, activities: string[], mac = onMac()): string | null {
  const plain = !event.ctrlKey && !event.metaKey && !event.altKey && !event.shiftKey;
  if (plain && event.code === "F1") return "surface keys";
  const mod = mac ? event.metaKey && !event.ctrlKey : event.ctrlKey && !event.metaKey;
  if (!mod || event.altKey) return null;
  const digit = /^Digit([1-9])$/.exec(event.code);
  if (digit && !event.shiftKey) {
    const slug = activities[Number(digit[1]) - 1];
    return slug ? `switch activity ${slug}` : null;
  }
  const chord = CHORDS.find((each) => each.code === event.code);
  if (!chord || chord.code === "KeyK") return null;
  // Ctrl+? is Ctrl+Shift+/ on most layouts; both are the list of keys.
  if (event.shiftKey && chord.code !== "Slash") return null;
  return chord.run;
}
