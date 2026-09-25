<script lang="ts">
  import { STEM_COLORS, STEM_KEYS, STEM_LABELS } from "./stems";
  import IconButton from "./controls/IconButton.svelte";
  import SvgFader from "./controls/SvgFader.svelte";
  import SvgKnob from "./controls/SvgKnob.svelte";
  import { portal } from "./controls/portal";
  import { dispatch, stemsStatus, type StemsStatus, type StemSwap } from "./api";
  import { wantsStemsOpen } from "./hands.svelte";
  import { onDestroy, onMount } from "svelte";

  let {
    deckNumber,
    muteState = [false, false, false, false],
    volumeState = [1.0, 1.0, 1.0, 1.0],
    eqState = [
      [1, 1, 1],
      [1, 1, 1],
      [1, 1, 1],
      [1, 1, 1],
    ],
    filterState = [0, 0, 0, 0],
    soloing = false,
    swap = null,
    deckCount = 2,
    startOpen = false,
  }: {
    deckNumber: number;
    muteState?: boolean[];
    volumeState?: number[];
    /**
     * Per-stem EQ trim, low/mid/high, from the engine.
     *
     * The DJ's own setting rather than the effective coefficient: the deck's
     * EQ multiplies into it, and a knob showing the product would jump every
     * time the channel strip moved.
     */
    eqState?: number[][];
    /** Per-stem filter sweep, -1 low-pass .. 0 open .. +1 high-pass. */
    filterState?: number[];
    /** The stem swap in force anywhere, from the engine. */
    swap?: StemSwap | null;
    deckCount?: number;
    /**
     * Whether a stem solo is held on this deck.
     *
     * From the engine, not from a local flag, because a controller can hold a
     * solo too and a button that disagreed with the audio would be worse than
     * no button.
     */
    soloing?: boolean;
    /**
     * Whether the layout asked for this module open.
     *
     * §5B's stem performance composition: "stem controls become first-class".
     * It is the layout's opening position, not a lock -- a DJ who folds it
     * away keeps it folded, the same as when a stem in use opened it.
     */
    startOpen?: boolean;
  } = $props();

  /**
   * Whether separation can run here.
   *
   * Asked on mount, and again every few seconds while a model is still
   * loading: the built-in separator plays from the first second and the
   * model takes over once it has been loaded and tried, which takes seconds
   * on a quarter-gigabyte model. After that it cannot change while the
   * application is running: the model is looked for at startup, so a DJ who
   * installs one mid-set has to restart -- and being told that is better
   * than pads that quietly do nothing.
   */
  const ASK_AGAIN_MS = 2000;
  let status = $state<StemsStatus>({ available: true, backend: null, reason: null });
  onMount(() => {
    let again: ReturnType<typeof setTimeout> | undefined;
    let gone = false;
    const ask = async () => {
      try {
        status = await stemsStatus();
      } catch (error) {
        status = {
          available: false,
          backend: null,
          reason: `could not ask about stems: ${error}`,
        };
      }
      if (status.loading && !gone) again = setTimeout(ask, ASK_AGAIN_MS);
    };
    void ask();
    return () => {
      gone = true;
      clearTimeout(again);
    };
  });


  /**
   * §120: *"what if i want to quickly mute/unmute some stem??"*
   *
   * The four stems are a row of chips that is always there, under the
   * waveform: one press mutes a stem or brings it back, and the chip says
   * which from across the booth — lit in its colour when it plays, dark and
   * struck through when it does not, with its level as the fill. Shift and a
   * press hears it alone; a press while a solo is held lets the solo go,
   * because the engine refuses mutes during one and a chip that did nothing
   * would be the complaint again.
   */
  function toggleMute(index: number) {
    if (soloing) {
      releaseSolo();
      return;
    }
    dispatch(`deck ${deckNumber} stem_mute ${STEM_KEYS[index]}`);
  }

  /** Which stem the chips soloed, to let the same one go. */
  let soloed = $state<number | null>(null);

  function soloStem(index: number) {
    stopFade();
    if (soloing) {
      releaseSolo();
      return;
    }
    soloed = index;
    dispatch(`deck ${deckNumber} stem_solo_on ${STEM_KEYS[index]}`);
  }

  function releaseSolo() {
    dispatch(`deck ${deckNumber} stem_solo_off ${STEM_KEYS[soloed ?? 0]}`);
    soloed = null;
  }

  function pressChip(event: MouseEvent, index: number) {
    // The second press of a double-click is the reset's, not a second toggle.
    if (event.detail > 1) return;
    if (event.shiftKey) soloStem(index);
    else toggleMute(index);
  }

  /**
   * §121: *"scroll horizontally over the stem buttons and smoothly reduce the
   * effectfulness of the switch/toggle ... double click resets the button to
   * 100% and on"*.
   *
   * Across the chip is the stem's level, in the direction its fill is drawn:
   * towards the right is more. A sideways scroll, or Shift and the wheel;
   * an up-and-down scroll is left to scroll the deck, which is what it was
   * doing before. A wheel notch is a twentieth, a trackpad as fine as it is.
   */
  const NOTCH_PX = 100;
  const NOTCH = 0.05;
  /** What the wheel last sent, and when: the snapshot is a frame or two behind it. */
  const sent: (number | null)[] = [null, null, null, null];
  const sentAt = [0, 0, 0, 0];

  function levelNow(index: number): number {
    const recent = performance.now() - sentAt[index] < 400;
    return recent && sent[index] !== null ? (sent[index] as number) : (volumeState[index] ?? 1);
  }

  function wheelChip(event: WheelEvent, index: number) {
    const across =
      // Shift and the wheel as the browsers themselves read it, a scroll to
      // the right for a turn towards you -- Chromium and GTK both turn it
      // into one, and a webview that does not must not go the other way.
      Math.abs(event.deltaX) > Math.abs(event.deltaY) ? event.deltaX : event.shiftKey ? event.deltaY : 0;
    if (across === 0 || !status.available) return;
    event.preventDefault();
    const px = event.deltaMode === 1 ? across * 16 : event.deltaMode === 2 ? across * 400 : across;
    const level = Math.min(1, Math.max(0, levelNow(index) + (px / NOTCH_PX) * NOTCH));
    sent[index] = level;
    sentAt[index] = performance.now();
    stopFade();
    dispatch(`deck ${deckNumber} stem_volume ${STEM_KEYS[index]}:${level.toFixed(3)}`);
  }

  /**
   * Full and on. The first press of the double-click has already toggled the
   * stem, so for the moment between the two it was off; this puts it on
   * outright rather than toggling it back, which would be wrong half the time.
   */
  function resetChip(index: number) {
    if (!status.available) return;
    sent[index] = 1;
    sentAt[index] = performance.now();
    dispatch(`deck ${deckNumber} stem_volume ${STEM_KEYS[index]}:1.000`);
    if (!soloing) dispatch(`deck ${deckNumber} stem_mute_off ${STEM_KEYS[index]}`);
  }

  /**
   * The tone controls — level, EQ and filter per stem, the swap, the vocal
   * macros — are a panel of their own.
   *
   * Opened by the DJ, it floats over the deck and goes away again (Escape, a
   * press elsewhere, or the same button): §120's *temporary windows can take
   * a lot of space for the focused moment and then get out of the way*. A
   * layout or a controller that asks for it (below) gets it in place instead,
   * because there it is the point rather than a visit.
   */
  let asked = $state(false);

  /**
   * Where the floating panel goes: under the strip, in window coordinates.
   *
   * Fixed rather than absolute, because the deck's upper zone scrolls, and a
   * panel positioned inside a scrolling box is cut off at its edge — the
   * first version was, and a press on a knob landed on whatever was under the
   * cut and put the panel away.
   */
  let stripEl = $state<HTMLElement | null>(null);
  let place = $state("");
  const PANEL_MIN_WIDTH = 440;
  function measure() {
    if (!stripEl) return;
    const rect = stripEl.getBoundingClientRect();
    const width = Math.min(window.innerWidth - 16, Math.max(rect.width, PANEL_MIN_WIDTH));
    const left = Math.max(8, Math.min(rect.left, window.innerWidth - width - 8));
    const top = rect.bottom + 4;
    place = `top: ${top}px; left: ${left}px; width: ${width}px; max-height: ${Math.max(160, window.innerHeight - top - 8)}px;`;
  }
  /** The floating panel, which lives in the document's body while open. */
  let floatEl = $state<HTMLElement | null>(null);
  $effect(() => {
    if (!asked || inline) return;
    measure();
    // A press anywhere but the strip and the panel puts it away. Captured,
    // so a control that stops propagation still counts as elsewhere.
    const away = (event: PointerEvent) => {
      const target = event.target;
      if (!(target instanceof Node)) return;
      if (stripEl?.contains(target) || floatEl?.contains(target)) return;
      asked = false;
    };
    window.addEventListener("resize", measure);
    window.addEventListener("scroll", measure, true);
    document.addEventListener("pointerdown", away, true);
    return () => {
      window.removeEventListener("resize", measure);
      window.removeEventListener("scroll", measure, true);
      document.removeEventListener("pointerdown", away, true);
    };
  });

  function onKey(event: KeyboardEvent) {
    if (event.key === "Escape" && asked && !inline) {
      asked = false;
      event.stopPropagation();
    }
  }

  /** low, mid, high — matching `EqBand::ALL` on the engine side. */
  const BANDS = ["low", "mid", "high"];

  function changeEq(index: number, band: number, value: string) {
    const gain = parseFloat(value);
    dispatch(
      `deck ${deckNumber} stem_eq_${BANDS[band]} ${STEM_KEYS[index]}:${gain.toFixed(3)}`,
    );
  }

  function changeStemFilter(index: number, value: string) {
    const position = parseFloat(value);
    dispatch(
      `deck ${deckNumber} stem_filter ${STEM_KEYS[index]}:${position.toFixed(3)}`,
    );
  }

  /**
   * Put one stem's tone back to flat.
   *
   * Four knobs to return by hand is four chances to leave one slightly off,
   * and "slightly off" on a stem EQ is the kind of thing a DJ hears twenty
   * minutes later and cannot find.
   */
  function resetTone(index: number) {
    for (let band = 0; band < BANDS.length; band += 1) {
      dispatch(`deck ${deckNumber} stem_eq_${BANDS[band]} ${STEM_KEYS[index]}:1`);
    }
    dispatch(`deck ${deckNumber} stem_filter ${STEM_KEYS[index]}:0`);
  }

  /** True when this stem's tone is anywhere but flat, so the reset can say so. */
  function toneTouched(index: number): boolean {
    const eq = eqState?.[index] ?? [1, 1, 1];
    return (
      eq.some((gain) => Math.abs(gain - 1) > 0.001) ||
      Math.abs(filterState?.[index] ?? 0) > 0.001
    );
  }

  /**
   * Whether anything here is actually doing something to the sound.
   *
   * The rule the FX rack already follows: a block that is not in use folds
   * away and opens itself the moment it matters. Four stems at unity, unmuted
   * and unswapped are four defaults, and they were costing about 370 px above
   * the waveform on every loaded deck -- more than three times what the
   * waveform itself got.
   *
   * A swap counts only when it lands on this deck. `swap` is the one swap in
   * force anywhere, so testing it for null alone would fling every deck's
   * stems open because somebody borrowed a vocal on deck 3.
   */
  /**
   * §53's worked example: *if there are no stem controls, expand stem
   * controls.*
   *
   * The one case where the hardware decides how much room a surface gets. A DJ
   * whose controller has stem pads reaches for those and the module is a
   * readout; a DJ whose controller cannot reach the stems has only the screen,
   * and a folded module is the feature hidden from the only person who needs it
   * open.
   *
   * Nothing plugged in leaves it folded, deliberately: see `wantsStemsOpen`.
   */
  const noStemControls = $derived(wantsStemsOpen());
  /** In place, because the layout or the hardware asked for it. */
  const inline = $derived(noStemControls || startOpen);
  const open = $derived(inline || asked);


  function changeVolume(index: number, value: string) {
    const vol = parseFloat(value);
    dispatch(`deck ${deckNumber} stem_volume ${STEM_KEYS[index]}:${vol.toFixed(3)}`);
  }

  /**
   * Acapella: hold the vocal alone, and let go again.
   *
   * **This latches.** The engine treats a solo as a held audition — it
   * snapshots the DJ's mutes on the way in and restores them on release, and
   * refuses every mute while one is held. Nothing in this panel ever sent the
   * release, so one click left the deck's whole stem section dead for the rest
   * of the set. A mouse cannot hold a button, so the second click is the
   * release.
   */
  function macroAcapella() {
    soloStem(0);
  }

  /**
   * Instrumental: the vocal muted, whatever it was doing before.
   *
   * `stem_mute` is a toggle — right for a controller pad, wrong for a macro
   * that names an outcome, because it un-mutes a vocal that was already muted
   * and does the opposite of what the button says. `stem_mute_on` states it.
   */
  function macroInstrumental() {
    stopFade();
    // A held solo would refuse the mute outright, so it is released first.
    if (soloing) dispatch(`deck ${deckNumber} stem_solo_off vocal`);
    dispatch(`deck ${deckNumber} stem_mute_on vocal`);
  }

  /**
   * Which stem this deck is sending elsewhere, if any.
   *
   * There is one swap in the whole engine, so a panel shows it only when this
   * deck is the source — otherwise every deck would offer to cancel a swap it
   * has nothing to do with.
   */
  const sending = $derived(swap && swap.from === deckNumber ? swap : null);
  /** The deck a swap would go to. Anything but this one. */
  let target = $state<number | null>(null);
  $effect(() => {
    if (target === null || target === deckNumber) {
      target = deckNumber === 1 ? 2 : 1;
    }
  });

  function swapStem(index: number) {
    if (sending) {
      dispatch("stem_swap_off");
      return;
    }
    if (target === null) return;
    dispatch(`stem_swap ${STEM_KEYS[index]} ${deckNumber} ${target}`);
  }

  /** The running vocal fade, so a second click stops it rather than racing it. */
  let fade = $state<ReturnType<typeof setInterval> | null>(null);

  function stopFade() {
    if (fade !== null) {
      clearInterval(fade);
      fade = null;
    }
  }

  // Two fades writing the same parameter fight, and one left running after the
  // panel is gone keeps dispatching at a deck nobody is looking at.
  onDestroy(stopFade);

  const FADE_MS = 100;
  const FADE_STEP = 0.05;

  function macroVocalFadeOut() {
    if (fade !== null) {
      stopFade();
      return;
    }
    let vol = volumeState?.[0] ?? 1.0;
    fade = setInterval(() => {
      vol -= FADE_STEP;
      // `<= 0` alone leaves 2.8e-17 after twenty steps from 1.0, so the fade
      // runs one tick past the end before clamping. Round to the step.
      if (vol < FADE_STEP / 2) {
        vol = 0;
        stopFade();
      }
      dispatch(`deck ${deckNumber} stem_volume vocal:${vol.toFixed(3)}`);
    }, FADE_MS);
  }
