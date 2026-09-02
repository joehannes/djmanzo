<script lang="ts">
  import type { MasterState, SplitOutput } from "./api";
  import { fill } from "./meter";

  let { master, ready, split = null, cueSplit = false, limiterOn = true, send }: {
    master: MasterState; ready: boolean; split?: SplitOutput | null; cueSplit?: boolean; limiterOn?: boolean; send: (action: string) => void | Promise<void>;
  } = $props();

  const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value));
  const norm = (value: number, min: number, max: number) => (clamp(value, min, max) - min) / (max - min);

  function valueFromPointer(event: PointerEvent, min: number, max: number): string {
    const rect = (event.currentTarget as SVGElement).getBoundingClientRect();
    return String(min + clamp((event.clientX - rect.left) / rect.width, 0, 1) * (max - min));
  }

  function drag(event: PointerEvent, min: number, max: number, action: (value: string) => string) {
    if (!ready) return;
    const target = event.currentTarget as SVGElement;
    target.setPointerCapture(event.pointerId);
    const move = (next: PointerEvent) => void send(action(valueFromPointer(next, min, max)));
    move(event);
    const done = (next: PointerEvent) => {
      target.releasePointerCapture(next.pointerId);
      target.removeEventListener("pointermove", move);
      target.removeEventListener("pointerup", done);
      target.removeEventListener("pointercancel", done);
    };
    target.addEventListener("pointermove", move);
    target.addEventListener("pointerup", done);
    target.addEventListener("pointercancel", done);
  }

  const cross = $derived(norm(master.crossfader, -1, 1));
  const gain = $derived(norm(master.gain_db, -24, 6));
  const cue = $derived(norm(master.cue_mix, 0, 1));
  const reduction = $derived(fill(master.limiter_reduction_db / 12));

  function activate(event: KeyboardEvent, action: () => void | Promise<void>) {
    if (event.key !== "Enter" && event.key !== " ") return;
    event.preventDefault();
    void action();
  }

  /** Every control on this panel is inert until the engine is connected. */
  function act(command: string) {
    if (ready) send(command);
  }
</script>

<!--
  One row, not two.
  --------------------------------------------------------------------------
  This was a 900x210 block of two rows, and at djmanzo's own default window
  size it started 687 px down and ran to 897 -- so master gain, quantize,
  limiter and both meters sat below an 800 px fold while 355 px of horizontal
  room went unused beside them. The browser budget in `ui/e2e/` measured that
  on its first run; it is the third time a control has gone off the bottom of
  the first screen, and the second time it was the crossfader's own panel.

  Flattening the two rows into one spends the width that was already there and
  brings the whole strip to 110 px, which fits. The reading order is what a DJ
  reaches for in order: what you mix with, how loud it is, what you hear, what
  is coming out, and what is protecting it -- so the limiter's reduction meter
  now sits beside the limiter rather than in a column of its own.
