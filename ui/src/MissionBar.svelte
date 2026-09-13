<script lang="ts">
  /**
   * §5's Mission Bar.
   *
   * > Always-present but compact. […] It should behave more like an aircraft
   * > HUD than a conventional application toolbar. Do not fill it with menus.
   *
   * # What this component decides
   *
   * Nothing. Every reading, every word and every colour is `dj_app::mission`'s,
   * for the reason that module's own note gives: the bar is a *gathering* of
   * state that already existed in six places, and a gathering assembled here
   * would give each reading its own threshold and its own idea of when
   * something is worth interrupting a mix for.
   *
   * What is left here is the part that is genuinely about pixels: the room
   * reading is a button because §39's other half is *click / expand*, and
   * everything else is text.
   *
   * # Why it polls
   *
   * Most of the bar moves on the scale of minutes — a phase, an occasion, a
   * posture, the room. The two that move fast, the load and the dropout count,
   * are already on the 60 Hz snapshot for the meters that need them there;
   * putting the whole bar on that stream would rebuild ten strings sixty times
   * a second to change one of them twice a night.
   */
  import { missionBar, type MissionItem } from "./api";

  /**
   * What each level draws beside the value.
   *
   * §33: *colour alone must never encode critical state*. Mirrors
   * `dj_app::mission::Level::mark`, and a Rust test reads this table and fails
   * when the two disagree — the mark is a glyph, which is a pixels question, but
   * *which* level gets one is a judgement and belongs where the levels are
   * decided.
   *
   * `aria-hidden` on the mark itself: a screen reader announcing "exclamation
   * exclamation" says less than the `title` beside it already does, in a voice
   * that costs a second to parse. The redundancy §33 asks for is a redundancy
   * for eyes; the sentence is the one for ears.
   */
  const MARK: Record<string, string> = {
    quiet: "",
    watch: "!",
    alarm: "!!",
  };

  interface Props {
    /** Opened when the room reading is pressed. §39's *click / expand*. */
    onOpenRoom: () => void;
  }

  let { onOpenRoom }: Props = $props();

  /**
   * How often the bar asks.
   *
   * Two seconds. Fast enough that a dropout or a failed recording reaches the
   * booth while it still matters, slow enough to be free: the answer is ten
   * short strings and the work behind it is a snapshot the pump has already
   * taken.
   */
  const EVERY_MS = 2000;

  let items = $state<MissionItem[]>([]);

  $effect(() => {
    const ask = () =>
      void missionBar()
        .then((read) => (items = read))
        // A bar that cannot be read shows what it last read rather than
        // emptying: the top bar collapsing to nothing is a worse thing to
        // happen mid-set than one stale figure.
        .catch(() => {});
    ask();
    const timer = setInterval(ask, EVERY_MS);
    return () => clearInterval(timer);
  });
</script>

<!--
  `output` is a status region rather than a live one: a screen reader
  announcing a CPU figure every two seconds would make the application unusable
  for anyone relying on it, and nothing here is an event -- it is a panel to be
  consulted.
-->
<div class="mission mono" role="status" aria-label="Mission bar">
  {#each items as item (item.slug)}
    {#if item.slug === "room"}
      <!--
        The one reading that is also a way in, and the only item here that can
        be pressed: §39's *click / expand*, and the only route to the room
        panel, since nothing can be watching until somebody has opened it.
      -->
      <button
        class="mission-item"
        data-mission={item.slug}
        data-level={item.level}
        title="{item.about} Press to open the room."
        onclick={onOpenRoom}
      >
        {#if item.label}<span class="mission-label">{item.label}</span>{/if}
        {item.value}
        {#if MARK[item.level]}<span class="mission-mark" aria-hidden="true">{MARK[item.level]}</span>{/if}
      </button>
    {:else}
      <span
        class="mission-item"
        data-mission={item.slug}
        data-level={item.level}
        title={item.about}
      >
        {#if item.label}<span class="mission-label">{item.label}</span>{/if}
        {item.value}
        {#if MARK[item.level]}<span class="mission-mark" aria-hidden="true">{MARK[item.level]}</span>{/if}
      </span>
    {/if}
  {/each}
</div>

<style>
  .mission {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 0.55rem;
    font-size: 0.8rem;
    color: var(--text);
  }

  .mission-item {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    white-space: nowrap;
  }

  /*
    The label is the word a DJ learns the position by, so it is quieter than the
    figure that changes.

    Quieter by being dim rather than by being faded. It was `opacity: 0.6` over
    the dim text the whole bar used, which multiplied out to 3.39:1 against the
    panel -- under the 4.5:1 that nine-pixel text needs, and §33 asks for
    *daylight* by name. The hierarchy is now the other way round: the figure is
    full-strength text and the label is the dim token, so the pair still reads
    as a caption and a reading, and both are legible with the sun on the screen.
  */
  .mission-label {
    color: var(--muted);
    letter-spacing: 0.04em;
  }

  /*
    Colour is the whole of the HUD idea, and it is spent sparingly: an item that
    is quiet looks exactly like the text around it, so the two that are not draw
    the eye without anything needing to flash.

    It is also never on its own. §33: *colour alone must never encode critical
    state*, and this strip was the clearest breach of it in the application --
    amber and red and nothing else, on the one surface whose entire job is to
    say that something has gone wrong. Three channels now, which is what the
    section asks for by name: the colour, the mark (iconography), and the
    outline that becomes a fill (border treatment, then texture). Any one of
    them alone tells a DJ which reading to look at; the first to survive a
    hazer, a sunlit window or deuteranopia wins.
  */
  .mission-item[data-level="watch"] {
    color: var(--warn);
    border: 1px solid currentColor;
    border-radius: var(--radius-s);
    padding: 0 0.3rem;
  }

  .mission-item[data-level="alarm"] {
    color: var(--danger);
    font-weight: 600;
    border: 1px solid currentColor;
    border-radius: var(--radius-s);
    padding: 0 0.3rem;
    background: var(--warn-bg);
  }

  /* Set in its own width so a reading gaining a mark does not shove the eight
     readings after it along the strip. */
  .mission-mark {
    font-weight: 700;
    letter-spacing: -0.05em;
  }

  /* The room is pressable; it should not look like a form control while doing
     it. Bordered rather than filled, so it reads as part of the strip. */
  button.mission-item {
    padding: 0.1rem 0.4rem;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: transparent;
    color: inherit;
    font: inherit;
    cursor: pointer;
  }

  button.mission-item:hover {
    border-color: var(--border-strong);
    color: var(--text);
  }
</style>
