<script lang="ts">
  import {
    dispatch,
    formatTime,
    loadTrack,
    padPages,
    type DeckState,
    type Layout,
    type PadPageDto,
    type SamplerState,
    type StemSwap,
  } from "./api";
  import JogWheel from "./JogWheel.svelte";
  import { fill } from "./meter";
  import Fx from "./Fx.svelte";
  import Stems from "./Stems.svelte";
  import Pads from "./Pads.svelte";
  import Overview from "./Overview.svelte";
  import Waveform from "./Waveform.svelte";
  import SvgKnob from "./controls/SvgKnob.svelte";
  import SvgFader from "./controls/SvgFader.svelte";
  import SvgPad from "./controls/SvgPad.svelte";
  import IconButton from "./controls/IconButton.svelte";
  import { open } from "@tauri-apps/plugin-dialog";

  let {
    deck,
    /** The sampler, for the pad page whose pads are not about this deck. */
    sampler,
    enabled,
    cueAvailable = false,
    layout = null,
    stemSwap = null,
    deckCount = 2,
  }: {
    deck: DeckState;
    sampler: SamplerState;
    enabled: boolean;
    cueAvailable?: boolean;
    /** The one stem swap in force, from the master snapshot. */
    stemSwap?: StemSwap | null;
    /** How many decks there are, so a swap can name a real one. */
    deckCount?: number;
    /**
     * The layout in force, or null before one has been chosen.
     *
     * Null is not "hide everything" — it is "the application has not been told
     * otherwise", so every flag falls back to shown. A DJ who never opens the
     * layout picker gets the full deck, which is the interface that existed
     * before layouts did.
     */
    layout?: Layout | null;
  } = $props();

  // Read once here rather than at each use, so the fallback lives in one place.
  const showPads = $derived(layout?.pads ?? true);
  const showLoops = $derived(layout?.loops ?? true);
  const showJump = $derived(layout?.beat_jump ?? true);
  const showEq = $derived(layout?.eq ?? true);
  const showFilter = $derived(layout?.filter ?? true);
  const showKeylock = $derived(layout?.keylock ?? true);
  // Shown with the loops: slip is what makes a loop something you can leave.
  const showSlip = $derived(layout?.loops ?? true);
  const showFx = $derived(layout?.fx ?? true);
  const showOverview = $derived(layout?.overview ?? true);
  const waveHeight = $derived(layout?.waveform_height ?? 96);

  let error = $state<string | null>(null);
  let loading = $state(false);

  // Name and artist come from the snapshot, not from this component.
  //
  // They used to be set here, by this deck's own Load button — which meant a
  // track arriving from the browser, the assistant, a preset or a controller
  // played perfectly while the header still read "no track". The engine has no
  // metadata, so the application remembers it per deck and sends it down with
  // everything else.
  const title = $derived(deck.title ?? "");
  const artist = $derived(deck.artist ?? "");

  const progress = $derived(
    deck.length_frames > 0 ? deck.position_frames / deck.length_frames : 0,
  );

  const analysis = $derived(deck.analysis);

  /** "4" for whole loops, "1/4" for halved ones, which is how DJs say them. */
  function formatBeats(beats: number): string {
    if (beats >= 1) return String(Math.round(beats * 100) / 100);
    return `1/${Math.round(1 / beats)}`;
  }

  /**
   * The pad pages, fetched once.
   *
   * Once because they do not change: a page is a fixed table in Rust, and the
   * only thing that varies is which deck number the actions are addressed to.
   * Everything that *does* change — what is lit, what is held — comes from the
   * snapshot the component already has.
   */
  let padPageList = $state<PadPageDto[]>([]);
  $effect(() => {
    void padPages(deck.number).then((pages) => {
      padPageList = pages;
    });
  });

  /**
   * Take the analyser's rejected octave.
   *
   * Autocorrelation genuinely cannot tell 80 from 160 — a curve periodic at one
   * is periodic at the other — so the octave is a *guess*, and the analyser
   * reports the runner-up precisely so a wrong guess costs one click instead of
   * a retapped grid. This only changes what is displayed; the stored analysis
   * is untouched until M2's grid editing lands.
   */
  let octaveSwapped = $state(false);
  function swapOctave() {
    if (analysis?.bpm_alternative != null) octaveSwapped = !octaveSwapped;
  }
  // Reset when the deck gets a different track, or the swap would carry over.
  $effect(() => {
    void deck.analysis;
    octaveSwapped = false;
  });

  async function pickTrack() {
    error = null;
    const path = await open({
      multiple: false,
      filters: [
        {
          name: "Audio",
          extensions: ["mp3", "flac", "wav", "aiff", "aif", "ogg", "m4a", "aac", "opus"],
        },
      ],
    });
    if (typeof path !== "string") return;

    loading = true;
    try {
      await loadTrack(deck.number, path);
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  /**
   * The three positions of the assignment switch.
   *
   * `THRU` in the middle because that is where it sits on a hardware mixer, and
   * because the middle position is the one that means "the crossfader does not
   * touch me" on both.
   */
  const assignments = [
    { value: "left", text: "A", title: "Cut by the left half of the crossfader" },
    {
      value: "thru",
      text: "\u2014",
      title: "Off the crossfader — this deck plays wherever the crossfader is parked",
    },
    { value: "right", text: "B", title: "Cut by the right half of the crossfader" },
  ] as const;

  /**
   * Whether the grid editing row is showing.
   *
   * Component state rather than engine state on purpose: it is a view
   * preference, not something a controller or the assistant should be able to
   * change out from under the person looking at it.
   */
  let gridOpen = $state(false);

  const send = async (action: string) => {
    try {
      await dispatch(action);
      error = null;
    } catch (e) {
      error = String(e);
    }
  };
</script>

<section class="deck" class:playing={deck.playing}>
  <header>
    <span class="number">{deck.number}</span>
    <div class="meta">
      <div class="title" title={title}>{title || "— no track —"}</div>
      <div class="artist">{artist || (deck.loaded ? "" : "load a file to begin")}</div>
    </div>

    <!--
      Tempo and key, once the analyser has them.
      
      Deliberately absent rather than zeroed while analysis runs, and deliberately
      absent again when the analyser could not tell: a plausible-looking 0.0 BPM
      or a guessed key is worse than a blank, because a DJ reads these at a
      glance and will not stop to wonder whether the number is real.
    -->
    {#if deck.loaded}
      <div class="analysis mono">
        {#if analysis?.bpm != null}
          <!--
            Confidence comes from the deck, not from `analysis`: the analyser's
            number is what it originally found, and a grid the DJ has since
            edited by hand is certain. Reading the cached number here would show
            "weak grid" beside an enabled Sync button.
          -->
          <span
            class="bpm"
            class:unsure={!deck.can_sync}
            title={deck.can_sync
              ? `Beat grid confidence ${(deck.grid_confidence * 100).toFixed(0)}%`
              : `Weak beat grid — ${(deck.grid_confidence * 100).toFixed(0)}% confidence. Sync stays disabled rather than guessing. Tap or nudge the grid to fix it.`}
          >
            <!--
              The tempo being *played*, not the tempo the file was recorded at.
              With the pitch fader moved or sync engaged those differ, and the
              one that matters when you are matching two tracks is this one.
            -->
            {(deck.effective_bpm ??
              (octaveSwapped && analysis.bpm_alternative != null
                ? analysis.bpm_alternative
                : analysis.bpm)
            ).toFixed(1)}<em>BPM</em>
          </span>
          <!--
            The octave offered as a control rather than by making the number
            itself clickable: a clickable number is not discoverable, and this
            way the alternative is visible before you commit to it.
          -->
          {#if analysis.bpm_alternative != null}
            <IconButton
              icon="fa-solid fa-circle-question"
              title={`Autocorrelation cannot tell one octave from its double. Use ${(octaveSwapped
                ? analysis.bpm
                : analysis.bpm_alternative
              ).toFixed(1)} instead.`}
              aria-label="Choose alternate octave"
              onClick={swapOctave}
            />
          {/if}
        {/if}
        {#if analysis?.key_camelot}
          <span
            class="key"
            class:unsure={(analysis.key_confidence ?? 0) < 0.5}
            title="{analysis.key_standard}{analysis.key_alternative
              ? ` — could also be ${analysis.key_alternative}`
              : ''} ({((analysis.key_confidence ?? 0) * 100).toFixed(0)}% correlation)"
          >
            {analysis.key_camelot}
          </span>
        {/if}
        {#if analysis == null}
          <span class="pending">analysing…</span>
        {/if}
        <!--
          The way in to grid editing, next to the number it corrects. Shown
          whether or not the analyser found a grid: a track it could not read at
          all is exactly the one that needs tapping in.
        -->
        <IconButton
          icon="fa-solid fa-table-cells"
          title={deck.can_sync ? "Edit the beat grid" : "Edit the beat grid — this one is too weak to sync to"}
          active={gridOpen}
          onClick={() => (gridOpen = !gridOpen)}
          disabled={!enabled}
        />
      </div>
    {/if}
    <IconButton icon="fa-solid fa-folder-open" title={loading ? "Loading…" : "Load"} onClick={pickTrack} disabled={!enabled || loading} />
  </header>

  {#if deck.loaded}
    <!-- M6.1 STEMS UI MODULE -->
    <Stems deckNumber={deck.number} muteState={deck.stem_mutes} volumeState={deck.stem_volumes} eqState={deck.stem_eq} filterState={deck.stem_filters} soloing={deck.stem_soloing} swap={stemSwap} deckCount={deckCount} />
  {/if}

  <!--
    Tiles come from the Rust renderer and are scrolled by a CSS transform;
    nothing here draws. See docs/adr/0004-waveform-rendering-strategy.md.
  -->
  <Waveform {deck} height={waveHeight} />

  <!--
    The whole track under the scrolling lane. Two views answering different
    questions: the lane says what is about to happen, this says where in the
    track you are and where the breakdown is.
  -->
  {#if deck.loaded && showOverview}
    <Overview {deck} height={30} />
  {/if}

  <div class="progress" role="progressbar" aria-valuenow={progress * 100}>
    <div class="fill" style:scale="{fill(progress)} 1"></div>
  </div>

  <div class="times mono">
    <span>{formatTime(deck.position_seconds)}</span>
    <span class="remaining">
      -{formatTime(Math.max(0, deck.length_seconds - deck.position_seconds))}
    </span>
  </div>

  <!--
    The pad zone. One grid of eight with a page selector, the way hardware
    works — and the way it has to work if a controller's pads are ever to mean
    the same thing as these. The pages come from Rust, already rendered into
    action strings; see `crates/dj-core/src/pads.rs`.

    This replaced five separate fixed rows — hot cues, auto loops, saved loops,
    roll — each of which took vertical space whether or not the DJ wanted it,
    and none of which a controller could have mapped onto without the mapping
    being written a second time.
  -->
  {#if deck.loaded && showPads}
    <Pads pages={padPageList} {deck} {sampler} {enabled} {send} />
  {/if}

  <!--
    Beat jump and auto loops. Only shown when there is a grid to measure them
    against: without one the buttons would be present and inert, which reads as
    broken rather than as "this track has no beats yet". Manual looping still
    works — see the in/out pair below, which needs no grid at all.
  -->
  {#if analysis?.bpm != null && (showJump || showLoops)}
    <div class="beatjump">
      {#if showJump}
      <span class="label">Jump</span>
      {#each [-4, -1, 1, 4] as beats (beats)}
        <IconButton
          title={beats > 0 ? `Forward ${beats} beat${beats === 1 ? '' : 's'}` : `Back ${Math.abs(beats)} beat${Math.abs(beats) === 1 ? '' : 's'}`}
          aria-label={beats > 0 ? `Forward ${beats}` : `Back ${Math.abs(beats)}`}
          disabled={!enabled || !deck.loaded}
          onClick={() => send(`deck ${deck.number} beatjump ${beats}`)}
        >
          {beats > 0 ? `+${beats}` : beats}
        </IconButton>
      {/each}
      {/if}

    </div>
  {/if}

  {#if deck.loaded && showLoops}
    <div class="beatjump loop-row">
      <IconButton
        icon="fa-solid fa-flag"
        title="Drop the loop's in point here"
        aria-label="Loop in"
        disabled={!enabled}
        onClick={() => send(`deck ${deck.number} loop_in`)}
      />
      <IconButton
        icon="fa-solid fa-flag-checkered"
        title="Drop the out point and start looping"
        aria-label="Loop out"
        disabled={!enabled}
        onClick={() => send(`deck ${deck.number} loop_out`)}
      />
      <IconButton
        icon="fa-solid fa-compress"
        title="Halve the loop, keeping its start"
        aria-label="Halve loop"
        disabled={!enabled || deck.active_loop == null}
        onClick={() => send(`deck ${deck.number} loop_halve`)}
      />
      <IconButton
        icon="fa-solid fa-expand"
        title="Double the loop, keeping its start"
        aria-label="Double loop"
        disabled={!enabled || deck.active_loop == null}
        onClick={() => send(`deck ${deck.number} loop_double`)}
      />
      <IconButton
        title="Stop looping and carry on"
        active={deck.active_loop != null}
        disabled={!enabled || deck.active_loop == null}
        onClick={() => send(`deck ${deck.number} loop_off`)}
      >
        {#if deck.active_loop}
          {deck.rolling ? "Rolling" : "Looping"}{deck.active_loop.beats
            ? ` ${formatBeats(deck.active_loop.beats)}`
            : ""}
        {:else}
          No loop
        {/if}
      </IconButton>
    </div>

  {/if}

  <!--
    The effect rack. Below the loops because that is the order a DJ builds in:
    find the section, loop it, then colour it.
  -->
  {#if showFx}
    <Fx slots={deck.fx} {enabled} target="deck {deck.number}" {send} />
  {/if}

  <!--
    Grid editing. Hidden behind a toggle rather than always on: it is the
    control a DJ reaches for once per track at most, and never during a mix,
    while everything above it is touched constantly.

    The order is the order of use. `Here` fixes phase, which is the common
    failure and the one-button case. The nudges are for the last few
    milliseconds. `Tap` is for a track with no usable grid at all, and the
    halve/double pair is for the octave errors autocorrelation cannot resolve.
  -->
  {#if deck.loaded && gridOpen}
    <div class="beatjump grid-row">
      <span class="label">Grid</span>
      <IconButton icon="fa-solid fa-location-dot" title="Put a beat on the playhead" onClick={() => send(`deck ${deck.number} grid_here`)} disabled={!enabled} />
      {#each [-10, -1, 1, 10] as ms (ms)}
        <IconButton
          title={`Slide the whole grid ${Math.abs(ms)} ms ${ms < 0 ? 'earlier' : 'later'}`}
          disabled={!enabled}
          onClick={() => send(`deck ${deck.number} grid_nudge ${ms}`)}
        >
          {ms > 0 ? `+${ms}` : ms}
        </IconButton>
      {/each}
      <IconButton icon="fa-solid fa-hand-pointer" title="Tap along with the music" onClick={() => send(`deck ${deck.number} grid_tap`)} disabled={!enabled} />
      <IconButton title="Halve the grid tempo, keeping the beat you lined up" aria-label="Grid ÷2" disabled={!enabled} onClick={() => send(`deck ${deck.number} grid_scale 0.5`)}>÷2</IconButton>
      <IconButton title="Double the grid tempo, keeping the beat you lined up" aria-label="Grid ×2" disabled={!enabled} onClick={() => send(`deck ${deck.number} grid_scale 2`)}>×2</IconButton>
      <IconButton icon="fa-solid fa-rotate-left" title="Reset grid edits" onClick={() => send(`deck ${deck.number} grid_reset`)} disabled={!enabled} />
    </div>
  {/if}

  <div class="transport">
    <SvgPad
      label="CUE"
      disabled={!enabled || !deck.loaded}
      onclick={() => send(`deck ${deck.number} cue`)}
    />
    <SvgPad
      label={deck.playing ? "PAUSE" : "PLAY"}
      active={deck.playing}
      disabled={!enabled || !deck.loaded}
      onclick={() => send(`deck ${deck.number} play_pause`)}
    />
    <SvgPad
      label="SYNC"
      active={deck.synced}
      disabled={!enabled || !deck.can_sync}
      onclick={() => send(`deck ${deck.number} ${deck.synced ? "sync_off" : "sync"}`)}
    />
    <SvgPad
      label="EJECT"
      disabled={!enabled || !deck.loaded}
      onclick={() => send(`deck ${deck.number} eject`)}
    />
  </div>

  <!--
    The platter. Drag the middle to scratch, the rim to bend, and wind it to
    search a paused deck -- the same three things the hardware does, and the
    same actions a controller mapping sends.
  -->
  <div class="jog-row">
    <JogWheel
      deckNumber={deck.number}
      touched={deck.jog_touched}
      mode={deck.jog_mode}
      bend={deck.jog_bend}
      enabled={enabled && deck.loaded}
    />
  </div>

  <!--
    Pre-fader listen. Deliberately explains itself when unavailable rather than
    just sitting greyed out: a 2-channel laptop output has nowhere to send a
    cue, and "why is this dead" is a bad thing to wonder mid-set.
  -->
  <button
    class="cue"
    class:on={deck.cue_enabled}
    disabled={!enabled || !cueAvailable}
    onclick={() => send(`deck ${deck.number} cue_toggle`)}
    title={cueAvailable
      ? "Pre-fader listen — hear this deck in the headphones"
      : "Needs a 4-channel output device"}
  >
    <span>CUE</span>
    <span class="pfl-meter" aria-hidden="true">
      <span class="pfl-fill" style:scale="{fill(deck.pre_fader_level)} 1"></span>
    </span>
  </button>

  <!--
    Isolator EQ: each knob runs from a true kill at 0 to +12 dB. Double-click
    resets to unity, because reaching for exactly 1.00 with a mouse mid-mix is
    not a thing anyone can do.
  -->
  {#if showEq}
  <div class="eq">
    {#each [{ id: "eq_high", label: "HI", value: deck.eq_high }, { id: "eq_mid", label: "MID", value: deck.eq_mid }, { id: "eq_low", label: "LOW", value: deck.eq_low }] as band (band.id)}
      <label class="band" class:killed={band.value < 0.001}>
        <SvgKnob
          value={band.value}
          min={0}
          max={4}
          step={0.01}
          label={band.label}
          readout={band.value < 0.001 ? "kill" : band.value.toFixed(2)}
          size={46}
          disabled={!enabled}
          oninput={(val) => send(`deck ${deck.number} ${band.id} ${val}`)}
          ondblclick={() => send(`deck ${deck.number} ${band.id} 1`)}
        />
        <button
          class="kill"
          class:on={band.value < 0.001}
          disabled={!enabled}
          onclick={() => send(`deck ${deck.number} ${band.id} ${band.value < 0.001 ? 1 : 0}`)}
          title="Kill {band.label}"
          aria-label="Kill {band.label}"
        ></button>
      </label>
    {/each}
  </div>
  {/if}

  {#if showFilter}
  <label class="control">
    <SvgKnob
      value={deck.filter}
      min={-1}
      max={1}
      step={0.01}
      label="Filter"
      readout={Math.abs(deck.filter) <= 0.02
        ? "off"
        : deck.filter < 0
          ? `LP ${Math.round(-deck.filter * 100)}%`
          : `HP ${Math.round(deck.filter * 100)}%`}
      disabled={!enabled}
      size={56}
      oninput={(val) => send(`deck ${deck.number} filter ${val}`)}
      ondblclick={() => send(`deck ${deck.number} filter 0`)}
    />
  </label>
  {/if}

  <label class="control fader-wrap">
    <SvgFader
      value={deck.volume}
      min={0}
      max={1}
      step={0.01}
      label="Volume"
      readout={deck.volume.toFixed(2)}
      disabled={!enabled}
      height={140}
      width={40}
      oninput={(val) => send(`deck ${deck.number} volume ${val}`)}
    />
  </label>

  <!--
    Pitch and keylock belong together: keylock only means anything once the
    fader has moved, and the two are always reached for in the same breath.
    Double-click the fader to snap back to zero — hitting exactly 0.0% with a
    mouse is not something anyone can do mid-mix.
  -->
  <div class="pitch-row">
    <label class="control fader-wrap">
      <SvgFader
        value={deck.pitch}
        min={-0.16}
        max={0.16}
        step={0.001}
        label="Pitch"
        readout={`${(deck.pitch * 100).toFixed(1)}%`}
        disabled={!enabled}
        height={140}
        width={40}
        oninput={(val) => send(`deck ${deck.number} pitch ${val}`)}
        ondblclick={() => send(`deck ${deck.number} pitch 0`)}
      />
    </label>
    <!--
      Harmonic mixing: shift the key in semitones without touching tempo.
      Separate from keylock, and engages the shifter on its own — but hidden
      with it, because a layout that judges keylock too advanced to show is not
      one that wants semitone transposition either.
    -->
    {#if showKeylock}
      <div class="keyshift">
      <IconButton
        icon="fa-solid fa-minus"
        title="Down a semitone"
        disabled={!enabled}
        onClick={() => send(`deck ${deck.number} key ${deck.key_shift - 1}`)}
        aria-label="Down a semitone"
      />
      <span class="mono" class:shifted={deck.key_shift !== 0}>
        {deck.key_shift > 0 ? `+${deck.key_shift}` : deck.key_shift}
      </span>
      <IconButton
        icon="fa-solid fa-plus"
        title="Up a semitone"
        disabled={!enabled}
        onClick={() => send(`deck ${deck.number} key ${deck.key_shift + 1}`)}
        aria-label="Up a semitone"
      />
    </div>
    <!--
      Slip, reverse and censor. One row because they are one idea: a shadow
      playhead that keeps running at the natural rate while something diverts
      the audible one. Censor is momentary — held, not toggled — because a
      toggled censor is just reverse with extra steps.
    -->
    {#if showSlip}
      <IconButton
        title={deck.slip
          ? "Slip on — loop, reverse or censor, and the track carries on underneath"
          : "Slip off — the playhead stays wherever a loop or a censor leaves it"}
        active={deck.slip}
        disabled={!enabled}
        onClick={() => send(`deck ${deck.number} slip_toggle`)}
        aria-label="Slip"
      >
        SLIP
      </IconButton>
      <IconButton
        icon="fa-solid fa-backward"
        title="Play backwards"
        active={deck.reversed}
        disabled={!enabled || !deck.loaded}
        onClick={() => send(`deck ${deck.number} reverse_toggle`)}
        aria-label="Reverse"
      />
      <!--
        Held rather than clicked, and on pointer events rather than mouse ones
        so it works from a touchscreen. `pointerleave` releases too: dragging
        off the pad mid-censor must not leave the deck stuck in reverse.
      -->
      <IconButton
        icon="fa-solid fa-hand-paper"
        title="Hold to reverse over a word, and land back on the beat"
        disabled={!enabled || !deck.loaded}
        onpointerdown={() => send(`deck ${deck.number} censor_on`)}
        onpointerup={() => send(`deck ${deck.number} censor_off`)}
        onpointerleave={() => send(`deck ${deck.number} censor_off`)}
        aria-label="Censor"
      />

      <!--
        Brake and backspin. Held rather than clicked, and momentary in an
        unusual way: the press *starts* a coast that runs on its own, and the
        release puts the motor back on wherever the record got to. Letting one
        run to the end leaves the deck stopped, which is the point.

        Only with a grid, because a coast is measured in beats — the engine
        refuses one it cannot measure, and a control that silently does nothing
        reads as broken.
      -->
      {#if analysis?.bpm != null}
        <IconButton
          title="Cut the motor and coast to a stop over two beats. Let go to put it back on."
          active={deck.spinning}
          disabled={!enabled || !deck.playing}
          onpointerdown={() => send(`deck ${deck.number} brake 2`)}
          onpointerup={() => send(`deck ${deck.number} brake_off`)}
          onpointercancel={() => send(`deck ${deck.number} brake_off`)}
          aria-label="Brake"
        >
          BRAKE
        </IconButton>
        <IconButton
          title="Throw the record backwards and let friction take it down over a beat"
          active={deck.spinning}
          disabled={!enabled || !deck.playing}
          onpointerdown={() => send(`deck ${deck.number} backspin 1`)}
          onpointerup={() => send(`deck ${deck.number} backspin_off`)}
          onpointercancel={() => send(`deck ${deck.number} backspin_off`)}
          aria-label="Spin"
        >
          SPIN
        </IconButton>
      {/if}
    {/if}
    <IconButton
      icon="fa-solid fa-lock"
      title={deck.keylock
        ? `Keylock on — tempo changes without changing key (adds ${deck.keylock_latency_ms.toFixed(0)} ms, compensated)`
        : "Keylock off — the pitch fader moves tempo and key together"}
      active={deck.keylock}
      disabled={!enabled}
      onClick={() => send(`deck ${deck.number} keylock_toggle`)}
      aria-label="Keylock"
    />
    {/if}
  </div>

  <!--
    Crossfader assignment. A hardware mixer puts this switch on every channel,
    and once four decks are on screen it stops being optional: without it the
    crossfader can only reach decks 1 and 2, so half the mixer is outside the
    one control a DJ uses without looking.
  -->
  <div class="xfader-assign" role="group" aria-label="crossfader assignment">
    <span class="label">X</span>
    {#each assignments as option (option.value)}
      <IconButton
        active={deck.crossfader_assign === option.value}
        disabled={!enabled}
        onClick={() => send(`deck ${deck.number} xfader_${option.value}`)}
        title={option.title}
        aria-pressed={deck.crossfader_assign === option.value}
      >
        {option.text}
      </IconButton>
    {/each}
  </div>

  <div class="meter" aria-label="deck level">
    <div class="meter-fill" style:scale="{fill(deck.peak)} 1"></div>
  </div>

  {#if error}
    <p class="error">{error}</p>
  {/if}
</section>

<style>
  .jog-row {
    display: flex;
    justify-content: center;
    padding: 0.4rem 0;
  }

  .deck {
    background: var(--panel);
    border: 1px solid var(--border);
    /* Border colour only: a deck that resized or moved when it started playing
       would shift everything around it, mid-set. */
    transition: border-color var(--motion-enter) var(--ease);
    border-radius: 10px;
    padding: 0.9rem;
    display: flex;
    flex-direction: column;
    gap: 0.65rem;
    min-width: 0;
  }

  .deck.playing {
    border-color: color-mix(in srgb, var(--accent-2) 45%, var(--border));
  }

  header {
    display: flex;
    align-items: center;
    gap: 0.65rem;
    min-width: 0;
  }
  .beatjump {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    font-size: 0.8em;
  }

  /*
    Slightly quieter than the row above it: grid editing is a repair job, not
    part of playing, and it should not compete with the loop controls for
    attention while a mix is running.
  */
  .beatjump .label {
    color: var(--text-dim);
    margin-right: 0.2rem;
  }
  .analysis {
    display: flex;
    align-items: baseline;
    gap: 0.5rem;
    font-size: 0.85em;
    white-space: nowrap;
  }

  .analysis em {
    font-style: normal;
    font-size: 0.7em;
    color: var(--text-dim);
    margin-left: 0.15em;
  }

  .bpm {
    color: var(--text);
    font-weight: 600;
  }
  /*
    A number the analyser is not sure of still gets shown — it is usually right,
    and a blank would be less useful — but it is visibly dimmer, because the one
    thing worse than no BPM is a wrong one presented with confidence.
  */
  .bpm.unsure,
  .key.unsure {
    color: var(--warn);
    font-weight: 400;
  }

  .key {
    color: var(--accent-2);
    font-weight: 600;
  }

  .pending {
    color: var(--text-dim);
    font-size: 0.9em;
  }

  .number {
    font-size: 1.5rem;
    font-weight: 700;
    color: var(--accent);
    width: 1.4rem;
    text-align: center;
    flex: none;
  }

  .meta {
    flex: 1;
    min-width: 0;
  }

  .title,
  .artist {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .title {
    font-weight: 600;
  }

  .artist {
    color: var(--text-dim);
    font-size: 0.85em;
  }

  .progress {
    height: 8px;
    background: var(--panel-raised);
    border-radius: 4px;
    overflow: hidden;
  }

  .fill {
    width: 100%;
    transform-origin: left center;
    height: 100%;
    background: linear-gradient(90deg, var(--accent), var(--accent-2));
  }

  .times {
    display: flex;
    justify-content: space-between;
    font-size: 0.85em;
    color: var(--text-dim);
  }

  .remaining {
    color: var(--warn);
  }

  /*
    Four columns, not three. Sync is a transport control -- it belongs beside
    play and cue, where a DJ's hand already is -- and adding it to a
    three-column grid dropped Eject onto a row of its own, which reads as two
    unrelated groups of buttons rather than one transport.
  */
  .transport {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 0.4rem;
  }

  .control {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.25rem;
    font-size: 0.85em;
    color: var(--text-dim);
  }


  /* Three knobs across, as on the mixer they stand in for. The sliders these
     replaced were stacked, which a knob cannot be without making the EQ taller
     than the transport. */
  .eq {
    display: flex;
    justify-content: space-evenly;
    align-items: flex-start;
    gap: 0.5rem;
  }

  .band {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.25rem;
    font-size: 0.75em;
    color: var(--text-dim);
    letter-spacing: 0.05em;
  }

  .band.killed :global(.label) {
    color: var(--danger);
  }

  .kill {
    width: 14px;
    height: 14px;
    padding: 0;
    border-radius: 3px;
    background: var(--panel-raised);
  }

  .kill.on {
    background: var(--danger);
    border-color: var(--danger);
  }

  .keyshift {
    display: flex;
    align-items: center;
    gap: 0.15rem;
    font-size: 0.72em;
  }
  .keyshift span {
    min-width: 1.6rem;
    text-align: center;
    color: var(--text-dim);
  }

  .keyshift span.shifted {
    color: var(--accent-2);
    font-weight: 600;
  }

  .pitch-row {
    display: grid;
    grid-template-columns: 1fr auto auto;
    align-items: end;
    gap: 0.5rem;
  }
  /* Momentary, so it must not look like something that stays pressed. */
  .cue {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.75em;
    letter-spacing: 0.08em;
    font-weight: 600;
  }

  .cue.on {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--on-accent);
  }

  .pfl-meter {
    flex: 1;
    height: 4px;
    background: var(--scrim);
    border-radius: 2px;
    overflow: hidden;
  }

  .pfl-fill {
    width: 100%;
    transform-origin: left center;
    display: block;
    height: 100%;
    background: var(--accent-2);
  }

  /*
    A compact three-way switch rather than three full-width buttons: it has to
    fit a quarter-width deck in the four-deck layout, and it is set once at the
    start of a mix rather than reached for constantly.
  */
  .xfader-assign {
    display: flex;
    align-items: center;
    gap: 2px;
  }

  .xfader-assign .label {
    font-size: 0.6rem;
    letter-spacing: 0.08em;
    color: var(--text-dim);
    margin-right: 0.3rem;
  }
  .meter {
    height: 4px;
    background: var(--panel-raised);
    border-radius: 2px;
    overflow: hidden;
  }

  .meter-fill {
    width: 100%;
    transform-origin: left center;
    height: 100%;
    background: var(--accent-2);
    /* No CSS transition: a level meter that lags is a lying level meter. */
  }

  .error {
    margin: 0;
    color: var(--danger);
    font-size: 0.8em;
  }
</style>
