//! The analysis seam, end to end.
//!
//! The unit tests in `dj-app::analysis` all use a fabricated [`Analysis`], so
//! they prove the caching and the auto-gain arithmetic but say nothing about
//! whether real audio actually reaches the real analyser and comes back with
//! usable numbers. That join is where a wiring mistake would hide: a store that
//! works perfectly against fake data and is never handed any real data looks
//! exactly like a working feature.
//!
//! So this runs the whole path — samples in, cache and deck assignment out — on
//! a synthetic track whose tempo is known by construction.

use dj_app::analysis::{AnalysisStore, analyse_or_cached, auto_gain_action};
use dj_core::{DeckId, SampleRate, TrackId};
use std::f32::consts::TAU;

const SR: SampleRate = SampleRate::DEFAULT;

/// A click track at a known tempo, carrying a C major chord.
///
/// Both halves matter: the clicks are what the onset detector locks onto, and
/// the sustained chord is what the key detector needs. A track with only one is
/// only half a test.
fn track(bpm: f64, seconds: f64, amplitude: f32) -> Vec<f32> {
    let rate = SR.get() as f64;
    let frames = (seconds * rate) as usize;
    let beat = 60.0 / bpm * rate;
    let mut audio = vec![0.0f32; frames * 2];

    for n in 0..frames {
        let t = n as f32 / SR.get() as f32;

        // C, E, G across two octaves.
        let mut v = 0.0f32;
        for hz in [130.81, 164.81, 196.0, 261.63, 329.63, 392.0] {
            v += (TAU * hz * t).sin();
        }
        v /= 6.0;

        // A short percussive burst on each beat.
        let since_beat = (n as f64 % beat) / rate;
        if since_beat < 0.01 {
            let decay = (1.0 - since_beat as f32 / 0.01).powi(2);
            // Alternating sign: a burst of broadband energy rather than a click
            // at one frequency, which is what the flux detector actually keys on.
            let sign = if (n * 7919) % 2 == 0 { 1.0 } else { -1.0 };
            v += decay * sign * 0.8;
        }

        let v = v * amplitude;
        audio[n * 2] = v;
        audio[n * 2 + 1] = v;
    }
    audio
}

fn deck(n: u8) -> DeckId {
    DeckId::from_human(n).unwrap()
}

