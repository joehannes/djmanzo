//! Controllers and the keyboard, wired to the action bus.
//!
//! # One path in
//!
//! A pad on a controller, a key on the laptop, a button in the interface and a
//! line in a script all end at [`crate::commands::perform`]. That is
//! [ADR-0003](../../docs/adr/0003-action-bus-and-parameter-registry.md) in
//! practice: nothing here can reach the engine directly, and nothing here can
//! do anything the interface cannot, because everything it produces is text
//! that has to survive `Action::parse`.
//!
//! # Where a mapping comes from
//!
//! Three places, in this order:
//!
//! 1. a file in the user's `mappings` directory, which wins;
//! 2. a mapping bundled with the application;
//! 3. nothing, and the controller sits there quietly.
//!
//! Bundled mappings are compiled in rather than installed, so a fresh install
//! on a machine with nothing configured still has a working keyboard.

use dj_hid::{KeyMap, Mapping};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};

/// A mapping the interface can list, whether or not it is in use.
#[derive(Debug, Clone, serde::Serialize)]
pub struct MappingDto {
    pub name: String,
    /// The device it is for, or empty when it is not for a particular one.
    pub device: String,
    /// How many controls it binds — the difference between a real mapping and
    /// a stub, at a glance.
    pub bindings: usize,
    /// False when it came from the user's own `mappings` directory.
    pub bundled: bool,
}

/// One key on the shortcut sheet.
#[derive(Debug, Clone, serde::Serialize)]
pub struct KeyDto {
    /// The canonical chord: `shift+space`, `keyq`.
    pub chord: String,
    /// What it does, in words.
    pub label: String,
    /// Which part of the sheet it belongs to.
    pub group: String,
    /// Whether it undoes itself on release.
    pub held: bool,
    pub press: Option<String>,
    pub release: Option<String>,
}

/// What is plugged in and what is listening to it.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ControlStatus {
    /// Every MIDI input the machine can see.
    pub inputs: Vec<String>,
    /// The port currently open, if any.
    pub open_port: Option<String>,
    /// The mapping in use on it.
    pub open_mapping: Option<String>,
    /// Why there are no inputs, when the reason is that MIDI itself is
    /// unavailable rather than that nothing is plugged in. Said out loud
    /// because "no controllers found" and "this machine has no MIDI service"
    /// are different problems and only one of them is fixed by plugging
    /// something in.
    pub unavailable: Option<String>,
    /// Whether the keyboard is listening.
    pub keyboard: bool,
    pub keyboard_name: String,
    /// What the open controller's mapping says about its own outputs, when it
    /// says anything. Shown so a DJ can see the arrangement being used rather
    /// than inferring it from which socket is quiet.
    pub audio: Option<AudioRoutingDto>,
}

/// A controller's own output arrangement, as the interface shows it.
///
/// Channel numbers are 1-based here because the sockets on the back of the
/// device are labelled 1-4, not 0-3, and a panel that disagreed with the
/// silkscreen would be worse than no panel.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AudioRoutingDto {
    pub master: (usize, usize),
    pub cue: Option<(usize, usize)>,
    pub booth: Option<(usize, usize)>,
    /// How many outputs the device must have for this arrangement.
    pub channels_needed: usize,
    /// Set when the arrangement cannot be used on the device that is open --
    /// the mapping names an output the device does not have -- in which case
    /// djmanzo falls back to guessing from the channel count and says so.
    pub not_applied: Option<String>,
}

/// Everything the controller layer owns.
pub struct ControlHub {
    /// Mappings that can be opened, bundled and user files together.
    mappings: Mutex<Vec<(Mapping, bool)>>,
    /// The keyboard mapping in force.
    keyboard: Mutex<KeyMap>,
    /// Whether the keyboard is listening at all. Off is a real setting: a DJ
    /// typing into the search box does not want space to start deck 1.
    keyboard_on: std::sync::atomic::AtomicBool,
    /// The open port. Dropping it closes the port.
    open: Mutex<Option<dj_hid::Connection>>,
    /// Where the open controller's mapping says its own sockets go.
    ///
    /// Kept beside the connection rather than inside it because the routing
    /// outlives every audio device: opening a device builds a fresh engine, so
    /// the arrangement has to be put back afterwards from somewhere that
    /// remembers it.
    audio: Mutex<Option<dj_hid::audio::AudioPreset>>,
    /// Where the MIDI thread posts translated actions, kept so a new
    /// connection reuses the drain that is already running.
    post: Sender<String>,
    /// What the mapping editor watches the port with.
    ///
    /// Held here rather than on the connection so that turning learning on
    /// before a controller is plugged in still works: the flag is already set
    /// when the port opens.
    listener: dj_hid::Listener,
}

