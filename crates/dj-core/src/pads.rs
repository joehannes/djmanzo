//! The pad zone: eight pads, and what each page puts on them.
//!
//! A page is **a mapping from pad number to action**, and nothing more. That is
//! the whole design, and it is deliberately small, because the same table has
//! two consumers that must not disagree: the eight buttons on screen and the
//! eight rubber pads on a controller. Writing the mapping twice is how a DJ
//! ends up with a pad that does one thing under their finger and another thing
//! on the display.
//!
//! It lives here rather than in the interface for the same reason the rest of
//! [ADR-0003](../../docs/adr/0003-action-bus-and-parameter-registry.md) does: a
//! pad press is an action, and there is one action vocabulary.
//!
//! What a page does *not* own is whether a pad is lit. A pad lights from live
//! state — a cue that has been set, a roll being held, an effect switched on —
//! and that state arrives in the snapshot. So each pad names the *condition* it
//! watches, in [`Lit`], and the interface evaluates that one enum rather than
//! carrying a branch per page.

use crate::action::{DeckAction, MixerAction};
use crate::fx::{EffectKind, FxChange};
use crate::hotcue::HOT_CUE_SLOTS;
use crate::sampler::SampleChange;
use serde::{Deserialize, Serialize};

/// Pads per page.
///
/// Eight because that is what the hardware has — every controller worth mapping
/// has two rows of four — and because eight hot cues is already the number
/// [`HOT_CUE_SLOTS`] settled on. A page with a different number would be a page
/// that cannot be played from a controller.
pub const PADS: usize = 8;

const _: () = assert!(PADS == HOT_CUE_SLOTS);

/// Which set of eight is on the pads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PadPage {
    #[default]
    Cues,
    Loops,
    Roll,
    /// Eight equal parts of a span of the grid, each one a pad.
    ///
    /// The page a DJ reaches for to rearrange a bar on the fly. Needs a grid
    /// for the same reason the loop pages do: without one there is nothing to
    /// divide.
    Slicer,
    Saved,
    Sampler,
    Fx,
}

