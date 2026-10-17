/**
 * Day and night on the island.
 *
 * "auto" follows the visitor's own clock: full night until five, a dawn to
 * seven, day until five in the afternoon, a long golden hour, night by eight.
 * "system" follows the operating system's light or dark setting. "day" and
 * "night" are what they say. The crossfader in the corner moves between them.
 *
 * The result is one number, 0 (night) to 1 (day), and the hour's name; the
 * theming mixes every colour by it, so dusk is a real in-between and not a
 * switch.
 */

import { profile } from "./profile.svelte";

export function sunlight(hours: number): number {
  // Piecewise: night, dawn, day, golden hour, dusk.
  if (hours < 5 || hours >= 20) return 0;
  if (hours < 7) return (hours - 5) / 2;
  if (hours < 17) return 1;
  return 1 - (hours - 17) / 3;
}

export function hourName(hours: number): string {
  if (hours < 5) return "after hours";
  if (hours < 7) return "sunrise set";
  if (hours < 12) return "morning";
  if (hours < 17) return "afternoon";
  if (hours < 20) return "golden hour";
  if (hours < 23) return "peak time";
  return "after hours";
}

class Daylight {
  now = $state(new Date());
  systemDark = $state(typeof matchMedia !== "undefined" ? matchMedia("(prefers-color-scheme: dark)").matches : true);

  constructor() {
    if (typeof window === "undefined") return;
    setInterval(() => (this.now = new Date()), 60_000);
    matchMedia("(prefers-color-scheme: dark)").addEventListener("change", (e) => (this.systemDark = e.matches));
  }

  get hours(): number {
    return this.now.getHours() + this.now.getMinutes() / 60;
  }

  /** 0 = night, 1 = day. */
  get level(): number {
    switch (profile.p.daylight) {
      case "day":
        return 1;
      case "night":
        return 0;
      case "system":
        return this.systemDark ? 0 : 1;
      default:
        return sunlight(this.hours);
    }
  }

  get name(): string {
    const m = profile.p.daylight;
    if (m === "day") return "day";
    if (m === "night") return "night";
    if (m === "system") return this.systemDark ? "night (system)" : "day (system)";
    return hourName(this.hours);
  }
}

export const daylight = new Daylight();
