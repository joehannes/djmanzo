<!--
  An edition as a record sleeve: the cover on the front, the tracklist —
  what it includes — on the back. Turn it over to read it; choose it to buy it.
-->
<script lang="ts">
  import type { Edition, EditionInfo, Feature } from "../../shared/catalog";

  interface Props {
    info: EditionInfo;
    features: Feature[];
    price: string;
    per: string;
    seats: number;
    chosen: boolean;
    pick: boolean;
    onchoose: (e: Edition) => void;
  }
  let { info, features, price, per, seats, chosen, pick, onchoose }: Props = $props();
  let flipped = $state(false);
  const included = $derived(new Set(info.features));
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
      <p class="small">{info.forWho}</p>
      <ol>
        {#each features as f (f.id)}
          <li class:no={!included.has(f.id)}>
            <span aria-hidden="true">{included.has(f.id) ? "✓" : "–"}</span>
            {f.name}{#if f.status !== "shipped" && included.has(f.id)}<em>{f.status === "beta" ? "beta" : "coming"}</em>{/if}
            <span class="sr-only">{included.has(f.id) ? "included" : "not included"}</span>
          </li>
        {/each}
      </ol>
      <p class="small">On {seats} computers at once. {info.commercial ? "Paid gigs allowed." : "For personal, non-commercial use."}</p>
      <button class="btn quiet" onclick={() => (flipped = false)} tabindex={flipped ? 0 : -1}>Turn it over</button>
    </div>
  </div>
</div>

<style>
  .slot {
    perspective: 1100px;
    min-height: 380px;
  }
  .card {
    position: relative;
    height: 100%;
    min-height: 380px;
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
    font-size: 1.6rem;
    margin: 8px 0 4px;
  }
  .back h3 {
    font-size: 1rem;
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
