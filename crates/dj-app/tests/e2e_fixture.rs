//! The snapshot the interface's layout budget is measured against.
//!
//! # Why the fixture is captured, not rebuilt
//!
//! `ui/e2e/` measures where the controls land when djmanzo opens at 1280x800,
//! and to do that it needs a snapshot. The obvious way to make one is to build
//! a [`dj_control::ParameterRegistry`] here and capture from it. That was the
//! first attempt and it was wrong twice over, in ways that both left the test
//! green while it measured a screen no DJ will ever see.
//!
//! **A fresh registry is all zeros.** The engine seeds it as it starts -- stem
//! volumes to one, gains to their unity points. Captured from the bare
//! registry, every stem reads as muted, the interface concludes the DJ is
//! working with stems, and it unfolds a 359 px module nobody opened.
//!
//! **Nothing is loaded.** An empty deck draws no pad grid and comes out about
//! 200 px shorter than a loaded one -- and the regression this whole budget
//! exists to catch is a loaded track pushing the controls down. A fixture of
//! empty decks measures precisely the case that never fails.
//!
//! So the fixture is captured from the running application, which is the only
//! thing that knows what a snapshot really looks like:
//!
//! ```text
//! DJMANZO_SNAPSHOT_OUT=ui/e2e/snapshot.json DJMANZO_DEMO=<a folder of audio> \
//!   DJMANZO_NULL_AUDIO=1 ./target/debug/djmanzo
//! ```
//!
//! # What this test does instead
//!
//! It cannot re-derive the *values* -- that needs the application running. It
//! can and does check the **shape**: every field the current [`Snapshot`] type
//! serialises must be present in the committed file, and no more. That is the
//! drift this guards against, because a field added in Rust would otherwise
//! leave the browser measuring a state djmanzo no longer produces, still green,
//! still telling you nothing.

use std::collections::BTreeSet;
use std::sync::Arc;

use dj_control::ParameterRegistry;
use dj_core::ParamId;
use dj_core::param::GlobalParam;

/// Where the browser test reads it from.
const FIXTURE: &str = "../../ui/e2e/snapshot.json";

/// Decks the fixture was captured with. The engine runs six; the interface
/// opens showing two, and both counts appear in the file.
const DECKS: usize = 6;

fn fixture() -> serde_json::Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    serde_json::from_str(&text).expect("the fixture is JSON")
}

/// The keys of one JSON object, or an empty set when it is not one.
fn keys(value: &serde_json::Value) -> BTreeSet<String> {
    value
        .as_object()
        .map(|map| map.keys().cloned().collect())
        .unwrap_or_default()
}

/// The fields the interface reads must all be in the fixture.
///
/// Checked one level down -- the snapshot itself, one deck, and the master --
/// rather than over every path in the tree. Going deeper sounds stricter and is
/// actually wrong: `stem_swap` and a deck's `analysis` are `Option`s, so their
/// inner fields are present or absent according to *state*, and a whole-tree
/// comparison fails whenever the captured state happens to differ from the one
/// the comparison is built from. It would be a test of what was playing.
///
/// These three objects are always present and carry every field a layout
/// depends on, which is the drift worth catching: a field added in Rust would
/// otherwise leave the browser measuring a state djmanzo no longer produces,
/// still green.
#[test]
fn the_browser_fixture_has_the_shape_the_application_sends() {
    // The values here are meaningless -- a bare registry is all zeros -- but
    // the *keys* are the ones the real thing serialises, which is what is
    // being compared.
    let registry = Arc::new(ParameterRegistry::new());
    registry.set(ParamId::Global(GlobalParam::SampleRate), 48_000.0);
    let fresh = serde_json::to_value(dj_app::snapshot::Snapshot::capture(&registry, DECKS))
        .expect("a snapshot serialises");
    let stored = fixture();

    let recapture = "Recapture it from the running application:\n    \
         DJMANZO_SNAPSHOT_OUT=ui/e2e/snapshot.json DJMANZO_DEMO=<audio folder> \\\n      \
         DJMANZO_NULL_AUDIO=1 ./target/debug/djmanzo";

    for (what, fresh, stored) in [
        ("the snapshot", &fresh, &stored),
        ("a deck", &fresh["decks"][0], &stored["decks"][0]),
        ("the master", &fresh["master"], &stored["master"]),
    ] {
        let expected = keys(fresh);
        let have = keys(stored);
        assert!(!expected.is_empty(), "{what} serialised to nothing");

        let missing: Vec<_> = expected.difference(&have).collect();
        let extra: Vec<_> = have.difference(&expected).collect();
        assert!(
            missing.is_empty() && extra.is_empty(),
            "\n{what} no longer matches the fixture the browser's layout budget \
             draws.\n\n  in the type but not the fixture: {missing:?}\n  \
             in the fixture but not the type: {extra:?}\n\n{recapture}\n"
        );
    }
}

/// The pad pages the interface asks for, as a golden file.
///
/// Generated rather than captured, unlike the snapshot: `pad_pages` is a pure
/// function of `dj_core::PadPage::ALL`, so it *can* be re-derived here and a
/// golden file is the stronger guard -- it fails on any change to the pages, not
/// merely on a change to their shape.
///
/// It exists because a browser stub that answers this command with `null` draws
/// **no pad zone at all**, and `Deck.svelte` says exactly what that means where
/// it handles the empty case: "a deck missing its whole performance surface with
/// nothing saying so". The layout budget spent three runs measuring that deck.
///
/// ```text
/// DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture
/// ```
#[test]
fn the_browser_fixture_has_the_pad_pages_the_interface_asks_for() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/e2e/pad-pages.json");
    let fresh = serde_json::to_string_pretty(&dj_app::commands::pad_pages(1))
        .expect("the pad pages serialise");

    if std::env::var_os("DJMANZO_BLESS").is_some() {
        std::fs::write(&path, format!("{fresh}\n")).expect("writing the pad pages");
        return;
    }

    let stored = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "{}: {error}\n\nGenerate it with:\n    \
             DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture",
            path.display()
        )
    });
    assert_eq!(
        stored.trim(),
        fresh.trim(),
        "\nThe pad pages have changed, so the browser's layout budget is drawing a \
         performance surface djmanzo no longer has.\n\nRegenerate with:\n    \
         DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture\n"
    );
}

/// The fixture has to describe a screen worth measuring.
///
/// Both of these were false in an earlier version of the fixture, and both
/// left the budget passing against a layout no DJ meets: a zero sample rate
/// draws "Waiting for the engine…", and empty decks draw no pads.
#[test]
fn the_fixture_describes_a_running_engine_with_records_on_it() {
    let stored = fixture();

    let rate = stored["master"]["sample_rate"].as_f64().unwrap_or(0.0);
    assert!(
        rate > 0.0,
        "the interface treats a zero sample rate as no engine and draws nothing to measure"
    );

    let loaded = stored["decks"]
        .as_array()
        .map(|decks| {
            decks
                .iter()
                .filter(|deck| deck["loaded"].as_bool().unwrap_or(false))
                .count()
        })
        .unwrap_or(0);
    assert!(
        loaded >= 2,
        "only {loaded} deck(s) are loaded; an empty deck draws no pad grid and is about \
         200 px shorter, so this would measure the case that never fails"
    );
}
