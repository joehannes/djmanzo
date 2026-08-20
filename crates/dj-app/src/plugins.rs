//! The plugin insert, on the application's side of the line.
//!
//! `dj_clap` knows how to load a plugin; this knows *when*. It holds the
//! instance — the half that may not leave the main thread — and it owns the
//! handover: a processor is created here, sent to the engine, and has to come
//! home before the instance can deactivate it.
//!
//! # Why there is a thread here
//!
//! A CLAP plugin instance is `!Send` — deliberately, because the specification
//! says it belongs to one thread and `clack` holds us to it. Tauri's
//! application state is shared across threads, so the instance cannot live in
//! it. It lives on a thread of its own instead, and everything else talks to it
//! by message, exactly as the audio host does.
//!
//! What the rest of the application sees is [`PluginHandle`]: a sender and a
//! snapshot of plain data. Nothing that crosses that line is `!Send`.
//!
//! # A plugin cannot yet ask for a rescan
//!
//! CLAP lets a plugin tell its host that its parameters have changed — after
//! loading a preset inside its own window, say. `dj_clap::host::Requests`
//! records that request, and nothing acts on it: the parameter list is read
//! once at load and again on nothing. It matters for a plugin with its own
//! preset browser, and not at all for one a DJ only drives from here. Wiring it
//! needs somewhere periodic to check from, which this thread does not have.
//!
//! # Why unloading is two steps
//!
//! The engine cannot give a plugin back synchronously; it hands things over
//! through the retirement queue, on its own schedule. So `clap clear` bypasses
//! the plugin in the engine, the engine is told to release it, and the
//! processor arrives here some blocks later to be deactivated. Between those
//! two moments the plugin is loaded, silent, and on its way out — which is
//! exactly what the interface shows.

use dj_clap::{Bundle, ClapError, Loaded, ParamInfo, Processor};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Sender, SyncSender, sync_channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The plugin insert, as anything outside this module sees it.
///
/// Plain data, kept in step by the plugin thread. Everything here is a copy —
/// nothing in it borrows the instance, which is the point.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PluginView {
    pub loaded: bool,
    pub name: String,
    pub vendor: String,
    /// Where it came from, so a set can be reopened.
    pub path: String,
    pub params: Vec<ParamInfo>,
}

/// What the plugin thread can be asked to do.
enum PluginCommand {
    Load {
        path: PathBuf,
        id: Option<String>,
        sample_rate: f64,
        max_frames: u32,
        reply: SyncSender<Result<Processor, ClapError>>,
    },
    /// A processor the engine has finished with, on its way to being
    /// deactivated. See the module note on why unloading is two steps.
    Retire(Box<Processor>),
    /// Let go of the instance. Sent after its processor has come home.
    Unload,
    /// Note a parameter's new value. The change itself went to the engine.
    Note {
        id: u32,
        value: f64,
    },
    Shutdown,
}

/// The handle the application holds.
#[derive(Debug, Clone)]
pub struct PluginHandle {
    commands: Sender<PluginCommand>,
    view: Arc<Mutex<PluginView>>,
}

/// How long to wait for the plugin thread to answer a load.
///
/// Generous: loading a plugin reads a dynamic library off a disk and runs its
/// initialiser, and a large synthesiser can take a second or two. Bounded all
/// the same, because a plugin that hangs must not take the interface with it.
const LOAD_TIMEOUT: Duration = Duration::from_secs(30);

impl Default for PluginHandle {
    fn default() -> Self {
        PluginHandle::start()
    }
}

impl PluginHandle {
    /// Start the plugin thread.
    #[must_use]
    pub fn start() -> PluginHandle {
        let (tx, rx) = std::sync::mpsc::channel();
        let view = Arc::new(Mutex::new(PluginView::default()));
        let theirs = Arc::clone(&view);
        // Detached: the thread ends when the sender is dropped, which happens
        // when the application state does.
        std::thread::Builder::new()
            .name("djmanzo-plugin".to_owned())
            .spawn(move || {
                let mut insert = Insert::default();
                while let Ok(command) = rx.recv() {
                    if matches!(command, PluginCommand::Shutdown) {
                        break;
                    }
                    insert.handle(command);
                    if let Ok(mut view) = theirs.lock() {
                        *view = insert.view();
                    }
                }
            })
            .expect("cannot start the plugin thread");
        PluginHandle { commands: tx, view }
    }

