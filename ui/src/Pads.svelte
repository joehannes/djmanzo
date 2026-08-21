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
  import SvgButton from "./SvgButton.svelte";

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
    if ("SliceAt" in condition) {
      // Where the playhead *is*, not what was pressed — so the light walks the
      // grid on its own and you can see the next slice coming.
      return deck.slice.at === condition.SliceAt;
    }
    // The M6 stem page is visible before separated buffers land. Until the
    // snapshot carries per-stem state, these pads behave and label correctly
    // but do not latch visually.
    if ("StemMuted" in condition || "StemSolo" in condition) return false;
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
        <SvgButton kind="tab" label={p.name} active={p.name === current.name} onclick={() => (page = p.name)} />
      {/each}
    </div>

    <!--
      Two rows of four, which is what the hardware has. A four-by-two grid
      rather than a single row of eight because a DJ finds pad 6 by its
      position, and a row of eight has no position worth learning.
    -->
    <div class="grid">
      {#each current.pads as pad, index (index)}
        <SvgButton
          kind="pad"
          label={pad.label}
          blank={!pad.press}
          lit={lit(pad.lit)}
          held={held === index}
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
        />
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

  .grid {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 0.25rem;
  }

</style>
