//! Reading the night.
//!
//! §11 asks for a context engine: one place that works out what the session is
//! doing, so that the interface, the theme, the suggester, the planner and the
//! assistant all read the same answer instead of each forming a private
//! opinion. `dj_core::context` defines what that answer *is*; this works it
//! out.
//!
//! # What it reads from
//!
//! **The records that have actually been played**, as they are played: how
//! loud each one is, how fast, and when it went on. That is evidence djmanzo
//! already has for every record in the library, and it is the evidence a DJ
//! would give if you asked them why they thought the room was building.
//!
//! Not the master meter. The master level says what the mixer is doing, not
//! what the night is doing: pulling the bass out for eight bars is not the
//! room cooling down, and turning the gain up is not the room lifting. The
//! meter is already published as [`dj_core::AudioMetrics`] and stays what it
//! is — a measurement, always true, about the audio.
//!
//! # Relative to the night, not to a table of genres
//!
//! A record's loudness maps to energy through a fixed window, but the *phase*
//! comes from comparing the recent records against the ones before them.
//! Absolute rules do not survive this project's repertoire: 96 BPM dembow and
//! 155 BPM merengue are both peak-time records, and a table that called one of
//! them a warm-up would be wrong in exactly the rooms djmanzo is built for.
//! What travels is the shape — where this hour sits against the last one.
//!
//! # No evidence, no reading
//!
//! Under [`ENOUGH`] records the answer is `None`, not a guess. `SessionRead`
//! was defaulted to *Peak at 0.95* once, which meant the interface announced
//! peak time thirty seconds into a warm-up — a claim nothing had made and
//! nothing could check. A night with two records in it has no shape yet, and
//! saying so is the honest answer.
//!
//! # It does not move unless something moved
//!
//! An ambiguous reading keeps the phase it had. The interface morphs to this,
//! and a phase that flickered between *Heat* and *Peak* while a DJ was
//! looking at it would be worse than one that lagged — the same rule the
//! density bands follow, for the same reason.

use dj_core::{EnvironmentContext, SessionPhase, SessionRead, TimeOfDay};
use std::sync::atomic::{AtomicU8, AtomicU32, Ordering};

/// How many records the night needs before it has a shape.
///
/// Three. Two records are a pair, not an arc: they can only rise or fall, and
/// either reading would be one record's mastering away from the opposite.
pub const ENOUGH: usize = 3;

/// How many of the most recent records count as "now".
const RECENT: usize = 3;

/// How much the recent stretch has to differ from the night to be a change.
///
/// A tenth of the scale. Below that the difference is mastering rather than
/// intent: two records cut in different decades differ by more than this
/// without anybody deciding anything.
const MARGIN: f64 = 0.1;

/// Loudness, in LUFS, that maps to no energy and to full energy.
///
/// Deliberately wide. Streaming normalises to -14 and club masters run hotter,
/// around -8 to -6; -20 is an ambient or vinyl-rip outlier and -5 is about as
/// hot as mastering goes. A record outside the window clamps rather than
/// distorting the scale for everything else.
const QUIET_LUFS: f64 = -20.0;
const LOUD_LUFS: f64 = -5.0;

/// Above this the room is being held rather than warmed.
const HOT: f64 = 0.6;
/// Below this it has been let down.
const COOL: f64 = 0.35;

/// One record, as evidence.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Played {
    /// The record's own tempo, when it has a grid.
    pub bpm: Option<f64>,
    /// Integrated loudness, when it has been measured.
    pub lufs: Option<f64>,
}

impl Played {
    /// This record's energy, 0..=1, or `None` when nothing measured it.
    ///
    /// Loudness only. Tempo is deliberately not in here — see the module note
    /// about the repertoire — and is used for the *direction* instead.
    #[must_use]
    pub fn energy(&self) -> Option<f64> {
        let lufs = self.lufs?;
        if !lufs.is_finite() {
            return None;
        }
        Some(((lufs - QUIET_LUFS) / (LOUD_LUFS - QUIET_LUFS)).clamp(0.0, 1.0))
    }
}

