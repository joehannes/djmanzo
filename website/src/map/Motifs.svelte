<!--
  One SVG pattern per genre, in that genre's colours: the islet's ground is
  printed with its scene's visual language — papel picado for cumbia,
  halftone for reggaeton, concrete hatching for techno.
-->
<script lang="ts">
  import { GENRES } from "../lib/genres";
</script>

<defs>
  {#each GENRES as g (g.id)}
    {@const p = g.palette}
    <pattern id="motif-{g.id}" width="64" height="64" patternUnits="userSpaceOnUse">
      <rect width="64" height="64" fill={p.surface} />
      {#if g.motif === "palms"}
        <path d="M32 58 C33 44 31 34 33 24" stroke={p.secondary} stroke-width="2.5" fill="none" />
        <path d="M33 24 C24 18 16 20 10 26 M33 24 C40 16 50 16 56 22 M33 24 C30 14 26 10 20 8 M33 24 C38 14 44 10 50 10" stroke={p.primary} stroke-width="3" fill="none" stroke-linecap="round" />
      {:else if g.motif === "waves"}
        <path d="M0 20 Q16 8 32 20 T64 20 M0 44 Q16 32 32 44 T64 44" stroke={p.primary} stroke-width="3" fill="none" />
      {:else if g.motif === "papel"}
        <path d="M0 6 H64" stroke={p.ink} stroke-width="1" />
        <path d="M2 6 L12 22 L22 6 Z" fill={p.primary} /><path d="M22 6 L32 22 L42 6 Z" fill={p.secondary} /><path d="M42 6 L52 22 L62 6 Z" fill={p.ink} opacity="0.8" />
        <circle cx="12" cy="13" r="2" fill={p.surface} /><circle cx="32" cy="13" r="2" fill={p.surface} /><circle cx="52" cy="13" r="2" fill={p.surface} />
      {:else if g.motif === "halftone"}
        {#each [8, 24, 40, 56] as x (x)}{#each [8, 24, 40, 56] as y (y)}<circle cx={x} cy={y} r={2 + ((x + y) % 48) / 12} fill={p.primary} opacity="0.85" />{/each}{/each}
      {:else if g.motif === "grid"}
        <path d="M0 0 H64 V64" stroke={p.primary} stroke-width="1" fill="none" />
      {:else if g.motif === "concrete"}
        <path d="M-8 16 L16 -8 M-8 48 L48 -8 M16 72 L72 16 M48 72 L72 48" stroke={p.ink} stroke-width="1.2" opacity="0.35" />
        <circle cx="20" cy="40" r="1.5" fill={p.secondary} />
      {:else if g.motif === "neon"}
        <rect x="10" y="10" width="44" height="44" rx="10" stroke={p.primary} stroke-width="2.5" fill="none" />
        <rect x="20" y="20" width="24" height="24" rx="6" stroke={p.secondary} stroke-width="2" fill="none" />
      {:else if g.motif === "disco"}
        <path d="M0 32 L32 0 L64 32 L32 64 Z" fill="none" stroke={p.primary} stroke-width="1.5" />
        <path d="M16 16 L32 0 L48 16 L32 32 Z" fill={p.ink} opacity="0.25" />
      {:else if g.motif === "graffiti"}
        <path d="M6 20 C14 8 26 30 36 14 S54 10 58 22" stroke={p.primary} stroke-width="5" fill="none" stroke-linecap="round" />
        <path d="M20 26 V40 M44 22 V34" stroke={p.primary} stroke-width="3" stroke-linecap="round" />
        <path d="M8 50 C20 44 30 58 54 48" stroke={p.secondary} stroke-width="3" fill="none" stroke-linecap="round" />
      {:else if g.motif === "plankton"}
        <circle cx="10" cy="14" r="1.8" fill={p.primary} /><circle cx="40" cy="8" r="1.2" fill={p.secondary} /><circle cx="28" cy="36" r="2.2" fill={p.primary} /><circle cx="54" cy="44" r="1.4" fill={p.primary} /><circle cx="12" cy="54" r="1.2" fill={p.secondary} />
      {:else if g.motif === "sunburst"}
        {#each [0, 30, 60, 90, 120, 150] as a (a)}<path d="M32 32 L{32 + 40 * Math.cos((a * Math.PI) / 180)} {32 + 40 * Math.sin((a * Math.PI) / 180)}" stroke={p.primary} stroke-width="3" opacity="0.7" />{/each}
        <circle cx="32" cy="32" r="8" fill={p.secondary} />
      {:else if g.motif === "tape"}
        <rect x="-10" y="18" width="90" height="12" fill={p.primary} opacity="0.85" transform="rotate(-12 32 32)" />
        <rect x="-10" y="38" width="90" height="10" fill={p.secondary} opacity="0.7" transform="rotate(9 32 32)" />
      {/if}
    </pattern>
  {/each}
</defs>
