<script lang="ts">
  /**
   * §119: the computer's webcam, inside djmanzo.
   *
   * For a stream: the DJ sees what the stream sees without leaving djmanzo,
   * and — popped out into its own window, "djmanzo - Camera" — OBS captures
   * that window. The webview's own camera, asked for when the DJ presses for
   * it and never before. Whether the DJ asked is Rust's for the run
   * (`commands::camera_wanted`), so a panel popped out shows the camera
   * without asking again, and a workspace restored at launch never turns one
   * on by itself.
   *
   * Its second tab is the other camera djmanzo offers, a phone
   * (`PhoneCamera.svelte`). Switching tabs leaves this one's camera as it is:
   * OBS may be capturing it while the phone is set up.
   */
  import { cameraWanted, setCameraWanted } from "./api";
  import PhoneCamera from "./PhoneCamera.svelte";

  const MIRROR_KEY = "djmanzo.camera.mirror";
  const DEVICE_KEY = "djmanzo.camera.device";
  const TAB_KEY = "djmanzo.camera.tab";

  function remembered(key: string): string {
    try {
      return localStorage.getItem(key) ?? "";
    } catch {
      return "";
    }
  }

  function remember(key: string, value: string) {
    try {
      localStorage.setItem(key, value);
    } catch {
      // A private window: the choice lasts as long as the panel.
    }
  }

  let video = $state<HTMLVideoElement | undefined>();
  let stream: MediaStream | null = null;
  let on = $state(false);
  let starting = $state(false);
  let devices = $state<{ id: string; label: string }[]>([]);
  let chosen = $state(remembered(DEVICE_KEY));
  let mirrored = $state(remembered(MIRROR_KEY) === "1");
  /** Only the picture: what OBS captures, with nothing of djmanzo's on it. */
  let clean = $state(false);
  let error = $state("");
  let tab = $state<"computer" | "phone">(remembered(TAB_KEY) === "phone" ? "phone" : "computer");

  /** Why the camera did not open, in the DJ's terms. */
  function why(e: unknown): string {
    const name = e instanceof DOMException ? e.name : "";
    if (name === "NotAllowedError") return "djmanzo was not allowed the camera.";
    if (name === "NotFoundError" || name === "OverconstrainedError") return "No camera was found.";
    if (name === "NotReadableError") return "The camera is busy — another program may have it.";
    return String(e);
  }

  function release() {
    const held = stream;
    stream = null;
    on = false;
    // The picture let go of first, the camera after: stopping a track while
    // a playing <video> still drew it froze WebKitGTK's window (driving
    // §119's camera with its mock device).
    if (video) {
      video.pause();
      video.srcObject = null;
    }
    setTimeout(() => {
      for (const track of held?.getTracks() ?? []) track.stop();
    }, 0);
  }

  async function start() {
    error = "";
    starting = true;
    release();
    const size = { width: { ideal: 1280 }, height: { ideal: 720 } };
    try {
      stream = await navigator.mediaDevices.getUserMedia({
        video: chosen ? { ...size, deviceId: { exact: chosen } } : size,
        audio: false,
      });
      if (video) video.srcObject = stream;
      on = true;
      await setCameraWanted(true);
      // Labels come only once the camera has been allowed.
      devices = (await navigator.mediaDevices.enumerateDevices())
        .filter((d) => d.kind === "videoinput")
        .map((d, i) => ({ id: d.deviceId, label: d.label || `Camera ${i + 1}` }));
      const opened = stream.getVideoTracks()[0]?.getSettings().deviceId;
      if (opened) chosen = opened;
    } catch (e) {
      // A camera remembered from another machine is not an error: the
      // default one is tried instead.
      if (chosen && e instanceof DOMException && e.name === "OverconstrainedError") {
        chosen = "";
        starting = false;
        return start();
      }
      error = why(e);
    } finally {
      starting = false;
    }
  }

  async function stop() {
    release();
    clean = false;
    await setCameraWanted(false);
  }

  function pick(id: string) {
    chosen = id;
    remember(DEVICE_KEY, id);
    if (on) void start();
  }

  // Shown again where the DJ already asked for it this run -- a panel popped
  // out, or brought home -- and given up when the panel goes, without
  // telling Rust: the panel's other place may be about to show it.
  $effect(() => {
    void cameraWanted()
      .then((wanted) => {
        if (wanted) void start();
      })
      .catch(() => {});
    return () => release();
  });

  $effect(() => remember(MIRROR_KEY, mirrored ? "1" : "0"));
  $effect(() => remember(TAB_KEY, tab));
