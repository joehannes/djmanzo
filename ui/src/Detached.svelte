<script lang="ts">
  /**
   * One panel, in a window of its own.
   *
   * The shell a detached window mounts instead of the whole application. Same
   * process, same state, same snapshot — Tauri's `emit` reaches every window,
   * so this draws from exactly the sixty-times-a-second stream the main window
   * draws from and there is no second path to keep in step.
   *
   * It deliberately renders *only* the panel. A detached browser that also
   * carried a set of decks would be a second application, and the DJ would have
   * two of everything to keep straight.
   */
  import Assistant from "./Assistant.svelte";
  import AtHand from "./AtHand.svelte";
  import Automix from "./Automix.svelte";
  import Browse from "./Browse.svelte";
  import Controllers from "./Controllers.svelte";
  import Crowd from "./Crowd.svelte";
  import EventPanel from "./Event.svelte";
  import Fx from "./Fx.svelte";
  import MappingEditor from "./MappingEditor.svelte";
  import Mic from "./Mic.svelte";
  import Mixes from "./Mixes.svelte";
  import Next from "./Next.svelte";
  import Night from "./Night.svelte";
  import Pair from "./Pair.svelte";
  import Plan from "./Plan.svelte";
  import Plugin from "./Plugin.svelte";
  import Practice from "./Practice.svelte";
  import Presets from "./Presets.svelte";
  import PressKit from "./PressKit.svelte";
  import Requests from "./Requests.svelte";
  import RoomSense from "./RoomSense.svelte";
  import Sampler from "./Sampler.svelte";
  import SessionLog from "./SessionLog.svelte";
  import Settings from "./Settings.svelte";
  import Shortcuts from "./Shortcuts.svelte";
  import SideView from "./SideView.svelte";
  import SingerScreen from "./SingerScreen.svelte";
  import Singers from "./Singers.svelte";
  import Watershed from "./Watershed.svelte";
  import Waveform from "./Waveform.svelte";
  import {
    activeDevice,
    attachPanel,
    chosenPack,
    cockpitWorkspace,
    controlMappings,
    controlStatus,
    dispatch,
    getSnapshot,
    interfaceDensity,
    interfaceSettings,
    liveEvent,
    onCockpitWorkspace,
    onInterfaceDensity,
    onInterfaceSettings,
    onKeyboardEnabled,
    onKnowledgePack,
    onLiveEvent,
    onSnapshot,
    setLiveEvent,
    setToolbars,
    windowAsk,
    type MappingInfo,
    type Snapshot,
  } from "./api";
  import { Keyboard } from "./keyboard.svelte";
  import { emptyWorld, getWorld, type World } from "./world";

  let { panel }: { panel: string } = $props();

  let snapshot = $state<Snapshot | null>(null);
  let error = $state<string | null>(null);

  const ready = $derived((snapshot?.master.sample_rate ?? 0) > 0);
  const deckCount = $derived(snapshot?.decks.length ?? 2);

  $effect(() => {
    const unlisten = onSnapshot((next) => {
      snapshot = next;
    });
    // The stream only emits on change, so a quiet startup would leave this
    // blank until the DJ touched something.
    void getSnapshot()
      .then((initial) => {
        snapshot ??= initial;
      })
      // A detached window with no first snapshot is a blank second screen,
      // and the DJ is looking at it precisely because it is not the first.
      .catch((problem) => {
        error = `the engine did not answer: ${problem}`;
      });
    return () => {
      void unlisten.then((fn) => fn());
    };
  });

  function send(action: string) {
    void dispatch(action).catch((e) => (error = String(e)));
  }

  /*
    The watershed is drawn from the world, not from the snapshot, and the world
    is pulled rather than pushed — twenty times a second, because it carries
    rates and the renderer interpolates between reads. Polled only while this
    window is actually showing it.
  */
  let world = $state<World>(emptyWorld());
  $effect(() => {
    if (panel !== "watershed") return;
    let alive = true;
    const tick = async () => {
      while (alive) {
        try {
          // `?? emptyWorld()` for the reason `App` gives: a null answer is the
          // same failure as a thrown one, arriving through the other door, and
          // it used to reach `world.entities` and take the window down.
          world = (await getWorld()) ?? emptyWorld();
        } catch {
          // A world we could not read is not worth an error in a booth; the
          // last one stays on screen until a read succeeds.
        }
        await new Promise((r) => setTimeout(r, 50));
      }
    };
    void tick();
    return () => {
      alive = false;
    };
  });

  /*
    §121: the night and requests need something of the main window's — the
    density band it is wearing, the collection it searches — and ask it
    through Rust, which checks the request and hands it to the main window as
    a key would (`window_ask`). The band is mirrored the other way: the main
    window says which it wears and every window is told.
  */
  let mainDensity = $state<string | undefined>(undefined);
  $effect(() => {
    if (panel !== "night") return;
    const unlisten = onInterfaceDensity((name) => (mainDensity = name));
    void interfaceDensity()
      .then((name) => (mainDensity ??= name))
      .catch(() => (mainDensity ??= "Standard"));
    return () => {
      void unlisten.then((fn) => fn());
    };
  });

  function ask(run: string) {
    void windowAsk(run).catch((e) => (error = String(e)));
  }

  /*
    The event being played is Rust's, and Rust tells every window when it
    changes; playing one here also takes the main window to the decks and
    the mixer, as it does there.
  */
  let liveId = $state<string | null>(null);
  $effect(() => {
    if (panel !== "event") return;
    const unlisten = onLiveEvent((id) => (liveId = id));
    void liveEvent()
      .then((id) => (liveId = id))
      .catch(() => {});
    return () => {
      void unlisten.then((fn) => fn());
    };
  });

  async function goLive(id: string) {
    try {
      liveId = await setLiveEvent(id);
    } catch (e) {
      error = String(e);
      return;
    }
    ask("switch activity mix");
  }

  /*
    The key map: the keys are read here to be shown, not acted on — this
    window listens to none of them. The switch is the main window's, kept by
    Rust, which tells every window when it flips; a key of the DJ's own,
    changed here, has the main window read its keys again.
  */
  const keys = new Keyboard();
  $effect(() => {
    if (panel !== "keys") return;
    void keys.load().catch((e) => (error = String(e)));
    void controlStatus()
      .then((status) => (keys.enabled = status.keyboard))
      .catch(() => {});
    const unlisten = onKeyboardEnabled((on) => (keys.enabled = on));
    return () => {
      void unlisten.then((fn) => fn());
    };
  });

  /** The controller mappings, for the controllers and the mapping editor. */
  let mappings = $state<MappingInfo[]>([]);
  $effect(() => {
    if (panel !== "controllers") return;
    void controlMappings()
      .then((got) => (mappings = got))
      .catch((e) => (error = String(e)));
  });

  /*
    The settings. What Rust holds — the toolbars, the knowledge pack, the
    arrangement's locks as last written — is read here and followed, since
    Rust tells every window when it changes. What only the main window can
    do — store the locks on its arrangement, open a preset's arrangement and
    theme, open the welcome — is asked of it. The theme itself is followed
    by every window through its own storage (`theme.svelte`).
  */
  let locked = $state<string[]>([]);
  let toolbars = $state(false);
  $effect(() => {
    if (panel !== "settings") return;
    void cockpitWorkspace()
      .then((resolved) => (locked = [...resolved.workspace.locked]))
      .catch(() => {});
    void interfaceSettings()
      .then((settings) => (toolbars = settings.toolbars))
      .catch(() => {});
    const unwatchWorkspace = onCockpitWorkspace((workspace) => (locked = [...workspace.locked]));
    const unwatchSettings = onInterfaceSettings((settings) => (toolbars = settings.toolbars));
    return () => {
      void unwatchWorkspace.then((fn) => fn());
      void unwatchSettings.then((fn) => fn());
    };
  });

  /** The open device's width, read again whenever the engine comes back. */
  let channels = $state<number | null>(null);
  $effect(() => {
    if (panel !== "settings" || !ready) return;
    void snapshot?.master.sample_rate;
    void activeDevice()
      .then((device) => (channels = device?.channels ?? null))
      .catch(() => {});
  });

  /** The knowledge pack the Next rail ranks by, as Rust holds it. */
  let pack = $state("");
  $effect(() => {
    if (panel !== "next") return;
    void chosenPack()
      .then((held) => (pack = held))
      .catch(() => {});
    const unlisten = onKnowledgePack((held) => (pack = held));
    return () => {
      void unlisten.then((fn) => fn());
    };
  });

  /** The decks with a river in the world, in order — as `App` picks them. */
  const rivers = $derived(
    world.entities
      .filter((e) => e.name === "deck.river" && e.index <= deckCount)
      .map((e) => e.index),
  );
