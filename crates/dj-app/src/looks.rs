//! §122: looks — the layered visuals djmanzo ships, and which one a screen
//! shows.
//!
//! A **look** is an ordered list of layers, bottom first; a **layer** is a
//! kind of picture, the signal it answers to, a colour, an amount and a
//! blend. The pictures themselves are the interface's (`ui/src/visualStack.ts`,
//! small WebGL fragment shaders), and so is checking a layer's kind and
//! signal before drawing it: what is kept here is data, so the catalogue can
//! be listed to the DJ's AI provider by name and description (step 6 of
//! `docs/VISUALS.md`) and grow with installed packs (step 7) without the
//! interface changing.
//!
//! The choice is kept in `looks.json` beside the other settings, one per
//! screen, because a screen that draws it is usually a window of its own: the
//! DJ picks in the Singers panel or the Video panel, and the singers' screen
//! or the projector's window reads it.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const SETTINGS: &str = "looks.json";

/// One layer of a look.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    /// What it draws: `wash`, `pulse`, `ripple`, `sparks`, `halo`.
    pub kind: String,
    /// What it answers to: `loudness`, `band.bass` … `band.treble`, `beat`,
    /// `stem.vocal` … `stem.other`, or `voice` (the singers' microphones).
    pub signal: String,
    /// `record` for the record's own colours, or a fixed `#rrggbb`.
    pub colour: String,
    /// How strongly it is drawn, 0..=1.
    pub amount: f32,
    /// How it lies over what is below: `over`, `add` or `screen`.
    pub blend: String,
    /// For a `milkdrop` layer, the MilkDrop preset it draws (step 4): one of
    /// [`PRESETS`]. Absent for every other kind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<String>,
}

/// A look: its name, what it is for, and its layers, bottom first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Look {
    pub name: String,
    /// One sentence on what it looks like and what music it suits: what the
    /// DJ reads in the list, and what an AI picking a look per song is told.
    pub description: String,
    pub layers: Vec<Layer>,
}

fn layer(kind: &str, signal: &str, amount: f32, blend: &str) -> Layer {
    Layer {
        kind: kind.to_owned(),
        signal: signal.to_owned(),
        colour: "record".to_owned(),
        amount,
        blend: blend.to_owned(),
        preset: None,
    }
}

/// A MilkDrop layer — Butterchurn drawing `preset` — as the look's main layer.
fn milkdrop(preset: &str, amount: f32) -> Layer {
    Layer {
        preset: Some(preset.to_owned()),
        ..layer("milkdrop", "loudness", amount, "over")
    }
}

/// The MilkDrop presets djmanzo ships, by name: written for djmanzo, under
/// its own licence (`docs/VISUALS.md` step 4).
///
/// Butterchurn's own format, and only its WebAssembly half: every equation is
/// MilkDrop's own language (`*_eqs_eel`) and none is JavaScript (`*_eqs_str`),
/// because the interface runs Butterchurn with `onlyUseWASM` and its content
/// rules allow WebAssembly to be compiled but never JavaScript to be
/// evaluated. The warp and composite shaders are empty, which is Butterchurn's
/// default pair — no shader code is shipped.
pub const PRESETS: [(&str, &str); 2] = [
    ("Drift", include_str!("../presets/milkdrop/drift.json")),
    ("Bloom", include_str!("../presets/milkdrop/bloom.json")),
];

/// The preset called `name`, as Butterchurn reads it.
///
/// # Errors
/// No preset has that name, which is a look naming one djmanzo does not ship.
pub fn preset(name: &str) -> Result<serde_json::Value, String> {
    let (_, text) = PRESETS
        .iter()
        .find(|(known, _)| *known == name)
        .ok_or_else(|| format!("there is no MilkDrop preset called {name:?}"))?;
    serde_json::from_str(text).map_err(|e| format!("the preset {name:?} is not JSON: {e}"))
}

