<!--
  The record shop up in the hills, where records wait like springs. Dig the
  crate — each record is something djmanzo does with a collection — and
  play with the key wheel that harmonic mixing runs on.
-->
<script lang="ts">
  import type { PlaceId } from "../lib/places";
  import { profile } from "../lib/profile.svelte";

  let { go }: { go: (id: PlaceId) => void; linkCode?: string | null } = $props();

  const CRATE = [
    { t: "Your collection, imported", b: "rekordbox, Serato, Traktor and iTunes collections come in with their cues, loops and beat grids. Serato's hot cues are read straight out of the files.", c: "#ff7eb6" },
    { t: "Analysis you can see", b: "BPM, key on the Camelot wheel, loudness (EBU R128), phrases of 16 and 32 beats — and a hatch on the waveform where the beat grid is a guess.", c: "#ffd166" },
    { t: "31 genre families", b: "From bachata and merengue to amapiano and techno, with their felt tempo and rhythmic grammar, so a suggestion never blends across a grammar that clashes.", c: "#5ef2d6" },
    { t: "Find it from memory", b: "Hum it. Type a half-remembered line — accents and commas don't matter. Or describe it: \"bachata with a piano hook, heard it at a beach bar\".", c: "#a58bff" },
    { t: "The Next rail", b: "Up to eight records that would follow the one playing, each with its reasons — harmonic, tempo, loudness, phrase. It suggests; you choose.", c: "#ff5d73" },
    { t: "Set plans with an arc", b: "Pick a length and a shape, and djmanzo assembles a set from your library — no repeats, no artist twice in a row, the opener protected.", c: "#2ec4b6" },
    { t: "Function tags", b: "What a record is for: opener, builder, peak, floor reset, singalong, closer, emergency. Ten tags, fixed, so they mean the same thing every night.", c: "#f9c80e" },
    { t: "It remembers your blends", b: "Keep a mix you liked and djmanzo remembers the pair, so next time the rail can say why it is offering it.", c: "#39ff14" },
  ];
  let top = $state(0);

  // The Camelot wheel: 12 numbers, A (minor) inside, B (major) outside.
  const KEYS_B = ["B", "F♯", "D♭", "A♭", "E♭", "B♭", "F", "C", "G", "D", "A", "E"];
  const KEYS_A = ["A♭m", "E♭m", "B♭m", "Fm", "Cm", "Gm", "Dm", "Am", "Em", "Bm", "F♯m", "D♭m"];
  let picked = $state<{ n: number; ring: "A" | "B" } | null>({ n: 8, ring: "A" });
  function compatible(n: number, ring: "A" | "B"): "same" | "good" | null {
    if (!picked) return null;
    if (picked.n === n && picked.ring === ring) return "same";
    const near = (a: number, b: number) => ((a - b + 12) % 12 === 1 || (b - a + 12) % 12 === 1);
    if (picked.ring === ring && near(picked.n, n)) return "good";
    if (picked.n === n && picked.ring !== ring) return "good";
    return null;
  }
  function seg(i: number, r1: number, r2: number): string {
    const a1 = ((i - 0.5) / 12) * Math.PI * 2 - Math.PI / 2;
    const a2 = ((i + 0.5) / 12) * Math.PI * 2 - Math.PI / 2;
    const p = (r: number, a: number) => `${(150 + r * Math.cos(a)).toFixed(1)},${(150 + r * Math.sin(a)).toFixed(1)}`;
    return `M${p(r1, a1)} A${r1},${r1} 0 0 1 ${p(r1, a2)} L${p(r2, a2)} A${r2},${r2} 0 0 0 ${p(r2, a1)} Z`;
  }
  const mid = (i: number, r: number) => {
    const a = (i / 12) * Math.PI * 2 - Math.PI / 2;
    return { x: 150 + r * Math.cos(a), y: 150 + r * Math.sin(a) };
  };
</script>

