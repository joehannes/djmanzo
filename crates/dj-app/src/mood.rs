//! Which theme djmanzo wears, and how slowly it changes its mind.
//!
//! §31 asks the theme to respond to venue ambience, musical context and the
//! session phase — and then spends most of its words on the other half:
//!
//! > But use slow adaptation. Never allow the interface to flicker from color
//! > to color every time the track changes.
//!
//! That sentence is the feature. Choosing a theme from a reading is arithmetic;
//! choosing one *and then leaving it alone* is the part that makes an adaptive
//! interface usable rather than exhausting. So this module is mostly a set of
//! brakes, and the test that matters is that a record change cannot move it.
//!
//! # Venue ambience, honestly
//!
//! §31's first input wants to know what the room is like. djmanzo has no light
//! sensor and no camera, so it cannot measure that — but it does not have to
//! guess either, because §81 already asks the DJ what kind of night it is, and
//! §31's own examples are *beach*, *club* and *daylight*: the same axis. So the
//! ambience here is [`crate::setting::Setting`], told rather than sensed. That
//! is the whole of why §81 came first.
//!
//! The one §31 example this cannot serve is *daylight* as a **measurement** —
//! whether the sun is actually on the screen right now. A DJ can choose the
//! daylight theme; djmanzo cannot notice that they should.
//!
//! # Musical context is the phase, and deliberately not the genre
//!
//! §31's second and third inputs collapse into one here on purpose. The phase
//! ([`SessionPhase`]) is read from energy and tempo — which *is* the musical
//! context, smoothed over minutes by `dj_core::ContextEngine`. Reading the
//! genre of the record on deck 1 instead would be reading a fact that changes
//! every four minutes, which is precisely the flicker §31 forbids.
//!
//! # The five brakes
//!
//! - **Minimum duration** ([`LEAST`]). Nothing changes within four minutes of
//!   the last change, whatever the reading says. A record is three to six
//!   minutes, so this alone makes a per-track flicker impossible.
//! - **Hysteresis** ([`SETTLE`]). A new answer has to be the answer
//!   *continuously* for forty seconds before it is taken. One tick of a
//!   different reading is noise.
//! - **Smoothing.** The change itself is a fade, not a cut — [`Mood::over`].
//! - **Transition duration**, which the caller may override or set to nothing.
//! - **A manual lock.** A DJ who has chosen a theme has decided, and djmanzo
//!   stops having an opinion. §31 lists it last; it overrides everything.

use std::time::Duration;

use crate::setting::Setting;
use dj_core::SessionPhase;

/// The shortest a theme may be worn before another may replace it.
///
/// Four minutes. A record is three to six, so this by itself makes "a
/// different colour every track" impossible — which is the failure §31 names
/// in as many words. Long enough to be a decision, short enough that a night
/// that genuinely turns still gets its theme within a couple of records.
pub const LEAST: Duration = Duration::from_secs(4 * 60);

/// How long a new answer has to hold before it is believed.
///
/// Forty seconds. The context engine already smooths its phase over minutes,
/// so this is a second brake rather than the only one — it catches the moment
/// a reading crosses a boundary and wobbles back, which is exactly when a
/// naive theme would flick and return.
pub const SETTLE: Duration = Duration::from_secs(40);

/// How long the change itself takes.
///
/// Three seconds. Long enough to read as the room changing rather than as a
/// repaint, short enough that nobody is waiting for the interface to finish
/// having an opinion.
pub const FADE: Duration = Duration::from_secs(3);

/// What djmanzo is wearing, and what it would like to wear.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mood {
    /// The theme package's id, exactly as `ui/src/controls/themes/packages.ts`
    /// spells it. An id rather than a description, because the interface owns
    /// the pixels and Rust owns the choice — the same split the density bands
    /// already use.
    pub theme: &'static str,
    /// How long the change should take. Zero when nothing is changing.
    pub over: Duration,
    /// True when the DJ has pinned it and djmanzo has stopped deciding.
    pub locked: bool,
}

