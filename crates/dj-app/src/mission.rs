//! §5's Mission Bar: the live state a DJ glances at, gathered once.
//!
//! > Always-present but compact. Contains only the most important live state
//! > […] It should behave more like an aircraft HUD than a conventional
//! > application toolbar. Do not fill it with menus.
//!
//! # Why this is a module and not a component
//!
//! Every reading on the bar already existed somewhere in djmanzo before §5 was
//! built: the phase in the Night panel, the occasion and the posture in the
//! assistant, the room in its own surface, the sample rate and the load in a
//! strip in the top bar. The bar is not new information — it is the *gathering*,
//! and the failure mode of a gathering written in the interface is that each
//! reading gets its own threshold, its own wording, and its own idea of when
//! something is worth a colour. So the whole bar is decided here, in one pass
//! over one reading, and the interface draws what comes back.
//!
//! That is the same split the density bands and `dj_app::mood` use: Rust owns
//! the rule, the interface owns the pixels.
//!
//! # What a HUD is for
//!
//! An instrument panel is read in the half second between two other things, so
//! the design rule is that **the normal state of every item is quiet**. An item
//! earns a colour by being worth interrupting a mix for, and nothing else does.
//! A bar where three things are always amber is a bar nobody looks at, which is
//! the failure §5 is warning about when it says *do not fill it with menus*.

use serde::Serialize;

/// How loudly one reading speaks.
///
/// Ordered, so "the loudest thing on the bar" is a `max` rather than a chain of
/// comparisons each consumer writes for itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Level {
    /// The normal state of everything. Read, not noticed.
    Quiet,
    /// Worth a glance at the end of this record.
    Watch,
    /// Worth interrupting a mix for.
    Alarm,
}

impl Level {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Quiet => "quiet",
            Self::Watch => "watch",
            Self::Alarm => "alarm",
        }
    }
}

/// One reading on the bar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Item {
    /// Which of §5's readings this is. Stable, so the interface can style and
    /// a test can find it without matching on words.
    pub slug: &'static str,
    /// The short word in front of the value, or empty when the value says it.
    pub label: &'static str,
    /// What it reads. Short: this is a HUD, not a sentence.
    pub value: String,
    pub level: Level,
    /// The whole of it, for a hover. Where the sentence goes.
    pub about: String,
}

/// §5's list of what belongs on the bar, and whether djmanzo can answer it.
///
/// Written out and checked in both directions, the way `dj_render::layer`
/// handles §25's twenty: a reading djmanzo cannot produce is **named absent**
/// rather than quietly missing, so the gap is a fact in a table instead of
/// something a future session has to rediscover by re-reading the directive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Asked {
    /// Where the set is in its arc.
    Phase,
    /// What kind of night the DJ said this is.
    Occasion,
    /// How much the assistant is allowed to do.
    Posture,
    /// What the floor is doing. §39's arrow.
    Room,
    /// The master bus: how hard it is being driven, and whether it is dropping.
    Output,
    /// Whether the set is being recorded.
    Recording,
    /// The tempo being played at.
    Tempo,
    /// How long the night has been running.
    Clock,
    /// Compact alerts.
    Alerts,
    /// Which sound card, at what rate and latency.
    Device,
    /// Whether the machine is keeping up.
    Health,
}

impl Asked {
    pub const ALL: &'static [Asked] = &[
        Asked::Phase,
        Asked::Occasion,
        Asked::Posture,
        Asked::Room,
        Asked::Output,
        Asked::Recording,
        Asked::Tempo,
        Asked::Clock,
        Asked::Alerts,
        Asked::Device,
        Asked::Health,
    ];

    /// The slug the item carries when it is on the bar.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Phase => "phase",
            Self::Occasion => "occasion",
            Self::Posture => "posture",
            Self::Room => "room",
            Self::Output => "output",
            Self::Recording => "recording",
            Self::Tempo => "tempo",
            Self::Clock => "clock",
            Self::Alerts => "alerts",
            Self::Device => "device",
            Self::Health => "health",
        }
    }

    /// §5's own words for it.
    #[must_use]
    pub const fn about(self) -> &'static str {
        match self {
            Self::Phase => "current session phase",
            Self::Occasion => "current occasion",
            Self::Posture => "AI posture",
            Self::Room => "room status",
            Self::Output => "master/output health",
            Self::Recording => "recording state",
            Self::Tempo => "current tempo",
            Self::Clock => "current time / set duration",
            Self::Alerts => "compact alerts",
            Self::Device => "hardware/device status",
            Self::Health => "performance confidence/warnings",
        }
    }

    /// Whether [`bar`] can ever produce this reading.
    ///
    /// Ten of the eleven, and the eleventh is a decision rather than a gap.
    /// **Alerts is deliberately not an item.** A "compact alert" is what an
    /// item at [`Level::Alarm`] already *is*, and a twelfth entry restating the
    /// other ten's warnings would be the two-descriptions trap this codebase
    /// keeps walking into: the day one of them changed its threshold, the bar
    /// would carry a warning and a summary of that warning that disagreed.
    #[must_use]
    pub const fn shows(self) -> bool {
        !matches!(self, Self::Alerts)
    }
}