<p>Up in the hills, where records wait. Dig the crate: every record is something djmanzo does with your collection.</p>

<div class="crate" role="group" aria-label="The crate">
  {#each CRATE as r, i (r.t)}
    <button
      class="rec"
      class:gone={i < top}
      style="--i:{i - top};--c:{r.c};z-index:{100 - i}"
      onclick={() => (top = i === top ? Math.min(CRATE.length - 1, top + 1) : i)}
      aria-label={i === top ? `${r.t}. ${r.b}` : r.t}
      tabindex={i === top ? 0 : -1}
      onkeydown={(e) => {
        if (e.key === "ArrowRight") top = Math.min(CRATE.length - 1, top + 1);
        else if (e.key === "ArrowLeft") top = Math.max(0, top - 1);
        else return;
        e.preventDefault();
        queueMicrotask(() => (document.querySelector(".rec:not(.gone)") as HTMLElement | null)?.focus());
      }}
    >
      <span class="sleeve">
        <span class="disc" aria-hidden="true"></span>
        <b>{r.t}</b>
        {#if i === top}<span class="body">{r.b}</span>{/if}
      </span>
    </button>
  {/each}
</div>
<div class="dig">
  <button class="btn quiet" onclick={() => (top = Math.max(0, top - 1))} disabled={top === 0}>◀ Back</button>
  <span class="muted small">{top + 1} of {CRATE.length} — tap the record or use the arrow keys</span>
  <button class="btn quiet" onclick={() => (top = Math.min(CRATE.length - 1, top + 1))} disabled={top === CRATE.length - 1}>Dig ▶</button>
</div>

<h2>The key wheel</h2>
<p>Harmonic mixing in one picture: from any key, the safe moves are one step round the wheel, or straight across between minor and major. Press a key.</p>
<div class="wheel-wrap">
  <svg viewBox="0 0 300 300" class="wheel" role="group" aria-label="Camelot wheel">
    {#each KEYS_B as k, i (k)}
      {@const n = ((i + 0) % 12) + 1}
      {@const st = compatible(n, "B")}
      <path d={seg(i, 140, 100)} class="seg {st ?? ''}" role="button" tabindex="0" aria-label="{n}B, {k} major" onclick={() => (picked = { n, ring: "B" })} onkeydown={(e) => e.key === "Enter" && (picked = { n, ring: "B" })} />
      {@const m = mid(i, 120)}
      <text x={m.x} y={m.y - 3} class="k">{n}B</text>
      <text x={m.x} y={m.y + 10} class="kk">{k}</text>
    {/each}
    {#each KEYS_A as k, i (k)}
      {@const n = ((i + 0) % 12) + 1}
      {@const st = compatible(n, "A")}
      <path d={seg(i, 98, 58)} class="seg a {st ?? ''}" role="button" tabindex="0" aria-label="{n}A, {k.replace('m', ' minor')}" onclick={() => (picked = { n, ring: "A" })} onkeydown={(e) => e.key === "Enter" && (picked = { n, ring: "A" })} />
      {@const m = mid(i, 78)}
      <text x={m.x} y={m.y - 3} class="k">{n}A</text>
      <text x={m.x} y={m.y + 10} class="kk">{k}</text>
    {/each}
    <circle cx="150" cy="150" r="54" class="hub" />
    <text x="150" y="146" class="hubt">{picked ? `${picked.n}${picked.ring}` : "—"}</text>
    <text x="150" y="164" class="hubs">{picked ? (picked.ring === "A" ? "minor" : "major") : ""}</text>
  </svg>
  <p class="small">
    {#if picked}
      From <b>{picked.n}{picked.ring}</b>: <b>{((picked.n + 10) % 12) + 1}{picked.ring}</b>, <b>{(picked.n % 12) + 1}{picked.ring}</b> and <b>{picked.n}{picked.ring === "A" ? "B" : "A"}</b> blend cleanly.
      djmanzo shows every record's key this way, filters by compatible keys, and its Next rail counts the key relation among its reasons.
    {/if}
    {#if profile.genre?.id === "bachata"}<br />Bachata lives in minor keys — the inner ring is home.{/if}
  </p>
</div>

<div class="row">
  <button class="btn" onclick={() => go("cabina")}>🎚️ Mix in the booth</button>
  <button class="btn" onclick={() => go("arena")}>🥊 How it compares</button>
</div>

<style>
  .crate {
    position: relative;
    height: 300px;
    perspective: 900px;
    margin: 10px 0 4px;
    outline-offset: 6px;
  }
  .rec {
    position: absolute;
    left: 50%;
    bottom: 0;
    width: min(320px, 82%);
    height: 270px;
    padding: 0;
    border: 0;
    background: none;
    cursor: pointer;
    transform-origin: 50% 100%;
    transform: translateX(-50%) translateZ(calc(var(--i) * -38px)) translateY(calc(var(--i) * -14px)) rotateX(calc(var(--i) * 4deg));
    transition: transform 0.55s cubic-bezier(0.3, 1.2, 0.4, 1), opacity 0.4s;
  }
  .rec.gone {
    transform: translateX(-50%) translateY(80px) rotateX(-80deg);
    opacity: 0;
    pointer-events: none;
  }
  .sleeve {
    position: relative;
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
    gap: 8px;
    height: 100%;
    padding: 16px;
    border-radius: 4px;
    text-align: left;
    color: #111;
    background:
      radial-gradient(circle at 80% 22%, rgba(255, 255, 255, 0.35) 0 14%, transparent 15%),
      linear-gradient(150deg, var(--c), color-mix(in srgb, var(--c) 55%, #000));
    box-shadow: 0 -2px 0 rgba(255, 255, 255, 0.25) inset, 0 12px 24px rgba(0, 0, 0, 0.35);
    overflow: hidden;
  }
  .disc {
    position: absolute;
    top: -60px;
    right: -60px;
    width: 180px;
    height: 180px;
    border-radius: 50%;
    background: repeating-radial-gradient(circle, #151515 0 3px, #222 3px 5px);
    opacity: 0.85;
  }
  .sleeve b {
    font-family: "Bungee", sans-serif;
    font-size: 1.25rem;
    line-height: 1.1;
  }
  .body {
    font-size: 0.95rem;
    font-weight: 600;
  }
  .dig {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 8px;
    margin-bottom: 18px;
  }
  .wheel-wrap {
    display: grid;
    grid-template-columns: minmax(220px, 320px) 1fr;
    gap: 16px;
    align-items: center;
  }
  .wheel {
    width: 100%;
  }
  .seg {
    fill: color-mix(in srgb, var(--surface-2) 80%, black);
    stroke: var(--surface);
    stroke-width: 1.5;
    cursor: pointer;
    transition: fill 0.2s;
  }
  .seg.a {
    fill: color-mix(in srgb, var(--surface-2) 60%, black);
  }
  .seg.good {
    fill: color-mix(in srgb, var(--secondary) 70%, black);
  }
  .seg.same {
    fill: var(--primary);
  }
  .k {
    font: 800 10px var(--body);
    fill: var(--ink);
    text-anchor: middle;
    pointer-events: none;
  }
  .kk {
    font: 600 8px var(--body);
    fill: var(--ink-soft);
    text-anchor: middle;
    pointer-events: none;
  }
  .hub {
    fill: var(--bg);
  }
  .hubt {
    font: 400 22px "Bungee", sans-serif;
    fill: var(--primary);
    text-anchor: middle;
  }
  .hubs {
    font: 600 11px var(--body);
    fill: var(--ink-soft);
    text-anchor: middle;
  }
  .row {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
    margin-top: 12px;
  }
  @media (max-width: 640px) {
    .wheel-wrap {
      grid-template-columns: 1fr;
    }
  }
</style>
