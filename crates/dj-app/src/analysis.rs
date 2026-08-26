//! Running the analyser, remembering what it found.
//!
//! `dj-analysis` works out tempo, key and loudness. This is what actually calls
//! it: on a worker thread when a track loads, once per track ever, with the
//! result kept on disk so the second load is instant.
//!
//! # Why it is not on the audio thread, and not on the UI thread either
//!
//! Analysis reads the whole track and runs several FFT passes over it —
//! hundreds of milliseconds for a five-minute file, seconds for a long one.
//! On the audio thread that is a guaranteed dropout; on the UI thread it is a
//! frozen window at the exact moment the DJ is trying to load the next track.
//! So it runs on a worker, and the interface shows the track immediately with
//! its analysis arriving a moment later.
//!
//! # Why the cache is keyed by content
//!
//! [`TrackId`] is a hash of the audio, not of the path. That is the right key
//! for three reasons: the same file analysed twice gives the same answer, a
//! file that moves or is renamed keeps its analysis, and two copies of the same
//! track in different folders share one entry. A path-keyed cache gets all
//! three wrong.
//!
//! # Why the on-disk format is its own thing
//!
//! The cached record is a flat, versioned struct rather than a serialisation of
//! [`Analysis`]. Coupling the file format to internal types means any change to
//! those types silently invalidates — or worse, silently misreads — every
//! cached file a user has built up. A deliberate schema with a version number
//! costs a few lines and makes the failure mode "recompute" instead of "wrong
//! BPM".

use dj_analysis::{Analysis, KeyAnalysis, Lufs, TempoAnalysis};
use dj_core::{Beatgrid, Bpm, Confidence, DeckId, FramePos, MusicalKey, SampleRate};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Bump when the schema below changes shape or an analyser changes its answer.
///
/// A cached record from an older version is discarded and recomputed rather
/// than read, which is why this number exists: reading an old record with new
/// code is how a library ends up full of confidently wrong BPMs.
const CACHE_VERSION: u32 = 2;

/// What the analyser found, in a form that survives a restart.
///
/// Flat and primitive on purpose — see the module docs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct CachedAnalysis {
    version: u32,
    bpm: Option<f64>,
    /// Frame position of a beat, from which every other beat follows.
    anchor_frames: Option<f64>,
    confidence: Option<f64>,
    /// The rejected octave, usually half or double.
    alternative_bpm: Option<f64>,
    /// Camelot hour, 1..=12.
    key_hour: Option<u8>,
    /// True for the Camelot "B" ring.
    key_major: Option<bool>,
    key_correlation: Option<f64>,
    key_alt_hour: Option<u8>,
    key_alt_major: Option<bool>,
    /// `None` for a silent track, where loudness is negative infinity and JSON
    /// has no way to say so.
    lufs: Option<f64>,
    /// Phrase length in beats, and the beat a phrase starts on. `None` for a
    /// track with no phrase structure, which is a real answer -- see
    /// `dj_analysis::structure`.
    phrase_beats: Option<u32>,
    phrase_anchor: Option<u32>,
    phrase_confidence: Option<f32>,
}

impl CachedAnalysis {
    fn from_analysis(analysis: &Analysis) -> Self {
        let key = analysis.key.as_ref();
        Self {
            version: CACHE_VERSION,
            bpm: analysis.tempo.as_ref().map(|t| t.grid.bpm.get()),
            anchor_frames: analysis.tempo.as_ref().map(|t| t.grid.anchor.get()),
            confidence: analysis.tempo.as_ref().map(|t| t.grid.confidence.get()),
            alternative_bpm: analysis
                .tempo
                .as_ref()
                .and_then(|t| t.alternative)
                .map(Bpm::get),
            key_hour: key.map(|k| k.key.hour()),
            key_major: key.map(|k| k.key.mode() == dj_core::Mode::Major),
            key_correlation: key.map(|k| k.correlation),
            key_alt_hour: key.and_then(|k| k.alternative).map(|a| a.hour()),
            key_alt_major: key
                .and_then(|k| k.alternative)
                .map(|a| a.mode() == dj_core::Mode::Major),
            lufs: analysis
                .loudness
                .get()
                .is_finite()
                .then(|| analysis.loudness.get()),
            phrase_beats: analysis.phrases.map(|p| p.beats),
            phrase_anchor: analysis.phrases.map(|p| p.anchor),
            phrase_confidence: analysis.phrases.map(|p| p.confidence),
        }
    }

