//! The parameter registry.

use dj_core::param::ParamId;
use std::sync::atomic::{AtomicU32, Ordering};

/// A flat table of every parameter in the application.
///
/// Backed by a fixed-size array of atomics, indexed by [`ParamId::index`]. That
/// makes a read from the audio thread a bounds-checked array index and one
/// relaxed atomic load -- no hashing, no allocation, no locking, no chance of
/// priority inversion.
///
/// Values are stored as `f32` bit patterns in `AtomicU32`. Booleans are 0.0/1.0
/// and counters are exact up to 2^24, which is far more than the counters here
/// need.
///
/// Ordering is `Relaxed` throughout, deliberately: each parameter is an
/// independent scalar, and no reader derives a happens-before relationship from
/// one to another. The UI may observe a snapshot where two parameters are from
/// slightly different instants, which at a 60 Hz refresh is invisible.
#[derive(Debug)]
pub struct ParameterRegistry {
    slots: Box<[AtomicU32; ParamId::COUNT]>,
}

impl ParameterRegistry {
    /// All parameters start at zero. The engine writes real defaults during
    /// startup, before any audio callback runs.
    #[must_use]
    pub fn new() -> Self {
        Self {
            slots: Box::new(std::array::from_fn(|_| AtomicU32::new(0))),
        }
    }

    /// Read one parameter. Realtime-safe.
    #[must_use]
    pub fn get(&self, id: ParamId) -> f32 {
        f32::from_bits(self.slots[id.index()].load(Ordering::Relaxed))
    }

    /// Write one parameter. Realtime-safe.
    ///
    /// Non-finite values are dropped rather than stored: a NaN here would
    /// propagate into the UI and, worse, back into the engine on the next read.
    pub fn set(&self, id: ParamId, value: f32) {
        if value.is_finite() {
            self.slots[id.index()].store(value.to_bits(), Ordering::Relaxed);
        }
    }

    /// Read a parameter as a boolean flag.
    #[must_use]
    pub fn get_bool(&self, id: ParamId) -> bool {
        self.get(id) >= 0.5
    }

    pub fn set_bool(&self, id: ParamId, value: bool) {
        self.set(id, if value { 1.0 } else { 0.0 });
    }

    /// Add to a parameter. Not atomic as a read-modify-write, so it is only
    /// correct when a single writer owns the parameter -- which is the rule for
    /// every counter in the table (the engine owns them all).
    pub fn add(&self, id: ParamId, delta: f32) {
        self.set(id, self.get(id) + delta);
    }

    /// Copy the whole table. For the UI snapshot pump; not for the audio thread.
    #[must_use]
    pub fn snapshot(&self) -> Vec<f32> {
        self.slots
            .iter()
            .map(|slot| f32::from_bits(slot.load(Ordering::Relaxed)))
            .collect()
    }

    /// Copy the table into an existing buffer, so the snapshot pump can run
    /// without allocating on every tick.
    ///
    /// The buffer is resized on first use and reused thereafter.
    pub fn snapshot_into(&self, out: &mut Vec<f32>) {
        out.clear();
        out.reserve(ParamId::COUNT);
        out.extend(
            self.slots
                .iter()
                .map(|slot| f32::from_bits(slot.load(Ordering::Relaxed))),
        );
    }
}

impl Default for ParameterRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dj_core::deck::DeckId;
    use dj_core::param::{DeckParam, GlobalParam};

    fn deck_param(n: u8, p: DeckParam) -> ParamId {
        ParamId::Deck(DeckId::from_human(n).unwrap(), p)
    }

    #[test]
    fn starts_zeroed() {
        let reg = ParameterRegistry::new();
        for id in ParamId::all() {
            assert_eq!(reg.get(id), 0.0, "{} was not zero", id.name());
        }
    }

    #[test]
    fn stores_and_reads_back_exactly() {
        let reg = ParameterRegistry::new();
        let id = deck_param(1, DeckParam::Position);
        reg.set(id, 123_456.75);
        assert_eq!(reg.get(id), 123_456.75);
    }

    #[test]
    fn parameters_do_not_alias() {
        let reg = ParameterRegistry::new();
        // Write a distinct value everywhere, then verify none overwrote another.
        for (i, id) in ParamId::all().enumerate() {
            reg.set(id, i as f32);
        }
        for (i, id) in ParamId::all().enumerate() {
            assert_eq!(reg.get(id), i as f32, "{} aliased another slot", id.name());
        }
    }

    #[test]
    fn booleans_round_trip() {
        let reg = ParameterRegistry::new();
        let id = deck_param(2, DeckParam::Playing);
        assert!(!reg.get_bool(id));
        reg.set_bool(id, true);
        assert!(reg.get_bool(id));
        assert_eq!(reg.get(id), 1.0);
        reg.set_bool(id, false);
        assert!(!reg.get_bool(id));
    }

    #[test]
    fn non_finite_values_are_rejected() {
        let reg = ParameterRegistry::new();
        let id = ParamId::Global(GlobalParam::MasterGainDb);
        reg.set(id, -3.0);
        reg.set(id, f32::NAN);
        assert_eq!(reg.get(id), -3.0, "NaN must not reach the table");
        reg.set(id, f32::INFINITY);
        assert_eq!(reg.get(id), -3.0);
    }

    #[test]
    fn add_accumulates() {
        let reg = ParameterRegistry::new();
        let id = ParamId::Global(GlobalParam::Xruns);
        reg.add(id, 1.0);
        reg.add(id, 1.0);
        assert_eq!(reg.get(id), 2.0);
    }

    #[test]
    fn snapshot_matches_the_table() {
        let reg = ParameterRegistry::new();
        reg.set(deck_param(1, DeckParam::Volume), 0.8);
        reg.set(ParamId::Global(GlobalParam::Crossfader), -0.5);

        let snap = reg.snapshot();
        assert_eq!(snap.len(), ParamId::COUNT);
        assert_eq!(snap[deck_param(1, DeckParam::Volume).index()], 0.8);
        assert_eq!(snap[ParamId::Global(GlobalParam::Crossfader).index()], -0.5);
    }

    #[test]
    fn snapshot_into_reuses_its_buffer() {
        let reg = ParameterRegistry::new();
        let mut buf = Vec::new();
        reg.snapshot_into(&mut buf);
        let capacity = buf.capacity();
        reg.set(deck_param(1, DeckParam::Volume), 0.5);
        reg.snapshot_into(&mut buf);
        assert_eq!(buf.len(), ParamId::COUNT);
        assert_eq!(buf.capacity(), capacity, "should not reallocate on reuse");
        assert_eq!(buf[deck_param(1, DeckParam::Volume).index()], 0.5);
    }

    #[test]
    fn is_shareable_across_threads() {
        use std::sync::Arc;
        let reg = Arc::new(ParameterRegistry::new());
        let id = deck_param(1, DeckParam::Position);

        let writer = {
            let reg = Arc::clone(&reg);
            std::thread::spawn(move || {
                for i in 0..1_000 {
                    reg.set(id, i as f32);
                }
            })
        };
        let reader = {
            let reg = Arc::clone(&reg);
            std::thread::spawn(move || {
                for _ in 0..1_000 {
                    // Never torn: always a value that was actually written.
                    let v = reg.get(id);
                    assert!(v.is_finite() && (0.0..1_000.0).contains(&v));
                }
            })
        };
        writer.join().unwrap();
        reader.join().unwrap();
    }
}
