<script lang="ts">
  /**
   * §121: how much of the chosen theme is worn — Basic, Parts or Full — and,
   * part by part, where each comes from.
   *
   * The palette is not on the list: it is what choosing a theme *is*. Each
   * part below can be the chosen theme's, another theme's, or off, and the
   * three settings above are the two ends of that and everything between.
   * The rule is `controls/themes/parts.ts`; this only draws it.
   */
  import { theme } from "./theme.svelte";
  import {
    EVENT_STYLES,
    PARTS,
    eventStyleOf,
    offered,
    type Amount,
    type PartName,
  } from "./controls/themes/parts";
  import { themePackages, type ThemePackage } from "./controls/themes/packages";

  const AMOUNTS: { id: Amount; label: string; hint: string }[] = [
    { id: "basic", label: "Basic", hint: "Its colours only: plain controls, standing still" },
    { id: "parts", label: "Parts", hint: "Its colours, and the parts you choose below" },
    { id: "full", label: "Full", hint: "Everything the theme has" },
  ];

  /** Whether a theme brings anything to a part, or would be the same as off. */
  const has = (pkg: ThemePackage, part: PartName): boolean =>
    part === "art" ? pkg.effects.length > 0 : part === "motion" ? pkg.behaviors.length > 0 : true;

  /** What a theme's part is, in a few words, where the name alone does not say. */
  const what = (pkg: ThemePackage, part: PartName): string =>
    part === "events" ? ` (${EVENT_STYLES[eventStyleOf(pkg)]})` : "";

  /**
   * The other themes worth offering: one of each kind, the chosen theme's
   * own kind claimed first so it is not offered twice under another name.
   */
  const others = (part: PartName): ThemePackage[] =>
    offered(part, [theme.chosen, ...themePackages.filter((pkg) => pkg.id !== theme.chosen.id)]).filter(
      (pkg) => pkg.id !== theme.chosen.id,
    );
</script>

<div class="parts">
  <div class="amount" role="radiogroup" aria-label="How much of the theme">
    {#each AMOUNTS as option (option.id)}
      <button
        type="button"
        role="radio"
        aria-checked={theme.amount === option.id}
        class:selected={theme.amount === option.id}
        title={option.hint}
        onclick={() => theme.setAmount(option.id)}
      >
        {option.label}
      </button>
    {/each}
  </div>

  <div class="rows">
    {#each PARTS as part (part.name)}
      <label class="row" for="theme-part-{part.name}" title={part.about}>{part.title}</label>
      <select
        id="theme-part-{part.name}"
        value={theme.parts[part.name]}
        onchange={(event) => theme.setPart(part.name, event.currentTarget.value)}
      >
        <option value="base">
          As in {theme.chosen.name}{has(theme.chosen, part.name) ? what(theme.chosen, part.name) : " — none"}
        </option>
        <option value="none">{part.off}</option>
        {#each others(part.name) as pkg (pkg.id)}
          <option value={pkg.id}>From {pkg.name}{what(pkg, part.name)}</option>
        {/each}
      </select>
    {/each}
  </div>
</div>

<style>
  .parts {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    margin-top: 0.4rem;
  }

  .amount {
    display: flex;
    border: 1px solid var(--border);
    border-radius: 6px;
    overflow: hidden;
    align-self: flex-start;
  }

  .amount button {
    background: transparent;
    color: var(--text);
    border: 0;
    border-right: 1px solid var(--border);
    border-radius: 0;
    padding: 0.3rem 0.8rem;
  }

  .amount button:last-child {
    border-right: 0;
  }

  .amount button.selected {
    background: var(--selected);
    color: var(--on-selected);
  }

  .rows {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    gap: 0.3rem 0.6rem;
    align-items: center;
  }

  .row {
    font-size: 0.85em;
    color: var(--text-dim);
  }

  select {
    min-width: 0;
    background: var(--panel-raised);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 0.2rem 0.3rem;
    font-size: 0.85em;
  }
</style>
