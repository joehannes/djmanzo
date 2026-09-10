<script lang="ts">
  /**
   * §20's second view: the collection as cards.
   *
   * "Use when album/artwork is valuable. Each card should still be
   * **operational**." Both halves matter and the second is the one that makes
   * this more than a picture wall — a card a DJ can look at but not act on is
   * a worse table.
   *
   * # Why this could not exist until now
   *
   * djmanzo read every tag except the pictures, so there was no artwork for a
   * card to be about. `dj_library::tags::artwork` is the read and `art://` is
   * how it arrives — as an ordinary image the webview loads and caches, not
   * base64 through the bridge.
   *
   * # The operations, and the three names djmanzo has one gesture for
   *
   * §20 lists *stage*, *add to prepare* and *queue* as separate card actions.
   * In djmanzo they are one thing: **set aside** puts a record in the Prepare
   * space, and the Sidelist is what Prepare holds. Three buttons doing one
   * thing would be three chances to wonder which one you wanted — see §21 on
   * Prepare being first class, and the inconsistent-Prepare failure it names.
   *
   * **Two of §20's list are genuinely absent**, and are left out rather than
   * faked. *Preview* needs a player that auditions a record without a deck,
   * which djmanzo does not have — the same gap §27's ghost track waits on.
   * *Compare* puts two records side by side, and the pair view compares two
   * **decks**; reaching it from here would mean loading both first, which is
   * the deck buttons and not a separate control.
   *
   * *Favourite* is a five-star rating rather than a flag of its own. djmanzo
   * already rates tracks, and a second boolean meaning "I like this" would be
   * a parallel opinion about the same question.
   */
  import { artUrl, formatTime, type LibraryTrack } from "./api";

  let {
    tracks,
    enabled,
    deckNumbers,
    loading = null,
    /** Reasons, when the list being shown is a suggestion rather than a crate. */
    why = {},
    favourite,
    onDeck,
    onAside,
    onAlike,
    onFavourite,
    onDrag,
  }: {
    tracks: LibraryTrack[];
    enabled: boolean;
    deckNumbers: number[];
    loading?: string | null;
    why?: Record<string, string[]>;
    /** The track "more like this" is currently showing, if any. */
    favourite?: string | null;
    onDeck: (track: LibraryTrack, deck: number) => void;
    onAside: (id: string) => void;
    onAlike: (id: string) => void;
    onFavourite: (track: LibraryTrack) => void;
    onDrag: (event: DragEvent, track: { id: string }) => void;
  } = $props();

  /**
   * Which covers failed to load.
   *
   * Driven by the image's own error rather than by asking first: a track with
   * no cover answers 404, and one round trip that the browser was going to
   * make anyway is cheaper than a command per card to find out. Cards whose
   * cover is missing draw a sleeve made from what djmanzo does know.
   */
  let missing = $state<Record<string, boolean>>({});

  /**
   * A colour for a record with no cover.
   *
   * From its key, because that is the one thing about a record a DJ already
   * reads as a colour — the Camelot wheel is a colour wheel. A record with no
   * key gets the neutral panel, which is honest: nothing is known about it.
   */
  function hue(track: LibraryTrack): string | null {
    const hour = Number(track.key?.match(/^(\d{1,2})/)?.[1]);
    if (!Number.isFinite(hour) || hour < 1 || hour > 12) return null;
    return `hsl(${(hour - 1) * 30} 45% 30%)`;
  }
</script>

