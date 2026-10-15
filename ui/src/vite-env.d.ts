/// <reference types="svelte" />
/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** `karaoke` in the karaoke build (`.env.karaoke`); unset otherwise. */
  readonly VITE_EDITION?: string;
}
