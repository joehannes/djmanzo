//! The packs that ship with the application.
//!
//! Chosen to be useful on the first night rather than to demonstrate the
//! mechanism. Everything here is something a DJ actually does, expressed in the
//! action vocabulary that exists today — which is why they set the desk rather
//! than steering the music. The musical half needs M2 and M3.
//!
//! The Caribbean pack is not decoration. djmanzo is built for a Dominican DJ,
//! and the moves that matter in bachata and merengue are not the ones that
//! matter in house.

use crate::{Category, Pack, Preset};

fn preset(
    id: &str,
    name: &str,
    description: &str,
    category: Category,
    per_deck: bool,
    actions: &[&str],
) -> Preset {
    Preset {
        id: id.to_owned(),
        name: name.to_owned(),
        description: description.to_owned(),
        category,
        actions: actions.iter().map(|a| (*a).to_owned()).collect(),
        per_deck,
    }
}

fn pack(id: &str, name: &str, description: &str, presets: Vec<Preset>) -> Pack {
    Pack {
        id: id.to_owned(),
        name: name.to_owned(),
        description: description.to_owned(),
        presets,
        user: false,
    }
}

/// Every built-in pack.
#[must_use]
pub fn packs() -> Vec<Pack> {
    vec![phases(), prep(), moves(), caribbean(), karaoke()]
}

/// Where you are in the night.
fn phases() -> Pack {
    pack(
        "phases",
        "Session phases",
        "Sets the desk for where you are in the night. These prepare the mixer; \
         steering tempo, genre and energy needs the analyser and library (M2, \
         M3), and these packs will grow into that.",
        vec![
            preset(
                "phase-warmup",
                "Warm up",
                "Everything flat and open, master down a little. Nothing dramatic \
                 while the room is filling.",
                Category::Phase,
                false,
                &["master gain -3", "crossfader 0", "cue mix 0"],
            ),
            preset(
                "phase-fiesta",
                "Fiesta",
                "Master back up, headphones split so you can work the next track \
                 while the floor is going.",
                Category::Phase,
                false,
                &["master gain 0", "cue split_on", "crossfader 0"],
            ),
            preset(
                "phase-peak",
                "Peak",
                "Full level, headphones on the incoming track only. The point of \
                 the night where you are cueing constantly.",
                Category::Phase,
                false,
                &["master gain 0", "cue split_off", "cue mix 0"],
            ),
            preset(
                "phase-slowdown",
                "Slow down",
                "Ease the master back and blend the headphones, for the stretch \
                 where you are bringing the energy down deliberately.",
                Category::Phase,
                false,
                &["master gain -2", "cue mix 0.3", "cue split_off"],
            ),
            preset(
                "phase-chillout",
                "Chill out",
                "Quieter still, and the booth down with it. People are talking \
                 again and should be able to.",
                Category::Phase,
                false,
                &["master gain -5", "booth gain -6", "cue mix 0.5"],
            ),
            preset(
                "phase-close",
                "Close",
                "Last track level. Down far enough that the lights coming up is \
                 not a shock.",
                Category::Phase,
                false,
                &["master gain -8", "booth gain -10"],
            ),
        ],
    )
}

/// Getting a deck ready before it goes out.
fn prep() -> Pack {
    pack(
        "prep",
        "Deck preparation",
        "Getting a deck ready before anyone hears it, so nothing goes out with a \
         setting left over from the last track.",
        vec![
            preset(
                "prep-cue",
                "Cue it up",
                "Fader down, headphones on, keylock engaged, EQ flat. The state \
                 you want before beatmatching anything.",
                Category::Prep,
                true,
                &[
                    "deck {deck} volume 0",
                    "deck {deck} cue_on",
                    "deck {deck} keylock_on",
                    "deck {deck} eq_low 1",
                    "deck {deck} eq_mid 1",
                    "deck {deck} eq_high 1",
                    "deck {deck} filter 0",
                    "deck {deck} pitch 0",
                ],
            ),
            preset(
                "prep-bassless",
                "Ready without bass",
                "Cued up with the low end already out, so the incoming track can \
                 be brought in under a playing one without two kicks fighting.",
                Category::Prep,
                true,
                &[
                    "deck {deck} volume 0",
                    "deck {deck} cue_on",
                    "deck {deck} keylock_on",
                    "deck {deck} eq_low 0",
                    "deck {deck} eq_mid 1",
                    "deck {deck} eq_high 1",
                ],
            ),
            preset(
                "prep-reset",
                "Reset the strip",
                "Everything on this deck back to neutral. The one to reach for \
                 when you have lost track of what is set where.",
                Category::Prep,
                true,
                &[
                    "deck {deck} eq_low 1",
                    "deck {deck} eq_mid 1",
                    "deck {deck} eq_high 1",
                    "deck {deck} filter 0",
                    "deck {deck} gain 0",
                    "deck {deck} pitch 0",
                    "deck {deck} volume 1",
                ],
            ),
        ],
    )
}

