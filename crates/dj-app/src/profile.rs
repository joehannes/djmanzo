//! How this DJ plays, per kind of night.
//!
//! §81, in one sentence: *do not store one universal DJ profile; store
//! conditional profiles.* The directive draws it as a tree — Johannes, and
//! under him Club, Beach, Wedding, Latin, Practice, Open Format — and lists
//! what may differ between the branches: density, technique preferences, genre
//! weights, transition style, automation tolerance.
//!
//! # Why one profile is worse than none
//!
//! A DJ who plays bachata at weddings and techno at clubs, averaged, is a DJ
//! who plays neither. The average is not a compromise between two true things;
//! it is a third thing that describes no evening either of them had. Worse, it
//! is *confident* — it has twice the evidence of each real answer — so a system
//! that offers it will offer it strongly. That is §13's failure with more rows
//! behind it, and the reason §81 exists.
//!
//! # The same discipline as a tendency, one level up
//!
//! [`crate::signals::Tendency`] is constructible only through `tendencies()`,
//! never without a phase and never on fewer than four occurrences in that
//! phase. A [`Profile`] is the same shape of promise about a whole night: it
//! has private fields, it can only be made by [`profiles`], and it is not made
//! at all until there are [`ENOUGH_NIGHTS`] of that setting. "How you play at
//! weddings" after one wedding is a description of one evening wearing the
//! word *always*.
//!
//! Each field then answers separately, because they arrive at different rates.
//! A night has a density from its first minute and a commonest transition style
//! only once it has had transitions, so a profile with plenty of nights behind
//! it can still have nothing to say about one of the five. `None` is a real
//! answer and is drawn as one.
//!
//! # Derived where it can be, kept where it cannot
//!
//! Genre weights are **derived**: the plays are in `history` and the genres are
//! on the tracks, so a stored weight would be a second copy that drifts the
//! first time a record is re-tagged.
//!
//! The other four are read off the **action log**, and the action log does not
//! outlive the run of the application that made it. So they are written to the
//! night's row as the night goes — see the `nights` table. That is not a second
//! copy of the log; it is the only trace that survives it.

use std::collections::BTreeMap;

use crate::setting::Setting;
use crate::signals::Did;
use dj_assistant::posture::Posture;
use dj_core::action::TransitionStyle;
use dj_library::Night;

/// How many nights of a setting before djmanzo will generalise about it.
///
/// Three. One night is an evening; two is a coincidence; three is the smallest
/// number that can show a habit rather than a repeat. The same argument
/// `signals::ENOUGH` makes about gestures, at the scale of a whole night —
/// and deliberately smaller, because nights are rarer than gestures and a
/// threshold nobody reaches is a feature that never speaks.
pub const ENOUGH_NIGHTS: usize = 3;

/// The most a profile may move a candidate's score, either way.
///
/// Three quarters of a point, the same bound taste gets and quoted from the
/// same scale: a same-key match is worth three and a key clash minus two and
/// a half. So a profile can reorder records that would *all* work — it can
/// prefer the bachata among two valid keys at a wedding — and can never lift
/// one that would not, because the gap it would have to cross is more than
/// seven times this.
///
/// **A profile breaks ties. It does not overrule the mixing.** It is also
/// added on top of taste rather than instead of it, which is deliberate and
/// is the reason for the bound: the two together can move a record by one and
/// a half, still well inside what a single key relation is worth.
pub const MOST_IT_MAY_MOVE: f64 = 0.75;

/// How much of the evidence has to agree before a single answer is given.
///
/// Half. A transition style used on two of five wedding nights is not "how you
/// mix at weddings", it is what you did twice — and a profile that reported it
/// anyway would be putting the DJ's own minority behaviour to them as their
/// habit.
const AGREE: f64 = 0.5;

/// What djmanzo has worked out about how this DJ plays in one setting.
///
/// Private fields on purpose: every one of them is a claim that needed enough
/// evidence, and a struct anyone can build is a claim anyone can make.
#[derive(Debug, Clone, PartialEq)]
pub struct Profile {
    setting: Setting,
    nights: usize,
    density: Option<String>,
    style: Option<TransitionStyle>,
    automation: Option<Posture>,
    techniques: Vec<Did>,
    genres: Vec<(String, f64)>,
}

impl Profile {
    /// Which kind of night this is about. There is no profile without one.
    #[must_use]
    pub fn setting(&self) -> Setting {
        self.setting
    }

    /// How many nights it rests on, so an interface can say so rather than
    /// presenting three nights and thirty with the same face.
    #[must_use]
    pub fn nights(&self) -> usize {
        self.nights
    }

    /// The interface density this DJ actually runs at here, when enough nights
    /// agree.
    #[must_use]
    pub fn density(&self) -> Option<&str> {
        self.density.as_deref()
    }

