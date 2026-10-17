//! §130/§131: the one place a command is refused for the edition.
//!
//! Every DJ has the same download (ADR-0011), so "is this here?" is asked at
//! run time, of the licence, at one boundary: every command the interface
//! sends passes [`guard`], which looks the command up in [`MODULE_OF`] and
//! refuses it, with the edition that has it, when the licence in force does
//! not carry that module.
//!
//! # What is gated, and what is not
//!
//! **What a command *does* is gated; what it *reads* is not.** Starting the
//! singers' screen is karaoke's; asking whether it is running is not, because
//! the interface draws the quiet card that says what karaoke is from the same
//! answers it would draw the panel from. A table of reads would also be a
//! table of ways to break a panel in an edition nobody tested.
//!
//! Actions on the bus (`deck 3 play`, `deck 1 stem vocal mute`) carry their
//! module in their shape rather than their name, so [`action_module`] reads
//! it from there, and `commands::perform_action_by` asks before any action
//! reaches the engine — whoever sent it: a hand, a controller, the autopilot.
//!
//! # With no licence
//!
//! Nothing is refused. Whether a djmanzo nobody has signed in to is held to
//! something less is the owner's decision (`docs/EDITIONS.md`, *The 7-day
//! trial*); until it is made, [`UNLICENSED`] is everything, so this boundary
//! changes nothing for anyone without a licence and holds a licensed edition
//! to what it carries.

use crate::AppState;
use dj_core::edition::{Edition, Module, Modules};
use tauri::Manager as _;

/// What a djmanzo with no licence in force may use. Everything, until the
/// owner says otherwise.
pub const UNLICENSED: Modules = Modules::all();

/// Commands that belong to a module, by the name the interface invokes them
/// with. A command not here is the foundation's, or only reads.
pub const MODULE_OF: &[(&str, Module)] = &[
    // Karaoke: the queue, the singers' screen, their microphones and words.
    ("karaoke_take", Module::Karaoke),
    ("karaoke_line", Module::Karaoke),
    ("karaoke_bring_in", Module::Karaoke),
    ("karaoke_verdict", Module::Karaoke),
    ("karaoke_versus", Module::Karaoke),
    ("karaoke_refuse", Module::Karaoke),
    ("karaoke_ask", Module::Karaoke),
    ("karaoke_sang", Module::Karaoke),
    ("karaoke_not_here", Module::Karaoke),
    ("karaoke_move", Module::Karaoke),
    ("karaoke_leave", Module::Karaoke),
    ("karaoke_key", Module::Karaoke),
    ("karaoke_clear", Module::Karaoke),
    ("karaoke_load", Module::Karaoke),
    ("karaoke_record_song", Module::Karaoke),
    ("karaoke_range_start", Module::Karaoke),
    ("karaoke_breaks_set", Module::Karaoke),
    ("sing_rules_set", Module::Karaoke),
    ("sing_photo_approve", Module::Karaoke),
    ("guests_save", Module::Karaoke),
    ("guests_song", Module::Karaoke),
    ("guests_message", Module::Karaoke),
    ("quiz_set", Module::Karaoke),
    ("quiz_screen", Module::Karaoke),
    ("word_timing_run", Module::Karaoke),
    ("word_timing_download", Module::Karaoke),
    ("audience_start", Module::Karaoke),
    ("vocals_open", Module::Karaoke),
    ("open_suno", Module::Karaoke),
    ("suno_fill", Module::Karaoke),
    // Stems out to their own outputs. Stems on a deck are actions.
    ("set_stem_out", Module::Stems),
    // The autopilot taking the decks.
    ("assistant_take_over", Module::Autopilot),
    // The assistant.
    ("ask", Module::Assistant),
    ("assistant_conduct", Module::Assistant),
    ("assistant_apply_pack", Module::Assistant),
    ("staged_prepare", Module::Assistant),
    ("setlist_build", Module::Assistant),
    ("setlist_steer", Module::Assistant),
    // The teacher.
    ("practice_rehearse", Module::Teacher),
    ("coach_report", Module::Teacher),
    ("learning_mark", Module::Teacher),
    ("practice_live_load", Module::Teacher),
    // Reading the room.
    ("set_camera_wanted", Module::Crowd),
    ("phone_camera_new_link", Module::Crowd),
    ("crowd_add", Module::Crowd),
    ("crowd_import", Module::Crowd),
    // Controllers beyond a shipped mapping.
    ("mapping_learn", Module::Controllers),
    ("mapping_bind", Module::Controllers),
    ("mapping_save", Module::Controllers),
    ("open_hid_controller", Module::Controllers),
    // Timecode vinyl.
    ("start_timecode", Module::Dvs),
    ("write_timecode_signal", Module::Dvs),
    // Video and visuals.
    ("look_choose", Module::Video),
    ("look_ai", Module::Video),
    ("milkdrop_add", Module::Video),
    // Going live.
    ("set_live", Module::Streaming),
    ("obs_stream", Module::Streaming),
    ("youtube_sign_in", Module::Streaming),
    ("share_to", Module::Streaming),
    // Plugins.
    ("load_plugin", Module::Plugins),
    // Online sources.
    ("search_sources", Module::Sources),
    ("resolve_source_track", Module::Sources),
    ("open_store", Module::Sources),
    // Events.
    ("new_event", Module::Events),
    ("save_event", Module::Events),
    ("set_live_event", Module::Events),
    ("kit_save", Module::Events),
    ("kit_send", Module::Events),
    // Replay.
    ("session_render", Module::Replay),
    ("session_render_mix", Module::Replay),
];