/// Why the engine reads the night the way it does.
///
/// Typed, like every other reason in this project: a reading that says
/// `Rising { from: 0.42, to: 0.61 }` can be argued with against the set list;
/// one that says "the room is building" cannot.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Because {
    /// The recent stretch is louder than the night so far.
    Rising { from: f64, to: f64 },
    /// And quieter.
    Falling { from: f64, to: f64 },
    /// Neither, so the phase stayed where it was.
    Holding { at: f64 },
    /// The recent stretch is at the loudest the night has been.
    NearThePeak { energy: f64 },
    /// Tempo is climbing across the recent records.
    TempoRising { from: f64, to: f64 },
    /// And falling.
    TempoFalling { from: f64, to: f64 },
}

/// What the engine currently believes, with its reasoning.
#[derive(Debug, Clone, PartialEq)]
pub struct Reading {
    pub read: SessionRead,
    /// 0..=1. How much evidence is behind it, not how strongly it is held.
    pub confidence: f64,
    /// How many measured records the reading is drawn from.
    pub records: usize,
    pub because: Vec<Because>,
}

/// The context engine.
///
/// Fed one record at a time, as each counts as played. Holds only what it
/// needs: a night is tens of records, so the whole session's evidence is a few
/// hundred bytes and there is nothing to page out or expire.
#[derive(Debug, Default)]
pub struct Engine {
    played: Vec<Played>,
    /// The phase last decided, so an ambiguous reading can keep it.
    phase: Option<SessionPhase>,
}

impl Engine {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// How many records the night has behind it.
    #[must_use]
    pub fn records(&self) -> usize {
        self.played.len()
    }

    /// Note a record that has been played to the room.
    pub fn played(&mut self, record: Played) {
        self.played.push(record);
    }

    /// Forget the night. A new session is a new night.
    pub fn clear(&mut self) {
        self.played.clear();
        self.phase = None;
    }

    /// What the night is doing, or `None` while it has no shape yet.
    ///
    /// `hour` is the local hour, 0..=23, for the environment half of the read.
    /// Out of range lands on the default part of the day rather than failing.
    #[must_use]
    pub fn read(&mut self, hour: u32) -> Option<Reading> {
        let energies: Vec<f64> = self.played.iter().filter_map(Played::energy).collect();
        if energies.len() < ENOUGH {
            return None;
        }

        let mean = |slice: &[f64]| slice.iter().sum::<f64>() / slice.len() as f64;
        let recent_slice = &energies[energies.len().saturating_sub(RECENT)..];
        let recent = mean(recent_slice);
        let whole = mean(&energies);
        let peak = energies.iter().copied().fold(f64::MIN, f64::max);

        let mut because = Vec::new();

        // The phase. Ordered from the strongest claim down, because a stretch
        // at the top of the night is a peak whether or not it is still rising.
        let phase = if recent >= peak - 0.05 && recent >= HOT {
            because.push(Because::NearThePeak { energy: recent });
            SessionPhase::Peak
        } else if recent > whole + MARGIN {
            because.push(Because::Rising {
                from: whole,
                to: recent,
            });
            SessionPhase::Heat
        } else if recent < whole - MARGIN {
            because.push(Because::Falling {
                from: whole,
                to: recent,
            });
            if recent < COOL {
                SessionPhase::ChillOut
            } else {
                SessionPhase::Cooldown
            }
        } else {
            because.push(Because::Holding { at: recent });
            // Ambiguous: keep what was decided, or open with the warm-up,
            // which is what a night with no direction yet actually is.
            self.phase.unwrap_or(SessionPhase::WarmUp)
        };
        self.phase = Some(phase);

        // Tempo, as direction only. It is not in the energy figure -- see the
        // module note -- but a set climbing from 95 to 130 BPM is doing
        // something a DJ would want said out loud.
        let tempos: Vec<f64> = self.played.iter().filter_map(|p| p.bpm).collect();
        if tempos.len() >= ENOUGH {
            let recent_bpm = mean(&tempos[tempos.len().saturating_sub(RECENT)..]);
            let whole_bpm = mean(&tempos);
            // Two BPM: below that it is one record's grid being half a beat
            // out, not the night changing gear.
            if recent_bpm > whole_bpm + 2.0 {
                because.push(Because::TempoRising {
                    from: whole_bpm,
                    to: recent_bpm,
                });
            } else if recent_bpm < whole_bpm - 2.0 {
                because.push(Because::TempoFalling {
                    from: whole_bpm,
                    to: recent_bpm,
                });
            }
        }

        // Confidence is about how much the reading rests on, not how strongly
        // it is held: a dozen measured records is as sure as this gets, and a
        // night of unanalysed files is not sure at all.
        let measured = energies.len() as f64 / self.played.len().max(1) as f64;
        let depth = (energies.len() as f64 / 12.0).min(1.0);

        Some(Reading {
            read: SessionRead {
                phase,
                #[allow(clippy::cast_possible_truncation)]
                energy: recent as f32,
                environment: EnvironmentContext {
                    time_of_day: TimeOfDay::from_hour(hour),
                },
            },
            confidence: depth * measured,
            records: energies.len(),
            because,
        })
    }
}

