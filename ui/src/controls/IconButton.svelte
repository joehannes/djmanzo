<script lang="ts">
  export let icon: string;
  export let title: string | null = null;
  export let active: boolean = false;
  export let disabled: boolean = false;
  export let onClick: () => void = () => {};
</script>

<button class:active on:click={() => !disabled && onClick()} {disabled} title={title} aria-pressed={active}>
  <i class={icon} aria-hidden="true"></i>
</button>

<style>
  button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 2.4rem;
    height: 2.4rem;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--panel-raised);
    color: var(--text);
    cursor: pointer;
    padding: 0;
  }
  button:hover:not(:disabled) { background: var(--panel-hover); }
  button.active { background: var(--accent); color: var(--on-accent); border-color: var(--accent-2); }
  button:disabled { opacity: 0.4; cursor: not-allowed; }
  i { font-size: 1.05rem; }
</style>
