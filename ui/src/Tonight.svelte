<script lang="ts">
  /**
   * §118: the night being played, and the decisions it was prepared for.
   *
   * > the live event: narrow focus, specific, prepared, flexible/extendible
   * > but starting exactly the way it should ... prepared for exceptions and
   * > emergencies/alternatives
   *
   * > whenever in a stressful situation a decision becomes necessary a super
   * > easy and quick to identify and select widget can pop up (as unobtrusive
   * > or occupying as necessary and adequate)
   *
   * One line in the top bar while the night is on: where it stands against
   * the agreed times, and the next moment with a time -- which lights up in
   * its last ten minutes, the one thing here that asks for attention by
   * itself. **If…** opens the quick decision: every trouble this night can
   * have, as large targets, each opening the plan the DJ wrote for it at
   * home. A trouble with no plan says so, and shows what DJs usually keep
   * ready as a hint -- never as a plan nobody wrote.
   *
   * What the night is, where it stands and what is next are Rust's
   * (`gig::tonight`), asked every half minute with the DJ's own date and
   * time; this draws them.
   */
  import { portal } from "./controls/portal";
  import { eventTonight, type GigDecision, type GigTonight } from "./api";

  let {
    id,
    onEnd,
    deciding = $bindable(false),
  }: {
    id: string;
    /** The DJ ended the night. */
    onEnd: () => void;
    /** Whether the quick decision is open -- which §118a's guides may ask for. */
    deciding?: boolean;
  } = $props();

  let tonight = $state<GigTonight | null>(null);
  let chosen = $state<GigDecision | null>(null);
  let ending = $state(false);
  let pill = $state<HTMLElement | undefined>();
  let place = $state({ top: 0, left: 0 });

  async function refresh() {
    try {
      tonight = await eventTonight(id);
    } catch {
      tonight = null;
    }
  }

  $effect(() => {
    void id;
    void refresh();
    const every = setInterval(refresh, 30_000);
    return () => clearInterval(every);
  });

  /** "1 h 05", "12 min". */
  function span(minutes: number): string {
    if (minutes < 60) return `${minutes} min`;
    const days = Math.floor(minutes / 1440);
    if (days >= 1) return days === 1 ? "tomorrow" : `in ${days} days`;
    return `${Math.floor(minutes / 60)} h ${String(minutes % 60).padStart(2, "0")}`;
  }

  const standing = $derived.by(() => {
    const s = tonight?.standing;
    if (!s || s.kind === "unscheduled") return "no start time set";
    if (s.kind === "before") return s.starts_in >= 1440 ? `starts ${span(s.starts_in)}` : `starts in ${span(s.starts_in)}`;
    if (s.kind === "playing") return s.left > 0 ? `${span(s.played)} in · ${span(s.left)} left` : `${span(s.played)} in`;
    return `${span(s.past)} past the end`;
  });

  /** The next moment's last ten minutes: the one thing that asks by itself. */
  const soon = $derived(tonight?.next != null && tonight.next.in_minutes <= 10);

  function openDecisions() {
    const r = pill?.getBoundingClientRect();
    if (r) place = { top: Math.round(r.bottom + 6), left: Math.round(r.left) };
    chosen = null;
    deciding = !deciding;
  }

  // Opened from elsewhere (the guides), it still opens under its own line.
  $effect(() => {
    if (!deciding) return;
    const r = pill?.getBoundingClientRect();
    if (r) place = { top: Math.round(r.bottom + 6), left: Math.round(r.left) };
  });

  $effect(() => {
    if (!deciding) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.stopPropagation();
      if (chosen) chosen = null;
      else deciding = false;
    };
    const onPress = (event: PointerEvent) => {
      const target = event.target as Element | null;
      if (target?.closest?.(".decide-float, .tonight, .guides-float")) return;
      deciding = false;
    };
    window.addEventListener("keydown", onKey, true);
    window.addEventListener("pointerdown", onPress, true);
    return () => {
      window.removeEventListener("keydown", onKey, true);
      window.removeEventListener("pointerdown", onPress, true);
    };
  });
</script>