</script>

<svelte:window onkeydown={onKey} />

<!--
  §120: the stems as a row that is always there, and the rest a press away.
  It used to be a fold, closed on every deck unless a layout opened it, so
  muting a vocal took two presses and a scroll.
-->
<div class="stems" class:inline data-stems-open={open}>
  <div class="stem-strip" role="group" aria-label="Stems on deck {deckNumber}" bind:this={stripEl}>
    {#each STEM_LABELS as name, i (name)}
      <button
        class="stem-chip"
        class:muted={muteState[i]}
        class:alone={soloing && soloed === i}
        style="--stem-color: {STEM_COLORS[i]}; --level: {muteState[i] ? 0 : (volumeState[i] ?? 1)}"
        data-stem={STEM_KEYS[i]}
        aria-pressed={!muteState[i]}
        disabled={!status.available}
        title={soloing
          ? `A solo is held — press to let it go`
          : `${name} at ${Math.round((volumeState[i] ?? 1) * 100)}%: press to ${muteState[i] ? "bring it back" : "mute it"}, Shift+press to hear it alone, scroll across to fade, double-click for full`}
        onclick={(event) => pressChip(event, i)}
        ondblclick={() => resetChip(i)}
        onwheel={(event) => wheelChip(event, i)}
      >
        <span class="chip-fill" aria-hidden="true"></span>
        <span class="chip-name">{name}</span>
      </button>
    {/each}
    {#if !status.available}
      <span class="off" title={status.reason ?? ""}>unavailable</span>
    {/if}
    {#if !inline}
      <button
        class="stem-more"
        aria-expanded={open}
        title={open ? "Put the stem tone away (Esc)" : "Level, EQ and filter per stem, the swap and the vocal moves"}
        onclick={() => (asked = !asked)}
      >
        {open ? "Close" : "Tone"}
      </button>
    {/if}
  </div>

  {#if open && inline}
    {@render module(false)}
  {/if}
</div>

{#if open && !inline}
  <div class="stems-float" use:portal bind:this={floatEl}>
    {@render module(true)}
  </div>
{/if}

{#snippet module(floating: boolean)}
    <div
      class="stems-module"
      class:floating
      class:unavailable={!status.available}
      style={floating ? place : undefined}
    >
      {#if !status.available}
        <p class="stems-reason" role="status">
          Stem separation is unavailable{status.reason ? ` — ${status.reason}` : ""}
        </p>
      {:else if status.reason}
        <!--
          Separating, but with the fallback. Worth saying: the controls work, and
          a downloaded model would work better. Not an error, so it does not read
          as one.
        -->
        <p class="stems-reason" role="status">
          Using the {status.backend ?? "built-in"} separator — {status.reason}
        </p>
      {/if}
      <div class="stems-grid">
        {#each STEM_LABELS as name, i (name)}
          <div class="stem-column" style="--stem-color: {STEM_COLORS[i]}">
            <span class="column-name">{name}</span>
            <!--
              Knobs and a fader, the deck's own controls, rather than native
              sliders: a drag of a hundred pixels is the whole range however
              narrow the column, Shift is fine, and a double press puts it
              back. The sliders these replace were a few pixels of travel per
              step in a narrow column.
            -->
            <SvgFader
              value={volumeState[i] ?? 1}
              min={0}
              max={1}
              step={0.01}
              label="Level"
              name="{name} level"
              readout={(volumeState[i] ?? 1).toFixed(2)}
              disabled={!status.available}
              height={70}
              width={26}
              oninput={(value) => changeVolume(i, String(value))}
              ondblclick={() => changeVolume(i, "1")}
            />
            <div class="stem-tone">
              {#each ["Lo", "Mid", "Hi"] as band, b (band)}
                <SvgKnob
                  value={eqState?.[i]?.[b] ?? 1}
                  min={0}
                  max={4}
                  step={0.01}
                  label={band}
                  name="{name} {['low', 'mid', 'high'][b]}"
                  face={(["eq-low", "eq-mid", "eq-high"] as const)[b]}
                  origin={1}
                  size={30}
                  disabled={!status.available}
                  oninput={(value) => changeEq(i, b, String(value))}
                  ondblclick={() => changeEq(i, b, "1")}
                />
              {/each}
              <SvgKnob
                value={filterState?.[i] ?? 0}
                min={-1}
                max={1}
                step={0.01}
                label="Flt"
                name="{name} filter"
                face="filter"
                origin={0}
                size={30}
                disabled={!status.available}
                oninput={(value) => changeStemFilter(i, String(value))}
                ondblclick={() => changeStemFilter(i, "0")}
              />
            </div>
            <button
              class="tone-reset"
              class:active={toneTouched(i)}
              disabled={!status.available || !toneTouched(i)}
              title="Put {name} back to flat"
              onclick={() => resetTone(i)}
            >
              flat
            </button>
          </div>
        {/each}
      </div>

      {#if status.available}
        <div class="swap-row">
          {#if sending}
            <span class="swap-note">
              {STEM_LABELS[sending.stem]} over deck {sending.to}
            </span>
            <IconButton
              icon="unlink"
              title="Put both decks back"
              active={true}
              onClick={() => swapStem(sending.stem)}
            />
          {:else}
            <span class="swap-note">Send a stem to deck</span>
            <select bind:value={target} aria-label="Which deck to send a stem to">
              {#each Array.from({ length: deckCount }, (_, i) => i + 1) as n (n)}
                {#if n !== deckNumber}
                  <option value={n}>{n}</option>
                {/if}
              {/each}
            </select>
            {#each STEM_LABELS as name, i (name)}
              <button
                class="swap-pick"
                style="--stem-color: {STEM_COLORS[i]}"
                title="Play this deck's {name.toLowerCase()} over deck {target}"
                onclick={() => swapStem(i)}
              >
                {name.slice(0, 2)}
              </button>
            {/each}
          {/if}
        </div>
      {/if}

      <div class="macros-row">
        <IconButton
          icon="fa-solid fa-microphone"
          title={soloing ? "Release the solo" : "Solo Vocals (Acapella)"}
          active={soloing}
          disabled={!status.available}
          onClick={macroAcapella}
        />
        <IconButton
          icon="fa-solid fa-guitar"
          title="Mute Vocals (Instrumental)"
          disabled={!status.available}
          onClick={macroInstrumental}
        />
        <IconButton
          icon="fa-solid fa-hand"
          title={fade ? "Stop the fade" : "Gradually fade out vocals"}
          active={fade !== null}
          disabled={!status.available}
          onClick={macroVocalFadeOut}
        />
      </div>
    </div>
{/snippet}

<style>
  .stems {
    position: relative;
  }

  /* The row that is always there. Chips share the width, so four stems read
     as four equal things on any deck, and nothing wraps. */
  .stem-strip {
    display: flex;
    align-items: stretch;
    gap: 0.3rem;
  }

  .stem-chip {
    position: relative;
    flex: 1 1 0;
    min-width: 0;
    /* No taller than the fold's header it replaced, so a deck that fitted
       its window still does (e2e/density.spec.ts measures it). */
    height: 1.45rem;
    padding: 0 0.3rem;
    overflow: hidden;
    border: 1px solid var(--stem-color);
    border-radius: 0.35rem;
    background: var(--panel);
    color: var(--text);
    font: inherit;
    cursor: pointer;
  }

  /* The level, as the chip's own fill: a stem turned down reads as less lit,
     a muted one as empty. */
  .chip-fill {
    position: absolute;
    inset: 0 auto 0 0;
    width: calc(var(--level) * 100%);
    background: color-mix(in srgb, var(--stem-color) 34%, transparent);
    pointer-events: none;
  }

  .chip-name {
    position: relative;
    display: block;
    overflow: hidden;
    font-size: 0.7rem;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-overflow: ellipsis;
    text-transform: uppercase;
    white-space: nowrap;
  }

  .stem-chip:hover:not(:disabled) {
    background: var(--panel-hover);
  }

  .stem-chip.muted {
    border-color: var(--border);
    color: var(--text-dim);
  }

  .stem-chip.muted .chip-name {
    text-decoration: line-through;
  }

  .stem-chip.alone {
    box-shadow: 0 0 0 2px var(--stem-color);
  }

  .stem-chip:disabled {
    cursor: default;
    opacity: 0.45;
  }

  .stem-more {
    flex: none;
    padding: 0 0.55rem;
    border: 1px solid var(--border);
    border-radius: 0.35rem;
    background: var(--panel);
    color: var(--text-dim);
    font: inherit;
    font-size: 0.7rem;
    cursor: pointer;
  }

  .stem-more[aria-expanded="true"] {
    border-color: var(--selected);
    color: var(--text);
  }

  .off {
    align-self: center;
    color: var(--text-dim);
    font-size: 0.7rem;
  }

  .stems-module {
    margin-top: 0.35rem;
    padding: 0.5rem;
    border: 1px solid var(--border);
    border-radius: 0.45rem;
    background: var(--panel);
  }

  /* Opened by the DJ: over the deck, not pushing it down, and gone again. */
  .stems-module.floating {
    position: fixed;
    z-index: 60;
    margin-top: 0;
    overflow: auto;
    background:
      linear-gradient(var(--panel-raised), var(--panel-raised)),
      var(--bg);
    box-shadow: 0 0.8rem 2rem rgb(0 0 0 / 0.45);
  }

  .stems-reason {
    margin: 0 0 0.4rem;
    color: var(--text-dim);
    font-size: 0.72rem;
  }

  .stems-module.unavailable .stems-grid {
    opacity: 0.45;
  }

  .stems-grid {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 0.4rem;
  }

  .stem-column {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.25rem;
    padding: 0.3rem 0.2rem;
    border-top: 2px solid var(--stem-color);
    border-radius: 0.3rem;
    background: color-mix(in srgb, var(--stem-color) 7%, transparent);
  }

  .column-name {
    font-size: 0.66rem;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  /* Four small knobs in two pairs: EQ low and mid, high and filter. */
  .stem-tone {
    display: grid;
    grid-template-columns: repeat(2, auto);
    gap: 0.15rem 0.3rem;
    justify-items: center;
  }

  .tone-reset {
    padding: 0.05rem 0.4rem;
    border: 1px solid var(--border);
    border-radius: 0.3rem;
    background: transparent;
    color: var(--text-dim);
    font: inherit;
    font-size: 0.65rem;
  }

  .tone-reset.active {
    border-color: var(--selected);
    color: var(--text);
    cursor: pointer;
  }

  .swap-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.3rem;
    margin-top: 0.5rem;
    font-size: 0.68rem;
    color: var(--text-dim);
  }

  .swap-note {
    white-space: nowrap;
  }

  .swap-pick {
    border: 1px solid var(--stem-color);
    background: var(--panel);
    color: var(--text);
    border-radius: 4px;
    padding: 0.1rem 0.3rem;
    font-size: 0.65rem;
    cursor: pointer;
  }

  .swap-pick:hover {
    background: var(--panel-hover);
  }

  .macros-row {
    display: flex;
    justify-content: space-between;
    margin-top: 0.5rem;
    gap: 8px;
  }
</style>
