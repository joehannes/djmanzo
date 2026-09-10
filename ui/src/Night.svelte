<script lang="ts">
  /**
   * Where the set is in its arc, and what says so.
   *
   * # It shows its working
   *
   * A panel that said "Peak" and nothing else would be asking to be believed.
   * Every field here is one half of a claim djmanzo can defend: the phase, what
   * produced it, how sure that makes it, and — where the DJ's occasion and the
   * night's own evidence disagree — which way the disagreement runs. The
   * sentences come from Rust, from the same reading the theme and the autopilot
   * see, so this panel cannot be showing a night the rest of the application is
   * not acting on.
   *
   * # Waiting is a state, not a blank
   *
   * `dj_core::ContextEngine` will not name a phase from evidence until it has
   * about six minutes of music to compare the last few minutes against. Before
   * that the honest answer is that nothing has read the night, and this says so
   * with the count — because an empty panel reads as broken, and a DJ who has
   * just opened the application deserves to know it is listening rather than
   * stuck.
   */
  import { onMount } from "svelte";
  import {
    nightNow,
    nightRead,
    noteNight,
    type NightRead,
    type NightSetting,
  } from "./api";
  import { ARC, BASIS, CERTAINTY, WARRANT, WHEN } from "./night";

  interface Props {
    /** False before an audio device is open, when there is no set to read. */
    enabled: boolean;
    /**
     * The density band the interface is actually running at, by name.
     *
     * Passed down rather than worked out here: App picks the band from the
     * window height, and a second calculation would eventually disagree with
     * the one that set the font size. It goes to Rust with the setting,
     * because §81 lists density among the things a profile differs in and this
     * side is the only thing that knows it.
     */
    density?: string;
  }

  let { enabled, density }: Props = $props();

  /**
   * §81: what kind of night this is.
   *
   * A different axis from the arc above. djmanzo reads the arc from the music
   * and is right to — energy and tempo are in the signal. **Nothing in the
   * signal says wedding.** A room dancing at 128 BPM is a club or a wedding
   * according to facts no microphone has, so this is told rather than guessed:
   * guessing it would file a whole night's habits under the wrong name, which
   * is §13's failure at the scale of an evening.
   *
   * It lives in this panel because this is where the night is. Two homes for
   * "what is tonight" would be two places to go and check.
   */
  const SETTINGS: { slug: string; title: string; about: string }[] = [
    { slug: "club", title: "Club", about: "A crowd that came to dance, and knows what it came for." },
    { slug: "beach", title: "Beach / sunset", about: "Warm, unhurried, and nobody is waiting for a drop." },
    { slug: "wedding", title: "Wedding", about: "A room that is not there for the DJ." },
    { slug: "latin", title: "Latin", about: "Where the technique is the genre's, not the format's." },
    { slug: "practice", title: "Practice", about: "Nobody is listening. What happens here is not a gig." },
    { slug: "open-format", title: "Open format", about: "Whatever the room turns out to want." },
  ];

  let tonight = $state<NightSetting | null>(null);
  let saying = $state(false);

  async function say(setting: string) {
    saying = true;
    try {
      tonight = await noteNight(setting, density);
      error = "";
    } catch (problem) {
      error = String(problem);
    } finally {
      saying = false;
    }
  }

  /**
   * How often the reading is fetched.
   *
   * Two seconds. The engine keeps one reading every four and a phase needs
   * three of them to change, so anything faster would ask a question whose
   * answer cannot have moved.
   */
  const EVERY_MS = 2000;

  let read = $state<NightRead | null>(null);
  let error = $state("");
  let timer: ReturnType<typeof setInterval> | undefined;

  async function refresh() {
    try {
      // Keep tonight's figures current while there is a log to read them
      // from. Only once the DJ has said what kind of night it is: writing a
      // row for a night nobody has named would put it in the open-format
      // profile by default, which is the guess this panel exists to avoid.
      tonight = tonight?.setting
        ? await noteNight(undefined, density)
        : await nightNow();
      read = await nightRead();
      error = "";
    } catch (problem) {
      error = String(problem);
    }
  }

  /**
   * Asked for once on mount whatever happens, then only while there is a set.
   *
   * The first fetch is unconditional because the panel has something true to
   * say before a device is open — that nothing has read the night — and a
   * surface that draws nothing at all until the engine starts is the blank
   * panel this whole design argues against.
   */
  function tick() {
    if (enabled) void refresh();
  }

  onMount(() => {
    void refresh();
    timer = setInterval(tick, EVERY_MS);
    return () => clearInterval(timer);
  });

  /** Whichever phase the arc should mark, or none. */
  const here = $derived(read?.phase ?? null);
  /** How far through the wait, 0..1, for the bar shown before any reading. */
  const waited = $derived.by(() => {
    if (!read) return 0;
    const total = read.readings + read.still_needed;
    return total > 0 ? read.readings / total : 0;
  });
  const percent = (value: number) => `${Math.round(value * 100)}%`;
