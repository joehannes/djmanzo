<script lang="ts">
  /**
   * K3: the chain behind a strip's row — every stage a singer's voice goes
   * through, each switched on or off and set, for the host who has a minute
   * before the next song (docs/KARAOKE.md §6: "the chain is behind a press
   * on the strip").
   *
   * Every range is Rust's (`dj_vocal::LIMITS`, carried as `limits`), and a
   * stage switched on starts at Rust's own starting point for it
   * (`every_stage`), so nothing here decides what a voice may be put through.
   * A change sends the strip's whole settings, which Rust holds to the same
   * limits, keeps and hands to the engine.
   */
  import type { StripSettings, VocalLimits } from "./api";

  let {
    index,
    strip,
    limits,
    everyStage,
    onchange,
  }: {
    index: number;
    strip: StripSettings;
    limits: VocalLimits;
    everyStage: StripSettings;
    onchange: (settings: StripSettings) => void;
  } = $props();

  type Stage = "gate" | "eq" | "compressor" | "de_esser" | "echo" | "reverb";
  type Top = "high_pass_hz" | "pan" | "to_main" | "to_monitor" | "to_cue";

  /** In the order the voice goes through them. */
  const STAGES: { stage: Stage; name: string }[] = [
    { stage: "gate", name: "Gate" },
    { stage: "eq", name: "EQ" },
    { stage: "compressor", name: "Compressor" },
    { stage: "de_esser", name: "De-esser" },
    { stage: "echo", name: "Echo" },
    { stage: "reverb", name: "Room" },
  ];

  /** The strip's own numbers, apart from the fader on its row. */
  const TOP: { key: Top; label: string; unit: string; step: number }[] = [
    { key: "high_pass_hz", label: "Low cut", unit: "Hz", step: 5 },
    { key: "pan", label: "Pan", unit: "", step: 0.05 },
    { key: "to_main", label: "PA", unit: "", step: 0.05 },
    { key: "to_monitor", label: "Monitor", unit: "", step: 0.05 },
    { key: "to_cue", label: "Headphones", unit: "", step: 0.05 },
  ];

  /** Each stage's numbers: which, said how, stepped how finely. */
  const CONTROLS: Record<Stage, { key: string; label: string; unit: string; step: number }[]> = {
    gate: [
      { key: "threshold_db", label: "Opens at", unit: "dB", step: 1 },
      { key: "range_db", label: "Closes by", unit: "dB", step: 1 },
    ],
    eq: [
      { key: "low_db", label: "Low", unit: "dB", step: 0.5 },
      { key: "mid_db", label: "Mid", unit: "dB", step: 0.5 },
      { key: "mid_hz", label: "Mid at", unit: "Hz", step: 50 },
      { key: "high_db", label: "High", unit: "dB", step: 0.5 },
    ],
    compressor: [
      { key: "threshold_db", label: "From", unit: "dB", step: 1 },
      { key: "ratio", label: "Ratio", unit: ": 1", step: 0.5 },
      { key: "makeup_db", label: "Makeup", unit: "dB", step: 0.5 },
    ],
    de_esser: [
      { key: "frequency_hz", label: "Above", unit: "Hz", step: 100 },
      { key: "threshold_db", label: "From", unit: "dB", step: 1 },
    ],
    echo: [
      { key: "delay_ms", label: "Delay", unit: "ms", step: 10 },
      { key: "feedback", label: "Repeats", unit: "", step: 0.05 },
      { key: "level", label: "Level", unit: "", step: 0.05 },
    ],
    reverb: [
      { key: "seconds", label: "Length", unit: "s", step: 0.1 },
      { key: "level", label: "Level", unit: "", step: 0.05 },
    ],
  };

  /** Rust's range for a number; a number Rust names no range for is not drawn. */
  const limit = (name: string): [number, number] | undefined => limits[name];

  /** A number as the host reads it. */
  function shown(name: string, value: number, unit: string): string {
    if (name === "high_pass_hz" && value <= 0) return "off";
    if (name === "pan") {
      if (Math.abs(value) < 0.025) return "centre";
      return `${value < 0 ? "L" : "R"} ${Math.round(Math.abs(value) * 100)}`;
    }
    return unit === "" ? `${Math.round(value * 100)} %` : `${Math.round(value * 10) / 10} ${unit}`;
  }

  function toggle(stage: Stage, on: boolean) {
    onchange({ ...strip, [stage]: on ? { ...everyStage[stage] } : null });
  }

  function setStage(stage: Stage, key: string, value: number) {
    const current = strip[stage] as Record<string, number> | null;
    if (!current) return;
    onchange({ ...strip, [stage]: { ...current, [key]: value } });
  }

  function setTop(key: Top, value: number) {
    onchange({ ...strip, [key]: value });
  }
