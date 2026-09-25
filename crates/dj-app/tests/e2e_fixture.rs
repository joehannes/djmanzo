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
/// §30's roles and the pairs a theme may not collapse, as a golden file.
///
/// Blessed from `cockpit::Role` for the reason §20's columns are: the pairs
/// are one judgement, and a second copy in the interface would be a second
/// answer. This is the file `ui/src/appearance.test.ts` holds every shipped
/// palette to -- so adding a pair here fails until a theme that collapses it
/// is fixed, which is §89's *compare appearance* with a mechanism behind it.
///
/// ```text
/// DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture
/// ```
#[test]
fn the_browser_fixture_has_the_roles_a_theme_must_keep_apart() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/e2e/roles.json");
    let fresh = serde_json::to_string_pretty(&dj_app::commands::semantic_roles())
        .expect("the roles serialise");

    if std::env::var_os("DJMANZO_BLESS").is_some() {
        std::fs::write(&path, format!("{fresh}\n")).expect("writing the roles");
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
        serde_json::from_str(&stored).expect("the stored roles are JSON");
    let fresh: serde_json::Value = serde_json::from_str(&fresh).expect("the fresh roles are JSON");
    assert_eq!(
        stored, fresh,
        "\nThe semantic roles have changed, so every shipped palette is being \
         checked against a set of distinctions djmanzo no longer makes -- or is \
         not being checked against one it now does.\n\nRegenerate with:\n    \
         DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture\n"
    );
}

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

/// §109's activities, as a golden file.
///
/// The strip in the browser tests draws what Rust ships — seven activities,
/// their keys and the workspaces they open — so the stub is generated from
/// `activity::shipped` rather than typed out, for the reason every other table
/// here is: a hand-written copy is a second description, and the copy is the
/// one that goes stale.
///
/// ```text
/// DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture
/// ```
#[test]
fn the_browser_fixture_has_the_activities_djmanzo_ships() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/e2e/activities.json");
    let fresh = serde_json::to_string_pretty(&dj_app::commands::activities_dto(
        &dj_app::activity::Kept::default(),
    ))
    .expect("the activities serialise");

    if std::env::var_os("DJMANZO_BLESS").is_some() {
        std::fs::write(&path, format!("{fresh}\n")).expect("writing the activities");
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
        serde_json::from_str(&stored).expect("the stored activities are JSON");
    let fresh: serde_json::Value =
        serde_json::from_str(&fresh).expect("the fresh activities are JSON");
    assert_eq!(
        stored, fresh,
        "\nThe activities have changed, so the browser is drawing a strip \
         djmanzo no longer offers.\n\nRegenerate with:\n    \
         DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture\n"
    );
}

/// §111's stores, as a golden file: the "find it to buy" links the browser
/// tests draw are the ones Rust answers with, karaoke's first when a karaoke
/// host asks.
///
/// ```text
/// DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture
/// ```
#[test]
fn the_browser_fixture_has_the_stores_djmanzo_links_to() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/e2e/stores.json");
    let fresh = serde_json::to_string_pretty(&serde_json::json!({
        "plain": dj_app::commands::store_links(false),
        "karaoke": dj_app::commands::store_links(true),
    }))
    .expect("the stores serialise");

    if std::env::var_os("DJMANZO_BLESS").is_some() {
        std::fs::write(&path, format!("{fresh}\n")).expect("writing the stores");
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
        serde_json::from_str(&stored).expect("the stored stores are JSON");
    let fresh: serde_json::Value = serde_json::from_str(&fresh).expect("the fresh stores are JSON");
    assert_eq!(
        stored, fresh,
        "\nThe stores have changed, so the browser is offering links djmanzo \
         no longer builds.\n\nRegenerate with:\n    \
         DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture\n"
    );
}

