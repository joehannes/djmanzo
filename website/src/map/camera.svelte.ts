/**
 * Where the visitor is looking: a point on the island and a zoom. Shared by
 * the map, the platter (the minimap) and the tour.
 */

import { animate } from "motion";

export const MIN_Z = 0.08;
export const MAX_Z = 2.4;

class Camera {
  x = $state(0);
  y = $state(120);
  z = $state(0.4);
  /** Where the visitor's record token rests: the last place they went. */
  tokenX = $state(40);
  tokenY = $state(640);
  flying = $state(false);
  private stop: (() => void) | null = null;

  clampZ(z: number): number {
    return Math.max(MIN_Z, Math.min(MAX_Z, z));
  }

  cancel() {
    this.stop?.();
    this.stop = null;
    this.flying = false;
  }

  /** Glide to a point: up and over for long trips, straight for short ones. */
  flyTo(x: number, y: number, z = this.z, opts: { duration?: number; token?: boolean } = {}): Promise<void> {
    this.cancel();
    const from = { x: this.x, y: this.y, z: this.z, tx: this.tokenX, ty: this.tokenY };
    const dist = Math.hypot(x - from.x, y - from.y);
    const reduced = typeof matchMedia !== "undefined" && matchMedia("(prefers-reduced-motion: reduce)").matches;
    const duration = reduced ? 0.01 : (opts.duration ?? Math.min(2.2, 0.7 + dist / 1800));
    // Zoom out in the middle of a long trip, the way a map application does.
    const peak = Math.min(from.z, z, Math.max(MIN_Z, 0.9 / (1 + dist / 900)));
    this.flying = true;
    return new Promise((resolve) => {
      const ctl = animate(0, 1, {
        duration,
        ease: [0.45, 0, 0.2, 1],
        onUpdate: (t) => {
          this.x = from.x + (x - from.x) * t;
          this.y = from.y + (y - from.y) * t;
          const arc = Math.sin(Math.PI * t);
          const base = from.z + (z - from.z) * t;
          this.z = dist > 600 ? base + (peak - Math.min(from.z, z)) * arc : base;
          if (opts.token !== false) {
            this.tokenX = from.tx + (x - from.tx) * t;
            this.tokenY = from.ty + (y - from.ty) * t;
          }
        },
        onComplete: () => {
          this.flying = false;
          resolve();
        },
      });
      this.stop = () => {
        ctl.stop();
        resolve();
      };
    });
  }
}

export const camera = new Camera();
