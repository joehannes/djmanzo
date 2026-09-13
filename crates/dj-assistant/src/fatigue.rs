//! §43: how much assistance this DJ actually tolerates.
//!
//! > A DJ who ignores 20 consecutive suggestions should cause the suggestion
//! > rate to fall. Do not spam.
//!
//! # What counts as ignoring one
//!
//! A record landing on a deck that djmanzo did not put in the rail. That is the
//! whole definition, and it is deliberately the strictest honest one:
//!
//! - **Only a load resolves an offer.** The rail is polled — a panel open for
//!   one record asks several times — so counting each answer as a fresh offer
//!   would reach twenty in about a minute of nobody doing anything, and the
//!   assistant would go quiet at a DJ who had never even looked at it. Twenty
//!   ignored suggestions means twenty *records played*, which is an hour of a
//!   set and a real opinion.
//! - **A load with nothing standing is neither.** If the rail has never
//!   answered, djmanzo suggested nothing and nothing was ignored. Counting it
//!   would make a DJ who works entirely from their own crates look like one who
//!   is rejecting advice, which is a different thing and deserves different
//!   behaviour.
//! - **One hit resets the streak.** §43's rule is about *consecutive* ignores,
//!   and a DJ who has just played something djmanzo offered has said the
//!   suggestions are landing again. Staying quiet after that would be punishing
//!   them for the machine's earlier noise.
//!
//! # Why the floor is one and not zero
//!
//! [`Appetite::Least`] still offers a suggestion. Silence is a trap: a rail
//! that offers nothing can never be taken from, so the streak can never reset
//! and the assistant is off for the rest of the night with nothing saying so.
//! The quietest state djmanzo has is a DJ *choosing* `Posture::Off`, and that
//! is a decision rather than a consequence.
//!
//! # What this does not do
//!
//! **It does not change the posture.** §43 lists six states and djmanzo already
//! has six — `Posture` — but those are the DJ's own setting, and a machine that
//! quietly moved a DJ from Suggest to Watch because it felt unwanted would be
//! making a decision that is not its own. What falls is the *rate*: how many of
//! the attention budget's allowance are actually offered. The DJ's posture is
//! where they left it.

use dj_core::TrackId;

/// §43's number, as §43 writes it.
pub const IGNORED_BEFORE_QUIETER: u32 = 20;

/// How much of what it is allowed to offer, the assistant is offering.
///
/// One step down per [`IGNORED_BEFORE_QUIETER`] consecutive ignored
/// suggestions, and back to [`Appetite::Full`] the moment one lands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Appetite {
    /// Everything the attention budget allows.
    Full,
    /// Half of it.
    Half,
    /// A quarter.
    Quarter,
    /// One, which is the floor. See the note above about why it is not none.
    Least,
}

impl Appetite {
    pub const ALL: [Self; 4] = [Self::Full, Self::Half, Self::Quarter, Self::Least];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Half => "half",
            Self::Quarter => "quarter",
            Self::Least => "least",
        }
    }

    /// Where a streak of ignored suggestions puts the rate.
    #[must_use]
    pub const fn of(ignored_in_a_row: u32) -> Self {
        match ignored_in_a_row / IGNORED_BEFORE_QUIETER {
            0 => Self::Full,
            1 => Self::Half,
            2 => Self::Quarter,
            _ => Self::Least,
        }
    }

    /// How many to offer, out of what the attention budget allows.
    ///
    /// Never zero while the budget is not: see the module note. A budget of
    /// zero — §18's emergency — stays zero, because that one is not about
    /// fatigue at all and a DJ sorting out a failed recording does not want a
    /// record suggested at them.
    #[must_use]
    pub const fn out_of(self, allowed: usize) -> usize {
        if allowed == 0 {
            return 0;
        }
        let want = match self {
            Self::Full => allowed,
            Self::Half => allowed / 2,
            Self::Quarter => allowed / 4,
            Self::Least => 1,
        };
        if want == 0 { 1 } else { want }
    }
}

/// What happened to one offer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The DJ played something djmanzo put in front of them.
    Taken,
    /// They played something else.
    Ignored,
    /// There was nothing standing, so there was nothing to ignore.
    Nothing,
}

/// What the assistant has offered, and what the DJ did with it.
#[derive(Debug, Clone, Default)]
pub struct Fatigue {
    /// The records the rail last put in front of the DJ.
    standing: Vec<TrackId>,
    ignored_in_a_row: u32,
    offers: u64,
    taken: u64,
    ignored: u64,
}