/// §108's share channels, as a golden file: the channels the share sheet
/// offers, and the message each one would carry for one fixed night — so the
/// browser tests draw what Rust writes, cut where Rust cuts it.
///
/// ```text
/// DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture
/// ```
#[test]
fn the_browser_fixture_has_what_each_share_channel_carries() {
    use dj_app::share::{Channel, Entry, Style, message_for};
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/e2e/share.json");
    let night: Vec<Entry> = [
        ("Aventura", "Obsesión"),
        ("Juan Luis Guerra", "Bachata Rosa"),
        ("Romeo Santos", "Propuesta Indecente"),
        ("Monchy & Alexandra", "Dos Locos"),
        ("Frank Reyes", "Tu Eres Ajena"),
        ("Xtreme", "Te Extraño"),
        ("Prince Royce", "Stand By Me"),
        ("Hector Acosta", "Me Duele la Cabeza"),
        ("Zacarías Ferreira", "Si Tú Te Vas"),
        ("Raulín Rodríguez", "Nadie Es Eterno"),
        ("Antony Santos", "Voy Pa' Allá"),
        ("Joe Veras", "Intentalo Tú"),
    ]
    .iter()
    .enumerate()
    .map(|(i, (artist, title))| Entry {
        at: i as i64 * 245,
        artist: (*artist).to_owned(),
        title: (*title).to_owned(),
    })
    .collect();
    let style = Style {
        heading: "Sábado".to_owned(),
        timestamps: true,
        limit_for_url: true,
    };
    let messages: serde_json::Map<String, serde_json::Value> = Channel::ALL
        .iter()
        .map(|channel| {
            let (message, dropped) = message_for(&night, &style, *channel);
            (
                channel.slug().to_owned(),
                serde_json::json!({ "message": message, "dropped": dropped, "total": night.len() }),
            )
        })
        .collect();
    let fresh = serde_json::to_string_pretty(&serde_json::json!({
        "channels": dj_app::commands::share_channels(),
        "messages": messages,
    }))
    .expect("the share channels serialise");

    if std::env::var_os("DJMANZO_BLESS").is_some() {
        std::fs::write(&path, format!("{fresh}\n")).expect("writing the share channels");
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
        serde_json::from_str(&stored).expect("the stored share channels are JSON");
    let fresh: serde_json::Value =
        serde_json::from_str(&fresh).expect("the fresh share channels are JSON");
    assert_eq!(
        stored, fresh,
        "\nThe share channels have changed, so the browser is offering a share \
         djmanzo no longer writes.\n\nRegenerate with:\n    \
         DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture\n"
    );
}

/// §117's tree behind the leader key, as a golden file: two decks, the
/// shipped activities, workspaces and preset packs — so the browser tests walk
/// the tree Rust builds, and a key that moves in Rust moves in the tests.
///
/// ```text
/// DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture
/// ```
#[test]
fn the_browser_fixture_has_the_leader_tree() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/e2e/leader.json");
    let tree = dj_app::leader::tree(
        2,
        &dj_app::activity::all(&[]),
        &dj_app::cockpit::workspaces(),
        &dj_presets::builtin::packs(),
    );
    let fresh = serde_json::to_string_pretty(&tree).expect("the tree serialises");

    if std::env::var_os("DJMANZO_BLESS").is_some() {
        std::fs::write(&path, format!("{fresh}\n")).expect("writing the leader tree");
        return;
    }

    let stored = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "{}: {error}\n\nGenerate it with:\n    \
             DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture",
            path.display()
        )
    });
    let stored: serde_json::Value = serde_json::from_str(&stored).expect("the stored tree is JSON");
    let fresh: serde_json::Value = serde_json::from_str(&fresh).expect("the fresh tree is JSON");
    assert_eq!(
        stored, fresh,
        "\nThe leader tree has changed, so the browser is walking one djmanzo \
         no longer builds.\n\nRegenerate with:\n    \
         DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture\n"
    );
}

/// §117's dashboard, as a golden file: for no activity and for Karaoke and
/// Dig, so the browser tests draw the sections and tiles Rust builds —
/// including each activity's own section first.
///
/// ```text
/// DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture
/// ```
#[test]
fn the_browser_fixture_has_the_dashboards() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/e2e/dashboards.json");
    let activities = dj_app::activity::all(&[]);
    let boards: serde_json::Map<String, serde_json::Value> = ["", "karaoke", "dig"]
        .iter()
        .map(|slug| {
            let current = activities.iter().find(|activity| activity.slug == *slug);
            let board = dj_app::dashboard::build(
                &activities,
                current,
                &dj_app::cockpit::workspaces(),
                &dj_presets::builtin::packs(),
                &dj_app::dashboard::Interface::default(),
            );
            (
                (*slug).to_owned(),
                serde_json::to_value(board).expect("the dashboard serialises"),
            )
        })
        .collect();
    let fresh = serde_json::to_string_pretty(&boards).expect("the dashboards serialise");

    if std::env::var_os("DJMANZO_BLESS").is_some() {
        std::fs::write(&path, format!("{fresh}\n")).expect("writing the dashboards");
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
        serde_json::from_str(&stored).expect("the stored dashboards are JSON");
    let fresh: serde_json::Value =
        serde_json::from_str(&fresh).expect("the fresh dashboards are JSON");
    assert_eq!(
        stored, fresh,
        "\nThe dashboards have changed, so the browser is drawing ones djmanzo \
         no longer builds.\n\nRegenerate with:\n    \
         DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture\n"
    );
}

