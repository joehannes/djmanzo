<script lang="ts">
  /**
   * Building a whole night before playing any of it.
   *
   * # Why this is not just a longer list of suggestions
   *
   * The suggester answers "what next", and a set built one locally-best answer
   * at a time is an hour at a single energy. An **arc** is the thing that makes
   * it a night: it climbs, it plateaus, it comes down, and the plateau matters
   * — a set that peaks and immediately drops feels like an accident.
   *
   * So the arc is the first control, and the **shape strip** is the first
   * thing shown. Twenty-five rows of text lose exactly the property the DJ
   * came here for; a line that rises and falls does not.
   *
   * # Why steering rather than rebuilding
   *
   * A DJ who says "take it up from here" has not asked for a different night.
   * Steering keeps everything already played, keeps the next record — it may
   * be cued or staged — and rechooses the rest. Rebuilding would throw away
   * every decision they had already accepted.
   *
   * # Why a plan is not saved until it is asked for
   *
   * A draft is not a playlist. Writing every build to the library would fill
   * the crate tree with abandoned attempts, so a plan stays here until the DJ
   * says where it should go: a playlist that outlives the panel, or the
   * assistant, which will play it.
   */
  import {
    ARCS,
    ARC_HELP,
    assistantSetSetlist,
    learnedTaste,
    setlistBuild,
    setlistSave,
    setlistSteer,
    type Arc,
    type SetlistSlot,
  } from "./api";
  import { onMount } from "svelte";

  interface Props {
    enabled: boolean;
  }

  let { enabled }: Props = $props();

  /**
   * How long a set is, by default.
   *
   * Ninety minutes: a support slot or a short headline, and the length most
   * DJs would have to change least often. Long enough that the arc has room to
   * be an arc — a twenty-minute Journey is three records and a shrug.
   */
  const DEFAULT_MINUTES = 90;

  let arc = $state<Arc>("journey");
  let minutes = $state(DEFAULT_MINUTES);
  let favours = $state<string[]>([]);
  let avoids = $state<string[]>([]);
  let avoidDraft = $state("");
  let useLearned = $state(true);
  let learnedFavourites = $state<string[]>([]);

  let plan = $state<SetlistSlot[]>([]);
  let summary = $state("");
  let busy = $state(false);
  let error = $state("");
  let saved = $state("");
  let name = $state("");

  /** What the taste knows, offered rather than imposed. */
  onMount(() => {
    void (async () => {
      try {
        learnedFavourites = (await learnedTaste()).favourites;
      } catch {
        // A DJ with no history yet is the normal case, not a fault.
        learnedFavourites = [];
      }
    })();
  });

  let tilting = $derived(
    useLearned ? [...new Set([...favours, ...learnedFavourites])] : favours,
  );

  async function build() {
    busy = true;
    saved = "";
    try {
      plan = await setlistBuild(arc, minutes, tilting, avoids);
      summary = "";
      error = plan.length === 0 ? "Nothing in the library fits that." : "";
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function steer(instruction: string, argument?: string) {
    if (plan.length === 0) return;
    busy = true;
    saved = "";
    try {
      // `played` is zero: nothing has been played, this is a plan. The one
      // slot the steer protects is therefore the opener, which is right — a
      // DJ adjusting the shape has not asked for a different first record.
      const out = await setlistSteer(plan, 0, instruction, argument);
      plan = out.plan;
      summary = out.summary;
      error = "";
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  function addAvoid() {
    const wanted = avoidDraft.trim();
    if (!wanted || avoids.includes(wanted)) return;
    avoids = [...avoids, wanted];
    avoidDraft = "";
  }

  async function save() {
    busy = true;
    try {
      const count = plan.length;
      await setlistSave(name.trim() || `${arc} · ${minutes} min`, plan.map((s) => s.track.id));
      saved = `Saved ${count} record${count === 1 ? "" : "s"} as a playlist.`;
      error = "";
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function handOver() {
    busy = true;
    try {
      const count = await assistantSetSetlist(plan.map((s) => s.track.id));
      saved = `The assistant has ${count} record${count === 1 ? "" : "s"} to play.`;
      error = "";
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  /**
   * The shape of the night as a path.
   *
   * A running total of what each slot's trajectory does — up for a lift, down
   * for an ease, level for a hold — normalised to the strip. It is the set's
   * *intent*, which is what the DJ is deciding about here; how loud each
   * record actually turns out to be is a different question and belongs on a
   * waveform, not on a plan.
   */
  let shape = $derived.by(() => {
    if (plan.length === 0) return "";
    let energy = 0;
    const points = plan.map((slot, i) => {
      energy +=
        slot.trajectory === "lift" ? 1 : slot.trajectory === "ease" ? -1 : 0;
      return { x: plan.length === 1 ? 0 : i / (plan.length - 1), energy };
    });
    const highest = Math.max(...points.map((p) => p.energy));
    const lowest = Math.min(...points.map((p) => p.energy));
    // A flat set is a flat line down the middle, not a division by zero.
    const span = highest - lowest || 1;
    return points
      .map(
        (p, i) =>
          `${i === 0 ? "M" : "L"} ${(p.x * 100).toFixed(2)} ${(
            28 - ((p.energy - lowest) / span) * 24
          ).toFixed(2)}`,
      )
      .join(" ");
  });

  /**
   * How many seams in the plan need a decision rather than a blend.
   *
   * Counted and stated, because the number is the thing a DJ wants before
   * reading twenty-five rows: a plan with two difficult joins is a plan to
   * play, and one with eleven is one to rebuild.
   */
  let risky = $derived(plan.filter((slot) => slot.link?.risky).length);

  function minutesOf(seconds: number): string {
    return `${Math.round(seconds / 60)}`;
  }

  let totalMinutes = $derived(
    Math.round(plan.reduce((n, s) => n + s.track.duration_seconds, 0) / 60),
  );
</script>

<div class="plan">
  <div class="setup">
    <!--
      A row, like the posture ladder: choosing between "journey" and "descent"
      is a decision about the whole night and should not mean opening a menu.
    -->
    <div class="arcs" role="group" aria-label="The shape of the night">
      {#each ARCS as choice (choice)}
        <button
          class:active={arc === choice}
          title={ARC_HELP[choice]}
          onclick={() => (arc = choice)}
        >
          {choice}
        </button>
      {/each}
    </div>
    <p class="help">{ARC_HELP[arc]}</p>

    <label class="minutes">
      <span>Minutes</span>
      <input type="number" min="10" max="600" step="10" bind:value={minutes} />
    </label>

    {#if learnedFavourites.length > 0}
      <!--
        Offered, not imposed. The taste steers the ranking, and a DJ building a
        set for somebody else's party should be able to turn it off in one
        click rather than wonder why their own habits keep surfacing.
      -->
      <label class="learned">
        <input type="checkbox" bind:checked={useLearned} />
        <span>Lean towards what I usually play</span>
        <span class="chips">
          {#each learnedFavourites as family (family)}
            <span class="chip">{family}</span>
          {/each}
        </span>
      </label>
    {/if}

    <div class="avoid">
      <label>
        <span>Keep out</span>
        <input
          bind:value={avoidDraft}
          placeholder="a genre, strictly"
          onkeydown={(e) => e.key === "Enter" && addAvoid()}
        />
      </label>
      <button onclick={addAvoid} disabled={!avoidDraft.trim()}>Add</button>
      {#each avoids as family (family)}
        <button
          class="chip removable"
          title="Allow {family} again"
          onclick={() => (avoids = avoids.filter((a) => a !== family))}
        >
          {family} ×
        </button>
      {/each}
    </div>

    <button class="build" disabled={!enabled || busy} onclick={build}>
      {plan.length > 0 ? "Build again" : "Build the set"}
    </button>
  </div>

  {#if error}
    <p class="error">{error}</p>
  {/if}

  {#if plan.length > 0}
    <!--
      The shape first. It is the property an arc exists to give a set, and the
      one a list of names cannot show.
    -->
    <svg class="shape" viewBox="0 0 100 30" preserveAspectRatio="none" aria-hidden="true">
      <path d={shape} />
    </svg>
    <p class="totals">
      <strong>{plan.length}</strong> records · about
      <strong>{totalMinutes}</strong> minutes{#if risky > 0} ·
        <strong class="warn">{risky}</strong>
        {risky === 1 ? "seam needs" : "seams need"} a cut{/if}
    </p>

    <div class="steer">
      <span class="lead">Steer</span>
      <button disabled={busy} onclick={() => steer("lift")}>take it up</button>
      <button disabled={busy} onclick={() => steer("hold")}>hold it</button>
      <button disabled={busy} onclick={() => steer("ease")}>bring it down</button>
    </div>

    {#if summary}
      <p class="summary">{summary}</p>
    {/if}

    <ol class="slots">
      {#each plan as slot, i (slot.track.id + ":" + i)}
        <!--
          The seam, drawn between the two records it joins rather than on
          either of them.

          §20's Set Flow asks for transition links and risk markers, and this
          is both: what changes across the join, and whether the join needs a
          decision instead of a blend. It sits between the rows because that
          is where it *is* -- putting it on the second record's line would read
          as a fact about that record, which it is not.
        -->
        {#if slot.link}
          <li class="seam" class:risky={slot.link.risky}>
            <span class="thread" aria-hidden="true"></span>
            <span class="seam-text">
              {slot.link.summary}{#if slot.link.risky} · <strong>needs a cut</strong>{/if}
            </span>
          </li>
        {/if}
        <li class:odd={i % 2 === 0}>
          <span class="mono position">{i + 1}</span>
          <span class="arrow" title={slot.trajectory}>
            {slot.trajectory === "lift" ? "↗" : slot.trajectory === "ease" ? "↘" : "→"}
          </span>
          <span class="named">
            <span class="title">{slot.track.title}</span>
            <span class="artist">{slot.track.artist}</span>
          </span>
          <span class="mono length">{minutesOf(slot.track.duration_seconds)}′</span>
          <!--
            "Later" rather than "drop" as the first control: a DJ saying "not
            yet" has not said "never", and the two want different buttons.
          -->
          <button
            class="tiny"
            disabled={busy}
            title="Not yet — move it later"
            onclick={() => steer("later", slot.track.id)}
          >↓</button>
          <button
            class="tiny"
            disabled={busy}
            title="Take it out of the set"
            onclick={() => steer("drop", slot.track.id)}
          >×</button>
        </li>
      {/each}
    </ol>

    <!--
      The result sits above the buttons, not below.

      Everything under the last control is off the bottom edge in a short
      panel, and what lands there is exactly what the DJ needs after pressing:
      whether it worked. Above them it is also where the eye already is.
    -->
    {#if saved}
      <p class="saved">{saved}</p>
    {/if}

    <div class="keep">
      <input bind:value={name} placeholder="{arc} · {minutes} min" aria-label="Name for the set" />
      <button disabled={busy} onclick={save}>Save as a playlist</button>
      <button disabled={busy} onclick={handOver}>Hand to the assistant</button>
    </div>
  {/if}
</div>

<style>
  .plan {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    flex: 1;
    min-height: 0;
    overflow: auto;
  }

  .setup {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    flex: none;
  }

  .arcs {
    display: flex;
    gap: 0.25rem;
    flex-wrap: wrap;
  }

  .arcs button.active {
    border-color: var(--accent);
    color: var(--accent);
  }

  .help,
  .totals,
  .summary,
  .saved,
  .error {
    margin: 0;
    font-size: 0.8em;
    line-height: 1.5;
    color: var(--text-dim);
  }

  .error {
    color: var(--danger, #dc2626);
  }

  .saved {
    color: var(--accent);
  }

  .minutes,
  .learned,
  .avoid label {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.82em;
  }

  .minutes input {
    width: 5rem;
  }

  .learned,
  .avoid {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    flex-wrap: wrap;
    font-size: 0.82em;
  }

  .chips {
    display: flex;
    gap: 0.25rem;
    flex-wrap: wrap;
  }

  .chip {
    padding: 0.05rem 0.35rem;
    border: 1px solid var(--border);
    border-radius: 3px;
    font-size: 0.9em;
    color: var(--accent);
  }

  .chip.removable {
    cursor: pointer;
    color: var(--warn, #d97706);
  }

  .build {
    align-self: flex-start;
  }

  /*
    The shape of the night. Deliberately unlabelled and unscaled — it is not a
    measurement, it is the silhouette of what was planned, and axes would
    invite reading precision into it that is not there.
  */
  .shape {
    flex: none;
    width: 100%;
    height: 2.6rem;
  }

  .shape path {
    fill: none;
    stroke: var(--accent);
    stroke-width: 1.2;
    stroke-linejoin: round;
    vector-effect: non-scaling-stroke;
  }

  .steer {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    flex-wrap: wrap;
    flex: none;
  }

  .lead {
    font-size: 0.75em;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: var(--text-dim);
  }

  .slots {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
  }

  .slots li {
    display: flex;
    align-items: center;
    gap: 0.45rem;
    padding: 0.2rem 0.35rem;
    border-radius: 4px;
    font-size: 0.82em;
  }

  /*
    Striped by the record's own position, not by `:nth-child`.

    The seams are list items too, so a positional selector counts them and the
    stripe stops tracking the records -- which is the one thing a stripe is
    for. The index is already in the template; using it is one class and no
    guessing.
  */
  .slots li.odd {
    background: color-mix(in srgb, var(--text) 4%, transparent);
  }

  .position,
  .length {
    color: var(--text-dim);
    flex: none;
  }

  .arrow {
    flex: none;
    color: var(--accent);
  }

  .named {
    flex: 1;
    min-width: 0;
    display: flex;
    gap: 0.4rem;
    overflow: hidden;
  }

  .title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .artist {
    color: var(--text-dim);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .tiny {
    padding: 0 0.3rem;
    font-size: 0.9em;
    flex: none;
  }

  .keep {
    display: flex;
    gap: 0.35rem;
    flex-wrap: wrap;
    align-items: center;
    flex: none;
  }

  .keep input {
    flex: 1;
    min-width: 8rem;
  }

  /*
    A seam is a thread between two records, not a row of its own.

    Deliberately quieter than the records it joins: it is context for them,
    and a join drawn as loudly as a title would make a plan read as fifty
    things rather than twenty-five.
  */
  .seam {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0 0.35rem 0 1.1rem;
    font-size: 0.7em;
    color: var(--text-dim);
    background: none;
  }

  .seam .thread {
    width: 1px;
    height: 0.85em;
    background: currentColor;
    opacity: 0.5;
    flex: none;
  }

  .seam-text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* A seam that needs a cut is the one thing on this panel worth interrupting
     a scan for, so it is the only thing that changes colour. */
  .seam.risky {
    color: var(--danger, #dc2626);
  }

  .warn {
    color: var(--danger, #dc2626);
  }
</style>
