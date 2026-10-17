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
    vocalsState,
  } from "./api";
  import { breakAt, partOf } from "./lyricBreak";
  import { MIC_TOKENS, spread } from "./micColours";
  import { lyricsAt } from "./lyricsAt";
  import Visuals from "./Visuals.svelte";
  import RoomEmoji from "./RoomEmoji.svelte";
  import { isGuide } from "./guideOffer";

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
  /** B3: the singer on each microphone, by strip; `null` where nobody is. */
  let onMic = $state<(string | null)[]>([]);
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
        // B3: who is on which microphone, for a duet's colours.
        if ((vocals?.inputs ?? 0) > 0) {
          try {
            const on = (await vocalsState()).on.map((mic) => mic?.singer ?? null);
            if (on.join("\n") !== onMic.join("\n")) onMic = on;
            const theme = getComputedStyle(document.documentElement);
            const order = spread(MIC_TOKENS.map((token) => theme.getPropertyValue(token)));
            if (order.join() !== micOrder.join()) micOrder = order;
          } catch {
            // No microphones known: no duet colours.
          }
        } else if (onMic.length > 0) onMic = [];
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
  /**
   * B3: an instrumental break long enough to be told how long — a bar that
   * fills to the next line, with the seconds left.
   */
  const pause = $derived(words && deck ? breakAt(words.lines, deck.position_seconds) : null);
  /**
   * B3: a duet — two or more singers on the microphones. Each wears their
   * microphone's colour: their name, lit while their microphone hears them,
   * and a line that begins with their name ("Ana: …") wiped in it.
   */
  // The theme's own colours, worn in the order that keeps them apart
  // (`./micColours`): a theme's two accents can be two greens a singer
  // cannot tell apart across a room. Read again with who is on the mics.
  let micOrder = $state<number[]>([0, 1, 2, 3]);
  /** A microphone's level above which its singer is heard, not handled. */
  const HEARD = 0.12;
  const micColour = (strip: number) => `var(${MIC_TOKENS[micOrder[strip % micOrder.length] ?? 0]})`;
  const duet = $derived(onMic.filter((name) => name !== null).length >= 2);
  const part = $derived(current && duet ? partOf(current.text, onMic) : null);
  /** A timed line's first word is the name itself ("Ana:"): not sung, not drawn. */
  const named = $derived(
    part?.singer != null && current !== null && (current.words[0]?.[1] ?? "").trim().endsWith(":"),
  );
  /**
   * B7: the singer's deck playing with the guide under them, before their
   * first line — the intro and the count-in, and gone once they sing.
   */
  const guided = $derived(Boolean(deck?.playing && isGuide(deck.voice) && moment && moment.index < 0));
  const following = $derived(moment && words ? (words.lines[moment.index + 1] ?? null) : null);
  /** B3: the line after, a duet's part read ahead in its singer's colour. */
  const nextPart = $derived(following && duet ? partOf(following.text, onMic) : null);

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

  /**
   * Karaoke beta B8: a moment at *Sang* — the singer just marked as sung,
   * cheered for a few seconds in the screen's upper band, never over the
   * words. Only a change the screen sees happen: the song sung before it
   * opened is not cheered again.
   */
  const CHEER_MS = 4_500;
  let cheer = $state<{ singer: string; title: string; score: number | null; at: number } | null>(null);
  let sungSeen: string | null = null;
  let cheerGone: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    const last = rotation?.lately?.[0];
    if (!rotation) return;
    const key = last ? `${rotation.lately.length}\n${last.singer}\n${last.title}` : "";
    if (sungSeen === null) {
      sungSeen = key;
      return;
    }
    if (key === sungSeen || !last) {
      sungSeen = key;
      return;
    }
    sungSeen = key;
    cheer = { singer: last.singer, title: last.title, score: last.score ?? null, at: Date.now() };
    // Not the effect's cleanup: the rotation is asked for every few seconds,
    // and each answer would cancel it.
    clearTimeout(cheerGone);
    cheerGone = setTimeout(() => (cheer = null), CHEER_MS);
  });
  /** The confetti: a few pieces, each its own place, delay and colour. */
  const PIECES = Array.from({ length: 14 }, (_, i) => ({
    x: ((i * 37) % 100) - 50,
    delay: (i % 5) * 70,
    colour: ["var(--stem-vocal)", "var(--warn)", "var(--accent-2)", "var(--danger)"][i % 4],
    turn: ((i * 53) % 360) - 180,
  }));
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
        {#if q.prompt}
          <!-- Finish the line: the line the record has just sung; the answers are what follows. -->
          <p class="quiz-prompt" data-quiz-prompt>{q.prompt}…</p>
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
  {#if singer && !duet && !cheer}
    <!-- B8: who is singing, rising in when it is someone new. -->
    {#key singer}
      <p class="now-singing" data-now-singing>{singer}</p>
    {/key}
  {/if}
  {#if cheer}
    {#key cheer.at}
      <div class="cheer" data-cheer role="status" aria-live="polite">
        <p class="cheer-name">¡Bravo, {cheer.singer}!</p>
        <p class="cheer-song">
          {cheer.title}{cheer.score !== null ? ` · ${Math.round(cheer.score)}` : ""}
        </p>
        <span class="confetti" aria-hidden="true">
          {#each PIECES as piece, i (i)}
            <i
              style:--x="{piece.x}vw"
              style:--delay="{piece.delay}ms"
              style:--colour={piece.colour}
              style:--turn="{piece.turn}deg"
            ></i>
          {/each}
        </span>
      </div>
    {/key}
  {/if}

  {#if duet}
    <p class="duet" data-duet role="group" aria-label="Singing together">
      {#each onMic as name, strip (strip)}
        {#if name}
          <span
            class="duet-name"
            data-mic={strip + 1}
            class:hot={(vocals?.strips[strip]?.level ?? 0) > HEARD}
            style:--mic={micColour(strip)}>{name}</span
          >
        {/if}
      {/each}
    </p>
  {/if}

  <div class="words" aria-live="polite">
    {#if pause}
      <div
        class="break"
        data-break
        role="progressbar"
        aria-label="Instrumental break, {pause.left} seconds to the next line"
        aria-valuemin="0"
        aria-valuemax="100"
        aria-valuenow={Math.round(pause.progress * 100)}
      >
        <span class="break-track"><span class="break-fill" style:--done="{pause.progress * 100}%"></span></span>
        <span class="break-left" data-break-left>{pause.left}s</span>
      </div>
    {/if}
    {#if guided}
      <p class="guided" data-guided>You'll hear the original singer quietly underneath</p>
    {/if}
    {#if moment?.countIn != null}
      <!-- B8: drawn again on each beat, so the dots pulse with it. -->
      {#key moment.countIn}
        <p class="count" role="img" data-count-in={moment.countIn} aria-label="{moment.countIn} beats to go">
          {#each Array.from({ length: 4 }, (_, i) => i) as dot (dot)}
            <span class="dot" class:lit={dot < moment.countIn}></span>
          {/each}
        </p>
      {/key}
    {/if}
    {#if words && words.lines.length > 0}
      {#if current && current.text !== ""}
        <p
          class="line now"
          data-line="now"
          data-part={part?.singer != null ? part.singer + 1 : undefined}
          style:--sing={part?.singer != null ? micColour(part.singer) : undefined}
        >
          {#if current.words.length > 0}
            {#each current.words as [, word], i (i)}
              {#if !(named && i === 0)}
                <!-- B3: the word being sung glows softly; the wipe is still the fill. -->
                <span
                  class="word"
                  class:sung={i + 1 <= (moment?.words ?? 0)}
                  class:singing={i < (moment?.words ?? 0) && i + 1 > (moment?.words ?? 0)}
                  style:--fill="{Math.max(0, Math.min(1, (moment?.words ?? 0) - i)) * 100}%"
                  >{word}</span
                >{" "}
              {/if}
            {/each}
          {:else}
            <span class="wipe" style:--fill="{(moment?.progress ?? 0) * 100}%">{part?.text ?? current.text}</span>
          {/if}
        </p>
      {:else if current}
        <p class="line gap" data-line="now">♪</p>
      {/if}
      {#if following && following.text !== ""}
        <p
          class="line next"
          data-line="next"
          data-part={nextPart?.singer != null ? nextPart.singer + 1 : undefined}
          style:color={nextPart?.singer != null
            ? `color-mix(in srgb, ${micColour(nextPart.singer)} 70%, transparent)`
            : undefined}
        >
          {nextPart?.text ?? following.text}
        </p>
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
      <!-- B8: the next singer's name rises in when it changes. -->
      {#key next.name}
        <p class="up-next" data-up-next>Next: <strong>{next.name}</strong> — {next.song}</p>
      {/key}
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

  <!-- Karaoke beta B2: the room's emoji, small and in the corner. -->
  <RoomEmoji />
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
    background: linear-gradient(90deg, var(--sing, var(--stem-vocal)) var(--fill), rgb(245 245 245) var(--fill));
    -webkit-background-clip: text;
    background-clip: text;
    color: transparent;
  }

  .word.sung {
    --fill: 100%;
  }

  /*
    B3: the word being sung, glowing softly in the voice's colour. A filter,
    not a text-shadow: a shadow would paint over the wipe clipped to the
    letters.
  */
  .word.singing {
    filter: drop-shadow(0 0 1.2vh color-mix(in srgb, var(--sing, var(--stem-vocal)) 75%, transparent));
  }

  /* B3: how long an instrumental break lasts — a thin bar, under the words' place. */
  .break {
    display: flex;
    align-items: center;
    gap: 1.4vh;
  }

  .break-track {
    position: relative;
    width: min(42vw, 56vh);
    height: 1.1vh;
    border-radius: 1vh;
    background: rgb(255 255 255 / 0.14);
    overflow: hidden;
  }

  .break-fill {
    position: absolute;
    inset: 0 auto 0 0;
    width: var(--done);
    background: var(--stem-vocal);
  }

  .break-left {
    min-width: 4ch;
    font-size: 2.4vh;
    font-variant-numeric: tabular-nums;
    color: rgb(255 255 255 / 0.65);
  }

  /* B3: a duet's singers, each in their microphone's colour, lit while heard. */
  .duet {
    margin: 0;
    display: flex;
    gap: 2vw;
    justify-content: center;
    font-size: 2.6vh;
  }

  .duet-name {
    color: var(--mic);
    opacity: 0.55;
    padding-bottom: 0.3vh;
    border-bottom: 0.35vh solid transparent;
  }

  .duet-name.hot {
    opacity: 1;
    border-bottom-color: var(--mic);
  }

  .guided {
    margin: 0;
    font-size: 2.6vh;
    opacity: 0.75;
    color: var(--stem-vocal);
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
    animation: beat 360ms ease-out;
  }

  /*
    B8's motion: transforms and opacity only, once and short — on a machine
    with no GPU, motion is paid for by the whole page (ADR-0009), so nothing
    here loops. With reduced motion, a fade and nothing else.
  */
  @keyframes beat {
    from {
      transform: scale(1.35);
      box-shadow: 0 0 2.4vh color-mix(in srgb, var(--stem-vocal) 70%, transparent);
    }
    to {
      transform: scale(1);
      box-shadow: 0 0 0 transparent;
    }
  }

  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(1.4vh);
    }
    to {
      opacity: 1;
      transform: none;
    }
  }

  @keyframes fade {
    from {
      opacity: 0;
    }
  }

  .now-singing {
    margin: 0.6vh 0 0;
    font-size: 4vh;
    font-weight: 700;
    letter-spacing: 0.02em;
    color: var(--stem-vocal);
    animation: rise 700ms ease-out;
  }

  .up-next {
    animation: rise 600ms ease-out;
  }

  /* B8: at Sang, in the band above the words. */
  .cheer {
    position: absolute !important;
    left: 50%;
    top: 8.5vh;
    transform: translateX(-50%);
    width: min(80vw, 120vh);
    pointer-events: none;
    animation: cheer 4.5s ease-out forwards;
  }

  .cheer-name {
    margin: 0;
    font-size: 6vh;
    font-weight: 800;
    color: var(--stem-vocal);
    filter: drop-shadow(0 0.4vh 1vh rgb(0 0 0 / 0.8));
  }

  .cheer-song {
    margin: 0.4vh 0 0;
    font-size: 2.6vh;
    color: rgb(255 255 255 / 0.8);
  }

  .confetti {
    position: absolute;
    left: 50%;
    top: 3vh;
  }

  .confetti i {
    position: absolute;
    width: 1.3vh;
    height: 2.2vh;
    border-radius: 0.2vh;
    background: var(--colour);
    opacity: 0;
    animation: piece 1.6s ease-out var(--delay) forwards;
  }

  @keyframes cheer {
    0% {
      opacity: 0;
      transform: translateX(-50%) scale(0.92);
    }
    10% {
      opacity: 1;
      transform: translateX(-50%) scale(1);
    }
    80% {
      opacity: 1;
    }
    100% {
      opacity: 0;
    }
  }

  @keyframes piece {
    0% {
      opacity: 1;
      transform: translate(0, 0) rotate(0);
    }
    70% {
      opacity: 0.9;
    }
    100% {
      opacity: 0;
      transform: translate(var(--x), 12vh) rotate(var(--turn));
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .dot.lit {
      animation: none;
    }
    .now-singing,
    .up-next {
      animation: fade 400ms ease-out;
    }
    .confetti {
      display: none;
    }
    .cheer {
      animation: fade 400ms ease-out;
    }
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

  .quiz-prompt {
    margin: 0;
    font-size: 5.4vh;
    font-weight: 600;
    font-style: italic;
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
