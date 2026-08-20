import { watchFrameRate, type FrameHealth } from "./framerate";

export type PerformanceLevel = "Eco" | "Balanced" | "Ultra" | "Auto";
export type ResolvedPerformance = "Eco" | "Balanced" | "Ultra";

const STORAGE_KEY = "djmanzo.performance";

/**
 * Healthy one-second windows before stepping back up a tier.
 *
 * Ten seconds. Long enough that a single hitch — a track loading, a window
 * being dragged — does not bounce the interface straight back into the load
 * that caused it, short enough that a DJ who wondered where the glow went is
 * not left wondering for a whole track.
 */
const RECOVERY_WINDOWS = 10;

function load(): PerformanceLevel {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored === "Eco" || stored === "Balanced" || stored === "Ultra" || stored === "Auto") {
      return stored;
    }
  } catch {
    // Ignore storage errors
  }
  return "Auto";
}

class PerformanceGovernor {
  /** The user's requested level */
  preference = $state<PerformanceLevel>(load());
  
  /** 
   * The active level decided by the governor if preference is "Auto". 
   * Starts at Ultra, but downgrades if frame drops are detected.
   */
  #autoResolved = $state<ResolvedPerformance>("Ultra");
  
  /**
   * Consecutive healthy *windows*, not frames.
   *
   * The first version counted callbacks from `watchFrameRate`'s edge-triggered
   * `onChange` and waited for 600 of them. That callback fires at most once per
   * degradation episode, so the counter never reached two and the advertised
   * auto-recovery could not happen at all: once the governor stepped down it
   * stayed down for the rest of the session. It now counts one-second windows,
   * which is a thing that actually arrives once a second.
   */
  #healthyWindows = 0;

  /** What the UI actually renders as */
  resolved = $derived<ResolvedPerformance>(
    this.preference === "Auto" ? this.#autoResolved : this.preference
  );

  constructor() {
    if (typeof window !== "undefined") {
      watchFrameRate(
        () => {},
        (health) => this.sample(health),
      );
    }
  }

  /**
   * One second's verdict.
   *
   * Down a tier immediately, up a tier slowly. The asymmetry is the point: a
   * dropped frame is felt at once and is worth reacting to at once, while
   * stepping back up too eagerly gives an interface that oscillates between
   * two looks — which is more distracting than either of them.
   *
   * Public so it can be tested without a browser; nothing else should call it.
   */
  sample(health: FrameHealth) {
    if (this.preference !== "Auto") return;

    if (health.degraded) {
      this.#healthyWindows = 0;
      if (this.#autoResolved === "Ultra") this.#autoResolved = "Balanced";
      else if (this.#autoResolved === "Balanced") this.#autoResolved = "Eco";
      return;
    }

    if (this.#autoResolved === "Ultra") return;
    this.#healthyWindows += 1;
    if (this.#healthyWindows < RECOVERY_WINDOWS) return;

    this.#healthyWindows = 0;
    this.#autoResolved = this.#autoResolved === "Eco" ? "Balanced" : "Ultra";
  }

  set(preference: PerformanceLevel) {
    this.preference = preference;
    
    // Reset auto-governor if they switch back to Auto manually
    if (preference === "Auto") {
      this.#autoResolved = "Ultra";
    }

    try {
      localStorage.setItem(STORAGE_KEY, preference);
    } catch {
      // Ignore
    }
  }
}

export const performance = new PerformanceGovernor();
