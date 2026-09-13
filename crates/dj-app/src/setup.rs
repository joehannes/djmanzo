//! §54's functional presets: one gesture that configures many systems.
//!
//! > Create presets that configure multiple systems simultaneously. A preset
//! > should be capable of defining: workspace, surfaces, density, theme,
//! > waveform layers, pad pages, assistant posture, assistant suggestion rate,
//! > technique packs, genre packs, session-phase weights, audience
//! > integration, automation limits.
//!
//! djmanzo already had presets that configured *one* system. §7's workspaces
//! set the cockpit — which surfaces, how dense, how many decks, which theme —
//! and stop at the edge of the screen. Everything else §54 lists is a control
//! somewhere else: the posture is the assistant's, the layers are §25's, the
//! pad pages are the decks'. A DJ setting up for a wedding had to find all of
//! them.
//!
//! # One per kind of night, and the kinds are §81's
//!
//! §54's own example is *"Beach Sunset / Latin Resort"*, which is not a layout —
//! it is an **occasion**. [`crate::setting::Setting`] is already §81's list of
//! those, told rather than guessed, and already read by the genre learning and
//! by §31's theme. Inventing a second list of occasions for §54 would be two
//! names for one thing, and the two would disagree the first time either grew.
//!
//! # It says what it changed, and it changes nothing quietly
//!
//! [`Setup::changes`] is the whole list in the DJ's own words, and it is what
//! the interface shows *before* anything is applied as well as after. The rule
//! is `crate::presets`' own: **a preset that silently changes eight things is
//! the kind of feature people stop trusting** — and §54's presets change more
//! than eight.
//!
//! # What §54 asks for and this cannot set
//!
//! Named here rather than quietly dropped, which is the same posture
//! [`crate::remembered`] takes towards the row djmanzo did not keep:
//!
//! - **Technique packs** and **genre packs** are §16 and §32, and neither
//!   ships as a *format*: djmanzo has genre families and theme packages, and
//!   nothing a preset could name.
//! - **Session-phase weights** would be a preset telling the context engine
//!   what to read, which is the wrong direction. §11 derives the phase from
//!   the music, and a wedding preset that pre-weighted it towards *warm-up*
//!   would be djmanzo deciding the night before hearing it.
//! - **Surfaces** are not separate from the workspace here, because in djmanzo
//!   a workspace *is* its surfaces — §7's `Workspace::surfaces` — so naming the
//!   workspace names them.

use crate::setting::Setting;
use dj_assistant::Posture;

/// One of §54's functional presets.
///
/// A table rather than a constructor, so what a night is set up as can be read
/// at a glance and a test can hold every field to the system that owns it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Setup {
    /// The kind of night. §81's own six, so §54 and §81 are one list.
    pub setting: Setting,
    /// The cockpit arrangement, by the name `cockpit::workspaces()` gives it.
    ///
    /// A name rather than a copy, for the reason `AppState::chosen_layout`
    /// gives about layouts: a DJ who has edited *Club* wants their edit, and a
    /// stored copy would hand them the version from whenever this table was
    /// written.
    pub workspace: &'static str,
    /// The theme package, by the id `ui/src/controls/themes/packages.ts` gives
    /// it. Empty means "leave the theme alone", which is a real answer: not
    /// every occasion has a look, and changing one a DJ chose would be §31's
    /// own complaint about an interface that overrides a deliberate choice.
    pub theme: &'static str,
    /// §25's layers this kind of night wants on the waveform.
    pub layers: &'static [&'static str],
    /// The pad pages a DJ reaches for on this kind of night, in order.
    pub pages: &'static [&'static str],
    /// How much the assistant does.
    pub posture: Posture,
    /// Whether the room's requests are part of this night.
    ///
    /// §54's *audience integration*. A wedding runs on requests and a club
    /// mostly does not, and the difference is not a preference — it is what
    /// the evening is.
    pub requests: bool,
}

