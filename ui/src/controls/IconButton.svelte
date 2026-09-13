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

  /**
   * How this button says it is on.
   *
   * `aria-pressed` on a toggle, and *nothing* when the caller has given the
   * button a role that forbids it. §33's audit found the browser's two tabs
   * reporting `aria-pressed` on `role="tab"`, which is not a warning about
   * tidiness: an attribute a role prohibits is dropped by assistive technology
   * along with whatever it was saying, so the pair announced neither which one
   * was open nor that either could be pressed. The role brings its own way of
   * saying it -- `aria-selected` -- and the call site already passes that.
   *
   * Spread rather than written inline because an attribute set to `undefined`
   * and an attribute absent are the same to Svelte only for the first case;
   * this keeps the two rules in one place where the next role that prohibits it
   * can be added to the list.
   */
  const PROHIBIT_PRESSED = ["tab", "menuitem", "option", "treeitem", "radio"];
  const pressed = $derived(
    active === undefined || PROHIBIT_PRESSED.includes(String(rest.role ?? ""))
      ? {}
      : { "aria-pressed": active },
  );
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
  {...pressed}
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
  /*
    The hover, for a button that is already on.

    Without this rule the plain hover above wins on specificity, so putting the
    pointer on an open panel's button replaced the accent fill with the dark
    hover fill and left the `--on-accent` text -- near-black -- sitting on it.
    1.23:1. The label of the panel you just opened disappeared for as long as
    your hand stayed where it was, which is most of the time.

    §33's audit found it; nothing else could have. `svelte-check` has no opinion
    about contrast, a screenshot test would have had to be taking the shot with
    the pointer parked, and the only reason it never came up in use is that the
    button you are hovering is the one you already know the name of.
  */
  .icon-button.active:hover:not(:disabled) {
    background: color-mix(in srgb, var(--active) 88%, white);
  }
  /*
    §30's `active` and `selected`, which that section lists separately and
    which `Role::must_differ_from` names as a pair that has to stay
    distinguishable: a panel that is *open* and a control the DJ has *picked*
    are two different facts, and they sit next to each other constantly.

    The fill and the ring were `--accent` and `--accent-2` already; naming them
    is the change, so the pair is now answerable to the rule instead of being
    two tokens that happened to differ.
  */
  .icon-button.active {
    background: var(--active);
    color: var(--on-accent);
    border-color: var(--selected);
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
