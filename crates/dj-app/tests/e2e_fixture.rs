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
/// These four objects are always present and carry every field a layout
/// depends on, which is the drift worth catching: a field added in Rust would
/// otherwise leave the browser measuring a state djmanzo no longer produces,
/// still green.
///
/// **The attention budget was not one of them until §18 grew a field**, and
/// that is exactly how it was found: `room_for` went onto `Attention`, the
/// browser kept measuring a budget without it, and every gate stayed green. It
/// qualifies on the same ground the other three do -- it is never an `Option`
/// and never absent -- so it is checked now.
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
        (
            "the attention budget",
            &fresh["attention"],
            &stored["attention"],
        ),
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

/// **The fixture's attention budget is one djmanzo really produces.**
///
/// The shape test above compares *keys*, which is the right guard for a
/// capture: the values are whatever was playing. The budget is the exception,
/// because it is not a measurement — it is one of four constants, chosen by
/// [`Attention::for_context`], and a fixture carrying five of its fields from a
/// capture and a sixth from somebody's memory is a fixture the browser measures
/// a state against that djmanzo has never been in.
///
/// That is not hypothetical: `room_for` was added to the budget and the field
/// was written into this file by hand rather than by recapturing, because a
/// recapture on different audio would have moved every number the layout budget
/// is drawn against. This is what makes that safe — the block has to equal one
/// of the four exactly, or the file is wrong.
#[test]
fn the_fixture_carries_a_budget_djmanzo_can_actually_be_in() {
    use dj_app::cockpit::Attention;

    let stored = fixture()["attention"].clone();
    let real = [
        ("performing", Attention::performing()),
        ("preparing", Attention::preparing()),
        ("learning", Attention::learning()),
        ("emergency", Attention::emergency()),
    ];
    assert!(
        real.iter().any(|(_, budget)| {
            serde_json::to_value(budget).expect("a budget serialises") == stored
        }),
        "\nthe fixture's attention budget is not one djmanzo produces:\n  \
         {stored:#}\n\nthe four it does produce are:\n{}\n",
        real.iter()
            .map(|(name, budget)| format!(
                "  {name}: {:#}",
                serde_json::to_value(budget).expect("a budget serialises")
            ))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// §54's functional presets, as a golden file.
///
/// Generated rather than captured, like the pad pages: `setups` is a pure
/// function of `dj_app::setup::ALL`, so a golden file is the stronger guard —
/// it fails on any change to what a preset *does*, not merely on a change to
/// its shape. That matters more here than anywhere: the claim a browser test
/// makes about §54 is that one press reaches six systems, and it can only make
/// it against the six the application really sends.
///
/// ```text
/// DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture
/// ```
#[test]
fn the_browser_fixture_has_the_setups_djmanzo_offers() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/e2e/setups.json");
    let fresh =
        serde_json::to_string_pretty(&dj_app::commands::setups()).expect("the setups serialise");

    if std::env::var_os("DJMANZO_BLESS").is_some() {
        std::fs::write(&path, format!("{fresh}\n")).expect("writing the setups");
        return;
    }

    let stored = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "{}: {error}\n\nGenerate it with:\n    \
             DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture",
            path.display()
        )
    });
    let stored: serde_json::Value =
        serde_json::from_str(&stored).expect("the stored setups are JSON");
    let fresh: serde_json::Value = serde_json::from_str(&fresh).expect("the fresh setups are JSON");
    assert_eq!(
        stored, fresh,
        "\nThe functional presets have changed, so the browser is checking a \
         press against a list of what djmanzo no longer does.\n\nRegenerate \
         with:\n    DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture\n"
    );
}

