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
    color: var(--text-dim);
  }

  .mission-item {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    white-space: nowrap;
  }

  /* The label is the word a DJ learns the position by, so it is quieter than
     the figure that changes. */
  .mission-label {
    opacity: 0.6;
    letter-spacing: 0.04em;
  }

  /*
    Colour is the whole of the HUD idea, and it is spent sparingly: an item that
    is quiet looks exactly like the text around it, so the two that are not draw
    the eye without anything needing to flash.
  */
  .mission-item[data-level="watch"] {
    color: var(--warn);
  }

  .mission-item[data-level="alarm"] {
    color: var(--danger);
    font-weight: 600;
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