/// The master bus is being driven this hard before the bar says so, in dB of
/// limiter reduction.
///
/// A limiter doing a decibel of work on a peak is a limiter doing its job. Six
/// is a mix that is louder than the chain can carry, which is audible as
/// flattening rather than as distortion — the exact failure a meter reading
/// post-limiter cannot show, and therefore the one worth a HUD entry.
pub const DRIVEN_HARD_DB: f32 = 6.0;

/// The load at which the machine is worth watching, and the one at which it is
/// worth acting on.
///
/// 0.7 is the figure the interface already used for its CPU readout, kept
/// rather than a second opinion invented beside it. 0.9 is where the audio
/// thread has a quarter of a buffer of headroom left, which is where a dropout
/// stops being a possibility and becomes a matter of time.
pub const BUSY: f32 = 0.7;
pub const OVERRUN: f32 = 0.9;

/// Everything the bar is built from, in one place.
///
/// A plain struct rather than a pile of arguments, and with no handle on any
/// application state, so every rule below is testable without an audio device,
/// a night or a camera — which matters because this container has none of them.
#[derive(Debug, Clone, Default)]
pub struct Reading {
    /// The phase as it appears mid-sentence, and how much to believe it.
    pub phase: Option<String>,
    pub phase_certainty: Option<String>,
    /// What the DJ said the night is.
    pub occasion: String,
    /// How much the assistant is allowed to do, and what that means.
    pub posture: String,
    pub posture_about: String,
    /// §39's arrow, already judged. `(mark, way, agreeing, of, says)`.
    pub room: Option<RoomRead>,
    /// The master bus.
    pub cpu: f32,
    pub xruns: f32,
    pub limiter_db: f32,
    /// Seconds of set recorded, when one is running, and whether it is broken.
    pub recording: Option<Recording>,
    /// The tempo of the record that is playing, when exactly one is.
    pub tempo: Option<f32>,
    /// Seconds since the night started.
    pub elapsed: Option<f64>,
    /// The open device.
    pub device: Option<Device>,
}

/// §39's arrow, as the bar receives it.
#[derive(Debug, Clone, Default)]
pub struct RoomRead {
    pub mark: String,
    pub way: String,
    pub agreeing: usize,
    pub of: usize,
    pub says: String,
}

