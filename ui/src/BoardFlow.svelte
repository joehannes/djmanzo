<script lang="ts">
  /**
   * §121: the music, while a board is up.
   *
   * > the ubuquitous main bar ... that should also hold basic visualization
   * > of music flow/ctrl/autopilot ... disaster ctrl ... emergency/basic/
   * > instinctive stuff
   *
   * A board takes the decks' place, so the decks come with it in brief: each
   * one's number, what is on it, how far through and how long is left, and
   * its play button; then the crossfader's position and the autopilot, in
   * one line under the top bar on every board.
   *
   * It reads the snapshot and sends the same actions the decks' own buttons
   * send. Nothing here is a second deck.
   */
  import type { DeckState, MasterState } from "./api";
  import Icon from "./controls/Icon.svelte";

  let {
    decks,
    master,
    enabled,
    send,
  }: {
    decks: DeckState[];
    master: MasterState;
    enabled: boolean;
    send: (action: string) => void;
  } = $props();

  const clock = (seconds: number) => {
    if (!Number.isFinite(seconds) || seconds < 0) return "–:––";
    const whole = Math.floor(seconds);
    return `${Math.floor(whole / 60)}:${String(whole % 60).padStart(2, "0")}`;
  };

  const left = (deck: DeckState) =>
    deck.loaded ? Math.max(0, deck.length_seconds - deck.position_seconds) : NaN;

  const through = (deck: DeckState) =>
    deck.loaded && deck.length_seconds > 0 ? deck.position_seconds / deck.length_seconds : 0;

  /** Under thirty seconds left on a playing deck: the one thing to notice. */
  const ending = (deck: DeckState) => deck.playing && left(deck) < 30;
</script>

<div class="flow" role="group" aria-label="The decks in brief">
  {#each decks as deck (deck.number)}
    <div
      class="deck"
      class:playing={deck.playing}
      class:ending={ending(deck)}
      class:empty={!deck.loaded}
      data-deck={deck.number}
    >
      <button
        class="play"
        disabled={!enabled || !deck.loaded}
        aria-label="{deck.playing ? 'Pause' : 'Play'} deck {deck.number}"
        aria-pressed={deck.playing}
        title="{deck.playing ? 'Pause' : 'Play'} deck {deck.number}"
        onclick={() => send(`deck ${deck.number} play_pause`)}
      ><Icon name={deck.playing ? "stop" : "forward"} size="0.8rem" /></button>
      <span class="number">{deck.number}</span>
      <span class="what" title={deck.loaded ? `${deck.title ?? ""}${deck.artist ? ` — ${deck.artist}` : ""}` : "Nothing loaded"}>
        {deck.loaded ? (deck.title ?? "Untitled") : "—"}
      </span>
      <span class="left mono" title="Time left">-{clock(left(deck))}</span>
      <span class="through" aria-hidden="true"><span style="width: {Math.round(through(deck) * 100)}%"></span></span>
    </div>
  {/each}
  <div class="master">
    <span class="fader" title="Crossfader" aria-label="Crossfader {Math.round(((master.crossfader + 1) / 2) * 100)}% towards B" role="img">
      <span class="thumb" style="left: {Math.round(((master.crossfader + 1) / 2) * 100)}%"></span>
    </span>
    <button
      class="autopilot"
      class:on={master.automix.enabled}
      disabled={!enabled}
      aria-pressed={master.automix.enabled}
      title={master.automix.enabled ? "The automix has the mix: take it back" : "Hand the mix to the automix"}
      onclick={() => send(master.automix.enabled ? "automix off" : "automix on")}
    ><Icon name="robot" size="0.85rem" /> Autopilot</button>
  </div>
</div>

<style>
  .flow {
    display: flex;
    align-items: stretch;
    gap: 0.4rem;
    min-width: 0;
  }

  .deck {
    position: relative;
    display: flex;
    align-items: center;
    gap: 0.35rem;
    flex: 1 1 0;
    min-width: 0;
    padding: 0.25rem 0.4rem 0.35rem;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--panel);
    font-size: 0.8rem;
  }

  .deck.playing {
    border-color: var(--selected);
  }

  /* §30: the one thing that needs noticing is a warning, not a colour. */
  .deck.ending {
    border-color: var(--warn);
  }

  .deck.empty {
    color: var(--text-dim);
  }

  .play {
    display: inline-grid;
    place-items: center;
    width: 1.6rem;
    height: 1.6rem;
    padding: 0;
    flex: none;
  }

  .number {
    font-weight: 700;
    color: var(--text-dim);
    flex: none;
  }

  .what {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .left {
    flex: none;
    color: var(--text-dim);
  }

  .ending .left {
    color: var(--warn);
  }

  .through {
    position: absolute;
    left: 0.4rem;
    right: 0.4rem;
    bottom: 0.15rem;
    height: 2px;
    background: var(--border);
    border-radius: 1px;
    overflow: hidden;
  }

  .through > span {
    display: block;
    height: 100%;
    background: var(--selected);
  }

  .master {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex: none;
  }

  .fader {
    position: relative;
    width: 4rem;
    height: 4px;
    border-radius: 2px;
    background: var(--border-strong);
  }

  .fader .thumb {
    position: absolute;
    top: -4px;
    width: 4px;
    height: 12px;
    margin-left: -2px;
    border-radius: 1px;
    background: var(--text);
  }

  .autopilot {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    font-size: 0.8rem;
  }

  .autopilot.on {
    color: var(--selected);
    border-color: var(--selected);
  }
</style>