/// The module `command` belongs to, if it belongs to one.
#[must_use]
pub fn module_of(command: &str) -> Option<Module> {
    MODULE_OF
        .iter()
        .find(|(name, _)| *name == command)
        .map(|(_, m)| *m)
}

/// Why `module` is refused: what it is and the cheapest edition that has it.
#[must_use]
pub fn refusal(module: Module) -> String {
    format!(
        "{} is in {} and in Full.",
        module.title(),
        Edition::cheapest_with(module).product_name()
    )
}

/// Whether `command` may run with `modules` in force; the sentence why not.
///
/// # Errors
/// The command belongs to a module `modules` does not carry.
pub fn allowed(command: &str, modules: Modules) -> Result<(), String> {
    match module_of(command) {
        Some(module) if !modules.has(module) => Err(refusal(module)),
        _ => Ok(()),
    }
}

/// The module an action on the bus belongs to, if any: decks past the
/// second are [`Module::FourDecks`], a stem moved on a deck or swapped
/// between decks is [`Module::Stems`].
#[must_use]
pub fn action_module(action: &dj_core::Action) -> Option<Module> {
    use dj_core::{Action, DeckAction, MixerAction};
    match action {
        Action::Deck {
            action: DeckAction::Stem { .. },
            ..
        }
        | Action::Mixer(MixerAction::StemSwap { .. } | MixerAction::StemSwapOff) => {
            Some(Module::Stems)
        }
        Action::Deck { deck, .. } if deck.index() >= 2 => Some(Module::FourDecks),
        _ => None,
    }
}

/// Whether `action` may reach the engine with `modules` in force.
///
/// # Errors
/// It belongs to a module `modules` does not carry.
pub fn allowed_action(action: &dj_core::Action, modules: Modules) -> Result<(), String> {
    match action_module(action) {
        Some(module) if !modules.has(module) => Err(refusal(module)),
        _ => Ok(()),
    }
}

