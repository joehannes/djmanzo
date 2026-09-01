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

<svg class="master-mixer" viewBox="0 0 900 210" role="group" aria-label="Master mixer controls">
  <defs><linearGradient id="meter-hot" x1="0" x2="1"><stop offset="0" stop-color="var(--accent-2)" /><stop offset="0.76" stop-color="var(--accent-2)" /><stop offset="1" stop-color="var(--warn)" /></linearGradient></defs>
  <rect class="shell" x="1" y="1" width="898" height="208" rx="18" />
    <g class="slider" role="slider" aria-label="Crossfader" aria-valuemin="-1" aria-valuemax="1" aria-valuenow={master.crossfader} tabindex={ready ? 0 : -1} onpointerdown={(e) => drag(e, -1, 1, (v) => `crossfader ${v}`)}>
    <text x="48" y="48">Crossfader</text><rect class="track" x="48" y="60" width="300" height="14" rx="7" /><rect class="split" x="196" y="56" width="4" height="22" rx="2" /><circle class="thumb" cx={48 + cross * 300} cy="67" r="17" /><text class="ends" x="48" y="100">1</text><text class="ends" x="338" y="100">2</text>
  </g>

  <g class="slider" role="slider" aria-label="Master gain" aria-valuemin="-24" aria-valuemax="6" aria-valuenow={master.gain_db} tabindex={ready ? 0 : -1} onpointerdown={(e) => drag(e, -24, 6, (v) => `master gain ${v}`)}>
    <text x="48" y="136">Master gain</text><text class="readout" x="176" y="136">{master.gain_db.toFixed(1)} dB</text><rect class="track" x="48" y="148" width="300" height="14" rx="7" /><rect class="fill" x="48" y="148" width={gain * 300} height="14" rx="7" /><circle class="thumb accent" cx={48 + gain * 300} cy="155" r="17" />
  </g>

  <g class:disabled={!master.cue_available} class="cue"><text x="398" y="48">Headphones</text>{#if master.cue_available}<g class="slider" role="slider" aria-label="Headphone cue mix" aria-valuemin="0" aria-valuemax="1" aria-valuenow={master.cue_mix} tabindex={ready && !master.cue_split ? 0 : -1} onpointerdown={(e) => !master.cue_split && drag(e, 0, 1, (v) => `cue mix ${v}`)}><rect class="track" x="398" y="60" width="250" height="14" rx="7" /><rect class="fill teal" x="398" y="60" width={cue * 250} height="14" rx="7" /><circle class="thumb teal" cx={398 + cue * 250} cy="67" r="16" /><text class="ends" x="398" y="100">cue</text><text class="ends" x="606" y="100">master</text></g><g class="svg-button" class:active={master.cue_split} role="button" aria-label="Toggle split cue" tabindex={ready ? 0 : -1} onclick={() => act(`cue ${cueSplit ? "split_off" : "split_on"}`)} onkeydown={(e) => activate(e, () => act(`cue ${cueSplit ? "split_off" : "split_on"}`))}><rect x="666" y="44" width="94" height="48" rx="12" /><text x="713" y="74">SPLIT</text></g>{:else}<text class="muted" x="398" y="75">No four-channel cue output</text>{/if}</g>

  <g class="svg-button" class:active={master.quantize} role="button" aria-label="Toggle quantize" tabindex={ready ? 0 : -1} onclick={() => act(`quantize ${master.quantize ? "off" : "on"}`)} onkeydown={(e) => activate(e, () => act(`quantize ${master.quantize ? "off" : "on"}`))}><rect x="398" y="126" width="148" height="52" rx="14" /><text x="472" y="158">QUANTIZE</text></g>
  <g class="svg-button" class:active={ready && limiterOn} role="button" aria-label="Toggle limiter" tabindex={ready ? 0 : -1} onclick={() => act(`limiter ${limiterOn ? "off" : "on"}`)} onkeydown={(e) => activate(e, () => act(`limiter ${limiterOn ? "off" : "on"}`))}><rect x="566" y="126" width="132" height="52" rx="14" /><text x="632" y="158">LIMITER</text></g>

  <g class="meters"><text x="740" y="48">Output</text><rect class="meter-bg" x="740" y="60" width="120" height="12" rx="6" /><rect class="meter-fill" x="740" y="60" width={fill(master.peak_left) * 120} height="12" rx="6" /><rect class="meter-bg" x="740" y="82" width="120" height="12" rx="6" /><rect class="meter-fill" x="740" y="82" width={fill(master.peak_right) * 120} height="12" rx="6" /><text class="muted" x="740" y="126">Reduction</text><rect class="meter-bg" x="740" y="138" width="120" height="12" rx="6" /><rect class="reduction" x={860 - reduction * 120} y="138" width={reduction * 120} height="12" rx="6" /><text class="readout" x="740" y="174">{master.limiter_reduction_db < 0.1 ? "—" : `-${master.limiter_reduction_db.toFixed(1)} dB`}</text>{#if split}<text class:bad={!split.healthy} class="muted" x="740" y="194">{split.drift_ppm >= 0 ? "+" : ""}{split.drift_ppm.toFixed(0)} ppm</text>{/if}</g>
</svg>

<style>
  /*
    Capped at its own viewBox width. Stretched wider than 900 the whole thing
    scales up with it -- a 16 px label becomes 22 px and a 17 px thumb becomes
    23 -- so on a wide window the master strip grew louder than the decks it
    serves. Below 900 it still scales down, which is what the viewBox is for.
  */
  .master-mixer { width: 100%; max-width: 900px; min-height: 12rem; touch-action: none; }
  .shell { fill: var(--panel); stroke: var(--border); stroke-width: 2; }
  text { fill: var(--text); font: 600 16px system-ui, sans-serif; pointer-events: none; }
  .readout, .ends, .muted { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-weight: 500; }
  .muted, .ends { fill: var(--text-dim); } .readout { fill: var(--accent-2); }
  .track, .meter-bg { fill: var(--panel-raised); stroke: var(--border); } .split { fill: var(--border-strong); }
  .fill { fill: var(--accent); } .fill.teal { fill: var(--accent-2); }
  .thumb { fill: var(--panel-hover); stroke: var(--accent); stroke-width: 3; filter: drop-shadow(0 5px 10px rgb(0 0 0 / 0.35)); } .thumb.teal { stroke: var(--accent-2); }
  .svg-button rect { fill: var(--panel-raised); stroke: var(--border); stroke-width: 2; } .svg-button text { text-anchor: middle; dominant-baseline: middle; font-size: 14px; }
  .svg-button.active rect { fill: var(--accent-2); stroke: var(--accent-2); } .svg-button.active text { fill: var(--on-accent); }
  .meter-fill { fill: url(#meter-hot); } .reduction { fill: var(--warn); } .disabled { opacity: 0.55; } .bad { fill: var(--danger); }
  [role="slider"], [role="button"] { cursor: pointer; outline: none; } [role="slider"]:focus-visible .thumb, [role="button"]:focus-visible rect { stroke: var(--focus, var(--accent-2)); stroke-width: 4; }
</style>
