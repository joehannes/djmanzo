<script lang="ts">
  /**
   * Looking at the room.
   *
   * # What this actually measures
   *
   * Three numbers, every two seconds, from a camera and a microphone:
   * how bright the frame is, how much of it changed since the last one, and
   * how loud it is. That is all a lens and a diaphragm can tell you. It cannot
   * tell you whether people are dancing or leaving, and this panel does not
   * pretend otherwise — the words come from `dj_assistant::room`, which only
   * ever compares the room with itself earlier the same night.
   *
   * # Why the frames never leave this function
   *
   * A camera pointed at a room full of people is the most sensitive thing
   * djmanzo will ever touch. So the video element is never recorded, the
   * canvas is never read back except as three numbers, and what crosses to
   * Rust is those three numbers. There is no image in the request, nothing
   * written to disk, and nothing sent anywhere. The preview exists so the DJ
   * can see what the camera sees and aim it; it can be turned off and the
   * measuring carries on.
   *
   * # Why the phone is not the sensor
   *
   * It should be — a phone can sit on a speaker stack facing the floor, and a
   * laptop is in the booth facing the DJ. But a browser will not open a camera
   * or a microphone on a page served over plain HTTP, and djmanzo cannot serve
   * HTTPS without shipping a TLS stack years out of date onto a port facing a
   * club's wifi. So today the eye is this window, which is a secure context
   * because it is served from localhost. A USB webcam on a long cable is the
   * honest workaround, and it is a good one.
   */
  import { roomForget, roomRead, roomSaw, type RoomRead } from "./api";
  import { onMount } from "svelte";

  interface Props {
    enabled: boolean;
  }

  let { enabled }: Props = $props();

  /**
   * How often the room is measured.
   *
   * Every two seconds. The near window is three minutes, so this fills it with
   * ninety readings — enough for a median to mean something, rare enough that
   * a laptop already mixing does not notice.
   */
  const EVERY_MS = 2000;

  /**
   * How small the frame is scaled before it is measured.
   *
   * 64×48. Luminance and frame difference are averages over the whole picture,
   * and averaging a quarter of a million pixels gives the same answer as
   * averaging three thousand while costing eighty times as much on a machine
   * whose spare time belongs to the audio thread.
   */
  const WIDE = 64;
  const HIGH = 48;

  let read = $state<RoomRead | null>(null);
  let error = $state("");
  let looking = $state(false);
  let showPreview = $state(true);
  /** What each source is giving us, so a refused permission is visible. */
  let haveCamera = $state(false);
  let haveMic = $state(false);

  let video: HTMLVideoElement | null = $state(null);
  let stream: MediaStream | null = null;
  let audio: AudioContext | null = null;
  let analyser: AnalyserNode | null = null;
  let canvas: HTMLCanvasElement | null = null;
  let previous: Uint8ClampedArray | null = null;
  let timer: ReturnType<typeof setInterval> | undefined;

  async function look() {
    error = "";
    try {
      // Asked for together so the browser prompts once. If both are refused
      // there is nothing to measure; if one is, we carry on with the other.
      stream = await navigator.mediaDevices.getUserMedia({
        video: { width: { ideal: 320 }, height: { ideal: 240 } },
        audio: true,
      });
    } catch {
      // Both together failed. Try the camera alone before giving up: a
      // machine with a webcam and no microphone is common, and half the
      // measurements is most of the value.
      try {
        stream = await navigator.mediaDevices.getUserMedia({ video: true });
      } catch (alone) {
        error = explain(alone);
        return;
      }
    }

    haveCamera = stream.getVideoTracks().length > 0;
    haveMic = stream.getAudioTracks().length > 0;

    if (haveCamera && video) {
      video.srcObject = stream;
      await video.play().catch(() => {});
    }
    if (haveMic) {
      audio = new AudioContext();
      analyser = audio.createAnalyser();
      analyser.fftSize = 2048;
      audio.createMediaStreamSource(stream).connect(analyser);
    }

    canvas = document.createElement("canvas");
    canvas.width = WIDE;
    canvas.height = HIGH;
    previous = null;
    looking = true;
    timer = setInterval(() => void measure(), EVERY_MS);
  }

  /**
   * Say what went wrong in terms of what to do about it.
   *
   * "No device" and "you said no" are the two answers a browser gives here,
   * and they need opposite responses from the DJ — plug something in, or click
   * allow. Reporting the raw `OverconstrainedError: Invalid constraint` for
   * both tells them neither.
   */
  function explain(problem: unknown): string {
    const name = problem instanceof Error ? problem.name : "";
    switch (name) {
      case "NotAllowedError":
      case "SecurityError":
        return (
          "djmanzo was not allowed to use the camera or microphone. Your " +
          "system's privacy settings decide this — allow djmanzo there, then " +
          "try again."
        );
      case "NotFoundError":
      case "DevicesNotFoundError":
      case "OverconstrainedError":
        return (
          "There is no camera or microphone on this machine. Any USB webcam " +
          "will do, and a long cable puts the lens where the floor is."
        );
      case "NotReadableError":
      case "TrackStartError":
        return (
          "The camera is there but something else is using it. Close the " +
          "other application and try again."
        );
      default:
        return `The camera could not be opened: ${problem}`;
    }
  }

  function stop() {
    clearInterval(timer);
    timer = undefined;
    stream?.getTracks().forEach((track) => track.stop());
    stream = null;
    void audio?.close();
    audio = null;
    analyser = null;
    canvas = null;
    previous = null;
    looking = false;
    haveCamera = false;
    haveMic = false;
  }

  /** One look: three numbers out, no pixels. */
  async function measure() {
    const reading: { light?: number; movement?: number; loudness?: number } = {};

    if (haveCamera && video && canvas && video.videoWidth > 0) {
      const context = canvas.getContext("2d", { willReadFrequently: true });
      if (context) {
        context.drawImage(video, 0, 0, WIDE, HIGH);
        const frame = context.getImageData(0, 0, WIDE, HIGH).data;
        let sum = 0;
        let changed = 0;
        for (let i = 0; i < frame.length; i += 4) {
          // Rec. 601 luma: the eye is not equally sensitive to the three
          // channels, and a plain average calls a red-lit room dark.
          const luma =
            0.299 * frame[i] + 0.587 * frame[i + 1] + 0.114 * frame[i + 2];
          sum += luma;
          if (previous) {
            const was =
              0.299 * previous[i] +
              0.587 * previous[i + 1] +
              0.114 * previous[i + 2];
            changed += Math.abs(luma - was);
          }
        }
        const pixels = frame.length / 4;
        reading.light = sum / pixels / 255;
        if (previous) {
          // Scaled so that ordinary movement lands mid-range rather than in
          // the bottom tenth: a whole-frame change of 255 never happens, and a
          // reading that only ever uses a sliver of its range is a reading
          // whose own night's distribution is all one bucket.
          reading.movement = Math.min(1, changed / pixels / 64);
        }
        previous = frame;
      }
    }

    if (analyser) {
      const samples = new Float32Array(analyser.fftSize);
      analyser.getFloatTimeDomainData(samples);
      let squares = 0;
      for (const sample of samples) squares += sample * sample;
      const rms = Math.sqrt(squares / samples.length);
      // Loudness is logarithmic, and a linear RMS spends its whole range in
      // the bottom tenth. -60 dBFS to 0 across the full scale.
      const db = 20 * Math.log10(Math.max(rms, 1e-6));
      reading.loudness = Math.min(1, Math.max(0, (db + 60) / 60));
    }

    if (reading.light === undefined && reading.loudness === undefined) return;
    try {
      await roomSaw(reading);
    } catch (e) {
      error = String(e);
    }
  }

  async function refresh() {
    try {
      read = await roomRead();
    } catch (e) {
      error = String(e);
    }
  }

  onMount(() => stop);

  $effect(() => {
    if (!enabled) return;
    void refresh();
    const poll = setInterval(() => void refresh(), 3000);
    return () => clearInterval(poll);
  });

  /** A 0..1 reading as a percentage, or a dash when there is none. */
  function meter(value: number | null): string {
    return value === null ? "—" : `${Math.round(value * 100)}%`;
  }