fn scratch_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("djmanzo-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// **The join.** Real samples through the real analyser, landing on a deck.
#[test]
fn a_real_track_is_analysed_and_lands_on_its_deck() {
    let store = AnalysisStore::new();
    let audio = track(128.0, 30.0, 0.3);

    let analysis = analyse_or_cached(&store, deck(1), TrackId::from_bytes([1; 32]), &audio, SR);

    let tempo = analysis.tempo.expect("a click track should have a tempo");
    assert!(
        (tempo.grid.bpm.get() - 128.0).abs() < 2.0,
        "measured {} against a constructed 128",
        tempo.grid.bpm.get()
    );
    assert!(
        analysis.is_sync_worthy(),
        "a metronomic track should be worth syncing to, confidence was {}",
        tempo.grid.confidence.get()
    );
    assert!(
        analysis.loudness.get().is_finite(),
        "audible material should have a measurable loudness"
    );

    // And it is on the deck the interface will ask about.
    assert_eq!(store.for_deck(1).as_deref(), Some(&*analysis));
}

/// The second load of the same audio must not re-analyse. This is the entire
/// point of hashing content rather than paths, and the difference between a
/// track appearing instantly and a two-second stall mid-set.
#[test]
fn the_same_audio_is_not_analysed_twice() {
    let store = AnalysisStore::new();
    let audio = track(124.0, 25.0, 0.3);
    let id = TrackId::from_bytes([2; 32]);

    let first = analyse_or_cached(&store, deck(1), id, &audio, SR);

    // Same id, deliberately different audio: if this were re-analysed the
    // answer would change, so an unchanged answer proves the cache was used.
    let decoy = track(90.0, 25.0, 0.3);
    let second = analyse_or_cached(&store, deck(2), id, &decoy, SR);

    assert_eq!(&*first, &*second, "the cached result was not reused");
    // Both decks now show it, which is what loading the same track twice means.
    assert_eq!(store.for_deck(1).as_deref(), Some(&*first));
    assert_eq!(store.for_deck(2).as_deref(), Some(&*first));
    assert_eq!(store.cached_tracks(), 1);
}

/// The cache has to survive a restart, or every session starts by re-analysing
/// a library that has not changed.
#[test]
fn analysis_survives_a_restart() {
    let dir = scratch_dir("pipeline-restart");
    let audio = track(130.0, 25.0, 0.3);
    let id = TrackId::from_bytes([3; 32]);

    let first = AnalysisStore::new();
    first.set_cache_dir(dir.clone());
    let before = analyse_or_cached(&first, deck(1), id, &audio, SR);

    let restarted = AnalysisStore::new();
    restarted.set_cache_dir(dir.clone());
    // No audio passed at all: an empty slice cannot be analysed, so anything
    // that comes back must have come off the disk.
    let after = analyse_or_cached(&restarted, deck(1), id, &[], SR);

    // Everything but the energy trajectory, which the cache rounds to three
    // decimals on the way out -- see `CachedAnalysis::from_analysis`. It is a
    // ratio around 1.0 drawn about thirty pixels tall, so the fourth decimal is
    // a thousandth of a pixel and writing it out nearly doubles the file. What
    // has to survive a restart is the *answer*, and it does.
    assert_eq!(
        before.tempo, after.tempo,
        "the grid did not survive a restart"
    );
    assert_eq!(before.key, after.key, "the key did not survive a restart");
    assert_eq!(before.loudness, after.loudness);
    assert_eq!(before.phrases, after.phrases);
    let (was, is) = (
        before.energy.as_ref().expect("a trajectory"),
        after.energy.as_ref().expect("a trajectory survives too"),
    );
    assert_eq!(was.breakdowns, is.breakdowns);
    assert_eq!(was.drops, is.drops);
    assert_eq!(was.first_beat, is.first_beat);
    assert_eq!(was.drive.len(), is.drive.len(), "the trajectory lost beats");
    for (left, right) in was.drive.iter().zip(&is.drive) {
        assert!(
            (left - right).abs() <= 0.0005,
            "a level came back as {right} rather than {left}, which is more than \
             the rounding the cache does"
        );
    }
    assert!(after.tempo.is_some());

    let _ = std::fs::remove_dir_all(&dir);
}

/// **The other half of the analysis's purpose.** A quiet track and a loud one
/// should end up trimmed to the same perceived level, without the DJ touching
/// anything.
#[test]
fn two_tracks_at_different_levels_get_opposite_trims() {
    let store = AnalysisStore::new();

    let quiet = analyse_or_cached(
        &store,
        deck(1),
        TrackId::from_bytes([4; 32]),
        &track(128.0, 20.0, 0.03),
        SR,
    );
    let loud = analyse_or_cached(
        &store,
        deck(2),
        TrackId::from_bytes([5; 32]),
        &track(128.0, 20.0, 0.6),
        SR,
    );

    assert!(
        quiet.loudness.get() < loud.loudness.get() - 10.0,
        "the fixtures are not actually different: {} vs {}",
        quiet.loudness.get(),
        loud.loudness.get()
    );

    let quiet_gain = quiet.auto_gain_db();
    let loud_gain = loud.auto_gain_db();
    assert!(quiet_gain > 0.0, "the quiet track should be turned up");
    assert!(
        loud_gain < quiet_gain,
        "the louder track should be trimmed less"
    );

    // Applying each trim lands both at the same level, which is the point.
    let quiet_after = quiet.loudness.get() + quiet_gain;
    let loud_after = loud.loudness.get() + loud_gain;
    assert!(
        (quiet_after - loud_after).abs() < 0.5,
        "levelled to {quiet_after} and {loud_after}"
    );

    // And both produce an action the parser accepts.
    for (n, analysis) in [(1u8, &quiet), (2, &loud)] {
        if let Some(action) = auto_gain_action(deck(n), analysis) {
            dj_core::Action::parse(&action).unwrap_or_else(|e| {
                panic!("auto-gain emitted {action:?}, which will not parse: {e}")
            });
        }
    }
}

/// Material with no tempo must say so rather than inventing one. Silently
/// syncing to a grid that was never there is the failure the whole crate is
/// written to avoid.
///
/// # The fixture is the hard part
///
/// This started as two sustained sines at 220 and 277 Hz, which the analyser
/// reported as 122.3 BPM at 0.83 confidence — apparently a bad miss. It was
/// not. Those two frequencies share a 1 Hz common factor, so the combined
/// waveform repeats *exactly* once a second, and 122.3 BPM is twice 61.2. The
/// analyser found a real periodicity that the fixture genuinely had.
///
/// So "no pulse" has to mean no repetition at any scale, which a continuous
/// glide guarantees: the frequency never returns to a previous value, so no
/// lag can correlate with any other. Measured at 0.017 confidence against the
/// 0.5 sync threshold — two orders of magnitude clear, rather than marginal.
#[test]
fn a_track_with_no_pulse_reports_no_grid() {
    let store = AnalysisStore::new();

    let rate = SR.get() as f32;
    let seconds = 25.0f32;
    let frames = (seconds * rate) as usize;
    let mut glide = vec![0.0f32; frames * 2];
    let mut phase = 0.0f32;
    for n in 0..frames {
        let t = n as f32 / rate;
        // A slow sweep across a fifth, integrated so the phase stays continuous.
        let hz = 180.0 + 90.0 * (t / seconds);
        phase += TAU * hz / rate;
        let v = phase.sin() * 0.15;
        glide[n * 2] = v;
        glide[n * 2 + 1] = v;
    }

    let analysis = analyse_or_cached(&store, deck(1), TrackId::from_bytes([6; 32]), &glide, SR);
    assert!(
        !analysis.is_sync_worthy(),
        "an unpulsed glide was offered for sync at {:?} BPM, confidence {:?}",
        analysis.tempo.map(|t| t.grid.bpm.get()),
        analysis.tempo.map(|t| t.grid.confidence.get()),
    );
    // It is still a perfectly measurable piece of audio in every other respect.
    assert!(analysis.loudness.get().is_finite());
}
