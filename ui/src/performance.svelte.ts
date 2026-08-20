import { watchFrameRate } from "./framerate";

export type PerformanceLevel = "Eco" | "Balanced" | "Ultra" | "Auto";
export type ResolvedPerformance = "Eco" | "Balanced" | "Ultra";

const STORAGE_KEY = "djmanzo.performance";

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
  
  /** Tracks consecutive healthy frames to allow slow recovery from a one-shot spike */
  #healthyTicks = 0;

  /** What the UI actually renders as */
  resolved = $derived<ResolvedPerformance>(
    this.preference === "Auto" ? this.#autoResolved : this.preference
  );

  constructor() {
    if (typeof window !== "undefined") {
      watchFrameRate((health) => {
        if (this.preference !== "Auto") return;

        if (health.degraded) {
          // Instant downgrade on lag
          this.#healthyTicks = 0;
          if (this.#autoResolved === "Ultra") {
            this.#autoResolved = "Balanced";
            console.warn(`[PerformanceGovernor] Frame rate dropped to ${Math.round(health.fps)}fps. Stepping down to Balanced mode.`);
          } else if (this.#autoResolved === "Balanced") {
            this.#autoResolved = "Eco";
            console.warn(`[PerformanceGovernor] Frame rate still dropping (${Math.round(health.fps)}fps). Stepping down to Eco mode.`);
          }
        } else {
          // Slow recovery if the spike was truly a one-shot event
          this.#healthyTicks++;
          // ~10 seconds of perfect frames (600 frames) to step back up one tier
          if (this.#healthyTicks > 600) {
            if (this.#autoResolved === "Eco") {
              this.#autoResolved = "Balanced";
              this.#healthyTicks = 0;
              console.info(`[PerformanceGovernor] Frame rate stable. Recovering to Balanced mode.`);
            } else if (this.#autoResolved === "Balanced") {
              this.#autoResolved = "Ultra";
              this.#healthyTicks = 0;
              console.info(`[PerformanceGovernor] Frame rate stable. Recovering to Ultra mode.`);
            }
          }
        }
      });
    }
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
