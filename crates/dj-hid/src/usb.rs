//! Opening a HID controller and reading its reports.
//!
//! # Why this is a thread and not a callback
//!
//! `midir` hands MIDI over on a callback the operating system owns. HID has no
//! such thing: a device is a file you read from, and somebody has to do the
//! reading. So djmanzo owns a thread per open device, blocking on a read with
//! a short timeout -- short so that closing the device does not wait on a
//! controller nobody is touching.
//!
//! The timeout is the only cost of that design and it is paid once per device:
//! at [`POLL_MS`] the thread wakes twenty times a second on a silent
//! controller, which is nothing, and returns immediately whenever a report
//! actually arrives.
//!
//! # Matched by name, like everything else
//!
//! A HID device is identified by numbers -- a vendor ID and a product ID --
//! and by strings it reports for itself. djmanzo matches the strings, loosely,
//! by the same rule [`Mapping::fits`] uses for a MIDI port, so one mapping
//! file works on a machine that spells the product differently. The numbers
//! are shown too, because two devices from one maker often share a name and
//! the numbers are what tell them apart.

use crate::mapping::Mapping;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;

/// How long a read waits before letting the thread check whether it should
/// stop. Twenty wake-ups a second on an idle device.
const POLL_MS: i32 = 50;

/// The largest input report djmanzo will read.
///
/// HID reports are small by specification -- 64 bytes is the full-speed
/// interrupt maximum, and DJ controllers use far less. The buffer is fixed so
/// the read thread never allocates.
const MAX_REPORT: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HidError {
    #[error("HID is not available on this machine: {0}")]
    Unavailable(String),
    #[error("no HID device called {0:?}")]
    NoSuchDevice(String),
    #[error("could not open {0:?}: {1}")]
    Refused(String, String),
}

/// A HID device the machine can see.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    /// Manufacturer and product as the device reports them, which is what a
    /// mapping's `device` is matched against.
    pub name: String,
    pub vendor: u16,
    pub product: u16,
    /// The operating system's own handle for this exact device. Two identical
    /// controllers differ only here.
    pub path: String,
}

impl DeviceInfo {
    /// `1234:5678`, as a device is written in documentation and in `lsusb`.
    #[must_use]
    pub fn id(&self) -> String {
        format!("{:04x}:{:04x}", self.vendor, self.product)
    }

    /// Whether `wanted` names this device.
    ///
    /// Three ways, because a DJ has three things to hand: the name on the box,
    /// the string the device reports, or the numbers from `lsusb`. Loose on
    /// the name for the same reason MIDI is -- a device is "DDJ-400" on one
    /// platform and "PIONEER DDJ-400" on another.
    #[must_use]
    pub fn answers_to(&self, wanted: &str) -> bool {
        if wanted.is_empty() {
            return false;
        }
        let wanted = wanted.to_lowercase();
        self.path == wanted || self.id() == wanted || self.name.to_lowercase().contains(&wanted)
    }
}

/// Every HID device the machine can see.
///
/// # Errors
/// When HID itself cannot be reached -- no permission to enumerate, or no
/// device layer at all, which is the normal state inside a container.
pub fn devices() -> Result<Vec<DeviceInfo>, HidError> {
    let api = hidapi::HidApi::new().map_err(|e| HidError::Unavailable(e.to_string()))?;
    Ok(api.device_list().map(describe).collect())
}

fn describe(device: &hidapi::DeviceInfo) -> DeviceInfo {
    // A device that reports neither string still has to be openable, so it
    // falls back to its numbers rather than to an empty name nothing matches.
    let name = match (device.manufacturer_string(), device.product_string()) {
        (Some(maker), Some(product)) => format!("{maker} {product}").trim().to_owned(),
        (None, Some(product)) => product.to_owned(),
        (Some(maker), None) => maker.to_owned(),
        (None, None) => format!("{:04x}:{:04x}", device.vendor_id(), device.product_id()),
    };
    DeviceInfo {
        name,
        vendor: device.vendor_id(),
        product: device.product_id(),
        path: device.path().to_string_lossy().into_owned(),
    }
}

/// An open HID device. Dropping it stops the reader and closes the device.
///
/// Held rather than detached for the same reason a MIDI connection is: a
/// controller that keeps sending after the DJ disconnected it is a controller
/// nobody can turn off.
pub struct Connection {
    stop: Arc<AtomicBool>,
    reader: Option<std::thread::JoinHandle<()>>,
    device: String,
    mapping: String,
}

impl Connection {
    #[must_use]
    pub fn device(&self) -> &str {
        &self.device
    }