#[derive(Debug, Clone, Default)]
pub struct Recording {
    pub seconds: f64,
    /// Samples that never reached the disk.
    pub dropped: u64,
    /// The writer gave up — a full disk, usually.
    pub failed: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Device {
    pub name: String,
    pub sample_rate: f32,
    pub latency_ms: f32,
}

/// Minutes and seconds, as a clock reads them.
///
/// Not `api.ts`'s `formatTime`, and deliberately: that one is a record's
/// position, which never passes an hour, and this one is a night's length,
/// which does by about half past midnight. `61:03` is a correct reading of a
/// track and a wrong reading of a set.
fn clock(seconds: f64) -> String {
    let whole = seconds.max(0.0) as u64;
    let (hours, minutes, seconds) = (whole / 3600, (whole % 3600) / 60, whole % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

/// The bar, in the order §5 lists it.
///
/// **Order is the design.** An instrument panel is learned by position — a
/// pilot's eye goes to where the reading was last time, not to where the label
/// is — so the sequence is fixed and an item that cannot be read is *absent*
/// rather than reordered around. The one thing that would break that is an item
/// appearing in the middle when something goes wrong, which is why recording
/// and the room sit at the ends of what they belong to rather than being
/// inserted wherever they become true.
#[must_use]
pub fn bar(read: &Reading) -> Vec<Item> {
    let mut items = Vec::new();

    if let Some(phase) = &read.phase {
        // An uncertain phase is still worth showing and is not worth a colour:
        // §9 keeps certainty and autonomy apart, and a HUD that went amber
        // every time the night was ambiguous would be amber most of a warm-up.
        let sure = read.phase_certainty.as_deref().unwrap_or("unsure");
        items.push(Item {
            slug: Asked::Phase.slug(),
            label: "",
            value: phase.clone(),
            level: Level::Quiet,
            about: format!("The set reads as {phase}. Certainty: {sure}."),
        });
    }

    if !read.occasion.is_empty() {
        items.push(Item {
            slug: Asked::Occasion.slug(),
            label: "",
            value: read.occasion.replace('_', " "),
            level: Level::Quiet,
            about: format!(
                "You said this is a {} night.",
                read.occasion.replace('_', " ")
            ),
        });
    }

    if !read.posture.is_empty() {
        items.push(Item {
            slug: Asked::Posture.slug(),
            label: "AI",
            value: read.posture.clone(),
            level: Level::Quiet,
            about: if read.posture_about.is_empty() {
                format!("The assistant is on {}.", read.posture)
            } else {
                format!(
                    "The assistant is on {}. {}",
                    read.posture, read.posture_about
                )
            },
        });
    }

    // §39's arrow, already decided. Passed through rather than re-read: the
    // reading on the bar and the panel behind it are one judgement seen twice.
    //
    // **Always on the bar, even with nothing to report**, which is the one item
    // here that is drawn rather than derived. Two reasons, and the second is
    // the load-bearing one: an em dash is the honest reading of a room nobody
    // is looking at, where STABLE would be a claim about a floor djmanzo has
    // never seen; and this is the way into the room panel, which nothing can be
    // watching from until somebody has opened and aimed a camera. A room
    // reachable only from its own reading would be reachable from nowhere.
    let (mark, says) = match &read.room {
        Some(room) => (
            room.mark.clone(),
            format!(
                "{} {} of {} {} this way.",
                room.says,
                room.agreeing,
                room.of,
                if room.of == 1 {
                    "sense reads"
                } else {
                    "senses read"
                }
            ),
        ),
        None => ("—".to_owned(), "Nothing is watching the room.".to_owned()),
    };
    items.push(Item {
        slug: Asked::Room.slug(),
        label: "ROOM",
        value: mark,
        level: Level::Quiet,
        about: says,
    });

    if let Some(tempo) = read.tempo {
        items.push(Item {
            slug: Asked::Tempo.slug(),
            label: "",
            value: format!("{tempo:.1} BPM"),
            level: Level::Quiet,
            about: format!("The record playing is at {tempo:.1} BPM."),
        });
    }

    if let Some(elapsed) = read.elapsed {
        items.push(Item {
            slug: Asked::Clock.slug(),
            label: "",
            value: clock(elapsed),
            level: Level::Quiet,
            about: format!("The night has been running {}.", clock(elapsed)),
        });
    }

    // A recording is state a DJ has to be able to confirm from across a booth,
    // and a recording that is *failing* is the one thing on this bar that
    // cannot wait for the end of the record: the file being written is the only
    // copy of a set that is already half over.
    if let Some(rec) = &read.recording {
        let (level, about) = if rec.failed {
            (
                Level::Alarm,
                "The recording stopped writing. What is on disk ends where it stopped.".to_owned(),
            )
        } else if rec.dropped > 0 {
            (
                Level::Alarm,
                format!(
                    "{} samples never reached the disk, so the file has a gap in it.",
                    rec.dropped
                ),
            )
        } else {
            (
                Level::Quiet,
                format!("Recording the set. {} so far.", clock(rec.seconds)),
            )
        };
        items.push(Item {
            slug: Asked::Recording.slug(),
            label: "REC",
            value: clock(rec.seconds),
            level,
            about,
        });
    }

    // The master bus. Quiet until it is not: a limiter working within itself
    // and no dropouts is the state of a healthy night, and saying so in colour
    // every night is how a bar stops being read.
    let (out_level, out_value, out_about) = if read.xruns > 0.0 {
        (
            Level::Alarm,
            format!("{} dropouts", read.xruns as u64),
            format!(
                "{} times the audio thread missed its deadline. That is audible.",
                read.xruns as u64
            ),
        )
    } else if read.limiter_db >= DRIVEN_HARD_DB {
        (
            Level::Watch,
            format!("-{:.0} dB", read.limiter_db),
            format!(
                "The limiter is taking {:.1} dB off the mix. Past about {DRIVEN_HARD_DB:.0} that \
                 flattens rather than protects -- the master meter cannot show it, because it \
                 reads after the limiter.",
                read.limiter_db
            ),
        )
    } else {
        (
            Level::Quiet,
            "clean".to_owned(),
            "No dropouts, and the limiter is working within itself.".to_owned(),
        )
    };
    items.push(Item {
        slug: Asked::Output.slug(),
        label: "OUT",
        value: out_value,
        level: out_level,
        about: out_about,
    });

    if let Some(device) = &read.device {
        items.push(Item {
            slug: Asked::Device.slug(),
            label: "",
            value: format!("{:.0} kHz", device.sample_rate / 1000.0),
            level: Level::Quiet,
            about: format!(
                "Playing out of {} at {:.0} kHz, {:.1} ms.",
                device.name,
                device.sample_rate / 1000.0,
                device.latency_ms
            ),
        });
    } else {
        // Louder than any reading, because it is not a reading: nothing is
        // open, so nothing on the bar above it means anything yet.
        items.push(Item {
            slug: Asked::Device.slug(),
            label: "",
            value: "no device".to_owned(),
            level: Level::Watch,
            about: "No sound card is open. Nothing will be heard until one is.".to_owned(),
        });
    }

    let health = if read.cpu >= OVERRUN {
        Level::Alarm
    } else if read.cpu >= BUSY {
        Level::Watch
    } else {
        Level::Quiet
    };
    items.push(Item {
        slug: Asked::Health.slug(),
        label: "CPU",
        value: format!("{:.0}%", read.cpu * 100.0),
        level: health,
        about: match health {
            Level::Alarm => format!(
                "The audio thread is using {:.0}% of its time. A dropout is a matter of when.",
                read.cpu * 100.0
            ),
            Level::Watch => format!(
                "The audio thread is using {:.0}% of its time. Worth a larger buffer.",
                read.cpu * 100.0
            ),
            Level::Quiet => format!(
                "The audio thread is using {:.0}% of its time.",
                read.cpu * 100.0
            ),
        },
    });

    items
}

/// The loudest thing on the bar, for anything that needs one number.
#[must_use]
pub fn loudest(items: &[Item]) -> Level {
    items
        .iter()
        .map(|item| item.level)
        .max()
        .unwrap_or(Level::Quiet)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A night in good health: an open device, a mix inside the limiter, no
    /// dropouts and a machine with room to spare.
    fn healthy() -> Reading {
        Reading {
            phase: Some("peak".to_owned()),
            phase_certainty: Some("sure".to_owned()),
            occasion: "club".to_owned(),
            posture: "suggest".to_owned(),
            posture_about: "Offers, with reasons. Never acts.".to_owned(),
            room: Some(RoomRead {
                mark: "STABLE".to_owned(),
                way: "steady".to_owned(),
                agreeing: 3,
                of: 3,
                says: "The room is holding where it has been for the last twenty minutes."
                    .to_owned(),
            }),
            cpu: 0.2,
            xruns: 0.0,
            limiter_db: 0.5,
            recording: None,
            tempo: Some(128.0),
            elapsed: Some(3720.0),
            device: Some(Device {
                name: "Pioneer DJM".to_owned(),
                sample_rate: 48_000.0,
                latency_ms: 5.3,
            }),
        }
    }

    /// A night in which every reading djmanzo can make is available.
    ///
    /// Separate from [`healthy`] because a healthy night is usually *not*
    /// recording, and the question "can the bar produce this reading at all" is
    /// a different one from "does a normal night show it".
    fn everything() -> Reading {
        Reading {
            recording: Some(Recording {
                seconds: 1830.0,
                dropped: 0,
                failed: false,
            }),
            ..healthy()
        }
    }

    fn find<'a>(items: &'a [Item], slug: &str) -> Option<&'a Item> {
        items.iter().find(|item| item.slug == slug)
    }

    /// **The bar is quiet when nothing is wrong.**
    ///
    /// The load-bearing test, and the one that would be easiest to lose: every
    /// individual rule below is about when an item *speaks up*, and a bar where
    /// each of those is a little too eager is a bar with three amber items on a
    /// normal night — which is a bar a DJ stops reading, which is the whole
    /// failure §5 is warning about.
    #[test]
    fn a_healthy_night_lights_nothing_up() {
        let items = bar(&healthy());
        let loud: Vec<_> = items
            .iter()
            .filter(|item| item.level != Level::Quiet)
            .map(|item| format!("{} ({})", item.slug, item.value))
            .collect();
        assert!(
            loud.is_empty(),
            "a night with nothing wrong with it lit up: {loud:?}"
        );
        assert_eq!(loudest(&items), Level::Quiet);
    }

    /// Every reading §5 asks for is on the bar, or is named as absent.
    #[test]
    fn every_reading_the_directive_asks_for_is_answered_or_named() {
        let items = bar(&everything());
        for asked in Asked::ALL {
            let on_bar = find(&items, asked.slug()).is_some();
            assert_eq!(
                on_bar,
                asked.shows(),
                "§5 asks for `{}` ({}); the table says shows() = {} and the bar {}",
                asked.slug(),
                asked.about(),
                asked.shows(),
                if on_bar { "has it" } else { "does not" }
            );
        }
        assert_eq!(
            Asked::ALL.len(),
            11,
            "§5 lists eleven readings; this table has drifted from the section"
        );
        assert!(
            !Asked::Alerts.shows(),
            "an alerts item appeared -- an alarm on an item is already the alert"
        );
    }

    /// Every item has its own slug, so the interface can tell them apart.
    #[test]
    fn every_reading_has_its_own_slug() {
        let mut seen = std::collections::BTreeSet::new();
        for asked in Asked::ALL {
            assert!(
                seen.insert(asked.slug()),
                "two readings are `{}`",
                asked.slug()
            );
        }
        for item in bar(&everything()) {
            assert!(
                Asked::ALL.iter().any(|a| a.slug() == item.slug),
                "the bar carries `{}`, which is not one of §5's readings",
                item.slug
            );
            assert!(!item.about.is_empty(), "`{}` says nothing", item.slug);
        }
    }

    /// **A dropout outranks a hot limiter, and both outrank a clean bus.**
    ///
    /// One slot, three states, and the order matters: a mix being flattened is
    /// worth knowing about and a mix with holes in it is worth stopping for, so
    /// the second must never be hidden behind the first.
    #[test]
    fn the_output_says_the_worst_thing_that_is_true_of_it() {
        let clean = bar(&healthy());
        assert_eq!(find(&clean, "output").expect("output").level, Level::Quiet);

        let mut hot = healthy();
        hot.limiter_db = DRIVEN_HARD_DB + 2.0;
        let hot = bar(&hot);
        assert_eq!(find(&hot, "output").expect("output").level, Level::Watch);

        // Both at once: the dropouts are the thing to say.
        let mut broken = healthy();
        broken.limiter_db = DRIVEN_HARD_DB + 2.0;
        broken.xruns = 3.0;
        let broken = bar(&broken);
        let out = find(&broken, "output").expect("output");
        assert_eq!(out.level, Level::Alarm);
        assert!(
            out.value.contains("dropout"),
            "a bus that is dropping audio reported the limiter instead: {}",
            out.value
        );
    }

    /// A recording that is losing samples is an alarm; one that is fine is not.
    ///
    /// The file being written is the only copy of a set that is already half
    /// over, which is why this is the one reading allowed to shout.
    #[test]
    fn a_recording_only_shouts_when_it_is_losing_the_set() {
        let mut running = healthy();
        running.recording = Some(Recording {
            seconds: 900.0,
            dropped: 0,
            failed: false,
        });
        let items = bar(&running);
        let rec = find(&items, "recording").expect("recording");
        assert_eq!(rec.level, Level::Quiet);
        assert_eq!(rec.value, "15:00");

        let mut gapped = healthy();
        gapped.recording = Some(Recording {
            seconds: 900.0,
            dropped: 4096,
            failed: false,
        });
        assert_eq!(
            find(&bar(&gapped), "recording").expect("recording").level,
            Level::Alarm
        );

        let mut dead = healthy();
        dead.recording = Some(Recording {
            seconds: 900.0,
            dropped: 0,
            failed: true,
        });
        assert_eq!(
            find(&bar(&dead), "recording").expect("recording").level,
            Level::Alarm
        );
    }

    /// No device is not a quiet state.
    ///
    /// It is the one condition under which every other reading on the bar is
    /// about a mix nobody can hear, so it is the one absence that speaks.
    #[test]
    fn no_sound_card_is_worth_saying_out_loud() {
        let mut silent = healthy();
        silent.device = None;
        let items = bar(&silent);
        let device = find(&items, "device").expect("device");
        assert_eq!(device.level, Level::Watch);
        assert!(device.value.contains("no device"));
    }

    /// The load thresholds are the ones the interface already used.
    #[test]
    fn the_machine_speaks_up_at_the_load_the_interface_already_used() {
        let mut easy = healthy();
        easy.cpu = BUSY - 0.01;
        assert_eq!(
            find(&bar(&easy), "health").expect("health").level,
            Level::Quiet
        );

        let mut busy = healthy();
        busy.cpu = BUSY;
        assert_eq!(
            find(&bar(&busy), "health").expect("health").level,
            Level::Watch
        );

        let mut over = healthy();
        over.cpu = OVERRUN;
        assert_eq!(
            find(&bar(&over), "health").expect("health").level,
            Level::Alarm
        );
    }

    /// A reading djmanzo does not have is left out rather than filled in.
    ///
    /// A bar that showed "—" for the phase, the occasion, the room, the tempo
    /// and the clock on a fresh install would be five slots of nothing with the
    /// two that matter lost among them.
    #[test]
    fn a_fresh_install_shows_only_what_it_can_actually_read() {
        let items = bar(&Reading {
            cpu: 0.1,
            ..Reading::default()
        });
        let slugs: Vec<_> = items.iter().map(|item| item.slug).collect();
        assert_eq!(
            slugs,
            vec!["room", "output", "device", "health"],
            "a djmanzo that has read nothing is claiming to have read something"
        );
        // And the room is there saying it has nothing, rather than claiming the
        // floor is steady. It is also the only way into the room panel.
        let room = find(&items, "room").expect("the room is always on the bar");
        assert_eq!(room.value, "—");
        assert_eq!(room.level, Level::Quiet);
        assert!(room.about.contains("Nothing is watching"));
    }

    /// The harness answers with the bar this module would build.
    ///
    /// `ui/e2e/shell.ts` types out the three-item bar of a shell with no
    /// device open, and every browser assertion about the HUD is a measurement
    /// of that stub unless it says what Rust says. The same lesson the density
    /// bands record, where the copy went stale twice.
    #[test]
    fn the_harness_and_rust_agree_about_an_unopened_bar() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../ui/e2e/shell.ts");
        let source = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("could not read the browser harness at {path}: {e}"));
        let table = source
            .split_once("mission_bar: [")
            .and_then(|(_, rest)| rest.split_once("\n  ],"))
            .map(|(inside, _)| inside)
            .expect("`mission_bar: [` ... `\n  ],` is no longer how the harness writes it");

