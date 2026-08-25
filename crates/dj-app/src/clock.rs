//! djmanzo as MIDI clock master.
//!
//! # What this is for
//!
//! A drum machine, a lighting desk, a second piece of software — anything that
//! counts twenty-four pulses to the beat — follows whatever djmanzo is
//! playing. It is the oldest sync protocol there is and still the one most
//! likely to be on the other end of a DIN cable in a small club.
//!
//! # Three layers, and only the last one touches hardware
//!
//! - [`dj_net::MidiClockOut`] does the arithmetic: how many pulses fall in a
//!   span, carrying the remainder so an awkward interval does not lose a
//!   fraction on every call.
//! - [`ClockDriver`], here, turns tempo and transport into bytes. **No thread,
//!   no clock, no port** — it is handed an elapsed time and a sink, which is
//!   what makes its timing provable in CI rather than by plugging a drum
//!   machine in and listening.
//! - [`MidiClock`] owns the thread and the port.
//!
//! # Why a thread rather than the audio callback
//!
//! Sending MIDI is I/O, and the audio thread does no I/O. A dedicated thread
//! measuring real time is also the *more* accurate of the two: the audio
//! callback fires in buffer-sized lumps — 5.3 ms at 256 frames — and a pulse
//! quantised to that would jitter by up to a whole buffer. This thread wakes
//! on its own schedule.

use dj_control::ParameterRegistry;
use dj_core::{Bpm, GlobalParam, ParamId};
use dj_engine::Command;
use dj_hid::out::{Sink, realtime};
use dj_net::MidiClockOut;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// How often the clock thread wakes.
///
/// At 200 BPM a pulse falls every 12.5 ms, so a 1 ms tick leaves the worst-case
/// quantisation error under a millisecond — comfortably below what any
/// receiver resolves, and far below the jitter of the USB or DIN link it goes
/// out over. Finer would burn a core for nothing.
const RESOLUTION: Duration = Duration::from_millis(1);

/// The tempo used when the room is silent but the clock is running.
///
/// A clock master that stops ticking when the last deck pauses drops every
/// follower out of sync, so the pulses keep going at the last known tempo.
/// This is only used when there has never been one.
const RESTING_BPM: f64 = 120.0;

/// Turns tempo and transport into MIDI bytes.
///
/// Deliberately ignorant of where the time came from and where the bytes go.
#[derive(Debug)]
pub struct ClockDriver {
    out: MidiClockOut,
    /// Whether a `START` has been sent and no `STOP` since.
    running: bool,
    /// The last tempo worth following, so a pause does not stop the pulses.
    tempo: Bpm,
}

impl Default for ClockDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl ClockDriver {
    #[must_use]
    pub fn new() -> Self {
        // 120 is inside `Bpm`'s 20..=400, so this cannot fail; a change to
        // either would be caught by the test below rather than at run time.
        let tempo = Bpm::new(RESTING_BPM).expect("120 BPM is a tempo");
        Self {
            out: MidiClockOut::new(tempo),
            running: false,
            tempo,
        }
    }

    /// Whether transport is currently marked as started.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// The tempo pulses are going out at.
    #[must_use]
    pub fn tempo(&self) -> Bpm {
        self.tempo
    }

    /// Emit whatever `elapsed` is worth into `sink`.
    ///
    /// `room_bpm` is the tempo of the loudest playing deck, or `None` when
    /// nothing is playing. Transport follows it: the first tempo starts the
    /// clock, losing it stops the clock, and finding one again **continues**
    /// rather than starting — `START` means "from the top", and a follower
    /// told to start from the top mid-set jumps to bar one.
    pub fn advance(&mut self, elapsed: Duration, room_bpm: Option<Bpm>, sink: &mut impl Sink) {
        // A tempo change takes effect for the *next* pulse, not the one
        // already in flight -- see `MidiClockOut::set_tempo`.
        if let Some(bpm) = room_bpm
            && bpm != self.tempo
        {
            self.tempo = bpm;
            self.out.set_tempo(bpm);
        }

        match (self.running, room_bpm.is_some()) {
            (false, true) => {
                // Continue rather than start: this is a set already in
                // progress as far as the follower is concerned.
                sink.send(&[realtime::CONTINUE]);
                self.running = true;
            }
            (true, false) => {
                sink.send(&[realtime::STOP]);
                self.running = false;
            }
            _ => {}
        }

        // Pulses keep going whether or not transport is running. A follower
        // that stops hearing them decides it has lost its master; STOP is how
        // it is told the music paused.
        for _ in 0..self.out.advance(elapsed) {
            // One byte per write. A System Realtime byte may legally appear
            // between the bytes of another message, so it is never batched
            // into something else.
            sink.send(&[realtime::CLOCK]);
        }
    }