</script>

<div class="camera" class:clean data-camera data-on={on}>
  {#if !clean}
    <div class="tabs" role="tablist" aria-label="Whose camera">
      <button role="tab" aria-selected={tab === "computer"} onclick={() => (tab = "computer")}>
        This computer
      </button>
      <button role="tab" aria-selected={tab === "phone"} onclick={() => (tab = "phone")}>
        A phone
      </button>
    </div>
  {/if}

  {#if tab === "phone" && !clean}
    <PhoneCamera />
  {/if}

  <div class="computer" class:away={tab !== "computer" && !clean}>
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
    <video
      bind:this={video}
      class:mirrored
      class:hidden={!on}
      autoplay
      playsinline
      muted
      aria-label="The camera"
      onclick={() => (clean = false)}
    ></video>

    {#if !on}
      <div class="off">
        <p>The camera is off. It is turned on only when you ask, and only djmanzo's own window can see it.</p>
        <button class="primary" disabled={starting} onclick={() => void start()}>
          {starting ? "Asking for the camera…" : "Show the camera"}
        </button>
      </div>
    {/if}

    {#if !clean}
      <div class="controls">
        {#if on}
          <button onclick={() => void stop()}>Turn the camera off</button>
          {#if devices.length > 1}
            <select aria-label="Which camera" value={chosen} onchange={(e) => pick(e.currentTarget.value)}>
              {#each devices as d (d.id)}<option value={d.id}>{d.label}</option>{/each}
            </select>
          {/if}
        {/if}
        <label><input type="checkbox" bind:checked={mirrored} /> Mirror</label>
        {#if on}
          <button onclick={() => (clean = true)} title="Click the picture to bring them back">Hide the controls</button>
        {/if}
        <span class="hint">For OBS: pop this panel out and add a <em>Window Capture</em> of “djmanzo - Camera”.</span>
      </div>
    {/if}

    {#if error}
      <p class="error" role="alert">{error}</p>
    {/if}
  </div>
</div>

<style>
  .camera {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    height: 100%;
    min-height: 0;
    padding: 0.4rem;
    box-sizing: border-box;
  }

  .tabs {
    display: flex;
    gap: 0.3rem;
    flex: none;
  }

  .tabs button[aria-selected="true"] {
    border-color: var(--accent);
    color: var(--text);
  }

  .tabs button[aria-selected="false"] {
    color: var(--text-dim);
  }

  .computer {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    flex: 1 1 auto;
    min-height: 0;
  }

  /* Out of sight, not unmounted: the camera keeps going. */
  .computer.away {
    display: none;
  }

  /*
   * Black, not a token: the bars around a picture of another shape are what
   * OBS captures with it, and a theme's tint there would be on the stream.
   */
  .camera.clean {
    padding: 0;
    background: rgb(0 0 0);
  }

  video {
    flex: 1 1 auto;
    min-height: 0;
    width: 100%;
    object-fit: contain;
    background: rgb(0 0 0);
    border-radius: var(--radius-s, 4px);
  }

  .clean video {
    border-radius: 0;
    cursor: pointer;
  }

  video.mirrored {
    transform: scaleX(-1);
  }

  video.hidden {
    display: none;
  }

  .off {
    flex: 1 1 auto;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    text-align: center;
    color: var(--text-dim);
  }

  .off p {
    margin: 0;
    max-width: 30rem;
  }

  .controls {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.4rem 0.6rem;
    font-size: 0.9em;
  }

  .controls label {
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
  }

  .hint {
    color: var(--text-dim);
    font-size: 0.9em;
  }

  .error {
    margin: 0;
    color: var(--danger);
  }
</style>
