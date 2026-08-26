//! Genre families, and the three facts about them a DJ application needs.
//!
//! A list of genre names is nearly useless. What a mixing decision actually
//! turns on is:
//!
//! 1. **Felt tempo is not written tempo.** Trap is written at 140 and felt at
//!    70, so it mixes with hip-hop at 70 and not with house at 140. A tempo
//!    comparison that ignores this rejects most of the pairings a real DJ
//!    makes.
//! 2. **Rhythmic grammar decides whether a blend is possible at all.** Dembow
//!    and four-on-the-floor are different grammars: their kicks do not land in
//!    the same places, and holding them together for eight bars sounds like a
//!    mistake however well the tempos match. Crossing grammars is a *cut*, or
//!    a deliberate effect, not a blend.
//! 3. **Families cross unevenly.** Amapiano into afro house is nothing;
//!    salsa into techno is a statement. The table below says which is which.
//!
//! # Scope, honestly
//!
//! This is a working DJ's map, not a musicology. The families are the ones that
//! fill floors in Latin America, North America, Europe and Africa, plus the
//! ones that travel. It will be wrong at the edges -- genre boundaries are
//! argued about by the people who make the music -- and the tempo ranges are
//! typical rather than exhaustive. It is written down so it can be argued with
//! and corrected, which is more than an implicit assumption allows.

use serde::{Deserialize, Serialize};

/// Where a family is principally danced. Not where it is *from* -- amapiano is
/// South African and fills rooms in London -- but it is the coarse grouping a
/// DJ building a night actually reaches for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Region {
    LatinAmerica,
    NorthAmerica,
    Europe,
    Africa,
    /// Travels widely, or belongs to no one region.
    Global,
}

/// How the written tempo relates to the one a body moves at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Feel {
    /// Felt as written.
    Straight,
    /// Felt at half the written tempo. Trap at 140 is danced at 70.
    Half,
    /// Written slow and felt fast -- salsa notated around 190 is counted in
    /// two, and a DJ thinks of it nearer 95.
    Double,
}

/// The pattern the kick and snare make. What decides whether two records can
/// be held together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Grammar {
    /// A kick on every beat. House, techno, disco, gqom, most of European
    /// dance music.
    FourOnFloor,
    /// The reggaetón/dembow pattern -- boom-ch-boom-chick. Also bachata's
    /// cousin, and the spine of most of what Latin America dances to now.
    Dembow,
    /// Broken kick and snare: drum and bass, jungle, breaks, footwork, jersey
    /// club, baile funk.
    Breakbeat,
    /// The hip-hop grammar: kick and snare on a slow backbeat, felt in half
    /// time.
    Boombap,
    /// Clave-based: salsa, son, timba, merengue, cumbia. A different rhythmic
    /// universe, and one that does not blend with a machine kick.
    Clave,
    /// Amapiano's log drum and afrobeats' loose, syncopated pulse. Neither
    /// four-on-the-floor nor broken; it swings.
    Loglines,
    /// No steady pulse to mix on: ambient, spoken word, film score.
    Free,
}

/// How well two families sit together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Blendability {
    /// Different grammars or wildly different feel. A cut, or nothing.
    Cut,
    /// Possible with care -- a short mix, or an effect over the seam.
    Careful,
    /// The same grammar at a workable tempo. Blend as long as you like.
    Easy,
}

/// One family.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Family {
    pub name: &'static str,
    pub region: Region,
    /// Typical **written** tempo range.
    pub bpm: (f32, f32),
    pub feel: Feel,
    pub grammar: Grammar,
    /// Other names the same music goes by, for matching a tag written by
    /// whoever ripped the file.
    pub aliases: &'static [&'static str],
}

impl Family {
    /// The tempo a body moves at, as a range.
    ///
    /// The number that matters for mixing. See the module docs: comparing
    /// written tempos puts trap and house together and trap and hip-hop apart,
    /// which is exactly backwards.
    #[must_use]
    pub fn felt_bpm(&self) -> (f32, f32) {
        match self.feel {
            Feel::Straight => self.bpm,
            Feel::Half => (self.bpm.0 / 2.0, self.bpm.1 / 2.0),
            Feel::Double => (self.bpm.0 / 2.0, self.bpm.1 / 2.0),
        }
    }

