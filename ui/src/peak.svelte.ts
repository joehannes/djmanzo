/**
 * §17 at the peak, as the owner chose (17 October): when djmanzo reads the
 * night as its peak — and only between mixes — each deck's effects fold
 * opens and the stem chips grow a little.
 *
 * `opened` stays true for the rest of the run once the peak has opened the
 * folds: djmanzo never closes what it opened, nor anything the DJ opened, so
 * an eased peak leaves them as they are. `closed` is the decks whose fold the
 * DJ closed while it was held open; theirs stays closed for the night.
 * `now` is the peak itself, for the stem chips, and goes when it does.
 */
export const peak = $state({
  now: false,
  opened: false,
  closed: [] as string[],
});

/** The peak as the shell reads it: a peak sure enough to act on, between mixes. */
export function readPeak(phase: string | null, certainty: string | null, audible: number): void {
  const at = phase === "peak" && certainty !== null && certainty !== "unsure";
  peak.now = at;
  if (at && audible < 2) peak.opened = true;
}
