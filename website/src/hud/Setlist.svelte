<!--
  The setlist taped to the booth: every place on the island and every genre
  islet, as a plain list. It is the way round for anyone who would rather not
  drag a map — and for screen readers, the map's table of contents.
-->
<script lang="ts">
  import { PLACES, type PlaceId } from "../lib/places";
  import type { Genre } from "../lib/genres";
  import { profile } from "../lib/profile.svelte";

  interface Props {
    onplace: (id: PlaceId) => void;
    ongenre: (g: Genre) => void;
    onclose: () => void;
    genres: Genre[];
    admin: boolean;
  }
  let { onplace, ongenre, onclose, genres, admin }: Props = $props();
</script>

<div class="scrim" onclick={onclose} aria-hidden="true"></div>
<div class="paper" role="dialog" aria-label="Setlist: every place on the island">
  <h2>Tonight's setlist</h2>
  <ol>
    {#each PLACES.filter((p) => !p.hidden || admin) as p (p.id)}
      <li>
        <button onclick={() => onplace(p.id)}>
          <span class="e" aria-hidden="true">{p.emoji}</span>
          <span><b>{p.name}</b> — {p.what}</span>
          {#if profile.p.stamps.includes(p.id)}<span class="done" title="Visited">✓</span>{/if}
          {#if p.pad}<kbd>{p.pad}</kbd>{/if}
        </button>
      </li>
    {/each}
  </ol>
  <h3>The genre islets</h3>
  <ul class="genres">
    {#each genres as g (g.id)}
      <li><button onclick={() => ongenre(g)}>{g.emoji} {g.name}</button></li>
    {/each}
  </ul>
  <p class="small">Keys: 1–8 jump to the pads, arrows move the map, + and − zoom, Esc closes.</p>
  <button class="close" onclick={onclose}>Close</button>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.35);
    z-index: 49;
  }
  .paper {
    position: fixed;
    top: 70px;
    right: 14px;
    width: min(420px, calc(100vw - 20px));
    max-height: calc(100dvh - 180px);
    overflow: auto;
    padding: 22px 22px 18px;
    background: #fbf7ec;
    color: #1d1d1d;
    border-radius: 4px;
    rotate: 1deg;
    box-shadow: var(--shadow);
    z-index: 50;
    background-image: repeating-linear-gradient(#fbf7ec 0 29px, #d9e6f2 29px 30px);
  }
  h2 {
    font-family: "Bungee", sans-serif;
    color: #c0263b;
  }
  h3 {
    margin-top: 14px;
    font-family: "Bungee", sans-serif;
  }
  ol {
    padding-left: 0;
    list-style: none;
    margin: 0;
  }
  li button {
    display: flex;
    gap: 10px;
    align-items: center;
    width: 100%;
    text-align: left;
    background: none;
    border: 0;
    padding: 6px 4px;
    color: inherit;
    cursor: pointer;
    font-size: 15px;
    border-radius: 6px;
  }
  li button:hover {
    background: rgba(0, 0, 0, 0.06);
  }
  .e {
    font-size: 20px;
  }
  kbd {
    margin-left: auto;
    font: 700 11px/1 system-ui;
    padding: 3px 6px;
    border: 1px solid #999;
    border-radius: 4px;
  }
  .done {
    color: #1f7a5c;
    font-weight: 800;
  }
  .genres {
    list-style: none;
    padding: 0;
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .genres button {
    width: auto;
    border: 1px solid #bbb;
    border-radius: 999px;
    padding: 4px 10px;
    font-size: 14px;
  }
  .close {
    margin-top: 8px;
    border: 2px solid #1d1d1d;
    background: none;
    border-radius: 10px;
    padding: 8px 16px;
    font-weight: 700;
    cursor: pointer;
    color: #1d1d1d;
  }
  .small {
    font-size: 13px;
    color: #555;
  }
</style>