    /// Whether this family can be blended with another.
    #[must_use]
    pub fn blends_with(&self, other: &Family) -> Blendability {
        // Nothing blends with music that has no pulse.
        if self.grammar == Grammar::Free || other.grammar == Grammar::Free {
            return Blendability::Cut;
        }

        let (a_low, a_high) = self.felt_bpm();
        let (b_low, b_high) = other.felt_bpm();
        // Ranges overlap, allowing a little pitch either way.
        let margin = 0.06;
        let overlaps = a_low * (1.0 - margin) <= b_high && b_low * (1.0 - margin) <= a_high;

        if !overlaps {
            return Blendability::Cut;
        }
        if self.grammar == other.grammar {
            return Blendability::Easy;
        }
        // Different grammars at a workable tempo. Some pairs are ordinary
        // moves that DJs make all night; the rest are a cut.
        if neighbouring_grammars(self.grammar, other.grammar) {
            Blendability::Careful
        } else {
            Blendability::Cut
        }
    }
}

/// Grammar pairs that working DJs cross routinely.
///
/// Not symmetric-by-accident: written as an explicit list because the
/// asymmetries matter musically even where this function treats them as
/// symmetric, and a future version that distinguishes direction should start
/// from a list rather than a rule.
fn neighbouring_grammars(a: Grammar, b: Grammar) -> bool {
    let pair = |x, y| (a == x && b == y) || (a == y && b == x);
    // Afrobeats and amapiano sit against house all night in any club that
    // plays either.
    pair(Grammar::Loglines, Grammar::FourOnFloor)
        // Hip-hop and reggaetón share a floor and a half-time feel.
        || pair(Grammar::Boombap, Grammar::Dembow)
        // Broken beats over a straight kick is most of UK dance music.
        || pair(Grammar::Breakbeat, Grammar::FourOnFloor)
        // Dembow and amapiano both swing against the grid.
        || pair(Grammar::Dembow, Grammar::Loglines)
        // Hip-hop over broken beats: jersey club and footwork exist because of
        // this pairing.
        || pair(Grammar::Boombap, Grammar::Breakbeat)
}

/// The families djmanzo knows.
#[must_use]
pub fn families() -> &'static [Family] {
    FAMILIES
}

/// Find the family a genre tag names.
///
/// Matches the name or any alias, case- and punctuation-insensitively, because
/// a tag in a file was typed by a person: "Drum & Bass", "drum and bass",
/// "dnb" and "DNB" are the same music.
#[must_use]
pub fn family_for(tag: &str) -> Option<&'static Family> {
    let wanted = normalise(tag);
    if wanted.is_empty() {
        return None;
    }
    FAMILIES
        .iter()
        .find(|f| normalise(f.name) == wanted || f.aliases.iter().any(|a| normalise(a) == wanted))
}