impl PadPage {
    /// Every page, in the order they are offered.
    pub const ALL: [PadPage; 7] = [
        PadPage::Cues,
        PadPage::Loops,
        PadPage::Roll,
        PadPage::Slicer,
        PadPage::Saved,
        PadPage::Sampler,
        PadPage::Fx,
    ];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            PadPage::Cues => "cues",
            PadPage::Loops => "loops",
            PadPage::Roll => "roll",
            PadPage::Slicer => "slicer",
            PadPage::Saved => "saved",
            PadPage::Sampler => "sampler",
            PadPage::Fx => "fx",
        }
    }

    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|page| page.name() == name)
    }

    /// Whether this page needs a beat grid to mean anything.
    ///
    /// Loops and rolls are measured in beats, so on a track the analyser could
    /// not read they would be eight pads that do nothing. Cues are positions
    /// and need no grid at all — which is why cues are the default page.
    #[must_use]
    pub const fn needs_grid(self) -> bool {
        matches!(self, PadPage::Loops | PadPage::Roll | PadPage::Slicer)
    }

    /// The eight pads of this page.
    #[must_use]
    pub fn pads(self) -> [Pad; PADS] {
        // Wrappers, so the tables below read as tables rather than as nested
        // constructors. `d` is a deck action, `m` a mixer one.
        fn d(action: DeckAction) -> Option<PadAction> {
            Some(PadAction::Deck(action))
        }
        fn m(action: MixerAction) -> Option<PadAction> {
            Some(PadAction::Mixer(action))
        }

        match self {
            PadPage::Cues => std::array::from_fn(|index| {
                let slot = index as u8 + 1;
                Pad {
                    label: PadLabel::Number(slot),
                    press: d(DeckAction::HotCue(slot)),
                    release: None,
                    clear: d(DeckAction::HotCueClear(slot)),
                    lit: Lit::HotCueSet(slot),
                }
            }),
            // Doubling from a sixteenth: the eight lengths a DJ actually
            // reaches for, and each pad is twice the one before it, so the
            // grid reads as a scale rather than as a list.
            PadPage::Loops => std::array::from_fn(|index| {
                let beats = beat_ladder(index);
                Pad {
                    label: PadLabel::Beats(beats),
                    press: d(DeckAction::LoopBeats(beats)),
                    release: None,
                    // A second press on a running loop leaves it, which is what
                    // the same pad twice should mean.
                    clear: d(DeckAction::LoopOff),
                    lit: Lit::LoopBeats(beats),
                }
            }),
            PadPage::Roll => std::array::from_fn(|index| {
                let beats = beat_ladder(index);
                Pad {
                    label: PadLabel::Beats(beats),
                    press: d(DeckAction::LoopRoll(Some(beats))),
                    // Momentary: the roll ends when the finger lifts.
                    release: d(DeckAction::LoopRoll(None)),
                    clear: None,
                    lit: Lit::RollBeats(beats),
                }
            }),
            PadPage::Slicer => std::array::from_fn(|index| {
                let slice = index as u8 + 1;
                Pad {
                    label: PadLabel::Number(slice),
                    press: d(DeckAction::Slice(Some(slice))),
                    // Momentary, like the roll: the slice ends when the finger
                    // lifts and the track is where it would have been.
                    release: d(DeckAction::Slice(None)),
                    clear: None,
                    // Lit by where the playhead *is*, not by what was pressed.
                    // The light walks the grid on its own, which is what makes
                    // the page readable before you touch it — you can see which
                    // slice is coming.
                    lit: Lit::SliceAt(slice),
                }
            }),
            PadPage::Saved => std::array::from_fn(|index| {
                let slot = index as u8 + 1;
                Pad {
                    label: PadLabel::Number(slot),
                    press: d(DeckAction::LoopRecall(slot)),
                    release: None,
                    // Saving is the destructive gesture, so it is the modified
                    // one — the same arrangement the browser uses.
                    clear: d(DeckAction::LoopSave(slot)),
                    lit: Lit::Never,
                }
            }),
            // The one page whose pads address the mixer rather than the deck.
            // The sampler is shared across the whole mixer — a sample does not
            // belong to a deck — but the pads that fire it are the deck's, the
            // same as on hardware. Both are always true and the pad model has
            // to allow it; see [`PadAction`].
            //
            // Release is sent whatever the mode, because the *slot* decides
            // whether a release means anything. A pad cannot know: the mode is
            // a setting the DJ changes, and baking it into the table would
            // freeze it at whatever it was when the page was read.
            PadPage::Sampler => std::array::from_fn(|index| {
                let slot = index as u8 + 1;
                Pad {
                    label: PadLabel::Number(slot),
                    press: m(MixerAction::Sample {
                        slot,
                        change: SampleChange::Trigger,
                    }),
                    release: m(MixerAction::Sample {
                        slot,
                        change: SampleChange::Release,
                    }),
                    clear: m(MixerAction::Sample {
                        slot,
                        change: SampleChange::Stop,
                    }),
                    lit: Lit::SamplePlaying(slot),
                }
            }),
            // Three slots, so the page is a switch and a select per slot rather
            // than eight of anything. The last two pads are spare and say so.
            PadPage::Fx => std::array::from_fn(|index| match index {
                0..=2 => {
                    let slot = index as u8 + 1;
                    Pad {
                        label: PadLabel::FxSlot(slot),
                        press: d(DeckAction::Fx {
                            slot,
                            change: FxChange::ToggleEnabled,
                        }),
                        release: None,
                        clear: d(DeckAction::Fx {
                            slot,
                            change: FxChange::Select(EffectKind::None),
                        }),
                        lit: Lit::FxSlotOn(slot),
                    }
                }
                3..=5 => {
                    let slot = index as u8 - 2;
                    Pad {
                        label: PadLabel::FxPlace(slot),
                        press: d(DeckAction::Fx {
                            slot,
                            change: FxChange::Place(crate::fx::Placement::PostFader),
                        }),
                        release: None,
                        clear: d(DeckAction::Fx {
                            slot,
                            change: FxChange::Place(crate::fx::Placement::PreFader),
                        }),
                        lit: Lit::FxSlotPost(slot),
                    }
                }
                _ => Pad::empty(),
            }),
        }
    }
}

/// The loop and roll ladder: 1/16 doubling to 8 beats.
fn beat_ladder(index: usize) -> f32 {
    // `powi` rather than a table, so the ladder is visibly a doubling and
    // cannot be mistyped in the middle.
    2.0_f32.powi(index as i32 - 4)
}