    #[must_use]
    pub fn mapping(&self) -> &str {
        &self.mapping
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(reader) = self.reader.take() {
            // Joining rather than detaching, so the device really is closed
            // when this returns and reopening it cannot race the old reader.
            // The wait is bounded by POLL_MS.
            let _ = reader.join();
        }
    }
}

impl std::fmt::Debug for Connection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Connection")
            .field("device", &self.device)
            .field("mapping", &self.mapping)
            .finish_non_exhaustive()
    }
}

/// What the editor watches a HID device with.
///
/// Different from the MIDI listener in one way that matters: a HID report says
/// nothing about what changed, so learning needs the **previous** report to
/// compare against. That is what `last` holds.
#[derive(Clone, Debug, Default)]
pub struct Listener {
    learning: Arc<AtomicBool>,
    seen: Arc<std::sync::Mutex<Option<String>>>,
    last: Arc<std::sync::Mutex<Option<Vec<u8>>>>,
}

impl Listener {
    pub fn start(&self) {
        self.clear();
        self.learning.store(true, Ordering::Release);
    }

    pub fn stop(&self) {
        self.learning.store(false, Ordering::Release);
    }

    #[must_use]
    pub fn is_learning(&self) -> bool {
        self.learning.load(Ordering::Acquire)
    }

    #[must_use]
    pub fn seen(&self) -> Option<String> {
        self.seen.lock().ok()?.clone()
    }

    /// Forget both what was seen and what it was compared against.
    ///
    /// The previous report goes too: keeping it would let the next `start`
    /// diff against a packet from before the DJ pressed anything, and name a
    /// control they touched a minute ago.
    pub fn clear(&self) {
        if let Ok(mut slot) = self.seen.lock() {
            *slot = None;
        }
        if let Ok(mut slot) = self.last.lock() {
            *slot = None;
        }
    }

    /// Called from the reader thread. `true` means the report was consumed by
    /// learning and must not also be acted on.
    fn note(&self, report: &[u8]) -> bool {
        if !self.is_learning() {
            return false;
        }
        let Ok(mut last) = self.last.lock() else {
            return true;
        };
        // The report ID is the first byte when the device numbers its reports.
        // A device that does not sends 0 there via `read`, which is exactly
        // the convention `Field` uses for "the packet is payload".
        let id = report.first().copied().unwrap_or(0);
        if let Some(before) = last.as_deref()
            && let Some(field) = crate::report::changed_field(id, before, report)
            && let Ok(mut slot) = self.seen.lock()
        {
            *slot = Some(field.describe());
        }
        *last = Some(report.to_vec());
        true
    }
}

/// Open the HID device `wanted` and send everything it says, translated, down
/// `out`.
///
/// # Errors
/// When HID is unavailable, no device answers to the name, or the platform
/// refuses to open the one that does -- which on Linux usually means the
/// device node belongs to root and needs a udev rule.
pub fn open(
    wanted: &str,
    mapping: Mapping,
    out: Sender<String>,
    listener: Listener,
) -> Result<Connection, HidError> {
    let api = hidapi::HidApi::new().map_err(|e| HidError::Unavailable(e.to_string()))?;
    let found = api
        .device_list()
        .map(describe)
        .find(|device| device.answers_to(wanted))
        .ok_or_else(|| HidError::NoSuchDevice(wanted.to_owned()))?;

    let path = std::ffi::CString::new(found.path.clone())
        .map_err(|e| HidError::Refused(found.name.clone(), e.to_string()))?;
    let device = api
        .open_path(&path)
        .map_err(|e| HidError::Refused(found.name.clone(), e.to_string()))?;

    let stop = Arc::new(AtomicBool::new(false));
    let name = found.name.clone();
    let mapping_name = mapping.name.clone();
    let reader = std::thread::Builder::new()
        .name("djmanzo-hid".into())
        .spawn({
            let stop = Arc::clone(&stop);
            move || pump(&device, mapping, &out, &listener, &stop)
        })
        .map_err(|e| HidError::Refused(name.clone(), e.to_string()))?;

    Ok(Connection {
        stop,
        reader: Some(reader),
        device: name,
        mapping: mapping_name,
    })
}

