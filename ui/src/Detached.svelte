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
  import Browse from "./Browse.svelte";
  import Fx from "./Fx.svelte";
  import Sampler from "./Sampler.svelte";
  import Watershed from "./Watershed.svelte";
  import Waveform from "./Waveform.svelte";
  import { dispatch, getSnapshot, onSnapshot, type Snapshot } from "./api";
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
          world = await getWorld();
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
    <Browse enabled={ready} {deckCount} decks={snapshot?.decks ?? []} />
  {:else if panel === "assistant"}
    <Assistant enabled={ready} />
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
