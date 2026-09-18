/**
 * The four currents, in the one order.
 *
 * `dj_core::Stem::index` fixes that order — vocal, drums, bass, other — and
 * the separator, the parameter registry, the pad pages and the interleaved
 * stem frame all index by it, so two modules disagreeing about whether drums
 * come first is not possible on the Rust side. This is the same table on this
 * side of the bridge.
 *
 * It lived inside `Stems.svelte` until §25's `stems` layer needed it on the
 * waveform too. A second copy would have been two descriptions of one order,
 * which is the way round that ends with the bass fader coloured like the
 * vocal band.
 */

/** What a DJ calls each, for a label. */
export const STEM_LABELS = ["Vocals", "Drums", "Bass", "Other"] as const;

/** What the action vocabulary calls each, for a command. */
export const STEM_KEYS = ["vocal", "drums", "bass", "other"] as const;

/**
 * One token per stem, never a hex value.
 *
 * These were four literal colours once, which made the stem pads the only
 * controls in djmanzo that ignored the theme. §30's `must_differ_from` holds
 * the four apart in every palette, so anything drawn in them is legible as
 * four things.
 */
export const STEM_COLORS = [
  "var(--stem-vocal)",
  "var(--stem-drums)",
  "var(--stem-bass)",
  "var(--stem-other)",
] as const;
