<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon from "./Icon.svelte";

  /**
   * A square button carrying an icon, or whatever the caller renders inside it.
   *
   * Written in runes like the rest of the interface. It began as Svelte 4
   * (`export let`, `on:click`, `<slot>`), which type-checked as a component
   * whose `icon` was required and whose children were not a prop at all -- so
   * every call site that passed a label instead of an icon, or a pointer
   * handler instead of `onClick`, was an error.
   */
  let {
    icon = null,
    label = null,
    title = null,
    active = undefined,
    disabled = false,
    onClick,
    class: extra = "",
    children,
    ...rest
  }: {
    /** A Font Awesome class, when the button is just an icon. */
    icon?: string | null;
    /**
     * Text beside the icon.
     *
     * Not decoration. A row of identical grey squares is unreadable without
     * hovering every one of them, and a DJ mid-set is not going to hover: the
     * control that opens the browser has to be findable at a glance, and it
     * cannot be if it looks exactly like the one that shows keyboard
     * shortcuts. Where a control has a name, say it.
     *
     * Left `null` where the icon really is unambiguous *in its context* -- the
     * folder on a deck's own header, the play triangle -- because a label
     * there is noise on a surface that has none to spare.
     */
    label?: string | null;
    title?: string | null;
    /**
     * Whether this is a toggle, and whether it is on.
     *
     * Left `undefined` for plain action buttons on purpose: a button that
     * reports `aria-pressed="false"` is telling a screen reader it is a
     * switch that happens to be off, which is a different thing from a button
     * that does something.
     */
    active?: boolean | undefined;
    disabled?: boolean;
    /** Returns `unknown` so an `async` handler is as welcome as a plain one. */
    onClick?: () => unknown;
    class?: string;
    children?: Snippet;
    [key: string]: unknown;
  } = $props();
</script>

<!--
  `...rest` goes last so a call site can override what is set here -- its own
  `aria-label`, or a pointer handler in place of the click.
-->
<button
  class="icon-button {extra}"
  class:active={active === true}
  {disabled}
  {title}
  aria-pressed={active === undefined ? undefined : active}
  onclick={() => {
    if (!disabled) onClick?.();
  }}
  {...rest}
>
  {#if children}
    {@render children()}
  {:else}
    {#if icon}
      <Icon name={icon} />
    {/if}
    {#if label}
      <span class="label">{label}</span>
    {/if}
  {/if}
</button>

<style>
  .icon-button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 2.6rem;
    height: 2.6rem;
    border-radius: var(--radius);
    border: 1px solid var(--edge);
    background: var(--panel-raised);
    color: var(--text);
    cursor: pointer;
    padding: 0 0.4rem;
    gap: 0.4rem;
    white-space: nowrap;
    transition:
      background 120ms ease,
      border-color 120ms ease,
      transform 100ms ease;
  }
  .icon-button:hover:not(:disabled) {
    background: var(--panel-hover);
  }
  .icon-button.active {
    background: var(--accent);
    color: var(--on-accent);
    border-color: var(--accent-2);
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.45);
  }
  .label {
    font-size: 0.8em;
    /* Trails the icon's optical edge rather than sitting flush, so the pair
       reads as one control instead of two things that happen to touch. */
    padding-right: 0.15rem;
  }

  .icon-button:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
</style>
