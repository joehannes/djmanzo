<!--
  The lighthouse: light for the room, drawn by the music. A WebGL scene in
  the visitor's genre colours that answers the bass, the mids and the highs
  of whatever the island is playing — three presets, the way djmanzo's own
  visuals switch between Milkdrop-style presets. three.js is loaded only when
  someone comes up here.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import type * as T from "three";
  import { getEngine, hasEngine } from "../lib/audio/engine";
  import { profile } from "../lib/profile.svelte";
  import { islandSound } from "../hud/sound.svelte";
  import type { PlaceId } from "../lib/places";

  let { go }: { go: (id: PlaceId) => void; linkCode?: string | null } = $props();

  type Preset = "sea" | "tunnel" | "rings";
  let preset = $state<Preset>("sea");
  let host: HTMLDivElement;
  let failed = $state(false);
  let meters = $state({ low: 0, mid: 0, high: 0 });

  onMount(() => {
    let disposed = false;
    let cleanup = () => {};
    void (async () => {
      try {
        const THREE = await import("three");
        if (disposed) return;
        const pal = profile.palette;
        const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: false });
        renderer.setPixelRatio(Math.min(2, window.devicePixelRatio));
        host.appendChild(renderer.domElement);
        const scene = new THREE.Scene();
        scene.background = new THREE.Color(pal.bg);
        scene.fog = new THREE.FogExp2(new THREE.Color(pal.bg), 0.035);
        const camera = new THREE.PerspectiveCamera(60, 1, 0.1, 200);
        const resize = () => {
          const w = host.clientWidth;
          const h = host.clientHeight;
          renderer.setSize(w, h);
          camera.aspect = w / h;
          camera.updateProjectionMatrix();
        };
        resize();
        const ro = new ResizeObserver(resize);
        ro.observe(host);

        const primary = new THREE.Color(pal.primary);
        const secondary = new THREE.Color(pal.secondary);

        // Preset 1 — the sea: a field of points that swells with the bass, swept by the beam.
        const sea = new THREE.Group();
        const N = 90;
        const geo = new THREE.BufferGeometry();
        const pos = new Float32Array(N * N * 3);
        const col = new Float32Array(N * N * 3);
        for (let i = 0; i < N; i++)
          for (let j = 0; j < N; j++) {
            const k = (i * N + j) * 3;
            pos[k] = (i - N / 2) * 0.4;
            pos[k + 2] = (j - N / 2) * 0.4;
            const c = primary.clone().lerp(secondary, j / N);
            col[k] = c.r;
            col[k + 1] = c.g;
            col[k + 2] = c.b;
          }
        geo.setAttribute("position", new THREE.BufferAttribute(pos, 3));
        geo.setAttribute("color", new THREE.BufferAttribute(col, 3));
        const points = new THREE.Points(geo, new THREE.PointsMaterial({ size: 0.09, vertexColors: true, transparent: true, opacity: 0.9, blending: THREE.AdditiveBlending, depthWrite: false }));
        sea.add(points);
        const beam = new THREE.Mesh(
          new THREE.ConeGeometry(2.2, 22, 32, 1, true),
          new THREE.MeshBasicMaterial({ color: new THREE.Color("#fff6d0"), transparent: true, opacity: 0.12, side: THREE.DoubleSide, blending: THREE.AdditiveBlending, depthWrite: false }),
        );
        beam.geometry.translate(0, -11, 0);
        const pivot = new THREE.Group();
        pivot.position.set(0, 4, -6);
        beam.rotation.z = Math.PI / 2;
        pivot.add(beam);
        sea.add(pivot);
        scene.add(sea);

        // Preset 2 — the tunnel: rings rushing toward the viewer, faster with the highs.
        const tunnel = new THREE.Group();
        const rings: T.Mesh[] = [];
        for (let i = 0; i < 40; i++) {
          const m = new THREE.Mesh(
            new THREE.TorusGeometry(3, 0.04, 8, 64),
            new THREE.MeshBasicMaterial({ color: i % 2 ? primary : secondary, transparent: true, opacity: 0.9, blending: THREE.AdditiveBlending }),
          );
          m.position.z = -i * 1.5;
          rings.push(m);
          tunnel.add(m);
        }
        scene.add(tunnel);

        // Preset 3 — the rings: a spectrum of bars standing in a circle, like a crowd around the booth.
        const bars = new THREE.Group();
        const B = 64;
        const barMeshes: T.Mesh[] = [];
        for (let i = 0; i < B; i++) {
          const a = (i / B) * Math.PI * 2;
          const m = new THREE.Mesh(new THREE.BoxGeometry(0.22, 1, 0.22), new THREE.MeshBasicMaterial({ color: primary.clone().lerp(secondary, i / B) }));
          m.position.set(Math.cos(a) * 4, 0, Math.sin(a) * 4);
          m.rotation.y = -a;
          barMeshes.push(m);
          bars.add(m);
        }
        scene.add(bars);

        const freq = new Uint8Array(256);
        let raf = 0;
        const start = performance.now();
        const frame = () => {
          const t = (performance.now() - start) / 1000;
          let lv = { low: 0.25 + 0.15 * Math.sin(t * 2), mid: 0.2, high: 0.15, all: 0.2 };
          if (hasEngine() && getEngine().anyPlaying) {
            const e = getEngine();
            lv = e.levels();
            e.analyser.getByteFrequencyData(freq);
          } else {
            for (let i = 0; i < freq.length; i++) freq[i] = 60 + 50 * Math.sin(t * 2 + i * 0.2);
          }
          meters = { low: lv.low, mid: lv.mid, high: lv.high };
          sea.visible = preset === "sea";
          tunnel.visible = preset === "tunnel";
          bars.visible = preset === "rings";
          if (preset === "sea") {
            camera.position.set(0, 4.5, 12);
            camera.lookAt(0, 0, -2);
            const p = geo.attributes.position.array as Float32Array;
            for (let i = 0; i < N; i++)
              for (let j = 0; j < N; j++) {
                const k = (i * N + j) * 3;
                p[k + 1] = Math.sin(i * 0.25 + t * 1.6) * Math.cos(j * 0.22 + t) * (0.4 + lv.low * 2.2);
              }
            geo.attributes.position.needsUpdate = true;
            pivot.rotation.y = t * 0.9;
            (beam.material as T.MeshBasicMaterial).opacity = 0.08 + lv.mid * 0.3;
          } else if (preset === "tunnel") {
            camera.position.set(Math.sin(t * 0.4) * 0.6, Math.cos(t * 0.3) * 0.6, 4);
            camera.lookAt(0, 0, -20);
            const speed = 0.05 + lv.high * 0.5;
            rings.forEach((r, i) => {
              r.position.z += speed;
              if (r.position.z > 4) r.position.z -= 60;
              const s = 1 + lv.low * 0.6 * Math.sin(i + t * 4);
              r.scale.set(s, s, 1);
              r.rotation.z = t * 0.2 + i * 0.1;
            });
          } else {
            camera.position.set(Math.sin(t * 0.25) * 9, 4 + Math.sin(t * 0.2) * 1.5, Math.cos(t * 0.25) * 9);
            camera.lookAt(0, 1, 0);
            barMeshes.forEach((m, i) => {
              const v = freq[Math.floor((i / B) * 120) + 2] / 255;
              m.scale.y = 0.2 + v * 7;
              m.position.y = (m.scale.y - 1) / 2;
            });
          }
          renderer.render(scene, camera);
          raf = requestAnimationFrame(frame);
        };
        raf = requestAnimationFrame(frame);
        cleanup = () => {
          cancelAnimationFrame(raf);
          ro.disconnect();
          renderer.dispose();
          scene.traverse((o) => {
            const m = o as T.Mesh;
            m.geometry?.dispose();
            const mat = m.material as T.Material | undefined;
            mat?.dispose?.();
          });
          renderer.domElement.remove();
        };
      } catch {
        failed = true;
      }
    })();
    return () => {
      disposed = true;
      cleanup();
    };
  });
