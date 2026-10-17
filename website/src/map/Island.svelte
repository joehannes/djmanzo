<!--
  Isla Manzo: one canvas with no edge. Drag (or swipe) to move, pinch or
  scroll to zoom, arrow keys and + / − from the keyboard. The main island
  holds djmanzo's places; the genre islets ring it, nearer for the music the
  visitor named; past them the sea goes on, and every so often there is a
  cay with something worth knowing on it.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { camera } from "./camera.svelte";
  import { drawSea } from "./sea";
  import { blobPath, hash, smoothOpen } from "./geometry";
  import Motifs from "./Motifs.svelte";
  import { PLACES, RIVER, RIVER_B, type PlaceId } from "../lib/places";
  import { GENRES, type Genre } from "../lib/genres";
  import { isletAt } from "./islets";
  import { LORE, type Lore } from "../lib/lore";
  import { profile } from "../lib/profile.svelte";
  import { daylight } from "../lib/daylight.svelte";
  import { mix } from "../lib/colour";
  import { getEngine, hasEngine } from "../lib/audio/engine";

  interface Props {
    admin: boolean;
    current: string | null;
    onplace: (id: PlaceId) => void;
    ongenre: (g: Genre) => void;
    onlore: (l: Lore, x: number, y: number) => void;
  }
  let { admin, current, onplace, ongenre, onlore }: Props = $props();

  let host: HTMLDivElement;
  let canvas: HTMLCanvasElement;
  let w = $state(1200);
  let h = $state(800);
  let pulse = $state(0);
  let beat = $state(false);

  // The island's coast, hills and beaches, fixed by seed so it is the same island for everyone.
  const coast = blobPath(-20, 40, 1260, 900, 7, 64, 0.17);
  const beach = blobPath(-20, 40, 1300, 935, 7, 64, 0.17);
  const reef = blobPath(-20, 40, 1480, 1080, 7, 64, 0.17);
  const hills = [blobPath(-420, -470, 560, 280, 11, 40, 0.22), blobPath(-470, -500, 340, 160, 12, 36, 0.25), blobPath(-500, -520, 160, 80, 13, 30, 0.3)];
  const cape = blobPath(880, -400, 180, 140, 21, 32, 0.25);
  const river = smoothOpen(RIVER);
  const riverB = smoothOpen(RIVER_B);

  const night = $derived(1 - daylight.level);
  const grass = $derived(mix(mix("#14402f", profile.palette.bg, 0.25), "#3fae6a", daylight.level));
  const hill = $derived(mix(mix("#0f3326", profile.palette.bg, 0.3), "#2f8a52", daylight.level));
  const sand = $derived(mix("#8a7350", "#f6dfa6", daylight.level));
  const water = $derived(mix("#0f5f86", "#38c2e0", daylight.level));

  // Genre islets: a ring around the island; the visitor's own genres sail in closer and grow.
  const islets = $derived(
    GENRES.map((g, i) => {
      const at = isletAt(g, profile.p.genres);
      return { g, ...at, path: blobPath(0, 0, at.size, at.size * 0.72, 100 + i, 36, 0.2) };
    }),
  );

  // Cays: the endless part. One per sea cell where the hash says so, beyond the islets.
  const CELL = 1100;
  const cays = $derived.by(() => {
    const out: { key: string; x: number; y: number; lore: Lore; seed: number }[] = [];
    const left = camera.x - w / 2 / camera.z;
    const right = camera.x + w / 2 / camera.z;
    const top = camera.y - h / 2 / camera.z;
    const bottom = camera.y + h / 2 / camera.z;
    for (let cx = Math.floor(left / CELL); cx <= Math.floor(right / CELL); cx++) {
      for (let cy = Math.floor(top / CELL); cy <= Math.floor(bottom / CELL); cy++) {
        const x = cx * CELL + hash(cx, cy, 1) * CELL * 0.7;
        const y = cy * CELL + hash(cx, cy, 2) * CELL * 0.7;
        if (Math.hypot(x, y / 0.85) < 3300) continue;
        if (hash(cx, cy, 3) > 0.42) continue;
        out.push({ key: `${cx}:${cy}`, x, y, lore: LORE[Math.floor(hash(cx, cy, 4) * LORE.length)], seed: cx * 31 + cy });
      }
      if (out.length > 60) break;
    }
    return out;
  });

  const places = $derived(PLACES.filter((p) => !p.hidden || admin));
  const markerScale = $derived(Math.max(0.55, Math.min(1.7, 0.9 / Math.sqrt(camera.z))));
  const far = $derived(camera.z < 0.3);

  // --- Moving around ----------------------------------------------------------

  const pointers = new Map<number, { x: number; y: number }>();
  let last = { x: 0, y: 0 };
  let velocity = { x: 0, y: 0 };
  let pinchDist = 0;
  let dragged = 0;
  let trail: { x: number; y: number; age: number }[] = [];

  function toWorld(sx: number, sy: number) {
    const rect = host.getBoundingClientRect();
    return { x: camera.x + (sx - rect.left - w / 2) / camera.z, y: camera.y + (sy - rect.top - h / 2) / camera.z };
  }

  function zoomAt(sx: number, sy: number, factor: number) {
    const before = toWorld(sx, sy);
    camera.z = camera.clampZ(camera.z * factor);
    const after = toWorld(sx, sy);
    camera.x += before.x - after.x;
    camera.y += before.y - after.y;
  }

  function down(e: PointerEvent) {
    if ((e.target as HTMLElement).closest("button, a, input, textarea, select")) return;
    camera.cancel();
    host.setPointerCapture(e.pointerId);
    pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
    last = { x: e.clientX, y: e.clientY };
    velocity = { x: 0, y: 0 };
    dragged = 0;
    if (pointers.size === 2) {
      const [a, b] = [...pointers.values()];
      pinchDist = Math.hypot(a.x - b.x, a.y - b.y);
    }
  }

  function move(e: PointerEvent) {
    const wpt = toWorld(e.clientX, e.clientY);
    if (night > 0.2 && Math.random() < 0.6) trail.push({ x: wpt.x, y: wpt.y, age: 0 });
    if (!pointers.has(e.pointerId)) return;
    pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
    if (pointers.size === 2) {
      const [a, b] = [...pointers.values()];
      const d = Math.hypot(a.x - b.x, a.y - b.y);
      if (pinchDist > 0) zoomAt((a.x + b.x) / 2, (a.y + b.y) / 2, d / pinchDist);
      pinchDist = d;
      return;
    }
    const dx = e.clientX - last.x;
    const dy = e.clientY - last.y;
    dragged += Math.abs(dx) + Math.abs(dy);
    camera.x -= dx / camera.z;
    camera.y -= dy / camera.z;
    velocity = { x: dx / camera.z, y: dy / camera.z };
    last = { x: e.clientX, y: e.clientY };
  }

  function up(e: PointerEvent) {
    pointers.delete(e.pointerId);
    if (pointers.size < 2) pinchDist = 0;
  }

  function wheel(e: WheelEvent) {
    e.preventDefault();
    camera.cancel();
    if (e.ctrlKey || Math.abs(e.deltaY) > Math.abs(e.deltaX)) {
      zoomAt(e.clientX, e.clientY, Math.exp(-e.deltaY * (e.ctrlKey ? 0.01 : 0.0015)));
    } else {
      camera.x += e.deltaX / camera.z;
    }
  }

  function key(e: KeyboardEvent) {
    if ((e.target as HTMLElement).closest("input, textarea, select, [contenteditable]")) return;
    const stepPx = 120 / camera.z;
    const k = e.key;
    if (k === "ArrowLeft") camera.x -= stepPx;
    else if (k === "ArrowRight") camera.x += stepPx;
    else if (k === "ArrowUp") camera.y -= stepPx;
    else if (k === "ArrowDown") camera.y += stepPx;
    else if (k === "+" || k === "=") camera.z = camera.clampZ(camera.z * 1.2);
    else if (k === "-" || k === "_") camera.z = camera.clampZ(camera.z / 1.2);
    else return;
    camera.cancel();
    e.preventDefault();
  }

  /** A click on a marker only counts if the visitor was not dragging the map. */
  function tap(fn: () => void) {
    return () => {
      if (dragged > 8) return;
      fn();
    };
  }

  // --- The loop ---------------------------------------------------------------

  onMount(() => {
    const ro = new ResizeObserver(() => {
      w = host.clientWidth;
      h = host.clientHeight;
      const dpr = Math.min(2, window.devicePixelRatio || 1);
      canvas.width = w * dpr;
      canvas.height = h * dpr;
      canvas.getContext("2d")!.setTransform(dpr, 0, 0, dpr, 0, 0);
    });
    ro.observe(host);
    if (window.innerWidth < 700) camera.z = 0.26;

    const ctx = canvas.getContext("2d")!;
    let raf = 0;
    let prev = performance.now();
    let lastBeat = -1;
    const reduced = matchMedia("(prefers-reduced-motion: reduce)").matches;
    const frame = (now: number) => {
      const dt = Math.min(0.05, (now - prev) / 1000);
      prev = now;
      // Inertia after a drag.
      if (pointers.size === 0 && (Math.abs(velocity.x) > 0.05 || Math.abs(velocity.y) > 0.05)) {
        camera.x -= velocity.x;
        camera.y -= velocity.y;
        velocity.x *= 0.9;
        velocity.y *= 0.9;
      }
      if (hasEngine() && getEngine().anyPlaying) {
        const lv = getEngine().levels();
        pulse = pulse * 0.7 + lv.low * 0.3;
        const b = lv.low > 0.55;
        if (b && lastBeat < now - 180) {
          beat = true;
          lastBeat = now;
          setTimeout(() => (beat = false), 110);
        }
      } else {
        pulse *= 0.95;
      }
      trail = trail.map((p) => ({ ...p, age: p.age + dt })).filter((p) => p.age < 2.5).slice(-90);
      drawSea(ctx, {
        w,
        h,
        x: camera.x,
        y: camera.y,
        z: camera.z,
        t: reduced ? 0 : now / 1000,
        day: daylight.level,
        deep: profile.palette.bg,
        accent: profile.palette.primary,
        pulse,
        trail,
      });
      raf = requestAnimationFrame(frame);
    };
    raf = requestAnimationFrame(frame);
    window.addEventListener("keydown", key);
    return () => {
      cancelAnimationFrame(raf);
      ro.disconnect();
      window.removeEventListener("keydown", key);
    };
  });