impl std::fmt::Debug for ControlHub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ControlHub").finish_non_exhaustive()
    }
}

impl ControlHub {
    /// Build the hub from the bundled mappings, and hand back the receiver the
    /// drain thread will read.
    ///
    /// The receiver comes back rather than being consumed here because the
    /// drain needs an `AppHandle`, and there is no handle until `setup` runs.
    #[must_use]
    pub fn new() -> (Self, Receiver<String>) {
        let (post, take) = std::sync::mpsc::channel();
        let mappings = dj_hid::bundled::controllers()
            .unwrap_or_default()
            .into_iter()
            .map(|mapping| (mapping, true))
            .collect();
        // A broken bundled keyboard is a build error caught by a test in
        // `dj_hid::bundled`. If one somehow ships, an empty map means the
        // keyboard does nothing rather than the application refusing to start.
        let keyboard = dj_hid::bundled::keyboard().unwrap_or_default();
        (
            ControlHub {
                mappings: Mutex::new(mappings),
                keyboard: Mutex::new(keyboard),
                keyboard_on: std::sync::atomic::AtomicBool::new(true),
                open: Mutex::new(None),
                audio: Mutex::new(None),
                post,
                listener: dj_hid::Listener::default(),
            },
            take,
        )
    }

    /// Load every `.toml` in the user's mapping directory, replacing anything
    /// bundled under the same name.
    ///
    /// Returns what could not be read, so a file with a typo in it is reported
    /// rather than silently missing. One bad file does not stop the others.
    pub fn load_user_mappings(&self, dir: &std::path::Path) -> Vec<String> {
        let mut problems = Vec::new();
        let Ok(entries) = std::fs::read_dir(dir) else {
            return problems;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                continue;
            }
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            let text = match std::fs::read_to_string(&path) {
                Ok(text) => text,
                Err(e) => {
                    problems.push(format!("{name}: {e}"));
                    continue;
                }
            };
            // A keyboard file and a controller file are told apart by what is
            // in them, not by where they are: a DJ who names theirs
            // `my-layout.toml` should not have to learn a directory
            // convention to be understood.
            if text.contains("[[key]]") {
                match KeyMap::parse(&text) {
                    Ok(map) => *self.keyboard.lock().unwrap() = map,
                    Err(e) => problems.push(format!("{name}: {e}")),
                }
                continue;
            }
            match Mapping::parse(&text) {
                Ok(mapping) => {
                    let mut all = self.mappings.lock().unwrap();
                    // The user's own file replaces the bundled one of the same
                    // name rather than sitting alongside it, so editing a
                    // shipped mapping works the way editing a file should.
                    all.retain(|(existing, _)| existing.name != mapping.name);
                    all.push((mapping, false));
                }
                Err(e) => problems.push(format!("{name}: {e}")),
            }
        }
        problems
    }

    // -- the mapping editor ------------------------------------------------

    /// Start describing controls instead of acting on them.
    ///
    /// Learning suppresses the action a control already has, because learning
    /// the play button by pressing the play button would otherwise start the
    /// deck -- sixty times, over a mapping session.
    pub fn start_learning(&self) {
        self.listener.start();
    }

    pub fn stop_learning(&self) {
        self.listener.stop();
    }

    #[must_use]
    pub fn is_learning(&self) -> bool {
        self.listener.is_learning()
    }

    /// The last control touched since learning began.
    #[must_use]
    pub fn learned(&self) -> Option<String> {
        self.listener.seen()
    }

    /// Forget it, so the next press is unambiguous.
    pub fn forget_learned(&self) {
        self.listener.clear();
    }

    /// Write a mapping into the user's `mappings` directory and load it.
    ///
    /// Saved through the parser, not around it: what lands on disk is read
    /// back before it is accepted, so a mapping that could not be reopened is
    /// reported now rather than at the start of the next set.
    ///
    /// # Errors
    /// If the draft will not parse, or the file cannot be written.
    pub fn save_mapping(
        &self,
        dir: &std::path::Path,
        draft: &dj_hid::editor::Draft,
    ) -> Result<std::path::PathBuf, String> {
        let text = draft.to_toml().map_err(|e| e.to_string())?;
        // Prove it reloads before writing it, so a broken file never reaches
        // the directory the loader scans.
        dj_hid::Mapping::parse(&text).map_err(|e| e.to_string())?;

        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let path = dir.join(format!("{}.toml", file_stem(&draft.name)));
        std::fs::write(&path, &text).map_err(|e| e.to_string())?;

        self.load_user_mappings(dir);
        Ok(path)
    }

    /// The mapping called `name`, if there is one.
    ///
    /// For the editor, which starts most drafts from a mapping that already
    /// nearly fits rather than from nothing.
    #[must_use]
    pub fn mapping_named(&self, name: &str) -> Option<dj_hid::Mapping> {
        self.mappings
            .lock()
            .ok()?
            .iter()
            .find(|(mapping, _)| mapping.name == name)
            .map(|(mapping, _)| mapping.clone())
    }

    /// Every mapping that can be opened.
    #[must_use]
    pub fn mappings(&self) -> Vec<MappingDto> {
        self.mappings
            .lock()
            .unwrap()
            .iter()
            .map(|(mapping, bundled)| MappingDto {
                name: mapping.name.clone(),
                device: mapping.device.clone(),
                bindings: mapping.bindings.len(),
                bundled: *bundled,
            })
            .collect()
    }

    /// The keyboard, as a shortcut sheet.
    #[must_use]
    pub fn keys(&self) -> Vec<KeyDto> {
        let keyboard = self.keyboard.lock().unwrap();
        keyboard
            .chords()
            .iter()
            .zip(&keyboard.keys)
            .map(|(chord, key)| KeyDto {
                chord: chord.text(),
                label: key.label.clone(),
                group: key.group.clone(),
                held: key.release.is_some(),
                press: key.press.clone(),
                release: key.release.clone(),
            })
            .collect()
    }

    /// Whether the keyboard is listening.
    #[must_use]
    pub fn keyboard_on(&self) -> bool {
        self.keyboard_on.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Turn the keyboard on or off.
    pub fn set_keyboard(&self, on: bool) {
        self.keyboard_on
            .store(on, std::sync::atomic::Ordering::Relaxed);
    }

    /// Open `port` with the mapping called `mapping`, or with whichever
    /// bundled mapping fits the port when none is named.
    ///
    /// # Errors
    /// When no such mapping exists, or the port cannot be opened.
    pub fn open(&self, port: &str, mapping: Option<&str>) -> Result<(), String> {
        let chosen = {
            let all = self.mappings.lock().unwrap();
            let found = match mapping {
                Some(name) => all.iter().find(|(m, _)| m.name == name),
                None => all.iter().find(|(m, _)| m.fits(port)),
            };
            found
                .ok_or_else(|| match mapping {
                    Some(name) => format!("no mapping called {name:?}"),
                    None => format!("no mapping fits {port:?} — choose one"),
                })?
                .0
                .clone()
        };

        // Taken before the mapping is handed to the port, which consumes it.
        let preset = chosen.audio.clone();

        let open = dj_hid::port::open(port, chosen, self.post.clone(), self.listener.clone())
            .map_err(|e| e.to_string())?;
        // Assigned last, so a failed open leaves the previous connection alone
        // rather than closing it and connecting to nothing.
        *self.open.lock().unwrap() = Some(open);
        *self.audio.lock().unwrap() = preset;
        Ok(())
    }

    /// Close whatever is open. Closing nothing is not an error.
    pub fn close(&self) {
        *self.open.lock().unwrap() = None;
        // Cleared with the connection: a routing left behind would send the
        // laptop's built-in output to sockets that belonged to a controller
        // which is no longer plugged in.
        *self.audio.lock().unwrap() = None;
    }

    /// What the open controller says about its own outputs.
    ///
    /// `None` when nothing is open, or when the mapping says nothing -- which
    /// is the normal case, since most controllers put the master first and the
    /// guess is right for them.
    #[must_use]
    pub fn audio_preset(&self) -> Option<dj_hid::audio::AudioPreset> {
        self.audio.lock().ok()?.clone()
    }

    /// The open controller's arrangement as the engine wants it.
    ///
    /// A preset that does not validate comes back as `None` rather than as an
    /// error: it was already refused when the mapping was parsed, so reaching
    /// here means something opened a mapping that never loaded, and the safe
    /// answer is the guess.
    #[must_use]
    pub fn routing(&self) -> Option<dj_engine::BusRouting> {
        let routing = self.audio_preset()?.routing().ok()?;
        Some(dj_engine::BusRouting::new(
            routing.master,
            routing.cue,
            routing.booth,
        ))
    }

    /// What is plugged in and what is listening.
    #[must_use]
    pub fn status(&self, channels: Option<usize>) -> ControlStatus {
        let (inputs, unavailable) = match dj_hid::port::inputs() {
            Ok(found) => (found, None),
            Err(e) => (Vec::new(), Some(e.to_string())),
        };
        let open = self.open.lock().unwrap();
        let keyboard = self.keyboard.lock().unwrap();
        ControlStatus {
            inputs,
            open_port: open.as_ref().map(|c| c.port().to_owned()),
            open_mapping: open.as_ref().map(|c| c.mapping().to_owned()),
            unavailable,
            keyboard: self.keyboard_on(),
            keyboard_name: keyboard.name.clone(),
            audio: self.audio_routing_dto(channels),
        }
    }

    /// The open controller's arrangement, described for the interface.
    ///
    /// `channels` is what the open audio device actually provides, so a
    /// mapping that asks for more outputs than the device has is reported as
    /// not applied instead of appearing to be in force.
    fn audio_routing_dto(&self, channels: Option<usize>) -> Option<AudioRoutingDto> {
        let routing = self.audio_preset()?.routing().ok()?;
        let not_applied = match channels {
            Some(available) if routing.channels_needed > available => Some(format!(
                "this mapping needs {} outputs and the open device has {available}; \
                 djmanzo is using the usual arrangement instead",
                routing.channels_needed
            )),
            _ => None,
        };
        Some(AudioRoutingDto {
            master: human(routing.master),
            cue: routing.cue.map(human),
            booth: routing.booth.map(human),
            channels_needed: routing.channels_needed,
            not_applied,
        })
    }
}

