<!--
  An edition as a record sleeve: the cover on the front, the tracklist —
  what it includes — on the back. Turn it over to read it; choose it to buy it.
-->
<script lang="ts">
  import type { Edition, EditionInfo } from "../../shared/catalog";
  import type { Fact } from "../../shared/facts";

  interface Props {
    info: EditionInfo;
    facts: Fact[];
    price: string;
    per: string;
    seats: number;
    chosen: boolean;
    pick: boolean;
    onchoose: (e: Edition) => void;
  }
  let { info, facts, price, per, seats, chosen, pick, onchoose }: Props = $props();
  let flipped = $state(false);
  // What is on it: the foundation's and its modules' — shipped first, and what is planned said so.
  const has = (f: Fact) => f.module === "foundation" || info.modules.includes(f.module);
  const shown = $derived(facts.filter((f) => f.status !== "planned" || has(f)));
</script>

<div class="slot" class:chosen>
  <div class="card" class:flipped>
    <div class="face front" style="--c:{info.colour}">
      {#if pick}<span class="sticker">The MC's pick for you</span>{/if}
      <span class="big" aria-hidden="true">{info.emoji}</span>
      <h3>{info.name}</h3>
      <p class="tag">{info.tagline}</p>
      <p class="price"><b>{price}</b> <span>{per}</span></p>
      <div class="acts">
        <button class="btn primary" onclick={() => onchoose(info.id)} aria-pressed={chosen}>{chosen ? "✓ Chosen" : "Choose"}</button>
        <button class="btn quiet" onclick={() => (flipped = true)}>What's on it</button>
      </div>
    </div>
    <div class="face back" aria-hidden={!flipped}>
      <h3>{info.name} — the tracklist</h3>
      <p class="small">For: {info.forWho}</p>
      {#if info.id === "custom"}
        <p class="small">The Hobbyist's foundation, with the modules you pick below.</p>
      {/if}
      <ol>
        {#each shown as f (f.id)}
          {@const on = has(f)}
          <li class:no={!on}>
            <span aria-hidden="true">{on ? "✓" : "–"}</span>
            {f.name}{#if on && f.status !== "shipped"}<em>{f.status === "planned" ? "planned" : "part-built"}</em>{/if}
            <span class="sr-only">{on ? "included" : "not included"}</span>
          </li>
        {/each}
      </ol>
      <p class="small">On {seats} computers at once. djmanzo is in beta — tested thoroughly, not yet through a real gig — and nothing is locked yet: every edition opens every feature for now, and the licence records what you bought.</p>
      <button class="btn quiet" onclick={() => (flipped = false)} tabindex={flipped ? 0 : -1}>Turn it over</button>
    </div>
  </div>
</div>

<style>
  .slot {
    perspective: 1100px;
    min-height: 420px;
  }
  .card {
    position: relative;
    height: 100%;
    min-height: 420px;
    transform-style: preserve-3d;
    transition: transform 0.7s cubic-bezier(0.3, 1.2, 0.4, 1);
  }
  .card.flipped {
    transform: rotateY(180deg);
  }
  .face {
    position: absolute;
    inset: 0;
    backface-visibility: hidden;
    -webkit-backface-visibility: hidden;
    border-radius: 6px;
    padding: 16px;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .front {
    background:
      repeating-radial-gradient(circle at 110% -10%, transparent 0 18px, rgba(255, 255, 255, 0.06) 18px 20px),
      linear-gradient(160deg, var(--c), color-mix(in srgb, var(--c) 40%, #111));
    color: #111;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.35);
  }
  .chosen .front {
    outline: 4px solid var(--ink);
    outline-offset: 3px;
  }
  .back {
    transform: rotateY(180deg);
    background: #f7f1e3;
    color: #222;
    overflow: auto;
  }
  .big {
    font-size: 54px;
    line-height: 1;
  }
  h3 {
    font-family: "Bungee", sans-serif;
    font-size: 1.4rem;
    margin: 8px 0 4px;
    white-space: nowrap;
  }
  .front > * {
    flex-shrink: 0;
  }
  .back h3 {
    font-size: 1rem;
    white-space: normal;
  }
  .tag {
    font-weight: 600;
    min-height: 3em;
  }
  .price {
    margin-top: auto;
    font-size: 1.05rem;
  }
  .price b {
    font-size: 2rem;
  }
  .acts {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }
  .front .btn.primary {
    background: #111;
    border-color: #111;
    color: #fff;
    box-shadow: none;
  }
  .front .btn.quiet {
    border-color: rgba(0, 0, 0, 0.4);
    color: #111;
  }
  .back .btn {
    color: #222;
    border-color: #999;
    margin-top: 6px;
  }
  .sticker {
    position: absolute;
    top: 14px;
    right: -34px;
    rotate: 35deg;
    padding: 4px 40px;
    background: #111;
    color: #fff;
    font-size: 11px;
    font-weight: 800;
  }
  ol {
    list-style: none;
    padding: 0;
    margin: 6px 0;
    font-size: 13.5px;
    flex: 1;
    min-height: 0;
    overflow-y: auto;
  }
  li {
    display: flex;
    gap: 6px;
    padding: 2px 0;
  }
  li.no {
    opacity: 0.45;
  }
  em {
    font-style: normal;
    font-size: 10.5px;
    font-weight: 800;
    padding: 0 5px;
    border-radius: 4px;
    background: #e2d6b5;
    margin-left: 4px;
  }
  .small {
    font-size: 13px;
  }
  @media (prefers-reduced-motion: reduce) {
    .card {
      transition: none;
    }
  }
</style>