    /// What is loaded, if anything.
    #[must_use]
    pub fn view(&self) -> PluginView {
        self.view.lock().map(|v| v.clone()).unwrap_or_default()
    }

    /// Load a plugin and activate it, giving back the half for the engine.
    ///
    /// # Errors
    /// When the bundle will not open, contains no plugin with that id, the
    /// plugin refuses the configuration, or the plugin thread has gone.
    pub fn load(
        &self,
        path: &Path,
        id: Option<&str>,
        sample_rate: f64,
        max_frames: u32,
    ) -> Result<Processor, ClapError> {
        let (reply, answer) = sync_channel(1);
        self.commands
            .send(PluginCommand::Load {
                path: path.to_path_buf(),
                id: id.map(ToOwned::to_owned),
                sample_rate,
                max_frames,
                reply,
            })
            .map_err(|_| ClapError::Host("the plugin thread has gone".to_owned()))?;
        answer
            .recv_timeout(LOAD_TIMEOUT)
            .map_err(|_| ClapError::Host("the plugin did not finish loading".to_owned()))?
    }

    /// Hand a processor back from the engine to be deactivated.
    pub fn retire(&self, processor: Box<Processor>) {
        let _ = self.commands.send(PluginCommand::Retire(processor));
    }

    /// Let go of the instance, once its processor has come home.
    pub fn unload(&self) {
        let _ = self.commands.send(PluginCommand::Unload);
    }

    /// Note a parameter's new value, so the slider stays where the DJ put it.
    pub fn note_param(&self, id: u32, value: f64) {
        let _ = self.commands.send(PluginCommand::Note { id, value });
    }
}

impl Drop for PluginHandle {
    fn drop(&mut self) {
        let _ = self.commands.send(PluginCommand::Shutdown);
    }
}

/// The plugin insert. Lives on the plugin thread and nowhere else.
#[derive(Debug, Default)]
struct Insert {
    /// The instance, while one is loaded. Never leaves this thread.
    loaded: Option<Loaded>,
    /// What its parameters were when they were last read.
    ///
    /// Cached rather than asked for on every snapshot: reading them is a call
    /// per parameter into the plugin, and a plugin with two hundred of them
    /// would be two hundred calls sixty times a second for numbers that change
    /// when somebody moves a slider.
    params: Vec<ParamInfo>,
}

impl Insert {
    fn handle(&mut self, command: PluginCommand) {
        match command {
            PluginCommand::Load {
                path,
                id,
                sample_rate,
                max_frames,
                reply,
            } => {
                let result = self.load(&path, id.as_deref(), sample_rate, max_frames);
                let _ = reply.send(result);
            }
            PluginCommand::Retire(processor) => self.retire(*processor),
            PluginCommand::Unload => self.unload(),
            PluginCommand::Note { id, value } => self.note_param(id, value),
            PluginCommand::Shutdown => {}
        }
    }

    fn view(&self) -> PluginView {
        PluginView {
            loaded: self.is_loaded(),
            name: self.name().unwrap_or_default().to_owned(),
            vendor: self.vendor().unwrap_or_default().to_owned(),
            path: self.path().unwrap_or_default().to_owned(),
            params: self.params.clone(),
        }
    }

