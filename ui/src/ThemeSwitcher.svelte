<script lang="ts">
  import { theme, type ThemePreference } from "./theme.svelte";
  import { themePackages } from "./controls/themes/packages";
  let open = $state(false);

  function toggle() {
    open = !open;
  }

  function choosePref(p: ThemePreference) {
    theme.set(p);
  }

  function choosePkg(id: string) {
    theme.setPackage(id);
  }
</script>

<div class="switcher" on:click_outside={() => (open = false)}>
  <button class="icon" onclick={toggle} title="Theme">
    {theme.activePackage.name} · {theme.resolved}
  </button>

  {#if open}
    <div class="menu">
      <div class="section">
        <strong>Appearance</strong>
        <div class="opts">
          <button class:active={theme.preference === 'dark'} onclick={() => choosePref('dark')}>Dark</button>
          <button class:active={theme.preference === 'light'} onclick={() => choosePref('light')}>Light</button>
          <button class:active={theme.preference === 'system'} onclick={() => choosePref('system')}>System</button>
        </div>
      </div>

      <div class="section">
        <strong>Package</strong>
        <div class="opts">
          {#each themePackages as pkg (pkg.id)}
            <button class:active={theme.activePackage.id === pkg.id} onclick={() => choosePkg(pkg.id)}>{pkg.name}</button>
          {/each}
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .switcher { position: relative; }
  .icon {
    background: var(--panel-raised);
    border: 1px solid var(--border);
    color: var(--text);
    padding: 0.35rem 0.6rem;
    border-radius: 6px;
  }
  .menu {
    position: absolute;
    right: 0;
    top: 2.6rem;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0.6rem;
    min-width: 220px;
    box-shadow: 0 6px 18px rgba(0,0,0,0.4);
    z-index: 40;
  }
  .section { margin-bottom: 0.5rem; }
  .opts { display:flex; gap:0.4rem; flex-wrap:wrap; margin-top:0.4rem }
  .opts button { padding: 0.4rem 0.6rem; border-radius:6px; border:1px solid var(--border); background:var(--panel-raised); color:var(--text); }
  .opts button.active { background: var(--accent); color: var(--on-accent); border-color: var(--accent-2); }
</style>
