//! A CLAP plugin, compiled in, for testing hosts against.
//!
//! # Why this is in `src` and not in `tests`
//!
//! There are no `.clap` bundles in CI, none in a fresh checkout, and none in
//! most containers. A host tested only against whatever happens to be
//! installed is a host tested nowhere — it compiles, ships, and fails the
//! first time a DJ points it at a plugin.
//!
//! So this is a real plugin: it implements CLAP's traits, declares a
//! parameter, activates for a sample rate and block size, and processes
//! blocks. `clack-host` can load an entry that is already in the process, so
//! it is hosted through exactly the same code path a plugin read off a disk
//! would take.
//!
//! It lives here rather than in this crate's own `tests/` because `dj-engine`
//! needs it too: the engine's plugin insert cannot be tested without a plugin
//! in it, and two copies of a test fixture is one copy too many. Behind a
//! feature that nothing in a shipped djmanzo turns on.

use clack_plugin::entry::DefaultPluginFactory;
use clack_plugin::events::event_types::ParamValueEvent;
use clack_plugin::prelude::*;

/// The parameter id the plugin exposes. Deliberately not zero and not an
/// index, because a host must address parameters by id and a test where id and
/// index agree would not notice if it used the wrong one.
pub const GAIN_ID: u32 = 77;

// -- the plugin -------------------------------------------------------------

#[derive(Debug)]
pub struct TestGain;

/// Shared between the plugin's threads: the gain, as the host last set it.
#[derive(Debug, Default)]
pub struct Shared {
    gain: std::sync::atomic::AtomicU64,
}

impl Shared {
    fn get(&self) -> f64 {
        f64::from_bits(self.gain.load(std::sync::atomic::Ordering::Relaxed))
    }

    fn set(&self, value: f64) {
        self.gain
            .store(value.to_bits(), std::sync::atomic::Ordering::Relaxed);
    }
}

impl<'a> PluginShared<'a> for Shared {}

#[derive(Debug)]
pub struct MainThread<'a> {
    shared: &'a Shared,
}

impl<'a> PluginMainThread<'a, Shared> for MainThread<'a> {}

#[derive(Debug)]
pub struct Processor<'a> {
    shared: &'a Shared,
}

impl<'a> PluginAudioProcessor<'a, Shared, MainThread<'a>> for Processor<'a> {
    fn activate(
        _host: HostAudioProcessorHandle<'a>,
        _main_thread: &mut MainThread<'a>,
        shared: &'a Shared,
        _config: PluginAudioConfiguration,
    ) -> Result<Self, PluginError> {
        Ok(Processor { shared })
    }

    fn process(
        &mut self,
        _process: Process,
        mut audio: Audio,
        events: Events,
    ) -> Result<ProcessStatus, PluginError> {
        // Parameter changes arrive with the block, which is how CLAP says a
        // host changes a parameter while audio is running.
        for event in events.input {
            // The id is checked, as any real plugin's would be. Without this
            // the host could address the wrong parameter and the tests would
            // never notice.
            if let Some(value) = event.as_event::<ParamValueEvent>()
                && u32::from(value.param_id().unwrap_or(ClapId::new(0))) == GAIN_ID
            {
                self.shared.set(value.value());
            }
        }

        let gain = self.shared.get() as f32;
        for mut port in &mut audio {
            let Some(channels) = port.channels()?.into_f32() else {
                continue;
            };
            for pair in channels {
                match pair {
                    ChannelPair::InputOutput(input, output) => {
                        for (out, inp) in output.iter_mut().zip(input) {
                            *out = *inp * gain;
                        }
                    }
                    ChannelPair::InPlace(buffer) => {
                        for sample in buffer.iter_mut() {
                            *sample *= gain;
                        }
                    }
                    ChannelPair::OutputOnly(output) => output.fill(0.0),
                    ChannelPair::InputOnly(_) => {}
                }
            }
        }
        Ok(ProcessStatus::Continue)
    }
}

impl Plugin for TestGain {
    type AudioProcessor<'a> = Processor<'a>;
    type Shared<'a> = Shared;
    type MainThread<'a> = MainThread<'a>;

    fn declare_extensions(builder: &mut PluginExtensions<Self>, _shared: Option<&Shared>) {
        builder.register::<clack_extensions::params::PluginParams>();
    }
}

impl DefaultPluginFactory for TestGain {
    fn get_descriptor() -> PluginDescriptor {
        PluginDescriptor::new("dev.djmanzo.testgain", "Test Gain")
            .with_vendor("djmanzo")
            .with_version("1.0.0")
    }

    fn new_shared(_host: HostSharedHandle<'_>) -> Result<Shared, PluginError> {
        let shared = Shared::default();
        shared.set(1.0);
        Ok(shared)
    }

    fn new_main_thread<'a>(
        _host: HostMainThreadHandle<'a>,
        shared: &'a Shared,
    ) -> Result<MainThread<'a>, PluginError> {
        Ok(MainThread { shared })
    }
}

impl clack_extensions::params::PluginAudioProcessorParams for Processor<'_> {
    fn flush(
        &mut self,
        input_parameter_changes: &InputEvents,
        _output_parameter_changes: &mut OutputEvents,
    ) {
        for event in input_parameter_changes {
            if let Some(value) = event.as_event::<ParamValueEvent>()
                && u32::from(value.param_id().unwrap_or(ClapId::new(0))) == GAIN_ID
            {
                self.shared.set(value.value());
            }
        }
    }
}

impl clack_extensions::params::PluginMainThreadParams for MainThread<'_> {
    fn count(&mut self) -> u32 {
        1
    }

    fn get_info(&mut self, param_index: u32, info: &mut clack_extensions::params::ParamInfoWriter) {
        if param_index != 0 {
            return;
        }
        info.set(&clack_extensions::params::ParamInfo {
            id: ClapId::new(GAIN_ID),
            flags: clack_extensions::params::ParamInfoFlags::IS_AUTOMATABLE,
            cookie: Default::default(),
            name: b"Gain",
            module: b"Amp/Gain",
            min_value: 0.0,
            max_value: 2.0,
            default_value: 1.0,
        });
    }

    fn get_value(&mut self, param_id: ClapId) -> Option<f64> {
        (u32::from(param_id) == GAIN_ID).then(|| self.shared.get())
    }

    fn value_to_text(
        &mut self,
        _param_id: ClapId,
        value: f64,
        writer: &mut clack_extensions::params::ParamDisplayWriter,
    ) -> std::fmt::Result {
        use std::fmt::Write;
        write!(writer, "{value:.2}")
    }

    fn text_to_value(&mut self, _param_id: ClapId, text: &std::ffi::CStr) -> Option<f64> {
        text.to_str().ok()?.parse().ok()
    }

    fn flush(
        &mut self,
        input_parameter_changes: &InputEvents,
        _output_parameter_changes: &mut OutputEvents,
    ) {
        for event in input_parameter_changes {
            if let Some(value) = event.as_event::<ParamValueEvent>()
                && u32::from(value.param_id().unwrap_or(ClapId::new(0))) == GAIN_ID
            {
                self.shared.set(value.value());
            }
        }
    }
}

/// The entry, as a host loads it.
pub type Entry = SinglePluginEntry<TestGain>;

/// This plugin, opened as a bundle.
///
/// # Errors
/// Never in practice — the entry is compiled in — but the loading path is
/// fallible and swallowing that would hide a real break.
pub fn bundle() -> Result<crate::Bundle, crate::ClapError> {
    crate::Bundle::from_clack::<Entry>("in-process/TestGain.clap")
}
