//! A loaded plugin, and the half of it that goes to the audio thread.
//!
//! # The split
//!
//! [`Loaded`] lives on the main thread and is not [`Send`]. It knows what the
//! plugin is called, what its parameters are, and how to activate and
//! deactivate it — the two operations that allocate.
//!
//! [`Processor`] is [`Send`] and does one thing. It crosses to the engine on
//! the command queue exactly as a track buffer does, and it comes back through
//! the retirement queue, because deactivating it is deallocation.
//!
//! # Planar in, interleaved out
//!
//! CLAP hands a plugin one buffer per channel; djmanzo's engine works in
//! interleaved stereo. So [`Processor`] owns scratch buffers, sized once at
//! activation, and de-interleaves into them on the way in and back on the way
//! out. That copy is the price of the format boundary and it is two passes over
//! a block — measurably nothing beside what a plugin then does with it.
//!
//! # Blocks smaller than the maximum
//!
//! A plugin is activated for a *range* of block sizes and the device may hand
//! over fewer frames than the maximum on any given callback. The scratch is
//! sized for the maximum and only the frames actually present are used, which
//! is why nothing here reallocates when a device changes its mind about buffer
//! size mid-stream.

use crate::host::{DjHost, Requests};
use crate::params::ParamInfo;
use clack_host::prelude::*;
use clack_host::process::PluginAudioProcessor;
use std::path::Path;
use std::sync::Arc;

/// Channels djmanzo carries. Stereo, like everything else in the engine.
pub const CHANNELS: usize = 2;

#[derive(Debug, thiserror::Error)]
pub enum ClapError {
    #[error("cannot load {path}: {reason}")]
    Load { path: String, reason: String },
    #[error("{path} contains no plugins")]
    Empty { path: String },
    #[error("{path} has no plugin with id {id}")]
    NoSuchPlugin { path: String, id: String },
    #[error("cannot start {name}: {reason}")]
    Activate { name: String, reason: String },
    #[error("{0}")]
    Host(String),
}

/// One plugin inside a bundle, before anything is instantiated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Descriptor {
    /// The plugin's stable id, which is what identifies it in a saved set.
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub version: String,
}

/// A plugin bundle, opened but with nothing instantiated.
///
/// Kept as its own step because one `.clap` file may contain several plugins,
/// and a DJ picking one from a list needs the list before anything is created.
pub struct Bundle {
    entry: PluginEntry,
    path: String,
}