/// Lowercase, letters and digits only, with `&` read as "and".
///
/// The ampersand matters: "Drum & Bass" and "drum and bass" are the same music
/// and both are in every library. Stripping the symbol without substituting the
/// word turns one into "drumbass" and the other into "drumandbass", which then
/// never match.
fn normalise(text: &str) -> String {
    text.replace('&', "and")
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Every family, grouped by region for reading.
static FAMILIES: &[Family] = &[
    // -- Latin America ----------------------------------------------------
    Family {
        name: "reggaeton",
        region: Region::LatinAmerica,
        bpm: (88.0, 100.0),
        feel: Feel::Straight,
        grammar: Grammar::Dembow,
        aliases: &["reggaetón", "perreo", "urbano latino"],
    },
    Family {
        name: "dembow",
        region: Region::LatinAmerica,
        bpm: (110.0, 125.0),
        feel: Feel::Straight,
        grammar: Grammar::Dembow,
        aliases: &["dembow dominicano"],
    },
    Family {
        name: "cumbia",
        region: Region::LatinAmerica,
        bpm: (85.0, 100.0),
        feel: Feel::Straight,
        grammar: Grammar::Clave,
        aliases: &["cumbia villera", "cumbia sonidera"],
    },
    Family {
        name: "bachata",
        region: Region::LatinAmerica,
        bpm: (118.0, 140.0),
        feel: Feel::Straight,
        grammar: Grammar::Clave,
        aliases: &["bachata moderna"],
    },
    Family {
        name: "merengue",
        region: Region::LatinAmerica,
        bpm: (120.0, 160.0),
        feel: Feel::Straight,
        grammar: Grammar::Clave,
        aliases: &["merengue típico", "perico ripiao"],
    },
    Family {
        name: "salsa",
        region: Region::LatinAmerica,
        bpm: (170.0, 200.0),
        feel: Feel::Double,
        grammar: Grammar::Clave,
        aliases: &["salsa dura", "timba", "son cubano"],
    },
    Family {
        name: "guaracha",
        region: Region::LatinAmerica,
        bpm: (126.0, 134.0),
        feel: Feel::Straight,
        grammar: Grammar::FourOnFloor,
        aliases: &["aleteo", "zapateo", "guaracha electronica"],
    },
    Family {
        name: "baile funk",
        region: Region::LatinAmerica,
        bpm: (125.0, 135.0),
        feel: Feel::Straight,
        grammar: Grammar::Breakbeat,
        aliases: &["funk carioca", "brazilian funk", "funk mandelao"],
    },
    Family {
        name: "moombahton",
        region: Region::LatinAmerica,
        bpm: (105.0, 112.0),
        feel: Feel::Straight,
        grammar: Grammar::Dembow,
        aliases: &[],
    },
    // -- North America ----------------------------------------------------
    Family {
        // Written wide on purpose. "Hip hop" as a floor spans classic boom bap
        // near 90 and modern half-time rap near 70 -- the same records a DJ
        // mixes with trap. A range that covered only boom bap put trap and
        // hip-hop on different floors, which a test caught and which no
        // open-format DJ would recognise.
        name: "hip hop",
        region: Region::NorthAmerica,
        bpm: (70.0, 100.0),
        feel: Feel::Straight,
        grammar: Grammar::Boombap,
        // No "hip-hop": punctuation is normalised away, so it already matches
        // the name and listing it is a duplicate the alias test catches.
        aliases: &["rap", "boom bap"],
    },
    Family {
        name: "trap",
        region: Region::NorthAmerica,
        bpm: (130.0, 150.0),
        feel: Feel::Half,
        grammar: Grammar::Boombap,
        aliases: &["drill"],
    },
    Family {
        name: "house",
        region: Region::NorthAmerica,
        bpm: (118.0, 128.0),
        feel: Feel::Straight,
        grammar: Grammar::FourOnFloor,
        aliases: &["deep house", "soulful house", "chicago house"],
    },
    Family {
        name: "disco",
        region: Region::NorthAmerica,
        bpm: (108.0, 126.0),
        feel: Feel::Straight,
        grammar: Grammar::FourOnFloor,
        aliases: &["nu-disco", "boogie"],
    },
    Family {
        name: "jersey club",
        region: Region::NorthAmerica,
        bpm: (130.0, 145.0),
        feel: Feel::Straight,
        grammar: Grammar::Breakbeat,
        aliases: &["baltimore club", "philly club"],
    },
    Family {
        name: "footwork",
        region: Region::NorthAmerica,
        bpm: (155.0, 165.0),
        feel: Feel::Half,
        grammar: Grammar::Breakbeat,
        aliases: &["juke", "ghetto house"],
    },
    Family {
        name: "rnb",
        region: Region::NorthAmerica,
        bpm: (60.0, 100.0),
        feel: Feel::Straight,
        grammar: Grammar::Boombap,
        aliases: &["r&b", "rhythm and blues", "contemporary r&b"],
    },
    // -- Europe -----------------------------------------------------------
    Family {
        name: "tech house",
        region: Region::Europe,
        bpm: (122.0, 130.0),
        feel: Feel::Straight,
        grammar: Grammar::FourOnFloor,
        // No "techhouse": punctuation and spacing are normalised away, so it
        // already matches the name.
        aliases: &["minimal", "microhouse"],
    },
    Family {
        name: "techno",
        region: Region::Europe,
        bpm: (128.0, 150.0),
        feel: Feel::Straight,
        grammar: Grammar::FourOnFloor,
        aliases: &["hard techno", "melodic techno", "industrial techno"],
    },
    Family {
        name: "trance",
        region: Region::Europe,
        bpm: (134.0, 142.0),
        feel: Feel::Straight,
        grammar: Grammar::FourOnFloor,
        aliases: &["psytrance", "progressive trance", "uplifting trance"],
    },
    Family {
        name: "drum and bass",
        region: Region::Europe,
        bpm: (168.0, 178.0),
        feel: Feel::Half,
        grammar: Grammar::Breakbeat,
        aliases: &["dnb", "d&b", "jungle", "liquid dnb"],
    },
    Family {
        name: "uk garage",
        region: Region::Europe,
        bpm: (128.0, 138.0),
        feel: Feel::Straight,
        grammar: Grammar::Breakbeat,
        aliases: &["ukg", "2-step", "speed garage", "bassline"],
    },
    Family {
        name: "dubstep",
        region: Region::Europe,
        bpm: (138.0, 145.0),
        feel: Feel::Half,
        grammar: Grammar::Breakbeat,
        aliases: &["grime", "riddim", "140"],
    },
    Family {
        name: "hardstyle",
        region: Region::Europe,
        bpm: (145.0, 160.0),
        feel: Feel::Straight,
        grammar: Grammar::FourOnFloor,
        aliases: &["hardcore", "gabber", "hard dance"],
    },
    Family {
        name: "eurodance",
        region: Region::Europe,
        bpm: (128.0, 145.0),
        feel: Feel::Straight,
        grammar: Grammar::FourOnFloor,
        aliases: &["italo disco", "hands up"],
    },
    // -- Africa -----------------------------------------------------------
    Family {
        name: "amapiano",
        region: Region::Africa,
        bpm: (110.0, 118.0),
        feel: Feel::Straight,
        grammar: Grammar::Loglines,
        aliases: &["piano", "private school amapiano", "3-step"],
    },
    Family {
        name: "afrobeats",
        region: Region::Africa,
        bpm: (98.0, 112.0),
        feel: Feel::Straight,
        grammar: Grammar::Loglines,
        aliases: &["afropop", "afro-fusion", "naija"],
    },
    Family {
        name: "afro house",
        region: Region::Africa,
        bpm: (118.0, 126.0),
        feel: Feel::Straight,
        grammar: Grammar::FourOnFloor,
        aliases: &["afro tech", "3 step house", "tribal house"],
    },
    Family {
        name: "gqom",
        region: Region::Africa,
        bpm: (120.0, 130.0),
        feel: Feel::Straight,
        grammar: Grammar::FourOnFloor,
        aliases: &["sgija"],
    },
    Family {
        name: "kuduro",
        region: Region::Africa,
        bpm: (135.0, 145.0),
        feel: Feel::Straight,
        grammar: Grammar::Breakbeat,
        aliases: &["batida", "afro house angolano"],
    },
    Family {
        name: "coupe decale",
        region: Region::Africa,
        bpm: (115.0, 130.0),
        feel: Feel::Straight,
        grammar: Grammar::Loglines,
        aliases: &["coupé-décalé", "ndombolo", "soukous"],
    },
    // -- Travels widely ---------------------------------------------------
    Family {
        name: "pop",
        region: Region::Global,
        bpm: (95.0, 130.0),
        feel: Feel::Straight,
        grammar: Grammar::FourOnFloor,
        aliases: &["top 40", "chart", "dance pop"],
    },
    Family {
        name: "ambient",
        region: Region::Global,
        bpm: (0.0, 0.0),
        feel: Feel::Straight,
        grammar: Grammar::Free,
        aliases: &["drone", "soundscape", "spoken word"],
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    fn family(name: &str) -> &'static Family {
        family_for(name).unwrap_or_else(|| panic!("no family called {name:?}"))
    }

    /// **Trap mixes with hip-hop, not with house.**
    ///
    /// The single most important thing this table encodes. Trap is written at
    /// 140 and danced at 70. A tempo comparison on the written number puts it
    /// beside house and away from hip-hop, which is exactly backwards, and
    /// would make the suggester reject most of what an open-format DJ actually
    /// plays.
    #[test]
    fn trap_is_felt_at_half_its_written_tempo() {
        let trap = family("trap");
        let (low, high) = trap.felt_bpm();
        assert!(
            (60.0..=80.0).contains(&low) && (60.0..=80.0).contains(&high),
            "trap is felt at {low}-{high}, expected around 70"
        );
        assert_eq!(
            trap.blends_with(family("hip hop")),
            Blendability::Easy,
            "trap and hip-hop were not recognised as the same floor"
        );
        assert_eq!(
            trap.blends_with(family("house")),
            Blendability::Cut,
            "trap and house were treated as mixable because both are near 140"
        );
    }

    /// **Salsa is counted in two.**
    ///
    /// Notated near 190 and thought of by a DJ nearer 95. Without this it would
    /// look like the fastest music in the library and mix with nothing.
    #[test]
    fn salsa_is_felt_in_two() {
        let (low, _) = family("salsa").felt_bpm();
        assert!(
            (80.0..=105.0).contains(&low),
            "salsa is felt at {low}, expected around 90"
        );
    }

    /// **Dembow and four-on-the-floor are different grammars.**
    ///
    /// Their kicks do not land in the same places, so holding them together for
    /// eight bars sounds like a mistake *however well the tempos match* -- and
    /// the table has to say so even when the numbers agree.
    ///
    /// The pair is dembow against afro house, both around 120, chosen because
    /// their felt tempos genuinely overlap. An earlier version used reggaetón
    /// against tech house, which are 90 and 125 apart: it passed with the
    /// grammar check deleted entirely, because the tempo test rejected the pair
    /// first. The test was measuring the wrong thing and mutation testing found
    /// it.
    #[test]
    fn dembow_and_four_on_the_floor_do_not_blend_despite_the_tempo() {
        let dembow = family("dembow");
        let afro = family("afro house");

        let (d_low, d_high) = dembow.felt_bpm();
        let (a_low, a_high) = afro.felt_bpm();
        assert!(
            d_low <= a_high && a_low <= d_high,
            "the fixture is not testing what it claims: {d_low}-{d_high} against \
             {a_low}-{a_high} do not overlap, so the tempo test would reject them \
             before the grammar is ever consulted"
        );

        assert_eq!(
            dembow.blends_with(afro),
            Blendability::Cut,
            "a dembow record was offered as blendable into four-on-the-floor at \
             the same tempo"
        );
    }

    /// **Amapiano into afro house is an ordinary move.**
    ///
    /// Any room that plays either plays both, all night. A table that called
    /// this a cut would be useless in exactly the places this music is played.
    #[test]
    fn amapiano_and_afro_house_sit_together() {
        assert!(
            family("amapiano").blends_with(family("afro house")) >= Blendability::Careful,
            "amapiano and afro house were treated as incompatible"
        );
    }

    /// **Nothing blends with music that has no pulse.**
    #[test]
    fn ambient_cannot_be_blended_with_anything() {
        for other in families() {
            assert_eq!(
                family("ambient").blends_with(other),
                Blendability::Cut,
                "ambient was offered as blendable with {}",
                other.name
            );
        }
    }

    /// **A genre tag was typed by a person.**
    ///
    /// "Drum & Bass", "drum and bass", "DnB" and "dnb" are the same music, and
    /// a library full of files ripped by different people contains all four.
    #[test]
    fn tags_match_however_they_were_typed() {
        for spelling in ["drum and bass", "Drum & Bass", "DnB", "d&b", "  dnb  "] {
            assert_eq!(
                family_for(spelling).map(|f| f.name),
                Some("drum and bass"),
                "{spelling:?} did not match"
            );
        }
        assert!(family_for("").is_none());
        assert!(family_for("polka").is_none());
    }

    /// **Blending is symmetric.**
    ///
    /// A into B must answer the same as B into A. It is easy to write a rule
    /// that is not, and the consequence is a suggester that offers a pairing in
    /// one direction and refuses it in the other -- which looks like a bug in
    /// the ranking rather than in the table.
    #[test]
    fn blendability_reads_the_same_both_ways() {
        for a in families() {
            for b in families() {
                assert_eq!(
                    a.blends_with(b),
                    b.blends_with(a),
                    "{} into {} disagrees with {} into {}",
                    a.name,
                    b.name,
                    b.name,
                    a.name
                );
            }
        }
    }

    /// Every family blends with itself. If one does not, its tempo range or
    /// its feel is wrong.
    #[test]
    fn every_family_blends_with_itself() {
        for f in families() {
            if f.grammar == Grammar::Free {
                continue;
            }
            assert_eq!(
                f.blends_with(f),
                Blendability::Easy,
                "{} does not mix with itself, so its tempo range is wrong",
                f.name
            );
        }
    }

    /// No two families share a name or an alias, or a tag would resolve to
    /// whichever happened to be listed first.
    #[test]
    fn no_name_or_alias_is_claimed_twice() {
        let mut seen: Vec<String> = Vec::new();
        for f in families() {
            for label in std::iter::once(f.name).chain(f.aliases.iter().copied()) {
                let key = normalise(label);
                assert!(
                    !seen.contains(&key),
                    "{label:?} is claimed by more than one family"
                );
                seen.push(key);
            }
        }
    }

    /// Every family covers all four regions plus global, so the table is not
    /// quietly Eurocentric.
    #[test]
    fn every_region_is_represented() {
        for region in [
            Region::LatinAmerica,
            Region::NorthAmerica,
            Region::Europe,
            Region::Africa,
            Region::Global,
        ] {
            assert!(
                families().iter().any(|f| f.region == region),
                "{region:?} has no families at all"
            );
        }
    }

    /// Tempo ranges are the right way round and plausible.
    #[test]
    fn tempo_ranges_are_ordered_and_sane() {
        for f in families() {
            assert!(f.bpm.0 <= f.bpm.1, "{}'s range runs backwards", f.name);
            if f.grammar != Grammar::Free {
                assert!(
                    f.bpm.0 >= 50.0 && f.bpm.1 <= 220.0,
                    "{} claims {:?}, which is not a dance tempo",
                    f.name,
                    f.bpm
                );
            }
        }
    }
}
