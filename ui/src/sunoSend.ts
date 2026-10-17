/**
 * Karaoke beta B5: a guest's song from Suno's window to WhatsApp, in Rust
 * (`dj_app::suno`). Session B's own calls, kept out of `api.ts`
 * (docs/karaoke_beta_release.md §4).
 */
import { invoke } from "@tauri-apps/api/core";

export interface Filling {
  /** The language of the words put in. */
  language: string;
  /** Suno's window fills the form by itself once the DJ has signed in and Create opens. */
  waits: boolean;
}

/** Fill Suno's form in djmanzo's Suno window; the DJ presses Create there. */
export const sunoFill = (id: string, language: string) => invoke<Filling>("suno_fill", { id, language });

/** The song's link, when djmanzo's Suno window shows a song; else none. */
export const sunoLink = () => invoke<string | null>("suno_link");

/** WhatsApp Web in djmanzo's own window, the message written; the DJ sends. */
export const whatsappHere = (id: string, link: string) => invoke<string>("guests_whatsapp_here", { id, link });