    /// Load a plugin and activate it, giving back the half for the engine.
    ///
    /// The old instance is *not* dropped here — its processor is still in the
    /// engine. The caller sends the new processor, and the old one arrives back
    /// through the retirement queue to be handed to [`Insert::retire`].
    ///
    /// # Errors
    /// When the bundle will not open, contains no plugin with that id, or the
    /// plugin refuses the sample rate and block size.
    #[allow(
        unsafe_code,
        reason = "loading a plugin is dlopen; there is no safe way to host one"
    )]
    pub fn load(
        &mut self,
        path: &Path,
        id: Option<&str>,
        sample_rate: f64,
        max_frames: u32,
    ) -> Result<Processor, ClapError> {
        // SAFETY: loading a plugin runs its initialiser in this process. There
        // is no way to host plugins that is not this; the DJ chose the file.
        let bundle = unsafe { Bundle::open(path) }?;
        let mut loaded = bundle.instantiate(id)?;
        let processor = loaded.activate(sample_rate, max_frames)?;
        self.params = loaded.params();
        self.loaded = Some(loaded);
        Ok(processor)
    }

    /// Take a processor back from the engine and deactivate it.
    ///
    /// Called from wherever the retirement queue is drained. Deactivation frees
    /// what the plugin allocated, which is the whole reason the engine hands it
    /// back rather than dropping it.
    fn retire(&mut self, processor: Processor) {
        if let Some(loaded) = self.loaded.as_mut() {
            loaded.deactivate(processor);
        }
        // If nothing is loaded the instance has already gone, and dropping the
        // processor is all that is left. That leaks whatever the plugin
        // allocated — which is why `unload` keeps the instance until the
        // processor is home.
    }

    /// Let go of the instance. Only once its processor has come back.
    pub fn unload(&mut self) {
        self.loaded = None;
        self.params.clear();
    }

    fn is_loaded(&self) -> bool {
        self.loaded.is_some()
    }

    /// What the plugin is called, for the interface.
    fn name(&self) -> Option<&str> {
        self.loaded.as_ref().map(|l| l.descriptor().name.as_str())
    }

    fn vendor(&self) -> Option<&str> {
        self.loaded.as_ref().map(|l| l.descriptor().vendor.as_str())
    }

    /// Where it came from, so a set can be reopened.
    fn path(&self) -> Option<&str> {
        self.loaded.as_ref().map(Loaded::path)
    }

    /// Note a parameter's new value without asking the plugin.
    ///
    /// The change itself went to the engine as an event; this keeps the cached
    /// list in step so the slider stays where the DJ put it. Re-reading from
    /// the plugin would be a call per parameter for one number.
    pub fn note_param(&mut self, id: u32, value: f64) {
        if let Some(param) = self.params.iter_mut().find(|p| p.id == id) {
            param.value = value.clamp(param.min, param.max);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_insert_says_so() {
        let insert = Insert::default();
        assert!(!insert.is_loaded());
        assert!(insert.name().is_none());
        assert!(insert.params.is_empty());
    }

    /// A file that is not a plugin must be an error, not a panic and not a
    /// silent nothing — a DJ who picked the wrong file needs to be told.
    #[test]
    fn loading_something_that_is_not_a_plugin_fails() {
        let mut insert = Insert::default();
        let error = insert.load(Path::new("/no/such/plugin.clap"), None, 48_000.0, 512);
        assert!(error.is_err());
        assert!(!insert.is_loaded(), "a failed load left something behind");
    }

    /// The cached value is clamped to the parameter's own range, because the
    /// number came from a slider and a slider can be dragged past its label.
    #[test]
    fn a_noted_parameter_stays_inside_its_range() {
        let mut insert = Insert {
            params: vec![ParamInfo {
                id: 7,
                name: "Gain".to_owned(),
                module: String::new(),
                min: 0.0,
                max: 2.0,
                default: 1.0,
                value: 1.0,
                stepped: false,
                read_only: false,
            }],
            ..Insert::default()
        };
        insert.note_param(7, 9.0);
        assert_eq!(insert.params[0].value, 2.0);
        insert.note_param(7, -9.0);
        assert_eq!(insert.params[0].value, 0.0);
    }

    #[test]
    fn noting_a_parameter_that_is_not_there_changes_nothing() {
        let mut insert = Insert::default();
        insert.note_param(1, 0.5);
        assert!(insert.params.is_empty());
    }

    #[test]
    fn unloading_forgets_the_parameters_too() {
        let mut insert = Insert {
            params: vec![ParamInfo {
                id: 1,
                name: "X".to_owned(),
                module: String::new(),
                min: 0.0,
                max: 1.0,
                default: 0.0,
                value: 0.0,
                stepped: false,
                read_only: false,
            }],
            ..Insert::default()
        };
        insert.unload();
        assert!(insert.params.is_empty(), "a stale control list survived");
    }
}