/// Start the thread that turns queued action text into actions.
///
/// One thread for every controller, because the actions have to arrive in the
/// order they were played: a censor-on that overtook its own censor-off would
/// leave the deck muted.
pub fn drain(handle: tauri::AppHandle, take: Receiver<String>) {
    std::thread::Builder::new()
        .name("djmanzo-control".into())
        .spawn(move || {
            use tauri::Manager;
            // `recv` blocks until the sender is dropped, which happens when the
            // application shuts down — so the loop ends by itself rather than
            // needing to be told to.
            while let Ok(action) = take.recv() {
                let state = handle.state::<crate::state::AppState>();
                if let Err(why) = crate::commands::perform(&state, &action) {
                    // Not fatal and not silent: a mapping bound to something
                    // the engine will not take yet — a deck action with no
                    // device open — is worth one line, not a dialog.
                    eprintln!("controller: {why}");
                }
            }
        })
        .expect("the control thread should start");
}

/// Hold the sender alive for as long as the application runs.
///
/// Without this the drain thread would end the moment the last connection
/// closed, and the next one opened would post into a channel nobody reads.
pub type Post = Arc<Sender<String>>;

/// A zero-based channel pair as the sockets are labelled on the device.
///
/// The engine counts from zero because that is how the buffer is indexed; the
/// back of a controller counts from one because that is how it is printed. The
/// translation happens once, here, at the edge where a number stops being an
/// index and starts being something a person reads.
fn human(pair: (usize, usize)) -> (usize, usize) {
    (pair.0 + 1, pair.1 + 1)
}

