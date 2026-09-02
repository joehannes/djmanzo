<script lang="ts">
  /**
   * Finding a record from what you remember of it.
   *
   * # Why three boxes and one list
   *
   * People do not remember a record one way. They remember half a line, a
   * rough idea of the sound, and a tempo their hands know — and any one of
   * those alone is usually not enough. So the three ways in sit together and
   * feed one list, and using two of them is the normal case rather than the
   * clever case.
   *
   * # What each one can actually do, said where it is used
   *
   * - **Words** searches lyrics djmanzo has fetched for *your* collection. It
   *   cannot find a record you have never had, and it needs the words fetched
   *   first, which is why the progress and the fetch button are right there
   *   rather than buried in settings.
   * - **A description** is the only one that can name a record you do not own,
   *   because it asks the assistant. It costs a model call, and it can be
   *   confidently wrong — so every guess shows its reason, and whether you
   *   already own it.
   * - **A hum** narrows. It does not identify. Recognising a recording from a
   *   hum needs a licensed fingerprint service with millions of reference
   *   melodies; djmanzo has none and says so here rather than getting it right
   *   one time in five and letting you find out during a set.
   */
  import {
    guessFromDescription,
    hum as sendHum,
    melodyProgress,
    melodySweep,
    wordsFetch,
    wordsProgress,
    wordsSearch,
    type Guess,
    type Hummed,
    type MelodyProgress,
    type WordHit,
    type WordsProgress,
  } from "./api";
  import { onMount } from "svelte";

  interface Props {
    enabled: boolean;
    /** Hand a name to the search box, which is the next gesture. */
    onFind?: (text: string) => void;
  }

  let { enabled, onFind }: Props = $props();

  /**
   * The rate a hum is sent at.
   *
   * 22 050 Hz. A hummed voice lives under 1 kHz and its harmonics under 5;
   * half the usual rate carries all of that and halves what crosses the
   * interface boundary, which for twelve seconds is the difference between a
   * noticeable pause and none.
   */
  const HUM_RATE = 22_050;
  /** How long a hum is recorded for before it is read. */
  const HUM_SECONDS = 8;

  let phrase = $state("");
  let hits = $state<WordHit[]>([]);
  let progress = $state<WordsProgress | null>(null);
  let fetching = $state(false);
  let fetchNote = $state("");

  let description = $state("");
  let guesses = $state<Guess[]>([]);
  let guessing = $state(false);

  let humming = $state(false);
  /** Whether a batch of contours is being made right now. */
  let sweeping = $state(false);
  /** What the last batch did, so pressing it again is an informed choice. */
  let sweptNote = $state("");
  /**
   * How much of the collection can be searched by tune.
   *
   * Read when the panel opens rather than taken from a hum result, because it
   * is the thing to know *before* humming: an empty contour index makes the
   * melody search return nothing, and finding that out by humming first wastes
   * the one gesture this feature asks for.
   */
  let tunes = $state<MelodyProgress | null>(null);
  let heard = $state<Hummed | null>(null);
  let humNote = $state("");

  let error = $state("");

  async function refreshProgress() {
    try {
      progress = await wordsProgress();
    } catch (e) {
      error = String(e);
    }
  }

  async function refreshTunes() {
    try {
      tunes = await melodyProgress();
    } catch (e) {
      error = String(e);
    }
  }

  onMount(() => {
    if (enabled) {
      void refreshProgress();
      void refreshTunes();
    }
  });

  async function search() {
    try {
      hits = await wordsSearch(phrase);
      error = "";
    } catch (e) {
      error = String(e);
    }
  }

  /**
   * Fetch words for records that have none, a batch at a time.
   *
   * Kept batched and stoppable rather than run to completion: a collection of
   * ten thousand is ten thousand requests, and a DJ who started it by accident
   * should be able to stop without killing the application.
   */
  async function fetchWords() {
    fetching = true;
    fetchNote = "";
    try {
      while (fetching) {
        const sweep = await wordsFetch();
        await refreshProgress();
        if (sweep.gave_up) {
          fetchNote = "The lyrics database could not be reached. Stopped here.";
          break;
        }
        if (sweep.asked === 0) {
          fetchNote = "Every record has been asked about.";
          break;
        }
        fetchNote = `${sweep.left} records still to ask about.`;
      }
    } catch (e) {
      error = String(e);
    } finally {
      fetching = false;
    }
  }

  async function askAssistant() {
    guessing = true;
    try {
      guesses = await guessFromDescription(description);
      error = "";
    } catch (e) {
      error = String(e);
    } finally {
      guessing = false;
    }
  }

  /**
   * Make pitch contours for a batch of records that have none.
   *
   * Started by hand rather than on a timer: it decodes every file it touches,
   * which is not something to have happening in the background at a gig
   * without being asked for.
   */
  async function sweep() {
    sweeping = true;
    sweptNote = "";
    try {
      const done = await melodySweep();
      await refreshTunes();
      sweptNote =
        done.left > 0
          ? `Read ${done.asked}, ${done.left} to go. Press again to carry on.`
          : `Read ${done.asked}. Every record has a contour now.`;
    } catch (problem) {
      sweptNote = `Could not read them: ${problem}`;
    } finally {
      sweeping = false;
    }
  }

  /**
   * Record a hum and read it.
   *
   * The audio never leaves this function as audio: it is decimated to mono at
   * {@link HUM_RATE} and handed to djmanzo's own analysis, which returns a key
   * and a tempo. Nothing is stored and nothing is sent anywhere.
   */
  async function listen() {
    humming = true;
    humNote = "";
    heard = null;
    let stream: MediaStream | null = null;
    let context: AudioContext | null = null;
    try {
      stream = await navigator.mediaDevices.getUserMedia({ audio: true });
      context = new AudioContext({ sampleRate: HUM_RATE });
      const source = context.createMediaStreamSource(stream);
      const collected: Float32Array[] = [];
      // A ScriptProcessor rather than an AudioWorklet: a worklet needs its own
      // module file served separately, and this runs for eight seconds once,
      // not for the length of a set. Deprecated and entirely adequate here.
      const tap = context.createScriptProcessor(4096, 1, 1);
      tap.onaudioprocess = (event) => {
        collected.push(new Float32Array(event.inputBuffer.getChannelData(0)));
      };
      source.connect(tap);
      // Connected to the destination because some browsers will not run a
      // processor that goes nowhere — with a gain of zero, so the DJ does not
      // hear themselves through the booth.
      const silence = context.createGain();
      silence.gain.value = 0;
      tap.connect(silence);
      silence.connect(context.destination);

      await new Promise((done) => setTimeout(done, HUM_SECONDS * 1000));

      const total = collected.reduce((sum, part) => sum + part.length, 0);
      const samples = new Float32Array(total);
      let at = 0;
      for (const part of collected) {
        samples.set(part, at);
        at += part.length;
      }
      heard = await sendHum(Array.from(samples), context.sampleRate);
      if (heard.near.length === 0 && heard.tempo === null) {
        humNote =
          "No steady tempo came through. Humming with a bit of rhythm to it " +
          "gives djmanzo something to measure.";
      }
      error = "";
    } catch (e) {
      error = String(e);
    } finally {
      stream?.getTracks().forEach((track) => track.stop());
      void context?.close();
      humming = false;
    }
  }

  function name(track: { title: string; artist: string | null }): string {
    return track.artist ? `${track.artist} - ${track.title}` : track.title;
  }

  /** "1 record" rather than "1 records". A collection of one is a real case. */
  function records(count: number): string {
    return count === 1 ? "1 record" : `${count} records`;
  }

  let asked = $derived(progress ? progress.tracks - progress.asked : 0);
