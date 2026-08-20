//! Hosting CLAP plugins.
//!
//! # Why CLAP and not VST
//!
//! CLAP is MIT-licensed, which is the whole reason it is here. VST3's SDK is
//! GPL-or-commercial and VST2's is no longer available at all; both are ruled
//! out by ADR-0002 before any technical question is asked. CLAP is also simply
//! a better fit — its threading model is written down, its parameters are
//! addressable by a stable id, and a host can drive one without a GUI.
//!
//! # The shape of it
//!
//! A CLAP plugin is two objects with two different homes, and the split maps
//! exactly onto the one this application already has:
//!
//! - the **instance** lives on the main thread. It is not [`Send`]. It answers
//!   questions about parameters, and it is the only thing that may activate or
//!   deactivate the plugin — which is where allocation happens.
//! - the **audio processor** is [`Send`], and it goes to the audio thread. It
//!   does one thing: process a block.
//!
//! So a plugin is loaded, activated and inspected here, and the processor
//! crosses to the engine on the command queue like a track buffer does. It
//! comes back through the retirement queue, because deactivating it is
//! deallocation and deallocation does not happen on the audio thread.
//!
//! # What this cannot promise
//!
//! A CLAP plugin's `process` is *supposed* to be free of allocation, locks and
//! I/O. Nothing in the specification enforces it and nothing here can. The
//! allocation-counting harness that proves djmanzo's own audio path clean will
//! catch a badly behaved plugin the moment one is loaded — but only for the
//! plugin actually being run, and only on the machine running it. A DJ loading
//! a third-party plugin into the master chain is taking on that risk, and
//! saying so is more honest than implying a guarantee.
//!
//! # Plugin windows
//!
//! Not hosted. A plugin's own interface is a native child window — an X11
//! window on Linux, an `NSView` on macOS — and there is nowhere to put one
//! inside a webview. Parameters are exposed generically instead: every
//! plugin's controls are readable and settable, and djmanzo draws them itself.
//! That is less pretty than the plugin's own panel and it works everywhere.

pub mod host;
pub mod params;
pub mod plugin;
pub mod scan;
#[cfg(feature = "test-plugin")]
pub mod testplug;

pub use host::DjHost;
pub use params::ParamInfo;
pub use plugin::{Bundle, ClapError, Descriptor, Loaded, Processor};
pub use scan::{Found, scan, search_paths};