    /// How they mostly join records here.
    #[must_use]
    pub fn style(&self) -> Option<TransitionStyle> {
        self.style
    }

    /// §81's "automation tolerance": how much they let the assistant do here.
    #[must_use]
    pub fn automation(&self) -> Option<Posture> {
        self.automation
    }

    /// The gestures that show up on most of these nights.
    #[must_use]
    pub fn techniques(&self) -> &[Did] {
        &self.techniques
    }

    /// What gets played here, as shares of the plays, commonest first. Sums to
    /// one over the genres present; records with no genre are left out rather
    /// than counted as a genre called nothing.
    #[must_use]
    pub fn genres(&self) -> &[(String, f64)] {
        &self.genres
    }

    /// How much this record's genre should move its score, at this kind of
    /// night. Zero when the profile has nothing to say about it.
    ///
    /// **Added, never multiplied**, and bounded — the same construction
    /// `dj_library::learned::Taste::tilt_for` uses, for the same two reasons.
    /// A suggestion's score is signed, so multiplying a key clash by anything
    /// above one makes it *better*; and a leaning is a ratio, so twice as
    /// often as an even split and half as often are equal and opposite, which
    /// only log space says.
    ///
    /// What is compared against is an **even split over the genres this kind
    /// of night actually contains**, not over the library. A wedding that is
    /// half bachata and half merengue has no leaning between them, and
    /// measuring against the whole collection would give both a large one for
    /// being a wedding at all.
    #[must_use]
    pub fn tilt_for(&self, genre: Option<&str>) -> f64 {
        let Some(share) = self.share_of(genre) else {
            return 0.0;
        };
        #[allow(clippy::cast_precision_loss)]
        let even = 1.0 / self.genres.len() as f64;
        if !(share > 0.0 && even > 0.0) {
            return 0.0;
        }
        (share / even)
            .log2()
            .clamp(-MOST_IT_MAY_MOVE, MOST_IT_MAY_MOVE)
    }

    /// What share of the plays here this genre has, when it has any.
    ///
    /// `None` for an untagged record and for a genre this kind of night has
    /// never seen — neutral rather than penalised, because most collections
    /// are half-tagged and pushing the untagged half down would hide it.
    #[must_use]
    pub fn share_of(&self, genre: Option<&str>) -> Option<f64> {
        let genre = genre?;
        self.genres
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(genre))
            .map(|(_, share)| *share)
    }

    /// Why a record moved, in words, when it moved at all.
    ///
    /// Always names the setting and the share, because §42 asks for
    /// explainable suggestions and "you play a lot of this" is not one: it
    /// cannot be checked, argued with, or recognised as wrong.
    #[must_use]
    pub fn because(&self, genre: Option<&str>) -> Option<String> {
        let genre = genre?;
        let share = self.share_of(Some(genre))?;
        (self.tilt_for(Some(genre)).abs() > f64::EPSILON).then(|| {
            #[allow(clippy::cast_possible_truncation)]
            let percent = (share * 100.0).round() as i64;
            format!(
                "{percent}% of a {} night",
                self.setting.title().to_lowercase()
            )
        })
    }

    /// One sentence, written here rather than in the interface.
    ///
    /// The same rule §13 follows: the words are Rust's, so a panel cannot
    /// assemble a claim out of the parts that djmanzo would not make. This one
    /// always names the setting and always names how many nights it is from,
    /// because a profile read without either is exactly the universal profile
    /// §81 forbids.
    #[must_use]
    pub fn words(&self) -> String {
        // The title as it is written, not lower-cased into a sentence: "at
        // wedding" and "at beach / sunset" both read badly, and the six titles
        // have no single preposition that fits them all. Leading with the name
        // works for every one of them.
        let mut said = format!(
            "{}, over {} night{}",
            self.setting.title(),
            self.nights,
            if self.nights == 1 { "" } else { "s" }
        );
        if let Some(style) = self.style {
            said.push_str(&format!(": mostly {style} transitions"));
        } else {
            said.push_str(": too varied so far to say how you mix");
        }
        if let Some((genre, share)) = self.genres.first() {
            said.push_str(&format!(
                ", {}% {genre}",
                (share * 100.0).round() as i64,
                genre = genre
            ));
        }
        if let Some(posture) = self.automation {
            said.push_str(&format!(", assistant on {}", posture.name()));
        }
        said.push('.');
        said
    }
}

