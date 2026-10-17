<!--
  The dock: what djmanzo is, said plainly, with the application itself on
  screen. The first thing a visitor reads should be true.
-->
<script lang="ts">
  import type { PlaceId } from "../lib/places";
  import { profile, recommend } from "../lib/profile.svelte";
  import { EDITION_INFO } from "../../shared/catalog";

  let { go }: { go: (id: PlaceId) => void; linkCode?: string | null } = $props();
  const rec = $derived(recommend(profile.p));
  const told = $derived(profile.p.asked.length > 0);
</script>

<p class="lead">
  <b>djmanzo</b> is a DJ and karaoke application for macOS and Linux. Two to six decks, a mixer, effects, stems, a karaoke rotation with a singers' screen — and an
  assistant that learns how you play, proposes, and never takes over unless you let it.
</p>

<figure>
  <img class="shot" src="/shots/cockpit.webp" alt="djmanzo's main screen: two decks with waveforms, stems, pads and a mixer" width="1280" height="800" loading="lazy" />
  <figcaption>The real application, version 0.45, with two test tracks loaded. Nothing on this island is a mock-up of the app.</figcaption>
</figure>

<h2>What it does differently</h2>
<ul class="diff">
  <li><span>🧠</span><div><b>It learns you, and asks before it acts.</b> The assistant speaks only in actions — a mix, a record, a loop — and you accept, change or reject. Touch any control and it is yours again.</div></li>
  <li><span>🎛️</span><div><b>Stems with no added latency.</b> Vocals, drums, bass and the rest are separated ahead of the playhead, so the voice knob and stem swaps cost nothing live.</div></li>
  <li><span>⌨️</span><div><b>A laptop is an instrument.</b> With nothing plugged in, the keyboard is a full controller — and a MIDI mapping uses the same vocabulary.</div></li>
  <li><span>🌴</span><div><b>Caribbean music is first-class.</b> Bachata, merengue, dembow, salsa and kompa sit next to house and techno in the analysis, the suggestions and the themes.</div></li>
  <li><span>🎤</span><div><b>Karaoke built in, not bolted on.</b> A rotation, words timed by speech recognition, a mic chain per singer, and guests signing up from their phones.</div></li>
</ul>

<h2>Where to next</h2>
{#if told}
  <p>
    You told the MC you're {profile.p.role === "karaoke" ? "a karaoke host" : profile.p.role === "both" ? "a DJ and a karaoke host" : profile.p.role === "dj" ? "a DJ" : "here to look around"}{#if profile.genre}, into {profile.genre.name.toLowerCase()}{/if}.
    The MC would point you to <b>{EDITION_INFO[rec.edition].name}</b>.
  </p>
{/if}
<div class="next">
  <button class="btn primary" onclick={() => go(profile.p.role === "karaoke" ? "tarima" : "cabina")}>{profile.p.role === "karaoke" ? "🎤 Try the karaoke stage" : "🎚️ Mix two genres in the booth"}</button>
  <button class="btn" onclick={() => go("gigs")}>📌 Which edition for which night</button>
  <button class="btn" onclick={() => go("taquilla")}>🎟️ Prices</button>
</div>

<h2>Honestly</h2>
<p class="muted">
  djmanzo is in <b>beta</b>. Its realtime engine is proven allocation-free by test and its features are tested one by one, but it has not yet been through a real gig on
  real hardware. It runs on macOS (Apple Silicon and Intel, plus a karaoke build for Macs back to 10.13) and Linux. There is no Windows build yet — that is on the wall
  of wishes.
</p>

<style>
  .lead {
    font-size: 1.15rem;
  }
  .diff {
    list-style: none;
    padding: 0;
    display: grid;
    gap: 12px;
  }
  .diff li {
    display: flex;
    gap: 12px;
  }
  .diff span {
    font-size: 26px;
    line-height: 1.2;
  }
  .next {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-bottom: 18px;
  }
  h2 {
    margin-top: 22px;
  }
</style>