    /// Rebuild the analysis, or `None` if the record is from another version or
    /// is internally inconsistent.
    ///
    /// Strict rather than forgiving: a half-readable record is recomputed in a
    /// second, whereas a half-*believed* one puts a wrong grid under a mix.
    fn to_analysis(&self) -> Option<Analysis> {
        if self.version != CACHE_VERSION {
            return None;
        }

        let tempo = match (self.bpm, self.anchor_frames, self.confidence) {
            (Some(bpm), Some(anchor), Some(confidence)) => Some(TempoAnalysis {
                grid: Beatgrid::new(
                    FramePos::new(anchor),
                    Bpm::new(bpm)?,
                    Confidence::new(confidence),
                ),
                alternative: self.alternative_bpm.and_then(Bpm::new),
            }),
            _ => None,
        };

        let key = match (self.key_hour, self.key_major, self.key_correlation) {
            (Some(hour), Some(major), Some(correlation)) => Some(KeyAnalysis {
                key: musical_key(hour, major)?,
                correlation,
                alternative: match (self.key_alt_hour, self.key_alt_major) {
                    (Some(hour), Some(major)) => musical_key(hour, major),
                    _ => None,
                },
            }),
            _ => None,
        };

        // All three or none. A phrase length without the beat it starts on is
        // not half an answer, it is a marker in an unknown place.
        let phrases = match (self.phrase_beats, self.phrase_anchor) {
            (Some(beats), Some(anchor)) => Some(dj_analysis::PhraseAnalysis {
                beats,
                anchor,
                confidence: self.phrase_confidence.unwrap_or(0.0),
            }),
            _ => None,
        };

        Some(Analysis {
            tempo,
            key,
            loudness: self.lufs.map_or(Lufs::SILENCE, Lufs::new),
            phrases,
        })
    }
}

fn musical_key(hour: u8, major: bool) -> Option<MusicalKey> {
    MusicalKey::new(
        hour,
        if major {
            dj_core::Mode::Major
        } else {
            dj_core::Mode::Minor
        },
    )
}

/// Analysis results, by deck and by track.
///
/// Two maps rather than one because they answer different questions. The
/// interface asks "what is on deck 2", and the loader asks "have we seen this
/// audio before" — the second outliving any particular deck assignment, which
/// is the whole value of a content-hash cache.
#[derive(Debug, Default)]
pub struct AnalysisStore {
    by_deck: Mutex<HashMap<u8, Arc<Analysis>>>,
    by_track: Mutex<HashMap<dj_core::TrackId, Arc<Analysis>>>,
    /// Where cached records live. `None` before the app config directory is
    /// known, in which case the cache is memory-only for the session.
    cache_dir: Mutex<Option<PathBuf>>,
}

