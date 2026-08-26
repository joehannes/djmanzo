<script lang="ts">
  /**
   * The learning module.
   *
   * # Why it appears and disappears, when nothing else here does
   *
   * The rest of the Conduct panel is deliberately fixed — a control that moves
   * is one you cannot build muscle memory for. This is the exception, and for
   * the opposite reason: coaching during a peak-time set is not a help, it is
   * a second thing competing for attention with the room.
   *
   * So it follows the occasion's own verbosity, which the backend sends. It is
   * loud when learning, brief when practising, and absent when playing to
   * people. That table lives in Rust beside the occasions; restating "peak is
   * quiet, learning is not" here would be a second copy that eventually
   * disagreed.
   *
   * # Why it names things before it corrects them
   *
   * Most of early DJing is doing something that worked and not knowing what it
   * was called, which means not being able to do it on purpose. A word for the
   * move comes first; the correction is at most one, because a learner handed
   * three applies none of them. See ASSISTANT.md §12.
   */
  import { coachReport, type CoachReport } from "./api";

  interface Props {
    enabled: boolean;
    /**
     * The occasion's own verbosity: 2 learning, 1 practising, 0 in front of
     * people. Passed in rather than fetched, because the panel above already
     * has it and two components polling the same value would disagree for a
     * few seconds at every change.
     */
    verbosity: number;
  }

  let { enabled, verbosity }: Props = $props();

  /**
   * Slower than the assistant's own tick.
   *
   * Six seconds. A transition takes tens of seconds, so this is already far
   * faster than anything it reports on, and a coach that redraws twice a
   * second is a coach nobody can read.
   */
  const REFRESH_MS = 6000;

  let report = $state<CoachReport | null>(null);

  $effect(() => {
    if (!enabled || verbosity < 1) {
      report = null;
      return;
    }
    const read = async () => {
      try {
        report = await coachReport();
      } catch {
        // Silent. A coach that cannot reach the backend has nothing to teach,
        // and an error where the encouragement goes is worse than a gap.
        report = null;
      }
    };
    void read();
    const timer = setInterval(() => void read(), REFRESH_MS);
    return () => clearInterval(timer);
  });

  /** Most recent first: what just happened is what is being asked about. */
  let recent = $derived([...(report?.observed ?? [])].reverse().slice(0, 4));
</script>

{#if report && (recent.length > 0 || report.note || report.next)}
  <section class="coach">
    <h3>What that was</h3>

    {#if recent.length > 0}
      <ul class="observed">
        {#each recent as move (move.technique + move.at)}
          <li>
            <strong>{move.technique}</strong>
            <span class="what">{move.what}</span>
            <!--
              The metaphor only at the loudest setting. Practising, a DJ
              wants the name; learning, they want the picture that makes it
              stick.
            -->
            {#if verbosity >= 2}
              <span class="metaphor">{move.metaphor}</span>
            {/if}
          </li>
        {/each}
      </ul>
    {:else}
      <p class="quiet">Nothing to name yet — go and do something.</p>
    {/if}

    {#if report.note}
      <!--
        One note, never a list. And the three parts stay separate: a DJ
        mid-mix reads only the last line.
      -->
      <div class="note">
        <p class="what">{report.note.what}</p>
        <p class="why">{report.note.why}</p>
        <p class="fix">{report.note.fix}</p>
      </div>
    {/if}

    {#if report.next}
      <p class="next">
        <span class="lead">Try next</span>
        <strong>{report.next}</strong>
        {#if verbosity >= 2 && report.next_metaphor}
          <span class="metaphor">{report.next_metaphor}</span>
        {/if}
      </p>
    {/if}
  </section>
{/if}

<style>
  .coach {
    margin-top: 0.7rem;
    padding-top: 0.6rem;
    border-top: 1px solid var(--border, #333);
  }

  h3 {
    margin: 0 0 0.4rem;
    font-size: 0.78em;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-dim);
  }

  .observed {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  .observed li {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
    font-size: 0.8em;
    line-height: 1.45;
  }

  .observed strong {
    color: var(--accent);
  }

  .what {
    color: var(--text);
  }

  /* Set apart, because it is a different kind of sentence -- a picture rather
     than an instruction, and it should read as one. */
  .metaphor {
    color: var(--text-dim);
    font-style: italic;
  }

  .note {
    margin-top: 0.55rem;
    padding: 0.45rem 0.55rem;
    border-radius: 5px;
    background: color-mix(in srgb, var(--warn, #d97706) 10%, transparent);
    font-size: 0.8em;
    line-height: 1.5;
  }

  .note p {
    margin: 0 0 0.25rem;
  }

  .note p:last-child {
    margin-bottom: 0;
  }

  .note .what {
    font-weight: 600;
  }

  .note .why,
  .quiet {
    color: var(--text-dim);
  }

  /* The line a DJ acts on, so it survives being the only one read. */
  .note .fix {
    color: var(--text);
  }

  .quiet,
  .next {
    margin: 0.5rem 0 0;
    font-size: 0.8em;
    line-height: 1.5;
  }

  .next {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
  }

  .lead {
    font-size: 0.88em;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-dim);
  }
</style>
