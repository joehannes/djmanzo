//! §109's activities: what a DJ is doing right now, and the screen for it.
//!
//! > create a different more interactive GUI mode ... where the current screen
//! > basically show the necessary (as of current activity) controls/widgets
//! > only ... then you can change activies. there's pre-arranged activities
//! > and the user can create his own
//! >
//! > i 's also like the ability to nicely switch those activies comfortable and
//! > easily and in a way that fits the task of a DJ (live)
//!
//! # An activity is not a second kind of workspace
//!
//! §7's workspaces already say which panels are open, how many decks there
//! are, what a deck is made of, how dense it is and what the DJ is focused on
//! — everything an activity needs to set. What they are organised by is
//! *style*: Club, Wedding, Latin, Scratch, Laptop Compact. Twenty-three of
//! them, which is a menu to choose from before a set and not a thing to move
//! between during one.
//!
//! An activity is organised by *the job in front of the DJ this minute* —
//! find the next record, blend into it, play with it, answer the room — and
//! there are few enough to have a key each. So each one here **carries a
//! workspace** and nothing else of its own but a name, a sentence, an icon and
//! a key. Choosing one applies its workspace through the same path choosing a
//! workspace always took, so pinned panels, locks and §5B's deck compositions
//! all behave exactly as they already do. A second description of what is on
//! screen is the failure this codebase keeps finding; this is not one.
//!
//! # What makes switching fit a live set
//!
//! Rules the interface keeps, stated here so they are one decision:
//!
//! - **The decks are always there.** Every activity keeps the performance zone.
//!   Whatever the DJ is doing, the record playing is on screen, so no switch
//!   ever hides the thing the room is hearing.
//! - **One key each, and one key back.** The shipped activities are `F1` to
//!   `F8`, a DJ's own take the next free ones, and the interface keeps the
//!   last one so a single key (`` ` ``) returns to it — digging for a record
//!   and coming back to the mix is the most common round trip of a night.
//!   Not the digits: those are the hot cues. See [`key_for`].
//! - **The density is the DJ's, not the activity's.** Every workspace names
//!   one, so these do too, and the interface keeps the DJ's own instead: how
//!   big things are drawn belongs to the window and the eyes in front of it.
//!   The first version applied it, and every switch rescaled the whole
//!   interface and moved the decks by 27 px — a browser test measures it.
//! - **The assistant suggests; it never switches.** [`suggest`] marks one
//!   activity with a reason. Moving the screen under somebody mid-set is §38's
//!   and §78's line, and an activity switch is the biggest move there is.

use serde::{Deserialize, Serialize};

use crate::cockpit::{Density, Dock, Focus, Placement, Workspace};

/// One activity, as the strip offers it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Activity {
    /// The stable name: what `ui activity <slug>` and the settings file say.
    pub slug: String,
    /// What the strip calls it. One word where one will do.
    pub title: String,
    /// What the DJ is doing in it, in their words.
    pub doing: String,
    /// A glyph the interface can draw — one of `ui/src/controls/icons.ts`,
    /// held by a test.
    pub icon: String,
    /// Whether djmanzo ships it. A DJ's own can be forgotten; these cannot.
    #[serde(default)]
    pub shipped: bool,
    /// The arrangement it opens.
    pub workspace: Workspace,
}

/// A panel in its home dock, in order. Shorthand for the table below.
fn at(surface: &str, dock: Dock, order: i32) -> Placement {
    Placement {
        surface: surface.to_owned(),
        dock,
        order,
        size: None,
        collapsed: false,
        pinned: false,
    }
}

/// A workspace for an activity: two decks, standard density, unlocked.
fn arrangement(
    name: &str,
    about: &str,
    surfaces: Vec<Placement>,
    focus: Focus,
    layout: &str,
) -> Workspace {
    Workspace {
        name: name.to_owned(),
        about: about.to_owned(),
        surfaces,
        density: Density::Standard,
        focus,
        theme: String::new(),
        decks: 2,
        layout: layout.to_owned(),
        locked: Vec::new(),
    }
}