/// §54's presets, one per kind of night.
///
/// Starting points rather than identities: every one of them is a set of
/// controls a DJ can change afterwards, and §7 says so explicitly about the
/// workspaces these name.
pub const ALL: [Setup; 6] = [
    Setup {
        setting: Setting::Club,
        workspace: "Club",
        theme: "pkg-industrial",
        // Everything a hand needs mid-mix and nothing that is paperwork: the
        // grid and the phrases to mix on, the marks, the loop, and where the
        // record ends. No mix-out window or ghost — at peak a DJ is deciding
        // those, not reading djmanzo's opinion of them.
        layers: &[
            "beats",
            "downbeats",
            "phrases",
            "cues",
            "loop",
            "runway",
            "confidence",
        ],
        pages: &["cues", "loops", "fx"],
        posture: Posture::Prepare,
        requests: false,
    },
    Setup {
        setting: Setting::Beach,
        workspace: "Open Format",
        theme: "pkg-sunset",
        // Long blends, so the mix-out window and the ghost earn their room;
        // the loop and the roll do not get used much at this tempo.
        layers: &[
            "beats",
            "downbeats",
            "phrases",
            "cues",
            "seam",
            "mix-out",
            "suggestion",
            "runway",
            "confidence",
        ],
        pages: &["cues", "saved"],
        posture: Posture::Assist,
        requests: true,
    },
    Setup {
        setting: Setting::Wedding,
        workspace: "Wedding / Event",
        theme: "pkg-organic",
        // A wedding is run from the browser and the microphone. What the
        // waveform is for here is knowing how long is left and where a record
        // can be left, not counting bars.
        layers: &[
            "beats",
            "downbeats",
            "cues",
            "loop",
            "seam",
            "mix-out",
            "runway",
        ],
        pages: &["cues", "saved", "sampler"],
        posture: Posture::Assist,
        requests: true,
    },
    Setup {
        setting: Setting::Latin,
        workspace: "Classic DJ",
        theme: "pkg-organic",
        // The phrase structure is the whole of it: Latin records turn over on
        // the phrase and a mix that lands off one is audibly wrong.
        layers: &[
            "beats",
            "downbeats",
            "phrases",
            "cues",
            "loop",
            "seam",
            "runway",
            "confidence",
        ],
        pages: &["cues", "loops", "roll"],
        posture: Posture::Suggest,
        requests: true,
    },
    Setup {
        setting: Setting::Practice,
        workspace: "Pro Performance",
        theme: "pkg-studio",
        // Everything djmanzo can draw. Practice is the one night where the
        // instrumentation is the point and there is nobody to play to.
        layers: &[
            "beats",
            "downbeats",
            "phrases",
            "cues",
            "loop",
            "seam",
            "mix-out",
            "suggestion",
            "confidence",
            "runway",
        ],
        pages: &["cues", "loops", "roll", "slicer"],
        // Watch, not Off: §69's practice room is about reviewing afterwards,
        // and advice during is the distraction `Posture::Watch` exists for.
        posture: Posture::Watch,
        requests: false,
    },
    Setup {
        setting: Setting::OpenFormat,
        workspace: "Open Format",
        // Nothing. Open format is the night with no look of its own, and §31's
        // theme adaptation is better placed to answer the room than a preset
        // written in advance.
        theme: "",
        layers: &[
            "beats",
            "downbeats",
            "phrases",
            "cues",
            "loop",
            "seam",
            "mix-out",
            "runway",
        ],
        pages: &["cues", "loops", "saved"],
        posture: Posture::Suggest,
        requests: true,
    },
];

/// The setup for a kind of night.
#[must_use]
pub fn setup(setting: Setting) -> Setup {
    ALL.into_iter()
        .find(|s| s.setting == setting)
        // Not reachable: a test holds the table to `Setting::ALL`. The fallback
        // is open format rather than a panic, because a missing row should cost
        // a DJ the preset and not the application.
        .unwrap_or(ALL[5])
}