/// Build every profile there is enough evidence for.
///
/// `genres` is looked up per setting by the caller, which is the layer with a
/// database; this module stays a pure function over what it is handed, the way
/// `signals::tendencies` does.
///
/// Settings with fewer than [`ENOUGH_NIGHTS`] produce **nothing at all** —
/// not an empty profile, which an interface would draw as a profile that knows
/// nothing rather than as one that does not exist yet.
#[must_use]
pub fn profiles(nights: &[Night], genres: &dyn Fn(Setting) -> Vec<(String, u32)>) -> Vec<Profile> {
    let mut per_setting: BTreeMap<Setting, Vec<&Night>> = BTreeMap::new();
    for night in nights {
        // A stored setting nobody recognises is skipped rather than folded
        // into open format — see `Setting::parse`.
        if let Some(setting) = Setting::parse(&night.setting) {
            per_setting.entry(setting).or_default().push(night);
        }
    }

    per_setting
        .into_iter()
        .filter(|(_, seen)| seen.len() >= ENOUGH_NIGHTS)
        .map(|(setting, seen)| Profile {
            setting,
            nights: seen.len(),
            density: agreed(seen.iter().map(|n| n.density.clone())),
            style: agreed(seen.iter().map(|n| n.style.clone()))
                .and_then(|word| TransitionStyle::parse(&word)),
            automation: agreed(seen.iter().map(|n| n.posture.clone()))
                .and_then(|word| Posture::ALL.into_iter().find(|p| p.name() == word)),
            techniques: usual(&seen),
            genres: shares(genres(setting)),
        })
        .collect()
}

/// The commonest answer, if enough of those who answered agree.
///
/// Counted over the nights that said *something*, not over all of them: a
/// night with no transitions has no opinion about transition style, and
/// counting it as a vote against would make a habit disappear as soon as the
/// DJ had a quiet evening.
fn agreed(values: impl Iterator<Item = Option<String>>) -> Option<String> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut answered = 0;
    for value in values.flatten() {
        if value.is_empty() {
            continue;
        }
        answered += 1;
        *counts.entry(value).or_default() += 1;
    }
    if answered == 0 {
        return None;
    }
    // Ties break by the name, which is arbitrary and stable — an answer that
    // changed each time it was asked would be worse than either.
    let (best, count) = counts
        .into_iter()
        .max_by_key(|(name, n)| (*n, name.clone()))?;
    #[allow(clippy::cast_precision_loss)]
    let share = count as f64 / answered as f64;
    (share >= AGREE).then_some(best)
}

/// The gestures that turn up on most of the nights.
///
/// Per *night*, not per occurrence: a DJ who looped forty times in one evening
/// and never again has done a thing once, and counting the forty would make it
/// their signature.
fn usual(nights: &[&Night]) -> Vec<Did> {
    let mut counts: BTreeMap<Did, usize> = BTreeMap::new();
    for night in nights {
        let Some(list) = night.techniques.as_deref() else {
            continue;
        };
        // Each gesture counts once per night however many times it names it.
        let mut seen: Vec<Did> = list.split(',').filter_map(Did::parse).collect();
        seen.sort_by_key(|d| d.slug());
        seen.dedup();
        for did in seen {
            *counts.entry(did).or_default() += 1;
        }
    }
    #[allow(clippy::cast_precision_loss)]
    let floor = (nights.len() as f64 * AGREE).ceil() as usize;
    let mut out: Vec<(Did, usize)> = counts
        .into_iter()
        .filter(|(_, n)| *n >= floor.max(1))
        .collect();
    out.sort_by_key(|(did, n)| (std::cmp::Reverse(*n), did.slug()));
    out.into_iter().map(|(did, _)| did).collect()
}

/// Play counts as shares of the total, commonest first.
fn shares(counts: Vec<(String, u32)>) -> Vec<(String, f64)> {
    let total: u32 = counts.iter().map(|(_, n)| *n).sum();
    if total == 0 {
        return Vec::new();
    }
    counts
        .into_iter()
        .map(|(genre, n)| (genre, f64::from(n) / f64::from(total)))
        .collect()
}

// -- what a profile may do about it ------------------------------------------

/// Whether djmanzo may act on a fit by itself, or has to be asked.
///
/// The distinction is §78's, not a preference. That section's whole subject is
/// *what the software may change about itself without being asked*, and a
/// profile is the strongest thing djmanzo has to say about a DJ — which makes
/// it exactly the thing most likely to be applied too eagerly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Doing {
    /// Safe unasked: it changes how much fits on a screen, and §78 names that
    /// as a freedom the DJ can withdraw with one lock.
    ItsOwn,
    /// Offered, and nothing happens until the DJ presses it.
    IfAsked,
}

impl Doing {
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::ItsOwn => "its-own",
            Self::IfAsked => "if-asked",
        }
    }
}