-->
<svg class="master-mixer" viewBox="0 0 1240 110" role="group" aria-label="Master mixer controls">
  <defs><linearGradient id="meter-hot" x1="0" x2="1"><stop offset="0" stop-color="var(--accent-2)" /><stop offset="0.76" stop-color="var(--accent-2)" /><stop offset="1" stop-color="var(--warn)" /></linearGradient></defs>
  <rect class="shell" x="1" y="1" width="1238" height="108" rx="18" />

  <g class="slider" role="slider" aria-label="Crossfader" aria-valuemin="-1" aria-valuemax="1" aria-valuenow={master.crossfader} tabindex={ready ? 0 : -1} onpointerdown={(e) => drag(e, -1, 1, (v) => `crossfader ${v}`)}>
    <text x="28" y="30">Crossfader</text><rect class="track" x="28" y="42" width="300" height="14" rx="7" /><rect class="split" x="176" y="38" width="4" height="22" rx="2" /><circle class="thumb" cx={28 + cross * 300} cy="49" r="17" /><text class="ends" x="28" y="82">1</text><text class="ends" x="318" y="82">2</text>
  </g>

  <g class="slider" role="slider" aria-label="Master gain" aria-valuemin="-24" aria-valuemax="6" aria-valuenow={master.gain_db} tabindex={ready ? 0 : -1} onpointerdown={(e) => drag(e, -24, 6, (v) => `master gain ${v}`)}>
    <text x="360" y="30">Master gain</text><text class="readout" x="484" y="30">{master.gain_db.toFixed(1)} dB</text><rect class="track" x="360" y="42" width="220" height="14" rx="7" /><rect class="fill" x="360" y="42" width={gain * 220} height="14" rx="7" /><circle class="thumb accent" cx={360 + gain * 220} cy="49" r="17" />
  </g>

  <g class:disabled={!master.cue_available} class="cue"><text x="612" y="30">Headphones</text>{#if master.cue_available}<g class="slider" role="slider" aria-label="Headphone cue mix" aria-valuemin="0" aria-valuemax="1" aria-valuenow={master.cue_mix} tabindex={ready && !master.cue_split ? 0 : -1} onpointerdown={(e) => !master.cue_split && drag(e, 0, 1, (v) => `cue mix ${v}`)}><rect class="track" x="612" y="42" width="190" height="14" rx="7" /><rect class="fill teal" x="612" y="42" width={cue * 190} height="14" rx="7" /><circle class="thumb teal" cx={612 + cue * 190} cy="49" r="16" /><text class="ends" x="612" y="82">cue</text><text class="ends" x="760" y="82">master</text></g><g class="svg-button" class:active={master.cue_split} role="button" aria-label="Toggle split cue" tabindex={ready ? 0 : -1} onclick={() => act(`cue ${cueSplit ? "split_off" : "split_on"}`)} onkeydown={(e) => activate(e, () => act(`cue ${cueSplit ? "split_off" : "split_on"}`))}><rect x="822" y="30" width="76" height="42" rx="12" /><text x="860" y="53">SPLIT</text></g>{:else}<text class="muted" x="612" y="58">No four-channel cue output</text>{/if}</g>

  <!--
    Output and reduction, stacked in one column: three bars of ten pixels read
    as well as three of twelve and are what let the row be 110 tall.
  -->
  <g class="meters"><text x="922" y="30">Output</text><rect class="meter-bg" x="922" y="38" width="120" height="10" rx="5" /><rect class="meter-fill" x="922" y="38" width={fill(master.peak_left) * 120} height="10" rx="5" /><rect class="meter-bg" x="922" y="54" width="120" height="10" rx="5" /><rect class="meter-fill" x="922" y="54" width={fill(master.peak_right) * 120} height="10" rx="5" /><text class="muted small" x="922" y="82">{master.limiter_reduction_db < 0.1 ? "no reduction" : `-${master.limiter_reduction_db.toFixed(1)} dB held back`}</text>{#if split}<text class:bad={!split.healthy} class="muted small" x="1052" y="30">{split.drift_ppm >= 0 ? "+" : ""}{split.drift_ppm.toFixed(0)} ppm</text>{/if}</g>

  <g class="svg-button" class:active={master.quantize} role="button" aria-label="Toggle quantize" tabindex={ready ? 0 : -1} onclick={() => act(`quantize ${master.quantize ? "off" : "on"}`)} onkeydown={(e) => activate(e, () => act(`quantize ${master.quantize ? "off" : "on"}`))}><rect x="1064" y="16" width="152" height="38" rx="12" /><text x="1140" y="35">QUANTIZE</text></g>
  <g class="svg-button" class:active={ready && limiterOn} role="button" aria-label="Toggle limiter" tabindex={ready ? 0 : -1} onclick={() => act(`limiter ${limiterOn ? "off" : "on"}`)} onkeydown={(e) => activate(e, () => act(`limiter ${limiterOn ? "off" : "on"}`))}><rect x="1064" y="60" width="152" height="38" rx="12" /><text x="1140" y="79">LIMITER</text></g>
  <!--
    The reduction bar sits under the limiter that causes it, rather than in the
    meter column, so the number and the switch that governs it are together.
  -->
  <rect class="meter-bg" x="1064" y="102" width="152" height="5" rx="2.5" /><rect class="reduction" x={1216 - reduction * 152} y="102" width={reduction * 152} height="5" rx="2.5" />
</svg>

<style>
  /*
    Capped at its own viewBox width, for the reason it always was: stretched
    wider, the whole thing scales up with it -- a 16 px label becomes 22 and a
    17 px thumb becomes 23 -- and the master strip grows louder than the decks
    it serves. Below its natural width it still scales down, which is what the
    viewBox is for; the strip getting shorter as the window narrows is right,
    because the fold comes up to meet it.

    No `min-height`. There was one of 12rem, which was harmless at 210 tall and
    would now letterbox a 110 px strip inside 192 px of nothing.
  */
  .master-mixer { width: 100%; max-width: 1240px; touch-action: none; }
  .shell { fill: var(--panel); stroke: var(--border); stroke-width: 2; }
  text { fill: var(--text); font: 600 16px system-ui, sans-serif; pointer-events: none; }
  .readout, .ends, .muted { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-weight: 500; }
  .muted, .ends { fill: var(--text-dim); } .readout { fill: var(--accent-2); }
  /* The reduction line is a caption under the meters, not a label beside a control. */
  .small { font-size: 13px; }
  .track, .meter-bg { fill: var(--panel-raised); stroke: var(--border); } .split { fill: var(--border-strong); }
  .fill { fill: var(--accent); } .fill.teal { fill: var(--accent-2); }
  .thumb { fill: var(--panel-hover); stroke: var(--accent); stroke-width: 3; filter: drop-shadow(0 5px 10px rgb(0 0 0 / 0.35)); } .thumb.teal { stroke: var(--accent-2); }
  .svg-button rect { fill: var(--panel-raised); stroke: var(--border); stroke-width: 2; } .svg-button text { text-anchor: middle; dominant-baseline: middle; font-size: 14px; }
  .svg-button.active rect { fill: var(--accent-2); stroke: var(--accent-2); } .svg-button.active text { fill: var(--on-accent); }
  .meter-fill { fill: url(#meter-hot); } .reduction { fill: var(--warn); } .disabled { opacity: 0.55; } .bad { fill: var(--danger); }
  [role="slider"], [role="button"] { cursor: pointer; outline: none; } [role="slider"]:focus-visible .thumb, [role="button"]:focus-visible rect { stroke: var(--focus, var(--accent-2)); stroke-width: 4; }
</style>