impl Setup {
    /// What applying this would change, in the DJ's own words.
    ///
    /// Shown *before* it is applied as well as after. §54's presets touch six
    /// systems at once, and the rule `crate::presets` states about the small
    /// ones applies with more force here: a preset that silently changes eight
    /// things is the kind of feature people stop trusting.
    #[must_use]
    pub fn changes(&self) -> Vec<String> {
        let mut said = vec![
            format!("Arrangement: {}", self.workspace),
            format!("Waveform: {} of §25's layers", self.layers.len()),
            format!("Pad pages: {}", self.pages.join(", ")),
            format!("Assistant: {}", self.posture.name()),
            format!("Requests: {}", if self.requests { "on" } else { "off" }),
        ];
        // The theme only when there is one, because "leave it alone" is an
        // answer and a line saying so would be a change that did not happen.
        if !self.theme.is_empty() {
            said.insert(1, format!("Theme: {}", self.theme));
        }
        said
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **§54's presets and §81's occasions are one list.**
    ///
    /// Two lists of what a night can be would disagree the first time either
    /// grew, and the one that went stale would be the one a DJ picked from.
    #[test]
    fn there_is_a_setup_for_every_kind_of_night_and_no_others() {
        assert_eq!(ALL.len(), Setting::ALL.len());
        for setting in Setting::ALL {
            assert_eq!(setup(setting).setting, setting, "{}", setting.slug());
        }
    }

    /// **The load-bearing one: every preset names things djmanzo actually has.**
    ///
    /// A preset is the one gesture that touches six systems at once, so a field
    /// naming something that does not exist fails in the worst possible place:
    /// a DJ picks *Wedding*, five of the six take, and the sixth silently does
    /// not — with nothing on screen saying which. Every name is checked against
    /// the table that owns it rather than against a copy.
    #[test]
    fn every_setup_names_an_arrangement_a_layer_and_a_page_that_exist() {
        let arrangements: Vec<String> = crate::cockpit::workspaces()
            .into_iter()
            .map(|w| w.name)
            .collect();

        for preset in ALL {
            assert!(
                arrangements.iter().any(|name| name == preset.workspace),
                "{} names the arrangement `{}`, which djmanzo does not ship",
                preset.setting.slug(),
                preset.workspace
            );
            assert!(
                !preset.layers.is_empty(),
                "{} would leave the waveform with nothing on it",
                preset.setting.slug()
            );
            for layer in preset.layers {
                let known = dj_render::layer(layer).unwrap_or_else(|| {
                    panic!("{} names no layer `{layer}`", preset.setting.slug())
                });
                assert!(
                    known.exists(),
                    "{} asks for `{layer}`, which nothing draws yet",
                    preset.setting.slug()
                );
            }
            assert!(
                !preset.pages.is_empty(),
                "{} would leave the pads with no page",
                preset.setting.slug()
            );
            for page in preset.pages {
                assert!(
                    dj_core::PadPage::parse(page).is_some(),
                    "{} names no pad page `{page}`",
                    preset.setting.slug()
                );
            }
        }
    }

    /// **Every theme a preset names is one the interface ships.**
    ///
    /// The house pattern, and the one field of this table that crosses the
    /// language boundary: the theme packages are the interface's, by id, and
    /// nothing else in djmanzo would notice a typo. A preset naming
    /// `pkg-sunsets` would apply five systems out of six and leave the sixth
    /// silently on whatever the DJ already had.
    #[test]
    fn every_theme_a_setup_names_is_one_the_interface_ships() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../ui/src/controls/themes/packages.ts"
        );
        // Line endings normalised: CI checks the repository out with CRLF on
        // Windows, and a scan of source text that does not allow for it fails
        // there and only there.
        let source = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("could not read the theme packages at {path}: {e}"))
            .replace("\r\n", "\n");

        for preset in ALL {
            if preset.theme.is_empty() {
                continue;
            }
            assert!(
                source.contains(&format!("id: \"{}\"", preset.theme)),
                "{} names the theme `{}`, which the interface does not ship",
                preset.setting.slug(),
                preset.theme
            );
        }
    }

    /// **A preset says everything it will do, and nothing it will not.**
    ///
    /// The line a DJ reads before pressing it. A preset that leaves the theme
    /// alone must not claim a theme line, because a change that did not happen
    /// is the one that makes the rest of the list untrustworthy.
    #[test]
    fn the_list_of_changes_is_the_changes() {
        for preset in ALL {
            let said = preset.changes();
            let themed = said.iter().any(|line| line.starts_with("Theme:"));
            assert_eq!(
                themed,
                !preset.theme.is_empty(),
                "{} is inconsistent about whether it changes the theme",
                preset.setting.slug()
            );
            assert!(
                said.iter().any(|line| line.contains(preset.workspace)),
                "{} does not say which arrangement it opens",
                preset.setting.slug()
            );
            assert_eq!(
                said.len(),
                if preset.theme.is_empty() { 5 } else { 6 },
                "{} says {said:?}",
                preset.setting.slug()
            );
        }
    }

    /// **Practice does not run the assistant, and the club does not run the
    /// autopilot.**
    ///
    /// The two ends of §10's posture axis, which §54 lets a preset set — and
    /// the two a preset must not get wrong. Nothing ships on autopilot: §72's
    /// matrix is the thing that grants that, and a preset granting it in one
    /// press would be the opposite of the warrant §9 asks for.
    #[test]
    fn no_preset_hands_over_the_night() {
        for preset in ALL {
            assert_ne!(
                preset.posture,
                Posture::Autopilot,
                "{} would put djmanzo in charge of a night in one press",
                preset.setting.slug()
            );
        }
        assert_eq!(setup(Setting::Practice).posture, Posture::Watch);
    }
}
