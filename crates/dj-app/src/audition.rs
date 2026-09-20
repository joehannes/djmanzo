//! Where an audition starts, and why.
//!
//! [§22 of the directive](../../../docs/DIRECTIVE.md) lists *audition* among
//! the six things a DJ may do to a candidate on the rail.
//! [`dj_engine::preview`] is the voice that plays one; this is the one
//! decision the host has to make before it can, and it is the decision that
//! separates an audition from a sampler slot.
//!
//! # Why this is not simply zero
//!
//! A record opens with an intro. Intros are where records sound most alike —
//! that is what an intro is *for*, since it has to mix out of whatever came
//! before — so a preview that always starts at 0:00 plays a DJ the least
//! informative sixteen bars of every candidate in the list. What they are
//! deciding about is the part of the record the room will hear.
//!
//! # The order, and why it is this order
//!
//! 1. **The first drop.** §75's `drops` is where a record comes back after
//!    thinning out, and it is the moment a DJ is buying. If a candidate has
//!    one, that is the question.
//! 2. **Where the vocal enters.** `dj_analysis::presence`, through §27's same
//!    reading. An instrumental has none and a record with a lead does, so this
//!    fires for exactly the records whose lead is the thing being judged —
//!    *will this vocal sit over what is playing* is the other question a rail
//!    gets asked.
//! 3. **The top.** Not a guess dressed as a choice. A record nobody has
//!    analysed has no landmark to jump to, and djmanzo inventing one — a third
//!    of the way in, say — would be a number with no reading behind it, drawn
//!    with the same confidence as the two above. Starting at the beginning is
//!    what every other DJ application does with a record it knows nothing
//!    about, and it is honest.
//!
//! [`Start`] carries which of the three it was, so the interface can say *from
//! the drop* rather than leaving a DJ to work out why one candidate opened
//! ninety seconds in and the next one did not.

use dj_analysis::energy::Trajectory;

/// Why an audition begins where it does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// §75's first drop: where the record comes back.
    Drop,
    /// §27's vocal entry: where the lead arrives.
    VocalEntry,
    /// The beginning, because nothing better is known about this record.
    Top,
}

impl Reason {
    /// The stable slug, for an interface to key off.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Reason::Drop => "drop",
            Reason::VocalEntry => "vocal-entry",
            Reason::Top => "top",
        }
    }

    /// What to tell the DJ, in their words rather than djmanzo's.
    #[must_use]
    pub const fn says(self) -> &'static str {
        match self {
            Reason::Drop => "from the drop",
            Reason::VocalEntry => "from the vocal",
            Reason::Top => "from the top",
        }
    }
}

/// Where an audition begins, in the candidate's own frames.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Start {
    pub frame: f64,
    pub reason: Reason,
}

