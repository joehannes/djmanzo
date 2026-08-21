<script lang="ts">
  /**
   * A tiny SVG level meter.
   *
   * The application moved away from HTML-painted controls: state should be a
   * shape the interface owns, not a styled rectangle the browser happens to
   * draw. This component is intentionally small so panel code can replace
   * legacy `<div><span style=...>` meters without inventing local SVG again.
   */
  let {
    value,
    label,
    tone = "var(--accent)",
    background = "var(--sunken, rgba(0, 0, 0, 0.3))",
  }: {
    value: number;
    label: string;
    tone?: string;
    background?: string;
  } = $props();

  const clamped = $derived(Math.max(0, Math.min(1, Number.isFinite(value) ? value : 0)));
  const width = $derived(clamped * 100);
</script>

<svg
  class="svg-meter"
  viewBox="0 0 100 10"
  role="meter"
  aria-label={label}
  aria-valuemin="0"
  aria-valuemax="100"
  aria-valuenow={Math.round(clamped * 100)}
>
  <rect x="0" y="1" width="100" height="8" rx="4" fill={background} />
  <rect class="fill" x="0" y="1" width={width} height="8" rx="4" fill={tone} />
</svg>

<style>
  .svg-meter {
    display: block;
    inline-size: 100%;
    block-size: 0.45rem;
    overflow: visible;
  }

  .fill {
    transition: width 60ms linear;
  }
</style>