impl Fatigue {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The rail has put these records in front of the DJ.
    ///
    /// Replaces whatever was standing without judging it: only a load resolves
    /// an offer, for the reason the module note gives. Re-offering the same
    /// records is not a second offer — the panel polls, and a DJ who leaves it
    /// open has been offered one thing, not fifty.
    pub fn offering(&mut self, tracks: &[TrackId]) {
        if self.standing == tracks {
            return;
        }
        self.standing = tracks.to_vec();
        if !tracks.is_empty() {
            self.offers += 1;
        }
    }

    /// A record has landed on a deck, from wherever it came.
    ///
    /// The one place an offer is resolved, and it is called from djmanzo's one
    /// load funnel, so a record dragged in, picked from a crate, sent by a
    /// controller or brought by the automix all count the same. §87's *one
    /// source of state truth*, used.
    pub fn landed(&mut self, track: TrackId) -> Outcome {
        if self.standing.is_empty() {
            return Outcome::Nothing;
        }
        let outcome = if self.standing.contains(&track) {
            self.taken += 1;
            self.ignored_in_a_row = 0;
            Outcome::Taken
        } else {
            self.ignored += 1;
            self.ignored_in_a_row += 1;
            Outcome::Ignored
        };
        self.standing.clear();
        outcome
    }

    #[must_use]
    pub const fn appetite(&self) -> Appetite {
        Appetite::of(self.ignored_in_a_row)
    }

    /// How many suggestions to offer, out of what §18's budget allows.
    #[must_use]
    pub const fn allowance(&self, allowed: usize) -> usize {
        self.appetite().out_of(allowed)
    }

    #[must_use]
    pub const fn ignored_in_a_row(&self) -> u32 {
        self.ignored_in_a_row
    }

    #[must_use]
    pub const fn offers(&self) -> u64 {
        self.offers
    }

    #[must_use]
    pub const fn taken(&self) -> u64 {
        self.taken
    }

    #[must_use]
    pub const fn ignored(&self) -> u64 {
        self.ignored
    }