</script>

<div class="night">
  {#if error}
    <p class="problem" role="alert">{error}</p>
  {/if}

  {#if read}
    <!--
      The sentences lead, because the first thing a DJ wants from this panel is
      a sentence. Everything below is the evidence for it.
    -->
    <div class="says">
      {#each read.notes as note (note)}
        <p class:lead={note === read.notes[0]}>{note}</p>
      {/each}
    </div>

    <!--
      The arc is a row rather than a dial: five names in the order a night goes
      through them, with the one djmanzo believes marked. A dial would imply a
      precision the reading does not have.
    -->
    <ol class="arc" aria-label="Where the set is in its arc">
      {#each ARC as step (step.phase)}
        <li
          class:here={step.phase === here}
          class:reads={step.phase === read.measured && step.phase !== here}
          aria-current={step.phase === here ? "step" : undefined}
        >
          {step.label}
        </li>
      {/each}
    </ol>

    <!--
      What kind of night, as opposed to where in it. Told, never read: see the
      note in the script. The chosen one stays marked so a DJ can see at a
      glance that djmanzo knows, and can correct it if they picked wrong.
    -->
    <div class="kind" role="group" aria-label="What kind of night this is">
      <span class="label">Tonight is</span>
      {#each SETTINGS as setting (setting.slug)}
        <button
          class:on={tonight?.setting === setting.slug}
          disabled={!enabled || saying}
          onclick={() => say(setting.slug)}
          title={setting.about}
        >{setting.title}</button>
      {/each}
    </div>
    {#if !tonight?.setting}
      <p class="hint" data-testid="night-unsaid">
        Say what kind of night this is and djmanzo keeps what it learns under
        that heading, rather than averaging your weddings with your club nights.
      </p>
    {/if}

    {#if read.phase}
      <dl class="facts">
        <div>
          <dt>Energy</dt>
          <dd>
            <span
              class="bar"
              style="--fill: {percent(read.energy ?? 0)}"
              aria-hidden="true"
            ></span>
            {percent(read.energy ?? 0)}
          </dd>
        </div>
        <div>
          <dt>From</dt>
          <dd>{BASIS[read.basis ?? "nothing"]}</dd>
        </div>
        <div>
          <dt>Certainty</dt>
          <dd class="certainty" data-certainty={read.certainty}>
            {CERTAINTY[read.certainty ?? "unsure"]}
            {#if read.certainty_about}
              <span class="about">{read.certainty_about}</span>
            {/if}
          </dd>
        </div>
        {#if read.time_of_day}
          <div>
            <dt>Clock</dt>
            <dd>{WHEN[read.time_of_day] ?? read.time_of_day}</dd>
          </div>
        {/if}
      </dl>
    {:else}
      <!--
        Nothing read yet. The count is the point: it says the engine is
        listening rather than broken, and how much longer it needs.
      -->
      <p class="waiting">
        <span
          class="bar"
          style="--fill: {percent(waited)}"
          aria-hidden="true"
        ></span>
        {read.readings} readings of tonight so far.
      </p>
    {/if}

    <p class="warrant" data-warrant={read.warrant}>
      {WARRANT[read.warrant] ?? read.warrant}
    </p>
  {:else if !error}
    <p class="waiting">Asking djmanzo what the night is…</p>
  {/if}

  <details class="about-panel">
    <summary>How this is worked out</summary>
    <p>
      Two things, and only two. <strong>What you declared</strong> — the occasion
      you set — and <strong>what the set has actually done</strong>, which is
      where the last few minutes sit in the whole night's own range of loudness
      and tempo.
    </p>
    <p>
      Neither is an absolute. "Loud" is a number about a room, a rig and a
      mastering engineer, so djmanzo never compares tonight with any other
      night; it compares tonight with itself. That takes about six minutes of
      music before it means anything, and until then the music says nothing.
    </p>
    <p>
      <strong>Where the two disagree, you win.</strong> djmanzo says so and names
      which way its evidence points, and it will not mix unasked while they
      disagree — but it never overrules the person who has been in the room all
      night.
    </p>
  </details>
</div>

<style>
  .night {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    flex: 1;
    min-height: 0;
    overflow: auto;
  }

  .says {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .says p {
    margin: 0;
    font-size: 0.85em;
    color: var(--text-dim);
    line-height: 1.5;
  }

  .says p.lead {
    font-size: 1em;
    color: var(--text);
  }

  /* Five names in the order a night goes through them. */
  /* What kind of night. Told rather than read, and marked so. */
  .kind {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.25rem;
  }

  .kind .label {
    font-size: 0.66rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--muted);
    margin-right: 0.2rem;
  }

  .kind button {
    font: inherit;
    font-size: 0.68rem;
    padding: 0.18rem 0.5rem;
    border-radius: 999px;
    border: 1px solid var(--line);
    background: var(--surface);
    color: var(--text);
    cursor: pointer;
  }

  .kind button:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .kind button.on {
    border-color: var(--accent);
    color: var(--accent);
    background: color-mix(in srgb, var(--accent) 12%, transparent);
  }

  .hint {
    margin: 0;
    font-size: 0.68rem;
    line-height: 1.45;
    color: var(--muted);
  }

  .arc {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .arc li {
    flex: 1 1 auto;
    text-align: center;
    padding: 0.2rem 0.35rem;
    border: 1px solid var(--border);
    border-radius: 4px;
    font-size: 0.72em;
    color: var(--text-dim);
    white-space: nowrap;
  }

  .arc li.here {
    color: var(--text);
    border-color: var(--accent);
    background: var(--panel-raised);
    font-weight: 600;
  }

  /*
    Where the music reads, when that is not where the night is. Dashed rather
    than solid because it is not the answer -- djmanzo keeps the DJ's word and
    marks its own reading beside it.
  */
  .arc li.reads {
    border-style: dashed;
    border-color: var(--warn);
    color: var(--text);
  }

  .facts {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem 0.9rem;
    margin: 0;
    font-size: 0.8em;
  }

  .facts div {
    display: flex;
    gap: 0.35rem;
    align-items: baseline;
  }

  .facts dt {
    color: var(--text-dim);
  }

  .facts dd {
    margin: 0;
    display: flex;
    gap: 0.35rem;
    align-items: center;
  }

  .certainty {
    text-transform: capitalize;
  }

  .certainty .about {
    color: var(--text-dim);
    text-transform: none;
    font-size: 0.92em;
  }

  /* Unsure is the one worth noticing: it is the state that stops the mix. */
  .certainty[data-certainty="unsure"] {
    color: var(--warn);
  }

  .bar {
    display: inline-block;
    width: 4.5rem;
    height: 0.45rem;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: linear-gradient(
      to right,
      var(--accent) var(--fill),
      transparent var(--fill)
    );
  }

  .waiting,
  .warrant {
    margin: 0;
    font-size: 0.8em;
    color: var(--text-dim);
    display: flex;
    gap: 0.4rem;
    align-items: center;
  }

  .warrant {
    border-top: 1px solid var(--border);
    padding-top: 0.45rem;
  }

  .problem {
    margin: 0;
    font-size: 0.85em;
    color: var(--danger);
  }

  .about-panel {
    font-size: 0.8em;
    color: var(--text-dim);
  }

  .about-panel p {
    line-height: 1.55;
  }
</style>