/// The activities djmanzo ships, in the order the strip shows them and the
/// number keys reach them.
///
/// Eight, each built from surfaces and deck compositions that exist. The
/// karaoke host's is last on purpose: it waited for §107 to give it a surface
/// of its own — the singer rotation — rather than borrow the request queue and
/// call it karaoke, and putting it last kept the seven keys that were already
/// learned where they were.
#[must_use]
pub fn shipped() -> Vec<Activity> {
    let activity =
        |slug: &str, title: &str, doing: &str, icon: &str, workspace: Workspace| Activity {
            slug: slug.to_owned(),
            title: title.to_owned(),
            doing: doing.to_owned(),
            icon: icon.to_owned(),
            shipped: true,
            workspace,
        };
    let autopilot = crate::cockpit::workspaces()
        .into_iter()
        .find(|workspace| workspace.name == "Autopilot")
        .unwrap_or_else(|| {
            arrangement(
                "Autopilot",
                "",
                vec![at("assistant", Dock::Right, 0), at("next", Dock::Right, 1)],
                Focus::Supervising,
                "",
            )
        });
    vec![
        activity(
            "dig",
            "Dig",
            "Find the next record.",
            "magnifying-glass",
            arrangement(
                "Dig",
                "The collection open under the decks, and what djmanzo would play next beside them.",
                vec![at("library", Dock::Bottom, 0), at("next", Dock::Right, 0)],
                Focus::Preparing,
                "Essentials",
            ),
        ),
        activity(
            "mix",
            "Mix",
            "Blend into the next record.",
            "sliders",
            arrangement(
                "Mix",
                "The two decks and the mixer, and nothing else to look at.",
                Vec::new(),
                Focus::Performing,
                "Essentials",
            ),
        ),
        activity(
            "perform",
            "Perform",
            "Play with the record: pads, loops, effects and stems.",
            "hand",
            arrangement(
                "Perform",
                "Every pad, loop, effect and stem on the decks, and the sampler beside them.",
                vec![at("sampler", Dock::Right, 0)],
                Focus::Performing,
                "Stem Performance",
            ),
        ),
        activity(
            "prepare",
            "Prepare",
            "Set cues and grids, and shape tonight's plan.",
            "list",
            arrangement(
                "Prepare",
                "The record being readied beside the decks, and the night's plan under them.",
                vec![at("prepare", Dock::Right, 0), at("plan", Dock::Bottom, 0)],
                Focus::Preparing,
                "Essentials",
            ),
        ),
        activity(
            "requests",
            "Requests",
            "Answer what the room has asked for.",
            "hand-pointer",
            arrangement(
                "Requests",
                "The room's requests beside the decks, and the collection to find them in.",
                vec![
                    at("requests", Dock::Right, 0),
                    at("library", Dock::Bottom, 0),
                ],
                Focus::Preparing,
                "Essentials",
            ),
        ),
        activity(
            "autopilot",
            "Autopilot",
            "Let djmanzo play; watch, and take it back.",
            "robot",
            Workspace {
                name: "Autopilot".to_owned(),
                ..autopilot
            },
        ),
        activity(
            "practice",
            "Practice",
            "Work on a technique, between sets.",
            "book",
            arrangement(
                "Practice",
                "Two records as a laboratory, with the coach under them.",
                vec![at("practice", Dock::Bottom, 0)],
                Focus::Learning,
                "Starter",
            ),
        ),
        activity(
            "karaoke",
            "Karaoke",
            "Host the singers: who is up, what, and in which key.",
            "microphone",
            arrangement(
                "Karaoke",
                "The singer rotation beside the decks, and the collection to find their songs in.",
                vec![
                    at("karaoke", Dock::Right, 0),
                    at("library", Dock::Bottom, 0),
                ],
                Focus::Performing,
                "Essentials",
            ),
        ),
    ]
}

/// The key that reaches an activity, by its place in the strip: `F1` to `F9`.
///
/// **Not the number keys**, which was the first plan. djmanzo's default
/// keyboard puts the hot cues on `1`–`4` and `7`–`0` and clears them with Alt,
/// so a digit that switched the screen would be a DJ reaching for a cue and
/// losing the mixer. The function keys are free in the default map, sit in a
/// row of their own, and are where DJ software has long put its views. On a
/// Mac laptop they may need `fn`, which is the operating system's choice.
///
/// Nine and then nothing: a tenth activity is still one click away.
#[must_use]
pub fn key_for(index: usize) -> Option<String> {
    (index < 9).then(|| format!("F{}", index + 1))
}

