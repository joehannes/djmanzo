<script lang="ts">
  /**
   * §115: the quiet proposer's one line.
   *
   * > a background AI always auto watching the DJ and proposing useful things
   * > without being intrusive
   *
   * What is proposed, and when, is `dj_app::whisper`'s: one thing at a time,
   * only once its condition has held, the urgent first, the quiet ones only
   * when the hands are free, and never again soon once declined. This draws
   * it in the space the top bar already has, so a proposal arriving moves
   * nothing a DJ is aiming at; it makes no sound and takes no focus. Taking
   * it runs the proposal's own action through the path any press takes.
   *
   * It arrives on the snapshot, not by asking: a poll once a second was a
   * question to Rust on every second of playback, which `asks.spec.ts`
   * refuses.
   */
  import { uiDo, whisperAnswer, type Proposal } from "./api";

  let {
    offered,
    send,
  }: {
    /** The proposal the latest snapshot carries, if any. */
    offered: Proposal | null | undefined;
    send: (action: string) => void | Promise<void>;
  } = $props();

  /**
   * What was just answered, hidden at once rather than when the next frame
   * arrives — an idle booth may not send one until its heartbeat.
   */
  let answered = $state<string | null>(null);
  $effect(() => {
    if (!offered || offered.kind !== answered) answered = null;
  });
  let proposal = $derived(offered && offered.kind !== answered ? offered : null);

  async function take() {
    const taken = proposal;
    if (!taken) return;
    answered = taken.kind;
    try {
      if (taken.run.startsWith("ui ")) await uiDo(taken.run);
      else await send(taken.run);
    } finally {
      void whisperAnswer(taken.kind, "taken").catch(() => {});
    }
  }

  function decline(answer: "not-now" | "not-tonight") {
    const declined = proposal;
    if (!declined) return;
    answered = declined.kind;
    void whisperAnswer(declined.kind, answer).catch(() => {});
  }
</script>

<div class="whisper" class:urgent={proposal?.urgent} data-whisper={proposal?.kind ?? ""} role="status" aria-live="polite">
  {#if proposal}
    <span class="says" title={proposal.says}>{proposal.says}</span>
    <button type="button" class="take" onclick={() => void take()}>{proposal.offer}</button>
    <button type="button" class="later" title="Quiet about this for ten minutes" onclick={() => decline("not-now")}>
      Not now
    </button>
    <button type="button" class="later" title="Not again tonight" onclick={() => decline("not-tonight")}>
      Not tonight
    </button>
  {/if}
</div>

<style>
  /*
    Takes the room the top bar has between its ends and nothing more: no
    minimum width, one line, the sentence cut short rather than the row
    wrapped — a proposal arriving must not move the decks.
  */
  .whisper {
    flex: 1 1 0;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 0.4rem;
    white-space: nowrap;
    overflow: hidden;
    font-size: 0.8em;
  }

  /*
    The sentence never goes below a readable width: in the first place this
    stood, the room ran out and the sentence was squeezed to nothing, leaving a
    button that said "Sync deck 2" about nothing anyone could read. Out of
    room, the buttons are what is cut.
  */
  .says {
    flex: 0 1 auto;
    min-width: 9rem;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--assistant);
    padding-left: 0.5rem;
    border-left: 2px solid var(--assistant);
  }

  .urgent .says {
    border-left-color: var(--warn);
  }

  button {
    flex: none;
    font: inherit;
    padding: 0.12rem 0.5rem;
    border-radius: 999px;
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text);
    cursor: pointer;
  }

  .take {
    border-color: var(--assistant);
  }

  .later {
    color: var(--text-dim);
    border-color: transparent;
    padding-inline: 0.3rem;
  }

  .later:hover,
  .later:focus-visible {
    color: var(--text);
  }
</style>
