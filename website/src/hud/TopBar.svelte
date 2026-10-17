<!--
  The strip along the top: who and when on the left; on the right the
  crossfader that sets the hour, the island's sound, the passport and the
  MC. Everything else on the site is reached on the map itself.
-->
<script lang="ts">
  import { daylight } from "../lib/daylight.svelte";
  import { profile, type DaylightMode } from "../lib/profile.svelte";
  import { session } from "../lib/session.svelte";
  import { PLACES, type PlaceId } from "../lib/places";
  import { islandSound } from "./sound.svelte";

  interface Props {
    onplace: (id: PlaceId) => void;
    onsetlist: () => void;
    onmc: () => void;
  }
  let { onplace, onsetlist, onmc }: Props = $props();

  const MODES: { id: DaylightMode; label: string; icon: string }[] = [
    { id: "day", label: "Day", icon: "☀️" },
    { id: "auto", label: "Your clock", icon: "🕰️" },
    { id: "system", label: "Your system's setting", icon: "💻" },
    { id: "night", label: "Night", icon: "🌙" },
  ];
  const stampable = PLACES.filter((p) => p.stamp).length;
  const index = $derived(MODES.findIndex((m) => m.id === profile.p.daylight));
</script>

<header class="bar">
  <button class="brand" onclick={() => onplace("muelle")} aria-label="djmanzo — back to the dock">
    <span class="disc" aria-hidden="true"></span>
    <span class="words">
      <span class="name">djmanzo</span>
      <span class="hour">Isla Manzo · {daylight.name}</span>
    </span>
  </button>

  <div class="tools">
    <div class="xfader" role="radiogroup" aria-label="Day or night">
      <span class="ends" aria-hidden="true">☀️</span>
      <div class="track">
        {#each MODES as m, i (m.id)}
          <button role="radio" aria-checked={profile.p.daylight === m.id} title={m.label} aria-label={m.label} onclick={() => profile.set("daylight", m.id)}>
            <span aria-hidden="true">{m.icon}</span>
          </button>
          {#if i < MODES.length - 1}<span class="tick" aria-hidden="true"></span>{/if}
        {/each}
        <span class="cap" style="--i:{index}" aria-hidden="true"></span>
      </div>
      <span class="ends" aria-hidden="true">🌙</span>
    </div>

    <button class="tool" class:on={islandSound.on} onclick={() => islandSound.toggle()} aria-pressed={islandSound.on} title={islandSound.on ? "Mute the island" : "Hear the island"}>
      <span aria-hidden="true">{islandSound.on ? "🔊" : "🔈"}</span><span class="label">{islandSound.on ? "Sound on" : "Sound"}</span>
    </button>
    <button class="tool" onclick={() => onplace("pase")} title="Your pass: stamps, sign-in and licences">
      <span aria-hidden="true">{session.user?.avatar ? "" : "🪪"}</span>
      {#if session.user?.avatar}<img src={session.user.avatar} alt="" />{/if}
      <span class="label">{profile.p.stamps.length}/{stampable}</span>
    </button>
    <button class="tool" onclick={onmc} title="Talk to the MC"><span aria-hidden="true">🎙️</span><span class="label">MC</span></button>
    <button class="tool" onclick={onsetlist} title="The setlist: every place on the island"><span aria-hidden="true">📝</span><span class="label">Setlist</span></button>
  </div>
</header>

<style>
  .bar {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
    padding: max(10px, env(safe-area-inset-top)) 14px 10px;
    pointer-events: none;
    z-index: 20;
  }
  .bar > * {
    pointer-events: auto;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    background: color-mix(in srgb, var(--surface) 86%, transparent);
    border: 2px solid var(--line);
    border-radius: 999px;
    padding: 6px 16px 6px 6px;
    cursor: pointer;
    backdrop-filter: blur(8px);
  }
  .disc {
    width: 36px;
    height: 36px;
    border-radius: 50%;
    background: radial-gradient(circle, var(--secondary) 0 24%, #111 25% 30%, #2b2b2b 31% 44%, #111 45% 48%, #2b2b2b 49%);
    animation: spin 3s linear infinite;
  }
  @keyframes spin {
    to {
      rotate: 360deg;
    }
  }
  .words {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    line-height: 1.05;
  }
  .name {
    font-family: "Bungee", sans-serif;
    font-size: 20px;
  }
  .hour {
    font-size: 12px;
    color: var(--ink-soft);
  }
  .tools {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .tool {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 44px;
    padding: 0 12px;
    border-radius: 999px;
    border: 2px solid var(--line);
    background: color-mix(in srgb, var(--surface) 86%, transparent);
    backdrop-filter: blur(8px);
    cursor: pointer;
    font-weight: 700;
  }
  .tool.on {
    border-color: var(--primary);
  }
  .tool img {
    width: 26px;
    height: 26px;
    border-radius: 50%;
  }
  .xfader {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 44px;
    padding: 0 10px;
    border-radius: 999px;
    border: 2px solid var(--line);
    background: color-mix(in srgb, var(--surface) 86%, transparent);
    backdrop-filter: blur(8px);
  }
  .ends {
    font-size: 13px;
  }
  .track {
    position: relative;
    display: flex;
    align-items: center;
    height: 30px;
    padding: 0 2px;
    border-radius: 6px;
    background: color-mix(in srgb, var(--bg) 70%, black);
  }
  .track button {
    position: relative;
    z-index: 2;
    width: 30px;
    height: 30px;
    border: 0;
    background: none;
    cursor: pointer;
    font-size: 14px;
    opacity: 0.55;
  }
  .track button[aria-checked="true"] {
    opacity: 1;
  }
  .tick {
    width: 2px;
    height: 8px;
    background: var(--line);
  }
  .cap {
    position: absolute;
    z-index: 1;
    top: 2px;
    left: calc(2px + var(--i) * 32px);
    width: 30px;
    height: 26px;
    border-radius: 5px;
    background: var(--primary);
    box-shadow: inset 0 -3px 0 rgba(0, 0, 0, 0.3);
    transition: left 0.25s cubic-bezier(0.3, 1.4, 0.5, 1);
  }
  @media (max-width: 760px) {
    .label,
    .hour,
    .ends {
      display: none;
    }
    .tool {
      padding: 0 10px;
    }
    .name {
      font-size: 17px;
    }
    .brand {
      padding-right: 12px;
    }
    .tools {
      gap: 5px;
    }
    .xfader {
      padding: 0 6px;
    }
    .track button {
      width: 26px;
    }
    .cap {
      width: 26px;
      left: calc(2px + var(--i) * 28px);
    }
  }
  @media (max-width: 420px) {
    .words {
      display: none;
    }
    .brand {
      padding: 4px;
    }
  }
</style>
