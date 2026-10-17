<!--
  The booth: two decks and a mixer, playable. Each deck plays a synthesised
  sketch of a genre's groove; the visitor beatmatches with sync, blends with
  the crossfader, swaps the bass with the EQ and throws the effects djmanzo
  has, by name. It teaches what a mix *is* in a minute — and then says
  plainly what the real application does with real records.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { getEngine, type Engine, type FxName } from "../lib/audio/engine";
  import { GENRES, GENRE_BY_ID } from "../lib/genres";
  import { profile } from "../lib/profile.svelte";
  import type { PlaceId } from "../lib/places";
  import Knob from "../ui/Knob.svelte";
  import { islandSound } from "../hud/sound.svelte";

  let { go }: { go: (id: PlaceId) => void; linkCode?: string | null } = $props();

  let engine = $state<Engine | null>(null);
  let genres = $state<[string, string]>([profile.genre?.id ?? "house", profile.secondGenre?.id ?? (profile.genre?.id === "bachata" ? "merengue" : "bachata")]);
  let playing = $state<[boolean, boolean]>([false, false]);
  let eq = $state([
    { low: 0, mid: 0, high: 0 },
    { low: 0, mid: 0, high: 0 },
  ]);
  let faders = $state([0.9, 0.9]);
  let xfade = $state(0.5);
  let tempo = $state(124);
  let sync = $state(true);
  let filter = $state(0);
  let fx = $state<Record<FxName, boolean>>({ echo: false, reverb: false, filter: false, flanger: false, crush: false, gate: false });
  let canvases: HTMLCanvasElement[] = [];
  let lights = $state([0, 0]);

  const suggested = $derived(new Set([...GENRE_BY_ID[genres[0]].fx, ...GENRE_BY_ID[genres[1]].fx].slice(0, 3)));

  async function ensure(): Promise<Engine> {
    const e = getEngine();
    await e.wake();
    if (!engine) {
      engine = e;
      // The island's own background loop gives way to the booth.
      if (islandSound.on) islandSound.stop();
      e.decks.forEach((d) => d.stop());
      e.setVolume(0.55);
      e.setCrossfader(xfade);
      e.sync = sync;
      e.setTempo(tempo);
      e.decks[0].setGenre(genres[0]);
      e.decks[1].setGenre(genres[1]);
    }
    return e;
  }

  async function toggle(i: 0 | 1) {
    const e = await ensure();
    if (playing[i]) {
      e.decks[i].stop();
      playing[i] = false;
    } else {
      e.decks[i].setGenre(genres[i]);
      e.decks[i].start();
      playing[i] = true;
    }
  }

  function setGenre(i: 0 | 1, id: string) {
    genres[i] = id;
    engine?.decks[i].setGenre(id);
    if (!sync) return;
    // With sync on, the master tempo follows the deck you just loaded if nothing else plays.
    if (!playing[i === 0 ? 1 : 0]) {
      const g = GENRE_BY_ID[id];
      tempo = Math.round((g.bpm[0] + g.bpm[1]) / 2);
      engine?.setTempo(tempo);
    }
  }

  function setEq(i: 0 | 1, band: "low" | "mid" | "high", v: number) {
    eq[i][band] = v;
    engine?.decks[i].eq(band, v);
  }

  function holdFx(name: FxName, on: boolean) {
    fx[name] = on;
    if (!engine) return;
    if (name === "filter") engine.setFilter(on ? (filter === 0 ? -0.6 : filter) : 0);
    else engine.setFx(name, on ? 1 : 0);
  }

  /** The classic: bass out of the playing deck as it comes into the other, on the phrase. */
  async function bassSwap() {
    const e = await ensure();
    const from = xfade < 0.5 ? 0 : 1;
    const to = from === 0 ? 1 : 0;
    if (!playing[to]) await toggle(to);
    setEq(to, "low", -26);
    const steps = 24;
    for (let s = 0; s <= steps; s++) {
      await new Promise((r) => setTimeout(r, 120));
      const t = s / steps;
      xfade = from === 0 ? t : 1 - t;
      e.setCrossfader(xfade);
      if (s === Math.floor(steps / 2)) {
        setEq(from as 0 | 1, "low", -26);
        setEq(to as 0 | 1, "low", 0);
      }
    }
    setEq(from as 0 | 1, "low", 0);
  }

  onMount(() => {
    let raf = 0;
    const data = new Uint8Array(512);
    const draw = () => {
      if (engine) {
        engine.decks.forEach((d, i) => {
          const c = canvases[i];
          if (!c) return;
          const ctx = c.getContext("2d")!;
          d.analyser.getByteTimeDomainData(data);
          const w = c.width;
          const h = c.height;
          // Scroll left one column at a time, like a waveform running toward the playhead.
          ctx.drawImage(c, -3, 0);
          let peak = 0;
          for (let k = 0; k < data.length; k++) peak = Math.max(peak, Math.abs(data[k] - 128) / 128);
          ctx.fillStyle = "rgba(10,10,14,1)";
          ctx.fillRect(w - 3, 0, 3, h);
          const g = GENRE_BY_ID[genres[i]];
          ctx.fillStyle = g.palette.primary;
          const ph = Math.max(1, peak * h * 0.95 * (d.playing ? 1 : 0));
          ctx.fillRect(w - 3, (h - ph) / 2, 3, ph);
          lights[i] = d.playing ? peak : 0;
        });
      }
      raf = requestAnimationFrame(draw);
    };
    raf = requestAnimationFrame(draw);
    return () => {
      cancelAnimationFrame(raf);
      if (engine) {
        engine.decks.forEach((d) => d.stop());
        Object.keys(fx).forEach((k) => engine!.setFx(k as FxName, 0));
        engine.setFilter(0);
      }
    };
  });
