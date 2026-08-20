/**
 * Which sound card, remembered.
 *
 * # Why this exists
 *
 * A DJ opening djmanzo on a laptop expects it to make sound. Until now the
 * first launch put a device in the dropdown and waited to be told to open it,
 * so loading a track and pressing play did nothing — with no visible reason,
 * because the interface looked exactly the same as one that was connected.
 * Every other DJ application opens the default output on launch, and so does
 * this one now.
 *
 * # Why localStorage rather than the backend
 *
 * It is a preference of this window, in the same family as the theme and the
 * motion setting, and those already live here. A device *choice* is not
 * something the engine needs to know between runs — the engine is told which
 * device to open every time either way.
 *
 * # Why the remembered device is checked against what is present
 *
 * Hardware moves. A DJ who played through a controller last night and opened
 * the laptop on a train has a stored device id that no longer exists, and
 * opening it would fail with a message about a device they are not currently
 * holding. So the stored choice is only used when it is still there.
 */

const KEY = "djmanzo.audio";

export interface AudioPreference {
  /** The main output. `null` means "whatever is default". */
  device: string | null;
  /** The second device carrying the headphone cue, if any. */
  cue: string | null;
  bufferFrames: number;
}

const FALLBACK: AudioPreference = { device: null, cue: null, bufferFrames: 256 };

/** Buffer sizes the interface offers. Anything else is somebody else's typo. */
const BUFFERS = [64, 128, 256, 512, 1024, 2048];

/** What was remembered, or the defaults. Never throws. */
export function readAudioPreference(store: Storage | null = safeStorage()): AudioPreference {
  if (!store) return { ...FALLBACK };
  try {
    const raw = store.getItem(KEY);
    if (!raw) return { ...FALLBACK };
    const parsed = JSON.parse(raw) as Partial<AudioPreference>;
    return {
      device: typeof parsed.device === "string" ? parsed.device : null,
      cue: typeof parsed.cue === "string" ? parsed.cue : null,
      // Validated rather than trusted: a hand-edited or half-written value
      // reaching the audio backend as a buffer size is a crash, not a typo.
      bufferFrames: BUFFERS.includes(parsed.bufferFrames as number)
        ? (parsed.bufferFrames as number)
        : FALLBACK.bufferFrames,
    };
  } catch {
    return { ...FALLBACK };
  }
}

/** Remember a choice. Never throws — a full or blocked store is not fatal. */
export function writeAudioPreference(
  preference: AudioPreference,
  store: Storage | null = safeStorage(),
): void {
  if (!store) return;
  try {
    store.setItem(KEY, JSON.stringify(preference));
  } catch {
    // A DJ whose preference does not stick has a small annoyance. One whose
    // application will not start has a problem.
  }
}

/**
 * Which device to open, given what is actually plugged in.
 *
 * Returns `null` only when there is nothing to open at all.
 */
export function deviceToOpen(
  remembered: string | null,
  present: { id: string; is_default: boolean }[],
): string | null {
  if (remembered && present.some((device) => device.id === remembered)) return remembered;
  return present.find((device) => device.is_default)?.id ?? present[0]?.id ?? null;
}

/**
 * Whether the remembered device is gone.
 *
 * Worth saying out loud rather than silently falling back: "playing through
 * the laptop speakers because your interface is not here" and "playing through
 * the laptop speakers because that is what you chose" look identical, and only
 * one of them is a surprise.
 */
export function deviceMissing(
  remembered: string | null,
  present: { id: string }[],
): boolean {
  return remembered !== null && !present.some((device) => device.id === remembered);
}

function safeStorage(): Storage | null {
  try {
    return typeof localStorage === "undefined" ? null : localStorage;
  } catch {
    return null;
  }
}
