<script lang="ts">
  /**
   * The sidebar: everything, playlists, folders, and what has been played.
   *
   * # Why a flat list rendered with indentation
   *
   * The tree comes from Rust flat, with parent ids, and is nested here. A
   * recursive component would be tidier to read and would re-mount every node
   * whenever anything moved; this walks the list once per change and the DOM
   * stays put, which is what keeps a drag from flickering.
   */
  import {
    addToPlaylist,
    createPlaylist,
    deletePlaylist,
    listPlaylists,
    movePlaylist,
    renamePlaylist,
    type Playlist,
  } from "./api";

  /** What the browser is currently showing. */
  export type Selection =
    | { kind: "all" }
    | { kind: "history" }
    | { kind: "duplicates" }
    | { kind: "playlist"; id: number; name: string }
    | { kind: "smart"; id: number; name: string; query: string };

  let {
    selection = $bindable(),
    onchange,
  }: { selection: Selection; onchange?: () => void } = $props();

  let nodes = $state<Playlist[]>([]);
  let error = $state<string | null>(null);
  /** Which node is being renamed, and to what. */
  let editing = $state<{ id: number; name: string } | null>(null);
  /** Node being dragged, for reparenting. */
  let dragging = $state<number | null>(null);

  /**
   * The drag payload for tracks coming from the browser.
   *
   * A custom type rather than `text/plain`, so a drop knows what it is being
   * given: reparenting a folder and filing tracks into a list are different
   * operations that happen to land on the same rows, and guessing between them
   * from the payload's *shape* is how a folder ends up with a track id in it.
   */
  const TRACKS = "application/x-djmanzo-tracks";

  /** Which node a track drag is currently over, so the target is visible. */
  let over = $state<number | null>(null);

  export async function refresh() {
    try {
      nodes = await listPlaylists();
      error = null;
    } catch (e) {
      error = String(e);
    }
  }

  /**
   * Depth-first, so indentation can be computed once rather than by walking
   * parents on every render.
   */
  const tree = $derived.by(() => {
    const byParent = new Map<number | null, Playlist[]>();
    for (const node of nodes) {
      const siblings = byParent.get(node.parent_id) ?? [];
      siblings.push(node);
      byParent.set(node.parent_id, siblings);
    }

    const out: Array<{ node: Playlist; depth: number }> = [];
    const walk = (parent: number | null, depth: number) => {
      // Guard against a cycle the database should not contain: without it a
      // bad parent chain would hang the interface rather than show a wrong
      // tree, and a hung interface is much harder to diagnose.
      if (depth > 12) return;
      for (const node of byParent.get(parent) ?? []) {
        out.push({ node, depth });
        walk(node.id, depth + 1);
      }
    };
    walk(null, 0);
    return out;
  });

  function select(next: Selection) {
    selection = next;
    onchange?.();
  }

  async function add(kind: "list" | "folder" | "smart") {
    // Inside the selected node when it is a folder, top level otherwise.
    // Creating a playlist inside a playlist is refused by Rust, and offering it
    // here only to have it fail would be worse than not offering it.
    const current = selection;
    const parent =
      (current.kind === "playlist" || current.kind === "smart") &&
      nodes.find((n) => n.id === current.id)?.kind === "folder"
        ? current.id
        : null;
    const name =
      kind === "folder" ? "New folder" : kind === "smart" ? "New filter" : "New playlist";
    try {
      const id = await createPlaylist(name, parent, kind);
      await refresh();
      // Straight into rename: a sidebar full of "New playlist" is what happens
      // when naming is a separate step nobody takes.
      editing = { id, name };
    } catch (e) {
      error = String(e);
    }
  }

  async function commitRename() {
    if (!editing) return;
    const { id, name } = editing;
    editing = null;
    if (!name.trim()) return;
    try {
      await renamePlaylist(id, name);
      await refresh();
    } catch (e) {
      error = String(e);
    }
  }

  async function remove(node: Playlist) {
    try {
      await deletePlaylist(node.id);
      if (
        (selection.kind === "playlist" || selection.kind === "smart") &&
        selection.id === node.id
      ) {
        select({ kind: "all" });
      }
      await refresh();
    } catch (e) {
      error = String(e);
    }
  }

  /**
   * Tracks dropped onto a list.
   *
   * Folders are not lists — a folder holds lists, and putting tracks in one
   * would need it to be both. Smart folders are not lists either: their
   * contents are a *query*, and adding a track by hand would produce a member
   * the filter does not select, which is a lie about what the folder means.
   */
  async function dropTracks(event: DragEvent, target: Playlist): Promise<void> {
    const payload = event.dataTransfer?.getData(TRACKS);
    over = null;
    if (!payload || target.kind !== "list") return;
    event.preventDefault();
    event.stopPropagation();
    try {
      const ids: string[] = JSON.parse(payload);
      for (const id of ids) await addToPlaylist(target.id, id);
      onchange?.();
    } catch (e) {
      error = String(e);
    }
  }

  /** Whether a drag carries tracks, as opposed to a node being reparented. */
  const carriesTracks = (event: DragEvent) =>
    event.dataTransfer?.types.includes(TRACKS) ?? false;

  async function drop(target: Playlist | null) {
    if (dragging == null) return;
    const moved = dragging;
    dragging = null;
    try {
      // Rust refuses a move that would put a node inside itself; the message
      // it returns is the one shown, rather than a guess made here.
      await movePlaylist(moved, target?.id ?? null);
      await refresh();
    } catch (e) {
      error = String(e);
    }
  }

  $effect(() => {
    void refresh();
  });
