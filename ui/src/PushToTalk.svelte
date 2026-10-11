<script lang="ts">
  /**
   * A2: push-to-talk (`docs/ASSISTANT.md` §4).
   *
   * Held, the DJ's microphone — one of the strips the Microphones section
   * already has open — takes a spoken command and is out of the PA and the
   * singers' monitor meanwhile; let go, it is put back. The button sends the
   * same two actions the key and a controller pad do, `voice listen_on` and
   * `voice listen_off`, so all three are one thing and a session file says
   * which was held when.
   *
   * Polled rather than told: the key may be held anywhere in the window, and
   * a take is a few seconds long.
   */
  import { onMount } from "svelte";
  import { pretty } from "./keyboard.svelte";
  import {
    dispatch,
    keyboardKeys,
    vocalsState,
    voiceState,
    voiceStripSet,
    type VoiceState,
  } from "./api";

  let voice = $state<VoiceState | null>(null);
  let inputs = $state(0);
  /** The key bound to push-to-talk in the DJ's keyboard map, as it is shown. */
  let key = $state<string | null>(null);
  let error = $state<string | null>(null);
  let holding = $state(false);

  async function refresh() {
    try {
      voice = await voiceState();
      inputs = (await vocalsState()).inputs;
    } catch {
      // A panel that cannot ask says nothing new rather than something wrong.
    }
  }

  onMount(() => {
    void refresh();
    void keyboardKeys()
      .then((keys) => {
        const bound = keys.find((binding) => binding.press === "voice listen_on");
        key = bound ? pretty(bound.chord) : null;
      })
      .catch(() => (key = null));
    const timer = setInterval(() => void refresh(), 250);
    return () => clearInterval(timer);
  });

  async function down(event: PointerEvent) {
    if (holding) return;
    holding = true;
    error = null;
    (event.currentTarget as HTMLElement).setPointerCapture?.(event.pointerId);
    try {
      await dispatch("voice listen_on");
    } catch (why) {
      error = String(why);
    }
    void refresh();
  }

  async function up() {
    if (!holding) return;
    holding = false;
    try {
      await dispatch("voice listen_off");
    } catch (why) {
      error = String(why);
    }
    void refresh();
  }

  async function choose(strip: number) {
    error = null;
    try {
      voice = await voiceStripSet(strip);
    } catch (why) {
      error = String(why);
    }
  }

  const mics = $derived(Array.from({ length: Math.max(inputs, (voice?.strip ?? 0) + 1) }, (_, i) => i));
  const listening = $derived(voice?.listening ?? false);
</script>

<div class="talk" data-listening={listening}>
  <button
    class="hold"
    class:on={listening}
    aria-pressed={listening}
    onpointerdown={down}
    onpointerup={up}
    onpointercancel={up}
    onlostpointercapture={up}
  >
    {listening ? "Listening…" : "Hold to talk"}
  </button>
  <label class="from">
    Commands from
    <select
      value={voice?.strip ?? 0}
      onchange={(e) => void choose(Number((e.currentTarget as HTMLSelectElement).value))}
      disabled={listening}
    >
      {#each mics as strip (strip)}
        <option value={strip}>Mic {strip + 1}</option>
      {/each}
    </select>
  </label>
  {#if key}
    <span class="key">or hold <kbd>{key}</kbd></span>
  {/if}
  <p class="said" aria-live="polite">
    {#if error}
      <span class="problem">{error}</span>
    {:else if listening && voice}
      Listening on Mic {voice.strip + 1}, out of the PA — {voice.seconds.toFixed(1)} s
    {:else if voice?.last?.problem}
      <span class="problem">{voice.last.problem}</span>
    {:else if voice?.hearing}
      Making out the words…
    {:else if voice?.said}
      <!-- What was heard before what was done: a mishearing and a wrong
           answer look the same until the words are shown. -->
      <span class="heard" data-source={voice.said.source}>
        {#if voice.said.heard}Heard “{voice.said.heard}”{/if}
        {#if voice.said.actions.length > 0}
          → <span class="done mono">{voice.said.actions.join(", ")}</span>
          {#if voice.said.source === "model"}<span class="by">(your AI)</span>{/if}
        {/if}
      </span>
      {#if voice.said.problem}
        <span class="problem">{voice.said.problem}</span>
      {/if}
      {#each voice.said.undelivered as refused (refused)}
        <span class="problem">Not done: {refused}</span>
      {/each}
    {:else if voice?.last}
      Took {voice.last.seconds.toFixed(1)} s from Mic {voice.strip + 1},
      {voice.last.voiced.toFixed(1)} s of it voice.
    {:else if inputs === 0}
      Open the microphones under Microphones to speak commands.
    {/if}
  </p>
</div>

<style>
  .talk {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.4rem 0.6rem;
    font-size: 0.8em;
  }

  .hold {
    touch-action: none;
    user-select: none;
    -webkit-user-select: none;
    min-width: 8.5rem;
  }

  /* Held: the one moment the DJ's voice is not in the room, so it shows. */
  .hold.on {
    background: var(--danger);
    border-color: var(--danger);
    color: var(--bg);
  }

  .from {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    color: var(--text-dim);
  }

  .key {
    color: var(--text-dim);
  }

  kbd {
    font-family: var(--mono, monospace);
    padding: 0 0.3rem;
    border: 1px solid var(--border);
    border-radius: 3px;
  }

  .said {
    flex-basis: 100%;
    margin: 0;
    min-height: 1.3em;
    color: var(--text-dim);
  }

  .problem {
    color: var(--warn);
  }

  .said .problem {
    display: block;
  }

  .done {
    color: var(--accent);
  }

  .by {
    font-size: 0.9em;
  }
</style>
