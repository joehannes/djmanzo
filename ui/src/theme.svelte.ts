/**
 * Light and dark.
 *
 * # Why the default is dark rather than "follow the system"
 *
 * A booth is dark, and every DJ application is dark, because a white screen at
 * eye level in a dark room destroys the night vision you need to find anything
 * on the actual mixer. Following the operating system would mean the interface
 * turns white at two in the afternoon while someone is preparing a set — fine —
 * and also that a laptop set to light mode goes white the moment it is carried
 * into a club — not fine.
 *
 * So the default is dark, "follow the system" is offered for the people who
 * prepare sets at a desk, and the choice is remembered.
 *
 * # Why this is not purely CSS
 *
 * The waveform is rasterised in Rust and delivered as PNG tiles (ADR-0004), so
 * a stylesheet cannot recolour it. The resolved theme travels in the tile URL,
 * where it forms part of the cache key — see `crates/dj-app/src/waveform.rs`.
 * That is the reason this module exports a resolved value rather than only
 * setting an attribute.
 */

export type ThemePreference = "dark" | "light" | "system";

/** What the interface actually renders as, once "system" has been resolved. */
export type ResolvedTheme = "dark" | "light";

import { themePackages, type ThemePackage } from "./controls/themes/packages";

const STORAGE_KEY = "djmanzo.theme";
const PKG_STORAGE_KEY = "djmanzo.themePackage";

/**
 * The system query, held once.
 *
 * Kept as a module-level handle so the listener is attached a single time no
 * matter how many components read the theme.
 */
const systemPrefersLight =
  typeof window !== "undefined" && typeof window.matchMedia === "function"
    ? window.matchMedia("(prefers-color-scheme: light)")
    : null;

function load(): ThemePreference {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored === "dark" || stored === "light" || stored === "system") {
      return stored;
    }
  } catch {
    // Private browsing, a locked-down webview, a corrupt profile. A theme is
    // not worth failing to start over.
  }
  return "dark";
}

function loadPackage(): string {
  try {
    const stored = localStorage.getItem(PKG_STORAGE_KEY);
    if (stored && themePackages.some(p => p.id === stored)) {
      return stored;
    }
  } catch {}
  return "pkg-organic";
}

class Theme {
  /** What the user asked for. */
  preference = $state<ThemePreference>(load());

  /**
   * Whether the system currently prefers light. Tracked as state so that
   * `resolved` recomputes when the OS setting changes mid-session, which is
   * exactly what someone who chose "system" is asking for.
   */
  #systemLight = $state(systemPrefersLight?.matches ?? false);

  /** The currently selected visual SVG package */
  #pkgId = $state<string>(loadPackage());

  /** The resolved ThemePackage object */
  activePackage = $derived<ThemePackage>(
    themePackages.find(p => p.id === this.#pkgId) ?? themePackages[0]
  );

  /** What is on screen right now. */
  resolved = $derived<ResolvedTheme>(
    this.preference === "system"
      ? this.#systemLight
        ? "light"
        : "dark"
      : this.preference,
  );

  constructor() {
    systemPrefersLight?.addEventListener("change", (event) => {
      this.#systemLight = event.matches;
      this.#apply();
    });
    this.#apply();
  }

  set(preference: ThemePreference) {
    this.preference = preference;
    this.#apply();
    try {
      localStorage.setItem(STORAGE_KEY, preference);
    } catch {
      // As above: the theme still applies for this session.
    }
  }

  setPackage(id: string) {
    this.#pkgId = id;
    try {
      localStorage.setItem(PKG_STORAGE_KEY, id);
    } catch {}
  }

  /**
   * Stamp the resolved theme on the root element.
   *
   * Called from the two places the answer can change rather than from an
   * effect. `main.ts` is a plain `.ts` module, where runes are not compiled, so
   * an effect there is a ReferenceError at load — which takes the whole
   * application down to a blank window. Doing it directly needs no rune
   * context and works before anything is mounted, which is also what stops the
   * first frame from flashing dark on a light theme.
   *
   * The stylesheet keys off `data-theme`; `color-scheme` is what makes the
   * webview's own furniture — scrollbars, focus rings, default form control
   * rendering — follow along. Setting only the former leaves light scrollbars
   * on a dark page.
   */
  #apply() {
    const root = document.documentElement;
    root.dataset.theme = this.resolved;
    root.style.colorScheme = this.resolved;
  }
}

export const theme = new Theme();