impl std::fmt::Debug for Bundle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The entry is a loaded dynamic library; there is nothing about it a
        // log would want that the path does not already say.
        f.debug_struct("Bundle")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl Bundle {
    /// Open a `.clap` bundle.
    ///
    /// # Errors
    /// When the file is not a CLAP bundle, or cannot be read.
    ///
    /// # Safety
    /// Loading a bundle runs its initialiser, which is arbitrary third-party
    /// code in this process. There is no way to host plugins that is not this,
    /// and pretending otherwise with a safe signature would be the lie.
    #[allow(
        unsafe_code,
        reason = "loading a plugin is dlopen; there is no safe way to host one"
    )]
    pub unsafe fn open(path: &Path) -> Result<Bundle, ClapError> {
        let text = path.display().to_string();
        // SAFETY: the caller has accepted that loading a plugin runs its code.
        let entry = unsafe { PluginEntry::load(path) }.map_err(|error| ClapError::Load {
            path: text.clone(),
            reason: error.to_string(),
        })?;
        Ok(Bundle { entry, path: text })
    }

    /// Open a bundle compiled into this binary rather than read from disk.
    ///
    /// This is how the tests here host a real plugin without a real file — see
    /// the crate note. It is not `unsafe`, because a statically linked entry is
    /// code that was already in the process.
    #[cfg(feature = "test-plugin")]
    pub fn from_clack<E: clack_plugin::entry::Entry>(name: &str) -> Result<Bundle, ClapError> {
        let path = std::ffi::CString::new(name).map_err(|e| ClapError::Load {
            path: name.to_owned(),
            reason: e.to_string(),
        })?;
        let entry = PluginEntry::load_from_clack::<E>(&path).map_err(|error| ClapError::Load {
            path: name.to_owned(),
            reason: error.to_string(),
        })?;
        Ok(Bundle {
            entry,
            path: name.to_owned(),
        })
    }

    /// What is inside.
    #[must_use]
    pub fn plugins(&self) -> Vec<Descriptor> {
        let Some(factory) = self.entry.get_plugin_factory() else {
            return Vec::new();
        };
        factory
            .plugin_descriptors()
            .filter_map(|descriptor| {
                Some(Descriptor {
                    // A plugin with no id cannot be instantiated and cannot be
                    // saved in a set, so it is skipped rather than listed.
                    id: descriptor.id().and_then(text)?,
                    // A plugin with no *name* is merely awkward: it falls back
                    // to its id below rather than being dropped.
                    name: descriptor.name().and_then(text).unwrap_or_default(),
                    vendor: descriptor.vendor().and_then(text).unwrap_or_default(),
                    version: descriptor.version().and_then(text).unwrap_or_default(),
                })
            })
            .map(|mut descriptor| {
                if descriptor.name.is_empty() {
                    descriptor.name = descriptor.id.clone();
                }
                descriptor
            })
            .collect()
    }

    /// Instantiate one of them.
    ///
    /// `id` is the plugin's stable id. `None` takes the first, which is what a
    /// bundle containing exactly one plugin means.
    ///
    /// # Errors
    /// When the bundle has no plugins, or none with that id.
    pub fn instantiate(&self, id: Option<&str>) -> Result<Loaded, ClapError> {
        let descriptors = self.plugins();
        let descriptor = match id {
            Some(wanted) => descriptors.iter().find(|d| d.id == wanted).ok_or_else(|| {
                ClapError::NoSuchPlugin {
                    path: self.path.clone(),
                    id: wanted.to_owned(),
                }
            })?,
            None => descriptors.first().ok_or_else(|| ClapError::Empty {
                path: self.path.clone(),
            })?,
        }
        .clone();

        let info = DjHost::info().map_err(|e| ClapError::Host(e.to_string()))?;
        let (shared, requests) = DjHost::shared();
        let id = std::ffi::CString::new(descriptor.id.as_str()).map_err(|e| ClapError::Load {
            path: self.path.clone(),
            reason: e.to_string(),
        })?;
        let instance = PluginInstance::<DjHost>::new(shared, |_| (), &self.entry, &id, &info)
            .map_err(|error| ClapError::Load {
                path: self.path.clone(),
                reason: error.to_string(),
            })?;

        Ok(Loaded {
            instance,
            descriptor,
            requests,
            path: self.path.clone(),
        })
    }
}

fn text(value: &std::ffi::CStr) -> Option<String> {
    Some(value.to_string_lossy().into_owned()).filter(|s| !s.is_empty())
}

/// An instantiated plugin, on the main thread.
///
/// `Debug` is hand-written for the same reason as [`Processor`]'s.
pub struct Loaded {
    instance: PluginInstance<DjHost>,
    descriptor: Descriptor,
    requests: Arc<Requests>,
    path: String,
}

