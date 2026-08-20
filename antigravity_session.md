# Antigravity AI session — reviewed and reconciled

**Written:** 19 August 2026 · **Reviewed:** 20 August 2026 · **Status: handled.**

Three commits arrived while this session was out of credits
(`3a2eec9`, `20607c3`). They have been read, audited, corrected and integrated.
This file is the record of both halves: what was claimed, and what was actually
there. It is kept rather than deleted because the corrections only make sense
next to the claims.

---

## The state the work arrived in

All four of the project's gates were red, so CI could not have passed:

| Gate | Result on arrival |
|---|---|
| `cargo test --workspace` | **fails to compile** — `dj-app` referenced `dj_core::AudioMetrics`, which was never exported |
| `RUSTFLAGS="-D warnings" cargo clippy --workspace --all-targets` | **5 errors** (unused bindings in the analyser's test) |
| `cargo fmt --all --check` | **fails** |
| `npx svelte-check` | **33 errors**, 5 warnings, 10 files |

Everything below is now green: 1261 tests, clippy clean, fmt clean,
svelte-check 0/0.

---

## What held up

- **The FFT is genuinely allocation-free.** `process_with_scratch` with a
  pre-planned transform and pre-sized scratch does not allocate, and the
  RT-safety harness confirms it. That was the load-bearing claim and it was
  right.
- **It is also cheap.** Measured at **9.5 µs** per transform in release, against
  a 5,333 µs budget for a 256-frame block at 48 kHz — 0.2%. The worry that an
  FFT in the callback would be a dropout risk was not borne out.
- The performance governor's *idea* — tier down fast, tier up slowly — is right,
  and is kept.
- The theme pipeline's *shape* — geometry, then behaviours, then effects — is a
  good decomposition, and is kept.

## What did not

### 1. The controls could not show their values

`Deck.svelte` replaced its HTML range inputs with `SvgKnob`, `SvgFader` and
`SvgPad`. The geometry generators behind them never read `normalized`, `angle`,
`active` or `pressed` — every knob drew the same circle and every fader the same
rectangle, at every setting. The numeric readouts (`Filter LP 60%`,
`Volume 1.00`, `Pitch 0.0%`) were deleted along with the markup that held them,
and `SvgPad` never rendered its label, so the transport was four blank boxes.

The net effect: a DJ could not see where the volume, EQ, filter or pitch were,
or which button was which.

Fixed by writing the value layer into the geometry — track arc, fill arc and
pointer for a knob; track, fill and handle for a fader; a lit face for a pad —
and by giving the controls a `readout` prop that carries the number back.

### 2. A theme could erase the value layer

`AudioReactiveStroke` restyled *every* path, so once the indicator existed it
would have been repainted to match the body and lost again. Paths now carry a
`role`, and `decorate()` passes over anything marked `"value"`. A theme may
restyle a control's body; it may not restyle the one part the setting is read
from.

### 3. The advertised auto-recovery could never fire

The governor counted callbacks from `watchFrameRate`'s `onChange` and waited for
600 of them. `onChange` is edge-triggered — it fires *only when the verdict
changes* — so the counter never reached two. Once the interface stepped down to
Balanced or Eco it stayed there for the rest of the session.

`watchFrameRate` now also takes an `onSample` that fires every one-second
window, and the governor counts those. Ten windows to step back up.

### 4. The session context was fabricated

`Snapshot::capture_all` hardcoded `phase: Peak, energy_level: 0.95` into every
snapshot, sixty times a second. Nothing measured either. An interface keyed to
them would have announced peak time thirty seconds into a warm-up.

`SessionContext` is now split: `audio` is measured and always true;
`session` is `Option<SessionRead>` and stays `None` until M9 has something that
actually reads the room. `EnvironmentContext` lost its hardcoded
`weather: "Clear", temperature_c: 20.0` for the same reason, and `time_of_day`
became an enum derived from the clock — the one part of it that can be known.

The TypeScript types had also declared `venue`, `vibe`, `density`,
`crowd_energy` and `tempo_variance`, none of which Rust ever sent. Reading any
of them gave `undefined` with the type system promising a number.

### 5. The bands were not comparable with each other

Each band was a sum of bin magnitudes. Treble spans ~325 bins and bass ~5, so
for the same amount of sound treble read far higher — and on any broadband
material the upper bands clamped to 1.0 and stayed there. The interface would
have been reacting to bin counts.

Now each band is an RMS amplitude (Parseval, one-sided, with the Hann window's
own gain divided out). A full-scale sine reads ≈0.707 whichever band it lands
in. Two tests pin this, and both fail if the old summation is put back.

### 6. The test that checked nothing

`test_spectral_bands_no_allocation` made no assertion about allocation. The
property it named is now proved where it can be — `the_spectrum_never_allocates`
in the RT-safety harness, which counts allocations through ~5,000 transforms.

### 7. Layering

The analyser went into `dj-engine`, whose own module documentation states its
dependency surface is deliberately `dj-core`, `dj-dsp`, `dj-decode`,
`dj-control`, `rtrb` "so this crate stays auditable" — and `rustfft` was added
without updating it. It has moved to `dj-dsp` as `spectrum.rs`, beside
`PeakMeter`, which is what it is: a meter. `dj-engine` is back to its stated
dependencies.

It also ran every block — a 1024-frame window recomputed every 256 frames,
analysing each sample four times to feed an interface that redraws at 60 Hz. It
now runs on a 512-frame hop, and skips the transform entirely on silence.

### 8. Dead code and small things

- `BaseTheme.svelte` and `OrganicTheme.svelte` (365 lines) were made dead by the
  second commit but left in the tree, still passed as a `theme` prop that no
  component destructured. Deleted, with the prop.
- The focus ring on all three controls targeted `.renderer`, a class belonging to
  a child component — Svelte scopes it away, so the controls showed no keyboard
  focus at all. Moved onto the container.
- `SvgRenderer` took one `size` and forced a square viewBox, so a 60×40 pad was
  drawn in a 60×60 box. Width and height are separate now.
- `#[derive(Clone)]` on the analyser — four heap buffers, on a type that only
  ever lives on the audio thread. Removed.
- `mock_peak_time()` / `mock_warm_up()` were production API in `dj-core`. Gone.
- `Settings.svelte` previewed themes against an invented peak-time reading;
  it now previews against silence, honestly labelled.

---

## The summary's own accuracy

The document claimed `crowd_energy`, `tempo_variance`, `venue` and `time_of_day`
were added to `EnvironmentContext`. Only `time_of_day` was, and only on the
TypeScript side. It also described the recovery window as "600-frame (~10
second)" when the callback it counted was not per-frame at all.

Recorded here because the next reader of a handover note should know how far it
can be trusted without checking.
