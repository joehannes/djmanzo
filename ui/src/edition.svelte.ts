/**
 * §130/§131: what this djmanzo's licence carries, for the interface.
 *
 * Asked of Rust once (`licence_status`); the interface never decides on its
 * own what is in an edition. With no licence in force nothing is missing,
 * because nothing is refused then (`dj_app::gate`).
 */
import { licenceRenew, licenceStatus, type LicenceStatus } from "./api";

/** The module each panel belongs to. A panel not here is the foundation's. */
export const SURFACE_MODULE: Record<string, string> = {
  practice: "teacher",
  event: "events",
  kit: "events",
  plan: "events",
  crowd: "crowd",
  camera: "crowd",
  room: "crowd",
  video: "video",
  karaoke: "karaoke",
  assistant: "assistant",
  next: "assistant",
};

export type Missing = LicenceStatus["missing"][number];

class Edition {
  status = $state<LicenceStatus | null>(null);

  async load(): Promise<void> {
    try {
      this.status = await licenceStatus();
    } catch {
      this.status = null;
    }
    // A fresh licence when one is due, in the background and quietly: a
    // failure here changes nothing (a purchase keeps working regardless).
    if (this.status?.renew_due) {
      void licenceRenew()
        .then((s) => (this.status = s))
        .catch(() => {});
    }
  }

  /** What stands in place of `surface`, when its module is not carried. */
  missingFor(surface: string): Missing | null {
    const module = SURFACE_MODULE[surface];
    if (!module) return null;
    return this.status?.missing?.find((m) => m.module === module) ?? null;
  }
}

export const edition = new Edition();