    /// Say the transport stopped, whatever it was doing.
    ///
    /// For switching the clock off: a follower left running with no master is
    /// a drum machine still going after the DJ has gone home.
    pub fn stop(&mut self, sink: &mut impl Sink) {
        if self.running {
            sink.send(&[realtime::STOP]);
            self.running = false;
        }
    }
}

/// The MIDI clock, as the application switches it on and off.
#[derive(Debug, Default)]
pub struct MidiClock {
    inner: std::sync::Mutex<Option<Running>>,
    error: std::sync::Mutex<Option<String>>,
}

#[derive(Debug)]
struct Running {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    port: String,
}

impl Drop for Running {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// What the interface shows.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ClockStatus {
    pub running: bool,
    /// The port it is sending to, once it is.
    pub port: Option<String>,
    pub error: Option<String>,
    /// The port djmanzo is *following*, when something else is the master.
    pub following: Option<String>,
    /// The tempo that clock is running at, once there are two pulses to
    /// compare. `None` while it is still counting, or when it stops.
    pub external_bpm: Option<f64>,
}

/// Following somebody else's clock.
///
/// The mirror of [`MidiClock`]. Kept apart because they are genuinely
/// independent: a DJ can be the master for a light desk while following a
/// drum machine, and the two are different cables.
#[derive(Debug, Default)]
pub struct ClockFollow {
    inner: std::sync::Mutex<Option<dj_hid::port::ClockConnection>>,
    /// The last tempo heard, for the interface to show.
    heard: Arc<std::sync::Mutex<Option<f64>>>,
    error: std::sync::Mutex<Option<String>>,
}

impl ClockFollow {
    /// Follow the MIDI clock arriving on `port`.
    ///
    /// Every estimate is sent to the engine as [`Command::SetExternalTempo`],
    /// which outranks every deck as the sync leader: a DJ who plugged the
    /// room's clock in wants the room's clock.
    ///
    /// # Errors
    /// When MIDI is unavailable, no input matches, or the port refuses.
    pub fn start(
        &self,
        port: &str,
        bus: Arc<dj_control::ActionBus<Command>>,
    ) -> Result<(), String> {
        // Stopped without telling the engine: a new tempo is a pulse away, and
        // clearing it first would unlock every synced deck for that moment.
        *self.inner.lock().unwrap() = None;
        let heard = Arc::clone(&self.heard);

        let connection = dj_hid::port::listen_to_clock(port, move |tempo| {
            if let Ok(mut slot) = heard.lock() {
                *slot = tempo.map(Bpm::get);
            }
            // A full queue means the engine is behind; the next pulse is
            // 20 ms away and carries the same news, so dropping this one
            // costs nothing. Blocking the MIDI thread would cost a lot.
            let _ = bus.send_command(Command::SetExternalTempo {
                bpm: tempo.map(Bpm::get),
            });
        })
        .map_err(|e| {
            let why = e.to_string();
            *self.error.lock().unwrap() = Some(why.clone());
            why
        })?;

        *self.inner.lock().unwrap() = Some(connection);
        *self.error.lock().unwrap() = None;
        Ok(())
    }

    /// Stop following, and tell the engine the room has no external tempo.
    ///
    /// Without the second half a synced deck would stay locked to the tempo of
    /// a clock that is no longer plugged in.
    pub fn stop(&self, bus: Option<&dj_control::ActionBus<Command>>) {
        *self.inner.lock().unwrap() = None;
        if let Ok(mut slot) = self.heard.lock() {
            *slot = None;
        }
        if let Some(bus) = bus {
            let _ = bus.send_command(Command::SetExternalTempo { bpm: None });
        }
    }

