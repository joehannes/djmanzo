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
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const SETTINGS: &str = "looks.json";

/// One layer of a look.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    /// What it draws: `wash`, `pulse`, `ripple`, `sparks`, `halo`;
    /// `milkdrop`, a MilkDrop preset; `artist`, photographs of the heard
    /// deck's artist (`crate::artists`), each credited while it shows.
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

/// The kinds, signals and blends the interface draws. Kept here as a
/// list so a typo in a built-in look fails a test, and a pack's look is
/// checked before it is installed, rather than leaving a layer out on
/// someone's projector; `ui/src/looks.ts` has the same lists and its own
/// test that they are these.
pub const KINDS: [&str; 7] = [
    "wash", "pulse", "ripple", "sparks", "halo", "milkdrop", "artist",
];
pub const SIGNALS: [&str; 11] = [
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
pub const BLENDS: [&str; 3] = ["over", "add", "screen"];

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

/// The shipped preset called `name`, as Butterchurn reads it.
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
            name: "Artist".to_owned(),
            description: "Photographs of the artist playing, from Wikimedia Commons — freely \
                          licensed, each credited in the corner — drifting slowly and pressing in \
                          on the beat, over the record's colours: for a singer's favourite, or \
                          anyone the room came for."
                .to_owned(),
            layers: vec![
                layer("wash", "loudness", 0.6, "over"),
                layer("artist", "beat", 0.85, "over"),
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

/// The folder, beside the other settings, that holds the DJ's own MilkDrop
/// presets.
const FOLDER: &str = "milkdrop";

/// How a layer names one of the DJ's own presets rather than a shipped one.
const YOURS: &str = "yours/";

/// One of the DJ's own MilkDrop presets, as the list shows it (step 4b).
///
/// Theirs to answer for: djmanzo draws it and never sends it anywhere.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Yours {
    /// The file's name without its extension: what the look is called after.
    pub name: String,
    pub file: String,
    /// Worth knowing, such as shaders left out.
    pub note: Option<String>,
    /// Why it cannot be drawn, when it cannot; it then has no look.
    pub problem: Option<String>,
}

/// What adding presets came to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Added {
    /// Every preset there is now.
    pub presets: Vec<Yours>,
    /// Each file not taken, and why, in words.
    pub refused: Vec<String>,
}

/// A file's text, read as MilkDrop's own files are — mostly ASCII, and
/// Windows-1252 where they are not, so anything that is not UTF-8 is read
/// leniently rather than refused.
fn text_of(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// The layers a look made of one MilkDrop preset draws: the preset, and the
/// singers' voices as a halo over it. The DJ's own presets are offered so,
/// and so is each look in a pack made of them (step 7b).
pub(crate) fn preset_layers(preset: &str) -> Vec<Layer> {
    vec![milkdrop(preset, 0.9), layer("halo", "voice", 0.5, "screen")]
}

/// The look a preset of the DJ's own is offered as.
fn look_for(yours: &Yours) -> Look {
    let mut description = format!("Your own MilkDrop preset, from {}", yours.file);
    if let Some(note) = &yours.note {
        description.push_str(&format!(" — {note}"));
    }
    description.push('.');
    Look {
        name: format!("Yours: {}", yours.name),
        description,
        layers: preset_layers(&format!("{YOURS}{}", yours.name)),
    }
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
    /// Step 6: the DJ's AI provider picks the look for each record, over
    /// the look chosen here, which shows until it has and whenever it
    /// cannot. Off unless the DJ turns it on, as every AI feature is; a
    /// screen chosen off stays off.
    pub ai: bool,
}

/// Where the AI's picks are kept, by record: asked once a record, ever.
const PICKS: &str = "looks-picked.json";

/// How long after a pick did not come it is asked for again.
pub const WAIT: Duration = Duration::from_secs(10 * 60);

/// Whether a record's look may be asked for now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Begun {
    /// Ask.
    Go,
    /// Being asked already.
    Busy,
    /// It failed lately, for this reason; asked again after [`WAIT`].
    Waiting(String),
}

#[derive(Debug)]
enum Ask {
    Now,
    Failed(Instant, String),
}

#[derive(Default)]
struct Inner {
    dir: Option<PathBuf>,
    chosen: Chosen,
    /// The AI's pick for each record, by its id.
    picks: BTreeMap<String, String>,
    asks: HashMap<String, Ask>,
    /// Step 7: the packs installed, each checked when it was read, with its
    /// manifest when it passed.
    packs: Vec<(crate::packs::Installed, Option<crate::packs::Manifest>)>,
}

