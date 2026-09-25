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
    /// §53: what this mapping actually puts under the hands.
    ///
    /// On every mapping rather than only the open one, so the picker can say
    /// what each would give you *before* it is opened — which is the question a
    /// DJ with two controllers in a bag is actually asking.
    pub hands: dj_hid::hands::Hands,
    /// How many controls this mapping lights.
    ///
    /// §53 asks which surfaces a controller deserves prominence for, and the
    /// mirror of that question is what the controller itself can show. Zero is
    /// a real answer and a common one — plenty of mappings bind a hundred
    /// controls and declare no feedback at all — and it is different from a
    /// mapping djmanzo simply has not opened.
    pub lights: usize,
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
    /// Every HID device the machine can see. Listed separately from the MIDI
    /// ports because they are opened differently -- a HID mapping must be
    /// chosen deliberately, never matched by "whichever fits".
    pub hid_inputs: Vec<HidDeviceDto>,
    /// Why there are no HID devices, when the reason is that HID itself is
    /// unreachable. On Linux that is usually permissions on the device node
    /// rather than an empty USB bus.
    pub hid_unavailable: Option<String>,
    pub open_hid: Option<String>,
    pub open_hid_mapping: Option<String>,
}

/// A HID device, as the interface lists it.
#[derive(Debug, Clone, serde::Serialize)]
pub struct HidDeviceDto {
    /// `2b73:0017`, the way `lsusb` and every manual write it. Shown because
    /// two controllers from one maker often report the same name.
    pub id: String,
    pub name: String,
    /// What `open_hid_controller` should be given: unique even when two
    /// identical controllers are plugged in.
    pub path: String,
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

/// One mapping djmanzo can open, and everything read out of its file.
///
/// All three parsed in one pass at load, because all three live in the same
/// TOML and a `Mapping` carries neither of the other two. Reading the file
/// again later to get the lights would mean holding the text as a fourth thing
/// to keep in step — which is the failure this codebase keeps finding from the
/// other side.
struct Known {
    mapping: Mapping,
    /// §53's reading of what this controller puts under the hands.
    hands: dj_hid::hands::Hands,
    /// The lights it declares. Empty for a mapping with no `[[feedback]]`.
    lights: dj_hid::feedback::FeedbackMap,
    /// Whether djmanzo shipped it, as opposed to the DJ writing it.
    bundled: bool,
}

/// What djmanzo is lighting on the open controller, or why it is not.
#[derive(Debug, Clone, serde::Serialize)]
pub struct LightsDto {
    /// How many controls are being driven. Zero when nothing is.
    pub lit: usize,
    /// The MIDI output they are going to, by name. Empty when none is open.
    pub port: String,
    /// Why a mapping's lights are not running, when it has some. Empty when
    /// there is nothing to explain — including when they are running.
    pub unlit: String,
}

/// Everything the controller layer owns.
pub struct ControlHub {
    /// Mappings that can be opened, bundled and user files together.
    /// Every mapping, with §53's reading of what it reaches and whether it
    /// shipped. The profile is derived once at load, from the same text: a
    /// `Mapping` does not carry its `[[feedback]]` blocks, so deriving it later
    /// would mean holding the text as a third thing to keep in step.
    mappings: Mutex<Vec<Known>>,
    /// The keyboard mapping in force.
    keyboard: Mutex<KeyMap>,
    /// Whether the keyboard is listening at all. Off is a real setting: a DJ
    /// typing into the search box does not want space to start deck 1.
    keyboard_on: std::sync::atomic::AtomicBool,
    /// The open port. Dropping it closes the port.
    open: Mutex<Option<dj_hid::Connection>>,
    /// The open controller's lights, being driven. Dropping it blacks out the
    /// board and joins its thread.
    ///
    /// §53 asks for a controller-aware interface, and this is the other
    /// direction of the same question: `FeedbackMap` was parsed, validated and
    /// consulted by nothing, so a mapping could declare sixty lights and the
    /// hardware would sit dark disagreeing with the screen all night.
    lights: Mutex<Option<dj_hid::feedback::Lights>>,
    /// Why the lights are not running, when a mapping has some and they are
    /// not. Empty when there is nothing to explain.
    unlit: Mutex<Option<String>>,
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
    /// The open HID device, if the controller speaks HID rather than MIDI.
    ///
    /// Separate from `open` rather than an enum because a DJ can genuinely
    /// have both: a MIDI mixer and a HID jog deck on the same table is a real
    /// setup, and refusing the second one to keep this field simple would be
    /// the software's convenience beating the DJ's.
    open_hid: Mutex<Option<dj_hid::usb::Connection>>,
    /// What the editor watches a HID device with.
    ///
    /// Its own listener because learning from HID needs the previous report to
    /// compare against -- a MIDI message names its own control and a HID
    /// packet does not.
    hid_listener: dj_hid::usb::Listener,
    /// Read by a mapping's script, when it has one.
    ///
    /// Held here rather than passed in per call because a controller is opened
    /// from the interface, and the interface has no business knowing that
    /// scripts read parameters.
    registry: Option<Arc<dj_control::ParameterRegistry>>,
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
        let mappings = dj_hid::bundled::controllers_read()
            .unwrap_or_default()
            .into_iter()
            .map(|(mapping, hands, lights)| Known {
                mapping,
                hands,
                lights,
                bundled: true,
            })
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
                lights: Mutex::new(None),
                unlit: Mutex::new(None),
                open_hid: Mutex::new(None),
                registry: None,
                hid_listener: dj_hid::usb::Listener::default(),
                audio: Mutex::new(None),
                post,
                listener: dj_hid::Listener::default(),
            },
            take,
        )
    }

    /// Give the hub the registry a mapping's script reads.
    ///
    /// Set once at startup rather than taken in `new`, because the hub is
    /// built before the registry exists — the same reason its action receiver
    /// comes back rather than being consumed.
    pub fn set_registry(&mut self, registry: Arc<dj_control::ParameterRegistry>) {
        self.registry = Some(registry);
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
            match (
                Mapping::parse(&text),
                dj_hid::hands::Hands::read(&text),
                dj_hid::feedback::FeedbackMap::parse(&text),
            ) {
                (Ok(mapping), Ok(hands), Ok(lights)) => {
                    let mut all = self.mappings.lock().unwrap();
                    // The user's own file replaces the bundled one of the same
                    // name rather than sitting alongside it, so editing a
                    // shipped mapping works the way editing a file should.
                    all.retain(|known| known.mapping.name != mapping.name);
                    all.push(Known {
                        mapping,
                        hands,
                        lights,
                        bundled: false,
                    });
                }
                // A file whose lights do not parse is refused whole. The
                // alternative — load the bindings and silently drop the
                // feedback — is a controller that works and never lights up,
                // with nothing anywhere saying why.
                (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => {
                    problems.push(format!("{name}: {e}"));
                }
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
    ///
    /// Both transports listen, so a DJ pressing a pad does not first have to
    /// know whether their controller speaks MIDI or HID -- which is a question
    /// about a USB descriptor, not about music.
    pub fn start_learning(&self) {
        self.listener.start();
        self.hid_listener.start();
    }

    pub fn stop_learning(&self) {
        self.listener.stop();
        self.hid_listener.stop();
    }

    #[must_use]
    pub fn is_learning(&self) -> bool {
        self.listener.is_learning()
    }

    /// The last control touched since learning began.
    ///
    /// MIDI first when both have something, because a MIDI message names its
    /// own control while a HID field is inferred from a difference -- so the
    /// MIDI answer is the more certain of the two.
    #[must_use]
    pub fn learned(&self) -> Option<String> {
        self.listener.seen().or_else(|| self.hid_listener.seen())
    }

    /// Forget it, so the next press is unambiguous.
    pub fn forget_learned(&self) {
        self.listener.clear();
        self.hid_listener.clear();
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
            .find(|known| known.mapping.name == name)
            .map(|known| known.mapping.clone())
    }

    /// Every mapping that can be opened.
    #[must_use]
    pub fn mappings(&self) -> Vec<MappingDto> {
        self.mappings
            .lock()
            .unwrap()
            .iter()
            .map(|known| MappingDto {
                name: known.mapping.name.clone(),
                device: known.mapping.device.clone(),
                bindings: known.mapping.bindings.len(),
                bundled: known.bundled,
                hands: known.hands,
                lights: known.lights.lights.len(),
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
                Some(name) => all.iter().find(|known| known.mapping.name == name),
                None => all.iter().find(|known| known.mapping.fits(port)),
            };
            let known = found.ok_or_else(|| match mapping {
                Some(name) => format!("no mapping called {name:?}"),
                None => format!("no mapping fits {port:?} — choose one"),
            })?;
            (known.mapping.clone(), known.lights.clone())
        };
        let (chosen, lights) = chosen;

        // Taken before the mapping is handed to the port, which consumes it.
        let preset = chosen.audio.clone();

        // The registry goes with it, so a script can ask what the engine is
        // doing -- "is the deck playing" is the difference between a mapping
        // that decides and a table with extra syntax.
        let open = dj_hid::port::open_with(
            port,
            chosen,
            self.post.clone(),
            self.listener.clone(),
            self.registry.clone(),
        )
        .map_err(|e| e.to_string())?;
        // Assigned last, so a failed open leaves the previous connection alone
        // rather than closing it and connecting to nothing.
        *self.open.lock().unwrap() = Some(open);
        *self.audio.lock().unwrap() = preset;
        self.light(port, lights);
        Ok(())
    }

    /// Start driving a mapping's lights, if it has any and there is anywhere
    /// to send them.
    ///
    /// **Failure here is not failure to open the controller.** A DJ whose
    /// device has no MIDI output, or whose output is already claimed by
    /// another application, still has a working controller — it simply does
    /// not light up — and refusing the open would be djmanzo deciding that a
    /// dark board is worse than no board. The reason is kept so
    /// [`Self::lights`] can say it rather than the panel guessing.
    ///
    /// The output is looked up by the **input** port's name. That is right far
    /// more often than not: a controller's two ports are one device and the
    /// operating system names them from the same string, and `out::open`
    /// matches loosely for the platforms where it decorates them differently.
    fn light(&self, port: &str, map: dj_hid::feedback::FeedbackMap) {
        // Replaced rather than added to, and the old pump's `Drop` blacks out
        // the board it was lighting before the new one starts.
        *self.lights.lock().unwrap() = None;
        *self.unlit.lock().unwrap() = None;

        if map.is_empty() {
            // Not a failure and not worth a message: plenty of mappings bind a
            // hundred controls and declare no feedback at all.
            return;
        }
        let Some(registry) = self.registry.clone() else {
            *self.unlit.lock().unwrap() =
                Some("djmanzo has no parameters to light them from yet".to_owned());
            return;
        };
        match dj_hid::out::open(port) {
            Ok(sink) => {
                let name = sink.name().to_owned();
                *self.lights.lock().unwrap() = Some(dj_hid::feedback::Lights::start(
                    map,
                    Box::new(sink),
                    registry,
                    name,
                ));
            }
            Err(e) => *self.unlit.lock().unwrap() = Some(e.to_string()),
        }
    }

    /// What djmanzo is lighting, or why it is not.
    ///
    /// Both halves, because §53's panel has to tell a DJ whose board stays
    /// dark *why*: a mapping with no feedback blocks, a device with no output,
    /// and an output another application already holds are three different
    /// problems with three different answers, and "no lights" is the same
    /// picture for all of them.
    #[must_use]
    pub fn lights(&self) -> LightsDto {
        let lit = self.lights.lock().unwrap();
        LightsDto {
            lit: lit.as_ref().map_or(0, dj_hid::feedback::Lights::lit),
            port: lit
                .as_ref()
                .map(|lights| lights.port().to_owned())
                .unwrap_or_default(),
            unlit: self.unlit.lock().unwrap().clone().unwrap_or_default(),
        }
    }

    /// Open a HID device with the mapping called `mapping`.
    ///
    /// Unlike the MIDI path there is no "whichever fits": a HID mapping states
    /// byte offsets into a report, and applying one device's offsets to
    /// another's packets would not fail -- it would bind the crossfader to a
    /// button. A mapping has to be chosen deliberately.
    ///
    /// # Errors
    /// When no such mapping exists, when it is not a HID mapping at all, or
    /// when the device cannot be opened.
    pub fn open_hid(&self, device: &str, mapping: &str) -> Result<(), String> {
        let chosen = {
            let all = self.mappings.lock().unwrap();
            all.iter()
                .find(|known| known.mapping.name == mapping)
                .ok_or_else(|| format!("no mapping called {mapping:?}"))?
                .mapping
                .clone()
        };
        if chosen.hid_fields().is_empty() {
            return Err(format!(
                "{mapping:?} has no HID bindings, so it cannot read a HID device — \
                 it is a MIDI mapping"
            ));
        }

        let preset = chosen.audio.clone();
        let open = dj_hid::usb::open(device, chosen, self.post.clone(), self.hid_listener.clone())
            .map_err(|e| e.to_string())?;
        // Assigned last, so a failed open leaves the previous device alone.
        *self.open_hid.lock().unwrap() = Some(open);
        if preset.is_some() {
            *self.audio.lock().unwrap() = preset;
        }
        Ok(())
    }

    /// Close the HID device. Closing nothing is not an error.
    pub fn close_hid(&self) {
        *self.open_hid.lock().unwrap() = None;
        // The routing only goes with it when no MIDI controller is still
        // holding one -- unplugging the jog deck should not unroute the mixer.
        if self.open.lock().unwrap().is_none() {
            *self.audio.lock().unwrap() = None;
        }
    }

    /// Every HID device the machine can see, and why it cannot see any.
    #[must_use]
    pub fn hid_devices(&self) -> (Vec<dj_hid::usb::DeviceInfo>, Option<String>) {
        match dj_hid::usb::devices() {
            Ok(found) => (found, None),
            Err(e) => (Vec::new(), Some(e.to_string())),
        }
    }

    /// Close whatever is open. Closing nothing is not an error.
    pub fn close(&self) {
        *self.open.lock().unwrap() = None;
        // Dropped with the connection, which blacks the board out and joins
        // the pump. A controller keeps its LEDs after the thing that set them
        // has let go — they are the device's state, not djmanzo's — so a DJ
        // unplugging mid-set would otherwise be left with a board still
        // showing a deck that is no longer playing.
        *self.lights.lock().unwrap() = None;
        *self.unlit.lock().unwrap() = None;
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
        let open_hid = self.open_hid.lock().unwrap();
        let (found, hid_unavailable) = self.hid_devices();
        let hid_inputs = found
            .into_iter()
            .map(|device| HidDeviceDto {
                id: device.id(),
                name: device.name,
                path: device.path,
            })
            .collect();
        let keyboard = self.keyboard.lock().unwrap();
        ControlStatus {
            inputs,
            open_port: open.as_ref().map(|c| c.port().to_owned()),
            open_mapping: open.as_ref().map(|c| c.mapping().to_owned()),
            unavailable,
            keyboard: self.keyboard_on(),
            keyboard_name: keyboard.name.clone(),
            audio: self.audio_routing_dto(channels),
            hid_inputs,
            hid_unavailable,
            open_hid: open_hid.as_ref().map(|c| c.device().to_owned()),
            open_hid_mapping: open_hid.as_ref().map(|c| c.mapping().to_owned()),
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
                // §109: a line for the interface -- an activity, a panel --
                // is checked against the Space tree and handed to the window,
                // which runs it exactly as the key would. In the same order
                // as everything else, so a button that switches activity and
                // then starts a deck does both, in that order.
                if let Some(checked) = crate::commands::interface_run(&state, &action) {
                    match checked {
                        Ok(run) => {
                            use tauri::Emitter;
                            if let Err(why) = handle.emit("leaf", &run) {
                                eprintln!("controller: {why}");
                            }
                        }
                        Err(why) => eprintln!("controller: {why}"),
                    }
                    continue;
                }
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

    /// **§109: a controller reaches what a key under Space reaches, and
    /// nothing else.** An interface line naming an activity, a panel or one
    /// of the interface's verbs is handed on as the tree would run it; one
    /// naming an activity or a panel djmanzo does not have is refused, and an
    /// engine action is not an interface line at all.
    #[test]
    fn a_controller_reaches_what_a_key_under_space_reaches() {
        let state = crate::state::AppState::new(true);
        let run = |line: &str| crate::commands::interface_run(&state, line);
        assert_eq!(
            run("interface switch activity karaoke"),
            Some(Ok("switch activity karaoke".to_owned()))
        );
        assert_eq!(run("interface ui back"), Some(Ok("ui back".to_owned())));
        assert_eq!(
            run("interface surface library"),
            Some(Ok("surface library".to_owned()))
        );
        for refused in [
            "interface switch activity séance",
            "interface surface hologram",
            "interface ui self_destruct",
        ] {
            assert!(matches!(run(refused), Some(Err(_))), "{refused}");
        }
        assert_eq!(run("deck 1 play_pause"), None);
    }

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
            .find(|k| k.chord == "keya")
            .expect("the bundled keyboard should play deck 1 on A");
        assert_eq!(play.press.as_deref(), Some("deck 1 play_pause"));
        assert!(!play.held);
        // Space is the guide's (§117), never a key the map sends to a deck.
        assert!(
            keys.iter().all(|k| k.chord != "space"),
            "Space is bound in the map"
        );
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

    /// **The lights a mapping declares reach the thing that would send them.**
    ///
    /// The guard for the failure this whole change is about: `FeedbackMap`
    /// parsed every `[[feedback]]` block and resolved every parameter name in
    /// it, and the count never left `dj_hid` — so a mapping could declare
    /// sixty lights and every layer above would be looking at a zero. A
    /// mapping that lights nothing and a mapping whose lights nothing reads
    /// are the same picture from here, which is why this counts rather than
    /// asserting a boolean.
    #[test]
    fn a_bundled_mapping_that_declares_lights_is_seen_to_declare_them() {
        let (hub, _take) = ControlHub::new();
        let lit: usize = hub.mappings().iter().map(|m| m.lights).sum();
        assert!(
            lit > 0,
            "no bundled mapping reaches this layer with a light on it"
        );
        // And nothing is lit until a controller is open, which is the honest
        // answer on a machine with no MIDI — this container included.
        let now = hub.lights();
        assert_eq!(now.lit, 0);
        assert!(now.port.is_empty());
    }

    /// **A file whose lights do not parse is refused whole.**
    ///
    /// The alternative is worse than a refusal: load the bindings, drop the
    /// feedback, and hand a DJ a controller that works perfectly and never
    /// lights up, with nothing anywhere saying why. The binding in this file
    /// is valid, so the only thing that can refuse it is the light.
    #[test]
    fn a_mapping_whose_light_names_nothing_is_refused_and_named() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("dark.toml"),
            r#"
            name = "Dark"
            device = "Dark"
            [[binding]]
            on = "note 1 36"
            press = "deck 1 play_pause"
            [[feedback]]
            when = "deck.1.glowing"
            send = "note 1 36"
            "#,
        )
        .unwrap();

        let (hub, _take) = ControlHub::new();
        let problems = hub.load_user_mappings(dir.path());
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].contains("dark.toml"), "{problems:?}");
        assert!(
            !hub.mappings().iter().any(|m| m.name == "Dark"),
            "a mapping with a broken light was loaded without it"
        );
    }

    /// A mapping with no `[[feedback]]` at all is not a broken one.
    ///
    /// Most mappings are this: a hundred bindings and no lights. Refusing them
    /// would refuse nearly every mapping a DJ has ever written.
    #[test]
    fn a_mapping_with_no_lights_loads_like_any_other() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("plain.toml"),
            r#"
            name = "Plain"
            device = "Plain"
            [[binding]]
            on = "note 1 36"
            press = "deck 1 play_pause"
            "#,
        )
        .unwrap();

        let (hub, _take) = ControlHub::new();
        assert!(hub.load_user_mappings(dir.path()).is_empty());
        let found = hub
            .mappings()
            .into_iter()
            .find(|m| m.name == "Plain")
            .expect("a mapping with no lights still loads");
        assert_eq!(found.lights, 0);
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