/// Put the edition's boundary in front of `inner`, the handler
/// `tauri::generate_handler!` builds.
pub fn guard<R: tauri::Runtime>(
    inner: impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static,
) -> impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static {
    move |invoke| {
        if module_of(invoke.message.command()).is_some() {
            let modules = invoke
                .message
                .webview()
                .try_state::<AppState>()
                .map_or(UNLICENSED, |state| state.licence().in_force());
            if let Err(why) = allowed(invoke.message.command(), modules) {
                invoke.resolver.reject(why);
                return true;
            }
        }
        inner(invoke)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The commands the application registers, read from `lib.rs` itself, by
    /// the name the interface invokes them with.
    fn registered() -> Vec<String> {
        let source = include_str!("lib.rs");
        let start = source.find("generate_handler![").expect("the handler list");
        let end = start + source[start..].find("])").expect("its end");
        source[start..end]
            .lines()
            .filter_map(|line| {
                let word = line.trim().strip_suffix(',')?;
                (word.contains("::") && !word.starts_with("//"))
                    .then(|| word.rsplit("::").next().unwrap_or(word).to_owned())
            })
            .collect()
    }

    /// **Every gated command exists.** A command renamed or removed would
    /// otherwise leave its row here gating nothing, silently — the failure
    /// `dj_app::tiers` already guards against for verbs.
    #[test]
    fn every_gated_command_is_one_the_application_registers() {
        let registered = registered();
        assert!(
            registered.len() > 400,
            "read {} commands from lib.rs",
            registered.len()
        );
        for (name, _) in MODULE_OF {
            assert!(
                registered.iter().any(|r| r == name),
                "{name} is gated and not registered"
            );
        }
    }

    /// **No command is gated twice**, so its module is one answer.
    #[test]
    fn no_command_is_gated_twice() {
        let mut names: Vec<&str> = MODULE_OF.iter().map(|(n, _)| *n).collect();
        names.sort_unstable();
        let before = names.len();
        names.dedup();
        assert_eq!(names.len(), before);
    }

    /// **The licence's own commands are never gated**: a DJ must always be
    /// able to sign in, paste a licence and sign out.
    #[test]
    fn the_licence_is_always_reachable() {
        for name in [
            "licence_status",
            "licence_install",
            "licence_sign_out",
            "dispatch",
            "get_snapshot",
        ] {
            assert_eq!(module_of(name), None, "{name}");
        }
    }

    /// **A Karaoke licence runs karaoke and is refused the teacher**, with
    /// the edition that has it; Full runs both; the foundation's commands run
    /// in every edition.
    #[test]
    fn an_edition_runs_what_it_carries_and_is_told_where_the_rest_is() {
        let karaoke = Edition::Karaoke.modules();
        assert_eq!(allowed("karaoke_take", karaoke), Ok(()));
        assert_eq!(
            allowed("practice_rehearse", karaoke),
            Err("Teacher is in djmanzo Apprentice and in Full.".to_owned())
        );
        assert_eq!(
            allowed("practice_rehearse", Edition::Full.modules()),
            Ok(())
        );
        for name in ["load_track", "library_search", "open_device"] {
            assert_eq!(allowed(name, Edition::Hobbyist.modules()), Ok(()), "{name}");
        }
    }

    /// **Every module has at least one command at the boundary**, except
    /// four decks, which is only actions on the bus ([`action_module`]).
    /// A module with nothing gated would be sold and never withheld.
    #[test]
    fn every_module_is_held_somewhere() {
        for module in Module::ALL {
            let gated = MODULE_OF.iter().any(|(_, m)| *m == module);
            assert!(
                gated || module == Module::FourDecks,
                "{} has no command at the boundary",
                module.slug()
            );
        }
    }

    /// **Decks three and four, and stems, are refused on the bus** in an
    /// edition without them — a Hobbyist's deck 3 and a Karaoke edition's
    /// stem mute — and run in Pro DJ; the first two decks run everywhere.
    #[test]
    fn the_bus_holds_decks_and_stems_to_the_edition() {
        let parse = |text: &str| dj_core::Action::parse(text).unwrap();
        let hobbyist = Edition::Hobbyist.modules();
        assert_eq!(allowed_action(&parse("deck 1 play"), hobbyist), Ok(()));
        assert_eq!(allowed_action(&parse("deck 2 play"), hobbyist), Ok(()));
        assert_eq!(
            allowed_action(&parse("deck 3 play"), hobbyist),
            Err("Four decks is in djmanzo Pro DJ and in Full.".to_owned())
        );
        assert!(
            allowed_action(&parse("deck 1 stem_mute vocal"), Edition::Karaoke.modules()).is_err()
        );
        assert!(allowed_action(&parse("stem_swap vocal 1 2"), hobbyist).is_err());
        let pro = Edition::ProDj.modules();
        for text in [
            "deck 3 play",
            "deck 1 stem_mute vocal",
            "stem_swap vocal 1 2",
        ] {
            assert_eq!(allowed_action(&parse(text), pro), Ok(()), "{text}");
            assert_eq!(allowed_action(&parse(text), UNLICENSED), Ok(()), "{text}");
        }
    }

    /// **With no licence nothing is refused**, until the owner decides
    /// otherwise.
    #[test]
    fn nothing_is_refused_without_a_licence() {
        for (name, _) in MODULE_OF {
            assert_eq!(allowed(name, UNLICENSED), Ok(()), "{name}");
        }
    }
}
