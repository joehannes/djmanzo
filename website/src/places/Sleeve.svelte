<!--
  Every place opens as a record pulled from a crate: the sleeve swings out
  in 3D and the disc slides halfway out of it. On a phone it is a sheet that
  rises from the bottom, and the map stays visible above it.
-->
<script lang="ts">
  import type { Snippet } from "svelte";
  import { onMount } from "svelte";

  interface Props {
    title: string;
    subtitle?: string;
    emoji?: string;
    wide?: boolean;
    tint?: { bg: string; surface: string; primary: string; secondary: string; ink: string };
    onclose: () => void;
    children: Snippet;
  }
  let { title, subtitle, emoji, wide = false, tint, onclose, children }: Props = $props();
  let panel: HTMLElement;

  onMount(() => {
    panel.focus({ preventScroll: true });
  });

  const style = $derived(
    tint ? `--surface:${tint.surface};--bg:${tint.bg};--primary:${tint.primary};--secondary:${tint.secondary};--ink:${tint.ink};--ink-soft:color-mix(in srgb, ${tint.ink} 72%, ${tint.surface});--line:color-mix(in srgb, ${tint.ink} 18%, ${tint.surface});` : "",
  );
</script>

<div class="stage" class:wide>
  <section class="sleeve" bind:this={panel} tabindex="-1" aria-label={title} {style}>
    <div class="record" aria-hidden="true"><span></span></div>
    <header>
      {#if emoji}<span class="emoji" aria-hidden="true">{emoji}</span>{/if}
      <div class="titles">
        <h1>{title}</h1>
        {#if subtitle}<p class="sub">{subtitle}</p>{/if}
      </div>
      <button class="close" onclick={onclose} aria-label="Close {title} and go back to the map">✕</button>
    </header>
    <div class="body">
      {@render children()}
    </div>
  </section>
</div>

<style>
  .stage {
    position: fixed;
    top: 72px;
    right: 14px;
    bottom: 104px;
    width: min(560px, calc(100vw - 28px));
    perspective: 1400px;
    isolation: isolate;
    z-index: 30;
    pointer-events: none;
  }
  .stage.wide {
    width: min(860px, calc(100vw - 28px));
  }
  .sleeve {
    position: relative;
    height: 100%;
    display: flex;
    flex-direction: column;
    border-radius: 6px 18px 18px 6px;
    background: var(--surface);
    color: var(--ink);
    box-shadow: var(--shadow), inset 6px 0 0 color-mix(in srgb, var(--primary) 65%, transparent);
    pointer-events: auto;
    transform-origin: 100% 50%;
    animation: pull 0.55s cubic-bezier(0.2, 0.9, 0.25, 1.05);
    outline: none;
  }
  @keyframes pull {
    from {
      transform: translateX(60px) rotateY(-28deg);
      opacity: 0;
    }
  }
  .record {
    position: absolute;
    z-index: -1;
    top: 18px;
    left: -62px;
    width: 120px;
    height: 120px;
    border-radius: 50%;
    background: radial-gradient(circle, var(--secondary) 0 20%, #0e0e0e 21% 23%, #1d1d1d 24% 36%, #0e0e0e 37% 39%, #222 40% 54%, #0e0e0e 55% 57%, #262626 58%);
    animation: slide 0.9s 0.15s cubic-bezier(0.2, 0.9, 0.25, 1) both, turn 6s 1s linear infinite;
  }
  .record span {
    position: absolute;
    inset: 46%;
    border-radius: 50%;
    background: var(--ink);
  }
  @keyframes slide {
    from {
      left: 10px;
      rotate: -40deg;
    }
  }
  @keyframes turn {
    to {
      rotate: 360deg;
    }
  }
  header {
    position: relative;
    display: flex;
    align-items: flex-start;
    gap: 14px;
    padding: 22px 22px 14px 26px;
    border-bottom: 2px solid var(--line);
  }
  .emoji {
    font-size: 36px;
    line-height: 1;
  }
  .titles {
    flex: 1;
    min-width: 0;
  }
  h1 {
    margin: 0;
    font-size: clamp(1.6rem, 3vw, 2.2rem);
  }
  .sub {
    margin: 4px 0 0;
    color: var(--ink-soft);
  }
  .close {
    flex: none;
    width: 44px;
    height: 44px;
    border-radius: 50%;
    border: 2px solid var(--line);
    background: var(--surface);
    cursor: pointer;
    font-size: 18px;
    z-index: 1;
  }
  .body {
    flex: 1;
    overflow: auto;
    padding: 18px 26px 28px;
    overscroll-behavior: contain;
  }
  @media (max-width: 760px) {
    .stage,
    .stage.wide {
      top: auto;
      left: 0;
      right: 0;
      bottom: 0;
      width: 100%;
      height: min(78dvh, 100dvh - 90px);
    }
    .sleeve {
      border-radius: 22px 22px 0 0;
      animation: up 0.45s cubic-bezier(0.2, 0.9, 0.25, 1);
      padding-bottom: env(safe-area-inset-bottom);
    }
    @keyframes up {
      from {
        transform: translateY(40%);
        opacity: 0;
      }
    }
    .record {
      display: none;
    }
    header {
      padding: 18px 16px 12px;
    }
    .body {
      padding: 14px 16px 24px;
    }
  }
</style>