impl AnalysisStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Point the store at a directory for its on-disk cache.
    ///
    /// Called once the Tauri app handle exists, which is later than
    /// construction; until then everything is in memory and a restart loses it.
    pub fn set_cache_dir(&self, dir: PathBuf) {
        if let Err(error) = std::fs::create_dir_all(&dir) {
            tracing::warn!(%error, "no analysis cache directory; results will not persist");
            return;
        }
        if let Ok(mut slot) = self.cache_dir.lock() {
            *slot = Some(dir);
        }
    }

    /// What is loaded on a deck, if it has been analysed yet.
    #[must_use]
    pub fn for_deck(&self, deck: u8) -> Option<Arc<Analysis>> {
        self.by_deck.lock().ok()?.get(&deck).cloned()
    }

    /// Forget a deck's analysis. Called the instant a new track starts loading,
    /// so the header never shows the previous track's BPM against the new one.
    pub fn clear_deck(&self, deck: DeckId) {
        if let Ok(mut map) = self.by_deck.lock() {
            map.remove(&deck.human_number());
        }
    }

    /// A previous result for this audio, from memory or from disk.
    #[must_use]
    pub fn cached(&self, id: &dj_core::TrackId) -> Option<Arc<Analysis>> {
        if let Ok(map) = self.by_track.lock()
            && let Some(hit) = map.get(id)
        {
            return Some(Arc::clone(hit));
        }

        let analysis = Arc::new(self.read_from_disk(id)?);
        if let Ok(mut map) = self.by_track.lock() {
            map.insert(*id, Arc::clone(&analysis));
        }
        Some(analysis)
    }

    /// Record a result against both the track and the deck it landed on.
    pub fn record(&self, deck: DeckId, id: dj_core::TrackId, analysis: Analysis) {
        let analysis = Arc::new(analysis);
        if let Ok(mut map) = self.by_track.lock() {
            map.insert(id, Arc::clone(&analysis));
        }
        if let Ok(mut map) = self.by_deck.lock() {
            map.insert(deck.human_number(), Arc::clone(&analysis));
        }
        self.write_to_disk(&id, &analysis);
    }

    /// Attach an already-known result to a deck, without re-analysing.
    pub fn assign(&self, deck: DeckId, analysis: Arc<Analysis>) {
        if let Ok(mut map) = self.by_deck.lock() {
            map.insert(deck.human_number(), analysis);
        }
    }

    fn path_for(&self, id: &dj_core::TrackId) -> Option<PathBuf> {
        let dir = self.cache_dir.lock().ok()?.clone()?;
        Some(dir.join(format!("{}.json", id.to_hex())))
    }

    fn read_from_disk(&self, id: &dj_core::TrackId) -> Option<Analysis> {
        let path = self.path_for(id)?;
        let text = std::fs::read_to_string(&path).ok()?;
        // A corrupt or outdated record is discarded rather than repaired.
        // Recomputing costs a second; believing half of one costs a mix.
        let record: CachedAnalysis = serde_json::from_str(&text).ok()?;
        record.to_analysis()
    }

    fn write_to_disk(&self, id: &dj_core::TrackId, analysis: &Analysis) {
        let Some(path) = self.path_for(id) else {
            return;
        };
        let record = CachedAnalysis::from_analysis(analysis);
        match serde_json::to_string(&record) {
            Ok(text) => {
                if let Err(error) = std::fs::write(&path, text) {
                    // Never fatal. A cache that cannot be written just means
                    // analysing again next time.
                    tracing::warn!(%error, "could not cache analysis");
                }
            }
            Err(error) => tracing::warn!(%error, "could not encode analysis"),
        }
    }

    #[must_use]
    pub fn cached_tracks(&self) -> usize {
        self.by_track.lock().map(|m| m.len()).unwrap_or(0)
    }
}

/// Analyse, or return the cached result for audio already seen.
///
/// Blocking and slow. Call it from a worker.
pub fn analyse_or_cached(
    store: &AnalysisStore,
    deck: DeckId,
    id: dj_core::TrackId,
    samples: &[f32],
    sample_rate: SampleRate,
) -> Arc<Analysis> {
    if let Some(hit) = store.cached(&id) {
        store.assign(deck, Arc::clone(&hit));
        return hit;
    }

    let analysis = dj_analysis::analyse(samples, sample_rate);
    store.record(deck, id, analysis);
    store
        .for_deck(deck.human_number())
        .unwrap_or_else(|| Arc::new(analysis))
}

/// The action that trims a freshly analysed track to the reference loudness.
///
/// Returns `None` when there is nothing to say: a silent or unmeasurable track,
/// or one already close enough that moving the trim would be noise. The
/// threshold is half a decibel, which is about where a level change stops being
/// audible on programme material.
#[must_use]
pub fn auto_gain_action(deck: DeckId, analysis: &Analysis) -> Option<String> {
    let gain = analysis.auto_gain_db();
    if !gain.is_finite() || gain.abs() < 0.5 {
        return None;
    }
    // Clamped to the same range the mixer accepts. A track 40 dB quiet is
    // usually a nearly-silent file rather than something to amplify hugely.
    let gain = gain.clamp(-24.0, 24.0);
    Some(format!("deck {} gain {gain:.1}", deck.human_number()))
}

/// Where analysis records live under the app's cache directory.
#[must_use]
pub fn cache_subdir(base: &Path) -> PathBuf {
    base.join("analysis")
}

#[cfg(test)]
mod tests {
    use super::*;
    use dj_core::Mode;

    fn track_id(byte: u8) -> dj_core::TrackId {
        dj_core::TrackId::from_bytes([byte; 32])
    }

