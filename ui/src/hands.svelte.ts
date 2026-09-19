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

/**
 * Whether the on-screen platter should be given room. §53's other prominence
 * judgement, and the same shape as the stem one.
 *
 * The deck's wheel is deliberately small — about 70 px — and the reasoning is
 * in `Deck.svelte`: the waveform above answers *where am I* better than a
 * circle does, and a full row for it cost more than the waveform got in three
 * of the four shipped arrangements. That reasoning assumes the hand has
 * somewhere better to be. For a DJ whose controller has **no jog at all** it
 * does not: a pad-and-fader controller puts their hands on the pads, the
 * faders and the EQ, and leaves the one gesture a platter is for — nudging a
 * record back into time — as the only thing on that deck they have to find
 * with a mouse. A 70-pixel target is the wrong size for the one control the
 * hardware cannot reach.
 *
 * **It only ever grows.** A controller *with* a platter makes the screen's
 * circle a readout, and the obvious symmetry would be to shrink it and hand
 * the pixels to the waveform — but that is contextual *demotion*, which §3
 * refuses and §17 is built never to do: nothing a DJ can see is taken away by
 * a piece of hardware being plugged in. So a platter on the desk changes
 * nothing on screen, and the adaptation has one direction.
 *
 * **False when nothing is plugged in**, for the reason `wantsStemsOpen` gives:
 * a laptop-only DJ is the case this interface is already designed around, and
 * the default is theirs. The adaptation is for the DJ whose hardware has a
 * gap.
 */
export function wantsJogRoom(): boolean {
  return hands.reach !== null && hands.reach.jogs === 0;
}