</script>

<div class="room">
  {#if error}
    <p class="error">{error}</p>
  {/if}

  <div class="strip">
    {#if looking}
      <span class="live">Watching</span>
      <span class="sources">
        {haveCamera ? "camera" : "no camera"} · {haveMic
          ? "microphone"
          : "no microphone"}
      </span>
      <button class="quiet" onclick={stop}>Stop looking</button>
    {:else}
      <button class="go" onclick={look}>Look at the room</button>
    {/if}
  </div>

  {#if read?.disagreement}
    <!--
      The one interpretation djmanzo offers, and it compares two things it
      actually knows: the occasion you set, and what the sensors measured. It
      does not say what to play — that is the planner's job, and ADR-0005's
      rule.
    -->
    <p class="disagreement">{read.disagreement}</p>
  {/if}

  {#if read && read.notes.length > 0}
    <ul class="notes">
      {#each read.notes as note (note)}
        <li>{note}</li>
      {/each}
    </ul>
  {:else if read?.enough}
    <p class="steady">The room is carrying on much as it has been.</p>
  {:else if looking}
    <p class="steady">
      Looking. It takes about half a minute before there is enough to say
      anything, and a while longer before “usual” means anything.
    </p>
  {/if}

  {#if read}
    <dl class="numbers">
      <div><dt>Light</dt><dd>{meter(read.light)}</dd></div>
      <div><dt>Movement</dt><dd>{meter(read.movement)}</dd></div>
      <div><dt>Loudness</dt><dd>{meter(read.loudness)}</dd></div>
      <div>
        <dt>Hour</dt>
        <dd>{read.hour === null ? "—" : `${read.hour}:00`}</dd>
      </div>
    </dl>
    <!--
      Said next to the numbers because the numbers invite the wrong reading:
      40% light is not "dim", it is 40% of what this lens reports, and the only
      true statement is a comparison with the same lens earlier tonight.
    -->
    <p class="note">
      These are this camera's own numbers, and mean nothing on their own — two
      lenses pointed at the same wall report different ones. Everything djmanzo
      says compares the room with itself earlier tonight.
      {#if read.recent > 0}({read.recent} readings in the last three minutes.){/if}
    </p>
  {/if}

  {#if looking && haveCamera}
    <label class="check">
      <input type="checkbox" bind:checked={showPreview} />
      Show what the camera sees
    </label>
  {/if}
  <!--
    Hidden rather than unmounted: the element is the frame source, so removing
    it would stop the measuring along with the picture.
  -->
  <video
    bind:this={video}
    class="preview"
    class:hidden={!looking || !haveCamera || !showPreview}
    muted
    playsinline
    aria-label="What the camera sees"
  ></video>

  <details class="about">
    <summary>What this can and cannot tell you</summary>
    <p>
      Nothing leaves this window. The picture is scaled to {WIDE}×{HIGH},
      averaged into three numbers, and thrown away — no frame is recorded, saved
      or sent anywhere.
    </p>
    <p>
      A camera can measure brightness and how much of the picture changed. It
      cannot tell dancing from leaving, so djmanzo never claims a mood: it says
      the floor is stiller or busier <em>than it has been tonight</em>, which is
      a fact about a number.
    </p>
    <p>
      <strong>Weather is not here.</strong> It is not something a camera sees — it
      is your location plus somebody else's forecast — so djmanzo does not pretend
      to know it. The hour it does know, from the clock.
    </p>
    <p>
      <strong>A phone would be better</strong>, sitting on a speaker stack facing
      the floor while the laptop faces you. Browsers will not open a camera on a
      page served over plain HTTP, and djmanzo will not ship an out-of-date TLS
      stack onto a club's wifi to get around it. A USB webcam on a long cable does
      the same job today.
    </p>
    <button class="quiet" onclick={() => roomForget().then(refresh)}>
      Forget tonight's readings
    </button>
    <p class="note">
      Everything is judged against the rest of tonight, so if you move the
      camera somewhere else, forget the readings first — otherwise you are
      comparing this corner with a different one.
    </p>
  </details>
</div>

<style>
  .room {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    flex: 1;
    min-height: 0;
    overflow: auto;
  }

  .strip {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
    flex: none;
  }

  .live {
    font-size: 0.72em;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--accent);
    border: 1px solid var(--accent);
    border-radius: 999px;
    padding: 0.1rem 0.45rem;
  }

  .sources {
    font-size: 0.78em;
    color: var(--text-dim);
  }

  /* The one sentence that is an interpretation, marked as one. */
  .disagreement {
    margin: 0;
    padding: 0.5rem 0.6rem;
    border-left: 2px solid var(--accent);
    background: var(--panel-raised);
    font-size: 0.9em;
    line-height: 1.5;
  }

  .notes {
    margin: 0;
    padding-left: 1.1rem;
    font-size: 0.85em;
    line-height: 1.55;
  }

  .numbers {
    display: flex;
    flex-wrap: wrap;
    gap: 0.9rem;
    margin: 0;
    font-size: 0.8em;
  }

  .numbers div {
    display: flex;
    gap: 0.35rem;
  }

  .numbers dt {
    color: var(--text-dim);
  }

  .numbers dd {
    margin: 0;
    font-variant-numeric: tabular-nums;
  }

  .preview {
    width: 100%;
    max-width: 16rem;
    border-radius: 6px;
    border: 1px solid var(--border);
    /*
      The panel's own sunken surface, not black. There is no reason a camera's
      letterbox has to be black, and a hex here would be the one part of this
      panel that looks the same on all six palettes.
    */
    background: var(--panel-raised);
  }

  .preview.hidden {
    display: none;
  }

  .check {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    font-size: 0.78em;
    color: var(--text-dim);
  }

  .about summary {
    font-size: 0.8em;
    color: var(--text-dim);
    cursor: pointer;
  }

  .about p {
    font-size: 0.78em;
    line-height: 1.55;
    color: var(--text-dim);
  }

  .steady,
  .note,
  .error {
    margin: 0;
    font-size: 0.8em;
    line-height: 1.55;
    color: var(--text-dim);
  }

  .error {
    color: var(--danger);
  }
</style>