/// The looks djmanzo ships, all its own.
///
/// Every one ends in a `halo` answering to the singers' microphones, not to
/// the vocal stem: on a karaoke screen the record's singer is usually the one
/// thing taken *out*, so the vocal stem is near silence exactly when a halo
/// around the words is wanted.
#[must_use]
pub fn built_in() -> Vec<Look> {
    vec![
        Look {
            name: "Glow".to_owned(),
            description: "The record's colours breathing with the room's loudness, and a glow \
                          around the words when someone sings: calm, for anything."
                .to_owned(),
            layers: vec![
                layer("wash", "loudness", 0.8, "over"),
                layer("halo", "voice", 0.6, "screen"),
            ],
        },
        Look {
            name: "Pulse".to_owned(),
            description: "Rings from the centre on every beat, as strong as the drums: for \
                          dance music, anything with a kick."
                .to_owned(),
            layers: vec![
                layer("wash", "loudness", 0.5, "over"),
                layer("pulse", "stem.drums", 0.8, "add"),
                layer("halo", "voice", 0.5, "screen"),
            ],
        },
        Look {
            name: "Waves".to_owned(),
            description: "Slow waves crossing the screen with the instruments and the mids: for \
                          ballads and anything that sways."
                .to_owned(),
            layers: vec![
                layer("wash", "loudness", 0.6, "over"),
                layer("ripple", "stem.other", 0.6, "screen"),
                layer("halo", "voice", 0.5, "screen"),
            ],
        },
        Look {
            name: "Sparkle".to_owned(),
            description: "Sparks that flare with the treble over a dark wash, the bass pulsing \
                          beneath: for disco, bright pop and anything with hi-hats."
                .to_owned(),
            layers: vec![
                layer("wash", "loudness", 0.35, "over"),
                layer("pulse", "band.bass", 0.5, "add"),
                layer("sparks", "band.treble", 0.9, "add"),
                layer("halo", "voice", 0.5, "screen"),
            ],
        },
        Look {
            name: "Drift".to_owned(),
            description: "A MilkDrop picture of djmanzo's own: slow rolling waves that swell with \
                          the bass and drift with the treble, the singer's glow over it — for \
                          long, building songs."
                .to_owned(),
            layers: vec![
                milkdrop("Drift", 0.9),
                layer("halo", "voice", 0.5, "screen"),
            ],
        },
        Look {
            name: "Bloom".to_owned(),
            description: "A MilkDrop picture of djmanzo's own: a burst of colour in the middle \
                          whose trails spread out on the kick, rings on the beat over it — for \
                          dance music and big choruses."
                .to_owned(),
            layers: vec![
                milkdrop("Bloom", 0.9),
                layer("pulse", "beat", 0.3, "add"),
                layer("halo", "voice", 0.5, "screen"),
            ],
        },
        Look {
            name: "Everything".to_owned(),
            description: "Every layer at once — colours, beat, waves, sparks and the singer's \
                          glow: for a big chorus, and for seeing what the layers do."
                .to_owned(),
            layers: vec![
                layer("wash", "loudness", 0.6, "over"),
                layer("pulse", "beat", 0.5, "add"),
                layer("ripple", "band.low_mid", 0.4, "screen"),
                layer("sparks", "band.treble", 0.7, "add"),
                layer("halo", "voice", 0.5, "screen"),
            ],
        },
    ]
}

/// Where a look can be shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Screen {
    /// Behind the words on the singers' screen.
    Singers,
    /// In the Video window when no deck with a picture is heard, so the
    /// projector is not left black.
    Video,
}

/// Which look each screen shows; `None` is off.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Chosen {
    pub singers: Option<String>,
    pub video: Option<String>,
}

#[derive(Default)]
struct Inner {
    dir: Option<PathBuf>,
    chosen: Chosen,
}

/// The catalogue, and the choice, kept between runs.
#[derive(Default)]
pub struct Looks {
    inner: Mutex<Inner>,
}

impl std::fmt::Debug for Looks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Looks").finish_non_exhaustive()
    }
}