</script>

<div
  class="island"
  bind:this={host}
  onpointerdown={down}
  onpointermove={move}
  onpointerup={up}
  onpointercancel={up}
  onwheel={wheel}
  role="application"
  aria-label="Isla Manzo, a map you can drag and zoom. The places are also listed under Setlist."
>
  <canvas bind:this={canvas} aria-hidden="true"></canvas>
  <div class="world" style="transform: translate({w / 2}px, {h / 2}px) scale({camera.z}) translate({-camera.x}px, {-camera.y}px)">
    <svg class="land" viewBox="-4000 -4000 8000 8000" width="8000" height="8000" style="left:-4000px;top:-4000px" aria-hidden="true">
      <Motifs />
      <defs>
        <radialGradient id="lamp">
          <stop offset="0" stop-color={profile.palette.primary} stop-opacity="0.38" />
          <stop offset="0.5" stop-color={profile.palette.primary} stop-opacity="0.1" />
          <stop offset="1" stop-color={profile.palette.primary} stop-opacity="0" />
        </radialGradient>
      </defs>
      <path d={reef} fill={water} opacity={0.35 + 0.25 * daylight.level} />
      <path d={beach} fill={sand} />
      <path d={coast} fill={grass} />
      {#each hills as hp, i (i)}
        <path d={hp} fill={hill} opacity={0.55 + i * 0.15} />
      {/each}
      <path d={cape} fill={hill} opacity="0.8" />
      <!-- Two rivers meet at the booth, the way two decks meet at the mixer; downstream is the future. -->
      <path d={riverB} class="river" stroke={water} />
      <path d={river} class="river" stroke={water} />
      <path d={riverB} class="flow" stroke={mix(water, "#ffffff", 0.5)} />
      <path d={river} class="flow" stroke={mix(water, "#ffffff", 0.5)} />
      {#if night > 0.2}
        {#each places as p (p.id)}
          <circle cx={p.x} cy={p.y - 40} r="190" fill="url(#lamp)" opacity={night} />
        {/each}
      {/if}
      {#each islets as it (it.g.id)}
        <g transform="translate({it.x} {it.y})">
          <path d={it.path} transform="scale(1.12)" fill={sand} />
          <path d={it.path} fill="url(#motif-{it.g.id})" />
          {#if it.mine}<path d={it.path} fill="none" stroke={it.g.palette.primary} stroke-width="10" opacity="0.8" />{/if}
        </g>
      {/each}
      {#each cays as c (c.key)}
        <ellipse cx={c.x} cy={c.y} rx={70 + (c.seed % 5) * 8} ry={44 + (c.seed % 3) * 6} fill={sand} />
        <circle cx={c.x + 10} cy={c.y - 8} r="20" fill={grass} />
      {/each}
    </svg>

    {#each islets as it (it.g.id)}
      <button
        class="islet-sign"
        class:mine={it.mine}
        class:far
        style="left:{it.x}px;top:{it.y}px;--s:{markerScale * (it.mine ? 1.15 : 1)};--c:{it.g.palette.primary};--b:{it.g.palette.bg};--i:{it.g.palette.ink}"
        onclick={tap(() => ongenre(it.g))}
      >
        <span class="e" aria-hidden="true">{it.g.emoji}</span>
        <span class="n">{it.g.name}</span>
      </button>
    {/each}

    {#each cays as c (c.key)}
      <button class="cay" style="left:{c.x}px;top:{c.y - 70}px;--s:{markerScale}" onclick={tap(() => onlore(c.lore, c.x, c.y))} aria-label="A cay: {c.lore.title}">
        {c.lore.kind === "karaoke" ? "🎤" : c.lore.kind === "culture" ? "🐚" : c.lore.kind === "djmanzo" ? "🍎" : "🎛️"}
      </button>
    {/each}

    {#each places as p (p.id)}
      <button
        class="place"
        class:here={current === p.id}
        class:stamped={profile.p.stamps.includes(p.id)}
        class:far
        class:beat
        style="left:{p.x}px;top:{p.y}px;--s:{markerScale};--glow:{night}"
        onclick={tap(() => onplace(p.id))}
        aria-label="{p.name}: {p.what}"
      >
        <span class="pin" aria-hidden="true">{p.emoji}</span>
        <span class="sign">
          <span class="name">{p.name}</span>
          <span class="what">{p.what}</span>
        </span>
        {#if p.pad}<kbd aria-hidden="true">{p.pad}</kbd>{/if}
      </button>
    {/each}

    <div class="token" style="left:{camera.tokenX}px;top:{camera.tokenY}px;--s:{markerScale}" aria-hidden="true">
      <div class="vinyl" style="--c:{profile.palette.primary}"></div>
    </div>
  </div>
</div>

<style>
  .island {
    position: fixed;
    inset: 0;
    overflow: hidden;
    touch-action: none;
    cursor: grab;
    user-select: none;
    -webkit-user-select: none;
  }
  .island:active {
    cursor: grabbing;
  }
  canvas {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
  }
  .world {
    position: absolute;
    left: 0;
    top: 0;
    transform-origin: 0 0;
    will-change: transform;
  }
  .land {
    position: absolute;
    overflow: visible;
    pointer-events: none;
  }
  .river {
    fill: none;
    stroke-width: 34;
    stroke-linecap: round;
    opacity: 0.95;
  }
  .flow {
    fill: none;
    stroke-width: 6;
    stroke-linecap: round;
    stroke-dasharray: 18 46;
    animation: flow 2.4s linear infinite;
    opacity: 0.7;
  }
  @keyframes flow {
    to {
      stroke-dashoffset: -64;
    }
  }
  button {
    font: inherit;
  }
  .place,
  .islet-sign,
  .cay {
    position: absolute;
    transform: translate(-50%, -100%) scale(var(--s));
    transform-origin: 50% 100%;
    border: 0;
    background: none;
    color: inherit;
    cursor: pointer;
    padding: 0;
  }
  .place {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    transition: transform 0.25s cubic-bezier(0.3, 1.6, 0.5, 1);
  }
  .place:hover,
  .place:focus-visible {
    transform: translate(-50%, -100%) scale(calc(var(--s) * 1.12));
  }
  .place.beat .pin {
    transform: translateY(-4px);
  }
  .pin {
    order: 2;
    display: grid;
    place-items: center;
    width: 64px;
    height: 64px;
        background: var(--primary);
    box-shadow: 0 6px 0 color-mix(in srgb, var(--primary) 55%, black), 0 14px 24px rgba(0, 0, 0, 0.35);
    font-size: 30px;
    transition: transform 0.1s;
  }
  .place .pin {
    border-radius: 18px;
  }
  .sign {
    order: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 8px 14px 9px;
    border-radius: 10px;
    background: var(--sign-bg);
    color: var(--sign-ink);
    border: 3px solid var(--sign-ink);
    box-shadow: 4px 4px 0 var(--sign-ink), 0 0 calc(var(--glow, 0) * 40px) color-mix(in srgb, var(--primary) 70%, transparent);
    white-space: nowrap;
  }
  .name {
    font-family: "Bungee", var(--display), system-ui;
    font-size: 22px;
    line-height: 1.05;
  }
  .what {
    font-size: 14px;
    font-weight: 600;
    opacity: 0.85;
  }
  .place.here .sign {
    background: var(--primary);
    color: var(--on-primary);
  }
  .place.stamped .pin::after {
    content: "✓";
    position: absolute;
    right: -8px;
    top: -8px;
    width: 26px;
    height: 26px;
    border-radius: 50%;
    background: var(--secondary);
    color: var(--on-secondary);
    font-size: 15px;
    display: grid;
    place-items: center;
    font-weight: 800;
  }
  .pin {
    position: relative;
  }
  kbd {
    order: 3;
    font: 700 12px/1 "Figtree", system-ui;
    padding: 3px 7px;
    border-radius: 6px;
    background: rgba(0, 0, 0, 0.45);
    color: #fff;
  }
  .far .sign .what,
  .far kbd {
    display: none;
  }
  .islet-sign {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 18px;
    border-radius: 999px;
    background: var(--b);
    color: var(--i);
    border: 3px solid var(--c);
    font-weight: 800;
    font-size: 22px;
    transform: translate(-50%, -50%) scale(var(--s));
    transform-origin: 50% 50%;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.35);
  }
  .islet-sign .e {
    font-size: 30px;
  }
  .islet-sign.mine {
    background: var(--c);
    color: var(--b);
  }
  .islet-sign:hover,
  .islet-sign:focus-visible {
    transform: translate(-50%, -50%) scale(calc(var(--s) * 1.1));
  }
  .cay {
    font-size: 40px;
    filter: drop-shadow(0 6px 8px rgba(0, 0, 0, 0.4));
    animation: bob 4s ease-in-out infinite;
  }
  @keyframes bob {
    50% {
      translate: 0 -8px;
    }
  }
  .token {
    position: absolute;
    transform: translate(-50%, -50%) scale(var(--s));
    pointer-events: none;
  }
  .vinyl {
    width: 54px;
    height: 54px;
    border-radius: 50%;
    background: radial-gradient(circle, var(--c) 0 22%, #111 23% 26%, #222 27% 40%, #111 41% 44%, #262626 45% 60%, #111 61% 64%, #2a2a2a 65%);
    box-shadow: 0 8px 18px rgba(0, 0, 0, 0.5);
    animation: spin 1.8s linear infinite;
  }
  @keyframes spin {
    to {
      rotate: 360deg;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .flow,
    .cay,
    .vinyl {
      animation: none;
    }
  }
</style>
