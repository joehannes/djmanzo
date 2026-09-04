<script lang="ts">
  /**
   * §74's contextual control rail.
   *
   * Four to eight controls, whichever the moment calls for. The moment is a
   * *mode* — scratching, stems, preparing, mixing — and djmanzo decides which
   * one from what the deck is doing: a hand on the platter, a muted stem, a
   * stopped deck. This component knows none of that. It is handed the table
   * once and the mode name in every snapshot, and it draws the row that
   * matches, which is the same arrangement the pad zone already uses.
   *
   * It does not latch. Every control here also exists as a widget on the deck,
   * and that widget is where its state is shown; a second lit copy in the rail
   * would be two things claiming to say whether slip is on. The rail makes a
   * control *reachable*, which is what §74 means by promoting one.
   */
  import type { DeckState, RailModeDto } from "./api";
  import SvgButton from "./SvgButton.svelte";

  let {
    deck,
    modes,
    enabled,
    send,
  }: {
    deck: DeckState;
    /** Every mode and its controls, from Rust. */
    modes: RailModeDto[];
    enabled: boolean;
    send: (action: string) => void;
  } = $props();

  /**
   * The row that matches what the deck is doing.
   *
   * `null` when the table has not arrived yet, or names a mode this build does
   * not have — which is a version mismatch between the two halves and is drawn
   * as nothing rather than as a guess.
   */
  const current = $derived(modes.find((mode) => mode.name === deck.rail_mode) ?? null);
</script>

{#if current}
  <!--
    Labelled by the mode, so what changed is legible rather than mysterious: a
    strip of buttons that quietly becomes a different strip is the hostile half
    of adaptive interfaces.
  -->
  <div class="rail" role="toolbar" aria-label="Contextual controls — {current.name}">
    <span class="mode">{current.name}</span>
    <div class="controls">
      {#each current.controls as control (control.action)}
        <SvgButton
          kind="chip"
          label={control.label}
          disabled={!enabled}
          onclick={() => send(control.action)}
        />
      {/each}
    </div>
  </div>
{/if}

<style>
  .rail {
    display: flex;
    align-items: center;
    gap: 0.35rem;
  }

  /*
    A grid, and the row height is set here rather than by the buttons.

    This is the lesson `Pads.svelte` already records: a `SvgButton` is an SVG
    with a fixed 100x52 aspect ratio, and an inline SVG with no width falls
    back to 300x150 — so a flex row of six of them measured 950 px tall and put
    the whole deck below the fold. The cell decides the size and the SVG
    stretches to it.

    `auto-fit` with a minimum wraps to a second row on a narrow deck rather
    than scrolling: six controls is few enough that two rows is the worst case,
    and a rail with a scrollbar is a menu.
  */
  .controls {
    flex: 1;
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(3.4rem, 1fr));
    /* 1.8rem rather than the pads' 2.9: the rail is a secondary strip, and at
       the 1.15 density band a taller row put the deck four pixels over the
       height its density band allows -- which `density.spec.ts` catches. The
       alternative was re-deriving `cockpit::BANDS`, and changing what density
       every window height gets is a much larger change than making a new,
       secondary control one notch shorter. */
    grid-auto-rows: 1.8rem;
    gap: 0.2rem;
  }

  /*
    What the rail is answering, in a word. Dim because it is a caption and not
    a control -- but present, because a DJ who looks down and finds four
    different buttons deserves to know why.
  */
  .mode {
    color: var(--text-dim);
    font-size: 0.65rem;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    padding-inline-end: 0.15rem;
  }
</style>