</script>

<main class="detached" data-panel={panel}>
  {#if error}
    <p class="error" role="alert">{error}</p>
  {/if}

  {#if panel === "browser"}
    <Browse enabled={ready} {deckCount} />
  {:else if panel === "assistant"}
    <Assistant enabled={ready} />
  {:else if panel === "plan"}
    <Plan enabled={ready} />
  {:else if panel === "practice"}
    <Practice enabled={ready} />
  {:else if panel === "kit"}
    <PressKit />
  {:else if panel === "crowd"}
    <Crowd />
  {:else if panel === "room"}
    <RoomSense enabled={ready} />
  {:else if panel === "mixes"}
    <Mixes enabled={ready} />
  {:else if panel === "presets"}
    <Presets enabled={ready} {deckCount} />
  {:else if panel === "athand"}
    <AtHand enabled={ready} {send} />
  {:else if panel === "log"}
    <SessionLog />
  {:else if panel === "night"}
    <!--
      Not drawn until the main window's band is known: the night notes the
      density it is played at, and a guess here would be a wrong note.
    -->
    {#if mainDensity !== undefined}
      <Night enabled={ready} density={mainDensity} onDensity={(slug) => ask(`density ${slug}`)} />
    {/if}
  {:else if panel === "event"}
    <EventPanel {liveId} onGoLive={(id) => void goLive(id)} />
  {:else if panel === "keys"}
    <!-- Its own close is the window's: the panel goes home, as it does there. -->
    <Shortcuts
      keyboard={keys}
      onclose={() => void attachPanel("keys")}
      decks={deckCount}
      onKeysChanged={() => ask("keys changed")}
    />
  {:else if panel === "controllers"}
    <div class="stack">
      <Controllers {mappings} />
      <MappingEditor {mappings} />
    </div>
  {:else if panel === "settings"}
    <Settings
      onLogoChange={() => {}}
      deviceChannels={channels}
      {locked}
      onLock={(next) => {
        locked = next;
        ask(`locks ${next.join(" ")}`.trim());
      }}
      onSetUp={(workspace, theme) => ask(`setup ${theme || "-"} ${workspace}`)}
      onWelcome={() => ask("welcome")}
      {toolbars}
      onToolbars={(on) =>
        void setToolbars(on)
          .then((settings) => (toolbars = settings.toolbars))
          .catch((e) => (error = String(e)))}
    />
  {:else if panel === "requests"}
    <!-- What a guest typed can run long; the collection's search needs the start of it. -->
    <Requests enabled={ready} onFind={(text) => ask(`find ${text.trim().slice(0, 200)}`)} />
  {:else if snapshot}
    {#if panel === "waveforms"}
      <!--
        Every deck's lane, stacked and large. This is what a second screen is
        actually for: the waveform is the thing a DJ looks at continuously, and
        it is the thing that has least room in the main window.
      -->
      <div class="lanes">
        {#each snapshot.decks as deck (deck.number)}
          <section class="lane">
            <header>
              <span class="number">{deck.number}</span>
              <span class="title">{deck.title ?? "—"}</span>
              {#if deck.effective_bpm != null}
                <span class="bpm">{deck.effective_bpm.toFixed(1)}</span>
              {/if}
            </header>
            <Waveform {deck} height={140} />
          </section>
        {/each}
      </div>
    {:else if panel === "fx"}
      <div class="racks">
        {#each snapshot.decks as deck (deck.number)}
          <section class="rack">
            <h3>Deck {deck.number}</h3>
            <Fx slots={deck.fx} enabled={ready} target={`deck ${deck.number}`} {send} />
          </section>
        {/each}
        <section class="rack">
          <h3>Master</h3>
          <Fx slots={snapshot.master.fx} enabled={ready} target="master" {send} />
        </section>
      </div>
    {:else if panel === "singers"}
      <SingerScreen decks={snapshot.decks} />
    {:else if panel === "karaoke"}
      <Singers enabled={ready} {deckCount} decks={snapshot.decks} vocals={snapshot.master.vocals} />
    {:else if panel === "prepare"}
      <SideView enabled={ready} {deckCount} decks={snapshot.decks} />
    {:else if panel === "next"}
      <Next enabled={ready} {deckCount} decks={snapshot.decks} {pack} />
    {:else if panel === "pair"}
      <Pair enabled={ready} {deckCount} decks={snapshot.decks} />
    {:else if panel === "booth"}
      <!-- The things set up once a night: as the main window's booth has them. -->
      <Mic mic={snapshot.master.mic} enabled={ready} {send} />
      <Automix automix={snapshot.master.automix} enabled={ready} {send} />
      <Plugin clap={snapshot.master.clap} enabled={ready} {send} />
      <section class="rack">
        <h3>Master FX</h3>
        <Fx slots={snapshot.master.fx} enabled={ready} target="master" {send} />
      </section>
    {:else if panel === "sampler"}
      <Sampler sampler={snapshot.master.sampler} enabled={ready} {send} />
    {:else if panel === "watershed"}
      {#if rivers.length > 0}
        <Watershed
          {world}
          decks={rivers}
          latencyMs={snapshot.master.output_latency_ms}
        />
      {:else}
        <p class="waiting">Nothing playing.</p>
      {/if}
    {:else}
      <p class="waiting">No panel called “{panel}”.</p>
    {/if}
  {:else}
    <p class="waiting">Waiting for the engine…</p>
  {/if}
</main>

<style>
  /* The controllers and the mapping editor, one under the other; the window
     scrolls them together. */
  .stack {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .detached {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    height: 100vh;
    padding: 0.6rem;
    box-sizing: border-box;
    overflow: auto;
    background: var(--bg);
    color: var(--fg);
  }

  /* A detached panel fills its window. That is the whole reason it is in one. */
  .detached > :global(*) {
    max-width: none;
  }

  .lanes,
  .racks {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    min-height: 0;
  }

  .racks {
    flex-direction: row;
    flex-wrap: wrap;
    align-items: flex-start;
  }

  .lane header,
  .rack h3 {
    display: flex;
    align-items: baseline;
    gap: 0.5rem;
    margin: 0 0 0.2rem;
    font-size: 0.72rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--muted);
  }

  .lane .number {
    font-weight: 700;
    color: var(--fg);
  }

  .lane .title {
    text-transform: none;
    letter-spacing: 0;
  }

  .lane .bpm {
    margin-left: auto;
    font-variant-numeric: tabular-nums;
  }

  .waiting,
  .error {
    margin: auto;
    font-size: 0.85rem;
    color: var(--muted);
  }

  .error {
    color: var(--warn, #d4756b);
  }
</style>