    #[must_use]
    pub fn port(&self) -> Option<String> {
        self.inner
            .lock()
            .ok()?
            .as_ref()
            .map(|c| c.port().to_owned())
    }

    #[must_use]
    pub fn heard(&self) -> Option<f64> {
        *self.heard.lock().ok()?
    }

    #[must_use]
    pub fn error(&self) -> Option<String> {
        self.error.lock().ok()?.clone()
    }
}

impl MidiClock {
    /// Start sending clock to the MIDI output whose name contains `port`.
    ///
    /// # Errors
    /// When MIDI is unavailable, no output matches, or the port refuses.
    pub fn start(
        &self,
        port: &str,
        registry: Arc<ParameterRegistry>,
    ) -> Result<ClockStatus, String> {
        // Stopped first, so restarting on the same port does not fail against
        // the copy of itself still holding it open.
        self.stop();

        let sink = dj_hid::out::open(port).map_err(|e| {
            let why = e.to_string();
            *self.error.lock().unwrap() = Some(why.clone());
            why
        })?;
        let name = sink.name().to_owned();

        let stop = Arc::new(AtomicBool::new(false));
        let thread = std::thread::Builder::new()
            .name("djmanzo-midi-clock".into())
            .spawn({
                let stop = Arc::clone(&stop);
                move || pump(sink, &registry, &stop)
            })
            .map_err(|e| e.to_string())?;

        *self.inner.lock().unwrap() = Some(Running {
            stop,
            thread: Some(thread),
            port: name.clone(),
        });
        *self.error.lock().unwrap() = None;
        Ok(ClockStatus {
            running: true,
            port: Some(name),
            error: None,
            following: None,
            external_bpm: None,
        })
    }

    /// Stop sending. Stopping nothing is not an error.
    pub fn stop(&self) {
        *self.inner.lock().unwrap() = None;
    }

    /// What both halves are doing, as one thing for the interface to show.
    #[must_use]
    pub fn status(&self, follow: &ClockFollow) -> ClockStatus {
        let inner = self.inner.lock().unwrap();
        ClockStatus {
            running: inner.is_some(),
            port: inner.as_ref().map(|r| r.port.clone()),
            // Sending and following fail independently, so whichever has
            // something to say gets to say it.
            error: self
                .error
                .lock()
                .unwrap()
                .clone()
                .or_else(|| follow.error()),
            following: follow.port(),
            external_bpm: follow.heard(),
        }
    }
}

