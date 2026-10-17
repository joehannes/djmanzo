<!--
  A rotary knob, like a mixer's: drag up or down, scroll, or use the arrow
  keys. Double-click returns it to the middle. It is an ARIA slider, so a
  screen reader hears its name and value.
-->
<script lang="ts">
  interface Props {
    value: number;
    min?: number;
    max?: number;
    centre?: number;
    label: string;
    display?: (v: number) => string;
    onchange: (v: number) => void;
    colour?: string;
  }
  let { value, min = -1, max = 1, centre = 0, label, display = (v) => v.toFixed(2), onchange, colour = "var(--primary)" }: Props = $props();
  const angle = $derived(-135 + ((value - min) / (max - min)) * 270);
  let startY = 0;
  let startV = 0;

  function set(v: number) {
    onchange(Math.max(min, Math.min(max, v)));
  }
  function down(e: PointerEvent) {
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    startY = e.clientY;
    startV = value;
  }
  function move(e: PointerEvent) {
    if (!(e.currentTarget as HTMLElement).hasPointerCapture(e.pointerId)) return;
    set(startV + ((startY - e.clientY) / 160) * (max - min));
  }
  function key(e: KeyboardEvent) {
    const step = (max - min) / 40;
    if (e.key === "ArrowUp" || e.key === "ArrowRight") set(value + step);
    else if (e.key === "ArrowDown" || e.key === "ArrowLeft") set(value - step);
    else if (e.key === "Home") set(min);
    else if (e.key === "End") set(max);
    else return;
    e.preventDefault();
  }
</script>

<div class="knob">
  <div
    class="dial"
    role="slider"
    tabindex="0"
    aria-label={label}
    aria-valuemin={min}
    aria-valuemax={max}
    aria-valuenow={Number(value.toFixed(2))}
    aria-valuetext={display(value)}
    onpointerdown={down}
    onpointermove={move}
    onkeydown={key}
    ondblclick={() => set(centre)}
    onwheel={(e) => {
      e.preventDefault();
      set(value - Math.sign(e.deltaY) * ((max - min) / 40));
    }}
    style="--a:{angle}deg;--c:{colour}"
  >
    <span class="cap"></span>
  </div>
  <span class="lbl">{label}</span>
  <span class="val">{display(value)}</span>
</div>

<style>
  .knob {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    min-width: 56px;
  }
  .dial {
    position: relative;
    width: 48px;
    height: 48px;
    border-radius: 50%;
    background: conic-gradient(from -135deg, var(--c) 0deg, var(--c) calc(var(--a) + 135deg), rgba(255, 255, 255, 0.12) calc(var(--a) + 135deg), rgba(255, 255, 255, 0.12) 270deg, transparent 270deg);
    cursor: ns-resize;
    touch-action: none;
  }
  .cap {
    position: absolute;
    inset: 6px;
    border-radius: 50%;
    background: radial-gradient(circle at 40% 35%, #4a4c55, #1d1e23);
    box-shadow: 0 3px 6px rgba(0, 0, 0, 0.5);
    rotate: var(--a);
  }
  .cap::after {
    content: "";
    position: absolute;
    left: 50%;
    top: 4px;
    width: 3px;
    height: 12px;
    translate: -50% 0;
    border-radius: 2px;
    background: #fff;
  }
  .lbl {
    font-size: 12px;
    font-weight: 700;
  }
  .val {
    font-size: 11px;
    color: var(--ink-soft);
    font-variant-numeric: tabular-nums;
  }
</style>