        let ours = bar(&Reading::default());
        assert_eq!(
            ours.len(),
            4,
            "the bar built from an empty reading used to have four items and now \
             has {} -- the harness is describing a different application",
            ours.len()
        );
        for item in &ours {
            for (what, wanted) in [
                ("slug", format!("slug: \"{}\",", item.slug)),
                ("value", format!("value: \"{}\",", item.value)),
                ("level", format!("level: \"{}\",", item.level.name())),
                ("about", format!("about: \"{}\",", item.about)),
            ] {
                assert!(
                    table.contains(&wanted),
                    "the harness gives `{}` a different {what} from this module -- \
                     it wants `{wanted}`",
                    item.slug
                );
            }
        }
    }

    /// An hour in reads as an hour, not as sixty-one minutes.
    #[test]
    fn the_clock_reads_as_a_clock() {
        assert_eq!(clock(0.0), "0:00");
        assert_eq!(clock(59.4), "0:59");
        assert_eq!(clock(600.0), "10:00");
        assert_eq!(clock(3661.0), "1:01:01");
        // A negative elapsed is a clock that has not started, not a negative
        // time: a machine whose clock stepped backwards mid-set should not put
        // "-0:04" on the HUD.
        assert_eq!(clock(-4.0), "0:00");
    }

    /// The loudest item is what a caller with one pixel gets.
    #[test]
    fn the_loudest_thing_on_the_bar_is_what_it_reports() {
        assert_eq!(loudest(&[]), Level::Quiet);
        let mut broken = healthy();
        broken.cpu = BUSY;
        broken.xruns = 1.0;
        assert_eq!(loudest(&bar(&broken)), Level::Alarm);
    }
}