/// §20's columns, as a golden file.
///
/// This was fifteen entries typed out by hand in `shell.ts`, under a comment
/// claiming there were fourteen — and the count had been wrong for at least
/// one column before anybody noticed, because nothing compared the two lists.
/// Adding §20's sixteenth found it: the picker in the browser tests had no
/// row to tick, and the test that needed one failed for a reason that had
/// nothing to do with the column.
///
/// `library_columns` is a pure function of `columns::Column::ALL`, so the
/// stub is generated from it for the same reason the setups above are: a
/// hand-written copy of a table is a second description of it, and the copy is
/// always the one that goes stale.
///
/// ```text
/// DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture
/// ```
#[test]
fn the_browser_fixture_has_the_columns_djmanzo_offers() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/e2e/columns.json");
    let fresh = serde_json::to_string_pretty(&dj_app::commands::library_columns())
        .expect("the columns serialise");

    if std::env::var_os("DJMANZO_BLESS").is_some() {
        std::fs::write(&path, format!("{fresh}\n")).expect("writing the columns");
        return;
    }

    let stored = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "{}: {error}\n\nGenerate it with:\n    \
             DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture",
            path.display()
        )
    });
    let stored: serde_json::Value =
        serde_json::from_str(&stored).expect("the stored columns are JSON");
    let fresh: serde_json::Value =
        serde_json::from_str(&fresh).expect("the fresh columns are JSON");
    assert_eq!(
        stored, fresh,
        "\nThe library columns have changed, so the browser is ticking boxes \
         against a list of columns djmanzo no longer offers.\n\nRegenerate \
         with:\n    DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture\n"
    );
}

/// §16's knowledge packs, as a golden file.
///
/// `knowledge_packs` is a pure function of `dj_assistant::pack::ALL` and the
/// technique catalogue, so this is generated rather than captured for the same
/// reason as the setups above — and for one more. The `teaches` count is the
/// only number in the interface that says what choosing a pack *costs*, and it
/// is derived from a table on the other side of the workspace. A stub written
/// by hand would go on reporting a count nothing computes.
///
/// ```text
/// DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture
/// ```
#[test]
fn the_browser_fixture_has_the_packs_djmanzo_teaches_from() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/e2e/packs.json");
    let fresh = serde_json::to_string_pretty(&dj_app::commands::knowledge_packs())
        .expect("the packs serialise");

    if std::env::var_os("DJMANZO_BLESS").is_some() {
        std::fs::write(&path, format!("{fresh}\n")).expect("writing the packs");
        return;
    }

    let stored = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "{}: {error}\n\nGenerate it with:\n    \
             DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture",
            path.display()
        )
    });
    let stored: serde_json::Value =
        serde_json::from_str(&stored).expect("the stored packs are JSON");
    let fresh: serde_json::Value = serde_json::from_str(&fresh).expect("the fresh packs are JSON");
    assert_eq!(
        stored, fresh,
        "\nThe knowledge packs have changed, so the browser is checking a \
         curriculum djmanzo no longer teaches.\n\nRegenerate with:\n    \
         DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture\n"
    );
}

/// §48's costs at Eco, as a golden file.
///
/// Generated from `dj_app::thrift::Spend::ALL`, which is held to §48's own
/// priority — nothing in the audio band ever given up, nothing cheaper kept
/// while something dearer goes. Eco rather than Ultra because Eco is where the
/// claim is: at Ultra the list is all *kept* and would pass whatever the
/// ordering said.
///
/// ```text
/// DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture
/// ```
#[test]
fn the_browser_fixture_has_what_a_struggling_machine_gives_up() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/e2e/spends.json");
    let fresh = serde_json::to_string_pretty(&dj_app::commands::under_load("Eco".to_owned()))
        .expect("the costs serialise");

    if std::env::var_os("DJMANZO_BLESS").is_some() {
        std::fs::write(&path, format!("{fresh}\n")).expect("writing the costs");
        return;
    }

    let stored = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "{}: {error}\n\nGenerate it with:\n    \
             DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture",
            path.display()
        )
    });
    let stored: serde_json::Value =
        serde_json::from_str(&stored).expect("the stored costs are JSON");
    let fresh: serde_json::Value = serde_json::from_str(&fresh).expect("the fresh costs are JSON");
    assert_eq!(
        stored, fresh,
        "\nWhat djmanzo gives up under load has changed, so the browser is \
         checking a priority it no longer keeps.\n\nRegenerate with:\n    \
         DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture\n"
    );
}

