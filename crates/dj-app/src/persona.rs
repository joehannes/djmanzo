//! §80's persona learning, and the half of it that makes the rest safe.
//!
//! > Eventually learn: *"This DJ likes a very dense layout." "This DJ almost
//! > never uses Automix during peaks." "This DJ prefers long blends." "This DJ
//! > uses stems mostly for vocals."* But express learned behavior as **editable
//! > preferences**. The system should show: *Learned preference* and let the
//! > user **reject/modify** it.
//!
//! # What djmanzo already learns, and why it was not enough
//!
//! §13's [`crate::signals::Tendency`] and §81's [`crate::profile::Profile`] both
//! ship, both are constructor-enforced so neither can generalise from one
//! surprising night, and both already produce sentences. What neither does is
//! the thing §80 spends half its words on: a DJ cannot **disagree**.
//!
//! That is not a missing button. A learned claim a DJ cannot reject is one they
//! have to keep reading, and — worse — one djmanzo may go on acting on after
//! being told it is wrong. §13 protects itself by writing observations rather
//! than preferences: *"You often sweep the filter when the night is peaking"*,
//! never *"you like filter sweeps"*. §80 asks for the second register, and the
//! second register is only honest if it comes with an answer.
//!
//! So this module is a table of things djmanzo may claim, each with the
//! evidence behind it, and a [`Verdict`] the DJ owns. A rejected trait is
//! **not offered again and not acted on** — both, because either alone is a
//! rejection that did not take.
//!
//! # Three of §80's four, and the fourth says why not
//!
//! The first three are derivable from §81's profiles today. The fourth is not,
//! and the reason is specific rather than a shrug: [`Trait::StemsForVocals`].

use crate::profile::Profile;

/// One of §80's four learnable traits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Trait {
    /// "This DJ likes a very dense layout."
    Density,
    /// "This DJ almost never uses Automix during peaks."
    AutomixAtPeak,
    /// "This DJ prefers long blends."
    BlendLength,
    /// "This DJ uses stems mostly for vocals."
    ///
    /// **Not derivable, and named rather than faked.** §14's gesture vocabulary
    /// records `stem-changed` and not *which* stem, so the log says a DJ
    /// reached for the stems and never says whether it was the vocal. Claiming
    /// this from what djmanzo has would mean inventing the part of the sentence
    /// that carries all of its meaning. Recording which stem is a change to
    /// `signals::Did` and to what the bus logs, and it belongs to §14.
    StemsForVocals,
}

impl Trait {
    /// §80's four, in §80's order.
    pub const ALL: [Self; 4] = [
        Self::Density,
        Self::AutomixAtPeak,
        Self::BlendLength,
        Self::StemsForVocals,
    ];

    /// The slug a verdict is stored under.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Density => "density",
            Self::AutomixAtPeak => "automix-at-peak",
            Self::BlendLength => "blend-length",
            Self::StemsForVocals => "stems-for-vocals",
        }
    }

    /// Why djmanzo cannot claim this yet, or empty where it can.
    #[must_use]
    pub const fn why_not(self) -> &'static str {
        match self {
            Self::StemsForVocals => {
                "djmanzo's log records that you reached for the stems and not \
                 which stem it was, so it cannot tell a vocal ride from a drum \
                 swap. Saying this anyway would be inventing the half of the \
                 sentence that means anything."
            }
            _ => "",
        }
    }

    /// A trait by its slug. `None` for anything else, never a fallback.
    #[must_use]
    pub fn parse(slug: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|t| t.slug() == slug)
    }
}

/// What the DJ has said about a learned claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Verdict {
    /// djmanzo thinks so and nobody has answered.
    #[default]
    Offered,
    /// The DJ agreed. djmanzo may act on it.
    Accepted,
    /// The DJ said no. djmanzo may neither act on it nor raise it again.
    Rejected,
}

impl Verdict {
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Offered => "offered",
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
        }
    }

    /// A verdict by its slug, or `Offered` for anything unrecognised.
    ///
    /// The fallback is the *quiet* end on purpose. A stored verdict djmanzo
    /// cannot read must not become `Accepted`, because that would be djmanzo
    /// acting on a claim nobody agreed to; `Offered` costs the DJ one answer.
    #[must_use]
    pub fn parse(slug: &str) -> Self {
        match slug.trim() {
            "accepted" => Self::Accepted,
            "rejected" => Self::Rejected,
            _ => Self::Offered,
        }
    }

    /// Whether djmanzo may act on a claim with this verdict.
    ///
    /// Only `Accepted`. An unanswered claim is not a licence — §80 asks for a
    /// *preference the user can reject*, and one that is already in force
    /// before they see it is one they are rejecting after the fact.
    #[must_use]
    pub const fn may_act(self) -> bool {
        matches!(self, Self::Accepted)
    }
}