</script>

<p>djmanzo puts visuals on a second screen for the room, drawn from the music. Up here it's the same idea in your colours — turn the island's sound on and watch it answer the bass.</p>

<div class="stage" bind:this={host}>
  {#if failed}<p class="notice">This browser could not start WebGL, so the lighthouse is dark. Everything else on the island still works.</p>{/if}
  <div class="meters" aria-hidden="true">
    <i style="--v:{meters.low}"></i><i style="--v:{meters.mid}"></i><i style="--v:{meters.high}"></i>
  </div>
</div>

<div class="row">
  <div class="presets" role="radiogroup" aria-label="Visual preset">
    {#each [["sea", "🌊 The sea"], ["tunnel", "🌀 The tunnel"], ["rings", "📊 The crowd"]] as [id, label] (id)}
      <button class="chip" role="radio" aria-checked={preset === id} onclick={() => (preset = id as Preset)}>{label}</button>
    {/each}
  </div>
  <button class="btn" onclick={() => islandSound.toggle()}>{islandSound.on ? "🔇 Quiet" : "🔊 Play the island's groove"}</button>
</div>

<h2>In djmanzo</h2>
<ul>
  <li><b>Milkdrop-style presets</b> driven by the music, on a second screen for the room (beta).</li>
  <li><b>Video on a deck</b>, and the singers' screen for karaoke on its own display.</li>
  <li><b>MIDI clock out</b>, so a light desk or a drum machine follows djmanzo's tempo.</li>
  <li><b>An overlay for live streams</b> — the record the room hears, on OBS for Twitch, YouTube, TikTok or Kick.</li>
</ul>
<p class="muted small">Art-Net / DMX lighting output and full VJ video mixing are on the roadmap, not in the app yet — they are on <button class="link" onclick={() => go("muro")}>the wall of wishes</button>.</p>

<style>
  .stage {
    position: relative;
    height: min(52vh, 420px);
    border-radius: 14px;
    overflow: hidden;
    background: #000;
  }
  .stage :global(canvas) {
    display: block;
    width: 100%;
    height: 100%;
  }
  .meters {
    position: absolute;
    left: 12px;
    bottom: 12px;
    display: flex;
    gap: 4px;
    align-items: flex-end;
    height: 50px;
  }
  .meters i {
    width: 8px;
    height: calc(4px + var(--v) * 46px);
    background: var(--primary);
    border-radius: 2px;
  }
  .row {
    display: flex;
    justify-content: space-between;
    gap: 10px;
    flex-wrap: wrap;
    margin: 12px 0;
  }
  .presets {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
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
  .notice {
    position: absolute;
    inset: 20px;
  }
</style>
