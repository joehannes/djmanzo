/**
 * A record carried from the collection, by drag and drop inside djmanzo.
 *
 * The library and the cards already carry the *ids* of what is dragged
 * (`application/x-djmanzo-tracks`, which the crates read to file it). A deck
 * or the karaoke host's form needs more than an id to act at once: the file
 * to load, and the words to show. So the dragged record goes along as well,
 * under a type of its own, and nothing that reads the ids is disturbed.
 *
 * The desktop's own drags (a file from the Finder) are Tauri's and arrive
 * through `dragdrop.svelte.ts`; this is the other half, the drags the page
 * itself starts.
 */

export const TRACK = "application/x-djmanzo-track";

/** What a dragged record carries: enough to load it and to name it. */
export interface DraggedTrack {
  id: string;
  path: string;
  title: string;
  artist: string;
}

/** Carry `track` on a drag that is starting. */
export function carryTrack(event: DragEvent, track: DraggedTrack): void {
  const carried: DraggedTrack = { id: track.id, path: track.path, title: track.title, artist: track.artist };
  event.dataTransfer?.setData(TRACK, JSON.stringify(carried));
}

/** Whether a drag over something carries a record, before it is dropped. */
export function carriesTrack(event: DragEvent): boolean {
  return Array.from(event.dataTransfer?.types ?? []).includes(TRACK);
}

/** The record a drop carried, or `null` for anything else, or anything malformed. */
export function droppedTrack(event: DragEvent): DraggedTrack | null {
  const raw = event.dataTransfer?.getData(TRACK);
  if (!raw) return null;
  try {
    const value = JSON.parse(raw) as Partial<DraggedTrack>;
    if (typeof value.id !== "string" || typeof value.path !== "string" || !value.path) return null;
    return {
      id: value.id,
      path: value.path,
      title: typeof value.title === "string" ? value.title : "",
      artist: typeof value.artist === "string" ? value.artist : "",
    };
  } catch {
    return null;
  }
}