/// A mapping name as a file name.
///
/// A name reaches the filesystem, so anything that is not plainly a name
/// becomes a dash: a mapping called `../../autostart` must not write outside
/// the directory it was given.
fn file_stem(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let trimmed = cleaned.trim_matches('-');
    if trimmed.is_empty() {
        "mapping".to_owned()
    } else {
        trimmed.to_owned()
    }
}

#[cfg(test)]
mod tests {

    // -- the mapping editor -------------------------------------------------

    fn draft() -> dj_hid::editor::Draft {
        let mut draft = dj_hid::editor::Draft::new("My Controller", "Some Device");
        draft
            .bind(
                "note 1 0x0b",
                &dj_hid::editor::Role::Latching {
                    press: "deck 1 play_pause".to_owned(),
                },
            )
            .expect("a normal binding");
        draft
    }

    #[test]
    fn learning_is_off_until_it_is_asked_for() {
        let (hub, _take) = ControlHub::new();
        assert!(!hub.is_learning());
        assert_eq!(hub.learned(), None);
    }

    #[test]
    fn learning_can_be_started_and_stopped() {
        let (hub, _take) = ControlHub::new();
        hub.start_learning();
        assert!(hub.is_learning());
        hub.stop_learning();
        assert!(!hub.is_learning());
    }