/// One thing djmanzo believes about how this DJ plays.
#[derive(Debug, Clone, PartialEq)]
pub struct Learned {
    /// Which of §80's four.
    pub which: Trait,
    /// The claim, in §80's own register: a preference, not an observation.
    pub says: String,
    /// The evidence, in the DJ's words.
    ///
    /// Never empty. §80 asks for a preference the DJ can *reject or modify*,
    /// and a claim whose basis they cannot see is one they can only agree with
    /// or refuse blindly.
    pub because: String,
}

/// What djmanzo can say about this DJ, given §81's profiles.
///
/// Only from settings there is already enough evidence for: [`Profile`] is
/// constructor-enforced at three nights, so a profile existing at all is the
/// threshold. A trait is claimed only where **every** profile that has an
/// opinion agrees — a DJ who runs a dense layout at clubs and a sparse one at
/// weddings does not "like a dense layout", they do two different jobs, and
/// §81 exists precisely so djmanzo stops averaging those.
#[must_use]
pub fn learned(profiles: &[Profile]) -> Vec<Learned> {
    let mut said = Vec::new();

    let densities: Vec<&str> = profiles.iter().filter_map(Profile::density).collect();
    if let Some(one) = all_the_same(&densities) {
        said.push(Learned {
            which: Trait::Density,
            // "You keep the layout at Ultra Dense" rather than "you like a
            // Ultra Dense layout". The density names are the interface's own
            // and are capitalised, so an article in front of them has to be
            // chosen per name — and the first version of this sentence said
            // "a Ultra Dense", which is what driving it showed. A sentence
            // shaped so that no article is needed cannot get one wrong.
            says: format!("You keep the layout at {one}."),
            because: format!("{}, you run {one}.", on_every(densities.len())),
        });
    }

    let postures: Vec<_> = profiles.iter().filter_map(Profile::automation).collect();
    if let Some(one) = all_the_same(&postures) {
        said.push(Learned {
            which: Trait::AutomixAtPeak,
            says: format!("You keep the assistant on {}.", one.name()),
            because: format!(
                "{}, it has been on {}.",
                on_every(postures.len()),
                one.name()
            ),
        });
    }

    let styles: Vec<_> = profiles.iter().filter_map(Profile::style).collect();
    if let Some(one) = all_the_same(&styles) {
        said.push(Learned {
            which: Trait::BlendLength,
            says: format!("You prefer {one} transitions."),
            because: format!(
                "{}, more than half your mixes are {one}.",
                on_every(styles.len())
            ),
        });
    }

    said
}

/// How much evidence there is, as the opening of a sentence.
///
/// It has to read correctly at **one**, which is the case a DJ actually meets
/// first: djmanzo needs three nights of a kind before that kind has a profile
/// at all, so a new DJ's first persona claim rests on exactly one kind of
/// night. The earlier wording — "every kind of night djmanzo has enough of …
/// 1 of them" — was true and read like a machine counting.
fn on_every(kinds: usize) -> String {
    if kinds == 1 {
        "On the one kind of night djmanzo has enough of".to_owned()
    } else {
        format!("On all {kinds} kinds of night djmanzo has enough of")
    }
}

/// The claim to put to the DJ for this trait, given what they last said.
///
/// `None` for a trait djmanzo has nothing to say about **and** for one the DJ
/// has already refused. The second is the point: §80 asks for a preference the
/// user can reject, and re-offering a refused claim is a rejection that did not
/// take. Both halves of a rejection live here and in [`Verdict::may_act`], and
/// neither is enough alone — a DJ asked the same thing every week has not been
/// heard, and one whose "no" is recorded and then ignored has been lied to.
#[must_use]
pub fn offering(claims: &[Learned], which: Trait, verdict: Verdict) -> Option<&Learned> {
    if verdict == Verdict::Rejected {
        return None;
    }
    claims.iter().find(|c| c.which == which)
}