/// One thing a profile would have djmanzo change.
#[derive(Debug, Clone, PartialEq)]
pub struct Fit<T> {
    /// What it would be changed to.
    pub to: T,
    pub doing: Doing,
    /// The evidence, in Rust's words. See [`Profile::words`] for why here.
    pub because: String,
}

/// What tonight's profile would fit, and what it is deliberately not offering.
///
/// Both halves matter. A profile that silently declined to act on a lock would
/// look exactly like one with nothing to say, and a DJ who locked the density
/// months ago is entitled to know that is why the interface is not moving.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Fits {
    pub density: Option<Fit<crate::cockpit::Density>>,
    pub posture: Option<Fit<Posture>>,
    /// Where the profile had an answer and djmanzo is not offering it, and why.
    pub withheld: Vec<String>,
}

/// What this kind of night's profile would fit the cockpit and the assistant to.
///
/// **The other half of §81.** A profile has been a thing djmanzo could *say*
/// about a DJ since it was written; §81 lists density and automation tolerance
/// among the five things a conditional profile differs in, and until this the
/// only one of the five anything acted on was the genre weights — see §12's
/// row. This is the doing, for the other two.
///
/// # Two different kinds of change, and why they are not treated alike
///
/// **Density is presentation.** §78 names *no automatic surface resizing* as
/// one of its four freedoms and §79 gives it a lock of its own, so a DJ who
/// does not want it has already said so in the one place made for saying it.
/// With the lock off, a profile moving the band is the same kind of act as the
/// window moving it — and the profile has better evidence, because it is three
/// or more nights of what this DJ actually ran at rather than one window
/// height.
///
/// **Posture is autonomy**, which is not presentation and has no lock because
/// it has its own control. §9's rule is that autonomy above confidence is
/// unsafe, and a profile is a statement about *past* nights: it is evidence
/// about a habit and not about tonight's certainty. So the rule here is
/// asymmetric and deliberately so — djmanzo may turn its own autonomy **down**
/// on a profile, and may only ever *offer* to turn it up. Quieting itself
/// without being asked costs the DJ nothing they cannot undo in one press;
/// the other direction hands a machine the mix because of what happened on
/// three previous evenings.
///
/// A field the profile has no answer for produces nothing at all, and a field
/// already where the profile wants it produces nothing either: a panel
/// offering to change something to what it already is is noise with a button
/// on it.
#[must_use]
pub fn fits(
    profile: &Profile,
    density_now: Option<crate::cockpit::Density>,
    posture_now: Posture,
    permits: crate::cockpit::Permits,
) -> Fits {
    use crate::cockpit::{Density, Freedom};

    let mut out = Fits::default();
    let over = format!(
        "over {} {} night{}",
        profile.nights(),
        profile.setting().title().to_lowercase(),
        if profile.nights() == 1 { "" } else { "s" }
    );

    if let Some(wants) = profile.density().and_then(Density::named) {
        if density_now == Some(wants) {
            // Already there. Nothing to say.
        } else if permits.allows(Freedom::Resize) {
            out.density = Some(Fit {
                to: wants,
                doing: Doing::ItsOwn,
                because: format!("You run {} {over}.", wants.name()),
            });
        } else {
            out.withheld.push(format!(
                "You run {} {over}. The density is locked, so djmanzo is leaving it.",
                wants.name()
            ));
        }
    }

    if let Some(wants) = profile.automation()
        && wants != posture_now
    {
        // Quieter than the assistant is now, on the axis `Posture::ALL` states.
        let quieter = rank(wants) < rank(posture_now);
        out.posture = Some(Fit {
            to: wants,
            doing: if quieter {
                Doing::ItsOwn
            } else {
                Doing::IfAsked
            },
            because: format!("You keep the assistant on {} {over}.", wants.name()),
        });
    }

    out
}

