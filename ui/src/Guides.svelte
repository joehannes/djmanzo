<script lang="ts">
  /**
   * §118a: the assistant's guides, one per topic.
   *
   * > the integrated ai assistant shall have different guides that are
   * > visually represented via floating widgets with visual presentations of
   * > topics and representations of (multi-choice) options that are easy to
   * > identify
   *
   * One button in the row the eye already reads for the state of the night.
   * It opens the topics as large pictures -- the next record, the mix, the
   * energy of the night, a request waiting, and while a night is on, what to
   * do if something goes wrong -- and each
   * topic opens as a floating widget of a few large choices, told apart by
   * their drawing before their words.
   *
   * Which guides can be opened, and on which decks, is Rust's
   * (`dj_app::guide`); a guide that cannot says why rather than opening onto
   * nothing. What each one offers is the work that already answers it: the
   * rail's three directions (`Decision.svelte`), the planner's styles for
   * the mix (held on a press, never performed by it -- as the pair view
   * holds), and the night's own plans (`Tonight.svelte`).
   */
  import Decision from "./Decision.svelte";
  import Icon from "./controls/Icon.svelte";
  import { portal } from "./controls/portal";
  import {
    audienceSettle,
    audienceWaiting,
    formatTime,
    listGuides,
    nightDeclare,
    nightRead,
    planTransition,
    transitionAdjust,
    transitionArm,
    transitionStyles,
    type Ask,
    type Guide,
    type NightRead,
    type SessionPhase,
    type Transition,
    type TransitionStyleInfo,
  } from "./api";

  let {
    send,
    onTrouble,
    onFind,
    decks,
  }: {
    send: (action: string) => void | Promise<void>;
    /** How many decks are on screen: only those are worked on. */
    decks: number;
    /** Open the night's quick decision. */
    onTrouble: () => void;
    /** Search the collection for what the room asked for, as the Requests panel's *Find it* does. */
    onFind?: (text: string) => void;
  } = $props();

  let picking = $state(false);
  let guides = $state<Guide[]>([]);
  let open = $state<Guide | null>(null);
  let button = $state<HTMLElement | undefined>();
  let place = $state({ top: 0, left: 0 });
  let error = $state<string | null>(null);

  // The request guide's state: the most wanted three, Rust's order.
  let asks = $state<Ask[]>([]);

  async function readAsks() {
    try {
      asks = (await audienceWaiting()).slice(0, 3);
      error = null;
    } catch (e) {
      error = String(e);
    }
  }

  async function settle(ask: Ask, standing: "played" | "passed") {
    try {
      await audienceSettle(ask.id, standing);
      await readAsks();
    } catch (e) {
      error = String(e);
    }
  }

  // The energy guide's state: the night as djmanzo reads it.
  let night = $state<NightRead | null>(null);

  async function readNight() {
    try {
      night = await nightRead();
      error = null;
    } catch (e) {
      error = String(e);
    }
  }

  /**
   * The DJ's word on where the night goes. The phase itself is read again a
   * moment later, because the engine takes the word on its next look at the
   * set; what was said is marked at once.
   */
  async function say(phase: SessionPhase | null) {
    try {
      night = await nightDeclare(phase);
      error = null;
      setTimeout(() => {
        if (open?.topic === "energy") void readNight();
      }, 400);
    } catch (e) {
      error = String(e);
    }
  }

  /** How far into the night, as a DJ says it: "25 min", "1 h 02". */
  function into(seconds: number): string {
    const minutes = Math.floor(seconds / 60);
    return minutes < 60 ? `${minutes} min` : `${Math.floor(minutes / 60)} h ${String(minutes % 60).padStart(2, "0")}`;
  }

  /** The energy guide's choices, each a phase declared, drawn as the night's line. */
  const WORDS: { phase: SessionPhase | null; label: string; does: string; line: string }[] = [
    { phase: "heat", label: "Keep building", does: "The rail ranks up.", line: "M2 20L30 4" },
    { phase: "peak", label: "Hold the peak", does: "The rail holds it there.", line: "M2 18C8 4 24 4 30 4" },
    { phase: "cooldown", label: "Bring it down", does: "The rail ranks down.", line: "M2 4L30 20" },
    { phase: null, label: "Let the music say", does: "Your word taken back.", line: "M2 12C6 4 10 4 14 12S22 20 26 12 30 8 30 8" },
  ];

  // The mix guide's state.
  let plan = $state<Transition | null>(null);
  let styles = $state<TransitionStyleInfo[]>([]);
  let held = $state<string | null>(null);

  function close() {
    picking = false;
    open = null;
    plan = null;
    held = null;
    night = null;
    error = null;
  }

  async function toggle() {
    if (picking || open) {
      close();
      return;
    }
    const r = button?.getBoundingClientRect();
    if (r) place = { top: Math.round(r.bottom + 6), left: Math.round(r.left) };
    try {
      guides = await listGuides(decks);
      error = null;
    } catch (e) {
      guides = [];
      error = String(e);
    }
    picking = true;
  }

  async function choose(guide: Guide) {
    if (guide.not_now) return;
    picking = false;
    if (guide.topic === "trouble") {
      onTrouble();
      return;
    }
    open = guide;
    if (guide.topic === "request") {
      await readAsks();
      return;
    }
    if (guide.topic === "energy") {
      await readNight();
      return;
    }
    if (guide.topic === "mix" && guide.from != null && guide.to != null) {
      try {
        [plan, styles] = await Promise.all([planTransition(guide.from, guide.to), transitionStyles()]);
        error = plan ? null : "djmanzo has no mix to propose between these two records yet.";
      } catch (e) {
        error = String(e);
      }
    }
  }

  /** Hold the mix with this style. Held, never performed: nothing moves. */
  async function hold(style: string) {
    const guide = open;
    if (!guide || guide.from == null || guide.to == null) return;
    try {
      await transitionArm(guide.from, guide.to);
      const now = await transitionAdjust({ style });
      if (now) plan = now;
      held = style;
      error = null;
    } catch (e) {
      error = String(e);
    }
  }

  /** The mix sits under the measured top bar, where the decision card does. */
  let top = $state(96);
  $effect(() => {
    if (open?.topic !== "mix" && open?.topic !== "request" && open?.topic !== "energy") return;
    const bar = document.querySelector(".topbar")?.getBoundingClientRect();
    if (bar) top = Math.round(bar.bottom + 8);
  });

  // Escape and a press elsewhere put the topics away; an open guide stays
  // until it is closed, because a DJ choosing may reach for a fader first.
  $effect(() => {
    if (!picking) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.stopPropagation();
      close();
    };
    const onPress = (event: PointerEvent) => {
      const target = event.target as Element | null;
      if (target?.closest?.(".guides-float, .guides-open")) return;
      picking = false;
    };
    window.addEventListener("keydown", onKey, true);
    window.addEventListener("pointerdown", onPress, true);
    return () => {
      window.removeEventListener("keydown", onKey, true);
      window.removeEventListener("pointerdown", onPress, true);
    };
  });

  /** Each topic's picture: three ways out, two records crossing, a warning. */
  const TOPIC: Record<Guide["topic"], string> = {
    next: "M4 12h6M10 12l9-7M10 12h10M10 12l9 7",
    mix: "M3 6c8 0 10 12 18 12M3 18c8 0 10-12 18-12",
    energy: "M2 19l5-5 4 3 5-8 6 4",
    request: "M4 5h16v10H10l-6 4z",
    trouble: "M12 3l10 18H2zM12 10v5M12 17.5v.5",
  };

  /**
   * Each style's picture: the outgoing record's level (solid) and the
   * incoming one's (dashed) across the mix, as the automix moves them.
   */
  const STYLE: Record<string, { out: string; into: string }> = {
    cut: { out: "M2 4H16V20H30", into: "M2 20H16V4H30" },
    fade: { out: "M2 4L30 20", into: "M2 20L30 4" },
    blend: { out: "M2 4H12C18 4 20 20 30 20", into: "M2 20C12 20 14 4 20 4H30" },
    echo: { out: "M2 4H12L14 20M16 12V20M19 15V20M22 17.5V20", into: "M2 20H14C18 20 18 4 22 4H30" },
    "vocal drop": { out: "M2 4H18L20 20H30", into: "M2 20H8L10 4H30" },
  };
  const FALLBACK = STYLE.blend;
