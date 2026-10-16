<script lang="ts">
  import { carriesTrack, droppedTrack } from "./trackdrag";
  /**
   * §107: the singer rotation, as a karaoke host works it.
   *
   * The rules — first come, first served; one song a turn; not ready is the
   * bottom of the list; a singer's key remembered with their song — are
   * `dj_app::karaoke`'s, and so is every change: this draws the rotation and
   * reports a press. What is here rather than there is the one thing only the
   * interface can do: put the up-next song on a deck, in the singer's key.
   */
  import BuyLinks from "./BuyLinks.svelte";
  import Guests from "./Guests.svelte";
  import Microphones from "./Microphones.svelte";
  import LookByAi from "./LookByAi.svelte";
  import MilkdropPresets from "./MilkdropPresets.svelte";
  import Packs from "./Packs.svelte";
  import WordTiming from "./WordTiming.svelte";
  import { validate } from "./looks";
  import { rangeWords } from "./notes";
  import {
    detachPanel,
    dispatch,
    karaokeAsk,
    karaokeBreaks,
    karaokeBreaksSet,
    karaokeClear,
    karaokeKey,
    karaokeLeave,
    karaokeLine,
    karaokeBringIn,
    karaokeFits,
    karaokeSongsFor,
    type RangeFit,
    type SongFor,
    karaokeScoring,
    karaokeVerdict,
    karaokeVersus,
    vocalLine,
    type VocalLine,
    quiz as readQuiz,
    quizAgain,
    quizEnd,
    quizNext,
    quizReveal,
    quizSet,
    type Quiz,
    type QuizKind,
    karaokeMove,
    karaokeNotHere,
    karaokeRefuse,
    karaokeRotation,
    karaokeSang,
    karaokeTake,
    librarySearch,
    karaokeLoad,
    karaokeRecordSong,
    karaokeRecordStop,
    singPhotoApprove,
    singPhotoRefuse,
    singPhotos,
    singRules,
    singRulesSet,
    lookChoose,
    lookChosen,
    looksCatalogue,
    vocalsState,
    vocalStripSinger,
    type Breaks,
    type DeckState,
    type LibraryTrack,
    type Look,
    type Rotation,
    type SingPhoto,
    type SingRules,
    type SingerRequest,
    type VoiceTag,
    type Vocals,
    type VocalsState,
  } from "./api";

  let {
    enabled,
    deckCount = 2,
    decks = [],
    vocals,
    buffer,
  }: {
    enabled: boolean;
    deckCount?: number;
    decks?: DeckState[];
    vocals?: VocalsState;
    /** K3: the interface's buffer, and how to change it — not in a popped-out window. */
    buffer?: { frames: number; set: (frames: number) => Promise<void> };
  } = $props();

  /**
   * §107: what the room hears of the recorded singer, per deck. Out is
   * karaoke; a guide — a quarter, about -12 dB — is the vocal a host leaves
   * under a nervous singer or a song nobody quite remembers. It is the deck's
   * own setting and stays set, so every song loaded onto a karaoke deck plays
   * the way the host chose.
   */
  const VOICES = [
    { level: 1, label: "As recorded" },
    { level: 0.25, label: "Guide" },
    { level: 0, label: "Out" },
  ] as const;

  /** K4: a singer's choice for the record's voice, in the host's words. The
   * level each sets on the deck is Rust's (`commands::load_up_next`). */
  const VOICE_WORD: Record<VoiceTag, string> = { "as-recorded": "as recorded", guide: "guide", out: "voice out" };

  /** K4: what a singer asked for, as the host reads it: "asks −2 · guide". */
  function asked(song: SingerRequest): string | null {
    const asks = song.asks;
    if (!asks) return null;
    const parts = [];
    if (asks.key !== null && asks.key !== undefined) parts.push(`key ${signed(asks.key)}`);
    if (asks.voice) parts.push(VOICE_WORD[asks.voice]);
    return parts.length > 0 ? `asks ${parts.join(" · ")}` : null;
  }

  function voiceOf(deck: number): number {
    return decks.find((d) => d.number === deck)?.voice ?? 1;
  }

  let rotation = $state<Rotation | null>(null);
  let error = $state("");

  let singer = $state("");
  let song = $state("");
  /** The record chosen from the collection for the song, if one was. */
  let picked = $state<LibraryTrack | null>(null);
  let matches = $state<LibraryTrack[]>([]);

  async function refresh() {
    try {
      rotation = await karaokeRotation();
    } catch (e) {
      error = String(e);
    }
  }

  $effect(() => {
    if (!enabled) return;
    void refresh();
  });

  /**
   * K4: singers from the room. Whether phones may put names down, and how —
   * the host's switches; the names themselves arrive in the rotation.
   */
  let phones = $state<SingRules | null>(null);
  /** K4: photos phones sent, for the host to show or refuse. */
  let photos = $state<SingPhoto[]>([]);
  function readPhones() {
    void singRules()
      .then((rules) => {
        phones = rules;
        if (rules.photos) return singPhotos().then((all) => (photos = all));
        photos = [];
      })
      .catch(() => {});
  }
  async function photo(change: Promise<SingPhoto[]>) {
    error = "";
    try {
      photos = await change;
    } catch (e) {
      error = String(e);
    }
  }
  $effect(() => {
    if (!enabled) return;
    readPhones();
  });
  // While phones may sign up, names arrive without anybody here pressing
  // anything: the rotation is read again every few seconds.
  const phonesOpen = $derived(phones?.open ?? false);
  $effect(() => {
    if (!enabled || !phonesOpen) return;
    const timer = setInterval(() => {
      void refresh();
      readPhones();
    }, 3000);
    return () => clearInterval(timer);
  });
  async function setPhones(change: {
    open?: boolean;
    songsEach?: number;
    songbook?: boolean;
    key?: boolean;
    guide?: boolean;
    photos?: boolean;
  }) {
    error = "";
    try {
      phones = await singRulesSet(change);
    } catch (e) {
      error = String(e);
    }
  }

  /**
   * §122: the look behind the words on the singers' screen — djmanzo's own
   * layers, or off. Kept in Rust, because the screen is another window.
   */
  let looks = $state<Look[]>([]);
  let lookName = $state<string | null>(null);
  /** Step 6: the DJ's AI picks each record's look. */
  let lookByAi = $state(false);
  function readLooks() {
    void Promise.all([looksCatalogue(), lookChosen()])
      .then(([all, chosen]) => {
        looks = all;
        lookName = chosen.singers;
        lookByAi = chosen.ai;
      })
      .catch(() => {});
  }
  $effect(() => {
    if (!enabled) return;
    readLooks();
  });
  const chosenLook = $derived(looks.find((l) => l.name === lookName) ?? null);
  const leftOut = $derived(chosenLook ? validate(chosenLook).dropped : []);
  const stillScreen =
    typeof window !== "undefined" && window.matchMedia?.("(prefers-reduced-motion: reduce)").matches === true;

  async function pickLook(name: string | null) {
    try {
      lookName = (await lookChoose("singers", name)).singers;
    } catch (e) {
      error = String(e);
    }
  }

  /**
   * §123: counted up on every change, so the guest book reads the journal
   * again — a song marked as sung is on the singer's record at once.
   */
  let journalRev = $state(0);

  /** §123: the up-next singer's record, to open in the guest book. */
  let asking = $state<{ name: string } | null>(null);

  /** Every change comes back as the whole rotation, so the list is Rust's. */
  /** K9: a recording's length as the host reads it: 1:05. */
  function clock(seconds: number): string {
    const whole = Math.floor(seconds);
    return `${Math.floor(whole / 60)}:${String(whole % 60).padStart(2, "0")}`;
  }

  /**
   * K9: while a song is being recorded, its length is read again every
   * second, so the host can see it running.
   */
  const recordingNow = $derived(!!rotation?.recording);
  $effect(() => {
    if (!enabled || !recordingNow) return;
    const timer = setInterval(() => void refresh(), 1000);
    return () => clearInterval(timer);
  });

  async function change(done: Promise<Rotation>) {
    error = "";
    try {
      rotation = await done;
    } catch (e) {
      error = String(e);
    }
    journalRev += 1;
  }

  /**
   * K3: the singers' microphones as Rust answers — shared with the section
   * below, so a singer put on a microphone here is on its row there.
   */
  let mics = $state<Vocals | null>(null);
  /** How many microphones the engine holds. */
  const micCount = $derived(vocals?.inputs ?? 0);
  const sameName = (a: string, b: string) => a.trim().toLowerCase() === b.trim().toLowerCase();

  /** Put a singer on a microphone, the way a record goes on a deck; `null` takes them off. */
  async function onMic(strip: number, singer: string | null) {
    error = "";
    try {
      mics = await vocalStripSinger(strip, singer);
    } catch (e) {
      error = String(e);
    }
  }

  /** A new night has nobody on a microphone: Rust has taken them off. */
  async function newNight() {
    await change(karaokeClear());
    if (micCount > 0) mics = await vocalsState().catch(() => mics);
  }

  /**
   * The collection, searched as the song is typed. A quarter of a second
   * after the last key, so a fast typist does not send a query a letter.
   */
  let searchTimer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    const text = song.trim();
    clearTimeout(searchTimer);
    if (!enabled || text.length < 2 || (picked && label(picked) === text)) {
      matches = [];
      return;
    }
    searchTimer = setTimeout(() => {
      void librarySearch(text)
        .then((found) => {
          matches = (found ?? []).slice(0, 5);
          void markFits();
        })
        .catch(() => (matches = []));
    }, 250);
    return () => clearTimeout(searchTimer);
  });

  function label(track: LibraryTrack): string {
    return track.artist ? `${track.artist} – ${track.title}` : track.title;
  }

  /**
   * K8: how each match fits the singer being typed, when their range has
   * been read — empty otherwise, and nothing is marked.
   */
  let fits = $state<RangeFit[]>([]);
  async function markFits() {
    const name = singer.trim();
    const known = rotation?.singers.find((s) => s.name.toLowerCase() === name.toLowerCase())?.range;
    if (!name || !known || matches.length === 0) {
      fits = [];
      return;
    }
    fits = await karaokeFits(
      name,
      matches.map((m) => m.id),
    ).catch(() => []);
  }

  // The singer may be typed after the song: marked again for whoever it is.
  $effect(() => {
    void singer;
    if (matches.length > 0) void markFits();
  });

  /** K8: a fit in the host's words. */
  function fitWords(fit: RangeFit | undefined): string {
    if (!fit) return "";
    switch (fit.fit) {
      case "as-recorded":
        return "fits as recorded";
      case "key":
        return `fits ${signed(fit.key)}`;
      case "stretch":
        return `a stretch at ${signed(fit.key)}`;
      case "no":
        return "out of their range";
      default:
        return "";
    }
  }

  /** K8: the key a fit offers — none for a stretch or no fit. */
  function offeredKey(fit: RangeFit | undefined): number | null {
    if (fit?.fit === "as-recorded") return 0;
    if (fit?.fit === "key") return fit.key;
    return null;
  }

  /** K8: the key the picked song is offered in, if its fit offered one. */
  let pickedKey = $state<number | null>(null);

  function pick(track: LibraryTrack, at: number) {
    picked = track;
    pickedKey = offeredKey(fits[at]);
    song = label(track);
    matches = [];
    fits = [];
  }

  /** K8: *Songs for them* — records that fit the singer being typed. */
  let songsFor = $state<SongFor[] | null>(null);
  const typedRange = $derived(
    rotation?.singers.find((s) => s.name.toLowerCase() === singer.trim().toLowerCase())?.range ?? null,
  );
  async function suggest() {
    error = "";
    try {
      songsFor = await karaokeSongsFor(singer.trim());
    } catch (e) {
      error = String(e);
    }
  }
  async function askFor(found: SongFor) {
    const name = singer.trim();
    const title = found.artist ? `${found.artist} – ${found.title}` : found.title;
    await change(karaokeAsk(name, title, found.track, found.path, offeredKey(found.offer)));
    if (!error) songsFor = null;
  }

  async function add() {
    const name = singer.trim();
    const title = song.trim();
    if (!name || !title) return;
    const chosen = picked && label(picked) === title ? picked : null;
    // K8: the key its fit offered — a starting key the host can still change.
    const key = chosen ? pickedKey : null;
    await change(karaokeAsk(name, title, chosen?.id ?? null, chosen?.path ?? null, key));
    if (!error) {
      singer = "";
      song = "";
      picked = null;
      pickedKey = null;
      songsFor = null;
    }
  }

  /**
   * Put the up-next song on a deck, in the singer's key, with the voice they
   * asked for — in Rust (K2), where `karaoke load N` does the same for a held
   * command or a controller pad.
   */
  async function load(deck: number) {
    const next = rotation?.singers.find((s) => s.name === rotation?.up_next)?.songs[0];
    if (!next?.path) return;
    error = "";
    try {
      rotation = await karaokeLoad(deck);
    } catch (e) {
      error = String(e);
    }
    // K5: the song on a deck is what *Bring them in* waits for.
    await refresh();
    // K6: and loading separates it, which is what reads its line.
    lineAsked += 1;
  }

  /**
   * K6: the line the up-next song's voice sings, read from its separated
   * vocal — what a singer will be scored against. Asked again when the song
   * changes or is loaded, since loading is what separates it.
   */
  let line = $state<VocalLine | null | undefined>(undefined);
  const lineTrack = $derived(
    rotation?.singers.find((s) => s.name === rotation?.up_next)?.songs[0]?.track ?? null,
  );
  let lineAsked = $state(0);
  $effect(() => {
    const track = lineTrack;
    void lineAsked;
    if (!enabled || !track) {
      line = undefined;
      return;
    }
    let live = true;
    vocalLine(track)
      .then((found) => {
        if (live) line = found;
      })
      .catch(() => {
        if (live) line = undefined;
      });
    return () => {
      live = false;
    };
  });

  // Not read yet: separation takes its time after a load, so it is asked
  // again every few seconds until it arrives.
  const lineWaiting = $derived(enabled && lineTrack !== null && line === null);
  $effect(() => {
    if (!lineWaiting) return;
    const timer = setInterval(() => (lineAsked += 1), 5000);
    return () => clearInterval(timer);
  });

  // While a song with a line can be scored, the scores are read again every
  // two seconds — they change as it is sung.
  const scoringLive = $derived(enabled && !!line?.scorable && !(rotation?.unscored ?? false));
  $effect(() => {
    if (!scoringLive) return;
    const timer = setInterval(() => void refresh(), 2000);
    return () => clearInterval(timer);
  });

  /**
   * K6: a versus round being set up — two team names and which team each
   * singer is on. Sent when the host starts it; Rust keeps the round.
   */
  let teamNames = $state(["Team A", "Team B"]);
  let sides = $state<Record<string, 0 | 1 | null>>({});
  async function startVersus() {
    const names = (rotation?.singers ?? []).map((s) => s.name);
    const teams = [0, 1].map((side) => ({
      name: teamNames[side],
      singers: names.filter((name) => sides[name] === side),
    }));
    await change(karaokeVersus(teams));
  }

  /** The line as an SVG path, broken where nobody sings, a semitone a unit. */
  function linePath(cents: (number | null)[]): { d: string; low: number; high: number } {
    const sung = cents.filter((c): c is number => c !== null);
    const low = Math.min(...sung);
    const high = Math.max(...sung);
    let d = "";
    let drawing = false;
    cents.forEach((c, i) => {
      if (c === null) {
        drawing = false;
        return;
      }
      d += `${drawing ? "L" : "M"}${i} ${((high - c) / 100).toFixed(1)}`;
      drawing = true;
    });
    return { d, low, high };
  }

  /**
   * K5: while a singer is being brought in, the row follows the mix until
   * it is done — the rotation is read again every two seconds.
   */
  const bringing = $derived(rotation?.bring_in?.bringing ?? false);
  $effect(() => {
    if (!enabled || !bringing) return;
    const timer = setInterval(() => void refresh(), 2000);
    return () => clearInterval(timer);
  });

  /**
   * One place up in calling order: in front of the waiting singer before
   * them. Found in the whole rotation, not in the list on screen — the
   * rotation also holds singers with nothing waiting, so the two positions
   * are not the same number.
   */
  /**
   * A singer carried by drag and drop to another's place in calling order:
   * the type a drop target reads it by, and who is being carried.
   */
  const SINGER = "application/x-djmanzo-singer";
  let carrying = $state<string | null>(null);
  /** Who a carried singer is over, for marking where they would land. */
  let landing = $state<string | null>(null);

  function carrySinger(event: DragEvent, name: string) {
    carrying = name;
    event.dataTransfer?.setData(SINGER, name);
    if (event.dataTransfer) event.dataTransfer.effectAllowed = "move";
  }

  function overSinger(event: DragEvent, name: string) {
    if (!Array.from(event.dataTransfer?.types ?? []).includes(SINGER)) return;
    event.preventDefault();
    if (event.dataTransfer) event.dataTransfer.dropEffect = "move";
    landing = name;
  }

  /**
   * Dropped on somebody: the carried singer takes their place in calling
   * order — `Rotation::move_to` takes them out and puts them in at that
   * place, so moving down lands after the one dropped on and moving up lands
   * before them, which is the place the host pointed at either way. Dropped
   * on the up-next card, they are next: the birthday singer.
   */
  async function dropSinger(event: DragEvent, onto: string) {
    const name = event.dataTransfer?.getData(SINGER) || carrying;
    carrying = null;
    landing = null;
    if (!name || name === onto) return;
    event.preventDefault();
    const to = rotation?.singers.findIndex((s) => s.name === onto) ?? -1;
    if (to < 0) return;
    await change(karaokeMove(name, to));
  }

  /** A song dragged from the collection onto the form is the song asked for. */
  let songOver = $state(false);

  function dropSong(event: DragEvent) {
    songOver = false;
    const dropped = droppedTrack(event);
    if (!dropped) return;
    event.preventDefault();
    picked = { ...dropped } as LibraryTrack;
    pickedKey = null;
    song = label(picked);
    matches = [];
    fits = [];
  }

  /**
   * One press for what dropping a singer on the up-next card does: they
   * sing next. The drag is far in a panel this tall; the button is where
   * the singer already is.
   */
  async function singNext(name: string) {
    const to = rotation?.singers.findIndex((s) => s.name === rotation?.up_next) ?? -1;
    if (to < 0) return;
    await change(karaokeMove(name, to));
  }

  async function moveUp(name: string, before: string) {
    const to = rotation?.singers.findIndex((s) => s.name === before) ?? -1;
    if (to < 0) return;
    await change(karaokeMove(name, to));
  }

  /**
   * §107: break music between singers — `dj_app::breaks` fades it in when
   * the room goes quiet and out when the next song starts. Asked again every
   * two seconds while it is on, so the line under it says where it has got
   * to.
   */
  let breaks = $state<Breaks | null>(null);

  async function refreshBreaks() {
    try {
      breaks = await karaokeBreaks();
    } catch (e) {
      error = String(e);
    }
  }

  $effect(() => {
    if (!enabled) return;
    void refreshBreaks();
  });

  // Its own effect on a derived switch: one that read `breaks` itself would
  // re-run on every answer it asked for, and ask again.
  const breaksOn = $derived(breaks?.on ?? false);
  $effect(() => {
    if (!enabled || !breaksOn) return;
    // The rotation too: what break music plays decides whether a singer
    // can be brought in from it.
    const timer = setInterval(() => {
      void refreshBreaks();
      void refresh();
    }, 2000);
    return () => clearInterval(timer);
  });

  async function setBreaks(
    change: Partial<Pick<Breaks, "on" | "deck" | "playlist" | "level" | "lead" | "mix_out">>,
  ) {
    if (!breaks) return;
    const next = { ...breaks, ...change };
    error = "";
    try {
      breaks = await karaokeBreaksSet(
        next.on,
        next.deck,
        next.playlist,
        next.level,
        deckCount,
        next.lead,
        next.mix_out,
      );
    } catch (e) {
      error = String(e);
    }
  }

  const BREAK_PHASES: Record<Breaks["phase"], string> = {
    off: "Waiting for the room to go quiet",
    loading: "Putting the next break record on",
    "fading-in": "Fading in",
    playing: "Playing",
    "fading-out": "Fading out for the singer",
  };

  const breakDeckHidden = $derived(!!breaks && breaks.deck > deckCount);

  const upNext = $derived(
    rotation?.singers.find((s) => s.name === rotation?.up_next) ?? null,
  );
  const waiting = $derived((rotation?.singers ?? []).filter((s) => s.songs.length > 0));

  /**
   * K7: a quiz made from the collection — `dj_app::quiz` makes the questions
   * and plays each clip on its deck; this sets a round and asks the next
   * question when the host says so. Nothing advances on its own: a room
   * needs the host's timing.
   */
  let quiz = $state<Quiz | null>(null);
  const LETTERS = ["A", "B", "C", "D"] as const;
  const QUIZ_KINDS: [QuizKind, string][] = [
    ["name-it", "Name it"],
    ["backwards", "Backwards"],
    ["chipmunk", "Chipmunk"],
    ["stems-in", "Stems in"],
    ["finish-the-line", "Finish the line"],
  ];

  async function quizDo(done: Promise<Quiz>) {
    error = "";
    try {
      quiz = await done;
    } catch (e) {
      error = String(e);
    }
  }

  function setQuiz(change: Partial<Pick<Quiz, "deck" | "kinds" | "count">>) {
    if (!quiz) return;
    // Kept before it is sent, so a second press before the answer builds on
    // the first rather than undoing it.
    const next = { ...quiz, ...change };
    quiz = next;
    void quizDo(quizSet(next.deck, next.kinds, next.count, deckCount));
  }

  function toggleKind(kind: QuizKind) {
    if (!quiz) return;
    const kinds = quiz.kinds.includes(kind) ? quiz.kinds.filter((k) => k !== kind) : [...quiz.kinds, kind];
    if (kinds.length > 0) setQuiz({ kinds });
  }

  $effect(() => {
    if (!enabled) return;
    void quizDo(readQuiz());
  });

  // While a clip plays or its question is open, asked again: the line under
  // it says when the clip is over, and phones answer for ten seconds after.
  const quizLive = $derived(!!quiz && (quiz.playing || (!!quiz.question && quiz.question.right === null)));
  $effect(() => {
    if (!enabled || !quizLive) return;
    const timer = setInterval(() => void quizDo(readQuiz()), 1000);
    return () => clearInterval(timer);
  });

  function signed(key: number): string {
    return key > 0 ? `+${key}` : `${key}`;
  }