/// §8's adaptation levels, as a golden file.
///
/// Generated from `dj_app::level::Level::ALL`, which holds each level's posture
/// and freedoms against the tables that own them. A hand-written stub of seven
/// would let the browser check a press against a range djmanzo no longer has —
/// and this is the axis where that matters most, since one press writes the
/// posture and all six of §79's locks.
///
/// ```text
/// DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture
/// ```
#[test]
fn the_browser_fixture_has_the_levels_djmanzo_offers() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/e2e/levels.json");
    let fresh = serde_json::to_string_pretty(&dj_app::commands::adaptation_levels())
        .expect("the levels serialise");

    if std::env::var_os("DJMANZO_BLESS").is_some() {
        std::fs::write(&path, format!("{fresh}\n")).expect("writing the levels");
        return;
    }

    let stored = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "{}: {error}\n\nGenerate it with:\n    \
             DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture",
            path.display()
        )
    });
    let stored: serde_json::Value =
        serde_json::from_str(&stored).expect("the stored levels are JSON");
    let fresh: serde_json::Value = serde_json::from_str(&fresh).expect("the fresh levels are JSON");
    assert_eq!(
        stored, fresh,
        "\nThe adaptation levels have changed, so the browser is checking a \
         press against a range djmanzo no longer offers.\n\nRegenerate with:\n    \
         DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture\n"
    );
}

/// §32's themes, as a golden file.
///
/// Generated from `dj_app::theme::ALL`, which is itself checked against
/// `packages.ts` in both directions. The browser needs it for the half of the
/// picker that is *not* drawn from the interface's own package list: the nine
/// themes §32 asked for and djmanzo has not built. Written by hand here, those
/// nine would be a third copy of a list that already exists twice for good
/// reasons.
///
/// ```text
/// DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture
/// ```
#[test]
fn the_browser_fixture_has_the_themes_djmanzo_accounts_for() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/e2e/themes.json");
    let fresh =
        serde_json::to_string_pretty(&dj_app::commands::themes()).expect("the themes serialise");

    if std::env::var_os("DJMANZO_BLESS").is_some() {
        std::fs::write(&path, format!("{fresh}\n")).expect("writing the themes");
        return;
    }

    let stored = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "{}: {error}\n\nGenerate it with:\n    \
             DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture",
            path.display()
        )
    });
    let stored: serde_json::Value =
        serde_json::from_str(&stored).expect("the stored themes are JSON");
    let fresh: serde_json::Value = serde_json::from_str(&fresh).expect("the fresh themes are JSON");
    assert_eq!(
        stored, fresh,
        "\nThe theme table has changed, so the browser is explaining a gap \
         djmanzo no longer has.\n\nRegenerate with:\n    \
         DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture\n"
    );
}

/// §81's six kinds of night, as a golden file.
///
/// The Night panel used to carry these eighteen strings itself. Moving them to
/// Rust only helps if the *browser* fixture is derived too: a hand-written stub
/// of six would be the copy again, one file further out, and a seventh occasion
/// would still reach a green test suite offering six.
///
/// ```text
/// DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture
/// ```
#[test]
fn the_browser_fixture_has_the_nights_djmanzo_knows() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/e2e/night-settings.json");
    let fresh = serde_json::to_string_pretty(&dj_app::commands::night_settings())
        .expect("the settings serialise");

    if std::env::var_os("DJMANZO_BLESS").is_some() {
        std::fs::write(&path, format!("{fresh}\n")).expect("writing the settings");
        return;
    }

    let stored = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "{}: {error}\n\nGenerate it with:\n    \
             DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture",
            path.display()
        )
    });
    let stored: serde_json::Value =
        serde_json::from_str(&stored).expect("the stored settings are JSON");
    let fresh: serde_json::Value =
        serde_json::from_str(&fresh).expect("the fresh settings are JSON");
    assert_eq!(
        stored, fresh,
        "\nThe kinds of night have changed, so the browser is offering an \
         evening djmanzo cannot file.\n\nRegenerate with:\n    \
         DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture\n"
    );
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
    // Compared as JSON rather than as text. A Windows runner checks the file
    // out with CRLF line endings, and a string comparison would then fail on
    // every line for a reason that has nothing to do with the pad pages --
    // `trim` only touches the ends. Parsing also makes the check indifferent to
    // how the file happens to be formatted, which is what a golden file should
    // be about.
    let stored: serde_json::Value =
        serde_json::from_str(&stored).expect("the stored pad pages are JSON");
    let fresh: serde_json::Value =
        serde_json::from_str(&fresh).expect("the fresh pad pages are JSON");
    assert_eq!(
        stored, fresh,
        "\nThe pad pages have changed, so the browser's layout budget is drawing a \
         performance surface djmanzo no longer has.\n\nRegenerate with:\n    \
         DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture\n"
    );
}

