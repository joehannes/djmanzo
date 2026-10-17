<!-- "Playing the set": the guided tour, one place after another, like tracks. -->
<script lang="ts">
  import { PLACE_BY_ID, type PlaceId } from "../lib/places";
  interface Props {
    order: PlaceId[];
    i: number;
    onstep: (d: number) => void;
    onstop: () => void;
  }
  let { order, i, onstep, onstop }: Props = $props();
</script>

<div class="tour" role="region" aria-label="The tour">
  <span class="eq" aria-hidden="true"><i></i><i></i><i></i></span>
  <span class="now">Track {i + 1} of {order.length}: <b>{PLACE_BY_ID[order[i]].name}</b></span>
  <button onclick={() => onstep(-1)} disabled={i === 0} aria-label="Previous place">⏮</button>
  <button class="next" onclick={() => onstep(1)}>{i === order.length - 1 ? "Finish" : "Next track ⏭"}</button>
  <button onclick={onstop} aria-label="Stop the tour">■</button>
</div>

<style>
  .tour {
    position: fixed;
    top: 74px;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px 8px 14px;
    border-radius: 999px;
    background: var(--ink);
    color: var(--bg);
    z-index: 40;
    box-shadow: var(--shadow);
    white-space: nowrap;
  }
  button {
    height: 36px;
    min-width: 36px;
    border-radius: 999px;
    border: 0;
    background: color-mix(in srgb, var(--bg) 12%, var(--ink));
    color: var(--bg);
    font-weight: 800;
    cursor: pointer;
    padding: 0 10px;
  }
  .next {
    background: var(--primary);
    color: var(--on-primary);
  }
  .eq {
    display: flex;
    gap: 2px;
    align-items: flex-end;
    height: 16px;
  }
  .eq i {
    width: 4px;
    background: var(--primary);
    animation: eq 0.9s ease-in-out infinite;
  }
  .eq i:nth-child(2) {
    animation-delay: 0.2s;
  }
  .eq i:nth-child(3) {
    animation-delay: 0.4s;
  }
  @keyframes eq {
    0%,
    100% {
      height: 4px;
    }
    50% {
      height: 16px;
    }
  }
  @media (max-width: 640px) {
    .tour {
      top: 66px;
      font-size: 13px;
      max-width: calc(100vw - 16px);
    }
    .now b {
      display: inline-block;
      max-width: 34vw;
      overflow: hidden;
      text-overflow: ellipsis;
      vertical-align: bottom;
    }
  }
</style>