/// What a pad sends.
///
/// Either kind of action, because a pad is a physical button and the things a
/// DJ reaches for on one are not all on the deck: the sampler is shared across
/// the whole mixer, and a Sampler page whose pads could only address a deck
/// would be a page that cannot fire a sample.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PadAction {
    Deck(DeckAction),
    Mixer(MixerAction),
}

/// One pad.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pad {
    pub label: PadLabel,
    /// Sent on press. `None` for a pad this page leaves blank — an absence
    /// rather than a do-nothing action, because a verb that does nothing would
    /// have to exist in the vocabulary and be explained to everything that
    /// reads it.
    pub press: Option<PadAction>,
    /// Sent on release, for a momentary pad. `None` means the pad latches.
    pub release: Option<PadAction>,
    /// Sent on the secondary gesture — right-click on screen, shift on
    /// hardware. `None` when the pad has no second meaning.
    pub clear: Option<PadAction>,
    /// What makes this pad light up.
    pub lit: Lit,
}

impl Pad {
    /// A pad this page leaves blank.
    ///
    /// A real pad with nothing on it rather than an `Option`, so a page is
    /// always eight pads and the interface never has to lay out a ragged grid.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            label: PadLabel::Blank,
            press: None,
            release: None,
            clear: None,
            lit: Lit::Never,
        }
    }

    #[must_use]
    pub const fn is_blank(&self) -> bool {
        self.press.is_none()
    }
}

/// What a pad says on it.
///
/// Structured rather than a string so the interface can format a beat count its
/// own way — "1/4" beside the loop controls, "0.25" in a log — and so a
/// controller with a character display gets the number rather than the glyphs.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum PadLabel {
    Blank,
    /// A slot number: a hot cue, a saved loop.
    Number(u8),
    /// A length in beats.
    Beats(f32),
    /// Effect slot `n`'s switch.
    FxSlot(u8),
    /// Effect slot `n`'s placement.
    FxPlace(u8),
}