/// Things done during a mix.
fn moves() -> Pack {
    pack(
        "moves",
        "Mix moves",
        "Single gestures as one press, for the things done with both hands \
         during a transition.",
        vec![
            preset(
                "move-bass-swap-in",
                "Take the bass",
                "Full low end on this deck. Pair it with the other deck's bass \
                 kill for the standard swap.",
                Category::Eq,
                true,
                &["deck {deck} eq_low 1"],
            ),
            preset(
                "move-bass-out",
                "Give up the bass",
                "Low end out on this deck, everything else untouched.",
                Category::Eq,
                true,
                &["deck {deck} eq_low 0"],
            ),
            preset(
                "move-isolate-vocal",
                "Lift the top",
                "Bass out and mids up: leaves the vocal and the top sitting over \
                 whatever is playing underneath.",
                Category::Eq,
                true,
                &[
                    "deck {deck} eq_low 0",
                    "deck {deck} eq_mid 1.4",
                    "deck {deck} eq_high 1.2",
                ],
            ),
            preset(
                "move-filter-build",
                "Filter build",
                "Sweeps the low-pass most of the way up for a build. Ride the \
                 filter from here by hand.",
                Category::Move,
                true,
                &["deck {deck} filter 0.7"],
            ),
            preset(
                "move-drop",
                "Drop",
                "Filter off, EQ flat, full fader. Everything back at once.",
                Category::Move,
                true,
                &[
                    "deck {deck} filter 0",
                    "deck {deck} eq_low 1",
                    "deck {deck} eq_mid 1",
                    "deck {deck} eq_high 1",
                    "deck {deck} volume 1",
                ],
            ),
            preset(
                "move-headphones-both",
                "Hear both",
                "Headphones halfway between cue and master, so you can hear the \
                 blend rather than one side of it.",
                Category::Mixer,
                false,
                &["cue split_off", "cue mix 0.5"],
            ),
        ],
    )
}

/// The repertoire this application is actually for.
fn caribbean() -> Pack {
    pack(
        "caribbean",
        "Caribbean",
        "Moves that matter in bachata, merengue, dembow and reggaetón, where the \
         bass and the guira live in different places from house music.",
        vec![
            preset(
                "carib-bachata-blend",
                "Bachata blend",
                "Bachata's bass guitar sits high enough that a hard kill loses \
                 the groove. Takes the low end down rather than out, keeping the \
                 requinto and the guira present.",
                Category::Eq,
                true,
                &[
                    "deck {deck} eq_low 0.35",
                    "deck {deck} eq_mid 1",
                    "deck {deck} eq_high 1.1",
                ],
            ),
            preset(
                "carib-merengue-drive",
                "Merengue drive",
                "Mids and top up: merengue lives in the guira and the tambora, \
                 and pushing the low end only muddies it.",
                Category::Eq,
                true,
                &[
                    "deck {deck} eq_low 0.9",
                    "deck {deck} eq_mid 1.25",
                    "deck {deck} eq_high 1.3",
                ],
            ),
            preset(
                "carib-dembow-swap",
                "Dembow swap",
                "Dembow's whole identity is the kick pattern, so two of them at \
                 once is unusable. Bass fully out for the overlap.",
                Category::Eq,
                true,
                &["deck {deck} eq_low 0", "deck {deck} eq_mid 1.1"],
            ),
            preset(
                "carib-tipico-open",
                "Típico wide open",
                "Flat and loud. Típico is fast and busy already and does not want \
                 help from the EQ.",
                Category::Prep,
                true,
                &[
                    "deck {deck} eq_low 1",
                    "deck {deck} eq_mid 1",
                    "deck {deck} eq_high 1",
                    "deck {deck} filter 0",
                    "deck {deck} volume 1",
                ],
            ),
        ],
    )
}