/// The reading, as the snapshot pump reads it.
///
/// Lock-free because it is read sixty times a second and written once a
/// record. The same reasoning as [`crate::setrec::RecordingState`]: reaching
/// through a mutex on every frame to find out that nothing has changed is a
/// lock the interface pays for and never uses.
#[derive(Debug, Default)]
pub struct NightState {
    /// Index into [`SessionPhase::ALL`], or [`NO_READING`].
    phase: AtomicU8,
    /// `f32` bits, because there is no `AtomicF32`.
    energy: AtomicU32,
    confidence: AtomicU32,
}

/// What [`NightState::phase`] holds when nothing has been read yet.
///
/// A sentinel rather than a second flag: two atomics that must agree are two
/// atomics that can disagree between one load and the next.
const NO_READING: u8 = u8::MAX;

impl NightState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            phase: AtomicU8::new(NO_READING),
            ..Self::default()
        }
    }

    /// Publish a reading, or the absence of one.
    pub fn publish(&self, reading: Option<&Reading>) {
        match reading {
            Some(reading) => {
                self.energy
                    .store(reading.read.energy.to_bits(), Ordering::Relaxed);
                #[allow(clippy::cast_possible_truncation)]
                self.confidence
                    .store((reading.confidence as f32).to_bits(), Ordering::Relaxed);
                #[allow(clippy::cast_possible_truncation)]
                let index = SessionPhase::ALL
                    .iter()
                    .position(|p| *p == reading.read.phase)
                    .unwrap_or(0) as u8;
                // The phase goes last, with a release: it is the field that
                // says the others mean anything, so a reader that sees it must
                // be able to trust what it finds beside it.
                self.phase.store(index, Ordering::Release);
            }
            None => self.phase.store(NO_READING, Ordering::Release),
        }
    }

    /// What to put in the snapshot: `None` until the night has a shape.
    #[must_use]
    pub fn read(&self, hour: u32) -> Option<SessionRead> {
        let index = self.phase.load(Ordering::Acquire);
        if index == NO_READING {
            return None;
        }
        Some(SessionRead {
            phase: SessionPhase::ALL
                .get(index as usize)
                .copied()
                .unwrap_or_default(),
            energy: f32::from_bits(self.energy.load(Ordering::Relaxed)),
            environment: EnvironmentContext {
                time_of_day: TimeOfDay::from_hour(hour),
            },
        })
    }

    /// How much evidence is behind the published reading, 0..=1.
    #[must_use]
    pub fn confidence(&self) -> f32 {
        f32::from_bits(self.confidence.load(Ordering::Relaxed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A record at a given loudness, with a tempo.
    fn record(lufs: f64, bpm: f64) -> Played {
        Played {
            bpm: Some(bpm),
            lufs: Some(lufs),
        }
    }

    /// Twenty-two hundred hours: the hour matters only to the environment half.
    const HOUR: u32 = 22;

    fn night(records: &[Played]) -> Engine {
        let mut engine = Engine::new();
        for record in records {
            engine.played(*record);
        }
        engine
    }

    /// **A night with nothing in it has no shape, and says so.**
    ///
    /// The failure this prevents is on the record: `SessionRead` was once
    /// defaulted to *Peak at 0.95*, so the interface announced peak time
    /// thirty seconds into a warm-up. Two records can only rise or fall, and
    /// either reading would be one record's mastering away from the opposite.
    #[test]
    fn a_night_with_too_little_in_it_gets_no_reading() {
        assert!(night(&[]).read(HOUR).is_none());
        assert!(night(&[record(-12.0, 120.0)]).read(HOUR).is_none());
        assert!(
            night(&[record(-12.0, 120.0), record(-10.0, 122.0)])
                .read(HOUR)
                .is_none(),
            "two records were treated as an arc"
        );
        assert!(
            night(&[
                record(-12.0, 120.0),
                record(-10.0, 122.0),
                record(-9.0, 124.0)
            ])
            .read(HOUR)
            .is_some(),
            "three records is the threshold and did not produce a reading"
        );
    }

    /// Records nobody has analysed are not evidence, however many there are.
    #[test]
    fn unmeasured_records_are_not_evidence() {
        let unanalysed = Played {
            bpm: None,
            lufs: None,
        };
        let mut engine = night(&[unanalysed; 8]);
        assert!(
            engine.read(HOUR).is_none(),
            "a night of unanalysed files produced a reading of the room"
        );
    }

    /// **A set that climbs reads as building.**
    #[test]
    fn a_rising_set_reads_as_heat() {
        let mut engine = night(&[
            record(-16.0, 118.0),
            record(-15.0, 120.0),
            record(-15.0, 120.0),
            record(-13.0, 124.0),
            record(-11.0, 126.0),
            record(-10.0, 128.0),
        ]);
        let reading = engine.read(HOUR).expect("six records is a shape");
        assert_eq!(reading.read.phase, SessionPhase::Heat);
        assert!(
            reading
                .because
                .iter()
                .any(|b| matches!(b, Because::Rising { .. })),
            "it called the room building without saying what rose: {:?}",
            reading.because
        );
        assert!(
            reading
                .because
                .iter()
                .any(|b| matches!(b, Because::TempoRising { .. })),
            "the tempo climbed eight BPM and nothing said so"
        );
    }

    /// **A set held at the top of its own range is a peak, not a climb.**
    ///
    /// The distinction that matters to a DJ: *heat* is where you are still
    /// spending, *peak* is where you are holding. A reading that only knew
    /// "rising" would announce the peak on the way down.
    #[test]
    fn a_set_at_its_own_ceiling_reads_as_peak() {
        let mut engine = night(&[
            record(-16.0, 120.0),
            record(-12.0, 124.0),
            record(-7.0, 128.0),
            record(-6.5, 128.0),
            record(-6.0, 128.0),
        ]);
        let reading = engine.read(HOUR).expect("a reading");
        assert_eq!(reading.read.phase, SessionPhase::Peak);
        assert!(
            reading.read.energy > 0.8,
            "a peak read as {}",
            reading.read.energy
        );
    }

    /// **And a set coming down reads as coming down.**
    #[test]
    fn a_falling_set_reads_as_a_cooldown() {
        let mut engine = night(&[
            record(-6.0, 128.0),
            record(-6.0, 128.0),
            record(-7.0, 126.0),
            record(-11.0, 120.0),
            record(-12.0, 118.0),
            record(-13.0, 112.0),
        ]);
        let reading = engine.read(HOUR).expect("a reading");
        assert_eq!(reading.read.phase, SessionPhase::Cooldown);
        assert!(
            reading
                .because
                .iter()
                .any(|b| matches!(b, Because::TempoFalling { .. })),
            "the tempo dropped sixteen BPM and nothing said so"
        );
    }

    /// Far enough down and it is not a cooldown any more.
    #[test]
    fn a_set_taken_right_down_reads_as_the_chill_out() {
        let mut engine = night(&[
            record(-6.0, 128.0),
            record(-6.0, 128.0),
            record(-7.0, 126.0),
            record(-18.0, 100.0),
            record(-19.0, 95.0),
            record(-19.0, 90.0),
        ]);
        assert_eq!(
            engine.read(HOUR).expect("a reading").read.phase,
            SessionPhase::ChillOut
        );
    }

    /// **The load-bearing one: the phase does not move unless something moved.**
    ///
    /// The interface morphs to this. A phase flickering between *Heat* and
    /// *Peak* while a DJ was looking at it would be worse than one that lags,
    /// which is why an ambiguous stretch keeps the phase it had rather than
    /// falling back to a default. The density bands settle for the same
    /// reason.
    #[test]
    fn an_ambiguous_stretch_keeps_the_phase_it_had() {
        let mut engine = night(&[
            record(-16.0, 120.0),
            record(-12.0, 124.0),
            record(-7.0, 128.0),
            record(-6.5, 128.0),
            record(-6.0, 128.0),
        ]);
        assert_eq!(
            engine.read(HOUR).expect("a reading").read.phase,
            SessionPhase::Peak
        );

        // Three records that sit exactly where the night already is: no rise,
        // no fall, and not at the ceiling either.
        for _ in 0..3 {
            engine.played(record(-9.0, 126.0));
        }
        let reading = engine.read(HOUR).expect("a reading");
        assert!(
            reading
                .because
                .iter()
                .any(|b| matches!(b, Because::Holding { .. })),
            "the stretch was not ambiguous, so this tests nothing: {:?}",
            reading.because
        );
        assert_eq!(
            reading.read.phase,
            SessionPhase::Peak,
            "an ambiguous stretch moved the phase"
        );
    }

    /// A night that starts flat opens as a warm-up rather than as nothing.
    #[test]
    fn a_flat_opening_is_a_warm_up() {
        let mut engine = night(&[
            record(-14.0, 120.0),
            record(-14.0, 120.0),
            record(-14.0, 120.0),
        ]);
        assert_eq!(
            engine.read(HOUR).expect("a reading").read.phase,
            SessionPhase::WarmUp
        );
    }

    /// Confidence is about how much is behind the reading. Three records is a
    /// shape worth drawing and not one worth trusting.
    #[test]
    fn confidence_grows_with_the_evidence() {
        let mut thin = night(&[
            record(-14.0, 120.0),
            record(-14.0, 120.0),
            record(-14.0, 120.0),
        ]);
        let thin = thin.read(HOUR).expect("a reading").confidence;

        let mut thick = night(&[record(-14.0, 120.0); 12]);
        let thick = thick.read(HOUR).expect("a reading").confidence;

        assert!(thin < 0.3, "three records claimed {thin} confidence");
        assert!(thick > 0.9, "twelve records claimed only {thick}");
    }

    /// Half a night of unanalysed files is half the confidence: the reading is
    /// drawn from what could be measured, and says how much that was.
    #[test]
    fn unmeasured_records_cost_confidence() {
        let unanalysed = Played {
            bpm: None,
            lufs: None,
        };
        let mut measured = night(&[record(-14.0, 120.0); 12]);
        let mut mixed = night(&[record(-14.0, 120.0); 12]);
        for _ in 0..12 {
            mixed.played(unanalysed);
        }
        assert!(
            mixed.read(HOUR).expect("a reading").confidence
                < measured.read(HOUR).expect("a reading").confidence,
            "a night half of which was unanalysed was as confident as one fully measured"
        );
    }

    /// The lock-free hand-off the snapshot reads: nothing published is nothing
    /// read, and a published reading survives the trip intact.
    #[test]
    fn the_published_reading_survives_the_trip() {
        let state = NightState::new();
        assert!(
            state.read(HOUR).is_none(),
            "an unread night claimed a phase"
        );

        let mut engine = night(&[
            record(-16.0, 120.0),
            record(-12.0, 124.0),
            record(-7.0, 128.0),
            record(-6.5, 128.0),
            record(-6.0, 128.0),
        ]);
        let reading = engine.read(HOUR).expect("a reading");
        state.publish(Some(&reading));

        let read = state.read(HOUR).expect("a published reading");
        assert_eq!(read.phase, reading.read.phase);
        assert!((read.energy - reading.read.energy).abs() < f32::EPSILON);
        assert!((f64::from(state.confidence()) - reading.confidence).abs() < 1e-6);

        state.publish(None);
        assert!(
            state.read(HOUR).is_none(),
            "a withdrawn reading was still being read"
        );
    }

    /// A new night is a new night.
    #[test]
    fn clearing_forgets_the_night() {
        let mut engine = night(&[record(-14.0, 120.0); 6]);
        assert!(engine.read(HOUR).is_some());
        engine.clear();
        assert!(engine.read(HOUR).is_none());
        assert_eq!(engine.records(), 0);
    }
}
