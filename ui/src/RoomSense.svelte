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
  import {
    roomForget,
    roomHistory,
    roomPollMs,
    roomRead,
    roomSaw,
    type RoomHistory,
    type RoomRead,
  } from "./api";
  import { onMount } from "svelte";
  import { performance } from "./performance.svelte";

  interface Props {
    enabled: boolean;
  }

  let { enabled }: Props = $props();

  /**
   * How often the room is measured, from djmanzo.
   *
   * Two seconds on a healthy machine — the near window is three minutes, so
   * that fills it with ninety readings, enough for a median to mean something
   * and rare enough that a laptop already mixing does not notice.
   *
   * **Eight when the machine is struggling.** §48 names *reduce audience
   * polling frequency* among the things a laptop mode gives up, and this is the
   * most expensive thing this panel does: a camera frame scaled down and
   * optical-flowed against the last one, every tick. Twenty-two readings still
   * fill the near window, which is the test of whether the saving costs a
   * feature or only sharpness.
   *
   * The number is asked for rather than decided here. §48's priority is a
   * table in `dj_app::thrift` and a second copy of "two seconds, or eight when
   * struggling" is how the two come to disagree — a Rust test fails if this
   * file grows its own again.
   */
  let everyMs = $state(2000);

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

  /**
   * The mean luma change per pixel, out of 255, that reads as full movement.
   */
  const MOVEMENT_SCALE = 64;

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
  /**
   * Which look this is. A measurement waits before it reads, and a DJ can stop
   * and start again inside that wait; a reading from the look that was
   * stopped must not be sent as one from the look that replaced it.
   */
  let generation = 0;
  /** Whether a measurement is already in its wait, so ticks cannot pile up. */
  let measuring = false;
  let timer: ReturnType<typeof setInterval> | undefined;

  /**
   * The camera as asked for. `ideal` rather than exact, so a webcam that
   * cannot do 320×240 gives whatever it can instead of refusing.
   */
  const CAMERA: MediaTrackConstraints = {
    width: { ideal: 320 },
    height: { ideal: 240 },
  };

  /**
   * The microphone as a **measuring** microphone, not a telephone one.
   *
   * A browser's default microphone is tuned for a call: automatic gain
   * control levels a quiet room up and a loud one down, noise suppression
   * treats a crowd as the noise it exists to remove, and echo cancellation
   * subtracts whatever it thinks is playing. Each of those is right for a
   * voice and wrong for a reading whose whole value is *how loud the room is
   * compared with earlier*: with them on, a floor that doubled in volume
   * could read unchanged.
   *
   * Measured, not assumed: against a file of low chatter made at -42 dBFS,
   * this surface read 0.21 with the defaults and 0.30 — the file's own level —
   * without them. The hum in Memory keeps the defaults on purpose: it reads
   * pitch rather than level, and a hummed tune arrived at the same level
   * either way, so there was nothing to fix. They are asked off rather than
   * required off, so a browser that cannot turn one off still opens the
   * microphone.
   */
  const MICROPHONE: MediaTrackConstraints = {
    echoCancellation: false,
    noiseSuppression: false,
    autoGainControl: false,
  };

  /**
   * Open whatever there is: both, then the camera alone, then the microphone
   * alone.
   *
   * Asked for together first so the browser prompts once. Then each on its
   * own, because half the senses is most of the value and both halves are
   * common on their own — a webcam with no microphone, and a booth whose only
   * input is a microphone, or a laptop with its lid shut. **It used to stop
   * after the camera**, so a machine with a microphone and no camera read
   * nothing at all and was told it had neither.
   *
   * A refusal ends it. A DJ who said no to the first prompt did not mean
   * "ask me twice more".
   */
  async function open(): Promise<MediaStream | null> {
    const asks: MediaStreamConstraints[] = [
      { video: CAMERA, audio: MICROPHONE },
      { video: CAMERA },
      { audio: MICROPHONE },
    ];
    let first: unknown = null;
    for (const ask of asks) {
      try {
        return await navigator.mediaDevices.getUserMedia(ask);
      } catch (problem) {
        first ??= problem;
        const name = problem instanceof Error ? problem.name : "";
        if (name === "NotAllowedError" || name === "SecurityError") break;
      }
    }
    error = explain(first);
    return null;
  }

  async function look() {
    error = "";
    const opened = await open();
    if (!opened) return;
    stream = opened;

    haveCamera = stream.getVideoTracks().length > 0;
    haveMic = stream.getAudioTracks().length > 0;

    if (haveCamera && video) {
      video.srcObject = stream;
      await video.play().catch(() => {});
    }
    if (haveMic) {
      audio = new AudioContext();
      analyser = audio.createAnalyser();
      // The longest window an analyser has: about two thirds of a second.
      // Loudness is read as the RMS of one window, and a window shorter than a
      // beat measures *where in the beat it landed* — the first version read
      // 43 ms, and the same record read 0.65 on one schedule and 0.80 on
      // another because one kept landing between kicks.
      analyser.fftSize = 32768;
      audio.createMediaStreamSource(stream).connect(analyser);
    }

    canvas = document.createElement("canvas");
    canvas.width = WIDE;
    canvas.height = HIGH;
    generation += 1;
    looking = true;
    timer = setInterval(() => void measure(), everyMs);
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
    generation += 1;
    looking = false;
    haveCamera = false;
    haveMic = false;
  }

  /**
   * How far apart the two frames that movement is read from are.
   *
   * **Movement used to be the difference between one tick's frame and the
   * last one's**, which made it two different measurements depending on how
   * often the room was looked at — two seconds normally, eight on a
   * struggling laptop (§48) — so a night that changed tier halfway compared
   * readings on two scales. Worse, it aliased with the music: at 120 BPM two
   * seconds is exactly four beats, every look caught the crowd in the same
   * pose, and a floor of people dancing read as a still room. A fixed short
   * gap measures the same thing at every tier. A sixth of a second is shorter
   * than half of any beat a DJ plays, so it can never span a whole bounce and
   * come back to where it started.
   */
  const GAP_MS = 160;

  /**
   * The most a measurement waits before it starts, chosen afresh each time.
   *
   * The gap alone still samples one point of the beat if every look starts on
   * the same one, and a timer running at two seconds under a record at 120 BPM
   * does exactly that. Waiting a random part of a second moves each look to a
   * different point, so across a few minutes of readings every part of the
   * beat is seen. A second covers every tempo down to 60 BPM.
   */
  const JITTER_MS = 1000;

  const wait = (ms: number) => new Promise((done) => setTimeout(done, ms));

  /** The picture as luma, scaled to WIDE×HIGH, or nothing yet. */
  function frame(): Float32Array | null {
    if (!haveCamera || !video || !canvas || video.videoWidth === 0) return null;
    const context = canvas.getContext("2d", { willReadFrequently: true });
    if (!context) return null;
    context.drawImage(video, 0, 0, WIDE, HIGH);
    const pixels = context.getImageData(0, 0, WIDE, HIGH).data;
    const luma = new Float32Array(WIDE * HIGH);
    for (let i = 0; i < luma.length; i++) {
      // Rec. 601 luma: the eye is not equally sensitive to the three
      // channels, and a plain average calls a red-lit room dark.
      luma[i] =
        0.299 * pixels[i * 4] + 0.587 * pixels[i * 4 + 1] + 0.114 * pixels[i * 4 + 2];
    }
    return luma;
  }

  /** One look: three numbers out, no pixels. */
  async function measure() {
    if (measuring) return;
    measuring = true;
    try {
      await measureOnce();
    } finally {
      measuring = false;
    }
  }

  async function measureOnce() {
    const mine = generation;
    const still = () => looking && generation === mine;
    await wait(Math.random() * Math.min(JITTER_MS, everyMs / 2));
    if (!still()) return;

    const reading: { light?: number; movement?: number; loudness?: number } = {};

    const before = frame();
    if (before) {
      await wait(GAP_MS);
      if (!still()) return;
      const after = frame();
      if (after) {
        let sum = 0;
        let changed = 0;
        for (let i = 0; i < after.length; i++) {
          sum += after[i];
          changed += Math.abs(after[i] - before[i]);
        }
        reading.light = sum / after.length / 255;
        // Scaled so that ordinary movement lands mid-range rather than in
        // the bottom tenth: a whole-frame change of 255 never happens, and a
        // reading that only ever uses a sliver of its range is a reading
        // whose own night's distribution is all one bucket.
        reading.movement = Math.min(1, changed / after.length / MOVEMENT_SCALE);
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

  /**
   * §37, and it is not on the poll.
   *
   * What has happened on *previous* nights cannot change while a DJ is
   * looking at it — it is written at the end of a mix and read across whole
   * nights — so asking every three seconds would be a database query a
   * thousand times an hour for an answer that moves once.
   */
  let history = $state<RoomHistory[]>([]);
  const said = $derived(
    history.map((row) => row.says).filter((says): says is string => says !== null),
  );

  onMount(() => stop);

  /**
   * Follow the governor.
   *
   * Re-asked when the tier changes rather than once at mount, because the whole
   * point of §48 is that the machine's answer moves during a set: a laptop that
   * started healthy and is now dropping frames has to actually slow down, and
   * one that recovers has to speed back up. A running look is restarted, since
   * `setInterval` keeps whatever period it was created with.
   */
  $effect(() => {
    const tier = performance.resolved;
    void roomPollMs(tier)
      .then((ms) => {
        if (ms === everyMs) return;
        everyMs = ms;
        if (looking && timer !== undefined) {
          clearInterval(timer);
          timer = setInterval(() => void measure(), everyMs);
        }
      })
      .catch(() => {
        // Keep the period we have. A room read at the wrong frequency is far
        // better than one that stops, and this panel's whole job is to keep
        // looking.
      });
  });

  $effect(() => {
    if (!enabled) return;
    void refresh();
    void roomHistory()
      .then((rows) => (history = rows))
      .catch(() => (history = []));
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
      §35's baseline, as a table: the current activity once, and where it sits
      against each reach it can be placed in. It is the numbers above given the
      only thing that makes them mean anything, which is why it sits directly
      under them.

      A reach with nothing behind it is an em dash rather than a blank or a
      zero — "we have never been here before" is an answer, and the two things
      it must not look like are "usual" and "lowest".
    -->
    {#if read.baseline.length > 0}
      <table class="baseline">
        <thead>
          <tr>
            <th scope="col">Against</th>
            <th scope="col">Now</th>
            <th scope="col">Last 20 min</th>
            <th scope="col">Tonight</th>
            <th scope="col"
              >{read.phase
                ? `At ${read.phase.replace(/_/g, " ")}`
                : "Similar phases"}</th
            >
          </tr>
        </thead>
        <tbody>
          {#each read.baseline as row (row.sense)}
            <tr>
              <th scope="row">{row.sense}</th>
              <td>{Math.round(row.now * 100)}%</td>
              {#each ["recent", "tonight", "phase"] as reach (reach)}
                {@const found = row.against.find((a) => a.horizon === reach)}
                <td
                  class="reach"
                  class:notable={found?.notable}
                  data-against={found?.against ?? "unknown"}
                  title={found ? `${row.sense} ${found.against} ${found.than}` : "Not enough of the night at this reach to compare against"}
                >{found ? found.against : "—"}</td>
              {/each}
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
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

  <!--
    §37: what has happened on previous nights, when there have been enough of
    them to say. Below the numbers because it is a different kind of claim —
    everything above is tonight, measured; this is a tally over nights, and it
    reads as history rather than as a reading.

    Never a causal sentence, and the panel does not add one: the wording is
    Rust's, it says what happened *after*, and the count is in it so a DJ can
    weigh it themselves.
  -->
  {#if said.length > 0}
    <div class="history">
      <h4>On nights like this one</h4>
      <ul>
        {#each said as sentence (sentence)}
          <li>{sentence}</li>
        {/each}
      </ul>
      <p class="note">
        What the room did afterwards, not what the mix did to it — nothing here
        can tell those apart.
      </p>
    </div>
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

  /* §30's `audience`: something the room did, rather than something the DJ or
     the assistant did. */
  .live {
    font-size: 0.72em;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--audience);
    border: 1px solid var(--audience);
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
    border-left: 2px solid var(--audience);
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

  /*
    §35's baseline table. Deliberately plain: it is a reference a DJ glances
    at, not a chart, and every cell is one word that already says everything
    it means.
  */
  .baseline {
    border-collapse: collapse;
    font-size: 0.75em;
    width: 100%;
  }

  .baseline th,
  .baseline td {
    padding: 0.15rem 0.4rem 0.15rem 0;
    text-align: left;
    font-weight: inherit;
    white-space: nowrap;
  }

  /* §37's tally over previous nights, which is not tonight's reading. */
  .history {
    border-top: 1px solid var(--line, #2a2a2a);
    padding-top: 0.4rem;
  }

  .history h4 {
    margin: 0 0 0.2rem;
    font-size: 0.75em;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--muted);
  }

  .history ul {
    margin: 0;
    padding-left: 1.1rem;
    font-size: 0.85em;
  }

  /*
    No special handling of the width, and that is a finding rather than an
    omission. Five nowrap columns in a side dock looked like a table that
    would push its own panel sideways, so wrapping headers and
    `table-layout: fixed` went in with a test to prove it. The test passed
    with every fix removed, at 1280 and at 900: the side dock's own floor is
    wider than this table's natural width, so the overflow cannot happen and
    neither fix was doing anything. Both came out again, with the test.
  */
  .baseline thead th {
    color: var(--muted);
    font-size: 0.9em;
  }

  .baseline tbody th {
    text-transform: capitalize;
  }

  /*
    The usual is not news, so it is not coloured. Only a reach with something
    to say takes the eye — the same rule the sentences above follow, applied
    to the table they came from.
  */
  .reach {
    color: var(--muted);
  }

  .reach.notable {
    color: var(--fg);
  }

  .reach.notable[data-against="highest"],
  .reach.notable[data-against="higher"] {
    color: var(--warn);
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