/// **The AI providers the settings draw**, as Rust describes them, for
/// `ui/e2e/ai.spec.ts`: OpenRouter and Google keyed and ready, the local
/// model not running, the rest waiting for a key.
///
/// ```text
/// DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture
/// ```
#[test]
fn the_browser_fixture_has_the_ai_providers() {
    use dj_assistant::{ProviderId, ProviderStatus};
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/e2e/providers.json");
    let rows: Vec<_> = ProviderId::all()
        .iter()
        .map(|&id| {
            let (status, hint) = match id {
                ProviderId::OpenRouter | ProviderId::Google => {
                    (ProviderStatus::Ready, Some("…4f2a".to_owned()))
                }
                ProviderId::Local => (
                    ProviderStatus::NotRunning {
                        hint: "Ollama is not running",
                    },
                    None,
                ),
                _ => (ProviderStatus::NeedsKey { secret: "key" }, None),
            };
            dj_app::assistant::provider_row(id, status, hint)
        })
        .collect();
    let fresh = serde_json::to_string_pretty(&rows).expect("the providers serialise");

    if std::env::var_os("DJMANZO_BLESS").is_some() {
        std::fs::write(&path, format!("{fresh}\n")).expect("writing the providers");
        return;
    }
    let stored = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "{}: {error}\n\nGenerate it with:\n    \
             DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture",
            path.display()
        )
    });
    // Compared as JSON, as the other fixtures are: Windows checks the file
    // out with CRLF line endings, and a byte comparison failed there on a
    // file whose content was right.
    let stored: serde_json::Value =
        serde_json::from_str(&stored).expect("the stored providers are JSON");
    let fresh: serde_json::Value =
        serde_json::from_str(&fresh).expect("the fresh providers are JSON");
    assert_eq!(
        stored, fresh,
        "ui/e2e/providers.json is stale; regenerate it with \
         DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture"
    );
}

