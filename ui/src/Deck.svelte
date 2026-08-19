<script lang="ts">
  import { dispatch, formatTime, loadTrack, type DeckState, type Layout } from "./api";
  import Overview from "./Overview.svelte";
  import Waveform from "./Waveform.svelte";
  import { open } from "@tauri-apps/plugin-dialog";

  let {
    deck,
    enabled,
    cueAvailable = false,
    layout = null,
  }: {
    deck: DeckState;
    enabled: boolean;
    cueAvailable?: boolean;
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

  /**
   * Loop length in whole beats, when it is one.
   *
   * Used only to light up the matching auto-loop button. A halved loop is half
   * a beat and matches none of them, which is correct — the buttons say what
   * they would set, not what is playing.
   */
  const activeLoopBeats = $derived.by(() => {
    const beats = deck.active_loop?.beats;
    if (beats == null) return null;
    const rounded = Math.round(beats);
    return Math.abs(beats - rounded) < 0.01 ? rounded : null;
  });

  /** "4" for whole loops, "1/4" for halved ones, which is how DJs say them. */
  function formatBeats(beats: number): string {
    if (beats >= 1) return String(Math.round(beats * 100) / 100);
    return `1/${Math.round(1 / beats)}`;
  }

  /**
   * Which roll pad is being held, if any.
   *
   * Local rather than read back from the deck because it is also a guard.
   * Releasing a roll ends the loop, so a `roll_off` sent by a pad that was
   * never pressed would cancel a loop the DJ set on purpose — the pointer
   * merely passing over the row would do it. The censor can be sloppy about
   * this because `censor_off` on a deck that is not censoring changes nothing.
   */
  let rolling = $state<number | null>(null);

  /**
   * Hold a roll.
   *
   * Pointer capture rather than a `pointerleave` release: dragging off a pad
   * mid-roll should keep rolling until the finger lifts, the way a hardware pad
   * does, and capture also guarantees the release arrives at all.
   */
  function startRoll(event: PointerEvent, beats: number) {
    // The action first. Capture is a convenience and the roll is the point, so
    // a browser that refuses the capture must still roll.
    rolling = beats;
    send(`deck ${deck.number} roll ${beats}`);
    (event.currentTarget as HTMLButtonElement).setPointerCapture(event.pointerId);
  }

  function endRoll() {
    if (rolling == null) return;
    rolling = null;
    send(`deck ${deck.number} roll_off`);
  }

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
            <button
              class="octave"
              onclick={swapOctave}
              title="Autocorrelation cannot tell one octave from its double. Use {(octaveSwapped
                ? analysis.bpm
                : analysis.bpm_alternative
              ).toFixed(1)} instead."
            >
              {(octaveSwapped ? analysis.bpm : analysis.bpm_alternative).toFixed(0)}?
            </button>
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
        <button
          class="grid-toggle"
          class:active={gridOpen}
          onclick={() => (gridOpen = !gridOpen)}
          title={deck.can_sync
            ? "Edit the beat grid"
            : "Edit the beat grid — this one is too weak to sync to"}
        >
          Grid
        </button>
      </div>
    {/if}
    <button onclick={pickTrack} disabled={!enabled || loading}>
      {loading ? "Loading…" : "Load"}
    </button>
  </header>

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
    <div class="fill" style:width="{Math.min(progress, 1) * 100}%"></div>
  </div>

  <div class="times mono">
    <span>{formatTime(deck.position_seconds)}</span>
    <span class="remaining">
      -{formatTime(Math.max(0, deck.length_seconds - deck.position_seconds))}
    </span>
  </div>

  <!--
    Hot cues. Always shown when a track is loaded, because they need no beat
    grid — a cue is a position, and a position exists whether or not the
    analyser found a tempo.
  -->
  {#if deck.loaded && showPads}
    <div class="pads">
      {#each deck.hot_cues as cue, index (index)}
        <button
          class="pad"
          class:set={cue != null}
          disabled={!enabled}
          onclick={() => send(`deck ${deck.number} hotcue ${index + 1}`)}
          oncontextmenu={(e) => {
            e.preventDefault();
            send(`deck ${deck.number} hotcue_clear ${index + 1}`);
          }}
          title={cue != null
            ? `Jump to cue ${index + 1} — right-click to clear`
            : `Set cue ${index + 1} here`}
        >
          {index + 1}
        </button>
      {/each}
    </div>
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
        <button
          onclick={() => send(`deck ${deck.number} beatjump ${beats}`)}
          disabled={!enabled || !deck.loaded}
          title="{beats > 0 ? 'Forward' : 'Back'} {Math.abs(beats)} beat{Math.abs(beats) === 1
            ? ''
            : 's'}"
        >
          {beats > 0 ? `+${beats}` : beats}
        </button>
      {/each}
      {/if}

      {#if showLoops}
      <span class="label loop-label">Loop</span>
      {#each [1, 2, 4, 8] as beats (beats)}
        <button
          class:active={activeLoopBeats === beats}
          onclick={() => send(`deck ${deck.number} loop ${beats}`)}
          disabled={!enabled || !deck.loaded}
          title="Loop {beats} beat{beats === 1 ? '' : 's'} from here"
        >
          {beats}
        </button>
      {/each}
      {/if}
    </div>
  {/if}

  <!--
    Loop roll. Held rather than clicked, and always slipping: the track carries
    on underneath, so letting go lands you where you would have been. That is
    the whole difference from the auto-loop row above, and it is why they are
    two rows rather than one — the same length means a different thing here.

    Fractions first, because the roll a DJ means by the word is the sub-beat
    one. A grid is required: the engine refuses a roll it cannot measure, and a
    pad that silently does nothing reads as broken.
  -->
  {#if deck.loaded && showLoops && analysis?.bpm != null}
    <div class="beatjump loop-row">
      <span class="label">Roll</span>
      {#each [0.125, 0.25, 0.5, 1, 2, 4] as beats (beats)}
        <button
          class="roll"
          class:on={rolling === beats && deck.rolling}
          disabled={!enabled}
          onpointerdown={(event) => startRoll(event, beats)}
          onpointerup={endRoll}
          onpointercancel={endRoll}
          title="Hold to roll {formatBeats(beats)} beat{beats === 1
            ? ''
            : 's'}; let go and the track carries on from where it would have been"
        >
          {formatBeats(beats)}
        </button>
      {/each}
    </div>
  {/if}

  {#if deck.loaded && showLoops}
    <div class="beatjump loop-row">
      <button
        onclick={() => send(`deck ${deck.number} loop_in`)}
        disabled={!enabled}
        title="Drop the loop's in point here"
      >
        In
      </button>
      <button
        onclick={() => send(`deck ${deck.number} loop_out`)}
        disabled={!enabled}
        title="Drop the out point and start looping"
      >
        Out
      </button>
      <button
        onclick={() => send(`deck ${deck.number} loop_halve`)}
        disabled={!enabled || deck.active_loop == null}
        title="Halve the loop, keeping its start"
      >
        ÷2
      </button>
      <button
        onclick={() => send(`deck ${deck.number} loop_double`)}
        disabled={!enabled || deck.active_loop == null}
        title="Double the loop, keeping its start"
      >
        ×2
      </button>
      <button
        class:active={deck.active_loop != null}
        onclick={() => send(`deck ${deck.number} loop_off`)}
        disabled={!enabled || deck.active_loop == null}
        title="Stop looping and carry on"
      >
        {#if deck.active_loop}
          <!-- A roll is a loop that will end on its own, and saying "looping"
               about one tells a DJ the wrong thing about what happens next. -->
          {deck.rolling ? "Rolling" : "Looping"}{deck.active_loop.beats
            ? ` ${formatBeats(deck.active_loop.beats)}`
            : ""}
        {:else}
          No loop
        {/if}
      </button>
    </div>

    <!--
      Saved loops. Four slots rather than eight: a saved loop is the section you
      come back to on a record you know, and a DJ who needs more than four of
      those on one track is editing, not playing.

      One pad per slot, the way the hot cue row works — click recalls, and
      shift-click saves the loop that is playing over it. Saving on a modifier
      rather than a separate row because the destructive gesture should be the
      deliberate one.
    -->
    {#if showLoops}
    <div class="beatjump loop-row">
      <span class="label">Saved</span>
      {#each [1, 2, 3, 4] as slot (slot)}
        <button
          onclick={(event) =>
            send(
              `deck ${deck.number} ${event.shiftKey ? "loop_save" : "loop_recall"} ${slot}`,
            )}
          disabled={!enabled}
          title="Recall saved loop {slot}. Shift-click to save the loop that is playing into it."
        >
          {slot}
        </button>
      {/each}
    </div>
    {/if}
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
      <button
        onclick={() => send(`deck ${deck.number} grid_here`)}
        disabled={!enabled}
        title="Put a beat on the playhead, leaving the tempo alone. Cue to the downbeat and press once."
      >
        Here
      </button>
      {#each [-10, -1, 1, 10] as ms (ms)}
        <button
          onclick={() => send(`deck ${deck.number} grid_nudge ${ms}`)}
          disabled={!enabled}
          title="Slide the whole grid {Math.abs(ms)} ms {ms < 0 ? 'earlier' : 'later'}"
        >
          {ms > 0 ? `+${ms}` : ms}
        </button>
      {/each}
      <button
        onclick={() => send(`deck ${deck.number} grid_tap`)}
        disabled={!enabled}
        title="Tap along with the music. Two taps give a tempo; the last sets the phase."
      >
        Tap
      </button>
      <button
        onclick={() => send(`deck ${deck.number} grid_scale 0.5`)}
        disabled={!enabled}
        title="Halve the grid tempo, keeping the beat you lined up"
      >
        ÷2
      </button>
      <button
        onclick={() => send(`deck ${deck.number} grid_scale 2`)}
        disabled={!enabled}
        title="Double the grid tempo, keeping the beat you lined up"
      >
        ×2
      </button>
      <button
        onclick={() => send(`deck ${deck.number} grid_reset`)}
        disabled={!enabled}
        title="Throw the edits away and go back to what the analyser found"
      >
        Reset
      </button>
    </div>
  {/if}

  <div class="transport">
    <button onclick={() => send(`deck ${deck.number} cue`)} disabled={!enabled || !deck.loaded}>
      Cue
    </button>
    <button
      class:active={deck.playing}
      onclick={() => send(`deck ${deck.number} play_pause`)}
      disabled={!enabled || !deck.loaded}
    >
      {deck.playing ? "Pause" : "Play"}
    </button>
    <button
      class:active={deck.synced}
      onclick={() => send(`deck ${deck.number} ${deck.synced ? "sync_off" : "sync"}`)}
      disabled={!enabled || !deck.can_sync}
      title={deck.can_sync
        ? "Match tempo and phase to the other playing deck"
        : "No beat grid solid enough to sync to. Syncing to a guess is how a mix derails."}
    >
      Sync
    </button>
    <button onclick={() => send(`deck ${deck.number} eject`)} disabled={!enabled || !deck.loaded}>
      Eject
    </button>
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
      <span class="pfl-fill" style:width="{Math.min(deck.pre_fader_level, 1) * 100}%"></span>
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
        <span>{band.label}</span>
        <input
          type="range"
          min="0"
          max="4"
          step="0.01"
          value={band.value}
          disabled={!enabled}
          oninput={(e) => send(`deck ${deck.number} ${band.id} ${e.currentTarget.value}`)}
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
    <span>
      Filter
      <em class="mono">
        {#if Math.abs(deck.filter) <= 0.02}off{:else if deck.filter < 0}LP {Math.round(
            -deck.filter * 100,
          )}%{:else}HP {Math.round(deck.filter * 100)}%{/if}
      </em>
    </span>
    <input
      type="range"
      min="-1"
      max="1"
      step="0.01"
      value={deck.filter}
      disabled={!enabled}
      oninput={(e) => send(`deck ${deck.number} filter ${e.currentTarget.value}`)}
      ondblclick={() => send(`deck ${deck.number} filter 0`)}
    />
  </label>
  {/if}

  <label class="control">
    <span>Volume <em class="mono">{deck.volume.toFixed(2)}</em></span>
    <input
      type="range"
      min="0"
      max="1"
      step="0.01"
      value={deck.volume}
      disabled={!enabled}
      oninput={(e) => send(`deck ${deck.number} volume ${e.currentTarget.value}`)}
    />
  </label>

  <!--
    Pitch and keylock belong together: keylock only means anything once the
    fader has moved, and the two are always reached for in the same breath.
    Double-click the fader to snap back to zero — hitting exactly 0.0% with a
    mouse is not something anyone can do mid-mix.
  -->
  <div class="pitch-row">
    <label class="control">
      <span>Pitch <em class="mono">{(deck.pitch * 100).toFixed(1)}%</em></span>
      <input
        type="range"
        min="-0.16"
        max="0.16"
        step="0.001"
        value={deck.pitch}
        disabled={!enabled}
        oninput={(e) => send(`deck ${deck.number} pitch ${e.currentTarget.value}`)}
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
      <button
        disabled={!enabled}
        onclick={() => send(`deck ${deck.number} key ${deck.key_shift - 1}`)}
        title="Down a semitone"
      >−</button>
      <span class="mono" class:shifted={deck.key_shift !== 0}>
        {deck.key_shift > 0 ? `+${deck.key_shift}` : deck.key_shift}
      </span>
      <button
        disabled={!enabled}
        onclick={() => send(`deck ${deck.number} key ${deck.key_shift + 1}`)}
        title="Up a semitone"
      >+</button>
    </div>
    <!--
      Slip, reverse and censor. One row because they are one idea: a shadow
      playhead that keeps running at the natural rate while something diverts
      the audible one. Censor is momentary — held, not toggled — because a
      toggled censor is just reverse with extra steps.
    -->
    {#if showSlip}
      <button
        class="slip"
        class:on={deck.slip}
        disabled={!enabled}
        onclick={() => send(`deck ${deck.number} slip_toggle`)}
        title={deck.slip
          ? "Slip on — loop, reverse or censor, and the track carries on underneath"
          : "Slip off — the playhead stays wherever a loop or a censor leaves it"}
      >
        SLIP
      </button>
      <button
        class="slip"
        class:on={deck.reversed}
        disabled={!enabled || !deck.loaded}
        onclick={() => send(`deck ${deck.number} reverse_toggle`)}
        title="Play backwards"
        aria-label="Reverse"
      >
        ◀◀
      </button>
      <!--
        Held rather than clicked, and on pointer events rather than mouse ones
        so it works from a touchscreen. `pointerleave` releases too: dragging
        off the pad mid-censor must not leave the deck stuck in reverse.
      -->
      <button
        class="slip censor"
        disabled={!enabled || !deck.loaded}
        onpointerdown={() => send(`deck ${deck.number} censor_on`)}
        onpointerup={() => send(`deck ${deck.number} censor_off`)}
        onpointerleave={() => send(`deck ${deck.number} censor_off`)}
        title="Hold to reverse over a word, and land back on the beat"
      >
        CENSOR
      </button>
    {/if}
    <button
      class="keylock"
      class:on={deck.keylock}
      disabled={!enabled}
      onclick={() => send(`deck ${deck.number} keylock_toggle`)}
      title={deck.keylock
        ? `Keylock on — tempo changes without changing key (adds ${deck.keylock_latency_ms.toFixed(0)} ms, compensated)`
        : "Keylock off — the pitch fader moves tempo and key together"}
    >
      KEY
    </button>
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
      <button
        class:active={deck.crossfader_assign === option.value}
        disabled={!enabled}
        onclick={() => send(`deck ${deck.number} xfader_${option.value}`)}
        title={option.title}
        aria-pressed={deck.crossfader_assign === option.value}
      >
        {option.text}
      </button>
    {/each}
  </div>

  <div class="meter" aria-label="deck level">
    <div class="meter-fill" style:width="{Math.min(deck.peak, 1) * 100}%"></div>
  </div>

  {#if error}
    <p class="error">{error}</p>
  {/if}
</section>

<style>
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

  .pads {
    display: grid;
    grid-template-columns: repeat(8, 1fr);
    gap: 0.25rem;
  }

  .pad {
    padding: 0.35rem 0;
    font-size: 0.8em;
    font-weight: 600;
    color: var(--text-dim);
  }

  /* A filled pad is filled, not merely labelled: mid-set this is read by shape
     and colour rather than by number. */
  .pad.set {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--on-accent);
  }

  .loop-label {
    margin-left: 0.5rem;
  }

  .loop-row button {
    padding: 0.2rem 0.5rem;
    font-size: 0.8em;
  }

  /* Held, not latched, so the lit state has to arrive and leave with the
     finger. Same accent as the other momentary controls. */
  .roll.on {
    background: var(--accent-2);
    border-color: var(--accent-2);
    color: var(--on-accent);
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
  .grid-row button {
    font-size: 0.85em;
  }

  .grid-toggle {
    padding: 0.05rem 0.35rem;
    font-size: 0.85em;
  }

  .grid-toggle.active {
    background: var(--accent-2);
    color: var(--on-accent);
    border-color: var(--accent-2);
  }

  .beatjump .label {
    color: var(--text-dim);
    margin-right: 0.2rem;
  }

  .beatjump button {
    padding: 0.2rem 0.5rem;
    font-size: 0.95em;
    font-variant-numeric: tabular-nums;
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

  .octave {
    padding: 0.05rem 0.3rem;
    font-size: 0.75em;
    border-radius: 4px;
    color: var(--text-dim);
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
    gap: 0.25rem;
    font-size: 0.85em;
    color: var(--text-dim);
  }

  .control em {
    font-style: normal;
    color: var(--text);
  }

  .eq {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }

  .band {
    display: grid;
    grid-template-columns: 2.2rem 1fr auto;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.75em;
    color: var(--text-dim);
    letter-spacing: 0.05em;
  }

  .band.killed span {
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

  .keyshift button {
    padding: 0.25rem 0.4rem;
    font-size: 0.9em;
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

  .slip {
    padding: 0.15rem 0.35rem;
    font-size: 0.7em;
    letter-spacing: 0.03em;
  }

  .slip.on {
    background: var(--accent-2);
    color: var(--on-accent);
    border-color: var(--accent-2);
  }

  /* Momentary, so it must not look like something that stays pressed. */
  .slip.censor:active {
    background: var(--accent-2);
    color: var(--on-accent);
  }

  .keylock {
    font-size: 0.7em;
    letter-spacing: 0.08em;
    font-weight: 600;
    padding: 0.3rem 0.5rem;
  }

  .keylock.on {
    background: var(--accent-2);
    border-color: var(--accent-2);
    color: var(--on-accent);
  }

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

  .xfader-assign button {
    flex: 1;
    padding: 0.15rem 0;
    font-size: 0.7rem;
    line-height: 1.2;
    min-width: 0;
  }

  .xfader-assign button.active {
    background: var(--accent-2);
    color: var(--on-accent);
    border-color: var(--accent-2);
  }

  .meter {
    height: 4px;
    background: var(--panel-raised);
    border-radius: 2px;
    overflow: hidden;
  }

  .meter-fill {
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