/// The key that returns to the previous activity. Free in the default
/// keyboard, under the Escape key, and reachable without looking.
pub const BACK_KEY: &str = "Backquote";

/// Every activity the strip shows: djmanzo's, then the DJ's own.
#[must_use]
pub fn all(kept: &[Activity]) -> Vec<Activity> {
    let mut out = shipped();
    out.extend(kept.iter().cloned().map(|activity| Activity {
        shipped: false,
        ..activity
    }));
    out
}

/// The slug a title makes: lower case, words joined by hyphens, anything
/// else dropped.
#[must_use]
pub fn slug_of(title: &str) -> String {
    let mut slug = String::new();
    for word in title.split(|c: char| !c.is_alphanumeric()) {
        if word.is_empty() {
            continue;
        }
        if !slug.is_empty() {
            slug.push('-');
        }
        slug.extend(word.chars().flat_map(char::to_lowercase));
    }
    slug
}

/// Why a DJ's own activity was not kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// A name with nothing in it.
    Empty,
    /// A name one of djmanzo's already has. Two "Mix" tabs would be a DJ
    /// pressing one and finding out afterwards which.
    Shipped(String),
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refused::Empty => write!(f, "an activity needs a name"),
            Refused::Shipped(name) => {
                write!(
                    f,
                    "djmanzo already has an activity called {name:?} — choose another name"
                )
            }
        }
    }
}

/// Keep the arrangement on screen as an activity of the DJ's own.
///
/// A name the DJ already used replaces that one — keeping "Warm-up" twice is
/// a DJ improving their warm-up, not asking for two — and a name djmanzo
/// ships is refused.
///
/// # Errors
/// See [`Refused`].
pub fn keep(
    kept: &[Activity],
    title: &str,
    workspace: Workspace,
) -> Result<Vec<Activity>, Refused> {
    let title = title.trim();
    let slug = slug_of(title);
    if slug.is_empty() {
        return Err(Refused::Empty);
    }
    if shipped().iter().any(|activity| activity.slug == slug) {
        return Err(Refused::Shipped(title.to_owned()));
    }
    let mine = Activity {
        slug: slug.clone(),
        title: title.to_owned(),
        doing: "Your own arrangement.".to_owned(),
        icon: "flag".to_owned(),
        shipped: false,
        workspace: Workspace {
            name: title.to_owned(),
            ..workspace
        },
    };
    let mut out: Vec<Activity> = kept.to_vec();
    match out.iter_mut().find(|activity| activity.slug == slug) {
        Some(existing) => *existing = mine,
        None => out.push(mine),
    }
    Ok(out)
}

/// Forget one of the DJ's own. djmanzo's cannot be forgotten, so a shipped
/// slug changes nothing.
#[must_use]
pub fn forget(kept: &[Activity], slug: &str) -> Vec<Activity> {
    kept.iter()
        .filter(|activity| activity.slug != slug)
        .cloned()
        .collect()
}

/// What is kept between runs: the DJ's own activities, and where they were.
///
/// The mode is kept as well as the list, because a DJ who works in activities
/// and restarts djmanzo mid-evening should come back to the strip and the
/// activity they were in, not to the full cockpit they had turned away from.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Kept {
    #[serde(default)]
    pub mine: Vec<Activity>,
    /// Whether the interface is in activity mode.
    #[serde(default)]
    pub on: bool,
    /// The activity on screen, by slug. Empty before the first choice.
    #[serde(default)]
    pub current: String,
    /// The one before it, which the back key returns to.
    #[serde(default)]
    pub previous: String,
}

/// The activity djmanzo would suggest, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Suggestion {
    pub activity: String,
    /// The reason, in the DJ's terms — which is the whole of what makes a
    /// suggestion something other than the machine having an opinion.
    pub because: String,
}

/// How near the end of a record djmanzo starts to say "what's next", in
/// seconds of real time.
///
/// Ninety: long enough to find and load a record without rushing, short
/// enough that a DJ who is five minutes into a seven-minute record is not
/// nagged about it.
pub const NEAR_THE_END_SECONDS: f32 = 90.0;

