<script lang="ts">
  /**
   * The pad zone: eight pads and a row of page tabs.
   *
   * The pages themselves come from Rust (`dj_core::pads`), already rendered
   * into action strings, because the same table is what a controller's pads
   * will map onto — and a mapping written twice is a pad that does one thing
   * under the finger and another on the screen.
   *
   * This component therefore knows almost nothing. It lays out a grid, sends
   * what the pad says to send, and evaluates one `Lit` condition against the
   * snapshot to decide what glows.
   */
  import type { DeckState, Lit, PadPageDto, SamplerState } from "./api";

  let {
    pages,
    deck,
    /**
     * The sampler, for the one page whose pads are not about this deck.
     *
     * A sample belongs to the set rather than to a deck, but the pads that
     * fire it are the deck's — the same as on hardware.
     */
    sampler,
    enabled,
    send,
  }: {
    pages: PadPageDto[];
    deck: DeckState;
    sampler: SamplerState;
    enabled: boolean;
    send: (action: string) => void;
  } = $props();

  /**
   * Which page is showing.
   *
   * Local to the deck, and deliberately not persisted: a DJ who left the pads
   * on the roll page an hour ago does not want to come back to a deck whose
   * cues are hidden. Cues are the page you fall back to.
   */
  let page = $state("cues");

  /**
   * Pages worth showing for this track.
   *
   * A page measured in beats on a track with no grid is eight buttons that do
   * nothing, which reads as broken rather than as "this track has no tempo
   * yet". Hidden rather than greyed out, on the same principle as the FX beat
   * control.
   */
  const usable = $derived(
    pages.filter((p) => !p.needs_grid || deck.analysis?.bpm != null),
  );

  const current = $derived(
    usable.find((p) => p.name === page) ?? usable[0] ?? null,
  );

  /**
   * Which pad is being held, if any.
   *
   * A guard as much as a light: releasing a roll ends its loop, so a release
   * sent by a pad that was never pressed would cancel a loop the DJ set on
   * purpose. Indexed by pad position within the current page.
   */
  let held = $state<number | null>(null);

  /** Close enough for a beat length that made the round trip through an f32. */
  const sameBeats = (a: number, b: number) => Math.abs(a - b) < 0.001;

  /**
   * Whether a pad glows, from live state rather than from what was pressed.
   *
   * One switch over the condition the pad names, instead of a branch per page.
   * That is what keeps a new page a matter of adding rows in Rust rather than
   * adding cases here.
   */
  function lit(condition: Lit): boolean {
    if (condition === "Never") return false;
    if ("HotCueSet" in condition) {
      return deck.hot_cues[condition.HotCueSet - 1] != null;
    }
    if ("LoopBeats" in condition) {
      // A roll is a loop too, so a roll would light the loop page's pad as
      // well. It is the same eddy — but the roll page is where it belongs.
      return (
        !deck.rolling &&
        deck.active_loop?.beats != null &&
        sameBeats(deck.active_loop.beats, condition.LoopBeats)
      );
    }
    if ("RollBeats" in condition) {
      return (
        deck.rolling &&
        deck.active_loop?.beats != null &&
        sameBeats(deck.active_loop.beats, condition.RollBeats)
      );
    }
    if ("FxSlotOn" in condition) {
      const slot = deck.fx[condition.FxSlotOn - 1];
      return slot != null && slot.enabled && slot.kind !== "none";
    }
    if ("FxSlotPost" in condition) {
      return deck.fx[condition.FxSlotPost - 1]?.post_fader ?? false;
    }
    if ("SamplePlaying" in condition) {
      return sampler.slots[condition.SamplePlaying - 1]?.playing ?? false;
    }
    return false;
  }

  function press(index: number, event: PointerEvent) {
    const pad = current?.pads[index];
    if (!pad?.press) return;
    // The action first: pointer capture is a convenience and the pad is the
    // point, so a browser that refuses the capture must still fire.
    send(pad.press);
    if (pad.release) {
      held = index;
      (event.currentTarget as HTMLButtonElement).setPointerCapture(event.pointerId);
    }
  }

  function release(index: number) {
    if (held !== index) return;
    held = null;
    const pad = current?.pads[index];
    if (pad?.release) send(pad.release);
  }
</script>

{#if current}
  <div class="zone">
    <div class="tabs">
      {#each usable as p (p.name)}
        <button class:active={p.name === current.name} onclick={() => (page = p.name)}>
          {p.name}
        </button>
      {/each}
    </div>

    <!--
      Two rows of four, which is what the hardware has. A four-by-two grid
      rather than a single row of eight because a DJ finds pad 6 by its
      position, and a row of eight has no position worth learning.
    -->
    <div class="grid">
      {#each current.pads as pad, index (index)}
        <button
          class="pad"
          class:blank={!pad.press}
          class:lit={lit(pad.lit)}
          class:held={held === index}
          disabled={!enabled || !pad.press}
          onpointerdown={(event) => press(index, event)}
          onpointerup={() => release(index)}
          onpointercancel={() => release(index)}
          oncontextmenu={(event) => {
            if (!pad.clear) return;
            event.preventDefault();
            send(pad.clear);
          }}
          title={pad.clear ? `${pad.label} — right-click for the second gesture` : pad.label}
        >
          {pad.label}
        </button>
      {/each}
    </div>
  </div>
{/if}

<style>
  .zone {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }

  .tabs {
    display: flex;
    gap: 0.2rem;
  }

  .tabs button {
    padding: 0.1rem 0.4rem;
    font-size: 0.75em;
    letter-spacing: 0.04em;
    color: var(--text-dim);
  }

  .tabs button.active {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--on-accent);
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 0.25rem;
  }

  .pad {
    padding: 0.4rem 0.2rem;
    font-size: 0.85em;
    font-weight: 600;
  }

  /* A filled pad is filled, not merely labelled: mid-set this is read by shape
     and colour rather than by the number on it. */
  .pad.lit {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--on-accent);
  }

  /* Held is a different colour from lit, because they are different facts: one
     is "this is set", the other is "your finger is on it right now". */
  .pad.held {
    background: var(--accent-2);
    border-color: var(--accent-2);
    color: var(--on-accent);
  }

  /* A pad with nothing on it keeps its place in the grid so the eight never
     reflow, but stops looking like something to press. */
  .pad.blank {
    background: none;
    border-style: dashed;
    opacity: 0.35;
  }
</style>
