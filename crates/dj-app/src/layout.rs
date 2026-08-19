//! Layout presets, and the skin system they are the built-in cases of.
//!
//! # One mechanism, not two
//!
//! A layout preset and a skin are the same thing: a description of which parts
//! of the interface are on screen and how densely. Building presets as special
//! cases in the interface and skins as a file format on top would mean two ways
//! of saying the same thing, differing eventually in some detail nobody meant.
//! So the four presets are ordinary layouts that happen to ship with the
//! application, and a DJ's own layout is loaded from JSON by the same code.
//!
//! # A layout is data
//!
//! It says what to show and how big; it does not say what anything *does*. No
//! layout can execute code, reach a file, or change what an action means —
//! [ADR-0003](../../../docs/adr/0003-single-action-vocabulary.md) puts every
//! behaviour behind the action vocabulary, and a theme is not a behaviour.
//! That is what makes it safe to load one somebody sent you.

use serde::{Deserialize, Serialize};

/// What a layout shows and how densely.
///
/// Every field has a default, so a layout file can name only what it changes.
/// A DJ hiding the FX rack should not have to restate the other nine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Layout {
    /// Shown in the picker.
    pub name: String,
    /// One line about who it is for.
    pub description: String,
    /// Decks on screen. Two or four; the engine always runs four.
    pub decks: u8,
    /// Height of the scrolling waveform lane, in pixels.
    pub waveform_height: u16,
    /// Whether the whole-track overview sits under the lane.
    pub overview: bool,
    pub pads: bool,
    pub loops: bool,
    /// Whether the three effect slots are on screen, per deck and on the
    /// master. The flag the doc comment above has always named.
    pub fx: bool,
    pub beat_jump: bool,
    pub eq: bool,
    pub filter: bool,
    pub keylock: bool,
    /// Whether the browser is open when the application starts.
    pub browser: bool,
    /// Overall scale, 0.8..=1.4. Multiplies the root font size, which every
    /// other measurement is in `em` of.
    pub density: f32,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            name: "Custom".to_owned(),
            description: String::new(),
            decks: 2,
            waveform_height: 96,
            overview: true,
            pads: true,
            loops: true,
            fx: true,
            beat_jump: true,
            eq: true,
            filter: true,
            keylock: true,
            browser: false,
            density: 1.0,
        }
    }
}

impl Layout {
    /// Bring a layout into the ranges the interface can actually draw.
    ///
    /// Clamped rather than refused, unlike most of this codebase: a layout is
    /// a preference, and a DJ whose file says `density: 40` wants big text, not
    /// an error message. The one thing that cannot be interpreted generously is
    /// a deck count the engine does not run.
    #[must_use]
    pub fn sane(mut self) -> Self {
        self.decks = if self.decks >= 4 { 4 } else { 2 };
        self.waveform_height = self.waveform_height.clamp(48, 320);
        self.density = if self.density.is_finite() {
            self.density.clamp(0.8, 1.4)
        } else {
            1.0
        };
        if self.name.trim().is_empty() {
            self.name = "Custom".to_owned();
        }
        self
    }
}

/// The four that ship.
///
/// They trade complexity for screen space, which is the axis a DJ actually
/// moves along: a beginner wants fewer things and bigger waveforms, and
/// somebody playing off a controller wants the screen to stay out of the way.
#[must_use]
pub fn builtin() -> Vec<Layout> {
    vec![
        Layout {
            name: "Starter".to_owned(),
            description: "Two decks and big waveforms. Everything you need and nothing else."
                .to_owned(),
            decks: 2,
            waveform_height: 160,
            pads: false,
            loops: false,
            fx: false,
            beat_jump: false,
            filter: false,
            keylock: false,
            density: 1.1,
            ..Layout::default()
        },
        Layout {
            name: "Essentials".to_owned(),
            description: "Two decks with cues, loops and the EQ.".to_owned(),
            decks: 2,
            waveform_height: 120,
            // Its own description says cues, loops and the EQ. A rack it does
            // not mention would be a preset that does not do what it says.
            fx: false,
            ..Layout::default()
        },
        Layout {
            name: "Pro".to_owned(),
            description: "Four decks, everything on screen, browser open.".to_owned(),
            decks: 4,
            waveform_height: 96,
            browser: true,
            ..Layout::default()
        },
        Layout {
            name: "Performance".to_owned(),
            description: "Maximum control density for a controller-driven set.".to_owned(),
            decks: 4,
            waveform_height: 72,
            density: 0.85,
            ..Layout::default()
        },
    ]
}

