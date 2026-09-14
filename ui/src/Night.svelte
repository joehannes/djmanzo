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
    assistantSetPosture,
    nightFits,
    nightNow,
    nightRead,
    nightSettings,
    noteNight,
    type Fits,
    type NightKind,
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
    /**
     * Wear a density band, by slug.
     *
     * §81's profiles know which band this DJ actually runs at for this kind of
     * night, and nothing could act on it: the shell owns the `--density`
     * property and this panel owns the setting. Passed in rather than reached
     * for, so the one function that applies a density stays the one function
     * that applies a density — a second copy here would be the band the window
     * fitted and the band a profile asked for disagreeing silently.
     */
    onDensity?: (slug: string) => void;
  }

  let { enabled, density, onDensity }: Props = $props();

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
   *
   * # Read off Rust, not written here
   *
   * The six used to be eighteen strings in this file, beside a table
   * `dj_app::setting` already owned. That is the second-description failure
   * this codebase keeps finding, and it fails in the worst direction: a seventh
   * occasion added to `Setting` would have left this panel offering six, and a
   * list that is merely short looks exactly like a list that is right. It is a
   * static table, so one read at mount is the whole cost.
   */
  let settings = $state<NightKind[]>([]);

  let tonight = $state<NightSetting | null>(null);
  let saying = $state(false);

  async function say(setting: string) {
    saying = true;
    try {
      tonight = await noteNight(setting, density);
      error = "";
      // §81's other two, at the moment the night is named. This is the one
      // place djmanzo knows *which kind of night* it is and has not yet acted
      // on what it knows about that kind.
      await fit();
    } catch (problem) {
      error = String(problem);
    } finally {
      saying = false;
    }
  }

  /**
   * §81's other two: what this kind of night's profile would fit.
   *
   * The rules are Rust's — `dj_app::profile::fits` — including the one that
   * matters: djmanzo may quiet its own assistant on a profile and may only
   * ever *offer* to make it louder, because a profile is evidence about past
   * nights and §9 forbids autonomy above confidence.
   */
  let fits = $state<Fits | null>(null);

  /**
   * The setting the one-shot has already been run for.
   *
   * **It sets; it does not own** — the contract §7's arrangements and §54's
   * setups both state. djmanzo fits the cockpit to a profile at the moment the
   * night is named and then leaves it alone; a version that applied on every
   * poll would spring back the instant a DJ moved the density themselves,
   * which is the interface arguing with its own switches.
   *
   * What is still offered afterwards is the *offer*: the rows below stay, so a
   * DJ who moved away can take it again deliberately.
   */
  let fittedFor = $state<string | null>(null);

  async function fit() {
    try {
      const got = await nightFits(density);
      fits = got;
      const once = tonight?.setting ?? null;
      if (!once || fittedFor === once) return;
      fittedFor = once;
      if (got.density?.doing === "its-own") onDensity?.(got.density.to);
      if (got.posture?.doing === "its-own") await assistantSetPosture(got.posture.to);
    } catch {
      // A profile that cannot be read changes nothing, which is the same
      // answer as a DJ with no history — and the great majority of nights.
      fits = null;
    }
  }

  /** Take an offer djmanzo would not take by itself. */
  async function take(what: "density" | "posture") {
    const offer = what === "density" ? fits?.density : fits?.posture;
    if (!offer) return;
    try {
      if (what === "density") onDensity?.(offer.to);
      else await assistantSetPosture(offer.to);
      // Ask again rather than clearing the row here: the answer is Rust's, and
      // a panel that hid an offer it had not actually applied would be the one
      // place this feature could lie.
      await fit();
    } catch (problem) {
      error = String(problem);
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
      // Kept current so an offer disappears when it is taken and reappears if
      // the DJ moves away from it. The *applying* is once per night; see
      // `fittedFor`.
      if (tonight?.setting) await fit();
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
    // Not in `refresh`: the occasions do not change while djmanzo is running,
    // and asking every two seconds for an answer that cannot have moved is the
    // thing every other poll here is written to avoid.
    void nightSettings()
      .then((kinds) => (settings = kinds))
      .catch(() => {
        // Nothing rather than a guess. A panel that invented its own six would
        // be the copy this read exists to remove.
      });
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
      {#each settings as setting (setting.slug)}
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

    <!--
      §81's density and automation tolerance, which nothing acted on until now.

      Every row says the evidence before it says the change, because that is
      the order a DJ can argue with it in: "you run Pro Dense over 6 club
      nights" is checkable and "djmanzo set the density" is not. The button is
      the only way a posture ever goes *up* — see `dj_app::profile::fits` for
      why that direction is never taken unasked.
    -->
    {#if fits?.density || fits?.posture || fits?.withheld.length}
      <div class="fits" data-testid="night-fits">
        {#if fits.density}
          <p class="fit" data-fit="density">
            <span class="because">{fits.density.because}</span>
            <button
              disabled={!enabled}
              onclick={() => take("density")}
            >Use {fits.density.name}</button>
          </p>
        {/if}
        {#if fits.posture}
          <p class="fit" data-fit="posture">
            <span class="because">{fits.posture.because}</span>
            <button
              disabled={!enabled}
              onclick={() => take("posture")}
            >Set the assistant to <span class="posture">{fits.posture.name}</span></button>
          </p>
        {/if}
        {#each fits.withheld as why (why)}
          <p class="fit withheld" data-fit="withheld">{why}</p>
        {/each}
      </div>
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
    border-color: var(--selected);
    color: var(--selected);
    background: color-mix(in srgb, var(--selected) 12%, transparent);
  }

  .hint {
    margin: 0;
    font-size: 0.68rem;
    line-height: 1.45;
    color: var(--muted);
  }

  /*
    What a profile would fit. Quiet by design: this is djmanzo saying what it
    worked out about the DJ, which belongs in the same register as the hint
    above rather than competing with the arc.
  */
  .fits {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }

  .fit {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 0.4rem;
    margin: 0;
    font-size: 0.68rem;
    line-height: 1.45;
  }

  .fit .because {
    color: var(--muted);
  }

  .fit button {
    font: inherit;
    padding: 0.1rem 0.45rem;
    border: 1px solid var(--line);
    border-radius: 0.25rem;
    background: transparent;
    color: var(--text);
    cursor: pointer;
  }

  .fit button:hover:not(:disabled) {
    border-color: var(--accent);
    color: var(--accent);
  }

  .fit button:disabled {
    opacity: 0.5;
    cursor: default;
  }

  /* The six postures are stored lower-case; this is where they are spoken. */
  .fit .posture {
    text-transform: capitalize;
  }

  /*
    A withheld fit reads as an absence rather than as an offer, because that is
    what it is: djmanzo has something to say and a lock saying not to act on it.
  */
  .fit.withheld {
    color: var(--muted);
    font-style: italic;
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
  /*
    §30's `uncertain`, which that section calls the role most often missing:
    "uncertainty needs a look of its own, or a low-confidence suggestion is
    presented exactly like a high-confidence one." This was `--warn`, which
    says *this will probably be a problem* -- a different claim from *djmanzo
    is not sure*, and `Role::must_differ_from` has no opinion about the pair
    only because they were never meant to be the same colour.
  */
  .certainty[data-certainty="unsure"] {
    color: var(--uncertain);
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