/// What makes a pad light.
///
/// One enum the interface evaluates against the snapshot, rather than a branch
/// per page. Adding a page then adds rows to a table instead of arms to a
/// switch somewhere else — which is the difference between a page being data
/// and a page being code in two places.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Lit {
    /// Never lights. A pad whose state cannot be known — a saved loop slot the
    /// engine does not report, a blank.
    Never,
    /// This hot cue has been set.
    HotCueSet(u8),
    /// A loop of this length is running.
    LoopBeats(f32),
    /// A roll of this length is being held.
    RollBeats(f32),
    /// This effect slot is switched on.
    FxSlotOn(u8),
    /// This effect slot sits after the fader.
    FxSlotPost(u8),
    /// This sampler slot is sounding.
    SamplePlaying(u8),
    /// The playhead is inside this slice of the slicer's domain.
    SliceAt(u8),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_page_offers_exactly_eight_pads() {
        for page in PadPage::ALL {
            assert_eq!(page.pads().len(), PADS, "{}", page.name());
        }
    }

    #[test]
    fn every_page_round_trips_through_its_name() {
        for page in PadPage::ALL {
            assert_eq!(PadPage::parse(page.name()), Some(page));
        }
        assert_eq!(PadPage::parse("stems"), None);
    }

    /// The cue page is the default because it is the only one that works on a
    /// track the analyser could not read. A default that needs a grid would
    /// leave a DJ looking at eight dead pads on an unanalysed track.
    #[test]
    fn the_default_page_needs_no_grid() {
        assert_eq!(PadPage::default(), PadPage::Cues);
        assert!(!PadPage::default().needs_grid());
        assert!(PadPage::Loops.needs_grid());
        assert!(PadPage::Roll.needs_grid());
        assert!(!PadPage::Saved.needs_grid());
        assert!(!PadPage::Sampler.needs_grid());
        assert!(!PadPage::Fx.needs_grid());
    }

    #[test]
    fn the_cue_page_is_the_eight_hot_cues_in_order() {
        let pads = PadPage::Cues.pads();
        for (index, pad) in pads.iter().enumerate() {
            let slot = index as u8 + 1;
            assert_eq!(pad.press, Some(PadAction::Deck(DeckAction::HotCue(slot))));
            assert_eq!(
                pad.clear,
                Some(PadAction::Deck(DeckAction::HotCueClear(slot)))
            );
            assert_eq!(pad.lit, Lit::HotCueSet(slot));
            assert_eq!(pad.label, PadLabel::Number(slot));
        }
    }

    /// Each pad twice the one before it, from a sixteenth to eight beats. The
    /// ladder is the point: a DJ reads the grid as a scale, and halving or
    /// doubling is one pad left or right.
    #[test]
    fn the_loop_ladder_doubles_across_the_grid() {
        let expected = [0.0625, 0.125, 0.25, 0.5, 1.0, 2.0, 4.0, 8.0];
        for (pad, want) in PadPage::Loops.pads().iter().zip(expected) {
            assert_eq!(pad.label, PadLabel::Beats(want));
            assert_eq!(
                pad.press,
                Some(PadAction::Deck(DeckAction::LoopBeats(want)))
            );
        }
        // And the roll page walks the same ladder, so a DJ who has learnt one
        // has learnt the other.
        for (pad, want) in PadPage::Roll.pads().iter().zip(expected) {
            assert_eq!(
                pad.press,
                Some(PadAction::Deck(DeckAction::LoopRoll(Some(want))))
            );
        }
    }

    /// A roll ends when the finger lifts; a loop does not. That difference is
    /// the whole reason they are two pages rather than one.
    #[test]
    fn only_the_roll_page_is_momentary() {
        for pad in PadPage::Roll.pads() {
            assert!(
                pad.release.is_some(),
                "a roll pad must send something on release"
            );
        }
        // The sampler is the exception, and deliberately: its pads always
        // send a release and the slot decides what to do with it.
        for page in [PadPage::Cues, PadPage::Loops, PadPage::Saved, PadPage::Fx] {
            for pad in page.pads() {
                assert!(
                    pad.release.is_none(),
                    "{} should latch, not be held",
                    page.name()
                );
            }
        }
    }

    /// A pad that does nothing must still be a pad, so the grid is never
    /// ragged — and it must not claim a light or a secondary gesture.
    #[test]
    fn a_blank_pad_is_inert_in_every_direction() {
        let blank = Pad::empty();
        assert!(blank.is_blank());
        assert_eq!(blank.press, None);
        assert_eq!(blank.release, None);
        assert_eq!(blank.clear, None);
        assert_eq!(blank.lit, Lit::Never);

        let fx = PadPage::Fx.pads();
        assert!(fx[6].is_blank() && fx[7].is_blank(), "the spare two");
        assert!(!fx[0].is_blank());
    }

    /// The Sampler page is the one whose pads address the mixer rather than
    /// the deck, and the reason the pad model allows both. A page that could
    /// only speak to a deck could not fire a sample.
    #[test]
    fn the_sampler_page_addresses_the_mixer() {
        for (index, pad) in PadPage::Sampler.pads().iter().enumerate() {
            let slot = index as u8 + 1;
            assert_eq!(
                pad.press,
                Some(PadAction::Mixer(MixerAction::Sample {
                    slot,
                    change: SampleChange::Trigger,
                }))
            );
            assert_eq!(pad.lit, Lit::SamplePlaying(slot));
        }
        // And every other page still speaks to its deck.
        for page in [PadPage::Cues, PadPage::Loops, PadPage::Roll, PadPage::Saved] {
            for pad in page.pads() {
                assert!(
                    matches!(pad.press, Some(PadAction::Deck(_))),
                    "{} should address the deck",
                    page.name()
                );
            }
        }
    }

    /// The sampler page sends a release whatever the pad's mode, because the
    /// *slot* decides whether a release means anything — the mode is a setting
    /// the DJ changes, and baking it into the table would freeze it at
    /// whatever it was when the page was read.
    #[test]
    fn the_sampler_page_always_sends_a_release() {
        for pad in PadPage::Sampler.pads() {
            assert!(pad.release.is_some());
        }
    }

    /// Every pad that can be pressed has to say what it says, or the interface
    /// draws an unlabelled button.
    #[test]
    fn every_usable_pad_carries_a_label() {
        for page in PadPage::ALL {
            for (index, pad) in page.pads().iter().enumerate() {
                if pad.press.is_none() {
                    assert!(
                        pad.is_blank(),
                        "{} pad {index} does nothing but is labelled",
                        page.name()
                    );
                } else {
                    assert!(
                        !pad.is_blank(),
                        "{} pad {index} does something but has no label",
                        page.name()
                    );
                }
            }
        }
    }
}