/// §118: the event panel's answers, as Rust gives them.
///
/// The browser cannot run the rules that decide what an event still lacks
/// or which ideas it is offered, so it is handed Rust's own answers for a
/// few events: the options the pickers offer, a wedding half prepared, the
/// same wedding after its first idea is taken, and a fresh event with
/// nothing in it yet. A test that drew made-up steps would pass against a
/// panel whose steps Rust never sends.
#[test]
fn the_browser_fixture_has_the_event_panels_answers() {
    use dj_app::gig::{self, Gig, Moment, Sky};
    use dj_app::setting::Setting;

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/e2e/events.json");
    let wedding = Gig {
        id: "anna-and-ben-2026-10-03".to_owned(),
        title: "Anna and Ben".to_owned(),
        date: "2026-10-03".to_owned(),
        starts: "21:00".to_owned(),
        minutes: 240,
        setting: Some(Setting::Wedding),
        place: "The old mill".to_owned(),
        sky: Sky::Both,
        crowd: "Family and friends, three generations".to_owned(),
        topic: "Songs they met to".to_owned(),
        genres: vec!["disco".to_owned()],
        wishes: vec!["September".to_owned()],
        moments: vec![Moment {
            at: "21:30".to_owned(),
            what: "First dance".to_owned(),
            record: "At Last".to_owned(),
        }],
        techniques: vec!["echo out".to_owned(), "cut".to_owned()],
        rehearsed: vec!["cut".to_owned()],
        updated: 1_790_000_000,
        ..Gig::default()
    };
    let wedding = gig::check(wedding).expect("the wedding is an event djmanzo keeps");
    let first = gig::ideas(&wedding)
        .into_iter()
        .next()
        .expect("a half-prepared wedding is offered ideas");
    let taken = gig::take(wedding.clone(), &first.adds);
    let fresh = Gig {
        id: gig::new_id("Summer party", "2026-07-04", &[]),
        title: "Summer party".to_owned(),
        date: "2026-07-04".to_owned(),
        ..Gig::default()
    };
    // Eighteen minutes in, with one plan written: the quick decision has to
    // tell a plan the DJ wrote from a trouble with only the usual answer.
    let mut playing = wedding.clone();
    playing.fallbacks.push(gig::Fallback {
        trouble: gig::Trouble::Power,
        plan: "Phone into the house desk; start again with September.".to_owned(),
    });
    let tonight = gig::tonight(&playing, "2026-10-03", 21 * 60 + 18);
    let fixture = serde_json::json!({
        "tonight": tonight,
        "options": gig::options(),
        "list": [gig::summary(&wedding), gig::summary(&fresh)],
        "wedding": gig::view(wedding),
        "taken": { "adds": first.adds, "view": gig::view(taken) },
        "fresh": gig::view(fresh),
    });
    let fresh_text = serde_json::to_string_pretty(&fixture).expect("the event answers serialise");

    if std::env::var_os("DJMANZO_BLESS").is_some() {
        std::fs::write(&path, format!("{fresh_text}\n")).expect("writing the event answers");
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
        serde_json::from_str(&stored).expect("the stored event answers are JSON");
    assert_eq!(
        stored, fixture,
        "ui/e2e/events.json is stale; regenerate it with \
         DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture"
    );
}

/// §118b: the welcome's plan, as Rust makes it.
///
/// What setting up will do is `welcome::plan`'s to say, sentence by
/// sentence; the browser is handed Rust's own plan for one DJ -- weddings
/// first, then Latin nights, a couple of moves, a level -- and the fresh
/// answers a first run starts from -- and what Rust says to one answer it
/// cannot use, a tempo range the wrong way round.
#[test]
fn the_browser_fixture_has_the_welcomes_plan() {
    use dj_app::setting::Setting;
    use dj_app::welcome::{self, Answers};

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/e2e/welcome.json");
    let answers = welcome::check(Answers {
        name: "Johannes".to_owned(),
        nights: vec![Setting::Wedding, Setting::Latin],
        genres: vec!["disco".to_owned()],
        bpm_low: 100,
        bpm_high: 128,
        favourites: vec!["Romeo Santos".to_owned()],
        moves: vec!["cut".to_owned()],
        learn: vec!["echo out".to_owned()],
        level: "suggest".to_owned(),
        theme: String::new(),
        done: false,
    })
    .expect("the answers are usable");
    let plan = welcome::plan(&answers);
    let wedding = dj_app::setup::setup(Setting::Wedding);
    let backwards = welcome::check(Answers {
        bpm_low: 140,
        bpm_high: 120,
        ..Answers::default()
    })
    .expect_err("a tempo range the wrong way round is refused");
    let fixture = serde_json::json!({
        "fresh": Answers::default(),
        "answers": answers,
        "plan": plan,
        "applied": { "workspace": wedding.workspace, "theme": wedding.theme },
        "refused": { "bpm_low": 140, "bpm_high": 120, "message": backwards.to_string() },
    });
    let text = serde_json::to_string_pretty(&fixture).expect("the welcome serialises");

    if std::env::var_os("DJMANZO_BLESS").is_some() {
        std::fs::write(&path, format!("{text}\n")).expect("writing the welcome");
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
        serde_json::from_str(&stored).expect("the stored welcome is JSON");
    assert_eq!(
        stored, fixture,
        "ui/e2e/welcome.json is stale; regenerate it with \
         DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture"
    );
}

/// §118a: a decision's own words, as Rust says them.
///
/// Which records a decision offers is the rail's ranking, tested in Rust;
/// the browser composes one from the rail's fixture with these -- each
/// direction's sentence, the stall for deck 1, and the seconds at which a
/// decision grows -- so the words it checks are Rust's and not a copy.
#[test]
fn the_browser_fixture_has_the_decisions_words() {
    use dj_app::decide::{self, Direction};

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/e2e/decide.json");
    let fixture = serde_json::json!({
        "directions": Direction::ALL
            .iter()
            .map(|d| serde_json::json!({ "direction": d, "says": d.says() }))
            .collect::<Vec<_>>(),
        "stall": decide::stall(1),
        "card_below": decide::CARD_BELOW,
        "whole_below": decide::WHOLE_BELOW,
    });
    let text = serde_json::to_string_pretty(&fixture).expect("the decision serialises");

    if std::env::var_os("DJMANZO_BLESS").is_some() {
        std::fs::write(&path, format!("{text}\n")).expect("writing the decision");
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
        serde_json::from_str(&stored).expect("the stored decision is JSON");
    assert_eq!(
        stored, fixture,
        "ui/e2e/decide.json is stale; regenerate it with \
         DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture"
    );
}

/// §118a: the guides, as Rust offers them in four booths.
///
/// Which guide can be opened, on which decks, and why the others cannot is
/// `dj_app::guide`'s; the browser is handed Rust's answer for a quiet booth,
/// one deck playing with the other empty, a record waiting on the other
/// deck, and the same with a prepared night being played.
#[test]
fn the_browser_fixture_has_the_guides() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/e2e/guides.json");
    let state = dj_app::AppState::new(true);
    let mut booth = dj_app::Snapshot::capture(&state.registry(), 2);
    for deck in &mut booth.decks {
        deck.loaded = false;
        deck.playing = false;
        deck.volume = 1.0;
    }
    booth.master.crossfader = 0.0;
    let quiet = dj_app::guide::guides(&booth, false, 2);
    booth.decks[0].loaded = true;
    booth.decks[0].playing = true;
    let playing = dj_app::guide::guides(&booth, false, 2);
    booth.decks[1].loaded = true;
    let waiting = dj_app::guide::guides(&booth, false, 2);
    let live = dj_app::guide::guides(&booth, true, 2);
    let fixture = serde_json::json!({
        "quiet": quiet,
        "playing": playing,
        "waiting": waiting,
        "live": live,
    });
    let text = serde_json::to_string_pretty(&fixture).expect("the guides serialise");

    if std::env::var_os("DJMANZO_BLESS").is_some() {
        std::fs::write(&path, format!("{text}\n")).expect("writing the guides");
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
        serde_json::from_str(&stored).expect("the stored guides are JSON");
    assert_eq!(
        stored, fixture,
        "ui/e2e/guides.json is stale; regenerate it with \
         DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture"
    );
}

/// §118d: the press kit, as Rust answers it.
///
/// A kit begun from the welcome, a whole one with a night booked, its
/// answer to a wedding enquiry, and what Rust says to a phone number that is
/// not one -- so the words the browser checks are Rust's.
#[test]
fn the_browser_fixture_has_the_press_kit() {
    use dj_app::commands::KitView;
    use dj_app::kit::{self, Booked, Fee, Kit, Occasion};
    use dj_app::setting::Setting;

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/e2e/kit.json");
    let view = |kit: Kit, kept: bool, booked: Vec<Booked>| {
        let occasions = Occasion::ALL
            .into_iter()
            .map(|o| kit::compose(&kit, o, None, &booked))
            .collect();
        let qr = (!kit.name.is_empty() && !(kit.email.is_empty() && kit.phone.is_empty()))
            .then(|| dj_net::sticker::qr_svg(&kit::vcard(&kit)).ok())
            .flatten();
        let sites = kit.links.iter().map(|l| kit::site(l).to_owned()).collect();
        KitView {
            kit,
            kept,
            booked,
            occasions,
            qr,
            sites,
        }
    };
    let welcomed = dj_app::welcome::Answers {
        name: "DJ Rosa".to_owned(),
        genres: vec!["disco".to_owned()],
        ..dj_app::welcome::Answers::default()
    };
    let fresh = view(kit::begun(&welcomed), false, Vec::new());
    let rosa = kit::check(Kit {
        name: "DJ Rosa".to_owned(),
        tagline: "Latin and disco for rooms that dance".to_owned(),
        bio: "Ten years of weddings and beach bars.".to_owned(),
        based: "Vienna".to_owned(),
        genres: vec!["disco".to_owned(), "salsa".to_owned()],
        experience: vec!["Resident at Sun Bar since 2019".to_owned()],
        email: "rosa@example.com".to_owned(),
        phone: "+43 660 123 4567".to_owned(),
        booking: "A deposit of 30% holds the date.".to_owned(),
        fees: vec![
            Fee {
                night: Some(Setting::Wedding),
                what: "Up to five hours, sound included".to_owned(),
                price: "€900".to_owned(),
            },
            Fee {
                night: None,
                what: "A club night".to_owned(),
                price: "€400".to_owned(),
            },
        ],
        links: vec![
            "https://soundcloud.com/djrosa".to_owned(),
            "https://www.instagram.com/djrosa/".to_owned(),
        ],
        photos: Vec::new(),
        documents: Vec::new(),
    })
    .expect("the kit is usable");
    let booked = vec![Booked {
        date: "2026-10-03".to_owned(),
        title: "Anna and Ben".to_owned(),
        place: "Schloss Hof".to_owned(),
        starts: "18:00".to_owned(),
    }];
    let wedding = kit::compose(&rosa, Occasion::Enquiry, Some(Setting::Wedding), &booked);
    let full = view(rosa.clone(), true, booked);
    let bad = Kit {
        phone: "call me".to_owned(),
        ..rosa
    };
    let refused = kit::check(bad).expect_err("not a number").to_string();
    let fixture = serde_json::json!({
        "fresh": fresh,
        "full": full,
        "wedding": wedding,
        "refused": { "phone": "call me", "message": refused },
    });
    let text = serde_json::to_string_pretty(&fixture).expect("the kit serialises");

    if std::env::var_os("DJMANZO_BLESS").is_some() {
        std::fs::write(&path, format!("{text}\n")).expect("writing the kit");
        return;
    }
    let stored = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "{}: {error}\n\nGenerate it with:\n    \
             DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture",
            path.display()
        )
    });
    let stored: serde_json::Value = serde_json::from_str(&stored).expect("the stored kit is JSON");
    assert_eq!(
        stored, fixture,
        "ui/e2e/kit.json is stale; regenerate it with \
         DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture"
    );
}