</script>

<div class="chain" role="group" aria-label="Mic {index + 1}'s chain">
  <fieldset>
    <legend>Mic {index + 1}</legend>
    {#each TOP as control (control.key)}
      {@const range = limit(control.key)}
      {#if range}
        <label>
          <span>{control.label}</span>
          <input
            type="range"
            min={range[0]}
            max={range[1]}
            step={control.step}
            value={strip[control.key]}
            aria-label="Mic {index + 1} {control.label}"
            onchange={(e) => setTop(control.key, Number(e.currentTarget.value))}
          />
          <output>{shown(control.key, strip[control.key], control.unit)}</output>
        </label>
      {/if}
    {/each}
    <label class="switch">
      <input
        type="checkbox"
        checked={strip.talkover}
        aria-label="Mic {index + 1} talks over the music"
        onchange={(e) => onchange({ ...strip, talkover: e.currentTarget.checked })}
      />
      <span>Talks over the music</span>
    </label>
  </fieldset>
  {#each STAGES as { stage, name } (stage)}
    {@const settings = strip[stage] as Record<string, number> | null}
    <fieldset data-stage={stage} data-on={settings !== null}>
      <legend>
        <label class="switch">
          <input
            type="checkbox"
            checked={settings !== null}
            aria-label="Mic {index + 1} {name}"
            onchange={(e) => toggle(stage, e.currentTarget.checked)}
          />
          <span>{name}</span>
        </label>
      </legend>
      {#if settings}
        {#each CONTROLS[stage] as control (control.key)}
          {@const range = limit(`${stage}.${control.key}`)}
          {#if range}
            <label>
              <span>{control.label}</span>
              <input
                type="range"
                min={range[0]}
                max={range[1]}
                step={control.step}
                value={settings[control.key]}
                aria-label="Mic {index + 1} {name} {control.label}"
                onchange={(e) => setStage(stage, control.key, Number(e.currentTarget.value))}
              />
              <output>{shown(`${stage}.${control.key}`, settings[control.key], control.unit)}</output>
            </label>
          {/if}
        {/each}
      {/if}
    </fieldset>
  {/each}
</div>

<style>
  /* As wide as the row and no wider: a column a stage where the panel has
     room, one column where it does not. */
  .chain {
    display: grid;
    min-width: 0;
    grid-template-columns: repeat(auto-fill, minmax(min(13rem, 100%), 1fr));
    gap: 0.4rem;
    grid-column: 1 / -1;
    padding-top: 0.3rem;
  }

  fieldset {
    margin: 0;
    padding: 0.3rem 0.5rem 0.4rem;
    border: 1px solid var(--border);
    border-radius: var(--radius, 6px);
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    font-size: 0.85em;
  }

  fieldset[data-on="false"] {
    color: var(--text-dim);
  }

  legend {
    padding: 0 0.2rem;
    font-weight: 600;
  }

  label {
    display: grid;
    grid-template-columns: 5.5rem minmax(4rem, 1fr) 3.6rem;
    align-items: center;
    gap: 0.3rem;
  }

  label.switch {
    display: flex;
    gap: 0.3rem;
  }

  input {
    font: inherit;
  }

  output {
    color: var(--text-dim);
    text-align: right;
  }
</style>
