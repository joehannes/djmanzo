<script lang="ts">
  /**
   * Choosing a theme by the room you are in.
   *
   * The previous picker was a row of unlabelled icons. It could tell you a
   * theme was called "Cyber Trance" and nothing about whether it would be
   * readable where you were standing — which is the only question anybody
   * actually has. So the themes are grouped by setting, each says when to reach
   * for it, and each shows a swatch of its own real colours rather than a
   * guessed icon.
   *
   * The swatch is drawn from `paletteFor`, so it cannot drift from what the
   * theme applies: it is the same table.
   */
  import { clickOutside } from "./controls/clickOutside";
  import { theme } from "./theme.svelte";
  import { themePackages } from "./controls/themes/packages";
  import type { ThemeSetting } from "./controls/themes/engine";
  import { paletteFor } from "./controls/themes/colors";
  import IconButton from "./controls/IconButton.svelte";

  let open = $state(false);

  function toggle() {
    open = !open;
  }

  /**
   * The settings, in the order a day runs.
   *
   * Not alphabetical and not by popularity: desk, evening, booth, room is the
   * order a DJ moves through, so the list reads as a journey rather than a
   * catalogue.
   */
  const groups: { setting: ThemeSetting; label: string; hint: string }[] = [
    { setting: "daylight", label: "Daylight", hint: "Bright rooms and outdoors" },
    { setting: "home", label: "At home", hint: "Preparing, at a desk" },
    { setting: "booth", label: "In the booth", hint: "Dark, mid-set" },
    { setting: "venue", label: "The room", hint: "A crowd is watching" },
  ];

  const bySetting = (setting: ThemeSetting) =>
    themePackages.filter((pkg) => pkg.setting === setting);

  // The swatch follows the *resolved* appearance, so a theme previewed in light
  // mode shows its light colours. Previewing the dark ones while the app is
  // white would be showing a theme nobody is about to get.
  const swatch = (id: string) => paletteFor(id, theme.resolved);
</script>

<div class="switcher" use:clickOutside={() => (open = false)}>
  <button class="icon" onclick={toggle} title="Theme">
    <span
      class="dot"
      style="background: {swatch(theme.activePackage.id)['--accent']}"
    ></span>
    {theme.activePackage.name}
  </button>

  {#if open}
    <div class="menu">
      <div class="section">
        <strong>Light and dark</strong>
        <p class="hint">
          Dark by default, because a white screen at eye level in a dark room
          costs the night vision you need to find anything on the mixer.
        </p>
        <div class="opts">
          <IconButton
            icon="fa-solid fa-moon"
            title="Dark"
            active={theme.preference === "dark"}
            onClick={() => theme.set("dark")}
          />
          <IconButton
            icon="fa-solid fa-sun"
            title="Light"
            active={theme.preference === "light"}
            onClick={() => theme.set("light")}
          />
          <IconButton
            icon="fa-solid fa-desktop"
            title="Follow the system"
            active={theme.preference === "system"}
            onClick={() => theme.set("system")}
          />
        </div>
      </div>

      {#each groups as group (group.setting)}
        {@const packages = bySetting(group.setting)}
        {#if packages.length > 0}
          <div class="section">
            <strong>{group.label}</strong>
            <span class="group-hint">{group.hint}</span>
            <div class="themes">
              {#each packages as pkg (pkg.id)}
                {@const colours = swatch(pkg.id)}
                <button
                  class="theme"
                  class:active={theme.activePackage.id === pkg.id}
                  onclick={() => theme.setPackage(pkg.id)}
                  title={pkg.when}
                >
                  <span
                    class="preview"
                    style="background: {colours['--panel']}; border-color: {colours['--border-strong']}"
                  >
                    <span style="background: {colours['--accent']}"></span>
                    <span style="background: {colours['--accent-2']}"></span>
                    <span style="background: {colours['--text']}"></span>
                  </span>
                  <span class="label">
                    <span class="name">{pkg.name}</span>
                    <span class="when">{pkg.when}</span>
                  </span>
                </button>
              {/each}
            </div>
          </div>
        {/if}
      {/each}
    </div>
  {/if}
</div>

<style>
  .switcher {
    position: relative;
  }

  .icon {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    background: var(--panel-raised);
    border: 1px solid var(--border);
    color: var(--text);
    padding: 0.35rem 0.6rem;
    border-radius: 6px;
  }

  .dot {
    width: 0.6rem;
    height: 0.6rem;
    border-radius: 50%;
    flex: none;
  }

  .menu {
    position: absolute;
    /* Anchored to the button's left edge, not its right. The switcher sits at
       the left of the header, so a right-anchored menu opened off the side of
       the window and half of it was unreachable. */
    left: 0;
    top: 2.6rem;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0.7rem;
    width: 22rem;
    max-height: min(70vh, 32rem);
    overflow: auto;
    box-shadow: 0 6px 18px var(--scrim);
    z-index: 40;
  }

  .section {
    margin-bottom: 0.8rem;
  }

  .group-hint {
    color: var(--text-dim);
    font-size: 0.75em;
    margin-left: 0.4rem;
  }

  .opts {
    display: flex;
    gap: 0.4rem;
    flex-wrap: wrap;
    margin-top: 0.4rem;
  }

  .hint {
    color: var(--text-dim);
    font-size: 0.75em;
    margin: 0.25rem 0 0;
  }

  .themes {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    margin-top: 0.4rem;
  }

  .theme {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    width: 100%;
    text-align: left;
    padding: 0.4rem;
    border-radius: 6px;
    border: 1px solid transparent;
    background: transparent;
    color: var(--text);
  }

  .theme:hover:not(:disabled) {
    background: var(--panel-hover);
  }

  .theme.active {
    border-color: var(--accent);
    background: var(--panel-raised);
  }

  /* The swatch shows the theme's own panel, accents and text colour, so what a
     theme looks like is visible before it is applied -- and it cannot drift,
     because it is drawn from the same table the theme applies. */
  .preview {
    display: flex;
    gap: 2px;
    align-items: center;
    padding: 4px;
    border-radius: 4px;
    border: 1px solid;
    flex: none;
  }

  .preview span {
    width: 0.55rem;
    height: 1rem;
    border-radius: 2px;
  }

  .label {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .name {
    font-size: 0.85em;
  }

  .when {
    color: var(--text-dim);
    font-size: 0.7em;
  }
</style>
