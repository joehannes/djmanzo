<script lang="ts">
  /**
   * §107: the singers' screen — the words, for a screen facing the microphone.
   *
   * What a karaoke host puts on the second display: the line being sung,
   * large, wiped as it is sung; the line after it, smaller, so a singer can
   * read ahead; a count-in before the first line and after a break; and who
   * is up next, so the next singer is at the stage when the song ends.
   * Nothing else — no decks, no waveform. A singer is reading from across a
   * room with a microphone in one hand.
   *
   * The words are Rust's (`singer_lyrics`, parsed by `dj_library::lrc`), the
   * moment is the deck's playhead, and the arithmetic between them is
   * `./lyricsAt`, tested there.
   *
   * Behind them (K1), `singer_backdrop`'s choice: the record's own cover, the
   * Cover Art Archive's, or the record's own colours — which are drawn
   * first, always, so the screen is never blank while a cover loads or when
   * one fails. Plain CSS over black: nothing the words wait on, nothing a
   * graphics driver can take down with them.
   */
  import {
    artUrl,
    karaokeRotation,
    singerBackdrop,
    singerLyrics,
    type DeckState,
    type Rotation,
    type SingerBackdrop,
    type SingerLyrics,
  } from "./api";
  import { lyricsAt } from "./lyricsAt";

  let { decks }: { decks: DeckState[] } = $props();

  /**
   * The deck a singer is singing to: the one playing with its vocal taken out
   * or down, then one merely set up for a singer, then whatever is playing.
   */
  const deck = $derived(
    decks.find((d) => d.playing && (d.voice ?? 1) < 0.99) ??
      decks.find((d) => d.loaded && (d.voice ?? 1) < 0.99) ??
      decks.find((d) => d.playing) ??
      decks.find((d) => d.loaded) ??
      null,
  );

  let words = $state<SingerLyrics | null>(null);
  let backdrop = $state<SingerBackdrop | null>(null);
  /** The cover would not load: the colours alone. */
  let coverFailed = $state(false);
  let asked = "";
  $effect(() => {
    // Asked again when the record on the deck changes, not every frame.
    const key = deck ? `${deck.number}/${deck.title}/${deck.length_frames}` : "";
    if (key === asked) return;
    asked = key;
    words = null;
    if (!deck) return;
    void singerLyrics(deck.number)
      .then((found) => (words = found))
      .catch(() => (words = null));
  });

  /**
   * A record's colours are measured seconds after it loads, so a screen
   * opened with the record (or a record loaded under an open screen) is told
   * they are pending, and asks again until they land — for a minute at most,
   * since a record silent throughout never has any.
   */
  const RECHECK_MS = 3_000;
  const MOST_RECHECKS = 20;
  /** Only ever counts up: each step is one more ask. */
  let recheck = $state(0);
  let backdropRecord = "";
  let backdropAsked = "";
  let rechecks = 0;
  $effect(() => {
    const record = deck ? `${deck.number}/${deck.title}/${deck.length_frames}` : "";
    const key = `${record}#${recheck}`;
    if (key === backdropAsked) return;
    backdropAsked = key;
    if (record !== backdropRecord) {
      backdropRecord = record;
      backdrop = null;
      coverFailed = false;
      rechecks = 0;
    }
    if (!deck) return;
    void singerBackdrop(deck.number)
      .then((found) => {
        // An answer for a record since replaced — the archive can take
        // seconds — is not this one's background.
        if (backdropAsked !== key) return;
        backdrop = found;
        if (found.pending && rechecks < MOST_RECHECKS) {
          rechecks += 1;
          setTimeout(() => {
            if (backdropAsked === key) recheck += 1;
          }, RECHECK_MS);
        }
      })
      .catch(() => {});
  });

  /** Where each of the record's colours glows from. */
  const GLOWS = ["22% 28%", "78% 24%", "30% 78%", "74% 72%"];

  /** The rotation, looked at every few seconds: it changes when a host marks a song sung. */
  let rotation = $state<Rotation | null>(null);
  $effect(() => {
    let alive = true;
    const look = async () => {
      while (alive) {
        try {
          rotation = await karaokeRotation();
        } catch {
          // A screen for singers says nothing rather than an error.
        }
        await new Promise((r) => setTimeout(r, 3_000));
      }
    };
    void look();
    return () => {
      alive = false;
    };
  });

  const moment = $derived(
    words && deck ? lyricsAt(words.lines, deck.position_seconds, deck.effective_bpm ?? null) : null,
  );
  const current = $derived(moment && words && moment.index >= 0 ? words.lines[moment.index] : null);
  const following = $derived(moment && words ? (words.lines[moment.index + 1] ?? null) : null);

  /**
   * Who is next. While a song plays its singer is still at the top of the
   * rotation — a host marks it sung when it ends — so next is the one after.
   */
  const next = $derived.by(() => {
    const waiting = (rotation?.singers ?? []).filter((s) => s.songs.length > 0);
    const singing = deck?.playing && (deck.voice ?? 1) < 0.99;
    const person = singing ? waiting[1] : waiting[0];
    return person ? { name: person.name, song: person.songs[0].title } : null;
  });
</script>

