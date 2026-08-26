/**
 * How hard a control should be to hit, given what the night is.
 *
 * A booth is dark and loud and full of people. Ejecting a playing deck at peak
 * time is heard by everyone; doing it alone at home costs nothing. So the same
 * button is a click in one setting and a deliberate hold in the other.
 *
 * # Why this lives here and not in the component
 *
 * Because it is a decision, and decisions are worth testing. The component is
 * left with nothing but a pointer listener and a width.
 *
 * # Why the occasion table is not repeated here
 *
 * `mistakes_are_costly` arrives from the backend, where the occasions are
 * defined. Restating "peak is risky, learning is not" in TypeScript would be a
 * second copy that could disagree with the first, and the disagreement would
 * show up as a control that is hard to press on a night it should not be.
 */

/**
 * How long a destructive control must be held before it fires.
 *
 * Six hundred milliseconds. Long enough that a brush past will not trigger it —
 * an accidental contact is tens of milliseconds — and short enough that it does
 * not feel broken when you mean it. Longer than about a second and people
 * assume the button is dead and press harder.
 */
export const HOLD_MS = 600;

/**
 * Controls that cannot be undone by pressing them again.
 *
 * The test for this list is not "is it important" but "if this fires by
 * accident, can the DJ put it back?". The crossfader is important and
 * reversible; ejecting a playing deck is neither.
 */
export type Destructive = "eject" | "load-over-playing" | "grid-reset";

/**
 * Whether this control needs a deliberate hold right now.
 *
 * Everything non-destructive is always a plain press: making ordinary controls
 * harder would slow a DJ down on every action to guard against a few.
 */
export function needsHold(
  costly: boolean,
  control: Destructive | null,
): boolean {
  return costly && control !== null;
}

/**
 * How far through a hold we are, 0..=1.
 *
 * Clamped at both ends: a clock that jumps backwards — which happens on a
 * suspended laptop, and a DJ's laptop is suspended between every gig — would
 * otherwise produce a negative width and an invisible progress bar at the exact
 * moment the DJ is watching it.
 */
export function holdProgress(startedAt: number, now: number): number {
  if (!Number.isFinite(startedAt) || !Number.isFinite(now)) return 0;
  const through = (now - startedAt) / HOLD_MS;
  return Math.min(1, Math.max(0, through));
}

/** Whether a hold begun at `startedAt` has completed by `now`. */
export function holdComplete(startedAt: number, now: number): boolean {
  return holdProgress(startedAt, now) >= 1;
}

/**
 * What to tell the DJ the control will do.
 *
 * The label changes with the requirement, because a button that needs holding
 * and does not say so reads as broken the first time it is pressed — and the
 * first time is in front of people.
 */
export function hint(costly: boolean, control: Destructive | null): string {
  if (!needsHold(costly, control)) return "";
  return "hold";
}
