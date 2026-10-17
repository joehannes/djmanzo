<!--
  Eight hot-cue pads, the way a DJ jumps to a point in a record: press one
  and the island takes you to that place. The number keys do the same.
-->
<script lang="ts">
  import { PLACES, type PlaceId } from "../lib/places";
  import { profile } from "../lib/profile.svelte";

  interface Props {
    current: PlaceId | null;
    onplace: (id: PlaceId) => void;
  }
  let { current, onplace }: Props = $props();
  const pads = PLACES.filter((p) => p.pad).sort((a, b) => a.pad! - b.pad!);
  const short: Record<string, string> = { muelle: "Dock", cabina: "Booth", tienda: "Records", tarima: "Karaoke", faro: "Visuals", taquilla: "Buy", muro: "Wishes", radio: "Blog" };
</script>

<nav class="pads" aria-label="Hot cues: the island's places">
  {#each pads as p, i (p.id)}
    <button
      class="pad"
      class:lit={current === p.id}
      class:visited={profile.p.stamps.includes(p.id)}
      style="--hue:{i % 2 ? 'var(--secondary)' : 'var(--primary)'}"
      onclick={() => onplace(p.id)}
      aria-label="{p.pad}: {p.name}, {p.what}"
      aria-current={current === p.id ? "location" : undefined}
    >
      <span class="num" aria-hidden="true">{p.pad}</span>
      <span class="emoji" aria-hidden="true">{p.emoji}</span>
      <span class="lbl">{short[p.id]}</span>
    </button>
  {/each}
</nav>

<style>
  .pads {
    position: fixed;
    left: 50%;
    bottom: max(12px, env(safe-area-inset-bottom));
    transform: translateX(-50%);
    display: grid;
    grid-template-columns: repeat(8, 64px);
    gap: 8px;
    padding: 10px;
    border-radius: 18px;
    background: color-mix(in srgb, #0c0d10 82%, transparent);
    border: 2px solid rgba(255, 255, 255, 0.08);
    box-shadow: var(--shadow);
    backdrop-filter: blur(10px);
    z-index: 15;
  }
  .pad {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 1px;
    height: 64px;
    border-radius: 10px;
    border: 0;
    cursor: pointer;
    color: #f4f4f4;
    background: linear-gradient(180deg, #2a2c33, #1c1d22);
    box-shadow: inset 0 0 0 2px color-mix(in srgb, var(--hue) 45%, transparent), 0 3px 0 #000;
    transition: transform 0.08s, box-shadow 0.2s, background 0.2s;
  }
  .pad:hover {
    box-shadow: inset 0 0 0 2px var(--hue), 0 3px 0 #000, 0 0 18px color-mix(in srgb, var(--hue) 40%, transparent);
  }
  .pad:active {
    transform: translateY(2px);
    box-shadow: inset 0 0 0 2px var(--hue), 0 1px 0 #000;
  }
  .pad.lit {
    background: var(--hue);
    color: #111;
    box-shadow: 0 0 24px var(--hue), 0 3px 0 #000;
  }
  .num {
    position: absolute;
    top: 4px;
    left: 7px;
    font-size: 10px;
    font-weight: 800;
    opacity: 0.6;
  }
  .pad.visited::after {
    content: "";
    position: absolute;
    top: 6px;
    right: 7px;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--hue);
  }
  .emoji {
    font-size: 21px;
    line-height: 1;
  }
  .lbl {
    font-size: 11px;
    font-weight: 700;
  }
  @media (max-width: 640px) {
    .pads {
      left: 8px;
      right: 8px;
      transform: none;
      grid-template-columns: repeat(8, 1fr);
      gap: 5px;
      padding: 7px;
      bottom: max(8px, env(safe-area-inset-bottom));
    }
    .pad {
      height: 52px;
    }
    .lbl {
      font-size: 9.5px;
    }
    .emoji {
      font-size: 18px;
    }
  }
</style>
