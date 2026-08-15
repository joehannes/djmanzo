//! What the UI is told, and how often.
//!
//! The engine writes into the [`ParameterRegistry`] at audio rate. The UI needs
//! it at frame rate. This module is the bridge: a thread that samples the
//! registry 60 times a second and emits a typed snapshot.
//!
//! Sampling rather than streaming events is deliberate. The playhead changes
//! every callback -- roughly 190 times a second at 256 frames -- and forwarding
//! each change would flood the IPC channel with data the display cannot use.

use dj_control::ParameterRegistry;
use dj_core::param::{DeckParam, GlobalParam};
use dj_core::{DeckId, ParamId};
use serde::Serialize;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// UI refresh rate. Matches a 60 Hz display; the engine runs far faster.
pub const SNAPSHOT_HZ: u64 = 60;

/// How long the pump may stay silent before emitting anyway.
///
/// Purely so a late subscriber is not left staring at an empty interface until
/// something happens to change.
///
/// Measured against the clock rather than counted in ticks: `thread::sleep`
/// guarantees only a *minimum*, and it overshoots by several milliseconds per
/// call on macOS. Counting 60 ticks of a nominal 16.7 ms sleep gives an interval
/// anywhere from 1.0 s to well past 1.3 s depending on the platform's timer
/// granularity and load.
pub const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DeckSnapshot {
    /// 1-based, as shown in the interface.
    pub number: u8,
    pub playing: bool,
    pub loaded: bool,
    pub position_frames: f32,
    pub length_frames: f32,
    pub position_seconds: f32,
    pub length_seconds: f32,
    pub rate: f32,
    pub pitch: f32,
    pub volume: f32,
    pub gain_db: f32,
    pub peak: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MasterSnapshot {
    pub crossfader: f32,
    pub gain_db: f32,
    pub peak_left: f32,
    pub peak_right: f32,
    pub sample_rate: f32,
    pub xruns: f32,
    pub cpu_load: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Snapshot {
    pub decks: Vec<DeckSnapshot>,
    pub master: MasterSnapshot,
}

impl Snapshot {
    /// Read the current state of `deck_count` decks.
    #[must_use]
    pub fn capture(registry: &ParameterRegistry, deck_count: usize) -> Self {
        let sample_rate = registry.get(ParamId::Global(GlobalParam::SampleRate));
        // Before a device is open the rate is zero; dividing by it would put
        // infinities on screen.
        let to_seconds = |frames: f32| {
            if sample_rate > 0.0 {
                frames / sample_rate
            } else {
                0.0
            }
        };

        let decks = (0..deck_count)
            .filter_map(|index| DeckId::new(index as u8))
            .map(|id| {
                let get = |param| registry.get(ParamId::Deck(id, param));
                let position = get(DeckParam::Position);
                let length = get(DeckParam::LengthFrames);
                DeckSnapshot {
                    number: id.human_number(),
                    playing: get(DeckParam::Playing) >= 0.5,
                    loaded: get(DeckParam::Loaded) >= 0.5,
                    position_frames: position,
                    length_frames: length,
                    position_seconds: to_seconds(position),
                    length_seconds: to_seconds(length),
                    rate: get(DeckParam::Rate),
                    pitch: get(DeckParam::Pitch),
                    volume: get(DeckParam::Volume),
                    gain_db: get(DeckParam::GainDb),
                    peak: get(DeckParam::PeakLevel),
                }
            })
            .collect();

        Self {
            decks,
            master: MasterSnapshot {
                crossfader: registry.get(ParamId::Global(GlobalParam::Crossfader)),
                gain_db: registry.get(ParamId::Global(GlobalParam::MasterGainDb)),
                peak_left: registry.get(ParamId::Global(GlobalParam::MasterPeakLeft)),
                peak_right: registry.get(ParamId::Global(GlobalParam::MasterPeakRight)),
                sample_rate,
                xruns: registry.get(ParamId::Global(GlobalParam::Xruns)),
                cpu_load: registry.get(ParamId::Global(GlobalParam::CpuLoad)),
            },
        }
    }
}

/// A running snapshot pump. Stops when dropped.
#[derive(Debug)]
pub struct SnapshotPump {
    alive: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl SnapshotPump {
    /// Start sampling, handing each snapshot to `emit`.
    pub fn start(
        registry: Arc<ParameterRegistry>,
        deck_count: usize,
        emit: impl FnMut(Snapshot) + Send + 'static,
    ) -> Self {
        Self::with_heartbeat(registry, deck_count, HEARTBEAT_INTERVAL, emit)
    }

    /// Start sampling with an explicit heartbeat interval.
    ///
    /// Exists so tests can exercise the heartbeat in milliseconds rather than
    /// sleeping for the production interval.
    pub fn with_heartbeat(
        registry: Arc<ParameterRegistry>,
        deck_count: usize,
        heartbeat: Duration,
        mut emit: impl FnMut(Snapshot) + Send + 'static,
    ) -> Self {
        let alive = Arc::new(AtomicBool::new(true));
        let thread = {
            let alive = Arc::clone(&alive);
            std::thread::Builder::new()
                .name("dj-snapshot".to_owned())
                .spawn(move || {
                    let period = Duration::from_micros(1_000_000 / SNAPSHOT_HZ);
                    let mut previous: Option<Snapshot> = None;
                    let mut last_emit = std::time::Instant::now();
                    while alive.load(Ordering::Relaxed) {
                        let snapshot = Snapshot::capture(&registry, deck_count);
                        let changed = previous.as_ref() != Some(&snapshot);

                        // Skip identical frames -- an idle application should not
                        // wake the webview 60 times a second for no reason. But
                        // emit anyway on the heartbeat, because a listener that
                        // subscribes during a quiet period would otherwise never
                        // receive anything and sit on a blank interface forever.
                        if changed || last_emit.elapsed() >= heartbeat {
                            emit(snapshot.clone());
                            previous = Some(snapshot);
                            last_emit = std::time::Instant::now();
                        }
                        std::thread::sleep(period);
                    }
                })
                .expect("failed to spawn snapshot thread")
        };

        Self {
            alive,
            thread: Some(thread),
        }
    }
}

impl Drop for SnapshotPump {
    fn drop(&mut self) {
        self.alive.store(false, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn capture_reads_the_registry() {
        let registry = ParameterRegistry::new();
        let deck = DeckId::from_human(1).unwrap();
        registry.set(ParamId::Global(GlobalParam::SampleRate), 48_000.0);
        registry.set(ParamId::Deck(deck, DeckParam::Playing), 1.0);
        registry.set(ParamId::Deck(deck, DeckParam::Position), 96_000.0);
        registry.set(ParamId::Deck(deck, DeckParam::LengthFrames), 480_000.0);

        let snapshot = Snapshot::capture(&registry, 2);
        assert_eq!(snapshot.decks.len(), 2);
        assert_eq!(snapshot.decks[0].number, 1);
        assert!(snapshot.decks[0].playing);
        assert!((snapshot.decks[0].position_seconds - 2.0).abs() < 1e-6);
        assert!((snapshot.decks[0].length_seconds - 10.0).abs() < 1e-6);
        assert!(!snapshot.decks[1].playing);
    }

    /// Before a device is open the sample rate is zero. Naive division would
    /// put `Infinity` or `NaN` on screen.
    #[test]
    fn capture_survives_a_zero_sample_rate() {
        let registry = ParameterRegistry::new();
        let snapshot = Snapshot::capture(&registry, 2);
        assert_eq!(snapshot.decks[0].position_seconds, 0.0);
        assert!(snapshot.decks[0].length_seconds.is_finite());
    }

    #[test]
    fn pump_emits_when_state_changes() {
        let registry = Arc::new(ParameterRegistry::new());
        let seen = Arc::new(Mutex::new(Vec::new()));

        let pump = {
            let seen = Arc::clone(&seen);
            SnapshotPump::start(Arc::clone(&registry), 2, move |snapshot| {
                seen.lock().unwrap().push(snapshot);
            })
        };

        std::thread::sleep(Duration::from_millis(50));
        let baseline = seen.lock().unwrap().len();
        assert!(baseline >= 1, "should emit an initial snapshot");

        registry.set(
            ParamId::Deck(DeckId::from_human(1).unwrap(), DeckParam::Playing),
            1.0,
        );
        std::thread::sleep(Duration::from_millis(80));
        assert!(
            seen.lock().unwrap().len() > baseline,
            "a state change should produce a new snapshot"
        );
        drop(pump);
    }

    /// An idle application must not wake the webview 60 times a second.
    #[test]
    fn pump_stays_quiet_when_nothing_changes() {
        let registry = Arc::new(ParameterRegistry::new());
        let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));

        // Heartbeat pushed far out so this measures deduplication alone, with
        // no dependence on timing.
        let pump = {
            let count = Arc::clone(&count);
            SnapshotPump::with_heartbeat(
                Arc::clone(&registry),
                2,
                Duration::from_secs(60),
                move |_| {
                    count.fetch_add(1, Ordering::Relaxed);
                },
            )
        };

        std::thread::sleep(Duration::from_millis(200));
        drop(pump);

        let emitted = count.load(Ordering::Relaxed);
        assert_eq!(
            emitted, 1,
            "idle pump emitted {emitted} snapshots; should be exactly the initial one"
        );
    }

    /// A UI that subscribes during a quiet period must still receive state.
    /// Without the heartbeat it waits forever on a blank interface -- which is
    /// exactly what happened the first time the application was run.
    #[test]
    fn pump_heartbeats_so_late_subscribers_get_state() {
        let registry = Arc::new(ParameterRegistry::new());
        let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));

        // A short heartbeat keeps the test fast; the mechanism is identical.
        let pump = {
            let count = Arc::clone(&count);
            SnapshotPump::with_heartbeat(
                Arc::clone(&registry),
                2,
                Duration::from_millis(100),
                move |_| {
                    count.fetch_add(1, Ordering::Relaxed);
                },
            )
        };

        // Several heartbeat intervals, with nothing changing at all. Generous
        // margin because sleep only guarantees a minimum.
        std::thread::sleep(Duration::from_millis(600));
        drop(pump);

        assert!(
            count.load(Ordering::Relaxed) >= 2,
            "expected at least one heartbeat beyond the initial snapshot"
        );
    }

    #[test]
    fn dropping_the_pump_stops_it() {
        let registry = Arc::new(ParameterRegistry::new());
        let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let pump = {
            let count = Arc::clone(&count);
            SnapshotPump::start(Arc::clone(&registry), 2, move |_| {
                count.fetch_add(1, Ordering::Relaxed);
            })
        };
        std::thread::sleep(Duration::from_millis(30));
        drop(pump);
        let after = count.load(Ordering::Relaxed);
        std::thread::sleep(Duration::from_millis(60));
        assert_eq!(
            count.load(Ordering::Relaxed),
            after,
            "pump outlived its handle"
        );
    }
}