/// The surfaces the cockpit can place, as a golden file.
///
/// Generated rather than captured, for the same reason as the pad pages:
/// `cockpit::surfaces()` is a constant table, so it can be re-derived here and
/// a golden file fails on any change to it rather than merely on a change to
/// its shape.
///
/// The browser reads it to know what each surface is called. A surface added in
/// Rust and not blessed here leaves the dock tests measuring a cockpit djmanzo
/// no longer has.
///
/// ```text
/// DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture
/// ```
#[test]
fn the_browser_fixture_has_the_surfaces_the_cockpit_can_place() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/e2e/surfaces.json");
    let fresh =
        serde_json::to_string_pretty(&dj_app::cockpit::surfaces()).expect("the surfaces serialise");

    if std::env::var_os("DJMANZO_BLESS").is_some() {
        std::fs::write(&path, format!("{fresh}\n")).expect("writing the surfaces");
        return;
    }

    let stored = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "{}: {error}\n\nGenerate it with:\n    \
             DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture",
            path.display()
        )
    });
    // Compared as JSON, not as text, so a Windows checkout's CRLF endings do
    // not fail every line for a reason with nothing to do with the surfaces.
    let stored: serde_json::Value =
        serde_json::from_str(&stored).expect("the stored surfaces are JSON");
    let fresh: serde_json::Value =
        serde_json::from_str(&fresh).expect("the fresh surfaces are JSON");
    assert_eq!(
        stored, fresh,
        "\nThe cockpit's surfaces have changed, so the browser's dock tests are \
         measuring a cockpit djmanzo no longer has.\n\nRegenerate with:\n    \
         DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture\n"
    );
}

/// The waveform's semantic layers, as a golden file.
///
/// The same reasoning as the surfaces: `dj_render::layers()` is a constant
/// table, so it can be re-derived here and a golden file fails on any change to
/// it. The browser reads it to check the other direction — that every
/// `data-layer` actually on screen is one djmanzo declares — which is how §25's
/// inventory stays a fact rather than a list somebody maintains.
///
/// ```text
/// DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture
/// ```
#[test]
fn the_browser_fixture_has_the_waveform_layers_the_renderer_declares() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/e2e/layers.json");
    // From the command's own shape rather than from `dj_render::layers()`: the
    // table gained `choosable` and `why_not` when §8's waveform row became a
    // preference, and a golden file of the bare inventory would have left the
    // browser stubbing a picker with no idea which boxes it may tick.
    let fresh = serde_json::to_string_pretty(&dj_app::commands::layer_choices())
        .expect("the layers serialise");

    if std::env::var_os("DJMANZO_BLESS").is_some() {
        std::fs::write(&path, format!("{fresh}\n")).expect("writing the layers");
        return;
    }

    let stored = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "{}: {error}\n\nGenerate it with:\n    \
             DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture",
            path.display()
        )
    });
    let stored: serde_json::Value =
        serde_json::from_str(&stored).expect("the stored layers are JSON");
    let fresh: serde_json::Value = serde_json::from_str(&fresh).expect("the fresh layers are JSON");
    assert_eq!(
        stored, fresh,
        "\nThe waveform's layers have changed, so the browser is checking what it \
         draws against a list djmanzo no longer has.\n\nRegenerate with:\n    \
         DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture\n"
    );
}

