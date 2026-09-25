<script lang="ts">
  /**
   * §118b: the welcome.
   *
   * > a new user is welcomed by a guide that introduces to the capabilities
   * > and metaphorical usage and navigation and features and workflow of the
   * > app ... the welcome guide of above also gets to know the DJ ... he asks
   * > him for his favorite genres/music/artists/songs/bpm/styles/
   * > combinations/tricks/transitions/techniques ... and automatically
   * > creates activities and personalised preset-packages for him, even his
   * > favorite colors and assembles a personalised theme for him, and auto
   * > puts a personalised app logo as of his DJ design/logo he will be asked
   * > for.
   *
   * A card over the booth, a question at a time, every one of them skippable.
   * The answers are kept as they are given (`welcome.json`), so a welcome put
   * away half way opens where it was left. The last step shows, in Rust's
   * sentences (`welcome::plan`), exactly what setting up will do -- nothing
   * changes before the DJ says so -- and **Set it up** does it through the
   * same commands a DJ's own presses use.
   *
   * The lists offered are the tables that own them: §81's nights, the genre
   * families and the moves come from the event panel's options, §8's levels
   * from `adaptation_levels`, the looks from the theme packages.
   */
  import { untrack } from "svelte";
  import { portal } from "./controls/portal";
  import { themePackages } from "./controls/themes/packages";
  import { theme } from "./theme.svelte";
  import {
    adaptationLevels,
    eventOptions,
    themeChosen,
    welcomeApply,
    welcomePlan,
    welcomeSave,
    type AdaptationLevel,
    type GigOptions,
    type WelcomeAnswers,
    type WelcomeApplied,
    type WelcomePlan,
  } from "./api";

  let {
    answers: given,
    onPickLogo,
    onApplied,
    onClose,
  }: {
    answers: WelcomeAnswers;
    /** Open the logo picker the top bar already has. */
    onPickLogo?: () => void;
    /** Set up: the interface opens the arrangement and wears the theme. */
    onApplied: (applied: WelcomeApplied) => void;
    /** Put away, finished or not. */
    onClose: () => void;
  } = $props();

  const STEPS = ["hello", "nights", "music", "moves", "help", "look", "ready"] as const;
  type Step = (typeof STEPS)[number];

  // A draft of the answers it was opened with, copied once on purpose: what
  // the DJ changes here is kept through `welcome_save`, not pushed back up.
  let answers = $state<WelcomeAnswers>(
    untrack(() => structuredClone($state.snapshot(given) as WelcomeAnswers)),
  );
  let step = $state<Step>("hello");
  let options = $state<GigOptions | null>(null);
  let levels = $state<AdaptationLevel[]>([]);
  let plan = $state<WelcomePlan | null>(null);
  let error = $state<string | null>(null);
  /**
   * What Rust refused, with the answers it refused: said only while those
   * are still the answers, so a tempo put right stops being called wrong.
   */
  let refused = $state<{ message: string; of: string } | null>(null);
  const refusal = $derived(
    refused && refused.of === JSON.stringify($state.snapshot(answers)) ? refused.message : null,
  );
  let applying = $state(false);
  /** The look the DJ had, so un-choosing a look puts it back at once. */
  const lookBefore = theme.activePackage.id;

  $effect(() => {
    void (async () => {
      try {
        [options, levels] = await Promise.all([eventOptions(), adaptationLevels()]);
      } catch (e) {
        error = String(e);
      }
    })();
  });

  const at = $derived(STEPS.indexOf(step));

  /** The answers as Rust takes them. */
  function sent(): WelcomeAnswers {
    return $state.snapshot(answers) as WelcomeAnswers;
  }

  /** Keep what was said so far. */
  async function keep() {
    const said = sent();
    try {
      answers = await welcomeSave(said);
      refused = null;
      return true;
    } catch (e) {
      refused = { message: String(e), of: JSON.stringify(said) };
      return false;
    }
  }

  async function go(to: Step) {
    if (!(await keep())) return;
    step = to;
    if (to === "ready") {
      try {
        plan = await welcomePlan(sent());
      } catch (e) {
        error = String(e);
      }
    }
  }

  async function close() {
    await keep().catch(() => {});
    onClose();
  }

  async function apply() {
    applying = true;
    try {
      const applied = await welcomeApply(sent());
      onApplied(applied);
    } catch (e) {
      error = String(e);
    } finally {
      applying = false;
    }
  }

  /**
   * Wear a look as it is chosen, or the one there was when it is un-chosen.
   *
   * Declared as well as painted: §31 adapts the theme on its own tick, and a
   * look only painted is put back a few seconds later -- which in the booth
   * read as the welcome forgetting the choice between two of its steps.
   */
  function wear(id: string) {
    answers.theme = id;
    const worn = id || lookBefore;
    theme.setPackage(worn);
    void themeChosen(worn).catch(() => {});
  }

  function toggle<T>(list: T[], item: T) {
    const index = list.indexOf(item);
    if (index >= 0) list.splice(index, 1);
    else list.push(item);
  }

  /**
   * Escape puts the welcome away. Every other key is left alone here: the
   * booth's shortcuts stand down by themselves while an `aria-modal` dialog
   * is open (`typing()` in `keyboard.svelte.ts`). Stopping every key at the
   * window, which the first version did, broke typing in WebKitGTK -- after
   * a space, nothing more reached the name field -- where Chromium typed on.
   */
  $effect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.stopPropagation();
      void close();
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  });

  const nightTitle = (slug: string) => options?.nights.find((n) => n.slug === slug)?.title ?? slug;