/// What a night of this kind, at this point, asks to look like.
///
/// Grounded in each package's own `when` sentence rather than invented here:
/// Booth is "playing in the dark, highest contrast, least motion", Sunset is
/// "golden hour on a terrace", Studio is "long evenings at a desk". The table
/// is which of those a given evening is, not a new opinion about colour.
///
/// §31's examples are the shape of it: warm-up subdued, peak with stronger
/// accents, cool-down calmer, beach warm, club dark and high-contrast.
#[must_use]
pub fn wanted(setting: Setting, phase: SessionPhase) -> &'static str {
    match setting {
        // Nobody is listening and nobody is dancing. A desk, for hours.
        Setting::Practice => "pkg-studio",
        // Dark room, and the peak of it should look like the music.
        Setting::Club => match phase {
            SessionPhase::WarmUp => "pkg-booth",
            SessionPhase::Heat => "pkg-industrial",
            SessionPhase::Peak => "pkg-cyber",
            SessionPhase::Cooldown | SessionPhase::ChillOut => "pkg-booth",
        },
        // §31 names this one: brighter warmth, softer contrast. Early on it is
        // still light out, which is the one honest use of the daylight theme —
        // chosen because the DJ said *beach*, not because a sensor saw the sun.
        Setting::Beach => match phase {
            SessionPhase::WarmUp => "pkg-daylight",
            SessionPhase::Heat | SessionPhase::Peak => "pkg-sunset",
            SessionPhase::Cooldown | SessionPhase::ChillOut => "pkg-sunset",
        },
        // A lit room that is not there for the DJ. Calm and readable
        // throughout; a wedding that suddenly went neon would be the interface
        // disagreeing with the evening.
        Setting::Wedding => "pkg-organic",
        // Warm, and it lifts, but it is not a techno room.
        Setting::Latin => match phase {
            SessionPhase::Peak => "pkg-sunset",
            _ => "pkg-organic",
        },
        Setting::OpenFormat => match phase {
            SessionPhase::Peak => "pkg-cyber",
            _ => "pkg-organic",
        },
    }
}

/// The brakes: what is worn, and what is being waited out.
///
/// Held by the caller across ticks. It is the whole of §31's "slow adaptation"
/// — [`wanted`] alone would change the theme the instant a reading crossed a
/// boundary, which is the interface §31 exists to prevent.
#[derive(Debug, Clone)]
pub struct Weather {
    worn: &'static str,
    /// When the current theme was put on.
    since: Duration,
    /// What has been asked for since [`Self::asking`], if it differs from what
    /// is worn.
    pending: Option<&'static str>,
    asking: Duration,
    locked: bool,
}

impl Weather {
    /// Start out wearing something, at a moment.
    #[must_use]
    pub fn new(worn: &'static str, at: Duration) -> Self {
        Weather {
            worn,
            since: at,
            pending: None,
            asking: at,
            locked: false,
        }
    }