<div class="screen" data-singer-screen>
  <div class="backdrop" data-backdrop={backdrop?.source ?? "none"} aria-hidden="true">
    {#each (backdrop?.shades ?? []).slice(0, GLOWS.length) as shade, i (i)}
      <span
        class="glow"
        data-shade={shade.colour}
        style:background="radial-gradient(circle at {GLOWS[i]}, {shade.colour} 0%, transparent 55%)"
        style:opacity={0.2 + 0.4 * shade.weight}
      ></span>
    {/each}
    {#if backdrop && backdrop.source !== "sound" && !coverFailed}
      <img class="cover" src={artUrl(backdrop.track)} alt="" onerror={() => (coverFailed = true)} />
    {/if}
  </div>

  {#if deck?.title}
    <p class="song">{deck.title}{deck.artist ? ` — ${deck.artist}` : ""}</p>
  {/if}

  <div class="words" aria-live="polite">
    {#if moment?.countIn != null}
      <p class="count" role="img" data-count-in={moment.countIn} aria-label="{moment.countIn} beats to go">
        {#each Array.from({ length: 4 }, (_, i) => i) as dot (dot)}
          <span class="dot" class:lit={dot < moment.countIn}></span>
        {/each}
      </p>
    {/if}
    {#if words && words.lines.length > 0}
      {#if current && current.text !== ""}
        <p class="line now" data-line="now">
          {#if current.words.length > 0}
            {#each current.words as [, word], i (i)}
              <span
                class="word"
                class:sung={i + 1 <= (moment?.words ?? 0)}
                style:--fill="{Math.max(0, Math.min(1, (moment?.words ?? 0) - i)) * 100}%"
                >{word}</span
              >{" "}
            {/each}
          {:else}
            <span class="wipe" style:--fill="{(moment?.progress ?? 0) * 100}%">{current.text}</span>
          {/if}
        </p>
      {:else if current}
        <p class="line gap" data-line="now">♪</p>
      {/if}
      {#if following && following.text !== ""}
        <p class="line next" data-line="next">{following.text}</p>
      {/if}
    {:else if words && words.plain.length > 0}
      <!-- No times for these words: shown still, because a wipe at a guessed pace leads a singer wrong. -->
      <div class="still" data-line="still">
        {#each words.plain as text, i (i)}
          <p>{text}</p>
        {/each}
      </div>
    {:else if words?.instrumental}
      <p class="line gap">♪ An instrumental ♪</p>
    {:else if deck}
      <p class="none">No words for this song yet.</p>
    {/if}
  </div>

  {#if next}
    <p class="up-next" data-up-next>Next: <strong>{next.name}</strong> — {next.song}</p>
  {/if}
</div>

<style>
  .screen {
    position: fixed;
    inset: 0;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    padding: 4vh 5vw;
    background: rgb(0 0 0);
    color: rgb(245 245 245);
    font-family: inherit;
    text-align: center;
  }

  /* Everything but the background sits above it. */
  .screen > :not(.backdrop) {
    position: relative;
    z-index: 1;
  }

  .backdrop {
    position: absolute;
    inset: 0;
    overflow: hidden;
    z-index: 0;
  }

  .glow {
    position: absolute;
    inset: -10%;
    animation: drift 40s ease-in-out infinite alternate;
  }

  .glow:nth-child(2) {
    animation-duration: 53s;
    animation-direction: alternate-reverse;
  }

  .glow:nth-child(3) {
    animation-duration: 47s;
  }

  /* The cover, faded and softened: something to look at between lines, never
     something to read past. */
  .cover {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    opacity: 0.32;
    filter: blur(0.6vh) saturate(1.1);
  }

  /* A veil over both, darkest where the words are, so a pale cover or a
     bright record never takes the contrast a singer across the room reads
     by. */
  .backdrop::after {
    content: "";
    position: absolute;
    inset: 0;
    background: radial-gradient(ellipse at 50% 50%, rgb(0 0 0 / 0.72) 0%, rgb(0 0 0 / 0.5) 75%);
  }

  @keyframes drift {
    from {
      transform: translate(-3%, -2%) scale(1);
    }
    to {
      transform: translate(3%, 2%) scale(1.08);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .glow {
      animation: none;
    }
  }

  .song {
    margin: 0;
    font-size: 2.2vh;
    color: rgba(255, 255, 255, 0.6);
  }

  .words {
    display: flex;
    flex-direction: column;
    justify-content: center;
    align-items: center;
    gap: 3vh;
    flex: 1;
  }

  .line {
    margin: 0;
    max-width: 90vw;
    line-height: 1.2;
  }

  .now {
    font-size: 7vh;
    font-weight: 700;
    /* Legible over any cover. */
    filter: drop-shadow(0 0.3vh 0.6vh rgb(0 0 0 / 0.8));
  }

  .next {
    font-size: 4.2vh;
    color: rgba(255, 255, 255, 0.55);
  }

  .gap {
    font-size: 6vh;
    color: rgba(255, 255, 255, 0.45);
  }

  /*
    The wipe: the sung part in the voice's own colour — the one the vocal
    fader and the vocal stem wear — the rest white. A gradient clipped to the letters, so it follows the words
    exactly and costs one repaint a snapshot.
  */
  .wipe,
  .word {
    background: linear-gradient(90deg, var(--stem-vocal) var(--fill), rgb(245 245 245) var(--fill));
    -webkit-background-clip: text;
    background-clip: text;
    color: transparent;
  }

  .word.sung {
    --fill: 100%;
  }

  .count {
    display: flex;
    gap: 2vw;
    margin: 0;
  }

  .dot {
    width: 3vh;
    height: 3vh;
    border-radius: 50%;
    border: 0.4vh solid var(--stem-vocal);
  }

  .dot.lit {
    background: var(--stem-vocal);
  }

  .still {
    max-height: 70vh;
    overflow: hidden;
    font-size: 3.6vh;
    line-height: 1.35;
  }

  .still p {
    margin: 0;
  }

  .none {
    font-size: 3vh;
    color: rgba(255, 255, 255, 0.45);
  }

  .up-next {
    margin: 0;
    font-size: 3vh;
    color: rgba(255, 255, 255, 0.8);
  }

  .up-next strong {
    color: var(--stem-vocal);
  }
</style>