</script>

<div class="memory">
  {#if error}
    <p class="error">{error}</p>
  {/if}

  <!-- Words --------------------------------------------------------------- -->
  <section>
    <h3>A line you remember</h3>
    <div class="row">
      <input
        bind:value={phrase}
        placeholder="no puedo dormir"
        onkeydown={(e) => e.key === "Enter" && search()}
        aria-label="Words you remember"
      />
      <button onclick={search}>Search</button>
    </div>
    {#if progress}
      <p class="note">
        {progress.with_words} of {records(progress.tracks)} have their words.
        {#if asked > 0}
          {records(asked)}
          {asked === 1 ? "has" : "have"} never been asked about.
          <button class="quiet" onclick={fetchWords} disabled={fetching}>
            {fetching ? "Fetching…" : "Fetch them"}
          </button>
          {#if fetching}
            <button class="quiet" onclick={() => (fetching = false)}>Stop</button>
          {/if}
        {/if}
      </p>
      {#if fetchNote}<p class="note">{fetchNote}</p>{/if}
    {/if}
    {#if hits.length > 0}
      <ul class="hits">
        {#each hits as hit (hit.track.id)}
          <li>
            <span class="what">{name(hit.track)}</span>
            <!--
              The line as the record has it, not as it was typed: seeing the
              accent and the punctuation back is how you recognise it.
            -->
            <span class="line">“{hit.line}”</span>
            <button class="find" onclick={() => onFind?.(name(hit.track))}>
              Find it
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </section>

  <!-- A description ------------------------------------------------------- -->
  <section>
    <h3>Roughly what it was like</h3>
    <textarea
      bind:value={description}
      rows="2"
      placeholder="bachata with a piano hook, sounds like Aventura, heard it at a beach bar last summer"
      aria-label="A description of the record"
    ></textarea>
    <div class="row">
      <button onclick={askAssistant} disabled={guessing || !description.trim()}>
        {guessing ? "Thinking…" : "Ask the assistant"}
      </button>
      <span class="note">Costs a model call. It can be confidently wrong.</span>
    </div>
    {#if guesses.length > 0}
      <ul class="hits">
        {#each guesses as guess (guess.artist + guess.title)}
          <li>
            <span class="what">{guess.artist} — {guess.title}</span>
            {#if guess.why}<span class="line">{guess.why}</span>{/if}
            {#if guess.owned}
              <span class="owned">in your collection</span>
              <button
                class="find"
                onclick={() => onFind?.(name(guess.owned!))}
              >
                Find it
              </button>
            {:else}
              <span class="note">not in your collection</span>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  </section>

  <!-- A hum --------------------------------------------------------------- -->
  <section>
    <h3>Hum it</h3>
    <!--
      The contour count comes first, because it is the precondition. Humming
      at an empty index returns nothing and looks like a broken feature; the
      same mistake the lyrics section avoids by putting its progress and its
      fetch button above the search rather than below it.
    -->
    {#if tunes}
      <div class="row">
        <button onclick={sweep} disabled={sweeping}>
          {sweeping ? "Reading…" : "Read more melodies"}
        </button>
        <span class="note">
          {tunes.with_melody} of {tunes.tracks} records have a pitch contour.
          {#if tunes.with_melody < tunes.tracks}
            Only those can be matched by tune.
          {/if}
        </span>
      </div>
      {#if sweptNote}<p class="note">{sweptNote}</p>{/if}
    {/if}

    <div class="row">
      <button onclick={listen} disabled={humming}>
        {humming ? `Listening for ${HUM_SECONDS}s…` : "Hum it"}
      </button>
      <!--
        Said next to the button rather than in a help panel, because this is
        exactly where somebody forms the wrong expectation.
      -->
      <span class="note">
        This searches your own collection — by the tune, and by key and tempo.
        It does not name a record you do not own: that needs a licensed
        fingerprint service djmanzo does not have.
      </span>
    </div>
    {#if heard}
      <p class="note">
        Heard {heard.seconds.toFixed(1)}s:
        {heard.key ?? "no clear key"} ·
        {heard.tempo ? `${heard.tempo.toFixed(1)} BPM` : "no steady tempo"}
        {#if heard.voiced < 0.3}
          · <span class="warn">mostly breath — hum a note rather than
          whispering it</span>
        {/if}
      </p>

      <!--
        The tune first, because it is the answer to the question actually
        asked. The key-and-tempo list below is every record it *could* be; this
        is the ones that sound like it.
      -->
      {#if heard.melody.length > 0}
        <h4>Sounds like the tune</h4>
        <ul class="hits">
          {#each heard.melody as hit (hit.track.id)}
            <li>
              <span class="what">{name(hit.track)}</span>
              <!--
                Where in the record, because a match halfway through a six
                minute track is a thing you want to go and hear rather than
                hunt for.
              -->
              <span class="note mono">
                {Math.floor(hit.at_seconds / 60)}:{String(
                  Math.floor(hit.at_seconds % 60),
                ).padStart(2, "0")}
              </span>
              <button class="find" onclick={() => onFind?.(name(hit.track))}>
                Find it
              </button>
            </li>
          {/each}
        </ul>
      {:else if tunes && tunes.with_melody === 0}
        <!--
          The difference between "nothing matched" and "nothing was searched"
          is the whole of whether this feature works, so it is said rather
          than left to be inferred from an empty list.
        -->
        <p class="note">
          No record has a pitch contour yet, so there was nothing to compare
          the tune against. Reading them takes a decode each and is not
          something to start in the middle of a set.
        </p>
      {:else}
        <p class="note">
          Nothing in the {tunes?.with_melody ?? 0} records with a contour sounds
          like that.
        </p>
      {/if}

      {#if heard.near.length > 0}
        <h4>Near that key and tempo</h4>
        <ul class="hits">
          {#each heard.near as track (track.id)}
            <li>
              <span class="what">{name(track)}</span>
              <button class="find" onclick={() => onFind?.(name(track))}>
                Find it
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    {/if}
    {#if humNote}<p class="note">{humNote}</p>{/if}
  </section>
</div>

<style>
  /* A heading inside a section, for the two lists a hum comes back with. The
     tune and the key-and-tempo shortlist are different answers, and an
     unlabelled second list reads as more of the first one. */
  h4 {
    margin: 0.6rem 0 0.2rem;
    font-size: 0.78em;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-dim);
  }

  .warn {
    color: var(--warn);
  }

  .memory {
    display: flex;
    flex-direction: column;
    gap: 0.9rem;
    flex: 1;
    min-height: 0;
    overflow: auto;
  }

  section {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  h3 {
    margin: 0;
    font-size: 0.8em;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-dim);
  }

  .row {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    flex-wrap: wrap;
  }

  .row input {
    flex: 1;
    min-width: 10rem;
  }

  textarea {
    width: 100%;
    resize: vertical;
    font: inherit;
    font-size: 0.85em;
    line-height: 1.45;
  }

  .hits {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }

  .hits li {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.35rem 0.5rem;
    border: 1px solid var(--border);
    border-radius: 6px;
    flex-wrap: wrap;
  }

  .what {
    font-size: 0.88em;
  }

  .line {
    flex: 1;
    min-width: 8rem;
    font-size: 0.82em;
    color: var(--text-dim);
    font-style: italic;
  }

  .owned {
    font-size: 0.78em;
    color: var(--accent);
  }

  .find {
    font-size: 0.78em;
    padding: 0.1rem 0.4rem;
  }

  .note,
  .error {
    margin: 0;
    font-size: 0.78em;
    line-height: 1.5;
    color: var(--text-dim);
  }

  .error {
    color: var(--danger);
  }
</style>