/// Read reports until told to stop.
///
/// Errors end the loop rather than being retried: on HID an error from a read
/// means the device went away, and a retry loop against an unplugged
/// controller is a thread spinning for the rest of the session.
fn pump(
    device: &hidapi::HidDevice,
    mut mapping: Mapping,
    out: &Sender<String>,
    listener: &Listener,
    stop: &AtomicBool,
) {
    let mut buffer = [0u8; MAX_REPORT];
    while !stop.load(Ordering::Acquire) {
        let read = match device.read_timeout(&mut buffer, POLL_MS) {
            Ok(0) => continue, // the timeout expired; nothing was sent
            Ok(read) => read,
            Err(_) => return,
        };
        let report = &buffer[..read];

        // While the editor is listening, a control says what it is instead of
        // doing what it does.
        if listener.note(report) {
            continue;
        }
        for action in mapping.translate_report(report) {
            // A disconnected channel means the application has gone away.
            // Nothing useful can be done about it from here.
            if out.send(action).is_err() {
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(name: &str, vendor: u16, product: u16) -> DeviceInfo {
        DeviceInfo {
            name: name.to_owned(),
            vendor,
            product,
            path: "/dev/hidraw0".to_owned(),
        }
    }

    /// A machine with no HID layer -- every container, and a stripped system
    /// -- must report that rather than panicking or hanging.
    #[test]
    fn listing_devices_survives_a_machine_without_hid() {
        match devices() {
            Ok(found) => {
                for device in found {
                    assert!(!device.id().is_empty());
                }
            }
            Err(HidError::Unavailable(why)) => assert!(!why.is_empty()),
            Err(other) => panic!("listing should not fail this way: {other}"),
        }
    }

    /// Opening something that is not there says which, rather than opening
    /// whatever happened to be first.
    #[test]
    fn opening_a_device_that_is_not_there_says_which() {
        let (post, _take) = std::sync::mpsc::channel();
        let mapping = Mapping::parse(
            "name = \"x\"\ndevice = \"y\"\n\n[[binding]]\non = \"hid 1 bit 0.0\"\n\
             press = \"deck 1 play_pause\"\n",
        )
        .expect("the mapping parses");
        match open(
            "not-a-real-device-anywhere",
            mapping,
            post,
            Listener::default(),
        ) {
            Err(HidError::NoSuchDevice(name)) => assert_eq!(name, "not-a-real-device-anywhere"),
            // No HID layer at all is the container's honest answer.
            Err(HidError::Unavailable(_)) => {}
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    /// Three ways to name a device, because a DJ has three things to hand: the
    /// name on the box, the string it reports, and the numbers from `lsusb`.
    #[test]
    fn a_device_answers_to_its_name_its_numbers_or_its_path() {
        let ddj = device("PIONEER DDJ-400", 0x2b73, 0x0017);
        assert!(ddj.answers_to("DDJ-400"));
        assert!(ddj.answers_to("ddj"), "matching must not be case sensitive");
        assert!(ddj.answers_to("2b73:0017"));
        assert!(ddj.answers_to("/dev/hidraw0"));
        assert!(!ddj.answers_to("SC6000"));
    }

    /// An empty name must match nothing. "Whatever fits" would connect a DJ's
    /// controller to somebody else's mapping.
    #[test]
    fn an_empty_name_answers_to_nothing() {
        assert!(!device("PIONEER DDJ-400", 0x2b73, 0x0017).answers_to(""));
    }

    /// The numbers are what tell two identical controllers apart, so they have
    /// to be printed the way documentation prints them.
    #[test]
    fn the_id_is_written_the_way_lsusb_writes_it() {
        assert_eq!(device("x", 0x2b73, 0x0017).id(), "2b73:0017");
        assert_eq!(device("x", 0x000f, 0x00a0).id(), "000f:00a0");
    }

    /// Learning needs two reports: the first is only something to compare
    /// against, and naming a control from it alone would be a guess.
    #[test]
    fn learning_names_the_control_that_moved_between_two_reports() {
        let listener = Listener::default();
        listener.start();

        assert!(listener.note(&[1u8, 0b0000_0000]));
        assert_eq!(listener.seen(), None, "one report cannot name a control");

        assert!(listener.note(&[1u8, 0b0000_1000]));
        assert_eq!(listener.seen().as_deref(), Some("hid 1 bit 0.3"));
    }

    /// Clearing forgets the previous report too. Otherwise the next learn
    /// would diff against a packet from before the DJ pressed anything and
    /// name a control they touched a minute ago.
    #[test]
    fn clearing_forgets_what_it_was_comparing_against() {
        let listener = Listener::default();
        listener.start();
        listener.note(&[1u8, 0b0000_0000]);
        listener.clear();

        // With the baseline forgotten, this report is a baseline again.
        assert!(listener.note(&[1u8, 0b0000_1000]));
        assert_eq!(listener.seen(), None, "it named a control from one report");
    }

    /// A listener that is not learning consumes nothing, or a controller would
    /// go dead whenever the editor panel had ever been opened.
    #[test]
    fn a_listener_that_is_not_learning_leaves_reports_alone() {
        let listener = Listener::default();
        assert!(!listener.note(&[1u8, 0b0000_1000]));
        listener.start();
        assert!(listener.note(&[1u8, 0b0000_0000]));
        listener.stop();
        assert!(!listener.note(&[1u8, 0b0000_1000]));
    }
}