    /// Starting again forgets the last control: a DJ who brushed the wrong pad
    /// and pressed the button again must not be shown the pad they brushed.
    #[test]
    fn starting_to_learn_forgets_the_previous_control() {
        let (hub, _take) = ControlHub::new();
        hub.start_learning();
        assert_eq!(hub.learned(), None);
        hub.forget_learned();
        assert_eq!(hub.learned(), None);
    }

    /// **What the editor is for.** A saved mapping has to appear in the list
    /// of mappings that can be opened, or the DJ's work went nowhere.
    #[test]
    fn a_saved_mapping_can_be_opened_afterwards() {
        let dir = tempfile::tempdir().unwrap();
        let (hub, _take) = ControlHub::new();
        let before = hub.mappings().len();

        let path = hub.save_mapping(dir.path(), &draft()).expect("saving");
        assert!(path.exists(), "nothing was written");

        let names: Vec<String> = hub.mappings().into_iter().map(|m| m.name).collect();
        assert!(
            names.iter().any(|n| n == "My Controller"),
            "the saved mapping is not in {names:?}"
        );
        assert_eq!(hub.mappings().len(), before + 1);
    }

    /// The file on disk is a mapping file like any other -- editable by hand,
    /// and readable by the loader that reads the bundled ones.
    #[test]
    fn what_is_saved_is_an_ordinary_mapping_file() {
        let dir = tempfile::tempdir().unwrap();
        let (hub, _take) = ControlHub::new();
        let path = hub.save_mapping(dir.path(), &draft()).unwrap();

        let text = std::fs::read_to_string(&path).unwrap();
        let reloaded = dj_hid::Mapping::parse(&text).expect("the saved file parses");
        assert_eq!(reloaded.name, "My Controller");
        assert_eq!(reloaded.bindings.len(), 1);
    }

    /// A mapping name reaches the filesystem. One that tries to climb out of
    /// the directory must not.
    #[test]
    fn a_mapping_name_cannot_escape_the_mappings_directory() {
        let dir = tempfile::tempdir().unwrap();
        let (hub, _take) = ControlHub::new();

        let mut escaping = draft();
        escaping.name = "../../autostart".to_owned();
        let path = hub.save_mapping(dir.path(), &escaping).expect("saving");

        assert_eq!(
            path.parent(),
            Some(dir.path()),
            "{path:?} was written outside the mappings directory"
        );
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        assert!(!name.contains(".."), "{name} still has a traversal in it");
    }

    /// A name with nothing usable in it still has to produce a file, rather
    /// than a dotfile or an empty name.
    #[test]
    fn a_nameless_mapping_still_gets_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let (hub, _take) = ControlHub::new();

