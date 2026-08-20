/**
 * Notices when the interface is not keeping up, and says so.
 *
 * The benchmark in ADR-0004 found that when WebKitGTK has no accelerated
 * compositing available, a scrolling waveform drops to ~16 fps — silently. The
 * silence is the real hazard: the application looks broken for no stated reason,
 * and the cause (a compositing fallback three layers down) is not something a
 * user could reasonably guess.
 *
 * So the application measures itself. Cheap: one timestamp per animation frame,
 * a rolling window, and no allocation in the loop.
 */

/** Below this sustained frame rate, something is wrong enough to say so. */
const DEGRADED_FPS = 40;
/** Frames in the rolling window — about a second at 60 fps. */
const WINDOW = 60;
/** Consecutive bad windows before reporting, so a single hitch stays quiet. */
const PATIENCE = 3;

export interface FrameHealth {
  fps: number;
  degraded: boolean;
}

/**
 * Start measuring.
 *
 * `onChange` fires only when the verdict changes, so a healthy application
 * never wakes anything up — right for a banner that appears and disappears.
 *
 * `onSample` fires once per completed window, about once a second, whatever the
 * verdict. Anything that has to *count* — "has it been well for ten seconds
 * yet" — needs this rather than `onChange`: an edge-triggered callback fires at
 * most once per episode, so a counter driven by it never gets past one.
 */
export function watchFrameRate(
  onChange: (health: FrameHealth) => void,
  onSample?: (health: FrameHealth) => void,
): () => void {
  const deltas = new Float32Array(WINDOW);
  let index = 0;
  let filled = 0;
  let previous = performance.now();
  let badWindows = 0;
  let reported = false;
  let frame = 0;

  const tick = () => {
    const now = performance.now();
    deltas[index] = now - previous;
    previous = now;
    index = (index + 1) % WINDOW;
    filled = Math.min(filled + 1, WINDOW);

    if (index === 0 && filled === WINDOW) {
      let total = 0;
      for (let i = 0; i < WINDOW; i++) total += deltas[i];
      const fps = 1000 / (total / WINDOW);
      const degraded = fps < DEGRADED_FPS;
      onSample?.({ fps, degraded });

      if (degraded) {
        badWindows++;
        if (badWindows >= PATIENCE && !reported) {
          reported = true;
          onChange({ fps, degraded: true });
        }
      } else {
        badWindows = 0;
        if (reported) {
          reported = false;
          onChange({ fps, degraded: false });
        }
      }
    }

    frame = requestAnimationFrame(tick);
  };
  frame = requestAnimationFrame(tick);

  return () => cancelAnimationFrame(frame);
}
