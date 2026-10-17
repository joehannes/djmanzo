<!-- What a cay out at sea holds. -->
<script lang="ts">
  import type { Lore } from "../lib/lore";
  interface Props {
    lore: Lore;
    onclose: () => void;
  }
  let { lore, onclose }: Props = $props();
  const KIND = { technique: "A move", culture: "From the culture", karaoke: "Hosting karaoke", djmanzo: "About djmanzo" };
</script>

<div class="card" role="dialog" aria-label={lore.title}>
  <p class="kind">{KIND[lore.kind]}</p>
  <h3>{lore.title}</h3>
  <p>{lore.body}</p>
  <p class="muted small">The sea goes on in every direction — there are more cays out there.</p>
  <button class="btn quiet" onclick={onclose}>Back to the water</button>
</div>

<style>
  .card {
    position: fixed;
    left: 50%;
    bottom: 110px;
    transform: translateX(-50%) rotate(-1deg);
    width: min(440px, calc(100vw - 24px));
    padding: 18px 20px;
    border-radius: 6px;
    background: #fdf3d6;
    color: #2a2116;
    box-shadow: var(--shadow);
    z-index: 30;
    animation: drop 0.35s cubic-bezier(0.3, 1.5, 0.5, 1);
  }
  .card::before {
    content: "";
    position: absolute;
    top: -10px;
    left: 50%;
    width: 90px;
    height: 22px;
    translate: -50% 0;
    rotate: 3deg;
    background: rgba(255, 220, 120, 0.75);
  }
  .kind {
    font-weight: 800;
    color: #9a4a1f;
    margin-bottom: 4px;
  }
  .card .muted {
    color: #6b5a44;
  }
  .card .btn {
    color: #2a2116;
    border-color: #2a2116;
  }
  @keyframes drop {
    from {
      transform: translateX(-50%) translateY(20px) rotate(-6deg);
      opacity: 0;
    }
  }
</style>