/// Where installed packs are kept, beside the other settings.
const PACKS: &str = "packs";

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
        self.inner().dir = Some(dir.to_path_buf());
        self.read_packs();
        let mut chosen: Chosen = std::fs::read_to_string(dir.join(SETTINGS))
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        // Not under the lock: the catalogue takes it to find the presets.
        let catalogue = self.catalogue();
        for kept in [&mut chosen.singers, &mut chosen.video] {
            if let Some(name) = kept
                && !catalogue.iter().any(|look| &look.name == name)
            {
                *kept = None;
            }
        }
        let picks = std::fs::read_to_string(dir.join(PICKS))
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        let mut inner = self.inner();
        inner.chosen = chosen;
        inner.picks = picks;
    }

    /// Write `what` to `file` beside the other settings, if there is a folder.
    fn save<T: Serialize>(dir: Option<&Path>, file: &str, what: &T) {
        let Some(dir) = dir else {
            return;
        };
        let written = serde_json::to_string_pretty(what)
            .map_err(|e| e.to_string())
            .and_then(|text| {
                std::fs::create_dir_all(dir)
                    .and_then(|()| std::fs::write(dir.join(file), text))
                    .map_err(|e| e.to_string())
            });
        if let Err(error) = written {
            tracing::warn!(%error, file, "a looks file was not written");
        }
    }

    /// Step 6: let the DJ's AI pick each record's look, or not.
    pub fn set_ai(&self, on: bool) -> Chosen {
        let mut inner = self.inner();
        inner.chosen.ai = on;
        Self::save(inner.dir.as_deref(), SETTINGS, &inner.chosen);
        inner.chosen.clone()
    }

    /// The look the AI picked for `track`, if it did and the look is still
    /// one there is: a removed look is no pick, and the record is asked
    /// about again.
    #[must_use]
    pub fn picked(&self, track: &str) -> Option<Look> {
        let name = self.inner().picks.get(track).cloned()?;
        self.catalogue().into_iter().find(|look| look.name == name)
    }

    /// Keep the AI's pick for `track`.
    pub fn keep_pick(&self, track: &str, name: &str) {
        let mut inner = self.inner();
        inner.picks.insert(track.to_owned(), name.to_owned());
        inner.asks.remove(track);
        Self::save(inner.dir.as_deref(), PICKS, &inner.picks);
    }

    /// Begin asking for `track`'s look, unless it is being asked for or
    /// failed within [`WAIT`] — so two screens hearing one record ask once.
    pub fn begin(&self, track: &str, now: Instant) -> Begun {
        let mut inner = self.inner();
        match inner.asks.get(track) {
            Some(Ask::Now) => return Begun::Busy,
            Some(Ask::Failed(when, why)) if now.duration_since(*when) < WAIT => {
                return Begun::Waiting(why.clone());
            }
            _ => {}
        }
        inner.asks.insert(track.to_owned(), Ask::Now);
        Begun::Go
    }

    /// The asking for `track` failed, for `why`.
    pub fn failed(&self, track: &str, why: String, now: Instant) {
        self.inner()
            .asks
            .insert(track.to_owned(), Ask::Failed(now, why));
    }

    /// Every look there is, as the DJ is offered them: djmanzo's own, then one
    /// for each of the DJ's presets that can be drawn.
    #[must_use]
    pub fn catalogue(&self) -> Vec<Look> {
        let mut looks = built_in();
        looks.extend(
            self.yours()
                .iter()
                .filter(|y| y.problem.is_none())
                .map(look_for),
        );
        let packs = self.inner().packs.clone();
        for (installed, manifest) in packs {
            if let Some(manifest) = manifest {
                looks.extend(
                    manifest
                        .looks
                        .iter()
                        .map(|look| crate::packs::offered(&installed.id, &manifest.name, look)),
                );
            }
        }
        looks
    }

    fn packs_folder(&self) -> Option<PathBuf> {
        self.inner().dir.as_ref().map(|dir| dir.join(PACKS))
    }

    /// Read every installed pack again, each checked whole: one changed on
    /// disk since it was installed is listed with why, and offers no look.
    fn read_packs(&self) {
        use crate::packs::{Installed, check, is_id, read_folder};
        let mut packs = Vec::new();
        if let Some(folder) = self.packs_folder() {
            let mut ids: Vec<String> = std::fs::read_dir(&folder)
                .into_iter()
                .flatten()
                .flatten()
                .filter(|entry| entry.path().is_dir())
                .filter_map(|entry| entry.file_name().into_string().ok())
                .filter(|id| is_id(id))
                .collect();
            ids.sort();
            for id in ids {
                let checked = read_folder(&folder.join(&id))
                    .and_then(check)
                    .and_then(|pack| {
                        if pack.id() == id {
                            Ok(pack)
                        } else {
                            Err("its folder is not its own".to_owned())
                        }
                    });
                packs.push(match checked {
                    Ok(pack) => (
                        Installed {
                            id: id.clone(),
                            name: pack.manifest.name.clone(),
                            author: pack.manifest.author.clone(),
                            licence: pack.manifest.licence.clone(),
                            version: pack.manifest.version.clone(),
                            description: pack.manifest.description.clone(),
                            looks: pack.manifest.looks.len(),
                            key: pack.fingerprint(),
                            problem: None,
                        },
                        Some(pack.manifest),
                    ),
                    Err(why) => (
                        Installed {
                            id: id.clone(),
                            name: id.clone(),
                            author: String::new(),
                            licence: String::new(),
                            version: String::new(),
                            description: String::new(),
                            looks: 0,
                            key: String::new(),
                            problem: Some(format!("changed since it was installed: {why}")),
                        },
                        None,
                    ),
                });
            }
        }
        self.inner().packs = packs;
    }

    /// Step 7: the packs installed.
    #[must_use]
    pub fn packs(&self) -> Vec<crate::packs::Installed> {
        self.inner()
            .packs
            .iter()
            .map(|(installed, _)| installed.clone())
            .collect()
    }

    /// What installing the pack at `path` would install, for the DJ to read
    /// first — its licence above all — or why it cannot be.
    #[must_use]
    pub fn inspect_pack(&self, path: &Path) -> crate::packs::Inspected {
        match crate::packs::read_archive(path).and_then(crate::packs::check) {
            Ok(pack) => {
                let installed = self
                    .inner()
                    .packs
                    .iter()
                    .find(|(_, manifest)| {
                        manifest
                            .as_ref()
                            .is_some_and(|m| m.name == pack.manifest.name)
                    })
                    .map(|(installed, _)| installed.version.clone());
                let mut seen = crate::packs::inspected(&pack, installed);
                seen.problem = self.clash(&pack).err();
                seen
            }
            Err(why) => crate::packs::Inspected {
                name: path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                author: String::new(),
                licence: String::new(),
                version: String::new(),
                description: String::new(),
                looks: Vec::new(),
                presets: 0,
                key: String::new(),
                installed: None,
                problem: Some(why),
            },
        }
    }

    /// A pack's name keeps its key: one of the same name signed by another
    /// key is not this pack's update.
    fn clash(&self, pack: &crate::packs::Checked) -> Result<(), String> {
        let inner = self.inner();
        for (installed, manifest) in &inner.packs {
            if let Some(manifest) = manifest
                && manifest.name == pack.manifest.name
                && installed.key != pack.fingerprint()
            {
                return Err(format!(
                    "a pack called {:?} is installed, signed by another key ({}); this one is signed by {}. Remove it first if this is the one you want",
                    manifest.name,
                    installed.key,
                    pack.fingerprint()
                ));
            }
        }
        Ok(())
    }

    /// Install the pack at `path`, checked again, in place of an earlier
    /// version of it.
    ///
    /// # Errors
    /// It does not pass [`crate::packs::check`], another key's pack has its
    /// name, there is no settings folder, or it could not be written.
    pub fn install_pack(&self, path: &Path) -> Result<Vec<crate::packs::Installed>, String> {
        let pack = crate::packs::read_archive(path).and_then(crate::packs::check)?;
        self.clash(&pack)?;
        let folder = self
            .packs_folder()
            .ok_or_else(|| "there is no settings folder yet".to_owned())?;
        let id = pack.id();
        let incoming = folder.join(format!(".incoming-{id}"));
        let _ = std::fs::remove_dir_all(&incoming);
        crate::packs::write_folder(&incoming, &pack)?;
        let target = folder.join(&id);
        if target.exists() {
            std::fs::remove_dir_all(&target).map_err(|e| e.to_string())?;
        }
        std::fs::rename(&incoming, &target).map_err(|e| e.to_string())?;
        self.read_packs();
        Ok(self.packs())
    }

    /// Remove the installed pack `id`, and any screen's choice of its looks.
    ///
    /// # Errors
    /// No pack is installed by that id.
    pub fn remove_pack(&self, id: &str) -> Result<Vec<crate::packs::Installed>, String> {
        let name = self
            .inner()
            .packs
            .iter()
            .find(|(installed, _)| installed.id == id)
            .map(|(installed, manifest)| {
                manifest
                    .as_ref()
                    .map_or_else(|| installed.name.clone(), |m| m.name.clone())
            })
            .ok_or_else(|| format!("there is no pack {id:?}"))?;
        let folder = self
            .packs_folder()
            .ok_or_else(|| "there is no settings folder yet".to_owned())?;
        // Found among the folders listed, never a name joined to a path the
        // listing did not give.
        std::fs::remove_dir_all(folder.join(id)).map_err(|e| e.to_string())?;
        self.read_packs();
        let prefix = format!("{name}: ");
        let mut inner = self.inner();
        let chosen = &mut inner.chosen;
        for kept in [&mut chosen.singers, &mut chosen.video] {
            if kept
                .as_deref()
                .is_some_and(|look| look.starts_with(&prefix))
            {
                *kept = None;
            }
        }
        Self::save(inner.dir.as_deref(), SETTINGS, &inner.chosen);
        drop(inner);
        Ok(self.packs())
    }

    /// Step 7b: the DJ's signing key — made the first time it is asked
    /// for — by its fingerprint, and where it is kept.
    ///
    /// # Errors
    /// There is no settings folder yet, or the key file there is not a key.
    pub fn pack_key(&self) -> Result<crate::packs::Key, String> {
        let dir = self
            .inner()
            .dir
            .clone()
            .ok_or_else(|| "there is no settings folder yet".to_owned())?;
        let pkcs8 = crate::packs::signing_key(&dir)?;
        let key = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            crate::packs::public_key(&pkcs8)?,
        )
        .map_err(|e| e.to_string())?;
        Ok(crate::packs::Key {
            fingerprint: crate::packs::fingerprint(&key),
            file: dir.join(crate::packs::KEY).display().to_string(),
        })
    }

    /// Step 7b: make a pack of the DJ's own presets, as `making` asks,
    /// signed with their key, and write it to `to` — `.djpack` added when it
    /// has no extension. What is written is read back and checked as an
    /// installer checks it before it is said to be made.
    ///
    /// # Errors
    /// A preset named that is not one of theirs, what
    /// [`crate::packs::make`] refuses, or the file could not be written.
    pub fn make_pack(
        &self,
        making: &crate::packs::Making,
        to: &Path,
    ) -> Result<crate::packs::Made, String> {
        let dir = self
            .inner()
            .dir
            .clone()
            .ok_or_else(|| "there is no settings folder yet".to_owned())?;
        let files = self.files();
        let mut presets = Vec::new();
        for name in &making.presets {
            // Found among the files listed, never by joining the name to a path.
            let path = files
                .iter()
                .find(|p| p.file_stem().is_some_and(|s| s.to_string_lossy() == *name))
                .ok_or_else(|| format!("there is no preset of yours called {name:?}"))?;
            let file = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            presets.push((file, std::fs::read(path).map_err(|e| e.to_string())?));
        }
        let pkcs8 = crate::packs::signing_key(&dir)?;
        let pack = crate::packs::make(making, &presets, &pkcs8)?;
        let bytes = crate::packs::archive(&pack)?;
        let to = if to.extension().is_none() {
            to.with_extension("djpack")
        } else {
            to.to_path_buf()
        };
        let part = to.with_extension("djpack-part");
        std::fs::write(&part, bytes)
            .and_then(|()| std::fs::rename(&part, &to))
            .map_err(|e| {
                let _ = std::fs::remove_file(&part);
                format!("{}: {e}", to.display())
            })?;
        let again = crate::packs::read_archive(&to).and_then(crate::packs::check)?;
        Ok(crate::packs::Made {
            path: to.display().to_string(),
            name: again.manifest.name.clone(),
            version: again.manifest.version.clone(),
            looks: again.manifest.looks.len(),
            key: again.fingerprint(),
        })
    }

    /// A pack's preset, `<id>/<name>`, found among the pack's own listed
    /// files and checked against its hash again.
    fn pack_preset(&self, named: &str) -> Result<serde_json::Value, String> {
        let (id, stem) = named
            .split_once('/')
            .ok_or_else(|| format!("there is no pack preset called {named:?}"))?;
        let (installed, manifest) = self
            .inner()
            .packs
            .iter()
            .find(|(installed, manifest)| installed.id == id && manifest.is_some())
            .cloned()
            .ok_or_else(|| format!("there is no pack {id:?}"))?;
        let manifest = manifest.unwrap_or_else(|| unreachable!("found with a manifest"));
        let (path, hash) = manifest
            .files
            .iter()
            .find(|(path, _)| {
                path.strip_prefix(crate::packs::PRESETS)
                    .and_then(|file| file.rsplit_once('.'))
                    .is_some_and(|(name, _)| name == stem)
            })
            .ok_or_else(|| format!("{} has no preset called {stem:?}", installed.name))?;
        let folder = self
            .packs_folder()
            .ok_or_else(|| "there is no settings folder yet".to_owned())?;
        let bytes = std::fs::read(folder.join(id).join(path)).map_err(|e| e.to_string())?;
        if !crate::packs::sha256(&bytes).eq_ignore_ascii_case(hash) {
            return Err(format!(
                "{path} in {} has changed since it was installed",
                installed.name
            ));
        }
        let file = path.strip_prefix(crate::packs::PRESETS).unwrap_or(path);
        crate::milk::read(file, &text_of(&bytes)).map(|read| read.preset)
    }

    fn folder(&self) -> Option<PathBuf> {
        self.inner().dir.as_ref().map(|dir| dir.join(FOLDER))
    }

    /// The preset files in the folder, by name: `.milk` and `.json` only.
    fn files(&self) -> Vec<PathBuf> {
        let Some(folder) = self.folder() else {
            return Vec::new();
        };
        let mut files: Vec<PathBuf> = std::fs::read_dir(folder)
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                path.is_file()
                    && path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
                        e.eq_ignore_ascii_case("milk") || e.eq_ignore_ascii_case("json")
                    })
            })
            .collect();
        files.sort();
        files
    }

    /// The DJ's own presets, each read to say whether it can be drawn.
    #[must_use]
    pub fn yours(&self) -> Vec<Yours> {
        self.files()
            .iter()
            .map(|path| {
                let file = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let name = path
                    .file_stem()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let read = std::fs::read(path)
                    .map_err(|e| e.to_string())
                    .and_then(|bytes| crate::milk::read(&file, &text_of(&bytes)));
                match read {
                    Ok(read) => Yours {
                        name,
                        file,
                        note: read.note,
                        problem: None,
                    },
                    Err(problem) => Yours {
                        name,
                        file,
                        note: None,
                        problem: Some(problem),
                    },
                }
            })
            .collect()
    }

    /// The preset a layer names, as Butterchurn reads it: one djmanzo ships,
    /// or one of the DJ's own (`yours/<name>`), read from its file now.
    ///
    /// # Errors
    /// No such preset, or the DJ's file cannot be read.
    pub fn preset(&self, name: &str) -> Result<serde_json::Value, String> {
        if let Some(packed) = name.strip_prefix("pack/") {
            return self.pack_preset(packed);
        }
        let Some(theirs) = name.strip_prefix(YOURS) else {
            return preset(name);
        };
        // Found among the files listed, never by joining the name to a path.
        let path = self
            .files()
            .into_iter()
            .find(|p| p.file_stem().is_some_and(|s| s.to_string_lossy() == theirs))
            .ok_or_else(|| format!("there is no preset of yours called {theirs:?}"))?;
        let file = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
        crate::milk::read(&file, &text_of(&bytes)).map(|read| read.preset)
    }

    /// Copy the DJ's preset files into the folder, each only if it can be
    /// drawn, and say why of each that is not.
    pub fn add(&self, paths: &[PathBuf]) -> Added {
        let mut refused = Vec::new();
        match self.folder() {
            None => refused.push("djmanzo has nowhere to keep presets yet".to_owned()),
            Some(folder) => {
                for path in paths {
                    let file = path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let taken = std::fs::metadata(path)
                        .map_err(|e| e.to_string())
                        .and_then(|meta| {
                            if meta.len() > crate::milk::LARGEST {
                                Err("it is too large to be a preset".to_owned())
                            } else {
                                std::fs::read(path).map_err(|e| e.to_string())
                            }
                        })
                        .and_then(|bytes| crate::milk::read(&file, &text_of(&bytes)).map(|_| bytes))
                        .and_then(|bytes| {
                            std::fs::create_dir_all(&folder)
                                .and_then(|()| std::fs::write(folder.join(&file), bytes))
                                .map_err(|e| e.to_string())
                        });
                    if let Err(why) = taken {
                        refused.push(format!("{file}: {why}"));
                    }
                }
            }
        }
        Added {
            presets: self.yours(),
            refused,
        }
    }

    /// Remove one of the DJ's presets; a screen showing its look shows none.
    ///
    /// # Errors
    /// No preset of theirs has that name, or the file would not go.
    pub fn remove(&self, name: &str) -> Result<Vec<Yours>, String> {
        let path = self
            .files()
            .into_iter()
            .find(|p| p.file_stem().is_some_and(|s| s.to_string_lossy() == name))
            .ok_or_else(|| format!("there is no preset of yours called {name:?}"))?;
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
        let look = format!("Yours: {name}");
        let chosen = self.chosen();
        for (screen, kept) in [
            (Screen::Singers, chosen.singers),
            (Screen::Video, chosen.video),
        ] {
            if kept.as_deref() == Some(look.as_str()) {
                self.choose(screen, None)?;
            }
        }
        Ok(self.yours())
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
        Self::save(inner.dir.as_deref(), SETTINGS, &inner.chosen);
        Ok(inner.chosen.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// A MilkDrop preset of the DJ's own, as a `.milk` file.
    const THEIRS: &str = "[preset00]\nfDecay=0.950000\nzoom=1.020000\nper_frame_1=rot = 0.01;\n";

    /// **The DJ's own presets: taken when they can be drawn, refused with
    /// the reason when they cannot, offered as looks, drawn from their file,
    /// and gone — with the screens showing them — when removed.**
    #[test]
    fn the_djs_own_presets_are_taken_offered_and_removed() {
        let home = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        let looks = Looks::new();
        looks.start(home.path());
        assert!(looks.yours().is_empty());

        let file = |name: &str, bytes: &[u8]| {
            let path = elsewhere.path().join(name);
            std::fs::write(&path, bytes).unwrap();
            path
        };
        let added = looks.add(&[
            file("Night Drive.milk", THEIRS.as_bytes()),
            file("scripted.json", br#"{"init_eqs_str":"a.q1=0;"}"#),
            file("notes.txt", b"fDecay=0.9"),
            // A valid preset, refused for its size alone.
            file("huge.milk", "fDecay=0.9\n".repeat(60_000).as_bytes()),
        ]);
        assert_eq!(added.presets.len(), 1, "{added:?}");
        assert_eq!(added.presets[0].name, "Night Drive");
        assert_eq!(added.presets[0].problem, None);
        assert_eq!(added.refused.len(), 3, "{added:?}");
        assert!(
            added.refused[0].starts_with("scripted.json: ")
                && added.refused[0].contains("JavaScript")
        );
        assert!(added.refused[1].starts_with("notes.txt: "));
        assert!(added.refused[2].contains("too large"));

        // Offered as a look, drawn from the file.
        let look = looks
            .catalogue()
            .into_iter()
            .find(|l| l.name == "Yours: Night Drive")
            .expect("a look for the preset");
        assert_eq!(look.layers[0].kind, "milkdrop");
        let name = look.layers[0].preset.clone().unwrap();
        let drawn = looks.preset(&name).unwrap();
        assert_eq!(drawn["baseVals"]["decay"], 0.95);
        assert_eq!(drawn["frame_eqs_eel"], "rot = 0.01;");
        // Shipped presets are still found by their own names.
        assert!(looks.preset("Drift").is_ok());

        // A file that cannot be drawn is listed with why, and has no look.
        std::fs::write(home.path().join(FOLDER).join("broken.json"), "not json").unwrap();
        let broken = looks
            .yours()
            .into_iter()
            .find(|y| y.name == "broken")
            .unwrap();
        assert!(broken.problem.is_some());
        assert!(!looks.catalogue().iter().any(|l| l.name == "Yours: broken"));

        // Chosen, then removed: the file goes, and so does the choice.
        looks
            .choose(Screen::Singers, Some("Yours: Night Drive".to_owned()))
            .unwrap();
        looks
            .choose(Screen::Video, Some("Glow".to_owned()))
            .unwrap();
        let left = looks.remove("Night Drive").unwrap();
        assert_eq!(
            left.iter().map(|y| y.name.as_str()).collect::<Vec<_>>(),
            ["broken"]
        );
        assert!(!home.path().join(FOLDER).join("Night Drive.milk").exists());
        assert_eq!(looks.chosen().singers, None);
        assert_eq!(
            looks.chosen().video.as_deref(),
            Some("Glow"),
            "another screen's choice stays"
        );
        assert!(looks.remove("Night Drive").is_err());
    }

    /// A preset's name finds a file in the folder, and nothing outside it.
    #[test]
    fn a_presets_name_reaches_nothing_outside_the_folder() {
        let home = tempfile::tempdir().unwrap();
        std::fs::write(home.path().join("outside.milk"), THEIRS).unwrap();
        // The folder exists, as it does once the DJ has added one: a path
        // through it to `..` would then resolve, so only the listing stops it.
        std::fs::create_dir_all(home.path().join(FOLDER)).unwrap();
        std::fs::write(home.path().join(FOLDER).join("inside.milk"), THEIRS).unwrap();
        let looks = Looks::new();
        looks.start(home.path());
        assert!(looks.preset("yours/inside").is_ok());
        assert!(looks.preset("yours/../outside").is_err());
        assert!(looks.remove("../outside").is_err());
        assert!(home.path().join("outside.milk").exists());
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

    /// **Step 6 is off until the DJ turns it on**, and kept: a settings file
    /// from before it existed reads as off.
    #[test]
    fn the_ai_picking_is_off_until_turned_on_and_kept() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(SETTINGS), r#"{"singers":"Glow"}"#).unwrap();
        let looks = Looks::new();
        looks.start(dir.path());
        assert!(!looks.chosen().ai);
        assert!(looks.set_ai(true).ai);
        let again = Looks::new();
        again.start(dir.path());
        assert!(again.chosen().ai);
        assert_eq!(again.chosen().singers.as_deref(), Some("Glow"));
        again.set_ai(false);
        let off = Looks::new();
        off.start(dir.path());
        assert!(!off.chosen().ai);
    }

    /// **One call a record, ever**: a pick is kept by the record's id between
    /// runs; a pick of a look since removed is no pick, so the record is
    /// asked about again rather than shown a look that draws nothing.
    #[test]
    fn a_records_pick_is_kept_and_a_removed_look_is_no_pick() {
        let dir = tempfile::tempdir().unwrap();
        let looks = Looks::new();
        looks.start(dir.path());
        assert_eq!(looks.picked("aa"), None);
        looks.keep_pick("aa", "Pulse");
        assert_eq!(
            looks.picked("aa").map(|look| look.name).as_deref(),
            Some("Pulse")
        );
        assert_eq!(
            looks.picked("bb"),
            None,
            "one record's pick is not another's"
        );

        let again = Looks::new();
        again.start(dir.path());
        assert_eq!(
            again.picked("aa").map(|look| look.name).as_deref(),
            Some("Pulse")
        );

        std::fs::write(dir.path().join(PICKS), r#"{"aa":"Yours: Gone"}"#).unwrap();
        let later = Looks::new();
        later.start(dir.path());
        assert_eq!(later.picked("aa"), None);
    }

    /// Two screens hearing one record ask once between them, and a failure
    /// is waited on — said why — before the record is asked about again.
    #[test]
    fn a_record_is_asked_about_once_and_a_failure_waited_on() {
        let looks = Looks::new();
        let now = Instant::now();
        assert_eq!(looks.begin("aa", now), Begun::Go);
        assert_eq!(looks.begin("aa", now), Begun::Busy);
        assert_eq!(
            looks.begin("bb", now),
            Begun::Go,
            "another record is asked about"
        );
        looks.failed("aa", "the budget is spent".to_owned(), now);
        assert_eq!(
            looks.begin("aa", now + WAIT / 2),
            Begun::Waiting("the budget is spent".to_owned())
        );
        assert_eq!(looks.begin("aa", now + WAIT), Begun::Go);
        looks.keep_pick("aa", "Glow");
        assert_eq!(
            looks.begin("aa", now + WAIT),
            Begun::Go,
            "a kept pick ends the asking"
        );
    }
}