</script>

<nav class="crates" aria-label="Playlists">
  <IconButton class="entry" active={selection.kind === "all"} onClick={() => select({ kind: "all" })}>All tracks</IconButton>
  <IconButton class="entry" active={selection.kind === "history"} onClick={() => select({ kind: "history" })}>History</IconButton>
  <!--
    Duplicates is a *view* of the collection rather than a crate, so it belongs
    up here beside "All tracks" and not in the tree a DJ built.
  -->
  <IconButton class="entry" active={selection.kind === "duplicates"} onClick={() => select({ kind: "duplicates" })}>Duplicates</IconButton>

  <div class="divider"></div>

  <!--
    Dropping on the background moves a node to the top level, which is the only
    way back out of a folder once something is inside one.
  -->
  <div
    class="list"
    role="tree"
    tabindex="-1"
    ondragover={(e) => e.preventDefault()}
    ondrop={(e) => {
      // Tracks dropped on the empty space below the tree land nowhere: there
      // is no list there, and silently filing them into the last one a DJ
      // happened to click would be worse than doing nothing.
      if (carriesTracks(e)) return;
      void drop(null);
    }}
  >
    {#each tree as { node, depth } (node.id)}
      <div
        class="row"
        class:active={(selection.kind === "playlist" || selection.kind === "smart") &&
          selection.id === node.id}
        style="padding-left: {0.4 + depth * 0.8}rem"
        role="treeitem"
        aria-selected={(selection.kind === "playlist" || selection.kind === "smart") &&
          selection.id === node.id}
        tabindex="-1"
        class:targeted={over === node.id}
        draggable="true"
        ondragstart={() => (dragging = node.id)}
        ondragover={(e) => {
          if (carriesTracks(e)) {
            // Only a plain list can take tracks, so only a plain list says yes.
            // A target that accepts a drop it cannot honour is worse than one
            // that visibly refuses.
            if (node.kind !== "list") return;
            e.preventDefault();
            over = node.id;
            return;
          }
          if (node.kind === "folder") e.preventDefault();
        }}
        ondragleave={() => {
          if (over === node.id) over = null;
        }}
        ondrop={(e) => {
          if (carriesTracks(e)) {
            void dropTracks(e, node);
            return;
          }
          e.stopPropagation();
          void drop(node);
        }}
      >
        {#if editing?.id === node.id}
          <!-- svelte-ignore a11y_autofocus -->
          <input
            class="rename"
            bind:value={editing.name}
            autofocus
            onblur={commitRename}
            onkeydown={(e) => {
              if (e.key === "Enter") commitRename();
              if (e.key === "Escape") editing = null;
            }}
          />
        {:else}
          <button
            class="entry"
            onclick={() =>
              select(
                node.kind === "smart"
                  ? {
                      kind: "smart",
                      id: node.id,
                      name: node.name,
                      query: node.query ?? "",
                    }
                  : { kind: "playlist", id: node.id, name: node.name },
              )}
            ondblclick={() => (editing = { id: node.id, name: node.name })}
            title={node.kind === "folder"
              ? "Folder"
              : node.kind === "smart"
                ? `Smart folder: ${node.query ?? "no filter"}`
                : `${node.track_count} tracks`}
          >
            <span class="icon"
              >{node.kind === "folder" ? "▸" : node.kind === "smart" ? "⁂" : "≡"}</span
            >
            <span class="name">{node.name}</span>
            {#if node.kind === "list"}
              <span class="count">{node.track_count}</span>
            {/if}
          </button>
          <IconButton
            icon="fa-solid fa-xmark"
            title={node.kind === "folder"
              ? "Delete this folder and everything in it. The tracks stay in your collection."
              : "Delete this playlist. The tracks stay in your collection."}
            aria-label={`Delete ${node.name}`}
            onClick={() => remove(node)}
          />
        {/if}
      </div>
    {/each}
  </div>

  <div class="actions">
    <IconButton icon="fa-solid fa-list" title="New playlist" onClick={() => add("list")} />
    <IconButton icon="fa-solid fa-folder-plus" title="New folder" onClick={() => add("folder")} />
    <IconButton icon="fa-solid fa-filter" title="New smart folder — a filter that keeps itself up to date" onClick={() => add("smart")} />
  </div>

  {#if error}
    <p class="error">{error}</p>
  {/if}
</nav>

<style>
  .crates {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    min-width: 11rem;
    max-width: 14rem;
    min-height: 0;
    font-size: 0.85em;
  }

  .list {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
  }

  /*
    The target a drop would land on. Shown while dragging rather than only on
    release, because a DJ aiming at a row in a list of twenty needs to know
    which one they have before they let go.
  */
  .row.targeted {
    outline: 2px solid var(--accent-2, #22d3aa);
    outline-offset: -2px;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 0.15rem;
  }

  /*
    `flex: 1` belongs only to an entry inside a `.row`, where it shares a
    horizontal line with the delete button. The two top-level entries are direct
    children of a *column* flex container, where the same rule makes them grow
    vertically — which stretched "All tracks" into a block and squeezed the tree
    out of the panel.
  */
  .entry {
    flex: none;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 0.35rem;
    text-align: left;
    background: none;
    border: none;
    border-radius: 4px;
    padding: 0.22rem 0.4rem;
    color: var(--text-dim);
    font: inherit;
    cursor: pointer;
  }

  .entry:hover {
    background: var(--panel-raised);
    color: var(--text);
  }

  .entry.active,
  .row.active .entry {
    background: var(--accent-2);
    color: var(--on-accent);
  }

  .row .entry {
    flex: 1;
  }

  .icon {
    opacity: 0.6;
    flex: none;
  }

  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .count {
    margin-left: auto;
    font-variant-numeric: tabular-nums;
    opacity: 0.65;
    font-size: 0.85em;
  }

  .rename {
    flex: 1;
    min-width: 0;
    font: inherit;
    padding: 0.1rem 0.3rem;
  }

  /* Only on hover: a delete button beside every row is a mis-click waiting. */
  .remove {
    visibility: hidden;
    padding: 0 0.3rem;
    line-height: 1.2;
  }

  .row:hover .remove {
    visibility: visible;
  }

  .divider {
    height: 1px;
    background: var(--border);
    margin: 0.3rem 0;
  }

  .actions {
    display: flex;
    gap: 0.3rem;
    flex: none;
  }

  .actions button {
    flex: 1;
    font-size: 0.9em;
    padding: 0.15rem 0.3rem;
  }

  .error {
    margin: 0;
    font-size: 0.85em;
    color: var(--danger, #dc2626);
  }
</style>
