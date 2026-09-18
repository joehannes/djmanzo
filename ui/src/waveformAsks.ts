/**
 * When the waveform has anything new to ask Rust about.
 *
 * # The defect this exists for
 *
 * `waveform_info` is deliberately not on the 60 Hz snapshot. Its own doc says
 * so: *"Sixty times a second for a curve that changes twice a track would be
 * the snapshot pump carrying furniture."* The interface did it anyway.
 *
 * The lane and the overview each ran an `$effect` that read
 * `deck.length_frames` and `deck.analysis` and then called `waveform_info`.
 * Those reads look like fine-grained dependencies and are not: `App.svelte`
 * replaces the whole snapshot on every frame — `snapshot = next` — so every
 * deck is a *new* `$state` proxy and every read through it is a new signal.
 * The effect therefore re-ran on every frame the pump sent, which during
 * playback is all of them.
 *
 * Measured rather than reasoned: ten frames of ordinary playback, thirty
 * milliseconds apart, produced **forty** `waveform_info` calls — two
 * components over two decks, every frame. At the pump's rate that is about
 * two hundred and forty IPC round trips a second, each one a library query for
 * saved loops and a clone of the whole energy trajectory.
 *
 * # Why a key rather than better dependencies
 *
 * Making the effect genuinely fine-grained would mean patching the snapshot in
 * place rather than replacing it, which is a change to how every panel in the
 * application receives state. This is the local fix: the effect still runs on
 * every frame — building a short string and comparing it is nothing — and the
 * *call* happens only when one of the things the answer depends on has moved.
 *
 * One function rather than one per component, because the lane and the
 * overview must agree about when there is something new to ask: two keys that
 * drifted would be two views of one record disagreeing about which loops are
 * saved in it.
 */
import type { DeckState } from "./api";

/**
 * What `waveform_info`'s answer depends on, as a string.
 *
 * Primitives only — `deck.analysis` is an object and would stringify to
 * `[object Object]` for every record ever analysed, which is a key that never
 * changes and a lane that never updates.
 *
 * - **number**: which deck is being asked about.
 * - **length_frames**: a different record is on the deck.
 * - **the grid**: analysis landed, or the DJ moved the grid. The mix windows
 *   are beats counted from it, so they move when it does.
 * - **marks**: the marks stored *with the record* changed — a saved loop or a
 *   hand-edited grid. See `dj_app::snapshot::Marks`. Nothing else here moves
 *   when a DJ keeps a loop mid-set, and a second grid nudge does not move
 *   `grid_confidence` because the first one already took it to certain.
 *
 * Deliberately **not** here: the playhead, the pitch fader, the crossfader and
 * everything else that moves sixty times a second. The bands are beats counted
 * from a grid; none of them cares where the needle is.
 */
export function waveformAsks(deck: DeckState): string {
  return [
    deck.number,
    deck.length_frames,
    // The analyser's answer, which appears when analysis lands.
    deck.analysis?.bpm ?? "-",
    deck.analysis?.key_camelot ?? "-",
    // And the grid actually in force, which is `grid_confidence`'s whole
    // reason for existing beside the one above: a hand-edited grid reads as
    // certain the moment it is edited.
    deck.grid_confidence,
    deck.marks,
  ].join("/");
}
