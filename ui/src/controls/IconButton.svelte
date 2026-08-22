<script lang="ts">
  export let icon: string | null = null;
  export let title: string | null = null;
  export let active: boolean = false;
  export let disabled: boolean = false;
  export let onClick: (() => void) | null = null;
  // forward any other attributes (role, aria-*, id, etc.) via Svelte's $$restProps
</script>

<button {...$$restProps} class:active on:click={() => !disabled && onClick?.()} {disabled} title={title} aria-pressed={active}>
  <slot>
    {#if icon}
      <i class={icon} aria-hidden="true"></i>
    {/if}
  </slot>
</button>

<style>
  button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 2.6rem;
    height: 2.6rem;
    border-radius: var(--radius);
    border: 1px solid var(--edge);
    background: var(--panel-raised);
    color: var(--text);
    cursor: pointer;
    padding: 0;
    transition: background 120ms ease, border-color 120ms ease, transform 100ms ease;
  }
  button:hover:not(:disabled) { background: var(--panel-hover); }
  button.active { background: var(--accent); color: var(--on-accent); border-color: var(--accent-2); box-shadow: 0 8px 24px rgba(0,0,0,0.45); }
  button:disabled { opacity: 0.45; cursor: not-allowed; }
  i { font-size: 1.05rem; }
</style>