{#if tonight}
  <div class="tonight" class:soon bind:this={pill} data-tonight={tonight.id} role="group" aria-label="Tonight: {tonight.title}">
    <span class="live" aria-hidden="true"></span>
    <strong class="title">{tonight.title}</strong>
    <span class="standing" data-standing={tonight.standing.kind}>{standing}</span>
    {#if tonight.next}
      <span class="next" class:soon title={tonight.next.record ? `The record: ${tonight.next.record}` : undefined}>
        {tonight.next.what}{tonight.next.at ? ` ${tonight.next.at}` : ""} · in {span(tonight.next.in_minutes)}{tonight.next.record ? ` · ${tonight.next.record}` : ""}
      </span>
    {/if}
    <button
      type="button"
      class="if"
      aria-expanded={deciding}
      title="What to do if something goes wrong: the plans you wrote for tonight"
      onclick={openDecisions}>If…</button
    >
    {#if ending}
      <button type="button" class="end yes" onclick={onEnd}>End the night</button>
      <button type="button" class="end" onclick={() => (ending = false)}>Keep playing</button>
    {:else}
      <button type="button" class="end" title="End the night" aria-label="End the night…" onclick={() => (ending = true)}>×</button>
    {/if}
  </div>

  {#if deciding}
    <div
      class="decide-float"
      use:portal
      style="top: {place.top}px; left: {place.left}px"
      role="dialog"
      aria-label="If something goes wrong"
    >
      {#if chosen}
        <div class="plan" data-plan={chosen.trouble}>
          <h3>{chosen.title}</h3>
          {#if chosen.plan}
            <p class="written">{chosen.plan}</p>
          {:else}
            <p class="none">No plan was written for this.</p>
            <p class="usual">What DJs usually keep ready: {chosen.usual}</p>
          {/if}
          <button type="button" onclick={() => (chosen = null)}>← Back</button>
        </div>
      {:else}
        <div class="decisions">
          {#each tonight.decisions as decision (decision.trouble)}
            <button
              type="button"
              class="decision"
              class:planned={decision.plan !== null}
              data-trouble={decision.trouble}
              onclick={() => (chosen = decision)}
            >
              {decision.title}
              <small>{decision.plan ? "your plan" : "no plan written"}</small>
            </button>
          {/each}
        </div>
        {#if tonight.wishes.length > 0 || tonight.never.length > 0 || tonight.contacts}
          <div class="lists">
            {#if tonight.wishes.length > 0}
              <p><span>Asked for</span> {tonight.wishes.join(" · ")}</p>
            {/if}
            {#if tonight.never.length > 0}
              <p class="never"><span>Never</span> {tonight.never.join(" · ")}</p>
            {/if}
            {#if tonight.contacts}
              <p><span>Who to call</span> {tonight.contacts}</p>
            {/if}
          </div>
        {/if}
      {/if}
    </div>
  {/if}
{/if}

<style>
  .tonight {
    display: flex;
    align-items: center;
    gap: 0.45rem;
    min-width: 0;
    max-width: 34rem;
    padding: 0.15rem 0.3rem 0.15rem 0.55rem;
    border: 1px solid var(--active);
    border-radius: 999px;
    font-size: 0.78em;
    white-space: nowrap;
    overflow: hidden;
  }

  .live {
    flex: none;
    width: 0.5rem;
    height: 0.5rem;
    border-radius: 50%;
    background: var(--active);
  }

  .title {
    flex: none;
  }

  .standing {
    color: var(--text-dim);
    flex: none;
  }

  .next {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--text-dim);
  }

  /* The next moment's last ten minutes. Colour and weight, not motion: a
     line that pulsed in the top bar all evening would be a line a DJ learns
     to ignore by the time it matters. */
  .next.soon {
    color: var(--warn);
    font-weight: 600;
  }

  .tonight.soon {
    border-color: var(--warn);
  }

  .if {
    flex: none;
    padding: 0.05rem 0.55rem;
    border-radius: 999px;
    font-weight: 600;
  }

  .end {
    flex: none;
    padding: 0 0.4rem;
    background: none;
    border: none;
    color: var(--text-dim);
  }

  .end.yes {
    color: var(--danger);
  }

  /* The quick decision: large, plain targets. */
  .decide-float {
    position: fixed;
    z-index: 62;
    width: min(30rem, calc(100vw - 2rem));
    max-height: min(32rem, calc(100vh - 8rem));
    overflow: auto;
    padding: 0.6rem;
    border: 1px solid var(--active);
    border-radius: 0.6rem;
    background: var(--panel);
    box-shadow: 0 18px 48px rgba(0, 0, 0, 0.55);
  }

  .decisions {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 0.4rem;
  }

  .decision {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 0.15rem;
    min-height: 3.2rem;
    padding: 0.45rem 0.6rem;
    text-align: left;
    font-size: 0.95em;
    font-weight: 600;
  }

  .decision small {
    font-weight: 400;
    font-size: 0.78em;
    color: var(--text-dim);
  }

  .decision.planned {
    border-color: var(--active);
  }

  .plan h3 {
    margin: 0 0 0.4rem;
    font-size: 1.05em;
  }

  .written {
    margin: 0 0 0.6rem;
    font-size: 1.15em;
    line-height: 1.45;
    white-space: pre-wrap;
  }

  .none {
    margin: 0 0 0.3rem;
    color: var(--warn);
  }

  .usual {
    margin: 0 0 0.6rem;
    color: var(--text-dim);
    line-height: 1.45;
  }

  .lists {
    margin-top: 0.5rem;
    border-top: 1px solid var(--border);
    padding-top: 0.4rem;
    font-size: 0.85em;
  }

  .lists p {
    margin: 0.2rem 0;
  }

  .lists span {
    color: var(--text-dim);
    margin-right: 0.3rem;
  }

  .lists .never {
    color: var(--danger);
  }
</style>