impl Looks {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn inner(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Read what was kept in `dir`. A look no longer in the catalogue is
    /// forgotten rather than shown as a name that draws nothing.
    pub fn start(&self, dir: &Path) {
        let mut inner = self.inner();
        inner.dir = Some(dir.to_path_buf());
        let mut chosen: Chosen = std::fs::read_to_string(dir.join(SETTINGS))
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        let catalogue = self.catalogue();
        for kept in [&mut chosen.singers, &mut chosen.video] {
            if let Some(name) = kept
                && !catalogue.iter().any(|look| &look.name == name)
            {
                *kept = None;
            }
        }
        inner.chosen = chosen;
    }

    /// Every look there is, as the DJ is offered them.
    #[must_use]
    pub fn catalogue(&self) -> Vec<Look> {
        built_in()
    }

    #[must_use]
    pub fn chosen(&self) -> Chosen {
        self.inner().chosen.clone()
    }

    /// Show `name` on `screen`, or nothing with `None`, and keep the choice.
    ///
    /// # Errors
    /// No look is called `name`.
    pub fn choose(&self, screen: Screen, name: Option<String>) -> Result<Chosen, String> {
        if let Some(name) = &name
            && !self.catalogue().iter().any(|look| &look.name == name)
        {
            return Err(format!("there is no look called {name:?}"));
        }
        let mut inner = self.inner();
        match screen {
            Screen::Singers => inner.chosen.singers = name,
            Screen::Video => inner.chosen.video = name,
        }
        if let Some(dir) = &inner.dir {
            let written = serde_json::to_string_pretty(&inner.chosen)
                .map_err(|e| e.to_string())
                .and_then(|text| {
                    std::fs::create_dir_all(dir)
                        .and_then(|()| std::fs::write(dir.join(SETTINGS), text))
                        .map_err(|e| e.to_string())
                });
            if let Err(error) = written {
                tracing::warn!(%error, "looks.json was not written");
            }
        }
        Ok(inner.chosen.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The kinds, signals and blends the interface draws. Kept here as a
    /// list so a typo in a built-in look fails a test rather than leaving a
    /// layer out on someone's projector; `ui/src/looks.ts` has the same lists
    /// and its own test that they are these.
    const KINDS: [&str; 6] = ["wash", "pulse", "ripple", "sparks", "halo", "milkdrop"];
    const SIGNALS: [&str; 11] = [
        "loudness",
        "band.bass",
        "band.low_mid",
        "band.high_mid",
        "band.treble",
        "beat",
        "stem.vocal",
        "stem.drums",
        "stem.bass",
        "stem.other",
        "voice",
    ];
    const BLENDS: [&str; 3] = ["over", "add", "screen"];

    #[test]
    fn every_built_in_look_is_drawable_and_named_once() {
        let looks = built_in();
        assert!(looks.len() >= 3);
        let mut names: Vec<_> = looks.iter().map(|l| l.name.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), looks.len(), "two looks share a name");
        for look in &looks {
            assert!(!look.description.is_empty(), "{} says nothing", look.name);
            assert!(!look.layers.is_empty(), "{} draws nothing", look.name);
            for layer in &look.layers {
                assert!(
                    KINDS.contains(&layer.kind.as_str()),
                    "{}: {layer:?}",
                    look.name
                );
                assert!(
                    SIGNALS.contains(&layer.signal.as_str()),
                    "{}: {layer:?}",
                    look.name
                );
                assert!(
                    BLENDS.contains(&layer.blend.as_str()),
                    "{}: {layer:?}",
                    look.name
                );
                assert!(
                    (0.0..=1.0).contains(&layer.amount),
                    "{}: {layer:?}",
                    look.name
                );
                assert_eq!(layer.colour, "record");
                // A MilkDrop layer names a preset djmanzo ships; nothing else names one.
                if layer.kind == "milkdrop" {
                    let name = layer
                        .preset
                        .as_deref()
                        .expect("a MilkDrop layer names its preset");
                    assert!(preset(name).is_ok(), "{}: {name} is not shipped", look.name);
                } else {
                    assert_eq!(layer.preset, None, "{}: {layer:?}", look.name);
                }
            }
            // The words' halo answers to whoever is singing *now*.
            assert!(
                look.layers
                    .iter()
                    .filter(|l| l.kind == "halo")
                    .all(|l| l.signal == "voice"),
                "{}'s halo follows the record's singer, who karaoke takes out",
                look.name
            );
        }
    }

    /// **Every shipped preset runs with no JavaScript evaluated.** The
    /// interface gives Butterchurn `onlyUseWASM`, and its content rules
    /// forbid `eval`: a preset carrying JavaScript equations would be
    /// refused the night it was chosen. Each must also have the four
    /// shapes and four waves Butterchurn's WebAssembly path indexes, or it
    /// throws while loading.
    #[test]
    fn every_shipped_preset_is_milkdrops_own_language_and_whole() {
        for (name, _) in PRESETS {
            let preset = preset(name).unwrap();
            let text = preset.to_string();
            assert!(
                !text.contains("_eqs_str"),
                "{name} carries JavaScript equations"
            );
            for field in [
                "init_eqs_eel",
                "frame_eqs_eel",
                "pixel_eqs_eel",
                "warp",
                "comp",
            ] {
                assert!(preset[field].is_string(), "{name} has no {field}");
            }
            assert!(preset["baseVals"].is_object(), "{name} has no base values");
            assert_eq!(
                preset["shapes"].as_array().map(Vec::len),
                Some(4),
                "{name}'s shapes"
            );
            assert_eq!(
                preset["waves"].as_array().map(Vec::len),
                Some(4),
                "{name}'s waves"
            );
            for wave in preset["waves"].as_array().unwrap() {
                assert!(wave["point_eqs_eel"].is_string(), "{name}: {wave}");
            }
        }
        assert!(preset("Lasers").unwrap_err().contains("Lasers"));
    }

    /// **MilkDrop's equations may be compiled; JavaScript may never be
    /// evaluated.** Butterchurn compiles a preset's equations to WebAssembly,
    /// which the content rules must allow (`'wasm-unsafe-eval'`) or no
    /// MilkDrop layer ever draws in the application — the browser tests,
    /// which have no such rule unless they add it, would not notice. And
    /// `'unsafe-eval'` must never be the price of it: it would let any
    /// string anywhere in the interface run as code.
    #[test]
    fn the_content_rules_let_milkdrop_compile_and_nothing_evaluate() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).expect("the config is JSON");
        let csp = config["app"]["security"]["csp"]
            .as_str()
            .expect("a content security policy");
        let scripts: Vec<&str> = csp
            .split(';')
            .map(str::trim)
            .find_map(|rule| rule.strip_prefix("script-src "))
            .expect("a script-src rule")
            .split_whitespace()
            .collect();
        assert!(scripts.contains(&"'wasm-unsafe-eval'"), "{scripts:?}");
        assert!(scripts.contains(&"'self'"), "{scripts:?}");
        assert!(!csp.contains("'unsafe-eval'"), "{csp}");
        assert!(!scripts.contains(&"'unsafe-inline'"), "{scripts:?}");
    }