/// The one value they all are, or `None` if the list is empty or disagrees.
///
/// Empty is `None` rather than vacuously true: a claim from no evidence is the
/// failure every learning type in this workspace is built to refuse.
fn all_the_same<T: PartialEq + Copy>(all: &[T]) -> Option<T> {
    let first = *all.first()?;
    all.iter().all(|x| *x == first).then_some(first)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::setting::Setting;
    use dj_assistant::Posture;

    /// Build profiles for a test without reaching into `profile`'s private
    /// fields: `profiles` is the only constructor, so the evidence is real.
    fn profiles_for(rows: &[(Setting, usize, &str, Posture)]) -> Vec<Profile> {
        let nights: Vec<dj_library::Night> = rows
            .iter()
            .flat_map(|(setting, count, density, posture)| {
                (0..*count).map(move |n| dj_library::Night {
                    session_id: format!("{}-{n}", setting.slug()),
                    setting: setting.slug().to_owned(),
                    began_at: 0,
                    density: Some((*density).to_owned()),
                    style: Some("blend".to_owned()),
                    posture: Some(posture.name().to_owned()),
                    techniques: None,
                })
            })
            .collect();
        crate::profile::profiles(&nights, &|_| Vec::new())
    }

    /// **The load-bearing one: a rejected claim is neither raised again nor
    /// acted on.**
    ///
    /// §80 spends half its words on this — *let the user reject/modify it* —
    /// and either half alone is a rejection that did not take. A DJ who is
    /// asked the same thing every week has not been heard; a DJ whose "no" is
    /// recorded and then ignored has been lied to. The second is the worse one
    /// and it is the one that is invisible, because the interface would look
    /// exactly right.
    #[test]
    fn a_rejected_claim_is_not_offered_again_and_is_never_acted_on() {
        assert!(!Verdict::Rejected.may_act());
        assert!(
            !Verdict::Offered.may_act(),
            "an unanswered claim is not a licence"
        );
        assert!(Verdict::Accepted.may_act());

        // And the other half: a refused claim is not put to them again.
        let claims = learned(&profiles_for(&[
            (Setting::Club, 4, "dense", Posture::Prepare),
            (Setting::Wedding, 4, "dense", Posture::Prepare),
        ]));
        assert!(
            offering(&claims, Trait::Density, Verdict::Offered).is_some(),
            "an unanswered claim is not being put to the DJ at all"
        );
        assert!(offering(&claims, Trait::Density, Verdict::Accepted).is_some());
        assert!(
            offering(&claims, Trait::Density, Verdict::Rejected).is_none(),
            "a refused claim is still being put to the DJ, which is a rejection \
             that did not take"
        );
        // And a trait with nothing behind it is absent whatever was said, so an
        // accepted verdict cannot resurrect a claim djmanzo no longer makes.
        assert!(offering(&claims, Trait::StemsForVocals, Verdict::Accepted).is_none());

        // A stored verdict djmanzo cannot read falls to the quiet end.
        assert_eq!(Verdict::parse("rejected"), Verdict::Rejected);
        assert_eq!(Verdict::parse("accepted"), Verdict::Accepted);
        assert_eq!(Verdict::parse(""), Verdict::Offered);
        assert_eq!(Verdict::parse("yes please"), Verdict::Offered);
        assert!(
            !Verdict::parse("Accepted").may_act(),
            "case is not forgiven"
        );
    }

    /// **Nothing is claimed from a DJ who does two different jobs.**
    ///
    /// §81 exists because averaging a wedding with a club night is worse than
    /// learning nothing. A persona statement is an average by construction, so
    /// it is only made where every profile that has an opinion agrees — and a
    /// DJ who runs dense at clubs and sparse at weddings gets no claim at all
    /// rather than the more common one.
    #[test]
    fn a_claim_needs_every_kind_of_night_to_agree() {
        let agreeing = profiles_for(&[
            (Setting::Club, 4, "dense", Posture::Prepare),
            (Setting::Wedding, 4, "dense", Posture::Prepare),
        ]);
        let said = learned(&agreeing);
        assert!(
            said.iter().any(|l| l.which == Trait::Density),
            "two agreeing profiles produced no claim: {said:?}"
        );

        let disagreeing = profiles_for(&[
            (Setting::Club, 4, "dense", Posture::Prepare),
            (Setting::Wedding, 4, "relaxed", Posture::Prepare),
        ]);
        assert!(
            !learned(&disagreeing)
                .iter()
                .any(|l| l.which == Trait::Density),
            "djmanzo averaged a dense club night with a relaxed wedding"
        );

        // And nothing at all from nothing at all.
        assert!(learned(&[]).is_empty());
    }

    /// **Every claim carries its evidence.**
    ///
    /// §80 asks for a preference the DJ can reject *or modify*, and a claim
    /// whose basis they cannot see is one they can only accept or refuse
    /// blindly. The register matters too: these are §80's preferences, not
    /// §13's observations, and the two are deliberately different sentences.
    #[test]
    fn every_claim_says_what_it_is_based_on() {
        let said = learned(&profiles_for(&[
            (Setting::Club, 4, "dense", Posture::Prepare),
            (Setting::Wedding, 4, "dense", Posture::Prepare),
        ]));
        assert!(!said.is_empty());
        for claim in &said {
            assert!(!claim.says.trim().is_empty(), "{claim:?} says nothing");
            assert!(
                !claim.because.trim().is_empty(),
                "{claim:?} gives no evidence, so it can only be refused blindly"
            );
            for mark in ['*', '`', '_', '#'] {
                assert!(
                    !claim.says.contains(mark) && !claim.because.contains(mark),
                    "{claim:?} is written in markup, which the panel draws as itself"
                );
            }
        }
    }

    /// **The sentences read correctly at one kind of night, which is the first
    /// case a real DJ meets.**
    ///
    /// A profile needs three nights of a kind before that kind exists at all,
    /// so the very first persona claim anybody sees rests on exactly one. The
    /// first version of these sentences said "a Ultra Dense layout" and "1 of
    /// them" — both true, both found by looking at the running application,
    /// neither catchable by a type-check or a browser test, because nothing
    /// automated reads copy.
    #[test]
    fn the_sentences_read_correctly_with_one_kind_of_night_behind_them() {
        let one_kind = learned(&profiles_for(&[(
            Setting::Club,
            4,
            "Ultra Dense",
            Posture::Suggest,
        )]));
        assert!(!one_kind.is_empty(), "one kind of night produced no claim");
        for claim in &one_kind {
            assert!(
                !claim.says.contains(" a Ultra")
                    && !claim.says.contains(" a Extra")
                    && !claim.says.contains(" a A")
                    && !claim.says.contains(" a E")
                    && !claim.says.contains(" a I")
                    && !claim.says.contains(" a O"),
                "{} puts `a` in front of a vowel",
                claim.says
            );
            assert!(
                claim.because.contains("the one kind of night"),
                "{} counts at the DJ instead of speaking to them",
                claim.because
            );
        }

        // And the plural still reads as a plural.
        let two_kinds = learned(&profiles_for(&[
            (Setting::Club, 4, "Ultra Dense", Posture::Suggest),
            (Setting::Wedding, 4, "Ultra Dense", Posture::Suggest),
        ]));
        for claim in &two_kinds {
            assert!(
                claim.because.contains("all 2 kinds of night"),
                "{} does not say how much is behind it",
                claim.because
            );
        }
    }

    /// **The fourth of §80's four is never claimed, and says why.**
    ///
    /// §14's vocabulary records `stem-changed` and not which stem, so *"you use
    /// stems mostly for vocals"* would be djmanzo inventing the half of the
    /// sentence that carries the meaning. Named rather than left off the list,
    /// because a list of three would read as the whole of §80.
    #[test]
    fn the_one_djmanzo_cannot_tell_is_named_rather_than_guessed() {
        let said = learned(&profiles_for(&[
            (Setting::Club, 4, "dense", Posture::Prepare),
            (Setting::Wedding, 4, "dense", Posture::Prepare),
        ]));
        assert!(
            !said.iter().any(|l| l.which == Trait::StemsForVocals),
            "djmanzo claimed something about stems it cannot see"
        );
        assert!(!Trait::StemsForVocals.why_not().is_empty());

        // The three it can tell carry no excuse, which is the other direction.
        for which in [Trait::Density, Trait::AutomixAtPeak, Trait::BlendLength] {
            assert!(
                which.why_not().is_empty(),
                "{} carries a reason it cannot be claimed and is claimed anyway",
                which.slug()
            );
        }
    }

    /// **A trait survives being written down, and a stranger is refused.**
    #[test]
    fn a_trait_round_trips_and_a_stranger_is_not_guessed() {
        for which in Trait::ALL {
            assert_eq!(Trait::parse(which.slug()), Some(which));
        }
        assert_eq!(Trait::parse(""), None);
        assert_eq!(Trait::parse("Density"), None);
        assert_eq!(Trait::parse("loudness"), None);
    }
}