</script>

<div class="singers" data-singers>
  {#if error}
    <p class="error" role="alert">{error}</p>
  {/if}

  {#if upNext}
    {@const next = upNext.songs[0]}
    <!--
      The up-next singer, large: the host reads it out, and reads it from
      across the booth with a microphone in the other hand.
    -->
    <section
      class="up"
      class:landing={landing === upNext.name}
      aria-label="Up next"
      ondragover={(event) => overSinger(event, upNext.name)}
      ondragleave={() => (landing = null)}
      ondrop={(event) => void dropSinger(event, upNext.name)}
    >
      <p class="who">
        {upNext.name}
        {#if upNext.ready}<span class="badge ready" data-ready>Ready</span>{/if}
        {#if upNext.from_phone}<span class="badge" title="Put their own name down from their phone">phone</span>{/if}
        {#if upNext.range}
          <!-- K8: their range, read from their microphone. -->
          <span class="range" data-range title="{upNext.name}'s range, read from their microphone tonight"
            >{rangeWords(upNext.range)}</span
          >
        {/if}
      </p>
      <p class="what">{next.title}</p>
      {#if next.track && line !== undefined}
        <!-- K6: the line the song's voice sings — what a singer is scored against. -->
        {#if line && line.scorable}
          {@const drawn = linePath(line.cents)}
          <svg
            class="sung-line"
            role="img"
            aria-label="The line {next.title}'s voice sings"
            viewBox="0 -1 {line.cents.length} {(drawn.high - drawn.low) / 100 + 2}"
            preserveAspectRatio="none"
          >
            <path d={drawn.d} />
          </svg>
          {#if (rotation?.scores ?? []).length > 0}
            <!-- K6: how each singer on a microphone is doing, as it goes. -->
            <p class="scoring" data-scoring>
              {#each rotation?.scores ?? [] as score (score.strip)}
                <span>{score.singer} {score.percent === null ? "—" : `${Math.round(score.percent)}%`}</span>
              {/each}
            </p>
          {/if}
        {:else if line}
          <p class="hint" data-unscored>
            Not scored: no single voice was found in this song's separated vocal.
          </p>
        {:else}
          <p class="hint" data-unscored>
            Not scored: this song has not been separated yet — loading it on a deck separates it.
          </p>
        {/if}
      {/if}
      {#if asked(next)}
        <div class="asks" role="group" aria-label="What {upNext.name} asks for" data-asks>
          <span>{asked(next)}</span>
          <button onclick={() => void change(karaokeTake(upNext.name))}>Take</button>
          <button onclick={() => void change(karaokeRefuse(upNext.name))}>No thanks</button>
        </div>
      {/if}
      <label class="switch">
        <input
          type="checkbox"
          checked={!(rotation?.unscored ?? false)}
          onchange={(event) => void change(karaokeScoring(event.currentTarget.checked))}
        />
        Score the singers
      </label>
      <div class="choices" role="group" aria-label="What decides tonight's best">
        <span>decided by</span>
        <button
          aria-pressed={(rotation?.verdict ?? "singing") === "singing"}
          onclick={() => void change(karaokeVerdict("singing"))}>the singing</button
        >
        <button
          aria-pressed={rotation?.verdict === "room"}
          title="The room's applause in the eight seconds after Sang, heard by the room's microphone"
          onclick={() => void change(karaokeVerdict("room"))}>the room</button
        >
      </div>
      <div class="key" role="group" aria-label="Key for {upNext.name}">
        <button
          aria-label="Lower the key"
          onclick={() => void change(karaokeKey(upNext.name, next.key - 1))}>−</button
        >
        <span class="mono" data-key>{next.key === 0 ? "as recorded" : `key ${signed(next.key)}`}</span>
        <button
          aria-label="Raise the key"
          onclick={() => void change(karaokeKey(upNext.name, next.key + 1))}>+</button
        >
      </div>
      <div class="actions">
        {#if next.path}
          {#each Array.from({ length: deckCount }, (_, i) => i + 1) as deck (deck)}
            <button onclick={() => void load(deck)}>Load on {deck}</button>
          {/each}
        {:else}
          <span class="hint">Not found in the collection yet — search for it below.</span>
          <BuyLinks title={next.title} karaoke />
        {/if}
        {#if rotation?.bring_in}
          <!--
            K5: from what the room hears into the song at the next phrase,
            the song keeping its tempo. Once pressed, it says so until the
            mix is done.
          -->
          <button
            class="bring"
            disabled={rotation.bring_in.bringing}
            title="Mix deck {rotation.bring_in.from} into {upNext.name}'s song on deck {rotation.bring_in
              .to} at the next phrase"
            onclick={() => void change(karaokeBringIn())}
            >{rotation.bring_in.bringing
              ? `Bringing ${upNext.name} in from deck ${rotation.bring_in.from}…`
              : "Bring them in"}</button
          >
        {/if}
        <button class="done" onclick={() => void change(karaokeSang())}>Sang</button>
        {#if rotation?.recording}
          <!-- K9: what is being recorded, and for how long; *Sang* stops it too. -->
          <span class="recording" role="status" data-recording>
            Recording {rotation.recording.singer}'s song — {clock(rotation.recording.seconds)}
          </span>
          <button onclick={() => void change(karaokeRecordStop())}>Stop recording</button>
        {:else if rotation?.may_record}
          <button
            title="Recorded from their microphone over the record, until Sang — they agreed to it in the guest book"
            onclick={() => void change(karaokeRecordSong(upNext.name))}>Record their song</button
          >
        {/if}
        <button
          title="Called and not here: to the bottom of the list, songs kept"
          onclick={() => void change(karaokeNotHere(upNext.name))}>Not here</button
        >
        <button
          title="Their record in the guest book: what they agree to, their details, their voice"
          onclick={() => (asking = { name: upNext.name })}>Guest book</button
        >
      </div>
      {#if micCount > 0 && mics}
        <!--
          K3: on a microphone, the way a record goes on a deck — and through
          the chain kept for them. Pressed again, off it.
        -->
        <div class="on-mic" role="group" aria-label="{upNext.name} on a microphone">
          {#each Array.from({ length: micCount }, (_, i) => i) as strip (strip)}
            {@const here = sameName(mics.on[strip]?.singer ?? "", upNext.name)}
            <button
              aria-pressed={here}
              title={here ? `Take ${upNext.name} off Mic ${strip + 1}` : `${upNext.name} sings on Mic ${strip + 1}`}
              onclick={() => void onMic(strip, here ? null : upNext.name)}>Mic {strip + 1}</button
            >
          {/each}
        </div>
      {/if}
    </section>
  {:else}
    <p class="empty">Nobody is queued. Add the first singer below.</p>
  {/if}

  <!--
    The vocal, per deck. Beside the rotation rather than on the deck, because
    this is where the host decides it; the deck says so too, once it is set.
  -->
  <div class="voices" role="group" aria-label="The recorded singer">
    {#each Array.from({ length: deckCount }, (_, i) => i + 1) as deck (deck)}
      <div class="voice" role="group" aria-label="Vocal on deck {deck}">
        <span class="deck-label mono">{deck}</span>
        {#each VOICES as choice (choice.level)}
          <button
            aria-pressed={Math.abs(voiceOf(deck) - choice.level) < 0.01}
            onclick={() => void dispatch(`deck ${deck} voice ${choice.level}`)}>{choice.label}</button
          >
        {/each}
      </div>
    {/each}
  </div>

  {#if breaks}
    <!--
      Break music: the silence between singers is where a karaoke night
      loses its room. Its own deck and its own playlist, so it never touches
      a singer's song.
    -->
    <section class="breaks" aria-label="Break music" data-phase={breaks.phase}>
      <label class="switch">
        <input
          type="checkbox"
          checked={breaks.on}
          onchange={(event) => void setBreaks({ on: event.currentTarget.checked })}
        />
        Break music between singers
      </label>
      <div class="break-row">
        <label>
          From
          <select
            aria-label="Break music playlist"
            value={breaks.playlist ?? ""}
            onchange={(event) => {
              const id = Number(event.currentTarget.value);
              void setBreaks({ playlist: Number.isFinite(id) && event.currentTarget.value !== "" ? id : null });
            }}
          >
            <option value="" disabled>Choose a playlist</option>
            {#each breaks.playlists as list (list.id)}
              <option value={list.id}>{list.name} ({list.tracks})</option>
            {/each}
          </select>
        </label>
        <div class="choices" role="group" aria-label="Break music deck">
          <span>on deck</span>
          {#each Array.from({ length: deckCount }, (_, i) => i + 1) as deck (deck)}
            <button aria-pressed={breaks.deck === deck} onclick={() => void setBreaks({ deck })}>{deck}</button>
          {/each}
        </div>
        <div class="choices" role="group" aria-label="Break music level">
          {#each breaks.levels as [level, name] (level)}
            <button aria-pressed={Math.abs(breaks.level - level) < 0.01} onclick={() => void setBreaks({ level })}
              >{name}</button
            >
          {/each}
        </div>
        <!--
          K5: the record that leads into the next singer's song, ranked by
          the Next rail's own scorer, rather than the next in the playlist.
        -->
        <label class="switch">
          <input
            type="checkbox"
            checked={breaks.lead}
            onchange={(event) => void setBreaks({ lead: event.currentTarget.checked })}
          />
          Lead into the next song
        </label>
        <!--
          K5: the break record brought in under the end of each song, at the
          break level, rather than after two seconds of quiet. Off by
          default: applause wants the room quiet.
        -->
        <label class="switch">
          <input
            type="checkbox"
            checked={breaks.mix_out}
            onchange={(event) => void setBreaks({ mix_out: event.currentTarget.checked })}
          />
          Mix out of each song
        </label>
      </div>
      {#if breaks.on}
        {#if breakDeckHidden}
          <p class="problem" role="status">
            Deck {breaks.deck} is not on screen. Choose one of these for break music.
          </p>
        {:else if breaks.problem}
          <p class="problem" role="status">{breaks.problem}</p>
        {:else}
          <p class="phase" role="status">{BREAK_PHASES[breaks.phase]}</p>
        {/if}
        {#if breaks.chosen && !breakDeckHidden}
          <!-- K5: what the record was chosen for, in the rail's words. -->
          <p class="chosen" role="note" aria-label="Chosen for">
            <span class="title">{breaks.chosen.title}</span>
            {breaks.chosen.after ? "after" : "for"}
            {breaks.chosen.singer}'s <span class="title">{breaks.chosen.song}</span>{#if breaks.chosen.why}<span
                class="why mono">{breaks.chosen.why}</span
              >{/if}
          </p>
        {/if}
      {/if}
    </section>
  {/if}

  {#if rotation && rotation.singers.length > 1}
    <!-- K6: two teams, scored by tonight's verdict, from the next song. -->
    <section class="versus" aria-label="Versus">
      {#if rotation.versus}
        <ul class="standing">
          {#each rotation.versus as team (team.name)}
            <li>
              <strong>{team.name}</strong>
              {team.mean === null ? "—" : Math.round(team.mean)}
              <span class="hint">{team.songs} {team.songs === 1 ? "song" : "songs"}: {team.singers.join(", ")}</span>
            </li>
          {/each}
        </ul>
        <button onclick={() => void change(karaokeVersus(null))}>End the round</button>
      {:else}
        <div class="teams">
          {#each [0, 1] as side (side)}
            <input aria-label="Team {side + 1}'s name" maxlength="24" bind:value={teamNames[side]} />
          {/each}
        </div>
        <ul class="sides">
          {#each rotation.singers as person (person.name)}
            <li>
              <span>{person.name}</span>
              <div class="choices" role="group" aria-label="{person.name}'s team">
                {#each [0, 1] as side (side)}
                  <button
                    aria-pressed={sides[person.name] === side}
                    onclick={() => (sides[person.name] = sides[person.name] === side ? null : (side as 0 | 1))}
                    >{teamNames[side]}</button
                  >
                {/each}
              </div>
            </li>
          {/each}
        </ul>
        <button
          disabled={![0, 1].every((side) => Object.values(sides).includes(side as 0 | 1))}
          onclick={() => void startVersus()}>Start the round</button
        >
      {/if}
    </section>
  {/if}

  {#if quiz}
    <!--
      K7: a quiz between singers, made by rules from the collection. Each
      clip plays on the quiz's deck and the deck is put back after it.
    -->
    <section class="quiz" aria-label="Quiz" data-quiz>
      <div class="quiz-row">
        <div class="choices" role="group" aria-label="Quiz questions a round">
          {#each quiz.counts as count (count)}
            <button aria-pressed={quiz.count === count} onclick={() => setQuiz({ count })}>{count}</button>
          {/each}
          <span>questions</span>
        </div>
        <div class="choices" role="group" aria-label="Quiz deck">
          <span>on deck</span>
          {#each Array.from({ length: deckCount }, (_, i) => i + 1) as deck (deck)}
            <button aria-pressed={quiz.deck === deck} onclick={() => setQuiz({ deck })}>{deck}</button>
          {/each}
        </div>
        <div class="choices" role="group" aria-label="Quiz kinds">
          {#each QUIZ_KINDS as [kind, words] (kind)}
            <button aria-pressed={quiz.kinds.includes(kind)} onclick={() => toggleKind(kind)}>{words}</button>
          {/each}
        </div>
      </div>
      {#if quiz.question}
        <p class="asking">
          <span class="mono">{quiz.question.number}/{quiz.count}</span>
          <strong>{quiz.question.words}</strong>
          {#if quiz.playing}<span class="hint" role="status">playing the clip</span>{/if}
        </p>
        {#if quiz.question.prompt}
          <p class="prompt" data-prompt>{quiz.question.prompt}…</p>
        {/if}
        <ol class="answers" aria-label="Answers">
          {#each quiz.question.answers as answer, at (at)}
            <li data-right={quiz.question.right === at || undefined}>
              <span class="letter mono">{LETTERS[at]}</span>{answer}{#if quiz.phones.counts}<span class="count mono"
                  >{quiz.phones.counts[at]}<span class="sr-only"
                    >{quiz.phones.counts[at] === 1 ? " phone" : " phones"}</span
                  ></span
                >{/if}
            </li>
          {/each}
        </ol>
      {/if}
      {#if quiz.phones.players > 0}
        <!-- K7: the phones playing along on the room's page. -->
        <p class="hint" data-phones>
          {quiz.phones.players}
          {quiz.phones.players === 1 ? "phone" : "phones"} playing{#if quiz.question}, {quiz.phones.answered} answered{/if}
        </p>
      {/if}
      {#if quiz.phones.leaders.length > 0}
        <ol class="leaders" aria-label="Quiz leaderboard">
          {#each quiz.phones.leaders.slice(0, 5) as [nickname, points], place (place)}
            <li><span>{nickname}</span><span class="mono">{points}</span></li>
          {/each}
        </ol>
      {/if}
      {#if quiz.problem}
        <p class="problem" role="status">{quiz.problem}</p>
      {:else if quiz.quiet}
        <p class="problem" role="status">Deck {quiz.deck}'s fader is down: the room will not hear the clip.</p>
      {/if}
      <div class="quiz-row">
        <button disabled={quiz.asked >= quiz.count} onclick={() => void quizDo(quizNext())}
          >{quiz.question ? "Next question" : "Start the quiz"}</button
        >
        {#if quiz.question}
          <button onclick={() => void quizDo(quizAgain())}>Play it again</button>
          <button disabled={quiz.question.right !== null} onclick={() => void quizDo(quizReveal())}
            >Show the answer</button
          >
          <button onclick={() => void quizDo(quizEnd())}>End the quiz</button>
        {/if}
      </div>
    </section>
  {/if}

  {#if waiting.length > 1}
    <ol class="order" aria-label="Calling order">
      {#each waiting.slice(1) as person, index (person.name)}
        <li
          draggable="true"
          class:carried={carrying === person.name}
          class:landing={landing === person.name && carrying !== person.name}
          title="Drag to another singer's place"
          ondragstart={(event) => carrySinger(event, person.name)}
          ondragend={() => {
            carrying = null;
            landing = null;
          }}
          ondragover={(event) => overSinger(event, person.name)}
          ondragleave={() => {
            if (landing === person.name) landing = null;
          }}
          ondrop={(event) => void dropSinger(event, person.name)}
        >
          <span class="pos mono">{index + 2}</span>
          <span class="name"
            >{person.name}{#if person.from_phone}<span
                class="badge"
                title="Put their own name down from their phone">phone</span
              >{/if}</span
          >
          <span class="song">{person.songs[0].title}</span>
          {#if asked(person.songs[0])}
            <span class="asked" data-asks>{asked(person.songs[0])}</span>
            <button
              class="quiet"
              aria-label="Take what {person.name} asks for"
              title="Take it"
              onclick={() => void change(karaokeTake(person.name))}>✓</button
            >
            <button
              class="quiet"
              aria-label="Refuse what {person.name} asks for"
              title="No thanks"
              onclick={() => void change(karaokeRefuse(person.name))}>✗</button
            >
          {/if}
          {#if person.songs[0].key !== 0}
            <span class="mono k">{signed(person.songs[0].key)}</span>
          {/if}
          {#if person.songs.length > 1}
            <span class="more" title="More songs waiting for later turns">+{person.songs.length - 1}</span>
          {/if}
          <button
            class="quiet"
            aria-label="{person.name} sings next"
            title="Sings next"
            onclick={() => void singNext(person.name)}>⤒</button
          >
          <button
            class="quiet"
            aria-label="Move {person.name} up"
            title="Move up"
            onclick={() => void moveUp(person.name, waiting[index].name)}>↑</button
          >
          <button
            class="quiet"
            aria-label="{person.name} leaves"
            title="Leaves — their keys are remembered"
            onclick={() => void change(karaokeLeave(person.name))}>×</button
          >
        </li>
      {/each}
    </ol>
  {/if}

  <form
    class="ask"
    class:song-over={songOver}
    onsubmit={(event) => {
      event.preventDefault();
      void add();
    }}
    ondragover={(event) => {
      if (!carriesTrack(event)) return;
      event.preventDefault();
      if (event.dataTransfer) event.dataTransfer.dropEffect = "copy";
      songOver = true;
    }}
    ondragleave={(event) => {
      if (!(event.currentTarget as HTMLElement).contains(event.relatedTarget as Node | null)) songOver = false;
    }}
    ondrop={dropSong}
  >
    <input bind:value={singer} placeholder="Singer" aria-label="Singer's name" />
    <div class="song-box">
      <input
        bind:value={song}
        placeholder="Song — searches your collection"
        aria-label="Song"
        oninput={() => (picked = null)}
      />
      {#if matches.length > 0}
        <ul class="matches" aria-label="Songs in your collection">
          {#each matches as track, at (track.id)}
            <li>
              <button type="button" onclick={() => pick(track, at)}
                >{label(track)}{#if fitWords(fits[at])}<span class="fit" data-fit={fits[at]?.fit}
                    >{fitWords(fits[at])}</span
                  >{/if}</button
              >
            </li>
          {/each}
        </ul>
      {/if}
    </div>
    <button type="submit" disabled={!singer.trim() || !song.trim()}>Add</button>
    {#if typedRange}
      <button type="button" class="quiet" onclick={() => void suggest()}>Songs for {singer.trim()}</button>
    {/if}
  </form>
  {#if songsFor}
    <!-- K8: records that fit the singer, as recorded first. -->
    <section class="songs-for" aria-label="Songs for {singer.trim()}">
      {#if songsFor.length === 0}
        <p class="hint">Nothing separated in the collection fits {singer.trim()} yet.</p>
      {:else}
        <ul>
          {#each songsFor as found (found.track)}
            <li>
              <button type="button" onclick={() => void askFor(found)}
                >{found.artist ? `${found.artist} – ${found.title}` : found.title}<span class="fit"
                  >{fitWords(found.offer)}</span
                ></button
              >
            </li>
          {/each}
        </ul>
      {/if}
    </section>
  {/if}

  <!--
    K4: a line of the host's own under the words on the singers' screen.
    Kept on Enter or on leaving the field; empty takes it away.
  -->
  <label class="line-field">
    On the singers' screen
    <input
      maxlength="80"
      value={rotation?.line ?? ""}
      onchange={(event) => void change(karaokeLine(event.currentTarget.value))}
      data-host-line
    />
  </label>

  {#if phones}
    <!--
      K4: singers put their own names down from their phones, through the
      room's page — off until the host opens it.
    -->
    <section class="phones" aria-label="Singers from phones" data-from-phones>
      <label class="switch">
        <input
          type="checkbox"
          checked={phones.open}
          onchange={(event) => void setPhones({ open: event.currentTarget.checked })}
        />
        Singers put their names down from their phones
      </label>
      {#if phones.open && !phones.serving}
        <p class="problem" role="status">
          The room's page is not being served. Start it in Requests; its QR code takes them there.
        </p>
      {/if}
      <div class="break-row">
        <div class="choices" role="group" aria-label="Songs waiting per singer">
          <span>songs each</span>
          {#each [1, 2, 3] as n (n)}
            <button aria-pressed={phones.songs_each === n} onclick={() => void setPhones({ songsEach: n })}>{n}</button>
          {/each}
        </div>
        <div class="choices" role="group" aria-label="How a singer chooses a song">
          <button aria-pressed={phones.songbook} onclick={() => void setPhones({ songbook: true })}
            >From the songbook</button
          >
          <button aria-pressed={!phones.songbook} onclick={() => void setPhones({ songbook: false })}
            >Written in</button
          >
        </div>
      </div>
      <div class="break-row" role="group" aria-label="What a phone may ask for">
        <label class="switch">
          <input
            type="checkbox"
            checked={phones.key}
            onchange={(event) => void setPhones({ key: event.currentTarget.checked })}
          />
          May ask for a key
        </label>
        <label class="switch">
          <input
            type="checkbox"
            checked={phones.guide}
            onchange={(event) => void setPhones({ guide: event.currentTarget.checked })}
          />
          May ask for the guide vocal
        </label>
        <label class="switch">
          <input
            type="checkbox"
            checked={phones.photos}
            onchange={(event) => void setPhones({ photos: event.currentTarget.checked })}
          />
          May send a photo
        </label>
      </div>
      {#if phones.photos && photos.length > 0}
        <!--
          K4: a photo is on the singers' screen only once shown here, and
          only while its singer sings. Redrawn on arrival; gone at New night.
        -->
        <ul class="photos" aria-label="Photos from phones" data-photos>
          {#each photos as sent (sent.id)}
            <li data-approved={sent.approved}>
              <img src={sent.url} alt="{sent.name}'s photo" />
              <span class="name">{sent.name}</span>
              {#if sent.approved}
                <span class="hint">on the screen while they sing</span>
                <button onclick={() => void photo(singPhotoRefuse(sent.id))}>Take it down</button>
              {:else}
                <button onclick={() => void photo(singPhotoApprove(sent.id))}>Show it</button>
                <button onclick={() => void photo(singPhotoRefuse(sent.id))}>Refuse</button>
              {/if}
            </li>
          {/each}
        </ul>
      {/if}
      <p class="hint" data-songbook>
        {#if phones.songbook}
          {phones.songs === 1 ? "1 record" : `${phones.songs} records`} with words in the songbook.
        {:else}
          A song written in is found in your collection by you before it can be called.
        {/if}
      </p>
    </section>
  {/if}

  {#if rotation && rotation.lately.length > 0}
    <details class="lately">
      <summary>Sung lately</summary>
      <ul>
        {#each rotation.lately as sung, i (i)}
          <li>
            {sung.singer} — {sung.title}{sung.key !== 0 ? ` (${signed(sung.key)})` : ""}
          </li>
        {/each}
      </ul>
    </details>
  {/if}

  <Microphones
    live={vocals}
    {enabled}
    bind:vocals={mics}
    singers={(rotation?.singers ?? []).map((s) => s.name)}
    sungKey={rotation?.sung_key ?? null}
    {buffer}
    onrange={() => void refresh()}
  />

  <WordTiming {decks} {deckCount} />

  <Guests rev={journalRev} {asking} />

  <!--
    The words for the singers go on a screen of their own, facing the
    microphone — a second display, where the host's own screen is not.
  -->
  <button class="screen-out" onclick={() => void detachPanel("singers").catch((e) => (error = String(e)))}
    >Singers' screen — the words, on a display of its own</button
  >

  {#if looks.length > 0}
    <section class="look" aria-label="Behind the words" data-look-picker>
      <label>
        Behind the words
        <select
          aria-label="The look behind the words"
          value={lookName ?? ""}
          onchange={(e) => void pickLook(e.currentTarget.value === "" ? null : e.currentTarget.value)}
        >
          <option value="">Off — the record's cover and colours</option>
          {#each looks as look (look.name)}
            <option value={look.name} title={look.description}>{look.name}</option>
          {/each}
        </select>
      </label>
      {#if chosenLook}
        <p class="hint" data-look-description>{chosenLook.description}</p>
      {/if}
      <LookByAi on={lookByAi} onchange={(chosen) => (lookByAi = chosen.ai)} />
      {#each leftOut as note (note)}
        <p class="hint" role="note">Left out, because this djmanzo cannot draw it: {note}</p>
      {/each}
      {#if chosenLook && stillScreen}
        <p class="hint" role="note">
          This computer asks for reduced motion, so the singers' screen draws no looks — only the
          record's cover and colours.
        </p>
      {/if}
      <MilkdropPresets onchange={readLooks} />
      <Packs onchange={readLooks} />
    </section>
  {/if}

  <button class="quiet new-night" onclick={() => void newNight()}
    title="Everybody's keys are kept, and so are the guests who agreed to be"
    >New night — clear the list and tonight's guests</button
  >
</div>

<style>
  .singers {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    padding: 0.4rem 0.2rem;
  }

  .breaks {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    border-top: 1px solid var(--border);
    padding-top: 0.5rem;
  }

  .break-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.4rem 0.8rem;
  }

  .choices {
    display: flex;
    align-items: center;
    gap: 0.25rem;
  }

  .choices button[aria-pressed="true"] {
    border-color: var(--accent);
    color: var(--accent);
    font-weight: 600;
  }

  .breaks .phase {
    color: var(--text-dim);
    margin: 0;
  }

  .breaks .problem {
    color: var(--warn);
    margin: 0;
  }

  .breaks .chosen {
    margin: 0;
    color: var(--text-dim);
  }

  .breaks .chosen .title {
    color: var(--text);
    font-style: italic;
  }

  .breaks .chosen .why {
    margin-left: 0.5rem;
    font-size: 0.85em;
  }

  .up .who .range {
    margin-left: 0.5rem;
    font-size: 0.85em;
    font-weight: 400;
    color: var(--text-dim);
  }

  .up {
    border: 1px solid var(--accent);
    border-radius: var(--radius);
    padding: 0.5rem 0.6rem;
    background: var(--panel);
  }

  .who {
    margin: 0;
    font-size: 1.4em;
    font-weight: 700;
  }

  .what {
    margin: 0.1rem 0 0.4rem;
    color: var(--text-dim);
  }

  .key,
  .actions,
  .on-mic {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.35rem;
    margin-top: 0.3rem;
  }

  .on-mic button[aria-pressed="true"] {
    color: var(--active);
    border-color: var(--active);
  }

  .done {
    border-color: var(--accent);
    font-weight: 600;
  }

  .versus {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    border-top: 1px solid var(--border);
    padding-top: 0.5rem;
  }

  .versus .teams {
    display: flex;
    gap: 0.4rem;
  }

  .versus .teams input {
    min-width: 0;
    flex: 1;
  }

  .versus ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .versus .sides li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
  }

  .quiz {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    border-top: 1px solid var(--border);
    padding-top: 0.5rem;
  }

  .quiz-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.4rem 0.75rem;
  }

  /* Five kinds do not fit one row of a narrow panel: the row wraps, and each
     button keeps its words on one line. */
  .quiz-row .choices {
    flex-wrap: wrap;
  }

  .quiz-row .choices button {
    white-space: nowrap;
  }

  .quiz .asking {
    margin: 0;
    display: flex;
    gap: 0.5rem;
    align-items: baseline;
  }

  .quiz .prompt {
    margin: 0;
    font-style: italic;
  }

  .quiz .answers {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
  }

  .quiz .answers .letter {
    display: inline-block;
    width: 1.4rem;
    color: var(--text-dim);
  }

  .quiz .answers li[data-right] {
    color: var(--accent);
    font-weight: 600;
  }

  .quiz .answers .count {
    margin-left: 0.5rem;
    color: var(--text-dim);
    font-weight: 400;
  }

  /* Read aloud, not drawn: beside an answer, the number says it to the eye. */
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }

  .quiz .hint {
    margin: 0;
  }

  .quiz .leaders {
    margin: 0;
    padding-left: 1.4rem;
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
  }

  .quiz .leaders li span:last-child {
    margin-left: 0.5rem;
    color: var(--text-dim);
  }

  .quiz .problem {
    color: var(--warn);
    margin: 0;
  }

  .scoring {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem 0.9rem;
    margin: 0 0 0.3rem;
    color: var(--accent);
  }

  .sung-line {
    display: block;
    width: 100%;
    height: 2.2rem;
    margin: 0.2rem 0 0.3rem;
  }

  .sung-line path {
    fill: none;
    stroke: var(--accent);
    stroke-width: 1.5;
    vector-effect: non-scaling-stroke;
  }

  .bring {
    border-color: var(--accent);
    color: var(--accent);
    font-weight: 600;
  }

  .voices {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
  }

  .voice {
    display: flex;
    align-items: center;
    gap: 0.2rem;
  }

  .voice button[aria-pressed="true"] {
    border-color: var(--stem-vocal);
    color: var(--stem-vocal);
    font-weight: 600;
  }

  .deck-label {
    color: var(--text-dim);
    margin-right: 0.15rem;
  }

  .order {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .order li {
    display: flex;
    align-items: baseline;
    gap: 0.4rem;
  }

  .pos {
    color: var(--text-dim);
    min-width: 1.2rem;
  }

  .name {
    font-weight: 600;
  }

  .song {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-dim);
  }

  .more,
  .k {
    font-size: 0.8em;
    color: var(--text-dim);
  }

  .ask {
    display: flex;
    flex-wrap: wrap;
    gap: 0.35rem;
  }

  /* A record from the collection held over the form, and a singer held over
     another's place: where the drop would land. */
  .ask.song-over,
  .landing {
    outline: 2px dashed var(--accent);
    outline-offset: 2px;
  }

  .carried {
    opacity: 0.45;
  }

  .order li[draggable="true"] {
    cursor: grab;
  }

  .song-box {
    position: relative;
    flex: 1;
    min-width: 10rem;
  }

  .song-box input {
    width: 100%;
  }

  .fit {
    margin-left: 0.5rem;
    font-size: 0.85em;
    color: var(--text-dim);
  }

  .songs-for ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
  }

  .songs-for button {
    width: 100%;
    text-align: left;
  }

  /* Over what follows rather than in the form's row: in the row it stretched
     the singer's name field to the list's height, found by driving it. */
  .matches {
    list-style: none;
    margin: 0.2rem 0 0;
    padding: 0;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg);
    position: absolute;
    top: 100%;
    left: 0;
    right: 0;
    z-index: 5;
    max-height: 14rem;
    overflow-y: auto;
  }

  .matches button {
    width: 100%;
    text-align: left;
    border: 0;
    background: none;
    padding: 0.3rem 0.5rem;
  }

  .matches button:hover {
    background: var(--panel-hover);
  }

  .empty,
  .hint {
    color: var(--text-dim);
    font-size: 0.9em;
  }

  .error {
    color: var(--danger);
    margin: 0;
  }

  .screen-out,
  .new-night {
    align-self: flex-start;
    font-size: 0.85em;
  }

  .look {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    font-size: 0.9em;
  }

  .look label {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
  }

  .look .hint {
    margin: 0;
  }
  .phones {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  .badge {
    display: inline-block;
    margin-left: 0.4em;
    padding: 0 0.4em;
    border-radius: 0.3em;
    font-size: 0.7em;
    vertical-align: middle;
    border: 1px solid currentColor;
    opacity: 0.8;
  }
  .badge.ready {
    color: var(--success);
    opacity: 1;
  }
  .asks {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.4rem;
    margin: 0.25rem 0;
  }
  .asks span,
  .asked {
    color: var(--warn);
  }
  .line-field {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }
  .photos {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }
  .photos li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.5rem;
  }
  .photos img {
    width: 4.5rem;
    height: 4.5rem;
    object-fit: cover;
    border-radius: 0.3rem;
  }
</style>