</script>

<button
  type="button"
  class="guides-open"
  bind:this={button}
  aria-expanded={picking || open !== null}
  aria-haspopup="dialog"
  title="The assistant's guides: the next record, the mix, the energy of the night, a request waiting, and tonight's plans"
  onclick={() => void toggle()}
>
  <Icon name="signpost" size="0.95rem" /> Guides
</button>

{#if picking}
  <div class="guides-float topics" use:portal style="top: {place.top}px; left: {place.left}px" role="dialog" aria-label="The assistant's guides">
    {#each guides as guide (guide.topic)}
      <button
        type="button"
        class="topic"
        data-topic={guide.topic}
        disabled={guide.not_now !== null}
        onclick={() => void choose(guide)}
      >
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d={TOPIC[guide.topic]} /></svg>
        <span class="words">
          <strong>{guide.title}</strong>
          <span class="about">{guide.not_now ?? guide.about}</span>
        </span>
      </button>
    {/each}
    {#if error}<p class="error">{error}</p>{/if}
  </div>
{/if}

{#if open?.topic === "next" && open.from != null}
  <Decision
    deck={open.from}
    {decks}
    presence="card"
    says="What follows deck {open.from}?"
    {send}
    later="Close"
    onTaken={close}
    onLater={close}
  />
{/if}

{#if open?.topic === "energy"}
  <div class="guides-float energy" use:portal style="top: {top}px" role="dialog" aria-label="The energy of the night">
    <header>
      <strong>{night?.words ? `Tonight is ${night.words}` : "The energy of the night"}</strong>
      <button type="button" onclick={close}>Close</button>
    </header>
    {#if night}
      {#if night.energy !== null}
        <div class="level" title="How hard the night is going, on tonight's own range">
          <span class="fill" style="width: {Math.round(night.energy * 100)}%"></span>
        </div>
      {/if}
      {#if night.arc.length > 0}
        <ol class="arc" aria-label="The night so far">
          {#each night.arc.slice(-6) as step (step.at_seconds)}
            <li data-phase={step.phase}>{step.words} <span class="when">· {into(step.at_seconds)}</span></li>
          {/each}
        </ol>
      {/if}
      <p class="held" role="status">{night.notes[0]}</p>
      <div class="words-choice">
        {#each WORDS as word (word.label)}
          <button
            type="button"
            class="style"
            data-word={word.phase ?? "music"}
            aria-pressed={night.declared === word.phase}
            onclick={() => void say(word.phase)}
          >
            <svg viewBox="0 0 32 24" aria-hidden="true"><path class="out" d={word.line} /></svg>
            <strong>{word.label}</strong>
            {#if night.declared === word.phase && word.phase !== null}<span class="mark">Your word</span>{/if}
            <span class="does">{word.does}</span>
          </button>
        {/each}
      </div>
    {/if}
    {#if error}<p class="error">{error}</p>{/if}
  </div>
{/if}

{#if open?.topic === "request"}
  <div class="guides-float request" use:portal style="top: {top}px" role="dialog" aria-label="A request waiting">
    <header>
      <strong>The room asked for</strong>
      <button type="button" onclick={close}>Close</button>
    </header>
    {#if asks.length === 0}
      <p class="held" role="status">Nothing else is waiting from the room.</p>
    {:else}
      <ul class="asks">
        {#each asks as ask (ask.id)}
          <li data-ask={ask.id}>
            <span class="asked">
              <strong>{ask.text}</strong>
              <span class="voices">{ask.voices === 1 ? "1 asked" : `${ask.voices} asked`}</span>
            </span>
            <span class="choices">
              <button
                type="button"
                onclick={() => {
                  onFind?.(ask.text);
                  close();
                }}>Find it</button>
              <button type="button" onclick={() => void settle(ask, "played")}>Played</button>
              <button type="button" onclick={() => void settle(ask, "passed")}>Not tonight</button>
            </span>
          </li>
        {/each}
      </ul>
    {/if}
    {#if error}<p class="error">{error}</p>{/if}
  </div>
{/if}

{#if open?.topic === "mix"}
  <div class="guides-float mix" use:portal style="top: {top}px" role="dialog" aria-label="The mix">
    <header>
      <strong>
        {#if plan}
          Into {plan.incoming.track.title} · from {formatTime(plan.start_seconds)} · {plan.length_beats} beats
        {:else}
          The mix
        {/if}
      </strong>
      <button type="button" onclick={close}>Close</button>
    </header>
    {#if plan}
      <div class="styles" style="--n: {styles.length}">
        {#each styles as style (style.name)}
          {@const drawn = STYLE[style.name] ?? FALLBACK}
          <button
            type="button"
            class="style"
            class:planned={plan.style === style.name}
            data-style={style.name}
            aria-pressed={held === style.name}
            onclick={() => void hold(style.name)}
          >
            <svg viewBox="0 0 32 24" aria-hidden="true">
              <path class="out" d={drawn.out} />
              <path class="into" d={drawn.into} />
            </svg>
            <strong>{style.name}</strong>
            {#if plan.style === style.name && held === null}<span class="mark">djmanzo's plan</span>{/if}
            <span class="does">{style.shape.does.join("; ")}</span>
          </button>
        {/each}
      </div>
      <p class="held" role="status">
        {#if held}
          Held: {held}, {plan.length_beats} beats from {formatTime(plan.start_seconds)}. The automix performs it
          when it is on, and the pair view shows it; or mix it yourself.
        {:else}
          Solid is the record playing, dashed the one coming in. Choosing holds the mix; nothing moves.
        {/if}
      </p>
    {/if}
    {#if error}<p class="error">{error}</p>{/if}
  </div>
{/if}

<style>
  .request {
    right: 1rem;
    width: min(30rem, calc(100vw - 2rem));
  }

  .request header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 0.5rem;
    margin-bottom: 0.4rem;
  }

  .asks {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .asks li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 0.4rem 0.8rem;
    min-height: 3.2rem;
    padding: 0.45rem 0.6rem;
    border: 1px solid var(--border);
    border-radius: 0.45rem;
  }

  .asked {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .voices {
    color: var(--text-dim);
    font-size: 0.8rem;
  }

  .choices {
    display: inline-flex;
    gap: 0.3rem;
  }

  .guides-open {
    flex: none;
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
  }

  .guides-float {
    position: fixed;
    z-index: 63;
    padding: 0.6rem;
    border: 1px solid var(--active);
    border-radius: 0.6rem;
    background: var(--panel);
    box-shadow: 0 18px 48px rgba(0, 0, 0, 0.55);
  }

  .topics {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    width: min(26rem, calc(100vw - 2rem));
  }

  .topic {
    display: flex;
    align-items: center;
    gap: 0.7rem;
    min-height: 3.6rem;
    padding: 0.5rem 0.7rem;
    text-align: left;
  }

  .topic svg,
  .style svg {
    flex: none;
    fill: none;
    stroke: currentColor;
    stroke-width: 2.2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .topic svg {
    width: 2rem;
    height: 2rem;
    color: var(--active);
  }

  .topic[data-topic="trouble"] svg {
    color: var(--warn);
  }

  .topic:disabled svg {
    color: var(--text-dim);
  }

  .words {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
    min-width: 0;
  }

  .about {
    color: var(--text-dim);
    font-size: 0.85em;
  }

  /* The mix: under the top bar in the middle, where the card goes. */
  .mix {
    left: 50%;
    transform: translateX(-50%);
    width: min(56rem, calc(100vw - 2rem));
    display: flex;
    flex-direction: column;
    gap: 0.55rem;
  }

  .energy {
    left: 50%;
    transform: translateX(-50%);
    width: min(44rem, calc(100vw - 2rem));
    display: flex;
    flex-direction: column;
    gap: 0.55rem;
  }

  .energy header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 0.6rem;
  }

  .level {
    height: 0.5rem;
    border-radius: 0.25rem;
    background: var(--panel-raised);
    overflow: hidden;
  }

  .level .fill {
    display: block;
    height: 100%;
    background: var(--active);
  }

  .arc {
    display: flex;
    flex-wrap: wrap;
    gap: 0.3rem;
    margin: 0;
    padding: 0;
    list-style: none;
    font-size: 0.8em;
  }

  .arc li {
    padding: 0.1rem 0.45rem;
    border: 1px solid var(--border);
    border-radius: 999px;
  }

  .arc li:last-child {
    border-color: var(--active);
  }

  .when {
    color: var(--text-dim);
  }

  .words-choice {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 0.45rem;
  }

  .mix header {
    display: flex;
    align-items: center;
    gap: 0.6rem;
  }

  .mix header strong {
    flex: 1 1 auto;
    min-width: 0;
  }

  .styles {
    display: grid;
    grid-template-columns: repeat(var(--n, 5), minmax(0, 1fr));
    gap: 0.45rem;
  }

  .style {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 0.2rem;
    min-width: 0;
    padding: 0.5rem;
    text-align: left;
    border: 2px solid var(--border);
    border-radius: 0.55rem;
  }

  .style.planned {
    border-color: var(--active);
  }

  .style[aria-pressed="true"] {
    border-color: var(--accent-warm);
    background: var(--panel-raised);
  }

  .style svg {
    width: 100%;
    height: 3rem;
  }

  .style .out {
    color: var(--outgoing, var(--accent));
    stroke: var(--outgoing, var(--accent));
  }

  .style .into {
    stroke: var(--incoming, var(--accent-2));
    stroke-dasharray: 3 2.5;
  }

  .mark {
    color: var(--active);
    font-size: 0.75em;
    font-weight: 700;
    text-transform: uppercase;
  }

  .does,
  .held {
    color: var(--text-dim);
    font-size: 0.8em;
  }

  .held {
    margin: 0;
  }

  .error {
    margin: 0;
    color: var(--warn);
  }

  @media (max-width: 40rem) {
    .words-choice,
    .styles {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }
</style>