    /// What is being worn now.
    #[must_use]
    pub fn worn(&self) -> &'static str {
        self.worn
    }

    #[must_use]
    pub fn locked(&self) -> bool {
        self.locked
    }

    /// Pin it, or let djmanzo decide again.
    ///
    /// Locking does not change what is worn — it stops it changing. A lock
    /// that also snapped the theme somewhere would be a second decision hiding
    /// inside a refusal to decide.
    pub fn lock(&mut self, locked: bool) {
        self.locked = locked;
        if locked {
            self.pending = None;
        }
    }

    /// The DJ chose a theme. That is a decision, and it takes effect now.
    ///
    /// It also resets the clock: whatever djmanzo was about to do, the DJ has
    /// answered, and the minimum duration starts from their answer rather than
    /// from the change they interrupted.
    pub fn choose(&mut self, theme: &'static str, at: Duration) {
        self.worn = theme;
        self.since = at;
        self.pending = None;
        self.asking = at;
    }

    /// Offer a reading. Answers what to wear, and how long the change takes.
    ///
    /// `at` is time since the set began, so this is testable without a clock
    /// and deterministic in a replay.
    pub fn consider(&mut self, want: &'static str, at: Duration) -> Mood {
        let steady = Mood {
            theme: self.worn,
            over: Duration::ZERO,
            locked: self.locked,
        };

        // §31's last brake, and it beats all the others: a DJ who has chosen
        // has decided.
        if self.locked || want == self.worn {
            self.pending = None;
            return steady;
        }

        // Hysteresis. A different answer starts a clock rather than a change,
        // and any wobble back to something else restarts it.
        if self.pending != Some(want) {
            self.pending = Some(want);
            self.asking = at;
            return steady;
        }
        if at.saturating_sub(self.asking) < SETTLE {
            return steady;
        }

        // The minimum duration. Checked last so that a want which has settled
        // stays pending rather than being forgotten — otherwise a night that
        // turned during the minimum would have to turn again to be noticed.
        if at.saturating_sub(self.since) < LEAST {
            return steady;
        }

        self.worn = want;
        self.since = at;
        self.pending = None;
        Mood {
            theme: want,
            over: FADE,
            locked: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secs(n: u64) -> Duration {
        Duration::from_secs(n)
    }

    /// A night that has been running long enough for the minimum to be spent.
    fn settled() -> Weather {
        Weather::new("pkg-organic", Duration::ZERO)
    }

    /// **The interface must not flicker from colour to colour every time the
    /// track changes.**
    ///
    /// §31's own sentence, and the whole reason this module is mostly brakes.
    /// A record is three to six minutes; four hours of alternating readings —
    /// the shape a naive theme would follow straight into a different colour
    /// every track — must move it a handful of times at most, and never twice
    /// inside a record.
    #[test]
    fn a_reading_that_flips_every_track_does_not_flicker() {
        let mut weather = settled();
        let mut changes = 0;
        let mut at = Duration::ZERO;

        // Four hours, a reading every second, flipping every three minutes as
        // though each new record read differently from the last.
        while at < secs(4 * 3600) {
            let want = if (at.as_secs() / 180).is_multiple_of(2) {
                "pkg-cyber"
            } else {
                "pkg-booth"
            };
            if weather.consider(want, at).over > Duration::ZERO {
                changes += 1;
            }
            at += secs(1);
        }

        // Eighty records' worth of disagreement. Anything approaching that
        // many changes is the failure; a handful is adaptation.
        assert!(
            changes <= 4 * 3600 / LEAST.as_secs() as i32,
            "the theme changed {changes} times in four hours"
        );
        assert!(
            changes > 0,
            "nothing adapted at all, which is not §31 either"
        );
    }

    /// **A momentary reading is not an answer.**
    ///
    /// The hysteresis. A phase that crosses a boundary and wobbles back is the
    /// commonest thing a context engine does, and a theme that followed it
    /// would change twice for a night that did not turn at all.
    #[test]
    fn a_want_that_does_not_hold_is_never_taken() {
        let mut weather = settled();
        let start = secs(600);

        // Asked for, then withdrawn, well inside the settling time.
        assert_eq!(weather.consider("pkg-cyber", start).over, Duration::ZERO);
        assert_eq!(
            weather.consider("pkg-cyber", start + SETTLE - secs(1)).over,
            Duration::ZERO
        );
        assert_eq!(
            weather.consider("pkg-organic", start + SETTLE).theme,
            "pkg-organic",
            "the original was not still being worn"
        );

        // And asking again starts the clock over rather than resuming it.
        assert_eq!(
            weather.consider("pkg-cyber", start + SETTLE + secs(1)).over,
            Duration::ZERO
        );
        assert_eq!(weather.worn(), "pkg-organic");
    }

    /// A want that *does* hold is taken, once both brakes are spent.
    #[test]
    fn a_want_that_holds_is_eventually_worn() {
        let mut weather = settled();
        let mut at = LEAST + secs(1);
        let mut changed = None;
        for _ in 0..200 {
            let mood = weather.consider("pkg-cyber", at);
            if mood.over > Duration::ZERO {
                changed = Some(at);
                break;
            }
            at += secs(1);
        }
        let changed = changed.expect("a settled want was never taken");
        assert!(
            changed >= LEAST && changed <= LEAST + SETTLE + secs(5),
            "took until {changed:?}"
        );
        assert_eq!(weather.worn(), "pkg-cyber");
    }

    /// **Nothing changes inside the minimum, however settled the want.**
    #[test]
    fn nothing_changes_inside_the_minimum_duration() {
        let mut weather = settled();
        let mut at = Duration::ZERO;
        while at < LEAST {
            assert_eq!(
                weather.consider("pkg-cyber", at).over,
                Duration::ZERO,
                "changed at {at:?}, inside the minimum"
            );
            at += secs(5);
        }
    }

    /// **A locked theme is not djmanzo's to change.**
    ///
    /// §31 lists the manual lock last and it overrides everything: a DJ who
    /// has chosen has decided, and an interface that overrode them would be
    /// worse than one that never adapted.
    #[test]
    fn a_lock_stops_it_deciding_at_all() {
        let mut weather = settled();
        weather.lock(true);

        let mut at = LEAST + SETTLE + secs(10);
        for _ in 0..100 {
            let mood = weather.consider("pkg-cyber", at);
            assert_eq!(mood.over, Duration::ZERO);
            assert!(mood.locked);
            assert_eq!(mood.theme, "pkg-organic");
            at += secs(30);
        }

        // Unlocked, it may decide again.
        weather.lock(false);
        let mut changed = false;
        for _ in 0..200 {
            if weather.consider("pkg-cyber", at).over > Duration::ZERO {
                changed = true;
                break;
            }
            at += secs(1);
        }
        assert!(changed, "unlocking left it stuck");
    }

    /// Locking pins what is worn; it does not move it.
    ///
    /// A lock that also snapped the theme somewhere would be a second decision
    /// hiding inside a refusal to decide.
    #[test]
    fn locking_changes_nothing_but_the_deciding() {
        let mut weather = settled();
        let before = weather.worn();
        weather.lock(true);
        assert_eq!(weather.worn(), before);
    }

    /// The DJ's own choice takes effect at once, and restarts the clock.
    #[test]
    fn a_chosen_theme_is_worn_immediately() {
        let mut weather = settled();
        weather.choose("pkg-daylight", secs(100));
        assert_eq!(weather.worn(), "pkg-daylight");
        // And djmanzo does not immediately undo it: the minimum runs from the
        // DJ's answer, not from whatever it was about to do.
        assert_eq!(
            weather
                .consider("pkg-cyber", secs(100) + LEAST - secs(1))
                .over,
            Duration::ZERO
        );
    }

    /// Every theme id the interface actually ships, read from its own source.
    ///
    /// Read rather than copied, the way `cockpit`'s band table is checked
    /// against the browser harness. A list of ids maintained twice is a list
    /// that will disagree, and this particular disagreement is **silent**: a
    /// theme djmanzo asks for and no package answers to looks exactly like a
    /// theme that decided not to change.
    fn shipped() -> Vec<String> {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../ui/src/controls/themes/packages.ts"
        );
        let source = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("could not read the theme packages at {path}: {e}"));

        let ids: Vec<String> = source
            .lines()
            .filter_map(|line| {
                let rest = line.trim().strip_prefix("id: \"")?;
                let end = rest.find('"')?;
                Some(rest[..end].to_owned())
            })
            .collect();
        assert!(
            !ids.is_empty(),
            "`id: \"...\"` is no longer how the packages are written, so this \
             guard has silently stopped guarding"
        );
        ids
    }

    /// **Every setting and phase names a theme that exists.**
    #[test]
    fn every_reading_names_a_theme_that_ships() {
        let shipped = shipped();
        for setting in Setting::ALL {
            for phase in SessionPhase::ALL {
                let theme = wanted(setting, phase);
                assert!(
                    shipped.iter().any(|id| id == theme),
                    "{setting:?} at {phase:?} wants {theme}, which no package in \
                     ui/src/controls/themes/packages.ts answers to"
                );
            }
        }
    }

    /// **A night that turns is a night the theme follows.**
    ///
    /// The other half of §31: brakes that never release are an interface that
    /// does not adapt. A club that reaches its peak should end up wearing the
    /// peak theme, and a beach at sunset should not be wearing the club's.
    #[test]
    fn different_evenings_end_up_looking_different() {
        assert_eq!(wanted(Setting::Club, SessionPhase::Peak), "pkg-cyber");
        assert_eq!(wanted(Setting::Club, SessionPhase::WarmUp), "pkg-booth");
        assert_eq!(wanted(Setting::Beach, SessionPhase::Peak), "pkg-sunset");
        assert_eq!(wanted(Setting::Practice, SessionPhase::Peak), "pkg-studio");
        assert_ne!(
            wanted(Setting::Club, SessionPhase::Peak),
            wanted(Setting::Beach, SessionPhase::Peak),
            "a club and a beach look the same at peak"
        );
        assert_ne!(
            wanted(Setting::Club, SessionPhase::WarmUp),
            wanted(Setting::Club, SessionPhase::Peak),
            "§31 asks warm-up to be subdued and peak to have stronger accents"
        );
    }
}