/// The clock thread.
fn pump(mut sink: impl Sink, registry: &ParameterRegistry, stop: &AtomicBool) {
    let mut driver = ClockDriver::new();
    let mut last = Instant::now();

    while !stop.load(Ordering::Acquire) {
        std::thread::sleep(RESOLUTION);
        let now = Instant::now();
        let elapsed = now.duration_since(last);
        last = now;

        let bpm = registry.get(ParamId::Global(GlobalParam::MasterBpm));
        // A zero here means nothing is playing, and `Bpm::new` refuses it
        // along with anything else outside 20..=400 -- which is exactly the
        // "is there something to be in time with" question.
        driver.advance(elapsed, Bpm::new(f64::from(bpm)), &mut sink);
    }

    // Never leave a follower running with nobody driving it.
    driver.stop(&mut sink);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Default)]
    struct Recorder(Vec<u8>);

    impl Sink for Recorder {
        fn send(&mut self, message: &[u8]) {
            self.0.extend_from_slice(message);
        }
    }

    impl Recorder {
        fn pulses(&self) -> usize {
            self.0.iter().filter(|b| **b == realtime::CLOCK).count()
        }
        fn transport(&self) -> Vec<u8> {
            self.0
                .iter()
                .copied()
                .filter(|b| *b != realtime::CLOCK)
                .collect()
        }
    }

    fn bpm(value: f64) -> Bpm {
        Bpm::new(value).expect("a real tempo")
    }

    /// **The number that matters.** Twenty-four pulses to the quarter note is
    /// not a convention djmanzo chose; a follower counts them and a wrong
    /// count is a drum machine at the wrong tempo.
    #[test]
    fn one_minute_at_120_bpm_is_exactly_2880_pulses() {
        let mut driver = ClockDriver::new();
        let mut sink = Recorder::default();
        // A minute, in the awkward lumps a real thread wakes in.
        for _ in 0..60_000 {
            driver.advance(Duration::from_millis(1), Some(bpm(120.0)), &mut sink);
        }
        // 120 beats a minute x 24 pulses a beat.
        assert_eq!(sink.pulses(), 2_880);
    }

    /// The remainder has to be carried, or every awkward interval loses a
    /// fraction and the clock drifts flat over an evening.
    ///
    /// 5.333 ms is 256 frames at 48 kHz, which is the interval that made this
    /// worth writing down.
    #[test]
    fn an_awkward_interval_does_not_lose_a_fraction_each_time() {
        let mut driver = ClockDriver::new();
        let mut sink = Recorder::default();
        let odd = Duration::from_nanos(5_333_333);
        // Ten minutes of them.
        let steps = (600.0 / odd.as_secs_f64()).round() as usize;
        for _ in 0..steps {
            driver.advance(odd, Some(bpm(128.0)), &mut sink);
        }
        // 128 BPM x 24 = 3072 pulses a minute, ten minutes.
        let expected = 30_720;
        let drift = (sink.pulses() as i64 - expected).abs();
        assert!(
            drift <= 1,
            "drifted {drift} pulses over ten minutes ({} vs {expected})",
            sink.pulses()
        );
    }

    /// Finding a tempo **continues**; it does not start. `START` means "from
    /// the top", and a follower told that mid-set jumps to bar one.
    #[test]
    fn the_first_tempo_continues_rather_than_starting_from_the_top() {
        let mut driver = ClockDriver::new();
        let mut sink = Recorder::default();
        driver.advance(Duration::from_millis(1), Some(bpm(128.0)), &mut sink);
        assert_eq!(sink.transport(), vec![realtime::CONTINUE]);
        assert!(driver.is_running());
        assert!(
            !sink.0.contains(&realtime::START),
            "a set already in progress was restarted from the top"
        );
    }

    /// Losing the room's tempo stops transport — and the pulses keep going,
    /// because a follower that stops hearing them decides it has lost its
    /// master altogether.
    #[test]
    fn a_pause_stops_transport_but_not_the_pulses() {
        let mut driver = ClockDriver::new();
        let mut sink = Recorder::default();
        driver.advance(Duration::from_millis(10), Some(bpm(120.0)), &mut sink);

        let before = sink.pulses();
        for _ in 0..500 {
            driver.advance(Duration::from_millis(1), None, &mut sink);
        }
        assert_eq!(sink.transport(), vec![realtime::CONTINUE, realtime::STOP]);
        assert!(
            sink.pulses() > before,
            "the pulses stopped when the music paused"
        );
        // At the last known tempo, not at some default.
        assert_eq!(driver.tempo(), bpm(120.0));
    }

    /// Transport is said once per change, not once per wake-up. Fifty pulses a
    /// second of `STOP` is a follower being told to stop fifty times.
    #[test]
    fn transport_is_said_once_per_change() {
        let mut driver = ClockDriver::new();
        let mut sink = Recorder::default();
        for _ in 0..100 {
            driver.advance(Duration::from_millis(1), Some(bpm(128.0)), &mut sink);
        }
        for _ in 0..100 {
            driver.advance(Duration::from_millis(1), None, &mut sink);
        }
        for _ in 0..100 {
            driver.advance(Duration::from_millis(1), Some(bpm(128.0)), &mut sink);
        }
        assert_eq!(
            sink.transport(),
            vec![realtime::CONTINUE, realtime::STOP, realtime::CONTINUE]
        );
    }

    /// A tempo change is followed, and the pulse rate changes with it.
    #[test]
    fn a_tempo_change_changes_the_pulse_rate() {
        let mut slow = Recorder::default();
        let mut driver = ClockDriver::new();
        for _ in 0..10_000 {
            driver.advance(Duration::from_millis(1), Some(bpm(60.0)), &mut slow);
        }

        let mut fast = Recorder::default();
        let mut driver = ClockDriver::new();
        for _ in 0..10_000 {
            driver.advance(Duration::from_millis(1), Some(bpm(180.0)), &mut fast);
        }

        // Three times the tempo is three times the pulses, within a pulse.
        let ratio = fast.pulses() as f64 / slow.pulses() as f64;
        assert!(
            (ratio - 3.0).abs() < 0.01,
            "60 BPM gave {} pulses and 180 gave {}, a ratio of {ratio}",
            slow.pulses(),
            fast.pulses()
        );
    }

    /// Switching the clock off must stop the follower, or a drum machine keeps
    /// going after the DJ has gone home.
    #[test]
    fn stopping_tells_the_follower_rather_than_going_quiet() {
        let mut driver = ClockDriver::new();
        let mut sink = Recorder::default();
        driver.advance(Duration::from_millis(10), Some(bpm(120.0)), &mut sink);
        driver.stop(&mut sink);
        assert_eq!(sink.transport(), vec![realtime::CONTINUE, realtime::STOP]);
        // And saying it twice says it once.
        driver.stop(&mut sink);
        assert_eq!(sink.transport(), vec![realtime::CONTINUE, realtime::STOP]);
    }

    /// The resting tempo has to be a tempo, or `ClockDriver::new` panics on
    /// the first construction rather than at a comfortable moment.
    #[test]
    fn the_resting_tempo_is_inside_what_bpm_allows() {
        assert!(
            Bpm::new(RESTING_BPM).is_some(),
            "{RESTING_BPM} is not a tempo"
        );
        assert_eq!(ClockDriver::new().tempo(), bpm(RESTING_BPM));
    }

    #[test]
    fn nothing_is_sending_until_it_is_asked_to() {
        let clock = MidiClock::default();
        let follow = ClockFollow::default();
        let status = clock.status(&follow);
        assert!(!status.running);
        assert_eq!(status.port, None);
        assert_eq!(
            status.following, None,
            "it was following before it was asked"
        );
        assert_eq!(status.external_bpm, None);
    }

    /// Following a port that is not there says which, and leaves nothing
    /// half-connected behind it.
    #[test]
    fn following_a_port_that_is_not_there_says_so() {
        let (bus, _engine) = dj_control::ActionBus::<Command>::new(16);
        let follow = ClockFollow::default();
        let why = follow
            .start("not-a-real-midi-input-anywhere", Arc::new(bus))
            .expect_err("there is no such port");
        assert!(!why.is_empty());
        assert_eq!(follow.port(), None, "it attached anyway");
        assert_eq!(follow.error().as_deref(), Some(&*why));
    }

    /// **Unplugging the clock has to reach the engine.** Without the command,
    /// a synced deck stays locked to the tempo of a clock nobody is sending
    /// any more, and no amount of looking at the interface explains why.
    #[test]
    fn giving_up_the_clock_tells_the_engine_the_room_has_no_tempo() {
        let (bus, mut engine) = dj_control::ActionBus::<Command>::new(16);
        let follow = ClockFollow::default();

        follow.stop(Some(&bus));

        match engine.pop() {
            Ok(Command::SetExternalTempo { bpm: None }) => {}
            other => panic!("the engine was not told: {other:?}"),
        }
    }

    /// A port that is not there says which, rather than opening whatever
    /// happened to be first.
    #[test]
    fn starting_on_a_port_that_is_not_there_says_so() {
        let clock = MidiClock::default();
        let why = clock
            .start(
                "not-a-real-midi-output-anywhere",
                Arc::new(ParameterRegistry::new()),
            )
            .expect_err("there is no such port");
        assert!(!why.is_empty());
        let follow = ClockFollow::default();
        assert!(!clock.status(&follow).running, "it started anyway");
        assert_eq!(clock.status(&follow).error.as_deref(), Some(&*why));
    }
}
