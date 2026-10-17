<!--
  A genre's islet: where the music comes from, the rooms, the clothes, the
  art, the dances, how its DJs mix, and the theme and skin djmanzo ships for
  that room. "Make this my island" repaints everything.
-->
<script lang="ts">
  import type { Genre } from "../lib/genres";
  import type { PlaceId } from "../lib/places";
  import { profile } from "../lib/profile.svelte";
  import { islandSound } from "../hud/sound.svelte";
  import { getEngine } from "../lib/audio/engine";

  let { genre, go }: { genre: Genre; go: (id: PlaceId) => void } = $props();
  const mine = $derived(profile.p.genres[0] === genre.id);

  function makeMine() {
    const rest = profile.p.genres.filter((g) => g !== genre.id);
    profile.set("genres", [genre.id, ...rest]);
    islandSound.follow();
  }

  let previewing = $state(false);
  async function preview() {
    const e = getEngine();
    await e.wake();
    if (previewing) {
      e.decks[1].stop();
      previewing = false;
      return;
    }
    e.decks[1].setGenre(genre.id);
    e.setTempo((genre.bpm[0] + genre.bpm[1]) / 2);
    e.setCrossfader(1);
    e.setVolume(0.45);
    e.decks[1].start();
    previewing = true;
  }

  const SHOTS: Record<string, string> = { "Bachata Rosa": "bachata", "Industrial Techno": "techno", "Bioluminescent Bay": "bay", "Karaoke Neon": "neon", "Malecón Night": "malecon" };
  const shot = $derived(SHOTS[genre.theme]);
</script>


<div class="actions">
  <button class="btn primary" onclick={makeMine} disabled={mine}>{mine ? "✓ This is your island" : `Make ${genre.name} my island`}</button>
  <button class="btn" onclick={preview} aria-pressed={previewing}>{previewing ? "■ Stop the groove" : "▶ Hear a sketch of the groove"}</button>
</div>
<p class="muted small">{genre.bpm[0]}–{genre.bpm[1]} BPM. The groove is synthesised in your browser from the genre's pattern — a sketch of its rhythm, not its sound.</p>

<h2>The rooms</h2>
<p>{genre.rooms}</p>
<h2>What people wear</h2>
<p>{genre.style}</p>
<h2>The art</h2>
<p>{genre.art}</p>
<h2>The dances</h2>
<ul class="dances">
  {#each genre.dances as d (d.name)}
    <li><b>{d.name}.</b> {d.how}</li>
  {/each}
</ul>
<h2>How its DJs mix</h2>
<p>{genre.mixing}</p>
<p>Effects they reach for: {#each genre.fx as f, i (f)}<span class="fx">{f}</span>{i < genre.fx.length - 1 ? " " : ""}{/each}. Try them in <button class="link" onclick={() => go("cabina")}>the booth</button>.</p>

<h2>In djmanzo</h2>
<p>The theme for this room is <b>{genre.theme}</b>{#if genre.skin}, worn with the <b>{genre.skin}</b> skin{/if}. {genre.skin ? "Both ship" : "It ships"} with the application.</p>
{#if shot}
  <figure>
    <img class="shot" src="/shots/{shot}.webp" alt="djmanzo in the {genre.theme} theme" loading="lazy" width="1280" height="800" />
    <figcaption>djmanzo in {genre.theme}, as it looks today.</figcaption>
  </figure>
{/if}

<style>
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-bottom: 6px;
  }
  .dances {
    padding-left: 18px;
  }
  .dances li {
    margin-bottom: 6px;
  }
  .fx {
    display: inline-block;
    padding: 1px 9px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--primary) 25%, transparent);
    font-weight: 700;
  }
  .link {
    background: none;
    border: 0;
    padding: 0;
    color: var(--primary);
    text-decoration: underline;
    cursor: pointer;
    font: inherit;
  }
  h2 {
    margin-top: 18px;
    font-size: 1.25rem;
  }
</style>