/// §119: a night's reactions, as Rust reads, places and answers them.
///
/// Two records -- the first with a drop, a breakdown and a voice entry --
/// and what was said over them: a burst at the drop, a question about the
/// record, a cold word, a request from the room, a hello. The browser is
/// handed Rust's reading of every one, its summary and the wedding's usual
/// goals answered, so what the panel draws is Rust's and not the panel's.
#[test]
fn the_browser_fixture_has_a_nights_crowd() {
    use dj_app::commands::{CrowdMeasure, CrowdView};
    use dj_app::crowd::{self, Measure, Played, Reaction, Source};
    use dj_app::setting::Setting;

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/e2e/crowd.json");
    let start = 1_790_000_000;
    let played = vec![
        Played {
            at: start,
            track_id: "a".repeat(64),
            title: "Ojalá Que Llueva Café".to_owned(),
            artist: "Juan Luis Guerra".to_owned(),
            drops: vec![60.0],
            breakdowns: vec![(40.0, 60.0)],
            vocal: Some(20.0),
        },
        Played {
            at: start + 200,
            track_id: "b".repeat(64),
            title: "Burbujas de Amor".to_owned(),
            artist: "Juan Luis Guerra".to_owned(),
            drops: Vec::new(),
            breakdowns: Vec::new(),
            vocal: None,
        },
    ];
    let said = |at: i64, source: Source, who: &str, text: &str| Reaction {
        at: start + at,
        source,
        who: who.to_owned(),
        text: text.to_owned(),
    };
    let reactions = vec![
        said(10, Source::YouTube, "Ana", "hello from Porto"),
        said(70, Source::YouTube, "Ben", "THIS DROP 🔥"),
        said(71, Source::YouTube, "Cleo", "🔥🔥🔥"),
        said(73, Source::TikTok, "Dan", "insane"),
        said(80, Source::YouTube, "Eve", "what song is this??"),
        said(150, Source::YouTube, "Finn", "boring, skip"),
        said(260, Source::Room, "", "play Bachata Rosa"),
    ];
    let delay = 8;
    let placed = crowd::place(&reactions, &played, delay);
    let summary = crowd::summary(&placed, &played);
    let goals = crowd::answer(&crowd::goals_for(Some(Setting::Wedding)), &summary);
    let view = CrowdView {
        // In the form `AppState` names a run's session: when it began.
        session: format!("session-{start}"),
        current: true,
        sessions: vec![
            format!("session-{start}"),
            format!("session-{}", start - 86_400),
        ],
        delay,
        played,
        placed,
        summary,
        goals,
        measures: Measure::ALL
            .into_iter()
            .map(|measure| CrowdMeasure {
                measure,
                title: measure.title(),
            })
            .collect(),
    };
    let fixture = serde_json::json!({ "view": view });
    let text = serde_json::to_string_pretty(&fixture).expect("the crowd serialises");

    if std::env::var_os("DJMANZO_BLESS").is_some() {
        std::fs::write(&path, format!("{text}\n")).expect("writing the crowd");
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
        serde_json::from_str(&stored).expect("the stored crowd is JSON");
    assert_eq!(
        stored, fixture,
        "ui/e2e/crowd.json is stale; regenerate it with \
         DJMANZO_BLESS=1 cargo test -p dj-app --test e2e_fixture"
    );
}