/// The browser's transition styles are djmanzo's.
///
/// The five styles and what each does are a table in `dj_app::shape` -- the one
/// the automix performs -- so the browser harness stubs `transition_styles`
/// from this file rather than from a list somebody typed. That is not
/// housekeeping: `vocal drop` was in the vocabulary and performed by the
/// automix for months while no panel offered it, because the interface's copy
/// of the list was hand-written and nothing made it wrong when a style was
/// added.
///
/// ```text
/// DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture
/// ```
#[test]
fn the_browser_fixture_has_the_transition_styles_djmanzo_offers() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/e2e/styles.json");
    let fresh = serde_json::to_string_pretty(&dj_app::commands::transition_styles())
        .expect("the styles serialise");

    if std::env::var_os("DJMANZO_BLESS").is_some() {
        std::fs::write(&path, format!("{fresh}\n")).expect("writing the styles");
        return;
    }

    let stored = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "{}: {error}\n\nGenerate it with:\n    \
             DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture",
            path.display()
        )
    });
    let stored: serde_json::Value =
        serde_json::from_str(&stored).expect("the stored styles are JSON");
    let fresh: serde_json::Value = serde_json::from_str(&fresh).expect("the fresh styles are JSON");
    assert_eq!(
        stored, fresh,
        "\nThe transition styles have changed, so the browser is testing panels \
         against styles djmanzo no longer offers -- or missing one it does.\n\n\
         Regenerate with:\n    \
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

/// §5B's two deck compositions, resolved, as a golden file.
///
/// The browser cannot reach these any other way. Every other arrangement test
/// presses a preset and watches what the shell *asks*; a composition is the one
/// thing that changes what a deck is *made of*, and the deck is built from the
/// resolved tree that `layout_tree` answers with. A tree typed out by hand here
/// would be a third description of a deck — after `layout::builtin()` and
/// `widgets::from_layout` — and the first one to drift.
///
/// More than one, because each is the others' control: the scratch deck has a
/// platter and a folded stem module, the stem deck has the ordinary wheel and
/// an open one, and the supervisory deck has neither and no pads. A single
/// fixture could pass with every prop ignored.
///
/// ```text
/// DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture
/// ```
#[test]
fn the_browser_fixture_has_the_deck_compositions_5b_names() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/e2e/compositions.json");

    // `Starter` is here for §5B's supervisory mode rather than for a
    // composition of its own: "performance display becomes simplified" is a
    // claim about what is *not* on the deck, and the only way to check that a
    // reduction really reduces is to build a deck out of it and look.
    let wanted = ["Scratch", "Starter", "Stem Performance"];
    let trees: std::collections::BTreeMap<String, dj_app::widgets::Resolved> =
        dj_app::layout::builtin()
            .into_iter()
            .filter(|layout| wanted.contains(&layout.name.as_str()))
            .map(|layout| {
                let name = layout.name.clone();
                (
                    name,
                    dj_app::widgets::resolve(&dj_app::widgets::from_layout(&layout)),
                )
            })
            .collect();
    assert_eq!(
        trees.len(),
        wanted.len(),
        "djmanzo ships {} of the compositions §5B's arrangements name; the browser \
         cannot drive a deck composition that does not exist",
        trees.len()
    );

    let fresh = serde_json::to_string_pretty(&trees).expect("the trees serialise");

    if std::env::var_os("DJMANZO_BLESS").is_some() {
        std::fs::write(&path, format!("{fresh}\n")).expect("writing the compositions");
        return;
    }

    let stored = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "{}: {error}\n\nGenerate it with:\n    \
             DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture",
            path.display()
        )
    });
    let stored: serde_json::Value =
        serde_json::from_str(&stored).expect("the stored compositions are JSON");
    let fresh: serde_json::Value =
        serde_json::from_str(&fresh).expect("the fresh compositions are JSON");
    assert_eq!(
        stored, fresh,
        "\nThe deck compositions have changed, so the browser is building a deck \
         djmanzo no longer makes.\n\nRegenerate with:\n    \
         DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture\n"
    );
}

/// §40's list and which half the assistant sees, as a golden file.
///
/// Generated from `dj_app::sight::ALL`. Hand-written in the harness it would be
/// the copy the panel was just relieved of, one file further out — and the half
/// that matters is the *unseen* half, where a stale copy would go on telling a
/// DJ the assistant cannot see something it now can.
///
/// ```text
/// DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture
/// ```
#[test]
fn the_browser_fixture_has_what_the_assistant_can_and_cannot_see() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/e2e/sight.json");
    let fresh = serde_json::to_string_pretty(&dj_app::commands::assistant_sight())
        .expect("the list serialises");

    if std::env::var_os("DJMANZO_BLESS").is_some() {
        std::fs::write(&path, format!("{fresh}\n")).expect("writing the list");
        return;
    }

    let stored = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "{}: {error}\n\nGenerate it with:\n    \
             DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture",
            path.display()
        )
    });
    let stored: serde_json::Value = serde_json::from_str(&stored).expect("the stored list is JSON");
    let fresh: serde_json::Value = serde_json::from_str(&fresh).expect("the fresh list is JSON");
    assert_eq!(
        stored, fresh,
        "\nWhat the assistant can see has changed, so the browser is checking a \
         panel against a claim djmanzo no longer makes.\n\nRegenerate with:\n    \
         DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture\n"
    );
}
