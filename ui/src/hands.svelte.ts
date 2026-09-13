/**
 * §53: what the controller now open puts under the DJ's hands.
 *
 * > The UI should know: number of decks actually controllable, available
 * > physical knobs, jogs, pads, stem controls […] Use that to determine which
 * > GUI surfaces deserve prominence.
 *
 * Held here rather than fetched per component for the reason the §8 rune gives:
 * a deck's stem module and the Controllers panel are asking the same question,
 * and four decks each asking Rust separately would be four answers that can
 * disagree — with the stems on deck 2 folded and the stems on deck 3 open.
 *
 * # Polled, because a controller is plugged in mid-set
 *
 * Slowly. This is the one question in the application whose answer changes when
 * somebody physically touches the machine, which is a scale of seconds, not of
 * frames — and the answer is a lookup over a table Rust already holds, so the
 * cost is a round trip rather than work.
 */

import { controllerHands, type Hands } from "./api";

/** How often it re-asks. */
const EVERY_MS = 5_000;

export const hands = $state({
  /** The open controller's reach, or `null` when nothing is open. */
  reach: null as Hands | null,
});

let timer: ReturnType<typeof setInterval> | undefined;

/** Ask once, and keep asking. Safe to call from several components. */
export function watchHands(): () => void {
  const ask = () => {
    void controllerHands()
      .then((reach) => {
        hands.reach = reach;
      })
      .catch(() => {
        // No controller is the ordinary answer and the one this falls back to,
        // so a failed round trip costs the adaptation rather than the panel.
        hands.reach = null;
      });
  };
  ask();
  clearInterval(timer);
  timer = setInterval(ask, EVERY_MS);
  return () => clearInterval(timer);
}

/**
 * Whether the stem module should be given room. §53's own worked example.
 *
 * > If a controller has dedicated stem pads: compact GUI stem panel. If there
 * > are no stem controls: expand stem controls.
 *
 * **False when nothing is plugged in**, which is the half worth stating: a
 * laptop-only DJ is already the case the interface is designed around, and
 * unfolding a 370-pixel module for them would be §53 answering a question
 * nobody asked. The adaptation is for the DJ whose hardware has a gap.
 */
export function wantsStemsOpen(): boolean {
  return hands.reach !== null && !hands.reach.stems;
}