        let mut nameless = draft();
        nameless.name = "///".to_owned();
        let path = hub.save_mapping(dir.path(), &nameless).expect("saving");
        assert_eq!(path.file_name().unwrap(), "mapping.toml");
    }

    /// Saving the same mapping twice replaces it rather than piling up
    /// `My Controller (2)` files nobody asked for.
    #[test]
    fn saving_twice_replaces_rather_than_accumulates() {
        let dir = tempfile::tempdir().unwrap();
        let (hub, _take) = ControlHub::new();

        hub.save_mapping(dir.path(), &draft()).unwrap();
        hub.save_mapping(dir.path(), &draft()).unwrap();

        let files: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
        assert_eq!(files.len(), 1, "saving twice left {} files", files.len());
    }
    use super::*;

    #[test]
    fn a_new_hub_has_the_bundled_mappings_and_a_keyboard() {
        let (hub, _take) = ControlHub::new();
        assert!(!hub.mappings().is_empty(), "no bundled controller mappings");
        assert!(hub.mappings().iter().all(|m| m.bundled));
        assert!(hub.keys().len() > 40, "only {} keys", hub.keys().len());
        assert!(hub.keyboard_on(), "the keyboard should start listening");
    }

    /// A DJ typing in the search box does not want the space bar to start
    /// deck 1, so the keyboard has an off switch and it has to actually work.
    #[test]
    fn the_keyboard_can_be_switched_off_and_on() {
        let (hub, _take) = ControlHub::new();
        hub.set_keyboard(false);
        assert!(!hub.keyboard_on());
        assert!(!hub.status(None).keyboard);
        hub.set_keyboard(true);
        assert!(hub.keyboard_on());
    }

    /// Held keys are marked as such for the sheet, because a label that says
    /// "(hold)" and a control that latches are two different instruments.
    #[test]
    fn the_sheet_says_which_keys_are_held() {
        let (hub, _take) = ControlHub::new();
        let keys = hub.keys();
        let censor = keys
            .iter()
            .find(|k| k.press.as_deref() == Some("deck 1 censor_on"))
            .expect("the bundled keyboard should have a censor");
        assert!(censor.held);
        assert_eq!(censor.release.as_deref(), Some("deck 1 censor_off"));

        let play = keys
            .iter()
            .find(|k| k.chord == "space")
            .expect("the bundled keyboard should have a space bar");
        assert!(!play.held);
    }

    /// Naming a mapping that does not exist has to say so. Falling back to
    /// "whatever fits" would connect a DJ's controller to somebody else's
    /// layout and look like it worked.
    #[test]
    fn opening_with_an_unknown_mapping_says_which_one_was_missing() {
        let (hub, _take) = ControlHub::new();
        let why = hub.open("anything", Some("Not A Mapping")).unwrap_err();
        assert!(why.contains("Not A Mapping"), "{why}");
    }

    #[test]
    fn a_user_mapping_replaces_the_bundled_one_of_the_same_name() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("mine.toml"),
            r#"
            name = "Generic 2-deck"
            device = "Mine"
            [[binding]]
            on = "note 1 36"
            press = "deck 1 play_pause"
            "#,
        )
        .unwrap();

        let (hub, _take) = ControlHub::new();
        let before = hub.mappings().len();
        assert!(hub.load_user_mappings(dir.path()).is_empty());
        let after = hub.mappings();
        assert_eq!(after.len(), before, "it should replace, not add");
        let replaced = after
            .iter()
            .find(|m| m.name == "Generic 2-deck")
            .expect("still there");
        assert!(!replaced.bundled);
        assert_eq!(replaced.device, "Mine");
    }

    /// A keyboard file is recognised by what is in it, so a DJ can call theirs
    /// anything.
    #[test]
    fn a_user_keyboard_file_is_recognised_by_its_contents() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("whatever-i-called-it.toml"),
            r#"
            name = "Mine"
            [[key]]
            on = "Space"
            press = "deck 3 play_pause"
            label = "Play"
            group = "Deck 3"
            "#,
        )
        .unwrap();

        let (hub, _take) = ControlHub::new();
        assert!(hub.load_user_mappings(dir.path()).is_empty());
        assert_eq!(hub.status(None).keyboard_name, "Mine");
        assert_eq!(hub.keys().len(), 1);
    }

    /// One bad file must not take the others down with it, and must be named.
    /// A mapping directory is hand-edited; a typo in one file is normal.
    #[test]
    fn a_broken_file_is_reported_and_the_others_still_load() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("broken.toml"),
            r#"
            name = "Broken"
            [[binding]]
            on = "note 1 36"
            press = "deck 1 fly_to_the_moon"
            "#,
        )
        .unwrap();
        std::fs::write(
            dir.path().join("fine.toml"),
            r#"
            name = "Fine"
            device = "Fine"
            [[binding]]
            on = "note 1 36"
            press = "deck 1 play_pause"
            "#,
        )
        .unwrap();

        let (hub, _take) = ControlHub::new();
        let problems = hub.load_user_mappings(dir.path());
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].contains("broken.toml"), "{problems:?}");
        assert!(hub.mappings().iter().any(|m| m.name == "Fine"));
    }

    /// A mapping directory that is not there is the normal case on a fresh
    /// install, not an error.
    #[test]
    fn a_missing_mapping_directory_is_quiet() {
        let (hub, _take) = ControlHub::new();
        assert!(
            hub.load_user_mappings(std::path::Path::new("/nowhere/at/all"))
                .is_empty()
        );
    }
}