    #[test]
    fn a_choice_is_kept_between_runs_and_off_is_a_choice() {
        let dir = tempfile::tempdir().unwrap();
        let looks = Looks::new();
        looks.start(dir.path());
        assert_eq!(looks.chosen().singers, None, "off until the DJ picks one");

        let chosen = looks
            .choose(Screen::Singers, Some("Pulse".to_owned()))
            .unwrap();
        assert_eq!(chosen.singers.as_deref(), Some("Pulse"));
        assert_eq!(chosen.video, None, "one screen's choice is not another's");
        looks
            .choose(Screen::Video, Some("Waves".to_owned()))
            .unwrap();

        let again = Looks::new();
        again.start(dir.path());
        assert_eq!(again.chosen().singers.as_deref(), Some("Pulse"));
        assert_eq!(again.chosen().video.as_deref(), Some("Waves"));

        again.choose(Screen::Singers, None).unwrap();
        let off = Looks::new();
        off.start(dir.path());
        assert_eq!(off.chosen().singers, None);
        assert_eq!(off.chosen().video.as_deref(), Some("Waves"));
    }

    #[test]
    fn a_look_that_is_not_there_is_refused_and_forgotten() {
        let dir = tempfile::tempdir().unwrap();
        let looks = Looks::new();
        looks.start(dir.path());
        looks
            .choose(Screen::Singers, Some("Glow".to_owned()))
            .unwrap();
        let refused = looks.choose(Screen::Singers, Some("Lasers".to_owned()));
        assert!(refused.unwrap_err().contains("Lasers"));
        assert_eq!(
            looks.chosen().singers.as_deref(),
            Some("Glow"),
            "kept as it was"
        );

        // A look kept from a pack since removed is forgotten, not shown blank.
        std::fs::write(
            dir.path().join(SETTINGS),
            r#"{"singers":"Gone","video":"Also gone"}"#,
        )
        .unwrap();
        let later = Looks::new();
        later.start(dir.path());
        assert_eq!(later.chosen().singers, None);
        assert_eq!(later.chosen().video, None);
    }
}
