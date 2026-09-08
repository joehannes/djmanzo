/**
 * The words the night is described in.
 *
 * Split out of `Night.svelte` for one reason: **typed as total maps, they
 * cannot go stale.** Every one of these is `Record<Union, string>`, so adding a
 * phase in Rust, regenerating the union in `api.ts` and forgetting the label
 * fails the type-check rather than rendering `undefined` into a panel a DJ is
 * reading in a dark room. The same omission with an untyped object literal
 * would be invisible until it happened.
 *
 * The *sentences* are not here. Those come from Rust — see `dj_app::night` —
 * because which fact leads is a judgement about the night rather than a label
 * for a value, and assembling one in a template from four enums produces
 * English nobody says.
 */
import type { Basis, Certainty, SessionPhase, TimeOfDay } from "./api";

/** The arc, in the order a night goes through it. */
export const ARC: { phase: SessionPhase; label: string }[] = [
  { phase: "warm_up", label: "Warm-up" },
  { phase: "heat", label: "Building" },
  { phase: "peak", label: "Peak" },
  { phase: "cooldown", label: "Coming down" },
  { phase: "chill_out", label: "Winding down" },
];

/** Roughly when it is, as a heading rather than as a wire name. */
export const WHEN: Record<TimeOfDay, string> = {
  dawn: "Dawn",
  day: "Daytime",
  dusk: "Dusk",
  night: "Night",
  small_hours: "The small hours",
};

/** What produced the phase, in the words that go after "From". */
export const BASIS: Record<Basis, string> = {
  nothing: "nothing has read it",
  declared: "your occasion",
  measured: "the music",
  agreed: "your occasion and the music",
  disputed: "your occasion, against the music",
};

/** How sure that makes it, spelled for a reader rather than for the wire. */
export const CERTAINTY: Record<Certainty, string> = {
  unsure: "unsure",
  fair: "fair",
  sure: "sure",
};

/** What the assistant may do right now. See `dj_assistant::Warrant`. */
export const WARRANT = {
  nothing: "The assistant is off.",
  watch: "Recording the set, saying nothing.",
  speak: "Offering, never acting.",
  stage: "Staging a deck; not touching anything the room hears.",
  act: "May move a control the room hears.",
  mix: "May perform a transition unasked.",
} as const satisfies Record<string, string>;