    fn analysis() -> Analysis {
        Analysis {
            tempo: Some(TempoAnalysis {
                grid: Beatgrid::new(
                    FramePos::new(1234.5),
                    Bpm::new(128.0).unwrap(),
                    Confidence::new(0.82),
                ),
                alternative: Bpm::new(64.0),
            }),
            key: Some(KeyAnalysis {
                key: MusicalKey::new(8, Mode::Major).unwrap(),
                correlation: 0.74,
                alternative: MusicalKey::new(8, Mode::Minor),
            }),
            loudness: Lufs::new(-9.3),
            phrases: None,
        }
    }

    /// **The point of the cache.** Everything the interface shows and everything
    /// sync depends on has to come back exactly, or a track would analyse
    /// differently on its second load than on its first.
    #[test]
    fn a_cached_record_round_trips_without_losing_anything() {
        let original = analysis();
        let record = CachedAnalysis::from_analysis(&original);
        let restored = record.to_analysis().expect("record should rebuild");
        assert_eq!(restored, original);
    }

    /// A record written by a different version of the analyser must be
    /// recomputed, not read. Believing it is how a library fills up with
    /// confidently wrong BPMs that nobody can explain.
    #[test]
    fn a_record_from_another_version_is_refused() {
        let mut record = CachedAnalysis::from_analysis(&analysis());
        record.version = CACHE_VERSION + 1;
        assert!(record.to_analysis().is_none());
    }

    /// Half a record is not a result. Recomputing costs a second; believing a
    /// grid with no anchor puts a wrong beat under a mix.
    #[test]
    fn an_incomplete_record_yields_no_tempo_rather_than_a_guess() {
        let mut record = CachedAnalysis::from_analysis(&analysis());
        record.anchor_frames = None;
        let restored = record.to_analysis().unwrap();
        assert!(
            restored.tempo.is_none(),
            "rebuilt a grid from half a record"
        );
        // The rest of the record is still perfectly good.
        assert!(restored.key.is_some());
        assert_eq!(restored.loudness, Lufs::new(-9.3));
    }

    /// Silence has no loudness, and JSON has no way to write negative infinity.
    /// It has to survive the trip as silence rather than as a number.
    #[test]
    fn silence_survives_the_round_trip() {
        let silent = Analysis {
            tempo: None,
            key: None,
            loudness: Lufs::SILENCE,
            phrases: None,
        };
        let restored = CachedAnalysis::from_analysis(&silent)
            .to_analysis()
            .unwrap();
        assert_eq!(restored.loudness, Lufs::SILENCE);
        // Not infinity: `Lufs::gain_to` already answers "no gain" for silence,
        // because there is no amount of gain that makes silence louder. Worth
        // asserting from this side too, since a cached record that rebuilt
        // silence as a *number* would quietly reintroduce the infinity.
        assert_eq!(restored.auto_gain_db(), 0.0);
    }

    #[test]
    fn a_track_is_remembered_and_found_again() {
        let store = AnalysisStore::new();
        let deck = DeckId::from_human(1).unwrap();
        store.record(deck, track_id(7), analysis());

        assert_eq!(store.cached_tracks(), 1);
        assert_eq!(store.cached(&track_id(7)).as_deref(), Some(&analysis()));
        assert!(store.cached(&track_id(8)).is_none());
        assert_eq!(store.for_deck(1).as_deref(), Some(&analysis()));
    }

    /// The header must never show the previous track's BPM against a new one,
    /// which is exactly what happens if a deck is not cleared at load.
    #[test]
    fn loading_clears_the_decks_previous_analysis() {
        let store = AnalysisStore::new();
        let deck = DeckId::from_human(2).unwrap();
        store.record(deck, track_id(1), analysis());
        assert!(store.for_deck(2).is_some());

        store.clear_deck(deck);
        assert!(store.for_deck(2).is_none());
        // The track itself is still known: clearing a deck is not forgetting
        // the audio, and reloading it must not re-analyse.
        assert!(store.cached(&track_id(1)).is_some());
    }