impl std::fmt::Debug for Loaded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Loaded")
            .field("descriptor", &self.descriptor)
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl Loaded {
    #[must_use]
    pub fn descriptor(&self) -> &Descriptor {
        &self.descriptor
    }

    /// Where it came from, so a set can be reopened.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Requests the plugin has made and nobody has dealt with yet.
    #[must_use]
    pub fn requests(&self) -> &Arc<Requests> {
        &self.requests
    }

    /// Everything the plugin lets a host change.
    #[must_use]
    pub fn params(&mut self) -> Vec<ParamInfo> {
        crate::params::read(&mut self.instance)
    }

    /// Activate for a sample rate and block size, producing the half that goes
    /// to the audio thread.
    ///
    /// Allocation happens here, on this thread, which is the point of the
    /// split. A plugin is activated once when it is loaded and again only if
    /// the device changes.
    ///
    /// # Errors
    /// When the plugin refuses the configuration.
    pub fn activate(&mut self, sample_rate: f64, max_frames: u32) -> Result<Processor, ClapError> {
        let configuration = PluginAudioConfiguration {
            sample_rate,
            // A device may hand over fewer frames than it promised on any given
            // callback, so the minimum is one rather than the maximum. A plugin
            // told both numbers are the same is entitled to assume it.
            min_frames_count: 1,
            max_frames_count: max_frames,
        };
        let stopped = self
            .instance
            .activate(|_, _| (), configuration)
            .map_err(|error| ClapError::Activate {
                name: self.descriptor.name.clone(),
                reason: error.to_string(),
            })?;

        Ok(Processor::new(stopped.into(), max_frames as usize))
    }

    /// Take the processor back and deactivate.
    ///
    /// The processor has to come home for this: deactivation frees whatever the
    /// plugin allocated, and it is the reason the engine hands its half back
    /// rather than dropping it.
    pub fn deactivate(&mut self, processor: Processor) {
        self.instance.deactivate(processor.processor.into_stopped());
    }
}

/// The audio-thread half.
///
/// `Debug` is hand-written: almost everything inside is a `clack` type or a
/// pointer into a plugin, none of which are `Debug` and none of which would
/// mean anything printed. What a reader of a log wants to know is that there is
/// a plugin here and whether it is still producing sound.
///
/// Sent to the engine on the command queue and handed back through the
/// retirement queue. Everything it needs is sized in [`Processor::new`].
pub struct Processor {
    processor: PluginAudioProcessor<DjHost>,
    ports_in: AudioPorts,
    ports_out: AudioPorts,
    /// De-interleaved input, one buffer per channel.
    scratch_in: Vec<Vec<f32>>,
    scratch_out: Vec<Vec<f32>>,
    events_in: EventBuffer,
    events_out: EventBuffer,
    /// Frames processed since activation, which is what CLAP calls steady time.
    steady: u64,
    /// The plugin stopped producing sound and asked to be left idle.
    ///
    /// Recorded rather than acted on: an effect that says it has nothing more
    /// to add is telling the host it may skip it, and skipping it is exactly
    /// what makes a reverb tail stop dead. See [`Processor::process`].
    quiet: bool,
}

impl std::fmt::Debug for Processor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Processor")
            .field("frames", &self.steady)
            .field("quiet", &self.quiet)
            .finish_non_exhaustive()
    }
}

impl Processor {
    fn new(processor: PluginAudioProcessor<DjHost>, max_frames: usize) -> Processor {
        Processor {
            processor,
            // Sized at construction so `with_input_buffers` never reallocates
            // on the audio thread. Exceeding this capacity is what would make
            // it, and nothing does: the block size is fixed at activation.
            ports_in: AudioPorts::with_capacity(CHANNELS, 1),
            ports_out: AudioPorts::with_capacity(CHANNELS, 1),
            scratch_in: vec![vec![0.0; max_frames]; CHANNELS],
            scratch_out: vec![vec![0.0; max_frames]; CHANNELS],
            events_in: EventBuffer::with_capacity(64),
            events_out: EventBuffer::with_capacity(64),
            steady: 0,
            quiet: false,
        }
    }

    /// Whether the plugin last reported it had nothing more to add.
    #[must_use]
    pub fn is_quiet(&self) -> bool {
        self.quiet
    }