/// Pick a starting point for a candidate.
///
/// `len_frames` is the record's length, and a landmark at or past the end is
/// **ignored** rather than clamped. The engine clamps too, but the two mean
/// different things: clamping there keeps a bad number from reading off the
/// end of a buffer, and ignoring here keeps djmanzo from telling a DJ it is
/// playing them the drop when what it is playing them is the last frame. A
/// trajectory that disagrees with a record's length is a trajectory measured
/// before the file changed, and the honest answer then is the top.
#[must_use]
pub fn start_of(trajectory: Option<&Trajectory>, len_frames: usize) -> Start {
    let end = len_frames as f64;
    let usable = |frame: f64| frame.is_finite() && frame > 0.0 && frame < end;

    if let Some(found) = trajectory {
        if let Some(&drop) = found.drops.iter().find(|frame| usable(**frame)) {
            return Start {
                frame: drop,
                reason: Reason::Drop,
            };
        }
        if let Some(voice) = found.voice_enters.filter(|frame| usable(*frame)) {
            return Start {
                frame: voice,
                reason: Reason::VocalEntry,
            };
        }
    }
    Start {
        frame: 0.0,
        reason: Reason::Top,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trajectory(drops: Vec<f64>, voice: Option<f64>) -> Trajectory {
        Trajectory {
            sections: Vec::new(),
            beats_per_section: 8,
            breakdowns: Vec::new(),
            drops,
            voice_enters: voice,
        }
    }

    /// **A record nobody has analysed opens at the top**, rather than at a
    /// number djmanzo made up.
    #[test]
    fn no_analysis_is_the_top() {
        let start = start_of(None, 1_000_000);
        assert_eq!(start.reason, Reason::Top);
        assert_eq!(start.frame, 0.0);
    }

    /// **The load-bearing one: the drop outranks the vocal.**
    ///
    /// Both are real landmarks and a record can have both, so the order has to
    /// be a decision rather than whichever the code happens to test first. A
    /// DJ auditioning from a rail is buying the drop.
    #[test]
    fn the_drop_wins_when_a_record_has_both() {
        let found = trajectory(vec![400_000.0], Some(100_000.0));
        let start = start_of(Some(&found), 1_000_000);
        assert_eq!(start.reason, Reason::Drop);
        assert_eq!(start.frame, 400_000.0);
    }

    /// **And the vocal is used when there is no drop**, which is most of what
    /// a wedding set is made of.
    #[test]
    fn a_record_with_no_drop_opens_at_the_vocal() {
        let found = trajectory(Vec::new(), Some(100_000.0));
        let start = start_of(Some(&found), 1_000_000);
        assert_eq!(start.reason, Reason::VocalEntry);
        assert_eq!(start.frame, 100_000.0);
    }

    /// **An instrumental with no drop opens at the top.**
    ///
    /// Two absences that are different facts -- no lead, and no analysis --
    /// landing on the same answer, which is right: neither says anything about
    /// where the interesting part is.
    #[test]
    fn an_instrumental_with_no_drop_is_the_top() {
        let found = trajectory(Vec::new(), None);
        assert_eq!(start_of(Some(&found), 1_000_000).reason, Reason::Top);
    }

    /// **A landmark past the end of the record is ignored, not clamped.**
    ///
    /// The engine clamps, so nothing reads off the end either way. What this
    /// prevents is the *sentence*: djmanzo telling a DJ it is playing them the
    /// drop while playing them the last frame of a record that has since been
    /// re-encoded shorter.
    #[test]
    fn a_landmark_past_the_end_is_not_believed() {
        let found = trajectory(vec![9_000_000.0], None);
        let start = start_of(Some(&found), 1_000_000);
        assert_eq!(start.reason, Reason::Top);
        assert_eq!(start.frame, 0.0);
    }

    /// And the same for a landmark exactly at the end, which is a record with
    /// nothing after it to hear.
    #[test]
    fn a_landmark_at_the_last_frame_is_not_a_landmark() {
        let found = trajectory(vec![1_000_000.0], None);
        assert_eq!(start_of(Some(&found), 1_000_000).reason, Reason::Top);
    }

    /// **The first usable drop, not the first drop.**
    ///
    /// A trajectory measured against a file that has since been trimmed can
    /// carry a drop past the end; the record may still have a later one that
    /// is fine. Taking `drops[0]` and giving up would throw away a real answer.
    #[test]
    fn a_nonsense_first_drop_does_not_lose_the_real_one() {
        let found = trajectory(vec![f64::NAN, 9_000_000.0, 400_000.0], None);
        let start = start_of(Some(&found), 1_000_000);
        assert_eq!(start.reason, Reason::Drop);
        assert_eq!(start.frame, 400_000.0);
    }

    /// **A drop at frame zero is not a drop.**
    ///
    /// It is a record that opens hot, and "from the drop" would be a sentence
    /// about a jump that did not happen.
    #[test]
    fn a_drop_at_the_very_start_reads_as_the_top() {
        let found = trajectory(vec![0.0], None);
        assert_eq!(start_of(Some(&found), 1_000_000).reason, Reason::Top);
    }

    /// Every reason says something, and says something different. A blank or a
    /// duplicate would be a rail row that explains nothing while looking as
    /// though it does.
    #[test]
    fn each_reason_has_its_own_words() {
        let all = [Reason::Drop, Reason::VocalEntry, Reason::Top];
        for reason in all {
            assert!(!reason.says().is_empty());
            assert!(!reason.slug().is_empty());
        }
        for (a, b) in all.iter().zip(all.iter().skip(1)) {
            assert_ne!(a.says(), b.says());
            assert_ne!(a.slug(), b.slug());
        }
    }
}