</script>

<p>Two decks, a mixer, and djmanzo's effects. Press play on both, keep <b>sync</b> on, and move the crossfader. That's a mix.</p>

<div class="booth">
  {#each [0, 1] as i (i)}
    {@const d = i as 0 | 1}
    {@const g = GENRE_BY_ID[genres[d]]}
    <div class="deck" style="--c:{g.palette.primary};--c2:{g.palette.secondary}">
      <div class="top">
        <span class="n">{d + 1}</span>
        <select value={genres[d]} onchange={(e) => setGenre(d, (e.currentTarget as HTMLSelectElement).value)} aria-label="Deck {d + 1} genre">
          {#each GENRES as gg (gg.id)}<option value={gg.id}>{gg.emoji} {gg.name}</option>{/each}
        </select>
      </div>
      <canvas bind:this={canvases[d]} width="300" height="56" class="wave" aria-hidden="true"></canvas>
      <div class="mid">
        <div class="platter" class:spinning={playing[d]} style="--speed:{60 / (sync ? tempo : (g.bpm[0] + g.bpm[1]) / 2) * 4}s" aria-hidden="true">
          <div class="label" style="background:{g.palette.secondary}">{g.emoji}</div>
        </div>
        <div class="readout">
          <span class="bpm">{(sync ? tempo : (g.bpm[0] + g.bpm[1]) / 2).toFixed(0)}<small>BPM</small></span>
          <button class="play" class:on={playing[d]} onclick={() => toggle(d)}>{playing[d] ? "❚❚ Pause" : "▶ Play"}</button>
        </div>
      </div>
      <div class="strip">
        <Knob label="High" value={eq[d].high} min={-26} max={6} display={(v) => (v <= -25 ? "kill" : `${v.toFixed(0)} dB`)} onchange={(v) => setEq(d, "high", v)} colour={g.palette.primary} />
        <Knob label="Mid" value={eq[d].mid} min={-26} max={6} display={(v) => (v <= -25 ? "kill" : `${v.toFixed(0)} dB`)} onchange={(v) => setEq(d, "mid", v)} colour={g.palette.primary} />
        <Knob label="Low" value={eq[d].low} min={-26} max={6} display={(v) => (v <= -25 ? "kill" : `${v.toFixed(0)} dB`)} onchange={(v) => setEq(d, "low", v)} colour={g.palette.primary} />
        <label class="fader">
          <span class="sr-only">Deck {d + 1} volume</span>
          <input type="range" min="0" max="1" step="0.01" value={faders[d]} oninput={(e) => { faders[d] = +(e.currentTarget as HTMLInputElement).value; engine?.decks[d].fader.gain.setTargetAtTime(faders[d], engine.ctx.currentTime, 0.02); }} />
          <span class="meter" style="--l:{lights[d]}"></span>
        </label>
      </div>
    </div>
  {/each}
</div>

<div class="mixer">
  <label class="xf">
    <span>Deck 1</span>
    <input type="range" min="0" max="1" step="0.005" value={xfade} aria-label="Crossfader" oninput={(e) => { xfade = +(e.currentTarget as HTMLInputElement).value; engine?.setCrossfader(xfade); }} />
    <span>Deck 2</span>
  </label>
  <div class="tempo">
    <label>
      <span>Master tempo <b>{tempo}</b> BPM</span>
      <input type="range" min="70" max="180" step="1" value={tempo} oninput={(e) => { tempo = +(e.currentTarget as HTMLInputElement).value; engine?.setTempo(tempo); }} />
    </label>
    <button class="chip" aria-pressed={sync} onclick={() => { sync = !sync; if (engine) engine.sync = sync; }}>Sync</button>
    <Knob label="Filter" value={filter} onchange={(v) => { filter = v; engine?.setFilter(v); }} display={(v) => (Math.abs(v) < 0.03 ? "open" : v < 0 ? "low-pass" : "high-pass")} />
  </div>
</div>

<h2>Effects</h2>
<p class="muted small">Hold a pad to throw an effect; let go and it rides out. Highlighted: what {GENRE_BY_ID[genres[0]].name} and {GENRE_BY_ID[genres[1]].name} DJs reach for.</p>
<div class="fxpads">
  {#each ["echo", "reverb", "filter", "flanger", "crush", "gate"] as name (name)}
    {@const n = name as FxName}
    <button
      class="fxpad"
      class:on={fx[n]}
      class:suggested={suggested.has(n as never)}
      onpointerdown={async () => { await ensure(); holdFx(n, true); }}
      onpointerup={() => holdFx(n, false)}
      onpointerleave={() => fx[n] && holdFx(n, false)}
      onkeydown={async (e) => { if (e.key === " " || e.key === "Enter") { e.preventDefault(); await ensure(); holdFx(n, !fx[n]); } }}
      aria-pressed={fx[n]}
    >
      {name}
    </button>
  {/each}
</div>
<button class="btn" onclick={bassSwap}>Show me a bass swap</button>

<h2>In djmanzo, with your records</h2>
<p>
  The same moves on real music: beatmatched sync, keylock, isolator EQ with kills, three effect slots per deck and on the master, beat-synced timing — plus stems,
  loop rolls, a slicer, a sampler, and up to six decks. The transition planner proposes where a mix should start and how long it should be, and shows its reasons.
</p>
<figure>
  <img class="shot" src="/shots/palette.webp" alt="djmanzo's command palette listing actions that match 'bach'" loading="lazy" width="1280" height="800" />
  <figcaption>Ctrl/Cmd + K: every action djmanzo knows, typed. Here, a Bachata EQ preset per deck and the Bachata Rosa theme.</figcaption>
</figure>
<div class="next">
  <button class="btn" onclick={() => go("tienda")}>💿 The record shop: your library</button>
  <button class="btn" onclick={() => go("faro")}>🗼 Light the room</button>
</div>

<style>
  .booth {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 14px;
  }
  .deck {
    padding: 12px;
    border-radius: 14px;
    background: #15161b;
    color: #f2f2f2;
    border: 2px solid color-mix(in srgb, var(--c) 45%, transparent);
  }
  .top {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .n {
    font-family: "Bungee", sans-serif;
    font-size: 26px;
    color: var(--c);
  }
  .top select {
    background: #0c0c10;
    color: #f2f2f2;
    border-color: #333;
  }
  .wave {
    width: 100%;
    height: 56px;
    margin: 10px 0;
    border-radius: 6px;
    background: #0a0a0e;
  }
  .mid {
    display: flex;
    gap: 12px;
    align-items: center;
  }
  .platter {
    flex: none;
    width: 92px;
    height: 92px;
    border-radius: 50%;
    background: repeating-radial-gradient(circle, #191919 0 2px, #222 2px 4px);
    display: grid;
    place-items: center;
    box-shadow: 0 0 0 4px #2a2b31;
  }
  .platter.spinning {
    animation: spin var(--speed) linear infinite;
  }
  @keyframes spin {
    to {
      rotate: 360deg;
    }
  }
  .label {
    width: 38px;
    height: 38px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    font-size: 18px;
  }
  .readout {
    display: flex;
    flex-direction: column;
    gap: 8px;
    flex: 1;
  }
  .bpm {
    font: 800 28px/1 "Figtree", sans-serif;
    font-variant-numeric: tabular-nums;
  }
  .bpm small {
    font-size: 12px;
    margin-left: 4px;
    opacity: 0.6;
  }
  .play {
    min-height: 44px;
    border-radius: 999px;
    border: 2px solid var(--c);
    background: transparent;
    color: #f2f2f2;
    font-weight: 800;
    cursor: pointer;
  }
  .play.on {
    background: var(--c);
    color: #111;
  }
  .strip {
    display: flex;
    justify-content: space-between;
    align-items: flex-end;
    margin-top: 12px;
    --ink-soft: #aaa;
  }
  .fader {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 90px;
  }
  .fader input {
    writing-mode: vertical-lr;
    direction: rtl;
    height: 86px;
    width: 26px;
    accent-color: var(--c);
  }
  .meter {
    width: 6px;
    height: 80px;
    border-radius: 3px;
    background: linear-gradient(to top, #2ecc71 0 60%, #f1c40f 60% 85%, #e74c3c 85%);
    clip-path: inset(calc((1 - var(--l)) * 100%) 0 0 0);
    opacity: 0.9;
  }
  .mixer {
    margin-top: 14px;
    padding: 14px;
    border-radius: 14px;
    background: #15161b;
    color: #f2f2f2;
    --ink-soft: #aaa;
  }
  .xf {
    display: flex;
    align-items: center;
    gap: 10px;
    font-weight: 700;
    font-size: 13px;
  }
  .xf input {
    flex: 1;
    height: 34px;
    accent-color: var(--primary);
  }
  .tempo {
    display: flex;
    align-items: center;
    gap: 14px;
    margin-top: 10px;
    flex-wrap: wrap;
  }
  .tempo label {
    flex: 1;
    min-width: 200px;
    display: grid;
    font-size: 13px;
  }
  .tempo input {
    accent-color: var(--secondary);
  }
  .fxpads {
    display: grid;
    grid-template-columns: repeat(6, 1fr);
    gap: 8px;
    margin-bottom: 12px;
  }
  .fxpad {
    height: 58px;
    border-radius: 10px;
    border: 0;
    background: #23252c;
    color: #eee;
    font-weight: 800;
    text-transform: capitalize;
    cursor: pointer;
    box-shadow: inset 0 0 0 2px #333, 0 3px 0 #000;
    touch-action: none;
  }
  .fxpad.suggested {
    box-shadow: inset 0 0 0 2px var(--primary), 0 3px 0 #000;
  }
  .fxpad.on {
    background: var(--primary);
    color: var(--on-primary);
    box-shadow: 0 0 22px var(--primary);
  }
  .next {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  h2 {
    margin-top: 20px;
  }
  @media (max-width: 760px) {
    .booth {
      grid-template-columns: 1fr;
    }
    .fxpads {
      grid-template-columns: repeat(3, 1fr);
    }
  }
</style>
