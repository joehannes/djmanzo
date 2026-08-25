/**
 * The icon set, drawn here rather than fetched.
 *
 * `index.html` used to pull Font Awesome from `cdnjs.cloudflare.com`. In a
 * packaged build that fails twice over: the application's own CSP is
 * `default-src 'self'`, which blocks the stylesheet outright, and a DJ at a
 * venue has no internet anyway. Every icon button rendered as an empty square.
 *
 * These are original drawings on a 24x24 grid, stroked with `currentColor` so
 * they take the theme, at a weight that stays legible at the 2.6rem button
 * size the interface uses. Drawing them here also keeps the question of icon
 * licensing from arising at all -- see ADR-0002.
 *
 * Names are the Font Awesome ones the call sites already use, so
 * `fa-solid fa-gear` and `gear` both resolve; see `normalizeIconName`.
 */

/** One drawing: a path, and whether it is filled rather than stroked. */
export type IconGlyph = {
  d: string;
  fill?: boolean;
};


export const ICONS: Record<string, IconGlyph> = {
  "backward": { d: "M20 6v12l-8-6 8-6zM11 6v12l-8-6 8-6z", fill: true },
  "ban": { d: "M12 4a8 8 0 1 0 0 16a8 8 0 1 0 0 -16zM6.3 6.3l11.4 11.4" },
  "book": { d: "M6 3h11a1 1 0 0 1 1 1v14H8a2 2 0 0 0 -2 2V3zM6 20a2 2 0 0 1 2 -2h10" },
  "check": { d: "M4 12.5l5 5L20 6" },
  "circle": { d: "M12 4a8 8 0 1 0 0 16a8 8 0 1 0 0 -16z" },
  "circle-question": { d: "M12 4a8 8 0 1 0 0 16a8 8 0 1 0 0 -16zM9.7 9.6a2.4 2.4 0 0 1 4.6 .9c0 1.6-2.3 2-2.3 3.5M12 17.2h.01" },
  "cloud": { d: "M7 18h10a4 4 0 0 0 0 -8 6 6 0 0 0 -11.7 1.7A3.5 3.5 0 0 0 7 18z" },
  "cog": { d: "M12 9a3 3 0 1 0 0 6a3 3 0 1 0 0 -6zM12 5.4a6.6 6.6 0 1 0 0 13.2a6.6 6.6 0 1 0 0 -13.2zM18.6 12.0L21.0 12.0M16.7 16.7L18.4 18.4M12.0 18.6L12.0 21.0M7.3 16.7L5.6 18.4M5.4 12.0L3.0 12.0M7.3 7.3L5.6 5.6M12.0 5.4L12.0 3.0M16.7 7.3L18.4 5.6" },
  "compact-disc": { d: "M12 4a8 8 0 1 0 0 16a8 8 0 1 0 0 -16zM12 9.5a2.5 2.5 0 1 0 0 5.0a2.5 2.5 0 1 0 0 -5.0z" },
  "compress": { d: "M9 4v5H4M15 4v5h5M9 20v-5H4M15 20v-5h5" },
  "desktop": { d: "M3 5h18v11H3zM9 20h6M12 16v4" },
  "eraser": { d: "M8 20h12M4 15l7-7 6 6-6 6H6z" },
  "expand": { d: "M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5" },
  "eye": { d: "M2 12s3.5-6 10-6 10 6 10 6-3.5 6-10 6-10-6-10-6zM12 9.5a2.5 2.5 0 1 0 0 5.0a2.5 2.5 0 1 0 0 -5.0z" },
  "eye-slash": { d: "M2 12s3.5-6 10-6c1.6 0 3 .35 4.3.9M22 12s-3.5 6-10 6c-1.6 0-3-.35-4.3-.9M4 4l16 16" },
  "file-import": { d: "M14 3H6v18h12V7l-4-4zM14 3v4h4M2 12h8M7 9l3 3-3 3" },
  "file-lines": { d: "M6 3h8l4 4v14H6zM14 3v4h4M9 12h6M9 16h6" },
  "filter": { d: "M3 5h18l-7 8v6l-4 2v-8L3 5z" },
  "flag": { d: "M6 21V4M6 5h12l-2.5 4L18 13H6" },
  "flag-checkered": { d: "M6 21V4M6 5h12v8H6zM6 5h3v4H6zM12 5h3v4h-3zM9 9h3v4H9zM15 9h3v4h-3z" },
  "floppy-disk": { d: "M4 4h13l3 3v13H4zM8 4v6h8V4M8 20v-6h8v6" },
  "folder-open": { d: "M3 19V6h6l2 3h7v3M3 19l3-7h17l-3 7H3z" },
  "folder-plus": { d: "M3 19V6h6l2 3h10v10H3zM12 11v6M9 14h6" },
  "gear": { d: "M12 9a3 3 0 1 0 0 6a3 3 0 1 0 0 -6zM12 5.4a6.6 6.6 0 1 0 0 13.2a6.6 6.6 0 1 0 0 -13.2zM18.6 12.0L21.0 12.0M16.7 16.7L18.4 18.4M12.0 18.6L12.0 21.0M7.3 16.7L5.6 18.4M5.4 12.0L3.0 12.0M7.3 7.3L5.6 5.6M12.0 5.4L12.0 3.0M16.7 7.3L18.4 5.6" },
  "guitar": { d: "M9 11a5 5 0 1 0 0 10a5 5 0 1 0 0 -10zM9 14.2a1.8 1.8 0 1 0 0 3.6a1.8 1.8 0 1 0 0 -3.6zM13 12l7-7M17 4l3 3" },
  "hand": { d: "M8 12V5.5a1.5 1.5 0 0 1 3 0V11m0-.5V4.5a1.5 1.5 0 0 1 3 0V11m0-.5V5.5a1.5 1.5 0 0 1 3 0V13m0-3.5a1.5 1.5 0 0 1 3 0V16a5 5 0 0 1 -5 5h-2a6 6 0 0 1 -6 -6v-3a1.6 1.6 0 0 1 3 -.8" },
  "hand-paper": { d: "M8 12V5.5a1.5 1.5 0 0 1 3 0V11m0-.5V4.5a1.5 1.5 0 0 1 3 0V11m0-.5V5.5a1.5 1.5 0 0 1 3 0V13m0-3.5a1.5 1.5 0 0 1 3 0V16a5 5 0 0 1 -5 5h-2a6 6 0 0 1 -6 -6v-3a1.6 1.6 0 0 1 3 -.8" },
  "hand-pointer": { d: "M10 11V4.5a1.5 1.5 0 0 1 3 0V11m0-.5a1.5 1.5 0 0 1 3 0V13m0-1.5a1.5 1.5 0 0 1 3 0V16a5 5 0 0 1 -5 5h-2a6 6 0 0 1 -6 -6v-3a1.6 1.6 0 0 1 3 -.8V11" },
  "headphones": { d: "M4 15v-3a8 8 0 0 1 16 0v3M4 14h3v6H5a1 1 0 0 1 -1 -1v-5zM20 14h-3v6h2a1 1 0 0 0 1 -1v-5z" },
  "image": { d: "M4 5h16v14H4zM4 16l4.5-4.5 3.5 3.5 3-3L20 16M9 8.1a1.4 1.4 0 1 0 0 2.8a1.4 1.4 0 1 0 0 -2.8z" },
  "industry": { d: "M3 20V10l6 4V10l6 4V6h6v14H3z" },
  "keyboard": { d: "M3 7h18v10H3zM7 10h.01M11 10h.01M15 10h.01M8 14h8" },
  "layer-group": { d: "M12 3l9 5-9 5-9-5 9-5zM3 13l9 5 9-5M3 17l9 5 9-5" },
  "leaf": { d: "M5 19c0-8 5-13 15-14 1 10-4 15-11 15a4 4 0 0 1 -4 -1zM7 19c3-4 6-6 9-7" },
  "list": { d: "M4 6h16M4 12h16M4 18h16" },
  "location-dot": { d: "M12 21s7-6.3 7-11a7 7 0 1 0 -14 0c0 4.7 7 11 7 11zM12 7.5a2.5 2.5 0 1 0 0 5.0a2.5 2.5 0 1 0 0 -5.0z" },
  "lock": { d: "M7 11h10v9H7zM9 11V8a3 3 0 0 1 6 0v3" },
  "magnifying-glass": { d: "M11 5a6 6 0 1 0 0 12a6 6 0 1 0 0 -12zM15.5 15.5L21 21" },
  "microphone": { d: "M12 3a3 3 0 0 1 3 3v6a3 3 0 0 1 -6 0V6a3 3 0 0 1 3 -3zM6 11a6 6 0 0 0 12 0M12 17v4M9 21h6" },
  "minus": { d: "M5 12h14" },
  "moon": { d: "M20 15.5A8.5 8.5 0 0 1 9.5 4a8.5 8.5 0 1 0 10.5 11.5z" },
  "palette": { d: "M12 3a9 9 0 0 0 0 18 2 2 0 0 0 1.6 -3.2 2 2 0 0 1 1.6 -3.2H18a3 3 0 0 0 3 -3c0-4.8-4-8.6-9-8.6zM8 8.8a1.2 1.2 0 1 0 0 2.4a1.2 1.2 0 1 0 0 -2.4zM12 6.3a1.2 1.2 0 1 0 0 2.4a1.2 1.2 0 1 0 0 -2.4zM15.5 8.8a1.2 1.2 0 1 0 0 2.4a1.2 1.2 0 1 0 0 -2.4z" },
  "paperclip": { d: "M20 11l-8.5 8.5a5 5 0 0 1 -7 -7l9-9a3.5 3.5 0 0 1 5 5l-9 9a2 2 0 0 1 -3 -3l8-8" },
  "plus": { d: "M12 5v14M5 12h14" },
  "repeat": { d: "M6 9V8a3 3 0 0 1 3 -3h9M18 15v1a3 3 0 0 1 -3 3H6M15 2l3 3-3 3M9 16l-3 3 3 3" },
  "robot": { d: "M6 9h12v9H6zM12 9V5M9 5h6M9 13h.01M15 13h.01M4 12v3M20 12v3" },
  "rotate-left": { d: "M4 10a8 8 0 1 1 2 6M4 4v6h6" },
  "stop": { d: "M7 7h10v10H7z", fill: true },
  "sun": { d: "M12 7.5a4.5 4.5 0 1 0 0 9.0a4.5 4.5 0 1 0 0 -9.0zM12.0 5.0L12.0 2.5M16.9 7.1L18.7 5.3M19.0 12.0L21.5 12.0M16.9 16.9L18.7 18.7M12.0 19.0L12.0 21.5M7.1 16.9L5.3 18.7M5.0 12.0L2.5 12.0M7.1 7.1L5.3 5.3" },
  "table-cells": { d: "M4 5h16v14H4zM4 12h16M9.3 5v14M14.7 5v14" },
  "th": { d: "M4 5h16v14H4zM4 12h16M9.3 5v14M14.7 5v14" },
  "toggle-off": { d: "M8 7h8a5 5 0 0 1 0 10H8A5 5 0 0 1 8 7zM8 9.5a2.5 2.5 0 1 0 0 5.0a2.5 2.5 0 1 0 0 -5.0z" },
  "toggle-on": { d: "M8 7h8a5 5 0 0 1 0 10H8A5 5 0 0 1 8 7zM16 9.5a2.5 2.5 0 1 0 0 5.0a2.5 2.5 0 1 0 0 -5.0z" },
  "trash": { d: "M4 7h16M9 7V5h6v2M6 7l1 13h10l1-13M10 11v6M14 11v6" },
  "unlink": { d: "M9 15l-2 2a3.5 3.5 0 0 1 -5 -5l2-2M15 9l2-2a3.5 3.5 0 0 1 5 5l-2 2M4 4l16 16" },
  "water": { d: "M2 8c3-2.5 5-2.5 8 0s5 2.5 8 0M2 13c3-2.5 5-2.5 8 0s5 2.5 8 0M2 18c3-2.5 5-2.5 8 0s5 2.5 8 0" },
  "xmark": { d: "M6 6l12 12M18 6L6 18" },
};

/**
 * Accept either `gear` or the `fa-solid fa-gear` the call sites already pass.
 *
 * Kept tolerant on purpose: the interface has some thirty call sites written
 * against Font Awesome's names, and rewriting them all to change where the
 * drawing comes from would be a much larger diff for no behavioural gain.
 */
export function normalizeIconName(name: string): string {
  return name
    .split(/\s+/)
    .map((part) => part.replace(/^fa-(solid|regular|brands|light|thin|duotone)$/, ""))
    .filter(Boolean)
    .map((part) => part.replace(/^fa-/, ""))
    .find((part) => part.length > 0) ?? "";
}

/** The drawing for `name`, or `undefined` if there is none. */
export function iconGlyph(name: string): IconGlyph | undefined {
  return ICONS[normalizeIconName(name)];
}