<div class="cards">
  {#each tracks as track (track.id)}
    <article
      class="card"
      class:unanalysed={!track.analysed}
      draggable="true"
      ondragstart={(event) => onDrag(event, track)}
    >
      <div class="sleeve" style:background={missing[track.id] ? (hue(track) ?? "") : ""}>
        {#if !missing[track.id]}
          <img
            src={artUrl(track.id)}
            alt=""
            loading="lazy"
            decoding="async"
            onerror={() => (missing = { ...missing, [track.id]: true })}
          />
        {:else}
          <!--
            Not a placeholder icon repeated fifty times, which is a wall of the
            same shape. The key and the tempo are what a DJ would have read off
            the sleeve anyway, and they differ per record.
          -->
          <span class="stand-in mono">
            {track.key ?? "—"}
            <em>{track.bpm != null ? track.bpm.toFixed(0) : "?"}</em>
          </span>
        {/if}
      </div>

      <div class="says">
        <strong class="title" title={track.title}>{track.title}</strong>
        <span class="artist" title={track.artist}>{track.artist}</span>
        <span class="facts mono">
          {track.bpm != null ? `${track.bpm.toFixed(0)} BPM` : "—"}
          · {track.key ?? "—"}
          · {formatTime(track.duration_seconds)}
        </span>
        {#if why[track.id]?.length}
          <!--
            §20's "why suggested", and it is on the card rather than in a
            tooltip: a suggestion whose reason has to be hovered for is a
            suggestion a DJ takes on trust, which is the opposite of what the
            reasons are for.
          -->
          <span class="why">
            {#each why[track.id] as reason (reason)}
              <span class="chip">{reason}</span>
            {/each}
          </span>
        {/if}
      </div>

      <div class="does">
        <button
          class="aside"
          onclick={() => onAside(track.id)}
          title="Set aside in Prepare"
          aria-label="Set aside {track.title}"
        >→</button>
        <button
          class="alike"
          onclick={() => onAlike(track.id)}
          title="Find records like this"
          aria-label="Find records like {track.title}"
        >≈</button>
        <button
          class="star"
          class:on={(track.rating ?? 0) >= 5}
          onclick={() => onFavourite(track)}
          title={(track.rating ?? 0) >= 5 ? "Not a favourite any more" : "Make it a favourite"}
          aria-label="Favourite {track.title}"
          aria-pressed={(track.rating ?? 0) >= 5}
        >★</button>
        <span class="decks">
          {#each deckNumbers as deck (deck)}
            <!--
              Labelled, because the visible text is a bare number. "1" is
              enough for an eye that can see the row it is in and tells a
              screen reader nothing at all — §33.
            -->
            <button
              onclick={() => onDeck(track, deck)}
              disabled={!enabled || loading === track.path}
              title="Load onto deck {deck}"
              aria-label="Load {track.title} onto deck {deck}"
            >{loading === track.path ? "…" : deck}</button>
          {/each}
        </span>
      </div>
      {#if favourite === track.id}<span class="showing"></span>{/if}
    </article>
  {/each}
</div>

<style>
  /*
    As many as fit, at a size where a sleeve is worth looking at. `auto-fill`
    rather than a column count: this view docks into a panel that is 380 px
    wide beside the decks and 1200 px along the bottom, and a fixed count would
    be either two enormous cards or twelve unreadable ones.
  */
  .cards {
    display: grid;
    /*
      A ceiling as well as a floor. `minmax(140px, 1fr)` alone gives a narrow
      panel one column that takes the whole width, and a square sleeve then
      makes a single card taller than the panel it is in — which is what the
      browser docked along the bottom actually did: one enormous cover, cut
      off, with nothing else reachable. Capped, a narrow panel gets one modest
      card and a wide one gets six.
    */
    grid-template-columns: repeat(auto-fill, minmax(140px, 190px));
    gap: 0.6rem;
    align-content: start;
    overflow-y: auto;
    padding-bottom: 0.4rem;
  }

  .card {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    padding: 0.4rem;
    border: 1px solid var(--edge);
    border-radius: var(--radius);
    background: var(--panel-raised);
    min-width: 0;
  }

  /* The same signal the table uses for a track sync cannot trust yet. */
  .card.unanalysed {
    opacity: 0.72;
  }

  .sleeve {
    position: relative;
    aspect-ratio: 1;
    /*
      Square until it would cost more height than the panel has. A sleeve is
      worth looking at; it is not worth being the only thing on screen, and
      `object-fit: cover` means the crop is the middle of the artwork rather
      than a squash.
    */
    max-height: 130px;
    display: grid;
    place-items: center;
    overflow: hidden;
    border-radius: 4px;
    background: var(--panel);
  }

  .sleeve img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .stand-in {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.1rem;
    font-size: 1.1rem;
    font-weight: 700;
    color: var(--text);
    text-shadow: 0 1px 2px rgb(0 0 0 / 0.5);
  }

  .stand-in em {
    font-style: normal;
    font-size: 0.7rem;
    font-weight: 400;
    opacity: 0.8;
  }

  .says {
    display: flex;
    flex-direction: column;
    gap: 0.05rem;
    min-width: 0;
  }

  .title,
  .artist {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 0.78rem;
  }

  .artist {
    color: var(--text-dim);
    font-size: 0.72rem;
  }

  .facts {
    font-size: 0.68rem;
    color: var(--muted);
  }

  .why {
    display: flex;
    flex-wrap: wrap;
    gap: 0.2rem;
    margin-top: 0.15rem;
  }

  .chip {
    font-size: 0.62rem;
    line-height: 1.4;
    padding: 0 0.3rem;
    border-radius: 0.25rem;
    background: var(--chip, rgba(128, 128, 128, 0.18));
    white-space: nowrap;
  }

  /*
    The operations. Every card carries them rather than revealing them on
    hover: a booth is dark, a trackpad is small, and a control that appears
    when the pointer is already on it is a control nobody finds.
  */
  .does {
    display: flex;
    align-items: center;
    gap: 0.2rem;
    margin-top: auto;
  }

  .does button {
    font-size: 0.68rem;
    padding: 0 0.35rem;
    min-width: 1.4rem;
  }

  .decks {
    display: flex;
    gap: 0.15rem;
    margin-left: auto;
  }

  .star.on {
    color: var(--accent);
  }
</style>
