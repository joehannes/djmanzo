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
   *
   * Over that background and under the veil that keeps the words legible
   * (§122), the look the DJ picked in the Singers panel, if any: djmanzo's
   * own layers, answering to the mix and the singers' microphones
   * (`./Visuals.svelte`). They are a canvas the words do not depend on — no
   * WebGL, a lost context or a picture that will not compile leaves K1's
   * background as it was.
   *
   * Under the words (K4): who is next and with what, the two after them by
   * name only, and a line of the host's own; the DJ's logo in a corner. All
   * below the words or beside the song's name, so none can cover a lyric.
   *
   * K7: while a quiz round runs the screen is the quiz's — the question in
   * the words' place, the answers lettered as on the phones, a countdown,
   * the answer with how many phones chose each, and between questions the
   * leaderboard and where to join. The record's name, words and cover are
   * not drawn while it does: on the room's display they would be the answer.
   */
  import {
    artUrl,
    hasBrandLogo,
    karaokeRotation,
    logoUrl,
    quizScreen,
    type QuizScreen,
    onBrandLogo,
    singerPhoto,
    lookChosen,
    looksCatalogue,
    singerBackdrop,
    singerLyrics,
    type AudioMetrics,
    type DeckState,
    type Look,
    type Rotation,
    type SingerBackdrop,
    type SingerLyrics,
    type VocalsState,
  } from "./api";
  import { lyricsAt } from "./lyricsAt";
  import Visuals from "./Visuals.svelte";

  let {
    decks,
    crossfader = 0,
    audio = { loudness: 0, bands: [0, 0, 0, 0] },
    vocals,
  }: {
    decks: DeckState[];
    crossfader?: number;
    audio?: AudioMetrics;
    vocals?: VocalsState;
  } = $props();

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
  /**
   * §122: the look behind the words, asked for as often: it is picked in the
   * Singers panel, in another window. The catalogue is asked again only when
   * the chosen name is not in it.
   */
  let catalogue: Look[] = [];
  let shown = $state<Look | null>(null);
  /** Step 6: the DJ's AI picks each record's look, over the one chosen. */
  let perSong = $state(false);
  $effect(() => {
    let alive = true;
    const look = async () => {
      while (alive) {
        try {
          rotation = await karaokeRotation();
        } catch {
          // A screen for singers says nothing rather than an error.
        }
        try {
          const chosen = await lookChosen();
          const name = chosen.singers;
          if (perSong !== chosen.ai) perSong = chosen.ai;
          if (name && !catalogue.some((l) => l.name === name)) catalogue = await looksCatalogue();
          const found = name ? (catalogue.find((l) => l.name === name) ?? null) : null;
          if (found?.name !== shown?.name) shown = found;
        } catch {
          // No look: K1's background alone, as before there were looks.
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
  const queue = $derived.by(() => {
    const waiting = (rotation?.singers ?? []).filter((s) => s.songs.length > 0);
    const singing = deck?.playing && (deck.voice ?? 1) < 0.99;
    return singing ? waiting.slice(1) : waiting;
  });
  const next = $derived(queue[0] ? { name: queue[0].name, song: queue[0].songs[0].title } : null);
  /** K4: the two after that, by name only — their songs are theirs to announce. */
  const then = $derived(queue.slice(1, 3).map((s) => s.name));

  /**
   * K4: who is singing — the top of the rotation while the singer's deck
   * plays with its vocal down — and their photo, if the host approved one.
   */
  const singer = $derived.by(() => {
    const waiting = (rotation?.singers ?? []).filter((s) => s.songs.length > 0);
    const singing = deck?.playing && (deck.voice ?? 1) < 0.99;
    return singing ? (waiting[0]?.name ?? null) : null;
  });
  let photo = $state<string | null>(null);
  $effect(() => {
    const name = singer;
    if (!name) {
      photo = null;
      return;
    }
    let alive = true;
    const ask = async () => {
      while (alive) {
        try {
          const url = await singerPhoto(name);
          if (alive && url !== photo) photo = url;
        } catch {
          // No photo is a screen as it was.
        }
        await new Promise((r) => setTimeout(r, 3_000));
      }
    };
    void ask();
    return () => {
      alive = false;
    };
  });

  /**
   * K7: the quiz, asked every second while it runs (the countdown), and every
   * three while it does not, so a round the host starts takes the screen.
   */
  let quiz = $state<QuizScreen | null>(null);
  const quizOn = $derived(quiz?.on ?? false);
  $effect(() => {
    let alive = true;
    const ask = async () => {
      while (alive) {
        try {
          quiz = await quizScreen();
        } catch {
          // No quiz is the screen as it was.
        }
        await new Promise((r) => setTimeout(r, quiz?.on ? 1_000 : 3_000));
      }
    };
    void ask();
    return () => {
      alive = false;
    };
  });
  const LETTERS = ["A", "B", "C", "D"] as const;
  const quizBetween = $derived(!!quiz?.on && (!quiz.question || quiz.question.right !== null));

  /** K4: the DJ's logo, when there is one; asked again when it changes. */
  let logo = $state<number | null>(null);
  $effect(() => {
    let version = 0;
    const ask = () =>
      void hasBrandLogo()
        .then((has) => (logo = has ? ++version : null))
        .catch(() => (logo = null));
    ask();
    const off = onBrandLogo(ask);
    return () => void off.then((unlisten) => unlisten()).catch(() => {});
  });
</script>

<div class="screen" data-singer-screen>
  <div class="backdrop" data-backdrop={quizOn ? "quiz" : (backdrop?.source ?? "none")} aria-hidden="true">
    {#each (quizOn ? [] : (backdrop?.shades ?? [])).slice(0, GLOWS.length) as shade, i (i)}
      <span
        class="glow"
        data-shade={shade.colour}
        style:background="radial-gradient(circle at {GLOWS[i]}, {shade.colour} 0%, transparent 55%)"
        style:opacity={0.2 + 0.4 * shade.weight}
      ></span>
    {/each}
    {#if !quizOn && backdrop && backdrop.source !== "sound" && !coverFailed}
      <img class="cover" src={artUrl(backdrop.track)} alt="" onerror={() => (coverFailed = true)} />
    {/if}
    {#if shown}
      <Visuals
        look={shown}
        {perSong}
        {decks}
        {crossfader}
        {audio}
        voices={(vocals?.strips ?? []).map((s) => s.level)}
        shades={backdrop?.shades ?? []}
      />
    {/if}
  </div>

  {#if quiz?.on}
    <!-- K7: the quiz, in the words' place; nothing of the record's own. -->
    <div class="quiz" data-quiz-screen aria-live="polite">
      {#if quiz.question}
        {@const q = quiz.question}
        <p class="quiz-head">Question {q.number} of {q.of} · <strong>{q.words}</strong></p>
        {#if q.right === null}
          {#if q.playing}
            <p class="quiz-state" data-quiz-state="listen">Listen…</p>
          {:else if q.closes_in !== null && q.closes_in > 0}
            <p class="quiz-state countdown" data-quiz-state="countdown">{Math.ceil(q.closes_in)}</p>
          {:else}
            <p class="quiz-state" data-quiz-state="closed">Time's up</p>
          {/if}
        {/if}
        <ol class="quiz-answers">
          {#each q.answers as answer, at (at)}
            <li data-right={q.right === at || undefined} data-wrong={(q.right !== null && q.right !== at) || undefined}>
              <span class="letter">{LETTERS[at]}</span>
              <span class="answer">{answer}</span>
              {#if q.counts}
                <span class="phones" data-phones-chose>{q.counts[at]} {q.counts[at] === 1 ? "phone" : "phones"}</span>
              {/if}
            </li>
          {/each}
        </ol>
      {:else}
        <p class="quiz-head"><strong>Quiz</strong></p>
        <p class="quiz-state">Get your phones out</p>
      {/if}
      {#if quizBetween}
        <div class="quiz-foot">
          {#if quiz.leaders.length > 0}
            <ol class="quiz-leaders" data-quiz-leaders>
              {#each quiz.leaders as [nickname, points], place (place)}
                <li><span class="place">{place + 1}</span> <span class="who">{nickname}</span> <strong>{points}</strong></li>
              {/each}
            </ol>
          {/if}
          {#if quiz.join}
            <div class="quiz-join" data-quiz-join>
              {#if quiz.join.qr}
                <!-- Rust's own square (dj_net::sticker), the one printed on the stickers. -->
                <span class="qr">{@html quiz.join.qr}</span>
              {/if}
              <p>Play along: <strong>{quiz.join.url}</strong></p>
            </div>
          {/if}
        </div>
      {/if}
    </div>
  {:else}
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
  {/if}

  <div class="foot">
    {#if next}
      <p class="up-next" data-up-next>Next: <strong>{next.name}</strong> — {next.song}</p>
    {/if}
    {#if then.length > 0}
      <p class="then" data-then>then {then.join(" · ")}</p>
    {/if}
    {#if rotation?.line}
      <p class="host-line" data-host-line>{rotation.line}</p>
    {/if}
    {#if rotation?.versus && !quizOn}
      <!-- K6: the round, two bars. -->
      <div class="versus" data-versus>
        {#each rotation.versus as team (team.name)}
          <div class="team">
            <span class="name">{team.name}</span>
            <span class="bar"><span class="fill" style:width="{team.mean ?? 0}%"></span></span>
            <strong>{team.mean === null ? "—" : Math.round(team.mean)}</strong>
          </div>
        {/each}
      </div>
    {/if}
    {#if quizOn}
      <!-- K7: the quiz's leaderboard is the one on the screen. -->
    {:else if singer && (rotation?.scores ?? []).some((s) => s.percent !== null)}
      <!-- K6: how the singing is going, small and under the words. -->
      <p class="scores" data-scores>
        {#each (rotation?.scores ?? []).filter((s) => s.percent !== null) as score (score.strip)}
          <span>{score.singer} <strong>{Math.round(score.percent ?? 0)}</strong></span>
        {/each}
      </p>
    {:else if !singer && (rotation?.best ?? []).length > 0}
      <!-- K6: between songs, tonight's best. -->
      <div class="best" data-best>
        <p class="best-title">Tonight's best</p>
        <ol>
          {#each rotation?.best ?? [] as sung, i (i)}
            <li><strong>{Math.round(sung.score ?? 0)}</strong> {sung.singer} — {sung.title}</li>
          {/each}
        </ol>
      </div>
    {/if}
  </div>

  {#if photo}
    <img class="singer-photo" src={photo} alt="" data-singer-photo />
  {/if}

  {#if logo !== null}
    <img class="logo" src={logoUrl(logo)} alt="" data-logo onerror={() => (logo = null)} />
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

  .foot {
    display: flex;
    flex-direction: column;
    gap: 0.6vh;
  }

  .then {
    margin: 0;
    font-size: 2.4vh;
    color: rgba(255, 255, 255, 0.65);
  }

  .host-line {
    margin: 0;
    font-size: 2.6vh;
    color: rgba(255, 255, 255, 0.9);
  }

  .versus {
    display: flex;
    flex-direction: column;
    gap: 0.6vh;
    width: min(60vw, 60vh * 1.6);
    margin: 0 auto;
    font-size: 2.4vh;
    color: rgba(255, 255, 255, 0.85);
  }

  .versus .team {
    display: grid;
    grid-template-columns: 12vw 1fr 5vh;
    align-items: center;
    gap: 1vw;
  }

  .versus .bar {
    height: 1.2vh;
    border-radius: 0.6vh;
    background: rgba(255, 255, 255, 0.15);
    overflow: hidden;
  }

  .versus .fill {
    display: block;
    height: 100%;
    background: var(--stem-vocal);
  }

  .scores {
    margin: 0;
    display: flex;
    gap: 2vw;
    font-size: 2.4vh;
    color: rgba(255, 255, 255, 0.8);
  }

  .best-title {
    margin: 0;
    font-size: 2.6vh;
    color: rgba(255, 255, 255, 0.9);
  }

  /* The numbers inside the lines: the footer is centred, and a marker
     outside a centred line lands at the screen's edge, far from its name. */
  .best ol {
    margin: 0.4vh 0 0;
    padding: 0;
    list-style-position: inside;
    font-size: 2.4vh;
    color: rgba(255, 255, 255, 0.8);
  }

  /* The singer's photo, in the corner under the words' height. */
  /* K7: the quiz, as large as the words it stands in for. */
  .quiz {
    flex: 1;
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 2vh;
    min-height: 0;
  }

  .quiz-head {
    margin: 0;
    font-size: 3.4vh;
    opacity: 0.85;
  }

  /* One height for "Listen…", the countdown and "Time's up", so the answers
     do not jump as it changes. */
  .quiz-state {
    margin: 0;
    height: 11vh;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 5vh;
    font-weight: 600;
  }

  .quiz-state.countdown {
    font-size: 9vh;
    font-variant-numeric: tabular-nums;
  }

  .quiz-answers {
    list-style: none;
    margin: 0 auto;
    padding: 0;
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1.6vh 3vw;
    width: min(100%, 120vh);
    text-align: left;
  }

  .quiz-answers li {
    display: flex;
    align-items: center;
    gap: 1.6vh;
    padding: 1.6vh 2vh;
    border-radius: 1.2vh;
    background: rgb(255 255 255 / 0.08);
    font-size: 3.6vh;
    line-height: 1.2;
  }

  .quiz-answers .letter {
    font-weight: 800;
    font-size: 4.4vh;
    min-width: 4.4vh;
    color: rgb(130 185 255);
  }

  .quiz-answers .answer {
    flex: 1;
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .quiz-answers .phones {
    font-size: 3vh;
    opacity: 0.8;
    font-variant-numeric: tabular-nums;
  }

  .quiz-answers li[data-right] {
    background: rgb(60 190 110 / 0.35);
    outline: 0.4vh solid rgb(110 230 150);
  }

  .quiz-answers li[data-wrong] {
    opacity: 0.45;
  }

  .quiz-foot {
    display: flex;
    justify-content: center;
    align-items: center;
    gap: 5vw;
  }

  .quiz-leaders {
    list-style: none;
    margin: 0;
    padding: 0;
    font-size: 2.8vh;
    text-align: left;
    columns: 2;
    column-gap: 4vw;
  }

  .quiz-leaders .place {
    display: inline-block;
    min-width: 3vh;
    opacity: 0.7;
  }

  .quiz-join {
    display: flex;
    align-items: center;
    gap: 2vh;
    font-size: 2.6vh;
  }

  .quiz-join p {
    margin: 0;
  }

  .quiz-join .qr {
    display: block;
    width: 16vh;
    height: 16vh;
    padding: 1vh;
    background: white;
    border-radius: 1vh;
  }

  .quiz-join .qr :global(svg) {
    width: 100%;
    height: 100%;
  }

  .screen > .singer-photo {
    position: absolute;
    left: 3vw;
    bottom: 3vh;
    max-height: 22vh;
    max-width: 22vw;
    object-fit: cover;
    border-radius: 1vh;
    box-shadow: 0 0 2vh rgba(0, 0, 0, 0.6);
  }

  /* In the corner, above nothing: the words are in the middle. */
  .screen > .logo {
    position: absolute;
    top: 3vh;
    right: 3vw;
    max-height: 8vh;
    max-width: 14vw;
    object-fit: contain;
    opacity: 0.9;
  }
</style>
