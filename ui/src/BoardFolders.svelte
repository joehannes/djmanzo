<script lang="ts">
  /**
   * §121: the press kit laid out like a desktop.
   *
   * > press kit desktop like folder organization
   *
   * A folder for each part of the kit, in the order a promoter reads one,
   * each opening that part where it is on the board — the kit beside it
   * scrolls to it — rather than a second copy of the kit's fields.
   */
  import Icon from "./controls/Icon.svelte";

  let { onopen }: { onopen: (section: string) => void } = $props();

  /** The kit's own headings, which are what a folder opens. */
  const FOLDERS = [
    { section: "Send", glyph: "paperclip", about: "Ready-made kits for a booking, a promoter, a festival" },
    { section: "Who you are", glyph: "hand", about: "Name, one line, where you are based, bio" },
    { section: "Your music", glyph: "compact-disc", about: "Genres, experience, sets" },
    { section: "Bookings", glyph: "calendar", about: "Fees and how to book you" },
    { section: "Where to find you", glyph: "location-dot", about: "Profiles and links" },
    { section: "Photos", glyph: "image", about: "Press photos" },
    { section: "Documents", glyph: "file-lines", about: "Rider, fact sheet, anything else" },
  ] as const;
</script>

<ul class="desktop" aria-label="Press kit folders">
  {#each FOLDERS as folder (folder.section)}
    <li>
      <button type="button" class="folder" title={folder.about} onclick={() => onopen(folder.section)}>
        <span class="icon" aria-hidden="true"><Icon name="folder-open" size="2.8rem" /><span class="inner"><Icon name={folder.glyph} size="1.05rem" /></span></span>
        <span class="name">{folder.section}</span>
      </button>
    </li>
  {/each}
</ul>

<style>
  .desktop {
    list-style: none;
    margin: 0;
    padding: 0.4rem;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(6.5rem, 1fr));
    gap: 0.6rem;
    align-content: start;
  }

  .folder {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.3rem;
    width: 100%;
    padding: 0.5rem 0.3rem;
    background: transparent;
    border: 1px solid transparent;
    border-radius: 8px;
    color: var(--text);
    cursor: pointer;
  }

  .folder:hover,
  .folder:focus-visible {
    background: var(--panel-hover);
    border-color: var(--border-strong);
  }

  .icon {
    position: relative;
    display: inline-grid;
    place-items: center;
    color: var(--text-dim);
  }

  .inner {
    position: absolute;
    right: -0.35rem;
    bottom: -0.2rem;
    display: inline-grid;
    place-items: center;
    padding: 0.15rem;
    border-radius: 999px;
    background: var(--panel-raised);
    color: var(--text);
  }

  .name {
    font-size: 0.8rem;
    text-align: center;
    line-height: 1.2;
  }
</style>
