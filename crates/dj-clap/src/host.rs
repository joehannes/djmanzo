//! What the plugin is allowed to ask of us.
//!
//! CLAP inverts the usual arrangement: a plugin can call back into its host to
//! ask for a restart, to ask to be processed, or to be called on the main
//! thread later. Those callbacks arrive on threads the host does not choose, so
//! `clack` splits the host into one type per thread specification and the
//! compiler holds us to it.
//!
//! Everything here is a flag. A plugin's request is recorded and acted on by
//! whoever next looks — never acted on inside the callback, because the
//! callback may be running on the audio thread and restarting a plugin means
//! deactivating it, which allocates.

use clack_host::prelude::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Requests a plugin has made and nobody has dealt with yet.
///
/// Shared rather than owned so the application can read them from the main
/// thread while the plugin sets them from wherever it likes.
#[derive(Debug, Default)]
pub struct Requests {
    restart: AtomicBool,
    process: AtomicBool,
    callback: AtomicBool,
}

impl Requests {
    /// The plugin wants to be deactivated and activated again — it has changed
    /// its latency, its ports, or something else that is fixed at activation.
    ///
    /// Reading clears it: this is a request to act on once, and leaving it set
    /// would restart the plugin on every pass.
    pub fn take_restart(&self) -> bool {
        self.restart.swap(false, Ordering::AcqRel)
    }

    /// The plugin has stopped being idle and wants blocks again.
    pub fn take_process(&self) -> bool {
        self.process.swap(false, Ordering::AcqRel)
    }

    /// The plugin wants `on_main_thread` called.
    pub fn take_callback(&self) -> bool {
        self.callback.swap(false, Ordering::AcqRel)
    }
}

/// The host, as the plugin sees it.
#[derive(Debug)]
pub struct Shared {
    requests: Arc<Requests>,
}

impl Shared {
    #[must_use]
    pub fn requests(&self) -> &Arc<Requests> {
        &self.requests
    }
}

impl<'a> SharedHandler<'a> for Shared {
    fn request_restart(&self) {
        self.requests.restart.store(true, Ordering::Release);
    }

    fn request_process(&self) {
        self.requests.process.store(true, Ordering::Release);
    }

    fn request_callback(&self) {
        self.requests.callback.store(true, Ordering::Release);
    }
}

/// djmanzo as a CLAP host.
#[derive(Debug)]
pub struct DjHost;

impl HostHandlers for DjHost {
    type Shared<'a> = Shared;
    type MainThread<'a> = ();
    type AudioProcessor<'a> = ();
}

impl DjHost {
    /// How the host introduces itself to a plugin.
    ///
    /// # Errors
    /// Never in practice — the strings are constants with no interior nul — but
    /// `HostInfo::new` is fallible and swallowing that would be a lie.
    pub fn info() -> Result<HostInfo, std::ffi::NulError> {
        HostInfo::new(
            "djmanzo",
            "djmanzo",
            "https://github.com/joehannes/djmanzo",
            env!("CARGO_PKG_VERSION"),
        )
    }

    /// Build the shared half, handing back the request flags to keep.
    pub fn shared() -> (impl FnOnce(&()) -> Shared, Arc<Requests>) {
        let requests = Arc::new(Requests::default());
        let mine = Arc::clone(&requests);
        (move |_| Shared { requests: mine }, requests)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_host_can_describe_itself() {
        assert!(DjHost::info().is_ok());
    }

    /// A request is acted on once. Leaving the flag set would restart the
    /// plugin on every pass, which is a deactivate-and-activate — allocation,
    /// sixty times a second.
    #[test]
    fn taking_a_request_clears_it() {
        let requests = Requests::default();
        assert!(!requests.take_restart());

        requests.restart.store(true, Ordering::Release);
        assert!(requests.take_restart());
        assert!(!requests.take_restart(), "the request came back");
    }

    #[test]
    fn the_three_requests_are_independent() {
        let requests = Requests::default();
        requests.process.store(true, Ordering::Release);
        assert!(!requests.take_restart());
        assert!(!requests.take_callback());
        assert!(requests.take_process());
    }
}