/// Where a posture sits on the autonomy axis.
///
/// [`Posture::ALL`] is documented as quietest first, so its position in that
/// list *is* the ordering. Read from it rather than written out again here,
/// because a seventh posture added to the enum would otherwise be ranked by a
/// table nobody remembered to update — and it would be ranked as the quietest,
/// which is the direction djmanzo acts on unasked.
fn rank(posture: Posture) -> usize {
    Posture::ALL
        .iter()
        .position(|p| *p == posture)
        .unwrap_or(usize::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn night(id: &str, setting: &str) -> Night {
        Night {
            session_id: id.to_owned(),
            setting: setting.to_owned(),
            began_at: 0,
            density: None,
            style: None,
            posture: None,
            techniques: None,
        }
    }

    fn nights(setting: &str, count: usize) -> Vec<Night> {
        (0..count)
            .map(|n| night(&format!("{setting}-{n}"), setting))
            .collect()
    }

    fn nothing(_: Setting) -> Vec<(String, u32)> {
        Vec::new()
    }

    /// A profile of one setting whose nights played these genres.
    fn profile_of(setting: &str, plays: &[(&str, u32)]) -> Profile {
        let counted: Vec<(String, u32)> = plays
            .iter()
            .map(|(name, n)| ((*name).to_owned(), *n))
            .collect();
        let genres = |_: Setting| counted.clone();
        profiles(&nights(setting, ENOUGH_NIGHTS), &genres)
            .into_iter()
            .next()
            .expect("enough nights for a profile")
    }

    /// A profile whose nights all ran at one density and one posture.
    fn settled(setting: &str, density: &str, posture: Posture) -> Profile {
        let mut seen = nights(setting, ENOUGH_NIGHTS);
        for night in &mut seen {
            night.density = Some(density.to_owned());
            night.posture = Some(posture.name().to_owned());
        }
        profiles(&seen, &nothing)
            .into_iter()
            .next()
            .expect("enough nights for a profile")
    }

    /// **djmanzo may quiet itself on a profile; it may never let itself loose
    /// on one.**
    ///
    /// The load-bearing rule of this half of §81, and the one worth a test of
    /// its own: a profile is evidence about *past* nights, and §9's rule is
    /// that autonomy above confidence is unsafe. Turning the assistant down
    /// costs a DJ one press to undo. Turning it up because of what happened on
    /// three previous evenings hands a machine the mix.
    #[test]
    fn a_profile_turns_the_assistant_down_by_itself_and_only_ever_offers_to_turn_it_up() {
        let quiet = settled("wedding", "Standard", Posture::Watch);
        let loud = settled("club", "Standard", Posture::Autopilot);

        let down = fits(
            &quiet,
            Some(crate::cockpit::Density::Standard),
            Posture::Autopilot,
            crate::cockpit::Permits::everything(),
        );
        let up = fits(
            &loud,
            Some(crate::cockpit::Density::Standard),
            Posture::Watch,
            crate::cockpit::Permits::everything(),
        );

        let down = down.posture.expect("a quieter posture was not offered");
        assert_eq!(down.to, Posture::Watch);
        assert_eq!(
            down.doing,
            Doing::ItsOwn,
            "djmanzo asked permission to make itself quieter"
        );

        let up = up.posture.expect("a louder posture was not offered at all");
        assert_eq!(up.to, Posture::Autopilot);
        assert_eq!(
            up.doing,
            Doing::IfAsked,
            "a profile handed the machine the mix without being asked"
        );
    }

    /// **A locked density is not moved, and the DJ is told that is why.**
    ///
    /// §79's lock, honoured at the one place a profile could ignore it. The
    /// silent version of this is worse than not building it: an interface that
    /// declined to act and said nothing looks exactly like one with nothing to
    /// say, and a DJ who locked the density months ago has no way to connect
    /// the two.
    #[test]
    fn a_locked_density_is_left_alone_and_the_reason_is_said() {
        let profile = settled("club", "Pro Dense", Posture::Suggest);
        let mut locked = crate::cockpit::Permits::everything();
        locked.resize = false;

        let free = fits(
            &profile,
            Some(crate::cockpit::Density::Standard),
            Posture::Suggest,
            crate::cockpit::Permits::everything(),
        );
        let held = fits(
            &profile,
            Some(crate::cockpit::Density::Standard),
            Posture::Suggest,
            locked,
        );

        assert_eq!(
            free.density.as_ref().map(|fit| fit.to),
            Some(crate::cockpit::Density::ProDense),
        );
        assert!(free.withheld.is_empty());
        assert!(held.density.is_none(), "a locked density was moved anyway");
        assert_eq!(held.withheld.len(), 1, "it was withheld without a word");
        assert!(
            held.withheld[0].contains("Pro Dense") && held.withheld[0].contains("locked"),
            "the reason does not say what was withheld or why: {}",
            held.withheld[0]
        );
    }

    /// **Nothing is offered where nothing would change.**
    ///
    /// A panel offering to set something to what it already is is noise with a
    /// button on it, and a DJ who pressed it and saw nothing happen would stop
    /// reading the panel.
    #[test]
    fn a_profile_that_already_has_what_it_wants_offers_nothing() {
        let profile = settled("wedding", "Compact", Posture::Prepare);
        let nothing_to_do = fits(
            &profile,
            Some(crate::cockpit::Density::Compact),
            Posture::Prepare,
            crate::cockpit::Permits::everything(),
        );
        assert_eq!(nothing_to_do, Fits::default());
    }

    /// A profile with no answer for a field says nothing about it, which is
    /// not the same as having nothing to say at all.
    #[test]
    fn a_profile_silent_on_one_field_still_speaks_on_the_other() {
        let mut seen = nights("club", ENOUGH_NIGHTS);
        // Every night agrees about the density and no two agree about the
        // posture, so the profile has one answer and not the other.
        for (night, posture) in
            seen.iter_mut()
                .zip([Posture::Autopilot, Posture::Off, Posture::Prepare])
        {
            night.density = Some("Ultra Dense".to_owned());
            night.posture = Some(posture.name().to_owned());
        }
        let profile = profiles(&seen, &nothing)
            .into_iter()
            .next()
            .expect("enough nights");
        assert!(
            profile.automation().is_none(),
            "three nights that disagreed became a habit"
        );

        let fit = fits(
            &profile,
            Some(crate::cockpit::Density::Standard),
            Posture::Suggest,
            crate::cockpit::Permits::everything(),
        );
        assert_eq!(
            fit.density.map(|f| f.to),
            Some(crate::cockpit::Density::UltraDense)
        );
        assert!(fit.posture.is_none());
    }

    /// The autonomy order is read from `Posture::ALL` rather than written out
    /// again, so this asserts the property that makes that safe.
    #[test]
    fn the_quietest_posture_ranks_lowest_and_the_loudest_highest() {
        assert_eq!(rank(Posture::Off), 0);
        assert_eq!(rank(Posture::Autopilot), Posture::ALL.len() - 1);
        for pair in Posture::ALL.windows(2) {
            assert!(
                rank(pair[0]) < rank(pair[1]),
                "{} does not rank below {}",
                pair[0].name(),
                pair[1].name()
            );
        }
    }

    /// **A profile breaks ties; it cannot overrule the mixing.**
    ///
    /// The load-bearing bound. A same-key match is worth three on the
    /// suggester's scale and a key clash minus two and a half, so a tilt that
    /// could reach either would let "you play a lot of this at weddings"
    /// promote a record that does not mix. Nothing about a DJ's habits should
    /// be able to do that.
    #[test]
    fn a_profile_can_reorder_records_that_work_and_lift_none_that_do_not() {
        // As lopsided as a profile can get: one genre, every play.
        let lopsided = profile_of("wedding", &[("Bachata", 200), ("Merengue", 1)]);
        let tilt = lopsided.tilt_for(Some("Bachata"));
        assert!(tilt > 0.0, "the commonest genre was not preferred");
        assert!(
            tilt <= MOST_IT_MAY_MOVE,
            "a profile moved a record by {tilt}, past its own bound"
        );
        // And that bound, seen through the most lopsided profile there can
        // be, is well under what one key relation is worth: a same-key match
        // scores three and a clash minus two and a half. Asserted on the
        // *measured* tilt rather than on the constant, which clippy rightly
        // refuses — an assertion on a literal is a comment that can fail.
        assert!(
            tilt < 2.5,
            "the most a profile can move a record is {tilt}, which can cross a key clash"
        );
    }

    /// **An even night has no preferences, however many nights it rests on.**
    ///
    /// The comparison is against an even split over the genres *this kind of
    /// night contains*, not over the library. A wedding that is half bachata
    /// and half merengue prefers neither, and measuring against the whole
    /// collection would hand both a large leaning for being a wedding at all.
    #[test]
    fn a_night_that_plays_everything_equally_prefers_nothing() {
        let even = profile_of("wedding", &[("Bachata", 50), ("Merengue", 50)]);
        assert!(even.tilt_for(Some("Bachata")).abs() < 1e-9);
        assert!(even.tilt_for(Some("Merengue")).abs() < 1e-9);
        assert_eq!(
            even.because(Some("Bachata")),
            None,
            "it explained a nudge it did not make"
        );
    }

    /// **A record with no genre, and a genre this night has never seen, are
    /// left alone rather than pushed down.**
    ///
    /// Most collections are half-tagged. Penalising the untagged half would
    /// hide half a library behind a habit.
    #[test]
    fn what_the_profile_has_never_seen_is_left_where_it_was() {
        let known = profile_of("club", &[("Techno", 80), ("House", 20)]);
        assert!((known.tilt_for(None) - 0.0).abs() < f64::EPSILON);
        assert!((known.tilt_for(Some("Bachata")) - 0.0).abs() < f64::EPSILON);
        assert_eq!(known.because(None), None);
        assert_eq!(known.because(Some("Bachata")), None);
    }

    /// **The rarer half is pushed down as far as the commoner half is lifted.**
    ///
    /// A leaning is a ratio, and log space is what makes twice-as-often and
    /// half-as-often equal and opposite. Without it a profile would only ever
    /// promote, and a night's ranking would drift one way all evening.
    #[test]
    fn a_leaning_and_its_reciprocal_are_equal_and_opposite() {
        // Three genres, so an even split is a third; 45 of 90 is one and a
        // half times that and 20 of 90 is two thirds of it, which are
        // reciprocals. Chosen to sit *inside* the bound — at twice and half
        // an even split both ends are already clamped, and a clamped pair
        // would look symmetric whatever the map did.
        let leaning = profile_of("club", &[("Techno", 45), ("House", 20), ("Disco", 25)]);
        let up = leaning.tilt_for(Some("Techno"));
        let down = leaning.tilt_for(Some("House"));
        assert!(up > 0.0 && down < 0.0, "{up} {down}");
        assert!(
            (up + down).abs() < 1e-9,
            "{up} against {down} is not opposite"
        );
        assert!(
            up < MOST_IT_MAY_MOVE && down > -MOST_IT_MAY_MOVE,
            "the fixture is clamped, so it proves nothing: {up} {down}"
        );
    }

    /// And a leaning past the bound stops at it, both ways.
    #[test]
    fn a_lopsided_night_is_held_at_the_bound() {
        let lopsided = profile_of("club", &[("Techno", 200), ("House", 1)]);
        assert!((lopsided.tilt_for(Some("Techno")) - MOST_IT_MAY_MOVE).abs() < 1e-9);
        assert!((lopsided.tilt_for(Some("House")) + MOST_IT_MAY_MOVE).abs() < 1e-9);
    }

    /// The reason names the setting and the share, so a DJ can disagree with
    /// the specific thing rather than with the whole ranking. §42.
    #[test]
    fn the_reason_names_the_night_and_the_evidence() {
        let wedding = profile_of("wedding", &[("Bachata", 80), ("Merengue", 20)]);
        let because = wedding
            .because(Some("Bachata"))
            .expect("a leaning worth explaining");
        assert_eq!(because, "80% of a wedding night");
        assert!(
            wedding.because(Some("bachata")).is_some(),
            "the genre was matched case-sensitively"
        );
    }

    /// **§81's whole point: two settings are two answers, never their
    /// average.**
    ///
    /// A DJ who plays bachata at weddings and techno at clubs, averaged, is a
    /// DJ who plays neither — and the average has twice the evidence of either
    /// real answer, so a system offering it would offer it strongly.
    #[test]
    fn a_wedding_and_a_club_are_two_profiles_and_not_one() {
        let mut rows = nights("wedding", 3);
        rows.extend(nights("club", 3));
        for row in &mut rows {
            row.style = Some(
                if row.setting == "wedding" {
                    "fade"
                } else {
                    "cut"
                }
                .to_owned(),
            );
        }

        let genres = |setting: Setting| match setting {
            Setting::Wedding => vec![("Bachata".to_owned(), 9), ("Techno".to_owned(), 1)],
            Setting::Club => vec![("Techno".to_owned(), 10)],
            _ => Vec::new(),
        };
        let built = profiles(&rows, &genres);

        assert_eq!(built.len(), 2, "two settings collapsed into {built:#?}");
        let wedding = built
            .iter()
            .find(|p| p.setting() == Setting::Wedding)
            .expect("no wedding profile");
        let club = built
            .iter()
            .find(|p| p.setting() == Setting::Club)
            .expect("no club profile");

        assert_eq!(wedding.style(), Some(TransitionStyle::Fade));
        assert_eq!(club.style(), Some(TransitionStyle::Cut));
        assert_eq!(wedding.genres()[0].0, "Bachata");
        assert_eq!(club.genres()[0].0, "Techno");
        // And neither is a blend of the two.
        assert!(
            wedding
                .genres()
                .iter()
                .all(|(g, share)| g != "Techno" || *share < 0.2)
        );
    }

    /// **One night is not a habit.**
    ///
    /// The §13 rule at the scale of a whole evening. A setting under the
    /// threshold produces *nothing*, not an empty profile — an interface draws
    /// an empty profile as one that knows nothing about you, which is a
    /// different and much less useful claim than one that does not exist yet.
    #[test]
    fn a_setting_below_the_threshold_produces_no_profile_at_all() {
        // The numbers, not `ENOUGH_NIGHTS`. Written against the constant this
        // test moves with it and defends nothing — lowering the threshold to
        // one left it green, which is the whole judgement gone. **One evening
        // is not a habit and neither is two.**
        for count in [0, 1, 2] {
            assert!(
                profiles(&nights("beach", count), &nothing).is_empty(),
                "{count} night(s) was enough to generalise about a whole setting"
            );
        }
        assert_eq!(profiles(&nights("beach", 3), &nothing).len(), 1);
        assert_eq!(
            ENOUGH_NIGHTS, 3,
            "the threshold moved; the three cases above are the claim"
        );
    }

    /// **A minority is not reported as a habit.**
    ///
    /// Two blends out of five is what the DJ did twice, not how they mix.
    /// Reporting it would put their own outlier to them as their signature.
    #[test]
    fn a_field_stays_quiet_until_enough_of_the_nights_agree() {
        let mut rows = nights("club", 5);
        rows[0].style = Some("blend".to_owned());
        rows[1].style = Some("blend".to_owned());
        rows[2].style = Some("cut".to_owned());
        rows[3].style = Some("fade".to_owned());
        rows[4].style = Some("echo".to_owned());
        assert_eq!(profiles(&rows, &nothing)[0].style(), None);

        // Three of five agreeing is enough.
        rows[3].style = Some("blend".to_owned());
        assert_eq!(
            profiles(&rows, &nothing)[0].style(),
            Some(TransitionStyle::Blend)
        );
    }

    /// **A quiet night is not a vote against.**
    ///
    /// A night with no transitions has no opinion about transition style.
    /// Counting it as disagreement would make a real habit vanish the moment
    /// the DJ had an easy evening.
    #[test]
    fn a_night_with_nothing_to_say_is_not_counted_against() {
        let mut rows = nights("latin", 4);
        rows[0].style = Some("blend".to_owned());
        rows[1].style = Some("blend".to_owned());
        // rows[2] and rows[3] said nothing at all.
        assert_eq!(
            profiles(&rows, &nothing)[0].style(),
            Some(TransitionStyle::Blend),
            "two silent nights outvoted two that spoke"
        );
    }

    /// **A gesture counts once per night, not once per press.**
    ///
    /// Forty loops in one evening and none since is a thing done once. Counted
    /// per occurrence it would be the DJ's signature technique.
    #[test]
    fn a_technique_is_counted_per_night_rather_than_per_press() {
        let mut rows = nights("club", 4);
        rows[0].techniques = Some("looped,looped,looped,looped".to_owned());
        rows[1].techniques = Some("eq-moved".to_owned());
        rows[2].techniques = Some("eq-moved".to_owned());
        rows[3].techniques = Some("eq-moved".to_owned());

        let built = profiles(&rows, &nothing);
        assert_eq!(
            built[0].techniques(),
            &[Did::EqMoved],
            "one busy night outranked three consistent ones"
        );
    }

    /// A stored setting djmanzo does not know belongs to no profile, rather
    /// than to the default one — the same refusal `Setting::parse` makes.
    #[test]
    fn a_night_of_an_unknown_setting_joins_nothing() {
        let mut rows = nights("bar-mitzvah", 5);
        rows.extend(nights("club", 3));
        let built = profiles(&rows, &nothing);
        assert_eq!(built.len(), 1);
        assert_eq!(built[0].setting(), Setting::Club);
        assert_eq!(built[0].nights(), 3, "unknown nights were counted as club");
    }

    /// **The sentence always names the setting and how many nights it is
    /// from.**
    ///
    /// A profile read without either is exactly the universal profile §81
    /// forbids — and the words are written here so that a panel cannot
    /// assemble the parts into a claim djmanzo would not make.
    #[test]
    fn the_words_never_drop_the_setting_or_the_evidence() {
        let mut rows = nights("wedding", 3);
        for row in &mut rows {
            row.style = Some("fade".to_owned());
            row.posture = Some("prepare".to_owned());
        }
        let said = profiles(&rows, &|_| vec![("Bachata".to_owned(), 4)])[0].words();
        assert!(said.contains("Wedding"), "{said}");
        assert!(said.contains("3 nights"), "{said}");
        assert!(said.contains("fade"), "{said}");
        assert!(said.contains("Bachata"), "{said}");
        assert!(said.contains("prepare"), "{said}");
    }

    /// And it says so plainly when it cannot say how you mix, rather than
    /// leaving a sentence that reads as though it had.
    #[test]
    fn it_admits_when_it_cannot_say_how_you_mix() {
        let said = profiles(&nights("practice", 3), &nothing)[0].words();
        assert!(said.contains("too varied"), "{said}");
        assert!(said.contains("Practice"), "{said}");
    }

    /// Genre shares are shares: they sum to one over what was played.
    #[test]
    fn genre_weights_are_shares_of_what_was_played() {
        let genres = |_| vec![("Bachata".to_owned(), 3), ("Salsa".to_owned(), 1)];
        let built = profiles(&nights("latin", 3), &genres);
        let shares = built[0].genres();
        assert_eq!(shares[0], ("Bachata".to_owned(), 0.75));
        assert_eq!(shares[1], ("Salsa".to_owned(), 0.25));
        let total: f64 = shares.iter().map(|(_, s)| s).sum();
        assert!((total - 1.0).abs() < 1e-9);
    }
}