</script>

<div class="welcome-backdrop" use:portal>
  <div class="welcome" role="dialog" aria-modal="true" aria-label="Welcome to djmanzo" data-welcome-step={step}>
    <header>
      <ol class="dots" aria-label="Step {at + 1} of {STEPS.length}">
        {#each STEPS as s, i (s)}
          <li class:done={i < at} class:here={i === at}></li>
        {/each}
      </ol>
      <button type="button" class="skip" onclick={() => void close()}>Not now</button>
    </header>

    {#if step === "hello"}
      <h2>Welcome to djmanzo</h2>
      <p class="lead">
        A DJ application that learns how you play. Two decks and a mixer in the middle; panels
        that dock beside them and lift over them when you need room; <strong>activities</strong>
        — dig, mix, perform, prepare, an event — that arrange the screen for what you are doing,
        on the number keys; and an assistant that suggests and never takes over unless you let it.
      </p>
      <p>A few questions, and djmanzo sets itself up the way you play. Every one can be skipped.</p>
      <label class="field">What do you play as?
        <input bind:value={answers.name} placeholder="Your DJ name" />
      </label>
      {#if onPickLogo}
        <p class="hint">
          Have a logo? <button type="button" class="link" onclick={onPickLogo}>Put it in the top bar</button>
          — djmanzo steps out of the way.
        </p>
      {/if}
    {:else if step === "nights"}
      <h2>Where do you play?</h2>
      <p class="lead">Each kind of night gets its own set-up and its own activity. Choose the one you play most first.</p>
      <div class="choices">
        {#each options?.nights ?? [] as night (night.slug)}
          {@const order = answers.nights.indexOf(night.slug)}
          <button
            type="button"
            class="choice"
            class:on={order >= 0}
            aria-pressed={order >= 0}
            data-night={night.slug}
            onclick={() => toggle(answers.nights, night.slug)}
          >
            {night.title}{#if order >= 0}<span class="order">{order + 1}</span>{/if}
          </button>
        {/each}
      </div>
    {:else if step === "music"}
      <h2>Your music</h2>
      <p class="lead">The genres you play, the tempo you are at home in, and the artists and records you love.</p>
      <div class="choices small">
        {#each options?.genres ?? [] as genre (genre)}
          <button
            type="button"
            class="choice"
            class:on={answers.genres.includes(genre)}
            aria-pressed={answers.genres.includes(genre)}
            onclick={() => toggle(answers.genres, genre)}>{genre}</button
          >
        {/each}
      </div>
      <div class="row">
        <label class="field">From
          <input
            type="number"
            min="40"
            max="220"
            placeholder="slowest"
            aria-label="Slowest BPM"
            value={answers.bpm_low || ""}
            oninput={(e) => (answers.bpm_low = Number(e.currentTarget.value) || 0)}
          />
        </label>
        <label class="field">to
          <input
            type="number"
            min="40"
            max="220"
            placeholder="fastest"
            aria-label="Fastest BPM"
            value={answers.bpm_high || ""}
            oninput={(e) => (answers.bpm_high = Number(e.currentTarget.value) || 0)}
          />
        </label>
        <span class="hint">BPM</span>
      </div>
      <label class="field">Artists and records you love, one a line
        <textarea
          rows="3"
          value={answers.favourites.join("\n")}
          oninput={(e) => (answers.favourites = e.currentTarget.value.split("\n"))}
        ></textarea>
      </label>
    {:else if step === "moves"}
      <h2>Your moves</h2>
      <p class="lead">What you reach for already — and what you would like to learn, which every new event then starts with, to learn and rehearse.</p>
      <div class="moves">
        {#each options?.moves ?? [] as move (move.name)}
          {@const plays = answers.moves.includes(move.name)}
          {@const learns = answers.learn.includes(move.name)}
          <div class="move" data-move={move.name} title={move.what}>
            <span>{move.name}</span>
            <button
              type="button"
              class:on={plays}
              aria-pressed={plays}
              aria-label="I play {move.name}"
              onclick={() => {
                toggle(answers.moves, move.name);
                if (answers.learn.includes(move.name)) toggle(answers.learn, move.name);
              }}>I play it</button
            >
            <button
              type="button"
              class:on={learns}
              aria-pressed={learns}
              aria-label="Learn {move.name}"
              onclick={() => {
                toggle(answers.learn, move.name);
                if (answers.moves.includes(move.name)) toggle(answers.moves, move.name);
              }}>Learn it</button
            >
          </div>
        {/each}
      </div>
    {:else if step === "help"}
      <h2>How much should djmanzo do?</h2>
      <p class="lead">From nothing at all to mixing on its own. You can change it any time in Settings.</p>
      <div class="levels">
        {#each levels as level (level.slug)}
          <button
            type="button"
            class="level"
            class:on={answers.level === level.slug}
            aria-pressed={answers.level === level.slug}
            data-level={level.slug}
            onclick={() => (answers.level = answers.level === level.slug ? "" : level.slug)}
          >
            <strong>{level.number} · {level.title}</strong>
            <span>{level.about}</span>
          </button>
        {/each}
      </div>
    {:else if step === "look"}
      <h2>Your look</h2>
      <p class="lead">Choose the one that feels like your nights — it changes as you choose. Leave it and each kind of night wears its own.</p>
      <div class="choices looks">
        {#each themePackages as pkg (pkg.id)}
          <button
            type="button"
            class="choice look"
            class:on={answers.theme === pkg.id}
            aria-pressed={answers.theme === pkg.id}
            data-theme-choice={pkg.id}
            title={pkg.when}
            onclick={() => wear(answers.theme === pkg.id ? "" : pkg.id)}>{pkg.name}</button
          >
        {/each}
      </div>
    {:else if step === "ready"}
      <h2>{answers.name ? `Ready, ${answers.name}` : "Ready"}</h2>
      {#if plan && plan.says.length > 0}
        <p class="lead">Setting up will:</p>
        <ul class="says" data-plan>
          {#each plan.says as line, i (i)}
            <li>
              {line}
              {#if i === 0 && plan.setup && plan.changes.length > 0}
                <ul class="changes">
                  {#each plan.changes as change, j (j)}
                    <li>{change}</li>
                  {/each}
                </ul>
              {/if}
            </li>
          {/each}
        </ul>
        {#if answers.nights.length > 1}
          <p class="hint">Your other nights — {answers.nights.slice(1).map(nightTitle).join(", ")} — are an activity away.</p>
        {/if}
      {:else}
        <p class="lead">Nothing chosen, so nothing will change. djmanzo starts as it is.</p>
      {/if}
      <p class="hint">
        Then: the number keys switch activity, <kbd>Space</kbd> shows every key, <kbd>0</kbd> opens
        the dashboard, and <strong>9</strong> prepares an event.
      </p>
    {/if}

    {#if error ?? refusal}
      <p class="error" role="alert">{error ?? refusal}</p>
    {/if}

    <footer>
      {#if at > 0}
        <button type="button" onclick={() => void go(STEPS[at - 1])}>Back</button>
      {/if}
      <span class="spacer"></span>
      {#if step === "ready"}
        <button type="button" class="primary" disabled={applying} onclick={() => void apply()}>
          {applying ? "Setting up…" : "Set it up"}
        </button>
      {:else}
        <button type="button" class="primary" onclick={() => void go(STEPS[at + 1])}>Next</button>
      {/if}
    </footer>
  </div>
</div>

<style>
  .welcome-backdrop {
    position: fixed;
    inset: 0;
    z-index: 80;
    display: grid;
    place-items: center;
    background: color-mix(in srgb, var(--bg, #000) 70%, transparent);
  }

  .welcome {
    width: min(38rem, calc(100vw - 2rem));
    max-height: calc(100vh - 3rem);
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    padding: 1rem 1.2rem;
    border: 1px solid var(--active);
    border-radius: 0.8rem;
    background: var(--panel);
    box-shadow: 0 24px 64px rgba(0, 0, 0, 0.6);
  }

  header {
    display: flex;
    align-items: center;
  }

  .dots {
    display: flex;
    gap: 0.3rem;
    list-style: none;
    margin: 0;
    padding: 0;
    flex: 1;
  }

  .dots li {
    width: 0.5rem;
    height: 0.5rem;
    border-radius: 50%;
    background: var(--border);
  }

  .dots li.done {
    background: var(--active);
  }

  .dots li.here {
    background: var(--selected);
  }

  .skip,
  .link {
    background: none;
    border: none;
    padding: 0;
    color: var(--text-dim);
    font: inherit;
    cursor: pointer;
  }

  .link {
    color: var(--accent);
  }

  h2 {
    margin: 0;
    font-size: 1.3em;
  }

  .lead {
    margin: 0;
    line-height: 1.5;
  }

  p {
    margin: 0;
    line-height: 1.45;
  }

  .hint {
    color: var(--text-dim);
    font-size: 0.9em;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    font-size: 0.9em;
    color: var(--text-dim);
  }

  .field input,
  .field textarea {
    font: inherit;
    color: var(--text);
  }

  .row {
    display: flex;
    gap: 0.5rem;
    align-items: flex-end;
  }

  .row input {
    width: 6.5rem;
  }

  .choices {
    display: flex;
    flex-wrap: wrap;
    gap: 0.35rem;
  }

  .choice {
    padding: 0.45rem 0.7rem;
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
  }

  .choices.small .choice {
    padding: 0.2rem 0.5rem;
    font-size: 0.85em;
  }

  .choice.on,
  .level.on,
  .move button.on {
    background: var(--selected);
    border-color: var(--selected);
    color: var(--on-selected);
  }

  .order {
    font-size: 0.75em;
    font-weight: 700;
    padding: 0 0.3rem;
    border-radius: 999px;
    background: var(--on-selected);
    color: var(--selected);
  }

  .moves {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    max-height: 18rem;
    overflow: auto;
  }

  .move {
    display: flex;
    align-items: center;
    gap: 0.35rem;
  }

  .move span {
    flex: 1;
  }

  .move button {
    font-size: 0.85em;
    padding: 0.1rem 0.45rem;
  }

  .levels {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }

  .level {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 0.1rem;
    padding: 0.4rem 0.6rem;
    text-align: left;
  }

  .level span {
    font-size: 0.85em;
    color: inherit;
    opacity: 0.85;
  }

  .says {
    margin: 0;
    padding-left: 1.1rem;
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    line-height: 1.45;
  }

  .changes {
    margin: 0.2rem 0 0;
    padding-left: 1rem;
    font-size: 0.85em;
    color: var(--text-dim);
  }

  footer {
    display: flex;
    gap: 0.4rem;
    align-items: center;
    margin-top: 0.3rem;
  }

  .spacer {
    flex: 1;
  }

  .primary {
    background: var(--selected);
    border-color: var(--selected);
    color: var(--on-selected);
    font-weight: 600;
  }

  .error {
    color: var(--danger);
    font-size: 0.9em;
  }

  kbd {
    font-family: inherit;
    font-size: 0.85em;
    padding: 0 0.3rem;
    border: 1px solid var(--border);
    border-radius: 3px;
  }
</style>