/// A karaoke night: what a host does by hand between the rotation's own
/// buttons (*Load*, *Bring them in*, *Sang*, in the Karaoke panel).
///
/// A singer is not a DJ's next record. The recorded voice comes out, the
/// key moves to where the singer can reach, and the tempo must not move
/// with it — and the room has open microphones in it, which a club night
/// does not.
fn karaoke() -> Pack {
    pack(
        "karaoke",
        "Karaoke night",
        "For hosting singers: the recorded voice out or left in as a guide, the \
         key moved to where a singer can reach it, and the mixer set for open \
         microphones in the room.",
        vec![
            preset(
                "karaoke-vocals-out",
                "Vocals out",
                "The singer's song with the recorded voice taken out, and key \
                 lock on, so slowing it down for a nervous singer never drops \
                 the key with it.",
                Category::Prep,
                true,
                &["deck {deck} voice 0", "deck {deck} keylock_on"],
            ),
            preset(
                "karaoke-guide-vocal",
                "Guide vocal",
                "A quarter of the recorded voice left in under the singer: \
                 enough to follow the melody, not enough to sing over. For a \
                 first-timer, or a song they half know.",
                Category::Prep,
                true,
                &["deck {deck} voice 0.25", "deck {deck} keylock_on"],
            ),
            preset(
                "karaoke-sing-along",
                "Everyone sings",
                "The record's own voice back in full, for a group number, a \
                 birthday or the last song of the night, when the room is the \
                 singer.",
                Category::Prep,
                true,
                &["deck {deck} voice 1"],
            ),
            preset(
                "karaoke-key-down",
                "Down a tone",
                "Two semitones lower, tempo untouched. Most people find a hit's \
                 original key a little high; this is the first thing to try \
                 when a chorus strains.",
                Category::Prep,
                true,
                &["deck {deck} keylock_on", "deck {deck} key -2"],
            ),
            preset(
                "karaoke-key-other-voice",
                "Down a fourth",
                "Five semitones lower: a song written for a high voice brought \
                 down to a low one, the usual move when a man sings a woman's \
                 song.",
                Category::Prep,
                true,
                &["deck {deck} keylock_on", "deck {deck} key -5"],
            ),
            preset(
                "karaoke-key-up",
                "Up a tone",
                "Two semitones higher, tempo untouched, for a singer whose voice \
                 sits above the record's.",
                Category::Prep,
                true,
                &["deck {deck} keylock_on", "deck {deck} key 2"],
            ),
            preset(
                "karaoke-key-home",
                "Original key",
                "Back to the key the record was made in, before the next \
                 singer or the break music.",
                Category::Prep,
                true,
                &["deck {deck} key 0"],
            ),
            preset(
                "karaoke-open-mics",
                "Open microphones",
                "Set before the first singer: the limiter on, the master two \
                 decibels down and the booth six, so a microphone pointed at a \
                 speaker has headroom before it howls.",
                Category::Mixer,
                false,
                &["limiter on", "master gain -2", "booth gain -6"],
            ),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_pack_has_an_id_a_name_and_a_real_description() {
        for pack in packs() {
            assert!(!pack.id.is_empty());
            assert!(!pack.name.is_empty());
            assert!(
                pack.description.len() > 25,
                "`{}` has a stub description",
                pack.id
            );
            assert!(
                !pack.user,
                "a built-in pack must not claim to be the user's"
            );
        }
    }

    /// A singer's song never changes tempo with its key, and a song with
    /// the voice out takes it fully out: a key move without key lock would
    /// drag the tempo, and a "vocals out" that left some in would be the
    /// guide vocal under another name.
    #[test]
    fn the_karaoke_pack_moves_the_key_not_the_tempo() {
        let pack = karaoke();
        let actions = |id: &str| {
            pack.presets
                .iter()
                .find(|p| p.id == id)
                .unwrap_or_else(|| panic!("no {id}"))
                .actions
                .clone()
        };
        for preset in &pack.presets {
            if preset
                .actions
                .iter()
                .any(|a| a.contains(" key -") || a.contains(" key 2"))
            {
                assert!(
                    preset.actions.iter().any(|a| a.ends_with("keylock_on")),
                    "{} moves the key without key lock",
                    preset.id
                );
            }
        }
        assert!(actions("karaoke-vocals-out").contains(&"deck {deck} voice 0".to_owned()));
        assert!(actions("karaoke-guide-vocal").contains(&"deck {deck} voice 0.25".to_owned()));
        assert!(actions("karaoke-open-mics").contains(&"limiter on".to_owned()));
        let fourth = actions("karaoke-key-other-voice");
        assert!(fourth.contains(&"deck {deck} key -5".to_owned()));
    }

    #[test]
    fn pack_ids_are_unique() {
        use std::collections::HashSet;
        let ids: HashSet<String> = packs().into_iter().map(|p| p.id).collect();
        assert_eq!(ids.len(), packs().len());
    }

    /// A per-deck preset must actually use the placeholder, and one that is not
    /// per-deck must not — otherwise the interface asks for a deck it will not
    /// use, or fails to ask for one it needs.
    #[test]
    fn the_per_deck_flag_matches_the_actions() {
        for pack in packs() {
            for preset in &pack.presets {
                let uses = preset.actions.iter().any(|a| a.contains("{deck}"));
                assert_eq!(
                    preset.per_deck,
                    uses,
                    "`{}` says per_deck={} but {} the placeholder",
                    preset.id,
                    preset.per_deck,
                    if uses { "uses" } else { "does not use" }
                );
            }
        }
    }

    /// The pack this project exists for.
    #[test]
    fn the_caribbean_pack_covers_the_main_genres() {
        let pack = caribbean();
        let text = format!("{pack:?}").to_lowercase();
        for genre in ["bachata", "merengue", "dembow", "típico"] {
            assert!(text.contains(genre), "{genre} missing from the pack");
        }
    }

    /// Bachata's bass guitar sits high enough that a hard kill loses the
    /// groove. If someone "simplifies" this to a kill, the test should object.
    #[test]
    fn the_bachata_blend_reduces_the_bass_rather_than_killing_it() {
        let preset = caribbean()
            .presets
            .into_iter()
            .find(|p| p.id == "carib-bachata-blend")
            .unwrap();
        let low = preset
            .actions
            .iter()
            .find(|a| a.contains("eq_low"))
            .expect("should set the low band");
        let value: f32 = low.split_whitespace().last().unwrap().parse().unwrap();
        assert!(
            value > 0.0 && value < 1.0,
            "bachata wants the bass down, not out: got {value}"
        );
    }
}