/// Which activity the moment seems to call for, if any.
///
/// **A suggestion, never a switch.** The interface marks the activity and says
/// why; the DJ decides. In order, the first that applies:
///
/// 1. The automix is driving → *Autopilot*, where the takeover is.
/// 2. Two records are audible → *Mix*.
/// 3. One record is playing and near its end: *Mix* if another is loaded to
///    follow it, *Dig* if nothing is.
/// 4. The room has asked for something and one record is playing steadily →
///    *Requests*.
/// 5. Nothing is loaded at all → *Dig*.
///
/// Otherwise nothing: a quiet moment is the normal state, and a strip with a
/// suggestion lit all night is a strip nobody reads.
#[must_use]
pub fn suggest(snapshot: &crate::Snapshot, requests_waiting: usize) -> Option<Suggestion> {
    let say = |activity: &str, because: String| {
        Some(Suggestion {
            activity: activity.to_owned(),
            because,
        })
    };
    if snapshot.master.automix.enabled {
        return say(
            "autopilot",
            "djmanzo is driving the mix — this is where you watch it and take it back.".to_owned(),
        );
    }
    let playing: Vec<_> = snapshot.decks.iter().filter(|deck| deck.playing).collect();
    let audible = playing.iter().filter(|deck| deck.volume > 0.01).count();
    if audible >= 2 {
        return say("mix", "Two records are audible.".to_owned());
    }
    if let [deck] = playing.as_slice() {
        let rate = if deck.rate > 0.01 { deck.rate } else { 1.0 };
        let left = (deck.length_seconds - deck.position_seconds).max(0.0) / rate;
        if deck.length_seconds > 0.0 && left <= NEAR_THE_END_SECONDS {
            let waiting = snapshot
                .decks
                .iter()
                .any(|other| other.number != deck.number && other.loaded && !other.playing);
            let seconds = left.round();
            return if waiting {
                say(
                    "mix",
                    format!(
                        "{seconds:.0} seconds left on deck {} and the next record is loaded.",
                        deck.number
                    ),
                )
            } else {
                say(
                    "dig",
                    format!(
                        "{seconds:.0} seconds left on deck {} and nothing loaded to follow it.",
                        deck.number
                    ),
                )
            };
        }
        if requests_waiting > 0 {
            let asks = if requests_waiting == 1 {
                "One request is".to_owned()
            } else {
                format!("{requests_waiting} requests are")
            };
            return say("requests", format!("{asks} waiting."));
        }
    }
    if playing.is_empty() && !snapshot.decks.iter().any(|deck| deck.loaded) {
        return say("dig", "Nothing is loaded yet.".to_owned());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Every activity is made of things that exist.** A surface nothing
    /// draws, or a deck composition nothing answers to, would be an activity
    /// that opens half of itself and says nothing — the arrangement would
    /// apply around the hole.
    #[test]
    fn every_activity_is_built_from_surfaces_and_layouts_that_exist() {
        let layouts: Vec<String> = crate::layout::builtin()
            .into_iter()
            .map(|l| l.name)
            .collect();
        // Drawn, not merely declared. The Requests activity shipped placing
        // `requests`, which is declared and was never drawn: the arrangement
        // applied around the hole and the activity opened half of itself. A
        // check against the declarations passed it; this one would not have.
        let drawn = crate::cockpit::shell_draws();
        for activity in shipped() {
            for placement in &activity.workspace.surfaces {
                assert!(
                    drawn.contains(&placement.surface),
                    "{} opens {:?}, which the shell never draws",
                    activity.slug,
                    placement.surface
                );
                let surface = crate::cockpit::surface(&placement.surface).unwrap_or_else(|| {
                    panic!(
                        "{} opens {:?}, which is not a surface",
                        activity.slug, placement.surface
                    )
                });
                assert!(
                    surface.docks.contains(&placement.dock),
                    "{} puts {} where it cannot go",
                    activity.slug,
                    placement.surface
                );
            }
            let layout = &activity.workspace.layout;
            assert!(
                layout.is_empty() || layouts.contains(layout),
                "{} names the deck composition {layout:?}, which does not exist",
                activity.slug
            );
            assert!(
                activity.workspace.decks >= 2,
                "{} hides the decks",
                activity.slug
            );
        }
    }

    /// **Every glyph is one the interface can draw.** An unknown name draws
    /// its first letter, which on a strip of large icons would be one tab that
    /// looks broken.
    #[test]
    fn every_icon_is_one_the_interface_draws() {
        let icons = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../ui/src/controls/icons.ts"
        ))
        .expect("the interface's icon table");
        let drawn = |name: &str| icons.contains(&format!("\"{name}\": {{"));
        for activity in shipped() {
            assert!(
                drawn(&activity.icon),
                "{} uses the icon {:?}, which the interface does not draw",
                activity.slug,
                activity.icon
            );
        }
        let own = keep(&[], "Mine", crate::cockpit::opening()).expect("a name");
        assert!(
            drawn(&own[0].icon),
            "a DJ's own activity's icon is not drawn"
        );
    }

    /// Slugs are unique, and the first nine have a key each.
    #[test]
    fn each_activity_has_its_own_name_and_the_first_nine_a_key() {
        let all = shipped();
        for (i, a) in all.iter().enumerate() {
            for b in &all[i + 1..] {
                assert_ne!(a.slug, b.slug);
                assert_ne!(a.title, b.title);
            }
            assert_eq!(key_for(i), Some(format!("F{}", i + 1)));
        }
        assert_eq!(key_for(8).as_deref(), Some("F9"));
        assert_eq!(key_for(9), None, "a tenth activity has no key");
    }

    /// **The activity keys are keys the default keyboard does not use.** The
    /// first plan was the digits, which are the hot cues; this holds that the
    /// keys chosen instead stay free, so a later edit to the default map that
    /// took `F3` for something would fail here rather than make a DJ lose the
    /// mixer reaching for it.
    #[test]
    fn the_activity_keys_are_free_in_the_default_keyboard() {
        let map = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../dj-hid/mappings/keyboard-default.toml"
        ))
        .expect("the default keyboard");
        let taken: Vec<&str> = map
            .lines()
            .filter_map(|line| line.trim().strip_prefix("on = \""))
            .filter_map(|rest| rest.strip_suffix('"'))
            .map(|chord| chord.rsplit('+').next().unwrap_or(chord))
            .collect();
        assert!(
            taken.contains(&"Digit1"),
            "the reading of the map is wrong: {taken:?}"
        );
        for key in (0..9).filter_map(key_for).chain([BACK_KEY.to_owned()]) {
            assert!(
                !taken.contains(&key.as_str()),
                "{key} is bound in the default keyboard"
            );
        }
    }

    /// **A DJ's own activity is kept under their name, a shipped name is
    /// refused, and keeping the same name again replaces it.**
    #[test]
    fn keeping_and_forgetting_a_djs_own() {
        let workspace = crate::cockpit::opening();
        let kept = keep(&[], "  Warm-up set  ", workspace.clone()).unwrap();
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].slug, "warm-up-set");
        assert_eq!(kept[0].title, "Warm-up set");
        assert!(!kept[0].shipped);

        let again = keep(&kept, "warm-up SET", workspace.clone()).unwrap();
        assert_eq!(again.len(), 1, "the same name kept twice made two");

        assert_eq!(
            keep(&kept, "Mix", workspace.clone()),
            Err(Refused::Shipped("Mix".to_owned()))
        );
        assert_eq!(keep(&kept, " -- ", workspace), Err(Refused::Empty));

        assert!(forget(&again, "warm-up-set").is_empty());
        assert_eq!(
            forget(&again, "mix").len(),
            1,
            "forgetting a shipped slug changed the DJ's own"
        );
        assert_eq!(all(&again).len(), shipped().len() + 1);
    }

    /// One deck: loaded, playing, where it is and how long it is, in seconds.
    type Deck = (bool, bool, f32, f32);

    /// A frame with these decks, everything else as captured from a fresh
    /// registry.
    fn night(decks: &[Deck]) -> crate::Snapshot {
        let registry = dj_control::ParameterRegistry::new();
        registry.set(
            dj_core::ParamId::Global(dj_core::param::GlobalParam::SampleRate),
            48_000.0,
        );
        let mut snapshot = crate::Snapshot::capture(&registry, decks.len().max(1));
        for (deck, &(loaded, playing, at, length)) in snapshot.decks.iter_mut().zip(decks) {
            deck.loaded = loaded;
            deck.playing = playing;
            deck.position_seconds = at;
            deck.length_seconds = length;
            deck.rate = 1.0;
            deck.volume = 1.0;
        }
        snapshot
    }

    fn suggested(snapshot: &crate::Snapshot, requests: usize) -> Option<String> {
        suggest(snapshot, requests).map(|s| s.activity)
    }

    /// **The load-bearing one: near the end of a record, the suggestion is
    /// what the DJ is missing** — the mix if the next record is loaded, a dig
    /// if nothing is — and in the middle of one, nothing at all.
    #[test]
    fn near_the_end_it_suggests_what_is_missing_and_otherwise_stays_quiet() {
        let ending_alone = night(&[(true, true, 250.0, 300.0), (false, false, 0.0, 0.0)]);
        assert_eq!(suggested(&ending_alone, 0).as_deref(), Some("dig"));

        let ending_with_next = night(&[(true, true, 250.0, 300.0), (true, false, 0.0, 280.0)]);
        assert_eq!(suggested(&ending_with_next, 0).as_deref(), Some("mix"));

        let middle = night(&[(true, true, 60.0, 300.0), (true, false, 0.0, 280.0)]);
        assert_eq!(
            suggested(&middle, 0),
            None,
            "a record in its middle drew a suggestion"
        );
    }

    /// The pitch counts: at +25 % a record's remaining 110 seconds are 88
    /// real ones, and "time left" is real time.
    #[test]
    fn time_left_is_real_time_at_the_pitch_it_is_playing() {
        // 110 record seconds left: not near the end at normal speed...
        let mut snapshot = night(&[(true, true, 190.0, 300.0), (false, false, 0.0, 0.0)]);
        assert_eq!(suggested(&snapshot, 0), None);
        // ...and 88 real ones at +25 %.
        snapshot.decks[0].rate = 1.25;
        assert_eq!(
            suggested(&snapshot, 0).as_deref(),
            Some("dig"),
            "110 record seconds at 1.25 is 88 real ones"
        );
    }

    /// Two audible is a mix whatever else is true, and the automix outranks
    /// everything — it is where the takeover is.
    #[test]
    fn two_audible_is_a_mix_and_the_automix_outranks_everything() {
        let blending = night(&[(true, true, 60.0, 300.0), (true, true, 10.0, 280.0)]);
        assert_eq!(suggested(&blending, 3).as_deref(), Some("mix"));
        let mut driving = blending.clone();
        driving.master.automix.enabled = true;
        assert_eq!(suggested(&driving, 3).as_deref(), Some("autopilot"));
    }

    /// Requests are suggested only while one record plays steadily, and
    /// nothing loaded is a dig.
    #[test]
    fn requests_wait_for_a_steady_moment_and_an_empty_rig_is_a_dig() {
        let steady = night(&[(true, true, 60.0, 300.0), (false, false, 0.0, 0.0)]);
        let found = suggest(&steady, 2).unwrap();
        assert_eq!(found.activity, "requests");
        assert!(found.because.contains('2'), "{:?}", found.because);
        assert_eq!(suggested(&steady, 0), None);

        let empty = night(&[(false, false, 0.0, 0.0), (false, false, 0.0, 0.0)]);
        assert_eq!(suggested(&empty, 0).as_deref(), Some("dig"));
    }

    /// Every suggestion names an activity the strip has, and says why.
    #[test]
    fn every_suggestion_is_an_activity_with_a_reason() {
        let slugs: Vec<String> = shipped().into_iter().map(|a| a.slug).collect();
        let mut driving = night(&[]);
        driving.master.automix.enabled = true;
        let cases = [
            night(&[(true, true, 250.0, 300.0)]),
            night(&[(true, true, 250.0, 300.0), (true, false, 0.0, 1.0)]),
            night(&[(true, true, 1.0, 300.0), (true, true, 1.0, 300.0)]),
            night(&[(false, false, 0.0, 0.0)]),
            driving,
        ];
        for case in &cases {
            let found = suggest(case, 1).expect("each case suggests something");
            assert!(slugs.contains(&found.activity), "{found:?}");
            assert!(found.because.len() > 10, "{found:?}");
        }
    }
}