/// Read a DJ's own layouts from a directory of JSON files.
///
/// A malformed file is logged and skipped rather than failing the load: one
/// file somebody was editing should not cost them the other nine, least of all
/// at the start of a set.
#[must_use]
pub fn load_dir(dir: &std::path::Path) -> Vec<Layout> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        match std::fs::read_to_string(&path).map(|text| serde_json::from_str::<Layout>(&text)) {
            Ok(Ok(layout)) => out.push(layout.sane()),
            Ok(Err(error)) => tracing::warn!(?path, %error, "skipping a malformed layout"),
            Err(error) => tracing::warn!(?path, %error, "could not read a layout"),
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_four_presets_ship_and_are_all_drawable() {
        let presets = builtin();
        let names: Vec<&str> = presets.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names, vec!["Starter", "Essentials", "Pro", "Performance"]);

        for preset in presets {
            let sane = preset.clone().sane();
            assert_eq!(sane, preset, "{} is not already sane", preset.name);
        }
    }

    /// They have to differ along the axis they exist for, or they are four
    /// names for one layout.
    #[test]
    fn the_presets_trade_complexity_for_space() {
        let presets = builtin();
        let starter = &presets[0];
        let performance = &presets[3];

        assert!(
            starter.waveform_height > performance.waveform_height,
            "a beginner wants bigger waveforms than a controller player"
        );
        assert!(starter.density > performance.density);
        assert!(!starter.pads && performance.pads);
        assert!(starter.decks < performance.decks);
    }

    /// A layout file should be able to name only what it changes.
    #[test]
    fn a_layout_file_may_name_only_what_it_changes() {
        let layout: Layout = serde_json::from_str(r#"{"name": "Mine", "decks": 4}"#).unwrap();
        assert_eq!(layout.name, "Mine");
        assert_eq!(layout.decks, 4);
        assert_eq!(
            layout.waveform_height,
            Layout::default().waveform_height,
            "everything unmentioned keeps its default"
        );
        assert!(layout.pads);
    }

    #[test]
    fn an_empty_layout_file_is_the_default_layout() {
        let layout: Layout = serde_json::from_str("{}").unwrap();
        assert_eq!(layout, Layout::default());
    }

    /// A preference is interpreted generously; a deck count the engine does not
    /// run is not a preference.
    #[test]
    fn absurd_values_are_clamped_rather_than_refused() {
        let layout = Layout {
            decks: 7,
            waveform_height: 9000,
            density: 40.0,
            name: "   ".to_owned(),
            ..Layout::default()
        }
        .sane();

        assert_eq!(layout.decks, 4);
        assert_eq!(layout.waveform_height, 320);
        assert!((layout.density - 1.4).abs() < 1e-6);
        assert_eq!(layout.name, "Custom", "a nameless layout is unpickable");
    }

    #[test]
    fn a_deck_count_between_two_and_four_rounds_down_to_two() {
        assert_eq!(
            Layout {
                decks: 3,
                ..Layout::default()
            }
            .sane()
            .decks,
            2
        );
        assert_eq!(
            Layout {
                decks: 0,
                ..Layout::default()
            }
            .sane()
            .decks,
            2
        );
    }

    #[test]
    fn a_non_finite_density_becomes_one() {
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let layout = Layout {
                density: bad,
                ..Layout::default()
            }
            .sane();
            assert!((layout.density - 1.0).abs() < 1e-6, "{bad}");
        }
    }

    #[test]
    fn a_layout_survives_the_round_trip_through_json() {
        for preset in builtin() {
            let text = serde_json::to_string(&preset).unwrap();
            let back: Layout = serde_json::from_str(&text).unwrap();
            assert_eq!(back, preset);
        }
    }

    // -- loading -----------------------------------------------------------

    #[test]
    fn a_directory_of_layouts_is_read_in_name_order() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("b.json"), r#"{"name": "Booth"}"#).unwrap();
        std::fs::write(dir.path().join("a.json"), r#"{"name": "Attic"}"#).unwrap();
        std::fs::write(dir.path().join("notes.txt"), "not a layout").unwrap();

        let found = load_dir(dir.path());
        assert_eq!(
            found.iter().map(|l| l.name.as_str()).collect::<Vec<_>>(),
            vec!["Attic", "Booth"]
        );
    }

    /// One file somebody was editing must not cost them the others.
    #[test]
    fn a_malformed_layout_is_skipped_not_fatal() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("good.json"), r#"{"name": "Good"}"#).unwrap();
        std::fs::write(dir.path().join("broken.json"), "{not json").unwrap();

        let found = load_dir(dir.path());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "Good");
    }

    #[test]
    fn a_missing_directory_is_no_layouts_rather_than_an_error() {
        assert!(load_dir(std::path::Path::new("/nowhere/layouts")).is_empty());
    }

    /// A layout from a file is put through the same clamping as any other.
    #[test]
    fn a_loaded_layout_is_made_sane() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("wild.json"),
            r#"{"name": "Wild", "decks": 9, "density": 100}"#,
        )
        .unwrap();

        let found = load_dir(dir.path());
        assert_eq!(found[0].decks, 4);
        assert!((found[0].density - 1.4).abs() < 1e-6);
    }
}