    /// Run one block of interleaved stereo through the plugin, in place.
    ///
    /// A block longer than the plugin was activated for is processed as far as
    /// the scratch allows and the rest passes through untouched. That cannot
    /// happen with a fixed buffer size, and truncating beats a panic on the
    /// audio thread if a device ever surprises us.
    pub fn process(&mut self, buffer: &mut [f32]) {
        let frames = (buffer.len() / CHANNELS).min(self.scratch_in[0].len());
        if frames == 0 {
            return;
        }

        if self.processor.ensure_processing_started().is_err() {
            return;
        }

        for (frame, samples) in buffer
            .as_chunks::<CHANNELS>()
            .0
            .iter()
            .take(frames)
            .enumerate()
        {
            for (scratch, sample) in self.scratch_in.iter_mut().zip(samples) {
                scratch[frame] = *sample;
            }
        }

        let status = {
            let (left_in, rest_in) = self.scratch_in.split_at_mut(1);
            let (left_out, rest_out) = self.scratch_out.split_at_mut(1);
            let inputs = self.ports_in.with_input_buffers([AudioPortBuffer {
                latency: 0,
                channels: AudioPortBufferType::f32_input_only(
                    [
                        InputChannel::variable(&mut left_in[0][..frames]),
                        InputChannel::variable(&mut rest_in[0][..frames]),
                    ]
                    .into_iter(),
                ),
            }]);
            let mut outputs = self.ports_out.with_output_buffers([AudioPortBuffer {
                latency: 0,
                channels: AudioPortBufferType::f32_output_only(
                    [&mut left_out[0][..frames], &mut rest_out[0][..frames]].into_iter(),
                ),
            }]);

            let Ok(started) = self.processor.as_started_mut() else {
                return;
            };
            started.process(
                &inputs,
                &mut outputs,
                &self.events_in.as_input(),
                &mut self.events_out.as_output(),
                Some(self.steady),
                None,
            )
        };

        self.events_in.clear();
        self.events_out.clear();
        self.steady += frames as u64;

        match status {
            Ok(status) => {
                // `Sleep` means the plugin has nothing more to add — its tail
                // has run out. Its output for this block is still valid and is
                // still copied back; treating "sleep" as "skip this block"
                // would cut a reverb tail off at the moment it went quiet,
                // which is audible and wrong.
                self.quiet = matches!(status, ProcessStatus::Sleep);
            }
            Err(_) => {
                // A plugin that failed mid-block has told us nothing about what
                // is in the output buffer. Leaving the input alone is the only
                // safe answer: the alternative is copying whatever garbage is
                // in the scratch to the speakers.
                return;
            }
        }

        for (frame, samples) in buffer
            .as_chunks_mut::<CHANNELS>()
            .0
            .iter_mut()
            .take(frames)
            .enumerate()
        {
            for (scratch, sample) in self.scratch_out.iter().zip(samples) {
                *sample = scratch[frame];
            }
        }
    }

    /// Queue a parameter change to be delivered with the next block.
    ///
    /// Events rather than a direct call, because that is how CLAP says a host
    /// changes a parameter while audio is running — and it is what lets a
    /// plugin smooth the change rather than step it.
    pub fn set_param(&mut self, id: u32, value: f64) {
        use clack_host::events::event_types::ParamValueEvent;
        use clack_host::events::{Match, Pckn};
        use clack_host::utils::Cookie;
        let Some(id) = ClapId::from_raw(id) else {
            return;
        };
        self.events_in.push(&ParamValueEvent::new(
            0,
            id,
            Pckn::new(0u16, 0u16, Match::All, Match::All),
            value,
            Cookie::empty(),
        ));
    }
}

// SAFETY: `PluginAudioProcessor` is the half CLAP defines as belonging to the
// audio thread, and clack marks it `Send` for exactly this journey. The scratch
// buffers and event buffers are plain owned data.
#[allow(
    unsafe_code,
    clippy::non_send_fields_in_send_ty,
    reason = "CLAP's own thread model says this half travels to the audio thread"
)]
unsafe impl Send for Processor {}