    /// Why the assistant is as loud or as quiet as it is.
    ///
    /// Said rather than left to be noticed. A machine that goes quiet without
    /// explaining reads as broken, and the DJ has no way back — this is the
    /// sentence that tells them the rail is short because of what they have
    /// been doing, and that playing one of its records will bring it back.
    #[must_use]
    pub fn says(&self) -> String {
        match self.appetite() {
            Appetite::Full if self.offers == 0 => {
                "Nothing offered yet, so nothing to go on.".to_owned()
            }
            Appetite::Full => format!(
                "Offering everything. {} of {} suggestions played.",
                self.taken, self.offers
            ),
            _ => format!(
                "Quieter: {} records in a row that were not suggested. Play one \
                 that is and this goes back to full.",
                self.ignored_in_a_row
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(byte: u8) -> TrackId {
        TrackId::from_bytes([byte; 32])
    }

    /// **The load-bearing one: §43's number, and the two ways past it.**
    ///
    /// Twenty *consecutive* ignored suggestions, said exactly: nineteen is not
    /// enough, twenty is, and one taken record puts it back. All three, because
    /// each alone passes wrongly — an off-by-one that fires at nineteen looks
    /// identical to a correct one until a DJ notices the rail thinning early,
    /// and a rule with no way back is an assistant that goes quiet for good.
    #[test]
    fn twenty_ignored_in_a_row_makes_it_quieter_and_one_taken_brings_it_back() {
        let mut fatigue = Fatigue::new();

        for n in 1..IGNORED_BEFORE_QUIETER {
            fatigue.offering(&[id(1), id(2)]);
            assert_eq!(fatigue.landed(id(99)), Outcome::Ignored);
            assert_eq!(
                fatigue.appetite(),
                Appetite::Full,
                "the rail thinned after {n} ignored suggestions -- §43 says twenty"
            );
        }

        fatigue.offering(&[id(1), id(2)]);
        assert_eq!(fatigue.landed(id(99)), Outcome::Ignored);
        assert_eq!(
            fatigue.appetite(),
            Appetite::Half,
            "twenty records in a row that djmanzo did not suggest, and it is \
             still offering everything -- §43's whole instruction is *do not spam*"
        );

        fatigue.offering(&[id(1), id(2)]);
        assert_eq!(fatigue.landed(id(1)), Outcome::Taken);
        assert_eq!(
            fatigue.appetite(),
            Appetite::Full,
            "a suggestion landed and the assistant stayed quiet, which punishes \
             the DJ for its own earlier noise"
        );
    }

    /// It keeps falling, and it stops falling.
    #[test]
    fn the_rate_falls_a_step_at_a_time_and_stops_at_one() {
        let mut fatigue = Fatigue::new();
        let ignore = |f: &mut Fatigue, times: u32| {
            for _ in 0..times {
                f.offering(&[id(1)]);
                f.landed(id(99));
            }
        };

        ignore(&mut fatigue, IGNORED_BEFORE_QUIETER);
        assert_eq!(fatigue.appetite(), Appetite::Half);
        ignore(&mut fatigue, IGNORED_BEFORE_QUIETER);
        assert_eq!(fatigue.appetite(), Appetite::Quarter);
        ignore(&mut fatigue, IGNORED_BEFORE_QUIETER);
        assert_eq!(fatigue.appetite(), Appetite::Least);
        ignore(&mut fatigue, IGNORED_BEFORE_QUIETER * 5);
        assert_eq!(
            fatigue.appetite(),
            Appetite::Least,
            "the floor is not a floor"
        );
    }

    /// **The floor still offers something.**
    ///
    /// A rail that offers nothing can never be taken from, so the streak can
    /// never reset: the assistant would be off for the rest of the night with
    /// nothing saying so, and no way back short of a restart.
    #[test]
    fn the_quietest_it_gets_still_offers_one() {
        for appetite in Appetite::ALL {
            assert!(
                appetite.out_of(8) >= 1,
                "`{}` offers nothing, so it can never be taken from and never \
                 recovers",
                appetite.name()
            );
        }
        assert_eq!(Appetite::Least.out_of(8), 1);
        assert_eq!(Appetite::Quarter.out_of(8), 2);
        assert_eq!(Appetite::Half.out_of(8), 4);
        assert_eq!(Appetite::Full.out_of(8), 8);
        // A budget of one cannot be halved into nothing.
        assert_eq!(Appetite::Quarter.out_of(1), 1);
    }

    /// §18's emergency stays silent, because that one is not about fatigue.
    #[test]
    fn a_budget_of_none_is_still_none_however_welcome_the_advice_is() {
        for appetite in Appetite::ALL {
            assert_eq!(
                appetite.out_of(0),
                0,
                "`{}` offered a suggestion during an emergency",
                appetite.name()
            );
        }
    }

    /// A DJ who works from their own crates is not rejecting advice.
    ///
    /// With nothing standing there was no suggestion to ignore, so a night
    /// spent never opening the rail must not end with the rail thinned.
    #[test]
    fn a_load_with_nothing_offered_is_neither_taken_nor_ignored() {
        let mut fatigue = Fatigue::new();
        for _ in 0..IGNORED_BEFORE_QUIETER * 3 {
            assert_eq!(fatigue.landed(id(7)), Outcome::Nothing);
        }
        assert_eq!(fatigue.appetite(), Appetite::Full);
        assert_eq!(fatigue.ignored(), 0);
    }

    /// A polled rail is one offer, not fifty.
    ///
    /// The panel asks again on every refresh. Counting each answer would reach
    /// twenty in about a minute of nobody doing anything, and the assistant
    /// would go quiet at a DJ who had not even looked at it.
    #[test]
    fn offering_the_same_records_again_is_the_same_offer() {
        let mut fatigue = Fatigue::new();
        for _ in 0..50 {
            fatigue.offering(&[id(1), id(2), id(3)]);
        }
        assert_eq!(fatigue.offers(), 1);
        fatigue.offering(&[id(1), id(2), id(4)]);
        assert_eq!(fatigue.offers(), 2);
    }

    /// The sentence changes when the behaviour does, and says the way back.
    #[test]
    fn it_says_why_it_is_quiet_and_how_to_undo_it() {
        let mut fatigue = Fatigue::new();
        assert!(fatigue.says().contains("Nothing offered yet"));

        for _ in 0..IGNORED_BEFORE_QUIETER {
            fatigue.offering(&[id(1)]);
            fatigue.landed(id(99));
        }
        let says = fatigue.says();
        assert!(says.contains("20"), "it does not say how many: {says}");
        assert!(
            says.contains("back to full"),
            "a DJ told the assistant has gone quiet, and not how to undo it, is \
             a DJ who thinks it is broken: {says}"
        );
    }

    /// Every appetite is spelled the same way stored as it is spoken.
    #[test]
    fn an_appetite_is_spelled_the_same_way_stored_as_it_is_spoken() {
        for appetite in Appetite::ALL {
            let stored = serde_json::to_string(&appetite).expect("it serialises");
            assert_eq!(stored, format!("\"{}\"", appetite.name()));
        }
    }
}