    /// The disk cache is the reason the second load of a track is instant.
    #[test]
    fn a_record_survives_a_new_store() {
        let dir = std::env::temp_dir().join(format!("djmanzo-analysis-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        let deck = DeckId::from_human(1).unwrap();
        let first = AnalysisStore::new();
        first.set_cache_dir(dir.clone());
        first.record(deck, track_id(3), analysis());

        // A completely separate store, as a restart would produce.
        let second = AnalysisStore::new();
        second.set_cache_dir(dir.clone());
        assert_eq!(second.cached_tracks(), 0, "nothing in memory yet");
        assert_eq!(
            second.cached(&track_id(3)).as_deref(),
            Some(&analysis()),
            "the record did not survive"
        );
        assert_eq!(second.cached_tracks(), 1, "the disk hit was not memoised");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A store with nowhere to write must still work, just without persistence.
    /// A user whose config directory is unavailable should lose caching, not
    /// analysis.
    #[test]
    fn a_store_with_no_cache_directory_still_works() {
        let store = AnalysisStore::new();
        let deck = DeckId::from_human(1).unwrap();
        store.record(deck, track_id(5), analysis());
        assert!(store.cached(&track_id(5)).is_some());
    }

    /// Corrupt files happen — a full disk, a killed process mid-write. The
    /// result must be a recomputation, not a crash and not a wrong answer.
    #[test]
    fn a_corrupt_record_is_ignored() {
        let dir = std::env::temp_dir().join(format!("djmanzo-corrupt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(format!("{}.json", track_id(9).to_hex())),
            "{ not json",
        )
        .unwrap();

        let store = AnalysisStore::new();
        store.set_cache_dir(dir.clone());
        assert!(store.cached(&track_id(9)).is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **The point of measuring loudness.** A quiet track should be turned up
    /// and a loud one turned down, without the DJ riding the trim.
    #[test]
    fn auto_gain_moves_a_track_towards_the_reference() {
        let deck = DeckId::from_human(1).unwrap();

        let quiet = Analysis {
            loudness: Lufs::new(-20.0),
            phrases: None,
            ..analysis()
        };
        assert_eq!(
            auto_gain_action(deck, &quiet).as_deref(),
            Some("deck 1 gain 6.0")
        );

        let loud = Analysis {
            loudness: Lufs::new(-8.0),
            phrases: None,
            ..analysis()
        };
        assert_eq!(
            auto_gain_action(deck, &loud).as_deref(),
            Some("deck 1 gain -6.0")
        );
    }

    /// Nudging the trim by a fraction of a decibel is noise in the session log
    /// and does nothing anyone can hear.
    #[test]
    fn a_track_already_at_the_reference_is_left_alone() {
        let deck = DeckId::from_human(1).unwrap();
        let already = Analysis {
            loudness: Lufs::new(-14.2),
            phrases: None,
            ..analysis()
        };
        assert_eq!(auto_gain_action(deck, &already), None);
    }

    /// A silent file has no measurable loudness, and the gain to bring silence
    /// to the reference is infinite. It must not reach the mixer.
    #[test]
    fn silence_produces_no_gain_action() {
        let deck = DeckId::from_human(1).unwrap();
        let silent = Analysis {
            loudness: Lufs::SILENCE,
            phrases: None,
            ..analysis()
        };
        assert_eq!(auto_gain_action(deck, &silent), None);
    }

    /// The action has to be one the parser accepts, or auto-gain would be a
    /// string that silently fails at the bus.
    #[test]
    fn the_auto_gain_action_parses() {
        let deck = DeckId::from_human(2).unwrap();
        let quiet = Analysis {
            loudness: Lufs::new(-19.0),
            phrases: None,
            ..analysis()
        };
        let action = auto_gain_action(deck, &quiet).unwrap();
        let parsed =
            dj_core::Action::parse(&action).expect("auto-gain emitted an unparseable action");
        assert_eq!(
            parsed,
            dj_core::Action::Deck {
                deck,
                action: dj_core::DeckAction::SetGainDb(5.0),
            }
        );
    }

    /// Clamped, because a nearly-silent file would otherwise ask for a gain the
    /// mixer does not have and the limiter would spend the night fighting.
    #[test]
    fn an_absurdly_quiet_track_does_not_ask_for_absurd_gain() {
        let deck = DeckId::from_human(1).unwrap();
        let nearly_silent = Analysis {
            loudness: Lufs::new(-90.0),
            phrases: None,
            ..analysis()
        };
        let action = auto_gain_action(deck, &nearly_silent).unwrap();
        assert_eq!(action, "deck 1 gain 24.0");
    }
}
